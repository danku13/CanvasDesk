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
//! (физические px относительно левого верхнего угла тела карточки).

use cosmic_text::{Action, Buffer, Cursor, Edit, Editor, FontSystem, Metrics, Motion, Selection};
use glyphon::{Attrs, Shaping, Wrap};
use winit::keyboard::{Key, NamedKey};

use crate::text::{offset_to_cursor, BODY_FONT_SIZE, BODY_LINE_HEIGHT};

/// Маркер форматирования текста заметки (пост-T7): markdown-подмножество,
/// см. markdown.rs. Хоткеи Ctrl+B/I/H тогглят маркер на выделении.
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
    fn as_str(self) -> &'static str {
        match self {
            Marker::Bold => "**",
            Marker::Italic => "*",
            Marker::Highlight => "==",
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

/// Тоггл маркера форматирования на выделении (чистая функция, TDD):
/// - выделение уже обёрнуто этим маркером — маркеры снимаются;
/// - выделение есть — оборачивается, выделение смещается на контент;
/// - выделения нет — пара маркеров вставляется в позицию курсора,
///   курсор оказывается между ними.
///
/// `selection`/`cursor` и результат — линейные байтовые offsets.
/// Возвращает (новый текст, курсор, выделение).
pub fn toggle_marker_text(
    text: &str,
    cursor: usize,
    selection: Option<(usize, usize)>,
    marker: &str,
) -> (String, usize, Option<(usize, usize)>) {
    let cursor = cursor.min(text.len());
    let m = marker.len();
    match selection {
        Some((start, end)) if start < end && end <= text.len() => {
            let before = &text[..start];
            let after = &text[end..];
            // Снятие засчитывается, только если вокруг выделения именно этот
            // маркер: одиночная '*' не должна совпасть с частью '**'
            let wrapped = if marker == "*" {
                before.ends_with('*')
                    && !before.ends_with("**")
                    && after.starts_with('*')
                    && !after.starts_with("**")
            } else {
                before.ends_with(marker) && after.starts_with(marker)
            };
            if wrapped {
                // Снятие: маркеры вокруг выделения удаляются
                let mut new = String::with_capacity(text.len() - 2 * m);
                new.push_str(&before[..before.len() - m]);
                new.push_str(&text[start..end]);
                new.push_str(&after[m..]);
                let sel = (start - m, end - m);
                (new, sel.1, Some(sel))
            } else {
                // Оборачивание: выделение смещается на контент без маркеров
                let mut new = String::with_capacity(text.len() + 2 * m);
                new.push_str(before);
                new.push_str(marker);
                new.push_str(&text[start..end]);
                new.push_str(marker);
                new.push_str(after);
                let sel = (start + m, end + m);
                (new, sel.1, Some(sel))
            }
        }
        _ => {
            let mut new = String::with_capacity(text.len() + 2 * m);
            new.push_str(&text[..cursor]);
            new.push_str(marker);
            new.push_str(marker);
            new.push_str(&text[cursor..]);
            (new, cursor + m, None)
        }
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

/// Сессия инлайн-редактирования одной текстовой ноды (T7).
pub struct EditingSession {
    buffer: Buffer,
    cursor: Cursor,
    selection: Selection,
    /// Индекс редактируемой ноды в `canvas.nodes`.
    node: usize,
    /// Исходный текст — для отката по Esc.
    original: String,
    /// Высота строки текущего кадра (физ. px) — для каретки.
    line_height_px: f32,
    /// Последние применённые размеры/зум — set_layout без изменений не
    /// перешейпывает буфер.
    layout: (f32, f32, f32),
}

impl EditingSession {
    /// Начать редактирование: буфер с текстом ноды, курсор в конец.
    /// `width_px`/`height_px` — область тела карточки в физических пикселях,
    /// `zoom_px` — zoom * scale_factor (перевод world-px в физические).
    pub fn new(
        font_system: &mut FontSystem,
        node: usize,
        text: &str,
        width_px: f32,
        height_px: f32,
        zoom_px: f32,
    ) -> Self {
        let font_size = BODY_FONT_SIZE * zoom_px;
        let line_height = BODY_LINE_HEIGHT * zoom_px;
        let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));
        buffer.set_wrap(font_system, Wrap::Word);
        buffer.set_size(font_system, Some(width_px), Some(height_px));
        buffer.set_text(font_system, text, Attrs::new(), Shaping::Advanced);
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
            node,
            original: text.to_owned(),
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

    /// Индекс редактируемой ноды.
    pub fn node(&self) -> usize {
        self.node
    }

    /// Текущий текст (строки через '\n').
    pub fn text(&self) -> String {
        self.buffer
            .lines
            .iter()
            .map(|line| line.text())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Исходный текст на момент начала редактирования.
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Текст изменился относительно исходного.
    pub fn changed(&self) -> bool {
        self.text() != self.original
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

    /// Выполнить команду клавиатуры. Commit/Cancel/Copy/Cut/Paste сессия
    /// не исполняет — их разбирает приложение (вернёт их же).
    pub fn apply(&mut self, font_system: &mut FontSystem, command: KeyCommand) -> KeyCommand {
        match &command {
            KeyCommand::Action(action) => {
                let action = *action;
                self.with_editor(|editor| editor.action(font_system, action));
                self.buffer.shape_until_scroll(font_system, false);
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
                self.with_editor(|editor| editor.insert_string(&text, None));
                self.buffer.shape_until_scroll(font_system, false);
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
        self.with_editor(|editor| editor.insert_string(text, None));
        self.buffer.shape_until_scroll(font_system, false);
    }

    /// Тоггл маркера форматирования (Ctrl+B/I/H): обернуть выделение, снять
    /// обёртку либо вставить пару маркеров в позицию курсора. Реализация —
    /// чистая `toggle_marker_text` над текстом целиком; буфер пересобирается
    /// (в редакторе стилей нет — маркеры видны как есть, source-режим).
    pub fn toggle_marker(&mut self, font_system: &mut FontSystem, marker: Marker) {
        let text = self.text();
        let cursor = cursor_to_offset(&text, self.cursor);
        let selection = match self.selection {
            Selection::Normal(anchor) => {
                let (a, b) = (cursor_to_offset(&text, anchor), cursor);
                Some((a.min(b), a.max(b)))
            }
            _ => None,
        };
        let (new_text, new_cursor, new_selection) =
            toggle_marker_text(&text, cursor, selection, marker.as_str());
        self.buffer
            .set_text(font_system, &new_text, Attrs::new(), Shaping::Advanced);
        self.buffer.shape_until_scroll(font_system, false);
        self.cursor = offset_to_cursor(&new_text, new_cursor);
        self.selection = match new_selection {
            Some((start, _)) => Selection::Normal(offset_to_cursor(&new_text, start)),
            None => Selection::None,
        };
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
            self.buffer.shape_until_scroll(font_system, false);
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

    fn session(text: &str) -> (FontSystem, EditingSession) {
        let mut font_system = FontSystem::new();
        let session = EditingSession::new(&mut font_system, 0, text, 300.0, 200.0, 1.0);
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

    /// Тоггл маркера (чистая функция): обернуть, снять, пара без выделения.
    #[test]
    fn toggle_marker_text_wrap_unwrap() {
        // Оборачивание выделения (кириллица: «два» — байты 7..13)
        let (text, cursor, sel) = toggle_marker_text("раз два три", 13, Some((7, 13)), "**");
        assert_eq!(text, "раз **два** три");
        assert_eq!(sel, Some((9, 15)), "выделение смещается на контент");
        assert_eq!(cursor, 15);
        // Повторный тоггл тем же выделением — снятие
        let (text, cursor, sel) = toggle_marker_text(&text, 15, sel, "**");
        assert_eq!(text, "раз два три");
        assert_eq!(sel, Some((7, 13)));
        assert_eq!(cursor, 13);
        // Без выделения — пара маркеров, курсор между ними (конец «текст» — байт 10)
        let (text, cursor, sel) = toggle_marker_text("текст", 10, None, "==");
        assert_eq!(text, "текст====");
        assert_eq!(cursor, 12);
        assert_eq!(sel, None);
        // Курсор в начале
        let (text, _, _) = toggle_marker_text("", 0, None, "*");
        assert_eq!(text, "**");
        // Разные маркеры независимы: * вокруг ** не снимается тогглом *
        let (text, _, _) = toggle_marker_text("**x**", 4, Some((2, 3)), "*");
        assert_eq!(text, "***x***");
    }

    /// Тоггл маркера в сессии: SelectAll + Ctrl+B оборачивает весь текст,
    /// кириллица (многобайтовые offsets) корректна, многострочное выделение.
    #[test]
    fn toggle_marker_in_session() {
        let (mut fs, mut s) = session("привет мир");
        s.apply(&mut fs, KeyCommand::SelectAll);
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Bold));
        assert_eq!(s.text(), "**привет мир**");
        // Выделение осталось на контенте: следующий тоггл снимает маркеры
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Bold));
        assert_eq!(s.text(), "привет мир");

        // Многострочное: выделить всё и обернуть подсветкой
        let (mut fs, mut s) = session("раз\nдва");
        s.apply(&mut fs, KeyCommand::SelectAll);
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Highlight));
        assert_eq!(s.text(), "==раз\nдва==");

        // Без выделения: пара маркеров, ввод попадает между ними
        let (mut fs, mut s) = session("");
        s.apply(&mut fs, KeyCommand::ToggleMarker(Marker::Italic));
        s.apply(&mut fs, KeyCommand::Insert("курсив".into()));
        assert_eq!(s.text(), "*курсив*");
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
        let mut s =
            EditingSession::new(&mut fs2, 0, &"длинное слово ".repeat(30), 100.0, 500.0, 1.0);
        assert!(
            s.content_size_px(&mut fs2).1 > BODY_LINE_HEIGHT,
            "wrap должен дать больше одной строки"
        );
        // Контент выше буфера: высота измеряется полностью, без clip
        let mut fs3 = FontSystem::new();
        let mut s = EditingSession::new(&mut fs3, 0, "1\n2\n3\n4\n5\n6\n7\n8", 200.0, 40.0, 1.0);
        assert_eq!(
            s.content_size_px(&mut fs3).1,
            BODY_LINE_HEIGHT * 8.0,
            "переполнение должно измеряться целиком"
        );
        // Ширина — самая длинная строка layout
        let mut fs4 = FontSystem::new();
        let mut s = EditingSession::new(
            &mut fs4,
            0,
            "короткая\nочень очень длинная строка",
            600.0,
            200.0,
            1.0,
        );
        let (w, _) = s.content_size_px(&mut fs4);
        assert!(w > 100.0, "ширина длинной строки: {w}");
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
