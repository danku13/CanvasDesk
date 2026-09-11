//! Инлайн-редактирование текстовой заметки (T7) на готовом cosmic-text Editor.
//!
//! Свой text editing state не пишем: курсор, выделение, word-jump и hit-test
//! клика даёт `cosmic_text::Editor` (та же версия, что тянет glyphon).
//! Модуль — чистая CPU-логика (без wgpu), поэтому тестируется без GPU.
//!
//! Сессия владеет `Buffer` напрямую, а `Editor` собирается на каждую операцию
//! через `BufferRef::Borrowed` — так буфер доступен рендеру как `&Buffer`
//! без unsafe, а курсор/выделение сохраняются полями сессии между операциями.
//!
//! Координаты клика/каретки/выделения — в пикселях буфера редактора
//! (физические px относительно левого верхнего угла области редактирования).

use canvas_core::{edge_midpoint, Canvas};
use cosmic_text::{Action, Buffer, Cursor, Edit, Editor, FontSystem, Metrics, Motion, Selection};
use glyphon::{Attrs, Shaping, Wrap};
use winit::keyboard::{Key, NamedKey};

use crate::markdown::{self, StyleFlag, StyleSpan};
use crate::text::{body_area, rich_spans, BODY_FONT_SIZE, BODY_LINE_HEIGHT};

/// Маркер форматирования текста заметки (пост-T7): markdown-подмножество,
/// см. markdown.rs. Хоткеи Ctrl+B/I/H тогглят стиль выделения (WYSIWYG:
/// маркеров в буфере редактора нет — только чистый текст + спаны).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// `**жирный**` (Ctrl+B).
    Bold,
    /// `*курсив*` (Ctrl+I).
    Italic,
    /// `==подсветка==` (Ctrl+H).
    Highlight,
}

impl Marker {
    fn flag(self) -> StyleFlag {
        match self {
            Marker::Bold => StyleFlag::Bold,
            Marker::Italic => StyleFlag::Italic,
            Marker::Highlight => StyleFlag::Highlight,
        }
    }
}

/// Курсор (строка, байтовый индекс) → линейный байтовый offset в тексте,
/// где строки соединены '\n' (формат `EditingSession::text()`).
fn cursor_to_offset(text: &str, cursor: Cursor) -> usize {
    let mut offset = 0usize;
    for (i, line) in text.split('\n').enumerate() {
        if i == cursor.line {
            return offset + cursor.index.min(line.len());
        }
        offset += line.len() + 1;
    }
    text.len()
}

/// Регион правки между старым и новым текстом: (prefix, before, after) —
/// длина общего префикса, заменённого хвоста старого и нового текста.
/// Границы корректируются до границ UTF-8 (байтовый diff может разрезать
/// многобайтовый символ: "а" (D0 B0) vs "п" (D0 BF) имеют общий префикс
/// длиной 1 байт).
fn edit_region(old: &str, new: &str) -> (usize, usize, usize) {
    let (old_b, new_b) = (old.as_bytes(), new.as_bytes());
    let mut prefix = 0usize;
    while prefix < old_b.len().min(new_b.len()) && old_b[prefix] == new_b[prefix] {
        prefix += 1;
    }
    while prefix > 0 && !old.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let max_suffix = (old_b.len() - prefix).min(new_b.len() - prefix);
    let mut suffix = 0usize;
    while suffix < max_suffix && old_b[old_b.len() - 1 - suffix] == new_b[new_b.len() - 1 - suffix]
    {
        suffix += 1;
    }
    while suffix > 0 && !old.is_char_boundary(old.len() - suffix) {
        suffix -= 1;
    }
    (
        prefix,
        old.len() - prefix - suffix,
        new.len() - prefix - suffix,
    )
}

/// «Липкие» флаги ввода: Ctrl+B без выделения переключает флаг, и
/// последующий ввод вставляется уже стилизованным (до повторного тогла).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct PendingStyle {
    bold: bool,
    italic: bool,
    highlight: bool,
}

impl PendingStyle {
    fn toggle(&mut self, flag: StyleFlag) {
        match flag {
            StyleFlag::Bold => self.bold = !self.bold,
            StyleFlag::Italic => self.italic = !self.italic,
            StyleFlag::Highlight => self.highlight = !self.highlight,
        }
    }
    fn any(&self) -> bool {
        self.bold || self.italic || self.highlight
    }
    fn get(self, flag: StyleFlag) -> bool {
        match flag {
            StyleFlag::Bold => self.bold,
            StyleFlag::Italic => self.italic,
            StyleFlag::Highlight => self.highlight,
        }
    }
}

/// Цель инлайн-редактирования: тело текстовой ноды (T7) или лейбл связи (T8).
/// Индексы — позиции в `canvas.nodes` / `canvas.edges`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditTarget {
    Node(usize),
    Edge(usize),
}

/// Ширина бокса редактирования лейбла связи в world-px (T8).
pub const EDGE_EDIT_WIDTH: f32 = 240.0;
/// Высота бокса редактирования лейбла связи в world-px (строка + отступы).
pub const EDGE_EDIT_HEIGHT: f32 = BODY_LINE_HEIGHT + 8.0;

/// Область редактирования лейбла связи: бокс EDGE_EDIT_WIDTH × EDGE_EDIT_HEIGHT
/// по центру связи (середина дуги; при avoid — по видимой огибающей линии).
/// None — связи нет или она висячая.
pub fn edge_edit_area(
    canvas: &Canvas,
    edge_index: usize,
    avoid: bool,
) -> Option<([f32; 2], f32, f32)> {
    let edge = canvas.edges.get(edge_index)?;
    let mid = edge_midpoint(canvas, edge, avoid)?;
    let origin = [
        mid[0] - EDGE_EDIT_WIDTH / 2.0,
        mid[1] - EDGE_EDIT_HEIGHT / 2.0,
    ];
    Some((origin, EDGE_EDIT_WIDTH, EDGE_EDIT_HEIGHT))
}

/// Область редактирования сессии в world-координатах (левый верхний угол,
/// ширина, высота): тело карточки для ноды, бокс у середины связи — для
/// лейбла связи (T8).
pub fn session_area(
    canvas: &Canvas,
    session: &EditingSession,
    avoid: bool,
) -> Option<([f32; 2], f32, f32)> {
    match session.target() {
        EditTarget::Node(index) => canvas.nodes.get(index).map(body_area),
        EditTarget::Edge(index) => edge_edit_area(canvas, index, avoid),
    }
}

/// Ширина каретки в пикселях буфера.
const CARET_WIDTH: f32 = 2.0;
/// Минимальная ширина прямоугольника выделения (визуализация пустого фрагмента).
const MIN_SELECTION_WIDTH: f32 = 2.0;

/// Команда клавиатуры для сессии редактирования — результат `map_key`.
/// Commit/Cancel/Copy/Cut/Paste исполняет приложение (модель, автосейв,
/// буфер обмена), остальное — сама сессия.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyCommand {
    /// Готовое действие cosmic-text (Enter при Shift, Backspace, Delete, ...).
    Action(Action),
    /// Навигация курсора; `extend` (Shift) — расширить выделение.
    Motion(Motion, bool),
    /// Вставка текста (Key::Character без Ctrl; в том числе кириллица).
    Insert(String),
    /// Enter — завершить с сохранением.
    Commit,
    /// Esc — завершить с откатом к исходному тексту.
    Cancel,
    Copy,
    Cut,
    Paste,
    SelectAll,
    /// Тоггл маркера форматирования на выделении (Ctrl+B/I/H).
    ToggleMarker(Marker),
}

/// Маппинг клавиши winit в команду редактирования (T7).
/// `ctrl`/`shift` — состояние модификаторов. None — клавиша не для редактора.
pub fn map_key(key: &Key, ctrl: bool, shift: bool) -> Option<KeyCommand> {
    match key {
        Key::Named(NamedKey::Enter) => Some(if shift {
            KeyCommand::Action(Action::Enter)
        } else {
            KeyCommand::Commit
        }),
        Key::Named(NamedKey::Escape) => Some(KeyCommand::Cancel),
        Key::Named(NamedKey::Backspace) => Some(KeyCommand::Action(Action::Backspace)),
        Key::Named(NamedKey::Delete) => Some(KeyCommand::Action(Action::Delete)),
        Key::Named(NamedKey::ArrowLeft) => Some(KeyCommand::Motion(
            if ctrl { Motion::LeftWord } else { Motion::Left },
            shift,
        )),
        Key::Named(NamedKey::ArrowRight) => Some(KeyCommand::Motion(
            if ctrl {
                Motion::RightWord
            } else {
                Motion::Right
            },
            shift,
        )),
        Key::Named(NamedKey::ArrowUp) => Some(KeyCommand::Motion(Motion::Up, shift)),
        Key::Named(NamedKey::ArrowDown) => Some(KeyCommand::Motion(Motion::Down, shift)),
        Key::Named(NamedKey::Home) => Some(KeyCommand::Motion(Motion::Home, shift)),
        Key::Named(NamedKey::End) => Some(KeyCommand::Motion(Motion::End, shift)),
        Key::Character(text) => {
            if ctrl {
                // Буфер обмена/выделение: латиница и кириллическая раскладка,
                // плюс control-коды, которые winit может выдать с Ctrl
                return match text.as_str() {
                    "c" | "C" | "с" | "С" | "\u{3}" => Some(KeyCommand::Copy),
                    "x" | "X" | "ч" | "Ч" | "\u{18}" => Some(KeyCommand::Cut),
                    "v" | "V" | "м" | "М" | "\u{16}" => Some(KeyCommand::Paste),
                    "a" | "A" | "ф" | "Ф" | "\u{1}" => Some(KeyCommand::SelectAll),
                    // Форматирование (Ctrl+B/I/H), латиница и кириллица
                    "b" | "B" | "и" | "И" | "\u{2}" => {
                        Some(KeyCommand::ToggleMarker(Marker::Bold))
                    }
                    "i" | "I" | "ш" | "Ш" | "\u{9}" => {
                        Some(KeyCommand::ToggleMarker(Marker::Italic))
                    }
                    "h" | "H" | "р" | "Р" | "\u{8}" => {
                        Some(KeyCommand::ToggleMarker(Marker::Highlight))
                    }
                    _ => None,
                };
            }
            // Прочие control-символы (табуляция и т.п.) не вставляем
            if text.chars().any(|c| c.is_control()) {
                return None;
            }
            Some(KeyCommand::Insert(text.to_string()))
        }
        _ => None,
    }
}

/// Сессия инлайн-редактирования (T7 — тело текстовой ноды, T8 — лейбл связи).
///
/// WYSIWYG-модель (приёмка п.7): буфер держит ЧИСТЫЙ текст (без маркеров),
/// стили — спанами (markdown.rs); маркеры появляются только при сериализации
/// (`text()` → `markdown::emit`). Тоггл стиля на поддиапазоне рана — no-op,
/// вложенные одинаковые маркеры больше не «съедают» стиль.
pub struct EditingSession {
    buffer: Buffer,
    cursor: Cursor,
    selection: Selection,
    /// Что редактируется: нода или лейбл связи (индекс в модели).
    target: EditTarget,
    /// Исходный текст с маркерами — для отката по Esc.
    original: String,
    /// Чистый текст буфера и стили на нём (plain-координаты).
    plain: String,
    spans: Vec<StyleSpan>,
    /// Липкие флаги ввода (Ctrl+B/I/H без выделения).
    pending: PendingStyle,
    /// Высота строки текущего кадра (физ. px) — для каретки.
    line_height_px: f32,
    /// Последние применённые размеры/зум — set_layout без изменений не
    /// перешейпывает буфер.
    layout: (f32, f32, f32),
}

impl EditingSession {
    /// Начать редактирование: текст цели разбирается на чистый текст и
    /// спаны (маркеры не показываются), буфер — с rich-атрибутами.
    /// `width_px`/`height_px` — область редактирования в физических пикселях,
    /// `zoom_px` — zoom * scale_factor (перевод world-px в физические).
    pub fn new(
        font_system: &mut FontSystem,
        target: EditTarget,
        text: &str,
        width_px: f32,
        height_px: f32,
        zoom_px: f32,
    ) -> Self {
        let font_size = BODY_FONT_SIZE * zoom_px;
        let line_height = BODY_LINE_HEIGHT * zoom_px;
        let (plain, spans) = markdown::parse(text);
        let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
        buffer.set_wrap(font_system, Wrap::Word);
        buffer.set_size(font_system, Some(width_px), Some(height_px));
        buffer.set_rich_text(
            font_system,
            rich_spans(&plain, &spans),
            Attrs::new(),
            Shaping::Advanced,
        );
        let last_line = buffer.lines.len().saturating_sub(1);
        let last_len = buffer
            .lines
            .last()
            .map(|line| line.text().len())
            .unwrap_or(0);
        Self {
            buffer,
            cursor: Cursor::new(last_line, last_len),
            selection: Selection::None,
            target,
            original: text.to_owned(),
            plain,
            spans,
            pending: PendingStyle::default(),
            line_height_px: line_height,
            layout: (width_px, height_px, zoom_px),
        }
    }

    /// Временный Editor поверх буфера с восстановленным курсором/выделением;
    /// после операции состояние сохраняется обратно в поля сессии.
    fn with_editor<R>(&mut self, f: impl FnOnce(&mut Editor) -> R) -> R {
        let mut editor = Editor::new(&mut self.buffer);
        editor.set_cursor(self.cursor);
        editor.set_selection(self.selection);
        let result = f(&mut editor);
        self.cursor = editor.cursor();
        self.selection = editor.selection();
        result
    }

    /// Цель редактирования (нода или лейбл связи).
    pub fn target(&self) -> EditTarget {
        self.target
    }

    /// Индекс ноды, если редактируется нода; None для лейбла связи.
    pub fn node_index(&self) -> Option<usize> {
        match self.target {
            EditTarget::Node(index) => Some(index),
            EditTarget::Edge(_) => None,
        }
    }

    /// Текст для сохранения в модель: чистый текст + маркеры (emit).
    pub fn text(&self) -> String {
        markdown::emit(&self.plain, &self.spans)
    }

    /// Исходный текст на момент начала редактирования.
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Текст/стили изменились относительно исходного (сравнение в
    /// plain-координатах — каноническая запись маркеров не считается правкой).
    pub fn changed(&self) -> bool {
        let (plain, spans) = markdown::parse(&self.original);
        plain != self.plain || spans != self.spans
    }

    /// Обновить метрики под текущий зум/размер области (зум во время
    /// редактирования допустим — буфер перешейпится).
    pub fn set_layout(
        &mut self,
        font_system: &mut FontSystem,
        width_px: f32,
        height_px: f32,
        zoom_px: f32,
    ) {
        // Покадровый вызов из рендера: без изменений не перешейпываем
        let new_layout = (width_px, height_px, zoom_px);
        if (new_layout.0 - self.layout.0).abs() < 0.5
            && (new_layout.1 - self.layout.1).abs() < 0.5
            && (new_layout.2 - self.layout.2).abs() < 1e-3
        {
            return;
        }
        self.layout = new_layout;
        let line_height = BODY_LINE_HEIGHT * zoom_px;
        self.line_height_px = line_height;
        self.buffer.set_metrics(
            font_system,
            Metrics::new(BODY_FONT_SIZE * zoom_px, line_height),
        );
        self.buffer
            .set_size(font_system, Some(width_px), Some(height_px));
    }

    /// Текст буфера как чистая строка (строки через '\n').
    fn buffer_plain(&self) -> String {
        self.buffer
            .lines
            .iter()
            .map(|line| line.text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// После мутации буфера Editor'ом: синхронизировать plain/spans с
    /// текстом буфера (diff по общему префиксу/суффиксу → adjust_spans) и
    /// обновить атрибуты. Курсор/выделение не трогаем — текст тот же.
    fn sync_from_buffer(&mut self, font_system: &mut FontSystem) {
        let new_plain = self.buffer_plain();
        if new_plain == self.plain {
            return;
        }
        let (prefix, before, after) = edit_region(&self.plain, &new_plain);
        self.spans = markdown::adjust_spans(&self.spans, prefix, before, after);
        self.plain = new_plain;
        self.refresh_styles(font_system);
    }

    /// Обновить rich-атрибуты буфера по текущим спанам (текст не меняется —
    /// курсор (строка, индекс) остаётся валидным).
    fn refresh_styles(&mut self, font_system: &mut FontSystem) {
        let attrs = rich_spans(&self.plain, &self.spans);
        self.buffer
            .set_rich_text(font_system, attrs, Attrs::new(), Shaping::Advanced);
        self.buffer.shape_until_scroll(font_system, false);
    }

    /// Начало вставки: старт заменяемого выделения или позиция курсора
    /// (байтовый offset в plain).
    fn insert_start(&self) -> usize {
        let cursor = cursor_to_offset(&self.plain, self.cursor);
        match self.selection {
            Selection::Normal(anchor) => {
                let anchor = cursor_to_offset(&self.plain, anchor);
                anchor.min(cursor)
            }
            _ => cursor,
        }
    }

    /// Вставить строку (ввод/паста) и синхронизировать спаны; при липких
    /// флагах вставленный диапазон стилизуется ими.
    fn insert_and_sync(&mut self, font_system: &mut FontSystem, text: &str) {
        let start = self.insert_start();
        self.with_editor(|editor| editor.insert_string(text, None));
        self.sync_from_buffer(font_system);
        if self.pending.any() && !text.is_empty() {
            let mut spans = std::mem::take(&mut self.spans);
            for flag in [StyleFlag::Bold, StyleFlag::Italic, StyleFlag::Highlight] {
                if self.pending.get(flag) {
                    spans = markdown::set_style(
                        self.plain.len(),
                        &spans,
                        start,
                        start + text.len(),
                        flag,
                        true,
                    );
                }
            }
            self.spans = spans;
            self.refresh_styles(font_system);
        }
    }

    /// Выполнить команду клавиатуры. Commit/Cancel/Copy/Cut/Paste сессия
    /// не исполняет — их разбирает приложение (вернёт их же).
    pub fn apply(&mut self, font_system: &mut FontSystem, command: KeyCommand) -> KeyCommand {
        match &command {
            KeyCommand::Action(action) => {
                let action = *action;
                self.with_editor(|editor| editor.action(font_system, action));
                self.sync_from_buffer(font_system);
            }
            KeyCommand::Motion(motion, extend) => {
                let (motion, extend) = (*motion, *extend);
                self.with_editor(|editor| {
                    if extend {
                        // Shift расширяет выделение: якорь — текущий курсор
                        // (идиома cosmic-edit)
                        if let Selection::None = editor.selection() {
                            editor.set_selection(Selection::Normal(editor.cursor()));
                        }
                    } else {
                        editor.set_selection(Selection::None);
                    }
                    editor.action(font_system, Action::Motion(motion));
                });
                self.buffer.shape_until_scroll(font_system, false);
            }
            KeyCommand::Insert(text) => {
                let text = text.clone();
                self.insert_and_sync(font_system, &text);
            }
            KeyCommand::SelectAll => {
                let last_line = self.buffer.lines.len().saturating_sub(1);
                let last_len = self
                    .buffer
                    .lines
                    .last()
                    .map(|line| line.text().len())
                    .unwrap_or(0);
                self.selection = Selection::Normal(Cursor::new(0, 0));
                self.cursor = Cursor::new(last_line, last_len);
            }
            KeyCommand::ToggleMarker(marker) => {
                let marker = *marker;
                self.toggle_marker(font_system, marker);
            }
            // Обрабатываются приложением
            _ => {}
        }
        command
    }

    /// Вставить строку (паста из буфера обмена).
    pub fn insert_text(&mut self, font_system: &mut FontSystem, text: &str) {
        self.insert_and_sync(font_system, text);
    }

    /// Тоггл стиля (Ctrl+B/I/H) на выделении в plain-координатах
    /// (markdown::toggle_style): поддиапазон рана — no-op, ровно ран —
    /// снятие, частично стилизованный диапазон — назначение всему.
    /// Без выделения переключает «липкий» флаг для последующего ввода.
    pub fn toggle_marker(&mut self, font_system: &mut FontSystem, marker: Marker) {
        let flag = marker.flag();
        let cursor = cursor_to_offset(&self.plain, self.cursor);
        let selection = match self.selection {
            Selection::Normal(anchor) => {
                let (a, b) = (cursor_to_offset(&self.plain, anchor), cursor);
                Some((a.min(b), a.max(b)))
            }
            _ => None,
        };
        match selection {
            Some((start, end)) if start < end => {
                self.spans =
                    markdown::toggle_style(self.plain.len(), &self.spans, start, end, flag);
                self.refresh_styles(font_system);
            }
            _ => self.pending.toggle(flag),
        }
    }

    /// Вырезать выделение: вернуть текст и удалить его из буфера.
    pub fn cut_selection(&mut self, font_system: &mut FontSystem) -> Option<String> {
        let copied = self.with_editor(|editor| {
            let copied = editor.copy_selection();
            if copied.is_some() {
                editor.delete_selection();
            }
            copied
        });
        if copied.is_some() {
            self.sync_from_buffer(font_system);
        }
        copied
    }

    /// Скопировать выделение (буфер не меняется).
    pub fn copy_selection(&self) -> Option<String> {
        if self.selection == Selection::None {
            return None;
        }
        // copy_selection — &self-метод трейта Edit; Editor собирается на
        // неизменяемый буфер через временную копию указателей курсора
        let mut scratch = EditingSessionScratch {
            buffer: &self.buffer,
            cursor: self.cursor,
            selection: self.selection,
        };
        scratch.copy_selection()
    }

    /// Клик мышью в координатах буфера (пиксели от левого верхнего угла
    /// области тела карточки).
    pub fn click(&mut self, font_system: &mut FontSystem, x: i32, y: i32) {
        self.with_editor(|editor| editor.action(font_system, Action::Click { x, y }));
        self.buffer.shape_until_scroll(font_system, false);
    }

    /// Драг (расширение выделения) в координатах буфера.
    pub fn drag(&mut self, font_system: &mut FontSystem, x: i32, y: i32) {
        self.with_editor(|editor| editor.action(font_system, Action::Drag { x, y }));
        self.buffer.shape_until_scroll(font_system, false);
    }

    /// Прямоугольник каретки в координатах буфера: [x, y, ширина, высота].
    pub fn caret_rect(&mut self, font_system: &mut FontSystem) -> Option<[f32; 4]> {
        let line_height = self.line_height_px;
        self.buffer.shape_until_scroll(font_system, false);
        self.with_editor(|editor| {
            editor
                .cursor_position()
                .map(|(x, y)| [x as f32, y as f32, CARET_WIDTH, line_height])
        })
    }

    /// Прямоугольники выделения по строкам в координатах буфера.
    pub fn selection_rects(&mut self, font_system: &mut FontSystem) -> Vec<[f32; 4]> {
        self.buffer.shape_until_scroll(font_system, false);
        self.with_editor(|editor| {
            let Some((start, end)) = editor.selection_bounds() else {
                return Vec::new();
            };
            editor.with_buffer(|buffer| {
                buffer
                    .layout_runs()
                    .filter_map(|run| {
                        run.highlight(start, end).map(|(x, width)| {
                            [
                                x,
                                run.line_top,
                                width.max(MIN_SELECTION_WIDTH),
                                run.line_height,
                            ]
                        })
                    })
                    .collect()
            })
        })
    }

    /// Размер контента (ширина самой длинной строки, высота всех строк) в
    /// пикселях буфера (T7). Для автороста заметки под текст.
    ///
    /// Буфер ограничен высотой тела карточки, и при переполнении layout
    /// обрезается — измеряем со снятым ограничением высоты, затем размер
    /// буфера восстанавливается (иначе set_layout не заметит разницы).
    pub fn content_size_px(&mut self, font_system: &mut FontSystem) -> (f32, f32) {
        let (width, height, _) = self.layout;
        self.buffer.set_size(font_system, Some(width), None);
        self.buffer.shape_until_scroll(font_system, false);
        let mut lines = 0usize;
        let mut max_width = 0.0f32;
        for run in self.buffer.layout_runs() {
            lines += 1;
            max_width = max_width.max(run.line_w);
        }
        let size = (max_width, lines.max(1) as f32 * self.line_height_px);
        self.buffer.set_size(font_system, Some(width), Some(height));
        size
    }

    /// Буфер для рендера (TextArea в TextSystem).
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }
}

/// Копирование выделения без изменения сессии: Editor требует &mut Buffer
/// даже для чтения, поэтому работаем на клоне буфера.
struct EditingSessionScratch<'a> {
    buffer: &'a Buffer,
    cursor: Cursor,
    selection: Selection,
}

impl EditingSessionScratch<'_> {
    fn copy_selection(&mut self) -> Option<String> {
        let mut cloned = self.buffer.clone();
        let mut editor = Editor::new(&mut cloned);
        editor.set_cursor(self.cursor);
        editor.set_selection(self.selection);
        editor.copy_selection()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::{Edge, Node, Side};

    fn session(text: &str) -> (FontSystem, EditingSession) {
        let mut font_system = FontSystem::new();
        let session = EditingSession::new(
            &mut font_system,
            EditTarget::Node(0),
            text,
            300.0,
            200.0,
            1.0,
        );
        (font_system, session)
    }

    /// Вставка текста, в том числе кириллицы: text() отдаёт результат,
    /// каретка в пределах буфера.
    #[test]
    fn insert_cyrillic_moves_cursor() {
        let (mut fs, mut session) = session("Привет");
        session.insert_text(&mut fs, ", мир");
        assert_eq!(session.text(), "Привет, мир");
        assert!(session.changed());
        let caret = session.caret_rect(&mut fs).expect("каретка есть");
        assert!(caret[0] >= 0.0 && caret[0] <= 300.0, "x каретки: {caret:?}");
        assert!(caret[1] >= 0.0 && caret[1] <= 200.0, "y каретки: {caret:?}");
        assert_eq!(caret[2], CARET_WIDTH);
        assert_eq!(caret[3], BODY_LINE_HEIGHT);
    }

    /// Курсор новой сессии — в конце текста.
    #[test]
    fn cursor_starts_at_end() {
        let (mut fs, mut session) = session("строка");
        session.apply(&mut fs, KeyCommand::Insert("!".into()));
        assert_eq!(session.text(), "строка!");
    }

    /// Backspace/Delete со/без выделения.
    #[test]
    fn backspace_delete_semantics() {
        let (mut fs, mut session) = session("абв");
        session.apply(&mut fs, KeyCommand::Action(Action::Backspace));
        assert_eq!(session.text(), "аб");
        // Выделить всё и удалить одним Backspace
        session.apply(&mut fs, KeyCommand::SelectAll);
        assert!(!session.selection_rects(&mut fs).is_empty());
        session.apply(&mut fs, KeyCommand::Action(Action::Backspace));
        assert_eq!(session.text(), "");
        // Delete на пустом — без паники и изменений
        session.apply(&mut fs, KeyCommand::Action(Action::Delete));
        assert_eq!(session.text(), "");
    }

    /// SelectAll + вставка заменяет весь текст.
    #[test]
    fn select_all_insert_replaces() {
        let (mut fs, mut session) = session("старый\nтекст");
        session.apply(&mut fs, KeyCommand::SelectAll);
        session.apply(&mut fs, KeyCommand::Insert("новый".into()));
        assert_eq!(session.text(), "новый");
        assert!(session.changed());
    }

    /// Commit/cancel — семантика приложения: text() против original().
    #[test]
    fn commit_cancel_texts() {
        let (mut fs, mut session) = session("исходный");
        session.insert_text(&mut fs, " + правка");
        assert_eq!(session.original(), "исходный");
        assert_eq!(session.text(), "исходный + правка");
    }

    /// Навигация: Home/End/стрелки, Ctrl+стрелка — по словам; Shift — выделение.
    #[test]
    fn navigation_and_shift_selection() {
        let (mut fs, mut session) = session("раз два три");
        // Ctrl+Right от начала — к концу слова (семантика cosmic-text RightWord)
        session.apply(&mut fs, KeyCommand::Motion(Motion::Home, false));
        session.apply(&mut fs, KeyCommand::Motion(Motion::RightWord, false));
        session.apply(&mut fs, KeyCommand::Insert("-".into()));
        assert_eq!(session.text(), "раз- два три");
        // Shift+Right выделяет символ
        session.apply(&mut fs, KeyCommand::Motion(Motion::Home, false));
        session.apply(&mut fs, KeyCommand::Motion(Motion::Right, true));
        let copied = session.copy_selection().expect("есть выделение");
        assert_eq!(copied, "р");
        // Без Shift выделение сбрасывается
        session.apply(&mut fs, KeyCommand::Motion(Motion::Right, false));
        assert!(session.copy_selection().is_none());
    }

    /// Cut возвращает текст и удаляет его из буфера.
    #[test]
    fn cut_selection_removes() {
        let (mut fs, mut session) = session("вырезать это");
        session.apply(&mut fs, KeyCommand::SelectAll);
        let cut = session.cut_selection(&mut fs).expect("есть выделение");
        assert_eq!(cut, "вырезать это");
        assert_eq!(session.text(), "");
    }

    /// Тоггл стиля в сессии (WYSIWYG): SelectAll + Ctrl+B стилизует весь
    /// текст (маркеры только в text()), повторный тогл снимает; многострочное
    /// выделение работает.
    #[test]
    fn toggle_marker_in_session() {
        let (mut fs, mut s) = session("привет мир");
        s.apply(&mut fs, KeyCommand::SelectAll);
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Bold));
        assert_eq!(s.text(), "**привет мир**");
        // Выделение осталось: повторный тогл снимает стиль
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Bold));
        assert_eq!(s.text(), "привет мир");

        // Многострочное: выделить всё и стилизовать подсветкой
        let (mut fs, mut s) = session("раз\nдва");
        s.apply(&mut fs, KeyCommand::SelectAll);
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Highlight));
        assert_eq!(s.text(), "==раз\nдва==");

        // Без выделения: липкий флаг — ввод вставляется стилизованным
        let (mut fs, mut s) = session("");
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Italic));
        s.apply(&mut fs, KeyCommand::Insert("курсив".into()));
        assert_eq!(s.text(), "*курсив*");
    }

    /// Приёмка п.7: повторный bold подстроки внутри bold-строки — no-op,
    /// подстрока НЕ теряет стиль (вложенные маркеры больше не тогглят флаг).
    #[test]
    fn toggle_marker_substring_inside_bold_is_noop() {
        let (mut fs, mut s) = session("**привет мир**");
        assert_eq!(s.text(), "**привет мир**", "буфер чистый, маркеры в text()");
        // Выделяем "риве" (байты 2..10): Home, шаг вправо, расширение ×3
        s.apply(&mut fs, KeyCommand::Motion(Motion::Home, false));
        s.apply(&mut fs, KeyCommand::Motion(Motion::Right, false));
        for _ in 0..3 {
            s.apply(&mut fs, KeyCommand::Motion(Motion::Right, true));
        }
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Bold));
        assert_eq!(s.text(), "**привет мир**", "подстрока осталась bold");
    }

    /// Диапазон, частично стилизованный (внутри него уже bold-слово),
    /// стилизуется целиком — уже стилизованная часть не ломается.
    #[test]
    fn toggle_marker_partial_range_sets_all() {
        let (mut fs, mut s) = session("aa **bb** cc");
        // Выделяем "a bb c" (символы 1..7): Home, шаг, расширение ×6
        s.apply(&mut fs, KeyCommand::Motion(Motion::Home, false));
        s.apply(&mut fs, KeyCommand::Motion(Motion::Right, false));
        for _ in 0..6 {
            s.apply(&mut fs, KeyCommand::Motion(Motion::Right, true));
        }
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Bold));
        assert_eq!(s.text(), "a**a bb c**c");
    }

    /// Набор внутри стилизованного региона продолжает стиль (adjust_spans
    /// расширяет спан на вставленный текст).
    #[test]
    fn typing_inside_bold_extends_style() {
        let (mut fs, mut s) = session("**жирный**");
        // Курсор новой сессии — в конце; двигаемся строго внутрь спана
        // (после «жирн», байт 10; вставка на границе спана — вне его)
        s.apply(&mut fs, KeyCommand::Motion(Motion::Home, false));
        for _ in 0.."жирн".chars().count() {
            s.apply(&mut fs, KeyCommand::Motion(Motion::Right, false));
        }
        s.apply(&mut fs, KeyCommand::Insert("!".into()));
        assert_eq!(s.text(), "**жирн!ый**");
    }

    /// Многострочность через Action::Enter (Shift+Enter на уровне приложения).
    #[test]
    fn shift_enter_newline() {
        let (mut fs, mut session) = session("один");
        session.apply(&mut fs, KeyCommand::Action(Action::Enter));
        session.apply(&mut fs, KeyCommand::Insert("два".into()));
        assert_eq!(session.text(), "один\nдва");
    }

    /// Зум во время редактирования: set_layout не теряет текст и курсор.
    #[test]
    fn relayout_keeps_state() {
        let (mut fs, mut session) = session("текст");
        session.set_layout(&mut fs, 600.0, 400.0, 2.0);
        assert_eq!(session.text(), "текст");
        session.apply(&mut fs, KeyCommand::Insert("!".into()));
        assert_eq!(session.text(), "текст!");
        let caret = session.caret_rect(&mut fs).expect("каретка есть");
        assert_eq!(caret[3], BODY_LINE_HEIGHT * 2.0);
    }

    /// Размер контента (T7): высота по строкам, wrap увеличивает счёт;
    /// переполнение измеряется даже когда буфер по высоте меньше контента.
    #[test]
    fn content_size_tracks_lines() {
        let (mut fs, mut s) = session("одна строка");
        assert_eq!(s.content_size_px(&mut fs).1, BODY_LINE_HEIGHT);
        // Две строки — двойная высота
        let (mut fs, mut s) = session("один\nдва");
        assert_eq!(s.content_size_px(&mut fs).1, BODY_LINE_HEIGHT * 2.0);
        // Длинная строка wrap'ится: в буфере 100px шириной строк больше одной
        let mut fs2 = FontSystem::new();
        let mut s = EditingSession::new(
            &mut fs2,
            EditTarget::Node(0),
            &"длинное слово ".repeat(30),
            100.0,
            500.0,
            1.0,
        );
        assert!(
            s.content_size_px(&mut fs2).1 > BODY_LINE_HEIGHT,
            "wrap должен дать больше одной строки"
        );
        // Контент выше буфера: высота измеряется полностью, без clip
        let mut fs3 = FontSystem::new();
        let mut s = EditingSession::new(
            &mut fs3,
            EditTarget::Node(0),
            "1\n2\n3\n4\n5\n6\n7\n8",
            200.0,
            40.0,
            1.0,
        );
        assert_eq!(
            s.content_size_px(&mut fs3).1,
            BODY_LINE_HEIGHT * 8.0,
            "переполнение должно измеряться целиком"
        );
        // Ширина — самая длинная строка layout
        let mut fs4 = FontSystem::new();
        let mut s = EditingSession::new(
            &mut fs4,
            EditTarget::Node(0),
            "короткая\nочень очень длинная строка",
            600.0,
            200.0,
            1.0,
        );
        let (w, _) = s.content_size_px(&mut fs4);
        assert!(w > 100.0, "ширина длинной строки: {w}");
    }

    /// Область редактирования (T8): нода — тело карточки, связь — бокс
    /// по центру кривой; невалидный индекс — None.
    #[test]
    fn session_area_targets() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "t", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "t", 500.0, 0.0));
        canvas.edges.push(Edge::new(
            "e1",
            "a",
            Some(Side::Right),
            "b",
            Some(Side::Left),
        ));
        let mut fs = FontSystem::new();
        let node_session = EditingSession::new(&mut fs, EditTarget::Node(0), "", 100.0, 50.0, 1.0);
        let area = session_area(&canvas, &node_session, false).expect("нода есть");
        assert_eq!(area, {
            let (origin, w, h) = body_area(&canvas.nodes[0]);
            (origin, w, h)
        });
        assert_eq!(node_session.node_index(), Some(0));

        let edge_session = EditingSession::new(&mut fs, EditTarget::Edge(0), "", 100.0, 50.0, 1.0);
        let (origin, w, h) = session_area(&canvas, &edge_session, false).expect("связь есть");
        assert_eq!((w, h), (EDGE_EDIT_WIDTH, EDGE_EDIT_HEIGHT));
        // Центр бокса — середина кривой (Node::text высотой 120: порты на y = 60)
        assert!((origin[1] + h / 2.0 - 60.0).abs() < 1e-3);
        assert_eq!(edge_session.node_index(), None);

        // Висячая/несуществующая связь — None
        let dangling = EditingSession::new(&mut fs, EditTarget::Edge(9), "", 100.0, 50.0, 1.0);
        assert!(session_area(&canvas, &dangling, false).is_none());
    }

    /// Маппинг клавиш: Enter — commit, Shift+Enter — новая строка, Esc — cancel,
    /// Ctrl+C/X/V/A на латинице и кириллице, символы — вставка.
    #[test]
    fn key_mapping() {
        let enter = Key::Named(NamedKey::Enter);
        assert_eq!(map_key(&enter, false, false), Some(KeyCommand::Commit));
        assert_eq!(
            map_key(&enter, false, true),
            Some(KeyCommand::Action(Action::Enter))
        );
        assert_eq!(
            map_key(&Key::Named(NamedKey::Escape), false, false),
            Some(KeyCommand::Cancel)
        );
        for key in ["c", "с"] {
            assert_eq!(
                map_key(&Key::Character(key.into()), true, false),
                Some(KeyCommand::Copy)
            );
        }
        assert_eq!(
            map_key(&Key::Character("v".into()), true, false),
            Some(KeyCommand::Paste)
        );
        assert_eq!(
            map_key(&Key::Character("м".into()), true, false),
            Some(KeyCommand::Paste)
        );
        assert_eq!(
            map_key(&Key::Character("ф".into()), true, false),
            Some(KeyCommand::SelectAll)
        );
        // Форматирование: Ctrl+B/I/H, латиница и кириллица
        assert_eq!(
            map_key(&Key::Character("b".into()), true, false),
            Some(KeyCommand::ToggleMarker(Marker::Bold))
        );
        assert_eq!(
            map_key(&Key::Character("и".into()), true, false),
            Some(KeyCommand::ToggleMarker(Marker::Bold))
        );
        assert_eq!(
            map_key(&Key::Character("ш".into()), true, false),
            Some(KeyCommand::ToggleMarker(Marker::Italic))
        );
        assert_eq!(
            map_key(&Key::Character("р".into()), true, false),
            Some(KeyCommand::ToggleMarker(Marker::Highlight))
        );
        // Без Ctrl эти буквы — обычная вставка
        assert_eq!(
            map_key(&Key::Character("b".into()), false, false),
            Some(KeyCommand::Insert("b".into()))
        );
        assert_eq!(
            map_key(&Key::Character("я".into()), false, false),
            Some(KeyCommand::Insert("я".into()))
        );
        // Control-символы без Ctrl не вставляются
        assert_eq!(map_key(&Key::Character("\t".into()), false, false), None);
        // Ctrl+Left — word jump, Shift+Left — расширение выделения
        assert_eq!(
            map_key(&Key::Named(NamedKey::ArrowLeft), true, false),
            Some(KeyCommand::Motion(Motion::LeftWord, false))
        );
        assert_eq!(
            map_key(&Key::Named(NamedKey::ArrowLeft), false, true),
            Some(KeyCommand::Motion(Motion::Left, true))
        );
        // Прочие клавиши редактору не нужны
        assert_eq!(map_key(&Key::Named(NamedKey::F5), false, false), None);
    }
}
