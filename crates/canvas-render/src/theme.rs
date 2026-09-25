//! Палитра темы интерфейса (тёмная/светлая): все тема-зависимые цвета
//! рендера в одной структуре. Значения — в sRGB 0..1 (так же, как
//! прежние константы в cards.rs/text.rs: GPU-пайплайны пишут их как есть
//! в sRGB-surface). Clear-color конвертируется в linear (`clear_color`).

use crate::config::srgb_to_linear;
use crate::Color;

/// Палитра темы: фон, сетка, карточки, текст, UI-оверлеи.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    /// Фон канваса, sRGB-байты (#1e1e22 тёмный / #f5f5f7 светлый).
    pub background: [u8; 3],
    /// Мелкая сетка, sRGB 0..1.
    pub grid_minor: [f32; 3],
    /// Крупная сетка, sRGB 0..1.
    pub grid_major: [f32; 3],
    /// Заливка карточки по умолчанию.
    pub card_fill: [f32; 4],
    /// FR-075 (выравнивание с прототипом): волосяная рамка КАЖДОЙ карточки
    /// — `prototype-unified.html` рисует контур `cardEdge` 1.25 px на всех
    /// нодах (тёмная #30323c / светлая #d5dae3), а не только на статусных.
    pub card_edge: [f32; 4],
    /// FR-075: цвет линии-разделителя зон (низ шапки / верх полосы
    /// «ИТОГ») — прототип `zoneTop` #2a2b33 / #e3e8f0.
    pub zone_top: [f32; 4],
    /// FR-075: фон-тинт полосы результата «ИТОГ» — прототип
    /// `flowBg` rgba(49,184,166,.06) / светлая rgba(31,143,128,.07).
    pub strip_tint: [f32; 4],
    /// FR-075: бирюзовый цвет значения результата («ИТОГ»/Σ) — прототип
    /// `value`/`--flow` #31b8a6 / светлая #1f8f80 (семейство value-рёбер).
    pub result_value: Color,
    /// FR-075: заливка чипа категории «РАСЧЁТ» (Text-нода с формулами) —
    /// прототип `chips.calc` #4da3ff / #2f7fe8.
    pub chip_calc: [f32; 4],
    /// FR-075: заливка чипа «ЗАМЕТКА» (Text-нода без формул) — прототип
    /// `chips.note` #a78bfa / #8b5cf6.
    pub chip_note: [f32; 4],
    /// FR-075: заливка чипа «ФАЙЛ» (file-нода) — прототип `chips.file`
    /// #d9a13c / #c98a1e.
    pub chip_file: [f32; 4],
    /// FR-075: зебра строк таблицы — прототип `zebra` rgba(255,255,255,.04)
    /// (светлые прогонки; DOM rgba(20,24,40,.03)); прежняя
    /// `search_row_fill` остаётся поиску.
    pub zebra_fill: [f32; 4],
    /// Подложка бокса редактирования лейбла связи.
    pub edge_edit_fill: [f32; 4],
    /// Подложка лейбла связи на кривой.
    pub edge_label_fill: [f32; 4],
    /// Фон меню и панелей.
    pub menu_fill: [f32; 4],
    /// Поле ввода панели поиска.
    pub search_input_fill: [f32; 4],
    /// Невыделенная строка результатов поиска (полупрозрачная подложка).
    pub search_row_fill: [f32; 4],
    /// Невыделенная карточка строки палитры шаблонов (CR-011).
    pub palette_row_fill: [f32; 4],
    /// Невыделенный чип категории палитры шаблонов (CR-011).
    pub palette_chip_fill: [f32; 4],
    /// Плитка иконки строки палитры шаблонов (CR-011).
    pub palette_tile_fill: [f32; 4],
    /// Выбранная строка/чип палитры шаблонов (CR-011).
    pub palette_selected_fill: [f32; 4],
    /// Hover строки палитры шаблонов (CR-011).
    pub palette_hover_fill: [f32; 4],
    /// Рамка дока палитры шаблонов (CR-011).
    pub palette_border: [f32; 4],
    /// Цвет заголовка карточки.
    pub title: Color,
    /// Цвет иконки в заголовке.
    pub icon: Color,
    /// Цвет тела заметки.
    pub body: Color,
    /// Цвет лейбла связи.
    pub edge_label: Color,
    /// Акцентный цвет ссылок `[text](url)` в теле заметки (GFM).
    pub link: Color,
    /// Цвет текста цитаты (`> …`) в теле заметки (GFM).
    pub quote: Color,
    /// Цвет текста фенса кода в теле заметки (GFM).
    pub code_text: Color,
    /// FR-061 этап D (O-5): цвет имени функции в формуле (прототип
    /// #c792ea, курсив) — тёмная тема.
    pub formula_fn: Color,
    /// FR-061 этап D (O-5): цвет операторов в формуле (прототип #666a7c).
    pub formula_op: Color,
    /// Фон фенса кода, sRGB 0..1 (GFM).
    pub gfm_code_fill: [f32; 4],
    /// Бар цитаты, sRGB 0..1 (GFM).
    pub gfm_quote_fill: [f32; 4],
    /// Буллиты списков, зачёркивание, чекбоксы и линия `---`, sRGB 0..1 (GFM).
    pub gfm_muted_fill: [f32; 4],
    /// Заливка рамки группы — акцент, слабая прозрачность.
    pub group_fill: [f32; 4],
    /// Рамка группы — акцент, средняя прозрачность.
    pub group_border: [f32; 4],
    /// Направляющая магнитной раскладки, источник «сосед» (FR-038, п.18):
    /// маджента; контраст к фону ≥ 3:1 на обеих темах (урок CR-007,
    /// WCAG 1.4.11 — нетекстовая графика) — проверяется тестами.
    pub guide_align: [f32; 4],
    /// Направляющая от сетки (источник grid): тот же тон приглушённее —
    /// отличима от «соседской» ещё и штрихом (шейдер guides.wgsl).
    pub guide_grid: [f32; 4],
    /// Акцент — единый источник акцентного семейства (PRD-0006 G4, FR-046):
    /// выделение, каретка/селекция текста, фокус-связь. Значение — из
    /// примитива `canvas_core::tokens::ACCENT`; обе темы совпадают (сегодняшняя
    /// единая константа), расхождение — решение владельца (v2).
    pub accent: [f32; 4],
    /// Заливка выделения текста в редакторе (T7) — акцент α0.35.
    /// Источник: `TEXT_SELECTION_FILL` renderer.rs:41.
    pub selection_fill: [f32; 4],
    /// Фон-подсветка `==текст==`. Источник: `HIGHLIGHT_FILL` renderer.rs:43.
    pub highlight: [f32; 4],
    /// Фон what-if строки (FR-017). Источник: `WHATIF_FILL` renderer.rs:46.
    pub whatif_fill: [f32; 4],
    /// Дельта-бейдж what-if (FR-017). Источник: `WHATIF_BADGE_COLOR` renderer.rs:49.
    pub whatif_badge: Color,
    /// Красный строки результата с ошибкой (FR-013). Источник: `RESULT_ERROR_COLOR` text.rs:108.
    pub error: Color,
    /// Цвет HUD F3. Источник: `HUD_COLOR` text.rs:140.
    pub hud: Color,
    /// Затемнение фона main stage (FR-042 §7.5, альфа-аппроксимация):
    /// тёмная 0.6 / светлая 0.5 (уточнение — демо E3, коридор PRD 0.55–0.65).
    pub stage_dim: [f32; 4],
    /// PRD-0007 (F-4/AC-3.4): цвет листа-константы в explain-дереве
    /// (полоса карточки и ветка к листу). Источник: `EXPLAIN_LEAF` /
    /// `EXPLAIN_LEAF_LIGHT` tokens.rs — по теме (контраст ≥ 3:1).
    pub explain_leaf: [f32; 4],
    /// FR-053 (U3 PRD-0009 F-9): слоты состояний контролов (пилоты: галерея
    /// схем, what-if бар, empty-state). Значения — из примитивов
    /// `canvas_core::tokens::CONTROL_*` (ноль скачка: бывшие hover_fill/dim).
    /// Hover строки списка/вторичной кнопки.
    pub control_hover_fill: [f32; 4],
    /// Hover primary-кнопки (обе темы: primary не дифференцирован).
    pub control_primary_hover_fill: [f32; 4],
    /// Выбранная строка списка (сегодня = hover — ноль скачка; семантика
    /// разделена слотами, дифференциация — v2).
    pub control_selected_fill: [f32; 4],
    /// Текст disabled-кнопок (бывшая локальная dim app.rs).
    pub control_disabled_text: Color,
}

impl ThemeColors {
    /// Тема тёмная? (по фону: тёмный фон #1e1e22 / светлый #f5f5f7).
    /// Управляет выбором палитры пресетов и «чернил» авто-контраста.
    pub fn is_dark(&self) -> bool {
        self.background.iter().map(|&c| c as u32).sum::<u32>() < 384
    }

    /// Тёмная палитра (базовая, SPEC T1) — прежние константы рендера.
    pub fn dark() -> Self {
        Self {
            background: [0x1e, 0x1e, 0x22],
            grid_minor: [0.141, 0.141, 0.161],
            grid_major: [0.169, 0.169, 0.190],
            card_fill: [0.149, 0.149, 0.173, 1.0],
            card_edge: [
                0x30 as f32 / 255.0,
                0x32 as f32 / 255.0,
                0x3c as f32 / 255.0,
                1.0,
            ],
            zone_top: [
                0x2a as f32 / 255.0,
                0x2b as f32 / 255.0,
                0x33 as f32 / 255.0,
                1.0,
            ],
            strip_tint: [0.121, 0.722, 0.651, 0.06],
            result_value: Color::rgb(0x31, 0xb8, 0xa6),
            chip_calc: [0.302, 0.639, 1.0, 1.0],
            chip_note: [0.655, 0.545, 0.980, 1.0],
            chip_file: [0.851, 0.627, 0.235, 1.0],
            zebra_fill: [1.0, 1.0, 1.0, 0.04],
            edge_edit_fill: [0.13, 0.13, 0.16, 0.95],
            edge_label_fill: [0.11, 0.11, 0.13, 0.85],
            menu_fill: [0.11, 0.11, 0.13, 0.97],
            search_input_fill: [0.16, 0.17, 0.20, 1.0],
            search_row_fill: [0.13, 0.14, 0.17, 0.55],
            palette_row_fill: [0.13, 0.14, 0.18, 0.65],
            palette_chip_fill: [0.17, 0.18, 0.22, 0.8],
            palette_tile_fill: [0.20, 0.22, 0.28, 0.9],
            palette_selected_fill: [0.18, 0.29, 0.48, 0.95],
            palette_hover_fill: [0.24, 0.30, 0.42, 0.6],
            palette_border: [0.22, 0.24, 0.30, 0.9],
            title: Color::rgb(0xe6, 0xe6, 0xe6),
            icon: Color::rgb(0x9a, 0xaa, 0xbf),
            body: Color::rgb(0xd4, 0xd4, 0xd4),
            edge_label: Color::rgb(0xcf, 0xd8, 0xe3),
            link: Color::rgb(0x6c, 0xb6, 0xff),
            quote: Color::rgb(0x9a, 0x9a, 0xa2),
            code_text: Color::rgb(0xa5, 0xd6, 0xff),
            // FR-061 D (O-5): прототип ux-node-body-fill (.fn/.op).
            formula_fn: Color::rgb(0xc7, 0x92, 0xea),
            formula_op: Color::rgb(0x66, 0x6a, 0x7c),
            gfm_code_fill: [0.22, 0.23, 0.27, 1.0],
            gfm_quote_fill: [0.45, 0.48, 0.55, 1.0],
            gfm_muted_fill: [0.55, 0.57, 0.62, 1.0],
            group_fill: [0.396, 0.612, 0.969, 0.08],
            group_border: [0.396, 0.612, 0.969, 0.40],
            // Маджента магнитной раскладки: контраст к фону #1e1e22 ≈ 5.2:1
            // (подложка grid — приглушённый тон того же тона, ≈ 4.0:1)
            guide_align: [1.0, 0.18, 0.83, 1.0],
            guide_grid: [0.86, 0.16, 0.72, 1.0],
            // FR-046: слоты v2 — из примитивов design/tokens (ноль скачка)
            accent: canvas_core::tokens::ACCENT,
            selection_fill: [
                canvas_core::tokens::ACCENT[0],
                canvas_core::tokens::ACCENT[1],
                canvas_core::tokens::ACCENT[2],
                canvas_core::tokens::ALPHA_35,
            ],
            highlight: canvas_core::tokens::HIGHLIGHT_FILL,
            whatif_fill: canvas_core::tokens::WHATIF_FILL,
            whatif_badge: Color::rgb(
                canvas_core::tokens::WHATIF_BADGE[0],
                canvas_core::tokens::WHATIF_BADGE[1],
                canvas_core::tokens::WHATIF_BADGE[2],
            ),
            error: Color::rgb(
                canvas_core::tokens::ERROR[0],
                canvas_core::tokens::ERROR[1],
                canvas_core::tokens::ERROR[2],
            ),
            hud: Color::rgb(
                canvas_core::tokens::HUD[0],
                canvas_core::tokens::HUD[1],
                canvas_core::tokens::HUD[2],
            ),
            stage_dim: [0.02, 0.02, 0.04, 0.6],
            // PRD-0007: лист explain-дерева — #9fd6ff (прототип v4).
            explain_leaf: canvas_core::tokens::EXPLAIN_LEAF,
            // FR-053 (U3 F-9): слоты состояний контролов — из примитивов.
            control_hover_fill: canvas_core::tokens::CONTROL_HOVER_FILL_DARK,
            control_primary_hover_fill: canvas_core::tokens::CONTROL_PRIMARY_HOVER_FILL,
            control_selected_fill: canvas_core::tokens::CONTROL_SELECTED_FILL_DARK,
            control_disabled_text: Color::rgb(
                canvas_core::tokens::CONTROL_DISABLED_TEXT[0],
                canvas_core::tokens::CONTROL_DISABLED_TEXT[1],
                canvas_core::tokens::CONTROL_DISABLED_TEXT[2],
            ),
        }
    }

    /// Светлая тема: тёмный текст на белых карточках, сетка светло-серая.
    pub fn light() -> Self {
        Self {
            background: [0xf5, 0xf5, 0xf7],
            // Контраст сетки ~50% к фону (симметрично тёмной теме)
            grid_minor: [0.851, 0.851, 0.878],
            grid_major: [0.769, 0.769, 0.812],
            card_fill: [0.984, 0.984, 0.992, 1.0],
            // FR-075: прототип light — cardEdge #d5dae3, zoneTop #e3e8f0,
            // flowBg rgba(31,143,128,.07), value #1f8f80, chips light-ряд.
            card_edge: [
                0xd5 as f32 / 255.0,
                0xda as f32 / 255.0,
                0xe3 as f32 / 255.0,
                1.0,
            ],
            zone_top: [
                0xe3 as f32 / 255.0,
                0xe8 as f32 / 255.0,
                0xf0 as f32 / 255.0,
                1.0,
            ],
            strip_tint: [0.122, 0.561, 0.502, 0.07],
            result_value: Color::rgb(0x1f, 0x8f, 0x80),
            chip_calc: [0.184, 0.498, 0.910, 1.0],
            chip_note: [0.545, 0.361, 0.965, 1.0],
            chip_file: [0.788, 0.541, 0.118, 1.0],
            zebra_fill: [0.078, 0.094, 0.157, 0.03],
            edge_edit_fill: [0.97, 0.97, 0.98, 0.95],
            edge_label_fill: [0.95, 0.95, 0.97, 0.85],
            menu_fill: [0.98, 0.98, 0.99, 0.97],
            search_input_fill: [0.90, 0.90, 0.93, 1.0],
            search_row_fill: [0.88, 0.88, 0.92, 0.55],
            palette_row_fill: [0.88, 0.88, 0.92, 0.65],
            palette_chip_fill: [0.90, 0.90, 0.93, 0.9],
            palette_tile_fill: [0.84, 0.86, 0.90, 1.0],
            palette_selected_fill: [0.18, 0.29, 0.48, 0.95],
            palette_hover_fill: [0.75, 0.80, 0.90, 0.6],
            palette_border: [0.75, 0.77, 0.82, 0.9],
            title: Color::rgb(0x20, 0x20, 0x24),
            icon: Color::rgb(0x6a, 0x7a, 0x8f),
            body: Color::rgb(0x38, 0x38, 0x3e),
            edge_label: Color::rgb(0x2a, 0x35, 0x42),
            link: Color::rgb(0x09, 0x69, 0xda),
            quote: Color::rgb(0x59, 0x63, 0x6e),
            code_text: Color::rgb(0x05, 0x50, 0xae),
            // FR-061 D (O-5): светлые варианты темнее (контраст на карточке).
            formula_fn: Color::rgb(0x6b, 0x21, 0xa8),
            formula_op: Color::rgb(0x4b, 0x55, 0x63),
            gfm_code_fill: [0.93, 0.94, 0.96, 1.0],
            gfm_quote_fill: [0.65, 0.69, 0.76, 1.0],
            gfm_muted_fill: [0.60, 0.63, 0.68, 1.0],
            group_fill: [0.396, 0.612, 0.969, 0.10],
            group_border: [0.36, 0.55, 0.90, 0.50],
            // Маджента на светлом фоне темнее (контраст ≈ 4.7:1;
            // приглушённый grid — ≈ 6.1:1) — урок CR-007: обе темы равны
            guide_align: [0.784, 0.118, 0.612, 1.0],
            guide_grid: [0.66, 0.10, 0.52, 1.0],
            // FR-046: слоты v2 — из примитивов (значения обеих тем совпадают
            // с прежними едиными константами — ноль скачка)
            accent: canvas_core::tokens::ACCENT,
            selection_fill: [
                canvas_core::tokens::ACCENT[0],
                canvas_core::tokens::ACCENT[1],
                canvas_core::tokens::ACCENT[2],
                canvas_core::tokens::ALPHA_35,
            ],
            highlight: canvas_core::tokens::HIGHLIGHT_FILL,
            whatif_fill: canvas_core::tokens::WHATIF_FILL,
            whatif_badge: Color::rgb(
                canvas_core::tokens::WHATIF_BADGE[0],
                canvas_core::tokens::WHATIF_BADGE[1],
                canvas_core::tokens::WHATIF_BADGE[2],
            ),
            error: Color::rgb(
                canvas_core::tokens::ERROR[0],
                canvas_core::tokens::ERROR[1],
                canvas_core::tokens::ERROR[2],
            ),
            hud: Color::rgb(
                canvas_core::tokens::HUD[0],
                canvas_core::tokens::HUD[1],
                canvas_core::tokens::HUD[2],
            ),
            stage_dim: [0.02, 0.02, 0.04, 0.5],
            // PRD-0007: лист explain-дерева — затемнённый слот (AC-3.4:
            // контраст к светлому фону ≥ 3:1).
            explain_leaf: canvas_core::tokens::EXPLAIN_LEAF_LIGHT,
            // FR-053 (U3 F-9): слоты состояний контролов — из примитивов.
            control_hover_fill: canvas_core::tokens::CONTROL_HOVER_FILL_LIGHT,
            control_primary_hover_fill: canvas_core::tokens::CONTROL_PRIMARY_HOVER_FILL,
            control_selected_fill: canvas_core::tokens::CONTROL_SELECTED_FILL_LIGHT,
            control_disabled_text: Color::rgb(
                canvas_core::tokens::CONTROL_DISABLED_TEXT[0],
                canvas_core::tokens::CONTROL_DISABLED_TEXT[1],
                canvas_core::tokens::CONTROL_DISABLED_TEXT[2],
            ),
        }
    }

    /// Пара «чернил» для авто-контраста текста на цветных карточках:
    /// чистые экстремумы (чёрный/белый). Гарантия WCAG: лучший из пары даёт
    /// ≥ 4.5:1 с ЛЮБОЙ заливкой (минимум min-max 4.58 на фоне ~#777).
    /// Вид темы на обычных заливках сохраняет не выбор чернил, а смешение
    /// исходного цвета к экстремуму внутри `contrast::ensure_contrast` —
    /// чернила — последний рубеж для мёртвой зоны средне-серых фонов.
    pub fn ink_candidates(&self) -> [Color; 2] {
        [Color::rgb(0x00, 0x00, 0x00), Color::rgb(0xff, 0xff, 0xff)]
    }

    /// Проверка/починка цвета текста на заливке карточки: окрашенные ноды
    /// обязаны читаться (WCAG AA ≥ 4.5:1), неокрашенные — тема как была.
    /// См. `contrast::ensure_contrast` — оттенок сохраняется по возможности.
    pub fn readable_on_card(&self, text: Color, fill: [f32; 4], colored: bool) -> Color {
        if !colored {
            return text;
        }
        let [ink_a, ink_b] = self.ink_candidates();
        crate::contrast::ensure_contrast(text, fill, 4.5, ink_a, ink_b)
    }

    /// Палитра по enum темы (canvas-core).
    pub fn from_theme(theme: canvas_core::Theme) -> Self {
        match theme {
            canvas_core::Theme::Dark => Self::dark(),
            canvas_core::Theme::Light => Self::light(),
        }
    }

    /// FR-047: эффективная палитра настроек — пресет по id
    /// (`theme_preset` из config.toml) или классическая тема. Неизвестный
    /// id (переименование пресета, ручная правка конфига) деградирует
    /// мягко — классика (ноль паник, выбор чинится из модалки).
    pub fn from_settings(theme: canvas_core::Theme, preset_id: &str) -> Self {
        crate::theme_presets::preset_theme(preset_id).unwrap_or_else(|| Self::from_theme(theme))
    }

    /// Цвет тела текста как [f32; 4] (образцы-толщины в меню связи).
    pub fn body_fill(&self) -> [f32; 4] {
        [
            self.body.r() as f32 / 255.0,
            self.body.g() as f32 / 255.0,
            self.body.b() as f32 / 255.0,
            self.body.a() as f32 / 255.0,
        ]
    }

    /// Clear-color фона канваса (linear space для wgpu).
    pub fn clear_color(&self) -> wgpu::Color {
        let channel = |i: usize| srgb_to_linear(self.background[i] as f64 / 255.0);
        wgpu::Color {
            r: channel(0),
            g: channel(1),
            b: channel(2),
            a: 1.0,
        }
    }
}

/// FR-055 (этап U4 PRD-0009, F-8): маппинг слотов темы → палитра-срез кита.
/// Направление зависимости — только render → ui (обратного нет, G7); кит не
/// знает конкретных цветов, тема отдаёт СЛОТЫ (значения — прежние константы
/// поверхностей: ноль визуального скачка, I-1 FR-046). Нормализация u8→f32 —
/// представление, не арифметика цвета; недостающие в v2 слоты читаются из
/// примитивов `canvas_core::tokens` (DIALOG_* — слоты диалогов T21).
impl From<&ThemeColors> for canvas_ui::kit::KitPalette {
    fn from(t: &ThemeColors) -> Self {
        let c4 = |c: Color| {
            [
                c.r() as f32 / 255.0,
                c.g() as f32 / 255.0,
                c.b() as f32 / 255.0,
                c.a() as f32 / 255.0,
            ]
        };
        let rgb3 = |rgb: [u8; 3]| {
            [
                rgb[0] as f32 / 255.0,
                rgb[1] as f32 / 255.0,
                rgb[2] as f32 / 255.0,
                1.0,
            ]
        };
        canvas_ui::kit::KitPalette {
            panel_fill: t.menu_fill,
            panel_border: t.palette_border,
            control_fill: t.palette_chip_fill,
            control_border: canvas_core::tokens::DIALOG_BUTTON_BORDER,
            control_primary: canvas_core::tokens::DIALOG_BUTTON_PRIMARY,
            control_danger: c4(t.error),
            hover_fill: t.control_hover_fill,
            primary_hover_fill: t.control_primary_hover_fill,
            selected_fill: t.control_selected_fill,
            text: c4(t.body),
            text_title: c4(t.title),
            text_muted: rgb3(canvas_core::tokens::DIALOG_TEXT_MUTED),
            disabled_text: c4(t.control_disabled_text),
            accent: t.accent,
        }
    }
}

/// Удобство: палитра кита из владеемой темы (панель витрины, DebugOverlay).
impl ThemeColors {
    pub fn kit_palette(&self) -> canvas_ui::kit::KitPalette {
        canvas_ui::kit::KitPalette::from(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тёмная палитра — прежние константы рендера (регрессия не меняет вид).
    #[test]
    fn dark_palette_matches_legacy_constants() {
        let dark = ThemeColors::dark();
        assert_eq!(dark.background, [0x1e, 0x1e, 0x22]);
        assert_eq!(dark.card_fill, [0.149, 0.149, 0.173, 1.0]);
        assert_eq!(dark.menu_fill, [0.11, 0.11, 0.13, 0.97]);
    }

    /// Светлая палитра отличается от тёмной по всем ключевым цветам.
    #[test]
    fn light_palette_differs_from_dark() {
        let dark = ThemeColors::dark();
        let light = ThemeColors::light();
        assert_ne!(dark.background, light.background);
        assert_ne!(dark.card_fill, light.card_fill);
        assert_ne!(dark.title, light.title);
        assert_ne!(dark.body, light.body);
        // Рамки групп в обеих темах полупрозрачны и различимы
        for theme in [dark, light] {
            assert!(
                theme.group_fill[3] > 0.0 && theme.group_fill[3] < 1.0,
                "заливка группы полупрозрачна: {:?}",
                theme.group_fill
            );
            assert!(
                theme.group_border[3] > 0.0 && theme.group_border[3] < 1.0,
                "рамка группы полупрозрачна: {:?}",
                theme.group_border
            );
        }
        assert_ne!(dark.group_border, light.group_border);
        // Светлый фон ярче тёмного по каждому каналу
        for i in 0..3 {
            assert!(light.background[i] > dark.background[i]);
        }
    }

    /// from_theme: enum ↔ палитра.
    #[test]
    fn from_theme_maps_both() {
        assert_eq!(
            ThemeColors::from_theme(canvas_core::Theme::Dark),
            ThemeColors::dark()
        );
        assert_eq!(
            ThemeColors::from_theme(canvas_core::Theme::Light),
            ThemeColors::light()
        );
    }

    /// Clear-color: sRGB-байты → linear (0x1e ≈ 0.01174 в linear).
    #[test]
    fn clear_color_is_linear() {
        let dark = ThemeColors::dark();
        let clear = dark.clear_color();
        assert!((clear.r - srgb_to_linear(0x1e as f64 / 255.0)).abs() < 1e-9);
        assert!((clear.b - srgb_to_linear(0x22 as f64 / 255.0)).abs() < 1e-9);
        assert_eq!(clear.a, 1.0);
        // Светлый фон в linear ярче тёмного
        let light = ThemeColors::light().clear_color();
        assert!(light.r > clear.r);
    }

    /// is_dark корректно различает темы (основа выбора палитры пресетов).
    #[test]
    fn is_dark_matches_background() {
        assert!(ThemeColors::dark().is_dark());
        assert!(!ThemeColors::light().is_dark());
    }

    /// FR-038: маджента направляющих читается на фоне ОБОИХ тем —
    /// ≥ 3:1 (WCAG 1.4.11, нетекстовая графика; урок CR-007).
    #[test]
    fn guide_colors_meet_contrast_on_both_themes() {
        use crate::contrast::contrast_ratio;
        for theme in [ThemeColors::dark(), ThemeColors::light()] {
            let bg = [
                theme.background[0] as f32 / 255.0,
                theme.background[1] as f32 / 255.0,
                theme.background[2] as f32 / 255.0,
            ];
            let align = contrast_ratio(
                [
                    theme.guide_align[0],
                    theme.guide_align[1],
                    theme.guide_align[2],
                ],
                bg,
            );
            assert!(align >= 3.0, "guide_align {align:.2} < 3:1 на фоне {bg:?}");
            let grid = contrast_ratio(
                [
                    theme.guide_grid[0],
                    theme.guide_grid[1],
                    theme.guide_grid[2],
                ],
                bg,
            );
            assert!(grid >= 3.0, "guide_grid {grid:.2} < 3:1 на фоне {bg:?}");
        }
    }

    /// FR-046 (PRD-0006 F-5/I-6): слоты v2 из примитивов — значения совпадают
    /// в обеих темах (прежние единые константы; расхождение — решение
    /// владельца v2), и соответствуют tokens-зеркалу байт-в-байт.
    #[test]
    fn v2_slots_match_tokens_in_both_themes() {
        use canvas_core::tokens;
        for theme in [ThemeColors::dark(), ThemeColors::light()] {
            assert_eq!(theme.accent, tokens::ACCENT);
            assert_eq!(theme.selection_fill[0..3], tokens::ACCENT[0..3]);
            assert_eq!(theme.selection_fill[3], tokens::ALPHA_35);
            assert_eq!(theme.highlight, tokens::HIGHLIGHT_FILL);
            assert_eq!(theme.whatif_fill, tokens::WHATIF_FILL);
            assert_eq!(
                [
                    theme.whatif_badge.r(),
                    theme.whatif_badge.g(),
                    theme.whatif_badge.b()
                ],
                tokens::WHATIF_BADGE
            );
            assert_eq!(
                [theme.error.r(), theme.error.g(), theme.error.b()],
                tokens::ERROR
            );
            assert_eq!([theme.hud.r(), theme.hud.g(), theme.hud.b()], tokens::HUD);
        }
        // FR-053 (U3 F-9): слоты состояний контролов — паритет по темам.
        assert_eq!(
            ThemeColors::dark().control_hover_fill,
            tokens::CONTROL_HOVER_FILL_DARK
        );
        assert_eq!(
            ThemeColors::light().control_hover_fill,
            tokens::CONTROL_HOVER_FILL_LIGHT
        );
        assert_eq!(
            ThemeColors::dark().control_selected_fill,
            tokens::CONTROL_SELECTED_FILL_DARK
        );
        assert_eq!(
            ThemeColors::light().control_selected_fill,
            tokens::CONTROL_SELECTED_FILL_LIGHT
        );
        for theme in [ThemeColors::dark(), ThemeColors::light()] {
            assert_eq!(
                theme.control_primary_hover_fill,
                tokens::CONTROL_PRIMARY_HOVER_FILL
            );
            assert_eq!(
                [
                    theme.control_disabled_text.r(),
                    theme.control_disabled_text.g(),
                    theme.control_disabled_text.b()
                ],
                tokens::CONTROL_DISABLED_TEXT
            );
        }
    }

    /// FR-046 (PRD-0006 F-5, протокол исключений): контраст новых слотов.
    /// Прохожие пороги фиксируются; существующие пары, НЕ проходящие WCAG,
    /// ЗАДОКУМЕНТИРОВАНЫ как известные исключения (значения не менялись —
    /// инвариант I-1) с нижней границей-регрессией; подстройка значений —
    /// отдельное решение владельца.
    #[test]
    fn v2_slots_contrast_documented() {
        use crate::contrast::{contrast_ratio, contrast_text_vs_fill};
        let dark = ThemeColors::dark();
        let light = ThemeColors::light();
        let bg = |t: &ThemeColors| {
            [
                t.background[0] as f32 / 255.0,
                t.background[1] as f32 / 255.0,
                t.background[2] as f32 / 255.0,
                1.0,
            ]
        };

        // error (текст результата) к фону канваса: ≥ 3:1 на обеих темах
        for theme in [&dark, &light] {
            let r = contrast_text_vs_fill(theme.error, bg(theme));
            assert!(
                r >= 3.0,
                "error {r:.2} < 3:1 к фону (dark={})",
                theme.is_dark()
            );
        }
        // error к заливке карточки: ИЗВЕСТНОЕ ИСКЛЮЧЕНИЕ к AA 4.5 (значения
        // не менялись: 4.32 тёмная / 3.38 светлая — строка результата живёт
        // на карточке, читаемость дотягивает автоконтраст on_card);
        // регрессионная граница ниже фактических значений.
        for theme in [&dark, &light] {
            let r = contrast_text_vs_fill(theme.error, theme.card_fill);
            assert!(
                r >= 3.2,
                "error-vs-card {r:.2} — ушла ниже задокументированной границы"
            );
        }

        // accent к фону: тёмная ≥ 3:1; светлая — ИЗВЕСТНОЕ ИСКЛЮЧЕНИЕ
        // (2.52; выделение — 1.5px рамка + заливка-гало, не самостоятельная
        // графика; v2 может затемнить акцент светлой темы по решению владельца)
        let acc = |t: &ThemeColors| {
            contrast_ratio(
                [t.accent[0], t.accent[1], t.accent[2]],
                [bg(t)[0], bg(t)[1], bg(t)[2]],
            )
        };
        assert!(acc(&dark) >= 3.0, "accent тёмная {:.2} < 3:1", acc(&dark));
        assert!(
            acc(&light) >= 2.4,
            "accent светлая {:.2} — ниже документированной границы",
            acc(&light)
        );

        // hud к фону: тёмная ≥ 3:1; светлая — ИЗВЕСТНОЕ ИСКЛЮЧЕНИЕ (2.51;
        // HUD отрисовывается с тенью, значение прежнее)
        let hud = |t: &ThemeColors| contrast_text_vs_fill(t.hud, bg(t));
        assert!(hud(&dark) >= 3.0, "hud тёмная {:.2} < 3:1", hud(&dark));
        assert!(
            hud(&light) >= 2.4,
            "hud светлая {:.2} — ниже документированной границы",
            hud(&light)
        );

        // FR-053 (U3 F-9): control_disabled_text к подложкам disabled-кнопок
        // what-if бара: тёмная — заливка WHATIF_CHIP_DIM; светлая — menu_fill
        // (значение #8a909c прежнее, обе пары проходят 3:1 — новые пороги).
        for theme in [&dark, &light] {
            let sub = if theme.is_dark() {
                canvas_core::tokens::WHATIF_CHIP_DIM
            } else {
                theme.menu_fill
            };
            let r = contrast_text_vs_fill(theme.control_disabled_text, sub);
            assert!(
                r >= 3.0,
                "control_disabled_text {r:.2} < 3:1 (dark={})",
                theme.is_dark()
            );
        }
        // control_hover_fill не текстовый (подложка hover) — контраст
        // фиксируется как различимость от базовой заливки строки: hover
        // обязан отличаться от menu_fill (суммарная дельта каналов) на обеих
        // темах. Фактические значения: тёмная 0.245, светлая 0.05 (формула
        // hover_fill насыщает до 1.0); регрессионная граница ниже фактических.
        for theme in [&dark, &light] {
            let delta = (theme.control_hover_fill[0] - theme.menu_fill[0]).abs()
                + (theme.control_hover_fill[1] - theme.menu_fill[1]).abs()
                + (theme.control_hover_fill[2] - theme.menu_fill[2]).abs();
            assert!(
                delta >= 0.045,
                "hover неотличим от фона строки: {delta} (dark={})",
                theme.is_dark()
            );
        }
    }

    /// Гарантия доступности (WCAG AA): readable_on_card возвращает ≥ 4.5:1
    /// на ЛЮБОЙ заливке — серая шкала, дефолтные заливки, все пресеты обеих
    /// тем и сетка цветовых шумов (средне-серые фоны спасает смешение к
    /// экстремуму внутри ensure_contrast, не пара «чернил» как таковая).
    #[test]
    fn readable_on_card_guarantees_aa_contrast_everywhere() {
        use crate::contrast::contrast_text_vs_fill;
        let dark = ThemeColors::dark();
        let light = ThemeColors::light();
        let mut fills: Vec<[f32; 4]> = Vec::new();
        // Серая шкала 0..255 — худшие случаи для любого авто-выбора
        for step in (0..=255).step_by(4) {
            let c = step as f32 / 255.0;
            fills.push([c, c, c, 1.0]);
        }
        // Цветовой шум: покрываем светлые/тёмные/средние тона всех оттенков
        for r in [0.1f32, 0.35, 0.6, 0.9] {
            for g in [0.1f32, 0.4, 0.65, 0.95] {
                for b in [0.1f32, 0.45, 0.7, 0.95] {
                    fills.push([r, g, b, 1.0]);
                }
            }
        }
        fills.push(dark.card_fill);
        fills.push(light.card_fill);
        for (_, fill) in crate::cards::PRESET_COLORS_DARK {
            fills.push(fill);
        }
        for (_, fill) in crate::cards::PRESET_COLORS_LIGHT {
            fills.push(fill);
        }
        // Проверяем ремонт всех «типовых» цветов текста темы
        for theme in [dark, light] {
            let samples = [theme.title, theme.body, theme.icon, theme.link, theme.quote];
            for fill in &fills {
                for &sample in &samples {
                    let fixed = theme.readable_on_card(sample, *fill, true);
                    let ratio = contrast_text_vs_fill(fixed, *fill);
                    assert!(
                        ratio >= 4.5,
                        "контраст {ratio:.2} < 4.5: text={sample:?}, fill={fill:?}, dark={:?}",
                        theme.is_dark()
                    );
                }
            }
        }
    }

    /// Пресеты читаются цветом заголовка своей темы: тёмная — AA (≥ 4.5,
    /// приглушённые тона остаются прежними — регрессия вида), светлая —
    /// AAA (≥ 7: пастели спроектированы под тёмный текст).
    #[test]
    fn presets_meet_contrast_with_theme_title() {
        use crate::contrast::contrast_text_vs_fill;
        for (_, fill) in crate::cards::PRESET_COLORS_DARK {
            let ratio = contrast_text_vs_fill(ThemeColors::dark().title, fill);
            assert!(
                ratio >= 4.5,
                "тёмная тема: {ratio:.2} < 4.5 у заливки {fill:?}"
            );
        }
        for (_, fill) in crate::cards::PRESET_COLORS_LIGHT {
            let ratio = contrast_text_vs_fill(ThemeColors::light().title, fill);
            assert!(
                ratio >= 7.0,
                "светлая тема: {ratio:.2} < 7 у заливки {fill:?}"
            );
        }
    }

    /// readable_on_card: неокрашенная нода — цвет темы без изменений;
    /// окрашенная — результат читается (≥ 4.5) на её заливке.
    #[test]
    fn readable_on_card_remaps_only_colored() {
        use crate::contrast::contrast_text_vs_fill;
        let dark = ThemeColors::dark();
        let light = ThemeColors::light();
        let title = dark.title;
        // Неокрашенная: тема не трогается даже на светлом card_fill... —
        // точнее: флаг colored=false отключает ремап целиком
        assert_eq!(dark.readable_on_card(title, light.card_fill, false), title);
        // Окрашенная светлая заливка в тёмной теме → текст сменён и читается
        let pastel = crate::cards::PRESET_COLORS_LIGHT[2].1; // yellow
        let fixed = dark.readable_on_card(title, pastel, true);
        assert_ne!(fixed, title);
        assert!(contrast_text_vs_fill(fixed, pastel) >= 4.5);
    }
}
