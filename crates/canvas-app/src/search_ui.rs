//! UI-модель панели поиска (T14, TASKS/SPEC §5.2): однострочное поле ввода с
//! кареткой, список результатов, геометрия панели, in-memory substring-поиск
//! по заметкам и именам нод.
//!
//! Модуль чистый (без wgpu/winit) — покрывается юнит-тестами. Рендер панели
//! выполняет приложение через `FrameOverlay` (screen_instances: CardInstance
//! с fill/border/скруглением; ScreenText для текста) — координаты берутся из
//! `layout`. egui в стеке нет и не заводится (план T14 §3).
//!
//! Ввод НЕ использует `EditingSession` (это многострочный редактор заметок):
//! поле поиска — однострочное, своя лёгкая модель `SearchInput`.
//!
//! W-f (аудит ui-kit §5/B7): модуль перенесён из `canvas-render` в
//! `canvas-app` — внутри рендера он НЕ использовался (единственные
//! потребители — app.rs/overlays.rs/ui_layout_lint.rs canvas-app), а вёрстка
//! целиком на примитивах `canvas_ui`. Публичный API сохранён бит-в-бит;
//! единственная правка тела — пути `crate::text::{measure_font_system,
//! SANS_FAMILY}` → `canvas_render::text::*` (владелец глобального FontSystem —
//! рендер; canvas-app и прежде читал его через тот же путь
//! `canvas_render::text`).
use std::collections::HashSet;

// Spacing-scale токены (W-d): значения = прежним литералам (I-1 ноль скачка).
use canvas_core::tokens::SPACING_MD as ROW_TEXT_RIGHT_PAD;
use canvas_core::tokens::SPACING_SM as PANEL_BOTTOM_MARGIN;

/// Ширина панели, логические px (клампится к окну − 2×PANEL_SIDE_MARGIN).
pub const PANEL_WIDTH: f32 = 460.0;
/// Боковой отступ панели от краёв окна, логические px (токен `SPACING_LG`).
pub use canvas_core::tokens::SPACING_LG as PANEL_SIDE_MARGIN;
/// Отступ панели от верхнего края окна, логические px (токен `SPACING_LG`).
pub use canvas_core::tokens::SPACING_LG as PANEL_TOP_MARGIN;
/// Высота поля ввода, логические px. LAY-W7 (аудит layouts-2026-10 §5):
/// канонизация на шкалу S3 — `canvas_ui::kit::TEXT_FIELD_HEIGHT` (30);
/// ранее 36 (вне шкалы, −6px).
pub const INPUT_HEIGHT: f32 = canvas_ui::kit::TEXT_FIELD_HEIGHT;
/// Максимум видимых строк результата (далее — прокрутка).
pub const MAX_VISIBLE_ROWS: usize = 8;
/// Внутренний отступ содержимого панели, логические px (токен `SPACING_SM`).
pub use canvas_core::tokens::SPACING_SM as PANEL_PADDING;

// FR-088: адаптивная вёрстка строк результата. Высота строки — НЕ константа,
// а замер контента тем же TextMeasurer, что и раскладка (ui-kit §5:
// «эвристики запрещены»): заголовок — одна строка с ellipsis, подзаголовок —
// перенос по словам. Линейные величины — дизайн-константы (каталог W3.3:
// «h — явная высота дизайн-константой», чип 26 ≠ измеренной 13·1.3).
/// Кегль заголовка строки результата (ui-kit: кегль контента 13).
pub const TITLE_FONT_SIZE: f32 = 13.0;
/// Кегль подзаголовка строки (вторичный текст).
pub const SUB_FONT_SIZE: f32 = 11.0;
/// Высота строки заголовка (кегль 13 · межстрочный 1.3 ≈ 17).
pub const TITLE_LINE_H: f32 = 17.0;
/// Высота строки подзаголовка (кегль 11 · 1.3 ≈ 15).
pub const SUB_LINE_H: f32 = 15.0;
/// Вертикальный пад строки результата (сверху и снизу; токен `SPACING_S`).
pub use canvas_core::tokens::SPACING_S as ROW_PAD_V;
/// Зазор между блоком заголовка и подзаголовком — 2 px hairline, вне шкалы
/// S1 (исключение LAY7 — «Исключения» 11-layouts.md, «Hairline-микрозначения
/// 2–4 px», LAY-W16: метрика плотности строки результата, [`row_height`]).
pub const TITLE_SUB_GAP: f32 = 2.0;
/// Отступ текста строки слева (строки нод; токен `SPACING_MD`).
pub use canvas_core::tokens::SPACING_MD as ROW_TEXT_X;
/// Отступ текста слева у строк-доков — 30 px, вне шкалы S1 (исключение LAY7 —
/// «Исключения» 11-layouts.md, «Поля/зазоры 20–40 px», LAY-W16: выравнивание
/// по зоне бейджа «?» (ширина бейджа + зазор), не свободный зазор шкалы).
pub const ROW_TEXT_X_DOCS: f32 = 30.0;
// Приватные токены `ROW_TEXT_RIGHT_PAD` (правый пад текста строки) и
// `PANEL_BOTTOM_MARGIN` (нижний запас панели от края окна, кламп суммарной
// высоты FR-088) импортированы сверху из `canvas_core::tokens`
// (`SPACING_MD` / `SPACING_SM`).

/// Высота строки результата по контенту: пад + заголовок (+ пад + зазор +
/// строки подзаголовка). Единый источник для раскладки и тестов.
pub fn row_height(subtitle_lines: usize) -> f32 {
    ROW_PAD_V
        + TITLE_LINE_H
        + if subtitle_lines == 0 {
            0.0
        } else {
            TITLE_SUB_GAP + subtitle_lines as f32 * SUB_LINE_H
        }
        + ROW_PAD_V
}

/// Разделитель слова для Ctrl+Backspace — простая эвристика: пробельный символ
/// Однострочное поле ввода поиска — обёртка kit
/// [`canvas_ui::kit::TextFieldModel`] (волна «input-адекватность» 2026-10-09,
/// design/rules/09-input.md IN1/IN9: единая модель поля; каретка и селекция —
/// в СИМВОЛАХ, не в байтах; словесные операции и клавиатурный контракт —
/// kit `TextFieldAction`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SearchInput {
    /// Модель кита (текст/каретка/селекция).
    pub field: canvas_ui::kit::TextFieldModel,
}

impl SearchInput {
    /// Пустое поле.
    pub fn new() -> Self {
        Self::default()
    }

    /// Текущий запрос.
    pub fn query(&self) -> &str {
        &self.field.text
    }

    /// Позиция каретки в СИМВОЛАХ (`chars().count()`).
    pub fn caret(&self) -> usize {
        self.field.caret
    }

    /// Заменить весь текст (повторное Ctrl+F: ввод замещает прошлый запрос),
    /// каретка в конец.
    pub fn set_query(&mut self, text: &str) {
        self.field.set_text(text.to_owned());
    }

    /// Применить kit-действие ввода (маппер клавиш —
    /// `crate::app::input::text_field_action`). Эффекты буфера (Copy/Cut/
    /// Paste) — доводит потребитель.
    pub fn apply_action(
        &mut self,
        action: canvas_ui::kit::TextFieldAction,
    ) -> canvas_ui::kit::TextFieldEffect {
        self.field.apply(action)
    }

    /// Вставить текст в позицию каретки (символы печати/IME).
    /// Возвращает true, если текст изменился.
    pub fn insert_str(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        let before = self.field.text.clone();
        self.field.insert(text);
        self.field.text != before
    }

    /// Backspace: удалить символ слева (многобайтный — целиком); с `word`
    /// (Ctrl+Backspace) — слово слева. Возвращает true, если текст изменился.
    pub fn backspace(&mut self, word: bool) -> bool {
        if word {
            self.field.delete_word_backward()
        } else {
            matches!(
                self.field.apply(canvas_ui::kit::TextFieldAction::Backspace),
                canvas_ui::kit::TextFieldEffect::Changed
            )
        }
    }

    /// Delete: удалить символ справа от каретки (многобайтный — целиком).
    /// Возвращает true, если текст изменился.
    pub fn delete(&mut self) -> bool {
        matches!(
            self.field.apply(canvas_ui::kit::TextFieldAction::Delete),
            canvas_ui::kit::TextFieldEffect::Changed
        )
    }

    /// Движение каретки: Home.
    pub fn move_to_start(&mut self) {
        self.field.move_caret(-(self.field.caret as isize), false);
    }

    /// Движение каретки: End.
    pub fn move_to_end(&mut self) {
        let total = self.field.text.chars().count();
        self.field
            .move_caret(total as isize - self.field.caret as isize, false);
    }

    /// Движение каретки: влево на один символ (в СИМВОЛАХ).
    pub fn move_left(&mut self) {
        self.field.move_caret(-1, false);
    }

    /// Движение каретки: вправо на один символ (в СИМВОЛАХ).
    pub fn move_right(&mut self) {
        self.field.move_caret(1, false);
    }
}

/// Класс результата поиска (владелец 2026-10-02: результаты по встроенной
/// документации должны быть явно помечены — это не поиск по нодам).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchRowKind {
    /// Нода канваса (FTS по файлам / substring по заметкам и именам).
    #[default]
    Node,
    /// Страница встроенной документации (открывается просмотрщиком доков).
    Docs,
}

/// Строка результата поиска (уже сматчена в ноду приложения / страницу доков).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRow {
    /// Заголовок: имя файла / первые символы текста заметки / метка страницы.
    pub title: String,
    /// Подзаголовок: хвост пути, «заметка» или «Документация · сниппет».
    pub subtitle: String,
    /// Класс результата: нода (дефолт) или страница документации (бейдж).
    pub kind: SearchRowKind,
}

impl SearchRow {
    /// Строка-нода (прежние два поля — все существующие вызовы дословно).
    pub fn node(title: String, subtitle: String) -> Self {
        Self {
            title,
            subtitle,
            kind: SearchRowKind::Node,
        }
    }

    /// Строка-страница документации (помечена классом Docs).
    pub fn docs(title: String, subtitle: String) -> Self {
        Self {
            title,
            subtitle,
            kind: SearchRowKind::Docs,
        }
    }
}

/// Действие панели, возвращаемое при обработке клавиш (Enter/Esc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelAction {
    /// Прыжок к строке с индексом.
    Jump(usize),
    /// Закрыть панель.
    Close,
}

/// Состояние панели поиска.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SearchPanel {
    /// Панель открыта (видима, получает клавиатуру).
    pub open: bool,
    /// Поле ввода.
    pub input: SearchInput,
    /// Строки результатов (соответствие нодам — в приложении).
    pub rows: Vec<SearchRow>,
    /// Выбранная строка (Enter прыгает к ней).
    pub selected: Option<usize>,
    /// Верхняя видимая строка (прокрутка).
    pub scroll_top: usize,
}

impl SearchPanel {
    /// Открыть панель (текст прошлой выборки сохраняется — ввод замещает).
    pub fn open(&mut self) {
        self.open = true;
    }

    /// Закрыть панель (rows сохраняются для цикла F3).
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Панель открыта?
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Установить результаты (после Query/scan_scene): сброс выбора на первую
    /// строку, скролл вверх.
    pub fn set_results(&mut self, rows: Vec<SearchRow>) {
        self.selected = if rows.is_empty() { None } else { Some(0) };
        self.scroll_top = 0;
        self.rows = rows;
    }

    /// Сдвиг выбора на `delta` строк с зацикливанием (F3/Shift+F3, Up/Down):
    /// вниз с последней строки — на первую, вверх с первой — на последнюю.
    /// Пустой список — no-op.
    pub fn move_selection(&mut self, delta: i32) {
        if self.rows.is_empty() {
            return;
        }
        // Длина списка и индексы много меньше i64 — переполнений нет.
        let len = self.rows.len() as i64;
        let current = self.selected.map_or(0, |s| s as i64);
        let wrapped = (current + delta as i64).rem_euclid(len);
        self.selected = Some(wrapped as usize);
    }

    /// Прокрутить к выбранной строке, если она вне видимого окна
    /// [scroll_top, scroll_top + MAX_VISIBLE_ROWS).
    pub fn ensure_selection_visible(&mut self) {
        if self.rows.is_empty() {
            self.scroll_top = 0;
            return;
        }
        let sel = self.selected.unwrap_or(0);
        if sel < self.scroll_top {
            // Выше окна — прокрутка к выбранной строке.
            self.scroll_top = sel;
        } else if sel >= self.scroll_top.saturating_add(MAX_VISIBLE_ROWS) {
            // Ниже окна — выбранная строка становится последней видимой.
            // Ветка гарантирует sel >= MAX_VISIBLE_ROWS — вычитание не
            // уходит в минус.
            self.scroll_top = sel - (MAX_VISIBLE_ROWS - 1);
        }
        // Прокрутка не дальше конца списка.
        self.scroll_top = self.scroll_top.min(self.rows.len());
    }

    /// Enter: прыжок к выбранной строке (нет выбора — к первой);
    /// пустой список — `None`.
    pub fn confirm(&self) -> Option<PanelAction> {
        if self.rows.is_empty() {
            None
        } else {
            Some(PanelAction::Jump(self.selected.unwrap_or(0)))
        }
    }

    /// Esc: закрыть панель.
    pub fn cancel(&self) -> PanelAction {
        PanelAction::Close
    }
}

/// Геометрия панели в логических px (для отрисовки через FrameOverlay).
///
/// Контракт формата — **xyxy** (`[x0, y0, x1, y1]`, уникальный в кодовой
/// базе: остальные раскладки — xywh). Потребители конвертируют явно
/// (`rect_xywh` в canvas-app; адаптер реестра — напрямую `UiRect::new`).
/// Унификация формата отложена: правка потребителей рискует визуальными
/// регрессиями без выигрыша в поведении (FR-054 §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct PanelLayout {
    /// Прямоугольник панели: `[x0, y0, x1, y1]`.
    pub panel_rect: [f32; 4],
    /// Прямоугольник поля ввода (без рамки-заголовка).
    pub input_rect: [f32; 4],
    /// Прямоугольники видимых строк (индекс = позиция в rows, начиная со
    /// scroll_top; длина ≤ MAX_VISIBLE_ROWS).
    pub row_rects: Vec<[f32; 4]>,
    /// Готовый текст видимых строк (параллельно row_rects, FR-088):
    /// заголовок с ellipsis и перенесённый подзаголовок с абсолютными
    /// позициями — рендер НЕ замеряет повторно и НЕ дублирует смещения
    /// (единый источник геометрии: раскладка, отрисовка, hit-тест).
    pub row_texts: Vec<RowText>,
}

/// Текст строки результата в экранных координатах (FR-088).
#[derive(Debug, Clone, PartialEq)]
pub struct RowText {
    /// Заголовок после ellipsis (одна строка).
    pub title: String,
    /// Позиция левого верхнего угла заголовка (лог. px).
    pub title_pos: [f32; 2],
    /// Ширина текстовой зоны строки (bounds клипа ScreenText).
    pub text_width: f32,
    /// Строки подзаголовка после переноса (пусто — подзаголовка нет).
    pub subtitle_lines: Vec<String>,
    /// Позиции строк подзаголовка (параллельно subtitle_lines).
    pub subtitle_pos: Vec<[f32; 2]>,
}

/// Геометрия панели: топ-центр, ширина PANEL_WIDTH (кламп к окну), высота =
/// поле + видимые строки (0 строк — только поле), скролл от panel.scroll_top.
///
/// Раскладка — примитивами `canvas_ui` (FR-054, миграция U5): ширина —
/// `constrain` (min 0, max окно−2×боковая маржа), панель — `stack`
/// (Center/Start), содержимое — `Column` [поле, распорка-паддинг, строки…]
/// с gap 0 (зазоры — дети-распорки, т.к. между строками зазора нет).
///
/// FR-088: высоты строк — адаптивные (замер контента, [`row_height`]):
/// заголовок — ellipsis до одной строки, подзаголовок — перенос по словам;
/// тексты строк готовятся здесь же (`PanelLayout::row_texts`) — рендер
/// берёт готовые строки и позиции. Число видимых строк дополнительно
/// клампится высотой окна (панель не выходит за нижний край —
/// PANEL_BOTTOM_MARGIN; минимум одна строка, далее — скролл).
///
/// Вырожденное окно (ширина/высота ≤ 0 — свёрнутое окно, ширина меньше двух
/// боковых отступов) схлопывает панель в точку — без паники.
pub fn layout(window_w: f32, window_h: f32, panel: &SearchPanel) -> PanelLayout {
    // W3.2 (каталог docs/plans/fr-068-w3-consumer-migration.md): замерщик —
    // канонические shared-точки на вызов (Text-детей нет — замерщик
    // геометрию не читает).
    let mut m = canvas_ui::measure::TextMeasurer::new();
    let mut fs = canvas_render::text::measure_font_system();
    layout_with(window_w, window_h, panel, &mut m, &mut fs)
}

/// То же с ЯВНЫМ замерщиком (для потребителей, уже держащих
/// `measure_font_system` — двойной лок глобального FontSystem невозможен).
pub fn layout_with(
    window_w: f32,
    window_h: f32,
    panel: &SearchPanel,
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> PanelLayout {
    use canvas_ui::geometry::{UiRect, UiVec2};
    use canvas_ui::layout::{constrain, stack, Column, MeasuredItem};

    let window_w = window_w.max(0.0);
    let available = (window_w - 2.0 * PANEL_SIDE_MARGIN).max(0.0);
    if available <= 0.0 || window_h <= 0.0 {
        // Схлопывание в точку (x — центр вырожденного окна).
        let cx = window_w / 2.0;
        let point = [cx, PANEL_TOP_MARGIN, cx, PANEL_TOP_MARGIN];
        return PanelLayout {
            panel_rect: point,
            input_rect: point,
            row_rects: Vec::new(),
            row_texts: Vec::new(),
        };
    }

    // Ширина: желаемая PANEL_WIDTH, потолок — полезная ширина окна.
    // FR-040 v2: на узких окнах (< 1024 px) сужаем панель на 40 px —
    // освобождаем место угловому кластеру (⚙/☼/RU-EN/?), выросшему на
    // одну кнопку (язык). Панель топ-центр, кластер — в углу; на 800 px
    // без сужения они пересекаются (G4-линт FR-054; WASM-аудит верстки).
    let width_target = if window_w < 1024.0 {
        PANEL_WIDTH - 40.0
    } else {
        PANEL_WIDTH
    };
    let width = constrain(
        UiVec2::new(0.0, 0.0),
        UiVec2::new(available, f32::INFINITY),
        UiVec2::new(width_target, 1.0),
    )
    .x;
    // Окно прокрутки клампится к длине списка (scroll_top задаётся извне).
    let scroll_top = panel.scroll_top.min(panel.rows.len());
    let visible_cap = (panel.rows.len() - scroll_top).min(MAX_VISIBLE_ROWS);

    // Панель — top-center вьюпорта (высота наследуется от контента ниже).
    let panel_x = stack(
        UiRect::new(0.0, 0.0, window_w, window_h.max(0.0)),
        UiVec2::new(width, 0.0),
        canvas_ui::layout::HAlign::Center,
        canvas_ui::layout::VAlign::Start,
    )
    .x;
    let panel_top = PANEL_TOP_MARGIN;

    // Внутренний слот (pad на PANEL_PADDING): поле + строки одной колонкой.
    // Крошечная ширина (< 2 отступов) — пустой внутренний слот (не вывернутый):
    // дети получают нулевую ширину, rect'ы остаются невырожденными по осям.
    let inner = canvas_ui::layout::pad(
        UiRect::new(panel_x, panel_top, width, f32::INFINITY),
        canvas_ui::geometry::EdgeInsets::uniform(PANEL_PADDING),
    );
    let inner_w = inner.w.max(0.0);

    // FR-088: адаптивные высоты строк — замер контента (заголовок: ellipsis
    // до одной строки; подзаголовок: перенос по словам) тем же замерщиком,
    // что и раскладка (ui-kit §5: ширины — только TextMeasurer). Тексты
    // готовятся ЗДЕСЬ и попадают в PanelLayout::row_texts — рендер не
    // дублирует ни замер, ни смещения строк.
    let content_top = PANEL_TOP_MARGIN + PANEL_PADDING + INPUT_HEIGHT + PANEL_PADDING;
    // Кламп по высоте окна: панель не выходит за нижний край; минимум одна
    // строка (дальше — скролл рядами, как и раньше).
    let max_bottom = (window_h - PANEL_BOTTOM_MARGIN)
        .max(content_top + row_height(0) + PANEL_PADDING + PANEL_BOTTOM_MARGIN);
    let mut rows_meta: Vec<(f32, String, Vec<String>, f32, f32)> = Vec::new();
    let mut acc = 0.0f32;
    for i in scroll_top..scroll_top + visible_cap {
        let row = &panel.rows[i];
        let text_x = if row.kind == SearchRowKind::Docs {
            ROW_TEXT_X_DOCS
        } else {
            ROW_TEXT_X
        };
        let text_w = (inner_w - text_x - ROW_TEXT_RIGHT_PAD).max(10.0);
        let title = m.ellipsis(
            fs,
            &row.title,
            canvas_render::text::SANS_FAMILY,
            TITLE_FONT_SIZE,
            text_w,
        );
        let sub_lines = if row.subtitle.is_empty() {
            Vec::new()
        } else {
            m.wrap(
                fs,
                &row.subtitle,
                canvas_render::text::SANS_FAMILY,
                SUB_FONT_SIZE,
                text_w,
            )
        };
        let h = row_height(sub_lines.len());
        if !rows_meta.is_empty() && content_top + acc + h > max_bottom {
            break; // строка не влезает — окно прокрутки короче списка
        }
        acc += h;
        rows_meta.push((h, title, sub_lines, text_x, text_w));
    }

    // Колонка gap 0: зазор после поля — вертикальный зазор. W3.2: дети —
    // MeasuredItem; высоты строк — замер контента выше (FR-088). ВАЖНО:
    // вертикальный зазор — именно Fixed{w: 0, h} — Spacer в колонке места
    // НЕ занимает (main-ось колонки — высота; см. оракул
    // measured_column_matches_manual_fixed_oracle в canvas-ui).
    let mut items = vec![MeasuredItem::Fixed {
        w: inner_w,
        h: INPUT_HEIGHT,
    }];
    if !rows_meta.is_empty() {
        items.push(MeasuredItem::Fixed {
            w: 0.0,
            h: PANEL_PADDING,
        });
        for (h, _, _, _, _) in &rows_meta {
            items.push(MeasuredItem::Fixed { w: inner_w, h: *h });
        }
    }
    let rects = Column {
        gap: 0.0,
        ..Column::default()
    }
    .lay_out_measured(inner, &items, m, fs, canvas_render::text::SANS_FAMILY, 12.0);

    let input = &rects[0];
    let input_rect = [input.x, input.y, input.right(), input.bottom()];
    // Строки — дети после поля и распорки (0 строк — срез пуст).
    let row_rects: Vec<[f32; 4]> = rects
        .get(2..)
        .unwrap_or(&[])
        .iter()
        .map(|r| [r.x, r.y, r.right(), r.bottom()])
        .collect();
    let row_texts: Vec<RowText> = row_rects
        .iter()
        .zip(&rows_meta)
        .map(|(r, (_, title, sub_lines, text_x, text_w))| {
            let title_pos = [r[0] + text_x, r[1] + ROW_PAD_V];
            let sub_top = r[1] + ROW_PAD_V + TITLE_LINE_H + TITLE_SUB_GAP;
            let subtitle_pos = (0..sub_lines.len())
                .map(|i| [r[0] + text_x, sub_top + i as f32 * SUB_LINE_H])
                .collect();
            RowText {
                title: title.clone(),
                title_pos,
                text_width: *text_w,
                subtitle_lines: sub_lines.clone(),
                subtitle_pos,
            }
        })
        .collect();
    // Панель заканчивается отступом ниже последнего контента (строки/поле).
    let content_bottom = rects[rects.len() - 1].bottom();
    let y1 = content_bottom + PANEL_PADDING;

    PanelLayout {
        panel_rect: [panel_x, panel_top, panel_x + width, y1],
        input_rect,
        row_rects,
        row_texts,
    }
}

/// Запись сцены для in-memory поиска: нода + заголовок + текст (заметка или
/// пустая строка для файлов).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneEntry<'a> {
    /// Индекс ноды в canvas.nodes.
    pub node: usize,
    /// Имя файла или текст заметки (заголовок).
    pub title: &'a str,
    /// Текст заметки (для text-нод) — пусто для файлов.
    pub text: &'a str,
}

/// In-memory substring-поиск (регистронезависимый) по заголовкам и текстам:
/// находит заметки и ноды, которых нет в FTS-индексе (файлы без текста,
/// ещё не проиндексированные). Возвращает индексы нод (порядок входа,
/// дубликаты нод — один раз). Пустой запрос — пустой результат.
pub fn scan_scene(query: &str, entries: &[SceneEntry<'_>]) -> Vec<usize> {
    if query.is_empty() {
        return Vec::new();
    }
    let needle = query.to_lowercase();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut result = Vec::new();
    for entry in entries {
        if seen.contains(&entry.node) {
            continue; // нода уже в результате — не добавляем дважды
        }
        let title = entry.title.to_lowercase();
        let text = entry.text.to_lowercase();
        if title.contains(needle.as_str()) || text.contains(needle.as_str()) {
            seen.insert(entry.node);
            result.push(entry.node);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPS: f32 = 1e-4;

    /// LAY-W7 (аудит layouts-2026-10 §5): `INPUT_HEIGHT` — псевдоним
    /// `kit::TEXT_FIELD_HEIGHT` (каноническая высота текстового поля S3).
    #[test]
    fn lay_w7_input_height_is_canonical_s3() {
        assert_eq!(INPUT_HEIGHT, canvas_ui::kit::TEXT_FIELD_HEIGHT);
    }

    /// `n` строк-заглушек.
    fn rows(n: usize) -> Vec<SearchRow> {
        (0..n)
            .map(|i| SearchRow {
                title: format!("строка {i}"),
                subtitle: String::new(),
                kind: SearchRowKind::Node,
            })
            .collect()
    }

    /// Открытая панель с `n` результатами (выбор на первой, скролл сверху).
    fn panel_with_rows(n: usize) -> SearchPanel {
        let mut panel = SearchPanel::default();
        panel.open();
        panel.set_results(rows(n));
        panel
    }

    /// Открытая панель с заданным выбором и прокруткой.
    fn panel_configured(rows_n: usize, selected: Option<usize>, scroll_top: usize) -> SearchPanel {
        SearchPanel {
            open: true,
            input: SearchInput::default(),
            rows: rows(rows_n),
            selected,
            scroll_top,
        }
    }

    // ---------- SearchInput ----------

    /// Вставка текста двигает каретку (латиница + кириллица); вставка в
    /// середину строки.
    #[test]
    fn input_insert_and_cursor() {
        let mut input = SearchInput::new();
        input.insert_str("аб");
        assert_eq!(input.query(), "аб");
        assert_eq!(input.caret(), 2, "каретка в СИМВОЛАХ (IN1)");
        input.insert_str("в");
        assert_eq!(input.query(), "абв");
        assert_eq!(input.caret(), 3);
        // Пустая вставка — no-op
        input.insert_str("");
        assert_eq!(input.query(), "абв");

        // Вставка в середину
        let mut input = SearchInput::new();
        input.set_query("abc");
        input.move_left();
        input.insert_str("X");
        assert_eq!(input.query(), "abXc");
        assert_eq!(input.caret(), 3);
    }

    /// Backspace удаляет символ слева (кириллица, 2 байта), на пустой строке
    /// и в начале строки — no-op.
    #[test]
    fn input_backspace_cyrillic() {
        let mut input = SearchInput::new();
        input.set_query("абв");
        assert!(input.backspace(false));
        assert_eq!(input.query(), "аб");
        assert_eq!(input.caret(), 2);
        assert!(input.backspace(false));
        assert!(input.backspace(false));
        assert_eq!(input.query(), "");
        assert_eq!(input.caret(), 0);
        // Пустая строка — no-op без паники
        assert!(!input.backspace(false));

        // Каретка в начале непустой строки — no-op
        let mut input = SearchInput::new();
        input.set_query("абв");
        input.move_to_start();
        assert!(!input.backspace(false));
        assert_eq!(input.query(), "абв");
    }

    /// Backspace не рвёт многобайтный символ: «𝕏» (4 байта) удаляется целиком.
    #[test]
    fn input_backspace_multibyte_whole_char() {
        let mut input = SearchInput::new();
        input.set_query("𝕏");
        assert_eq!(input.caret(), 1, "1 символ (не 4 байта)");
        assert!(input.backspace(false));
        assert_eq!(input.query(), "");
        assert_eq!(input.caret(), 0);

        // Символы вокруг многобайтного: «a𝕏b» — удаляем b, затем 𝕏 целиком
        let mut input = SearchInput::new();
        input.set_query("a𝕏b");
        assert_eq!(input.caret(), 3);
        input.backspace(false);
        assert_eq!(input.query(), "a𝕏");
        assert_eq!(input.caret(), 2);
        input.backspace(false);
        assert_eq!(input.query(), "a");
        assert_eq!(input.caret(), 1);
    }

    /// Ctrl+Backspace: слово + разделители до предыдущего слова (эвристика:
    /// разделитель = пробел или ASCII-знак препинания; кириллица и
    /// многобайтные символы — символы слова).
    #[test]
    fn input_word_backspace() {
        // латиница
        let mut input = SearchInput::new();
        input.set_query("hello world");
        assert!(input.backspace(true));
        assert_eq!(input.query(), "hello");
        assert_eq!(input.caret(), 5);

        // кириллица
        let mut input = SearchInput::new();
        input.set_query("привет мир");
        input.backspace(true);
        assert_eq!(input.query(), "привет");

        // слово + цифры, затем пробел
        let mut input = SearchInput::new();
        input.set_query("смета 2026");
        input.backspace(true);
        assert_eq!(input.query(), "смета");

        // многобайтный «словесный» символ — тоже слово
        let mut input = SearchInput::new();
        input.set_query("мир 𝕏");
        input.backspace(true);
        assert_eq!(input.query(), "мир");

        // каретка в начале — no-op
        let mut input = SearchInput::new();
        input.set_query("hello");
        input.move_to_start();
        assert!(!input.backspace(true));
        assert_eq!(input.query(), "hello");

        // каретка после пробелов: фаза «слова» пуста, удаляются пробелы
        let mut input = SearchInput::new();
        input.set_query("hello   ");
        input.backspace(true);
        assert_eq!(input.query(), "hello");

        // знак препинания — разделитель: «file.txt|» -> «file|»
        let mut input = SearchInput::new();
        input.set_query("file.txt");
        input.backspace(true);
        assert_eq!(input.query(), "file");

        // одно слово без разделителей — удаляется целиком
        let mut input = SearchInput::new();
        input.set_query("абвг");
        input.backspace(true);
        assert_eq!(input.query(), "");
        assert_eq!(input.caret(), 0);
    }

    /// Delete удаляет символ справа (многобайтный — целиком); в конце строки
    /// и на пустой — no-op. (set_query ставит каретку в конец — перед delete
    /// каретка переводится в начало.)
    #[test]
    fn input_delete() {
        let mut input = SearchInput::new();
        input.set_query("abc");
        input.move_to_start(); // каретка в начало
        assert!(input.delete());
        assert_eq!(input.query(), "bc");
        assert_eq!(input.caret(), 0); // каретка на месте

        input.move_to_end();
        assert!(!input.delete());
        assert_eq!(input.query(), "bc");

        // многобайтный символ справа удаляется целиком
        let mut input = SearchInput::new();
        input.set_query("𝕏b");
        input.move_to_start();
        input.delete();
        assert_eq!(input.query(), "b");

        let mut input = SearchInput::new();
        input.set_query("a𝕏"); // каретка 5
        input.move_left(); // перед 𝕏
        assert!(input.delete());
        assert_eq!(input.query(), "a");

        // пустая строка — no-op без паники
        let mut input = SearchInput::new();
        assert!(!input.delete());
    }

    /// Стрелки/Home/End ходят по границам СИМВОЛОВ (IN1): кириллица и
    /// «𝕏» — по одному символу за шаг; за края — no-op.
    #[test]
    fn input_arrows_home_end() {
        let mut input = SearchInput::new();
        input.set_query("абвг"); // 4 символа
        assert_eq!(input.caret(), 4);
        input.move_left();
        assert_eq!(input.caret(), 3);
        input.move_left();
        assert_eq!(input.caret(), 2);
        input.move_right();
        assert_eq!(input.caret(), 3);
        input.move_to_start();
        assert_eq!(input.caret(), 0);
        input.move_left(); // на начале — no-op
        assert_eq!(input.caret(), 0);
        input.move_to_end();
        assert_eq!(input.caret(), 4);
        input.move_right(); // в конце — no-op
        assert_eq!(input.caret(), 4);

        // многобайтные символы: шаг каретки = 1 символ
        let mut input = SearchInput::new();
        input.set_query("𝕏𝕏");
        assert_eq!(input.caret(), 2);
        input.move_left();
        assert_eq!(input.caret(), 1);
        input.move_left();
        input.move_left();
        assert_eq!(input.caret(), 0);
    }

    /// set_query замещает текст, каретка — в конец (в т.ч. многобайтный).
    #[test]
    fn input_set_query() {
        let mut input = SearchInput::new();
        input.set_query("смета");
        assert_eq!(input.query(), "смета");
        assert_eq!(input.caret(), 5);
        input.set_query("𝕏");
        assert_eq!(input.caret(), 1);
        // повторная установка замещает текст
        input.set_query("a");
        assert_eq!(input.query(), "a");
        assert_eq!(input.caret(), 1);
        input.set_query("");
        assert_eq!(input.query(), "");
        assert_eq!(input.caret(), 0);
    }

    /// Все операции на пустом поле — no-op без паники.
    #[test]
    fn input_empty_operations_are_noop() {
        let mut input = SearchInput::new();
        input.insert_str("");
        assert!(!input.backspace(false));
        assert!(!input.backspace(true));
        assert!(!input.delete());
        input.move_left();
        input.move_right();
        input.move_to_start();
        input.move_to_end();
        input.set_query("");
        assert_eq!(input.query(), "");
        assert_eq!(input.caret(), 0);
    }

    // ---------- SearchPanel ----------

    /// open/close переключают видимость; rows и выбор переживают закрытие
    /// (цикл F3 после Esc).
    #[test]
    fn panel_open_close_preserves_rows() {
        let mut panel = SearchPanel::default();
        assert!(!panel.is_open());
        panel.open();
        assert!(panel.is_open());
        panel.set_results(rows(2));
        panel.move_selection(1);
        panel.close();
        assert!(!panel.is_open());
        assert_eq!(panel.rows.len(), 2);
        assert_eq!(panel.selected, Some(1));
        panel.open();
        assert!(panel.is_open());
        assert_eq!(panel.rows.len(), 2); // данные пережили закрытие
    }

    /// set_results: непустой список — выбор на 0 и скролл вверх; пустой —
    /// выбора нет.
    #[test]
    fn panel_set_results_resets_selection() {
        let mut panel = panel_configured(5, Some(4), 3);
        panel.set_results(rows(3));
        assert_eq!(panel.selected, Some(0));
        assert_eq!(panel.scroll_top, 0);
        // пустой список
        panel.set_results(Vec::new());
        assert!(panel.rows.is_empty());
        assert_eq!(panel.selected, None);
        assert_eq!(panel.scroll_top, 0);
    }

    /// move_selection зацикливается: вниз с последней — на первую, вверх с
    /// первой — на последнюю; большой шаг сворачивается по модулю длины;
    /// пустой список — no-op.
    #[test]
    fn panel_move_selection_wraps() {
        let mut panel = panel_with_rows(3);
        assert_eq!(panel.selected, Some(0));
        panel.move_selection(1);
        assert_eq!(panel.selected, Some(1));
        panel.move_selection(1);
        assert_eq!(panel.selected, Some(2));
        // вниз с последней — на первую (зацикливание)
        panel.move_selection(1);
        assert_eq!(panel.selected, Some(0));
        // вверх с первой — на последнюю
        panel.move_selection(-1);
        assert_eq!(panel.selected, Some(2));
        // большой шаг: (2 + 5) mod 3 = 1
        panel.move_selection(5);
        assert_eq!(panel.selected, Some(1));
        // большой шаг назад: (1 - 5) mod 3 = 2
        panel.move_selection(-5);
        assert_eq!(panel.selected, Some(2));

        // пустой список — no-op без паники
        let mut empty = SearchPanel::default();
        empty.set_results(Vec::new());
        empty.move_selection(3);
        empty.move_selection(-2);
        assert_eq!(empty.selected, None);
    }

    /// ensure_selection_visible: выбор за нижней границей окна — окно
    /// подгоняется (выбранная строка последняя видимая); выше окна —
    /// прокрутка к ней; видимая — не трогается; пустой список — no-op.
    #[test]
    fn panel_ensure_selection_visible() {
        // 10 строк, выбор 9 — вне окна [0, 8)
        let mut panel = panel_configured(10, Some(9), 0);
        panel.ensure_selection_visible();
        assert_eq!(panel.scroll_top, 2); // окно [2, 10)
        let sel = panel.selected.unwrap();
        assert!(sel >= panel.scroll_top && sel < panel.scroll_top + MAX_VISIBLE_ROWS);

        // выбор выше окна
        let mut panel = panel_configured(10, Some(3), 5);
        panel.ensure_selection_visible();
        assert_eq!(panel.scroll_top, 3);

        // уже видимая — прокрутка не меняется
        let mut panel = panel_configured(10, Some(5), 0);
        panel.ensure_selection_visible();
        assert_eq!(panel.scroll_top, 0);

        // пустой список — no-op без паники
        let mut empty = SearchPanel::default();
        empty.ensure_selection_visible();
        assert_eq!(empty.scroll_top, 0);
    }

    /// confirm: пустой список — None; иначе Jump(выбранная), без выбора —
    /// Jump(0). cancel — всегда Close.
    #[test]
    fn panel_confirm_and_cancel() {
        let mut panel = SearchPanel::default();
        // пустой список — Enter ничего не прыгает
        assert_eq!(panel.confirm(), None);
        assert_eq!(panel.cancel(), PanelAction::Close);

        panel.set_results(rows(3));
        assert_eq!(panel.confirm(), Some(PanelAction::Jump(0)));
        panel.move_selection(2);
        assert_eq!(panel.confirm(), Some(PanelAction::Jump(2)));

        // выбор сброшен внешне — прыжок к первой
        let no_selection = panel_configured(3, None, 0);
        assert_eq!(no_selection.confirm(), Some(PanelAction::Jump(0)));
        assert_eq!(no_selection.cancel(), PanelAction::Close);
    }

    // ---------- layout ----------

    /// Окно 1280×720, 3 строки: панель топ-центр (ширина 460, x-центр = 640),
    /// поле ввода внутри с отступом, строки ниже поля (высота — замер
    /// контента: строки-заглушки без подзаголовка — только заголовок).
    #[test]
    fn layout_centered_with_three_rows() {
        // строки-заглушки — без подзаголовка: пад + заголовок + пад
        let row_h = row_height(0);
        let panel = panel_with_rows(3);
        let lay = layout(1280.0, 720.0, &panel);
        let pr = lay.panel_rect;
        // ширина и центрирование
        assert!((pr[2] - pr[0] - PANEL_WIDTH).abs() < EPS);
        assert!(((pr[0] + pr[2]) / 2.0 - 640.0).abs() < EPS);
        assert!((pr[1] - PANEL_TOP_MARGIN).abs() < EPS);
        // точные координаты (все значения представимы в f32 точно)
        // LAY-W7: INPUT_HEIGHT мигрировал 36→30 (kit::TEXT_FIELD_HEIGHT) —
        // высота панели и координаты строк ниже поля опустились на 6 px.
        assert_eq!(pr, [410.0, 12.0, 870.0, 153.0]);

        // поле ввода внутри панели с отступом
        let ir = lay.input_rect;
        assert_eq!(ir, [418.0, 20.0, 862.0, 50.0]);
        assert!((ir[1] - (pr[1] + PANEL_PADDING)).abs() < EPS);
        assert!((pr[2] - PANEL_PADDING - ir[2]).abs() < EPS);
        assert!((ir[3] - ir[1] - INPUT_HEIGHT).abs() < EPS);

        // три строки
        assert_eq!(lay.row_rects.len(), 3);
        let first = lay.row_rects[0];
        assert!((first[1] - (ir[3] + PANEL_PADDING)).abs() < EPS); // ниже поля
        assert!((first[3] - first[1] - row_h).abs() < EPS);
        assert!((first[0] - ir[0]).abs() < EPS);
        assert!((first[2] - ir[2]).abs() < EPS);
        // строки идут подряд
        assert!((lay.row_rects[1][1] - first[3]).abs() < EPS);
        assert_eq!(lay.row_rects[0], [418.0, 58.0, 862.0, 87.0]);
        assert_eq!(lay.row_rects[2], [418.0, 116.0, 862.0, 145.0]);
        // панель заканчивается отступом ниже последней строки
        let last = lay.row_rects[2];
        assert!((pr[3] - (last[3] + PANEL_PADDING)).abs() < EPS);
        // FR-088: тексты строк параллельны rect'ам, позиции — внутри строки
        assert_eq!(lay.row_texts.len(), 3);
        for (r, rt) in lay.row_rects.iter().zip(&lay.row_texts) {
            assert!((rt.title_pos[0] - (r[0] + ROW_TEXT_X)).abs() < EPS);
            assert!((rt.title_pos[1] - (r[1] + ROW_PAD_V)).abs() < EPS);
            assert!(rt.subtitle_lines.is_empty());
        }
    }

    /// 12 строк — максимум 8 видимых прямоугольников (окно 720 — по высоте
    /// всё влезает), каждый — высоты заголовочной строки.
    #[test]
    fn layout_caps_rows_at_eight() {
        let panel = panel_with_rows(12);
        let lay = layout(1280.0, 720.0, &panel);
        assert_eq!(lay.row_rects.len(), MAX_VISIBLE_ROWS);
        for r in &lay.row_rects {
            assert!((r[3] - r[1] - row_height(0)).abs() < EPS);
        }
    }

    /// FR-088: длинный подзаголовок переносится — строка ВЫШЕ однострочной,
    /// высота = row_height(число строк), позиции строк подзаголовка идут
    /// подряд ниже заголовка.
    #[test]
    fn adaptive_row_wraps_subtitle() {
        let mut panel = SearchPanel::default();
        panel.open();
        panel.set_results(vec![SearchRow::node(
            "нода".to_owned(),
            "длинный подзаголовок с результатом поиска, который заведомо не "
                .repeat(6)
                .to_string(),
        )]);
        let lay = layout(1280.0, 720.0, &panel);
        assert_eq!(lay.row_rects.len(), 1);
        let rt = &lay.row_texts[0];
        assert!(rt.subtitle_lines.len() >= 2, "перенос состоялся");
        let rect = lay.row_rects[0];
        assert!((rect[3] - rect[1] - row_height(rt.subtitle_lines.len())).abs() < EPS);
        // заголовок сверху, строки подзаголовка ниже с шагом SUB_LINE_H
        assert!((rt.title_pos[1] - (rect[1] + ROW_PAD_V)).abs() < EPS);
        for (i, pos) in rt.subtitle_pos.iter().enumerate() {
            let expected =
                rect[1] + ROW_PAD_V + TITLE_LINE_H + TITLE_SUB_GAP + i as f32 * SUB_LINE_H;
            assert!((pos[1] - expected).abs() < EPS, "строка {i}");
            assert!((pos[0] - (rect[0] + ROW_TEXT_X)).abs() < EPS);
        }
    }

    /// FR-088: длинный заголовок обрезается ellipsis до одной строки —
    /// высота строки не растёт, текст короче источника.
    #[test]
    fn adaptive_row_title_ellipsis() {
        let long_title = "очень длинное имя ноды без пробелов ".repeat(12);
        let mut panel = SearchPanel::default();
        panel.open();
        panel.set_results(vec![SearchRow::node(long_title.clone(), String::new())]);
        let lay = layout(1280.0, 720.0, &panel);
        let rt = &lay.row_texts[0];
        assert!(rt.title.chars().count() < long_title.chars().count());
        assert!(rt.title.ends_with('…'), "хвост — многоточие");
        assert!((lay.row_rects[0][3] - lay.row_rects[0][1] - row_height(0)).abs() < EPS);
    }

    /// FR-088: строка-документация — текст правее (зона бейджа «?»),
    /// ширина текстовой зоны соответствует отступу.
    #[test]
    fn adaptive_docs_row_text_offset() {
        let mut panel = SearchPanel::default();
        panel.open();
        panel.set_results(vec![
            SearchRow::node("нода".to_owned(), String::new()),
            SearchRow::docs("FAQ".to_owned(), "Документация · ответ".to_owned()),
        ]);
        let lay = layout(1280.0, 720.0, &panel);
        assert_eq!(lay.row_texts.len(), 2);
        assert!((lay.row_texts[0].title_pos[0] - (lay.row_rects[0][0] + ROW_TEXT_X)).abs() < EPS);
        assert!(
            (lay.row_texts[1].title_pos[0] - (lay.row_rects[1][0] + ROW_TEXT_X_DOCS)).abs() < EPS
        );
    }

    /// FR-088: кламп панели по высоте окна — на низком окне видимых строк
    /// меньше списка (панель не выходит за нижний край), минимум одна строка.
    #[test]
    fn panel_height_clamped_to_window() {
        let panel = panel_with_rows(20);
        // 300 px: строки по 29 — влезает 8 (58 + 8·29 = 290 ≤ 292, 9-я — нет).
        // LAY-W7: INPUT_HEIGHT 36→30 сдвинул верх строк с 64 до 58 — теперь
        // в 300-пиксельное окно помещается на одну строку больше.
        let lay = layout(1280.0, 300.0, &panel);
        assert_eq!(lay.row_rects.len(), 8);
        assert!(lay.panel_rect[3] <= 300.0 + EPS, "панель внутри окна");

        // совсем низкое окно — минимум одна строка
        let lay = layout(1280.0, 80.0, &panel);
        assert_eq!(lay.row_rects.len(), 1);
    }

    /// Узкое окно 300×600: ширина клампится к 300 − 2×12 = 276; широкое окно —
    /// панель полной ширины PANEL_WIDTH.
    #[test]
    fn layout_narrow_window_clamps_width() {
        let panel = panel_with_rows(3);
        let lay = layout(300.0, 600.0, &panel);
        let pr = lay.panel_rect;
        assert!((pr[2] - pr[0] - 276.0).abs() < EPS);
        assert!(((pr[0] + pr[2]) / 2.0 - 150.0).abs() < EPS);
        assert_eq!(lay.row_rects.len(), 3);

        // широкое окно — полная ширина
        let lay = layout(2000.0, 1000.0, &panel);
        assert!((lay.panel_rect[2] - lay.panel_rect[0] - PANEL_WIDTH).abs() < EPS);
    }

    /// scroll_top задаёт окно строк: 10 строк со scroll_top=5 — прямоугольники
    /// строк 5..10; scroll_top за пределами списка клампится (строк нет).
    #[test]
    fn layout_respects_scroll_window() {
        let mut panel = panel_with_rows(10);
        panel.scroll_top = 5;
        let lay = layout(1280.0, 720.0, &panel);
        // окно прокрутки: строки 5..10 — 5 видимых
        assert_eq!(lay.row_rects.len(), 5);
        let rows_top = PANEL_TOP_MARGIN + PANEL_PADDING + INPUT_HEIGHT + PANEL_PADDING;
        for (i, r) in lay.row_rects.iter().enumerate() {
            assert!((r[1] - (rows_top + i as f32 * row_height(0))).abs() < EPS);
            assert!((r[3] - r[1] - row_height(0)).abs() < EPS);
        }

        // scroll_top больше длины списка — кламп, видимых строк нет
        panel.scroll_top = 20;
        let lay = layout(1280.0, 720.0, &panel);
        assert!(lay.row_rects.is_empty());
    }

    /// Вырожденное окно (ширина/высота ≤ 0, ширина меньше двух боковых
    /// отступов) — панель схлопывается в точку, без паники; крошечная ширина
    /// не выворачивает внутренние rect'ы.
    #[test]
    fn layout_degenerate_window_collapses() {
        let panel = panel_with_rows(3);
        for (w, h) in [
            (0.0, 600.0),
            (-40.0, 600.0),
            (1280.0, 0.0),
            (1280.0, -5.0),
            (24.0, 600.0),
        ] {
            let lay = layout(w, h, &panel);
            let pr = lay.panel_rect;
            assert!((pr[2] - pr[0]).abs() < EPS, "нулевая ширина при ({w}, {h})");
            assert!(lay.row_rects.is_empty(), "нет строк при ({w}, {h})");
        }

        // крошечное окно (30 px): ширина 6, внутренние rect'ы не вывернуты
        let lay = layout(30.0, 600.0, &panel);
        assert!((lay.panel_rect[2] - lay.panel_rect[0] - 6.0).abs() < EPS);
        assert!(lay.input_rect[2] >= lay.input_rect[0]);
        assert_eq!(lay.row_rects.len(), 3);
    }

    /// 0 строк: панель — только поле ввода (высота = отступ + поле + отступ).
    #[test]
    fn layout_without_rows_is_input_only() {
        let mut panel = SearchPanel::default();
        panel.open();
        panel.set_results(Vec::new());
        let lay = layout(1280.0, 720.0, &panel);
        assert!(lay.row_rects.is_empty());
        let pr = lay.panel_rect;
        let expected_h = PANEL_TOP_MARGIN + PANEL_PADDING + INPUT_HEIGHT + PANEL_PADDING;
        assert!((pr[3] - expected_h).abs() < EPS);
        assert!((pr[3] - 58.0).abs() < EPS); // 12 + 8 + 30 + 8 (LAY-W7: INPUT_HEIGHT 36→30)
    }

    /// FR-054 (G4-линт миграции U5): вьюпорты 1280×800 / 1024×640 / 800×560 ×
    /// пустой/полный список/скролл — панель внутри вьюпорта (боковые поля),
    /// строки внутри панели, поле и строки попарно не пересекаются.
    #[test]
    fn g4_lint_viewports() {
        use canvas_ui::geometry::{UiPoint, UiRect};
        let viewports = [[1280.0, 800.0], [1024.0, 640.0], [800.0, 560.0]];
        let mut panel_full = panel_with_rows(12);
        panel_full.scroll_top = 3;
        let panels = [SearchPanel::default(), panel_with_rows(3), panel_full];
        for panel in &panels {
            for vp in viewports {
                let lay = layout(vp[0], vp[1], panel);
                let pr = lay.panel_rect;
                // Панель внутри вьюпорта с боковыми полями (кламп ширины).
                assert!(pr[0] >= PANEL_SIDE_MARGIN - 0.01, "left {vp:?}");
                assert!(pr[2] <= vp[0] - PANEL_SIDE_MARGIN + 0.01, "right {vp:?}");
                assert!(pr[1] >= PANEL_TOP_MARGIN - 0.01, "top {vp:?}");
                assert!(pr[3] <= vp[1] + 0.01, "bottom {vp:?}");
                // Rect-in-rect: поле и строки внутри панели (границы включительно).
                let inside = |r: [f32; 4]| {
                    r[0] >= pr[0] - 0.01
                        && r[1] >= pr[1] - 0.01
                        && r[2] <= pr[2] + 0.01
                        && r[3] <= pr[3] + 0.01
                };
                assert!(inside(lay.input_rect), "поле внутри панели {vp:?}");
                let ir = lay.input_rect;
                let input_r =
                    UiRect::from_min_max(UiPoint::new(ir[0], ir[1]), UiPoint::new(ir[2], ir[3]));
                for r in &lay.row_rects {
                    assert!(inside(*r), "строка внутри панели {vp:?}");
                    let row_r =
                        UiRect::from_min_max(UiPoint::new(r[0], r[1]), UiPoint::new(r[2], r[3]));
                    assert!(!row_r.intersects(&input_r), "строка ∩ поле {vp:?}");
                }
            }
        }
    }

    // ---------- scan_scene ----------

    /// Латиница: регистр запроса и заголовка не важен.
    #[test]
    fn scan_latin_case_insensitive() {
        let entries = [SceneEntry {
            node: 0,
            title: "Report.TXT",
            text: "",
        }];
        assert_eq!(scan_scene("report", &entries), vec![0usize]);
        assert_eq!(scan_scene("REPORT", &entries), vec![0usize]);
        assert_eq!(scan_scene("rep", &entries), vec![0usize]);
        assert_eq!(scan_scene("missing", &entries), Vec::<usize>::new());
    }

    /// Кириллица: регистронезависимый substring (критерий приёмки T14 —
    /// «смета» находит «Смета_2026.xlsx»).
    #[test]
    fn scan_cyrillic_case_insensitive() {
        let entries = [SceneEntry {
            node: 3,
            title: "Смета_2026.xlsx",
            text: "",
        }];
        assert_eq!(scan_scene("смета", &entries), vec![3usize]);
        assert_eq!(scan_scene("СМЕТА", &entries), vec![3usize]);
        assert_eq!(scan_scene("2026", &entries), vec![3usize]);
        assert_eq!(scan_scene("план", &entries), Vec::<usize>::new());
    }

    /// Матч по text (заметка), не только по title.
    #[test]
    fn scan_matches_text_not_only_title() {
        let entries = [
            SceneEntry {
                node: 0,
                title: "Заметка про бюджет",
                text: "здесь упоминается смета 2026",
            },
            SceneEntry {
                node: 1,
                title: "План",
                text: "черновик",
            },
        ];
        // матч только по text
        assert_eq!(scan_scene("упоминается", &entries), vec![0usize]);
        // матч по title
        assert_eq!(scan_scene("план", &entries), vec![1usize]);
        // матч по text второй записи
        assert_eq!(scan_scene("черновик", &entries), vec![1usize]);
        assert_eq!(scan_scene("нет такого", &entries), Vec::<usize>::new());
    }

    /// Пустой запрос — пустой результат; пустой список записей — пусто.
    #[test]
    fn scan_empty_query_or_entries() {
        let entries = [SceneEntry {
            node: 0,
            title: "смета",
            text: "",
        }];
        assert_eq!(scan_scene("", &entries), Vec::<usize>::new());
        assert!(scan_scene("смета", &[]).is_empty());
    }

    /// Дубликат ноды (две записи с одним node) — в результате один раз.
    #[test]
    fn scan_deduplicates_nodes() {
        let entries = [
            SceneEntry {
                node: 2,
                title: "смета",
                text: "",
            },
            SceneEntry {
                node: 2,
                title: "другой заголовок",
                text: "тоже про смету",
            },
            SceneEntry {
                node: 5,
                title: "смета_2026",
                text: "",
            },
        ];
        assert_eq!(scan_scene("смет", &entries), vec![2usize, 5usize]);
    }

    /// Порядок результата — порядок первых вхождений во входном списке.
    #[test]
    fn scan_preserves_entry_order() {
        let entries = [
            SceneEntry {
                node: 9,
                title: "смета",
                text: "",
            },
            SceneEntry {
                node: 4,
                title: "прочее",
                text: "",
            },
            SceneEntry {
                node: 7,
                title: "СМЕТА",
                text: "",
            },
        ];
        assert_eq!(scan_scene("смета", &entries), vec![9usize, 7usize]);
    }
}
