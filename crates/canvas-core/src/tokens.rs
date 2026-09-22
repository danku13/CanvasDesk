//! Design-токены — слой примитивов дизайн-системы (PRD-0006 F-1, FR-046).
//!
//! Единая точка правды для значений, разделяемых несколькими потребителями
//! (`ThemeColors`, компонентные палитры, метрики карточек/текста, анимации).
//! Источник данных — JSON-файлы `design/tokens/*.json` (структура в духе
//! W3C Design Tokens); этот модуль — платформенно-нейтральное зеркало без
//! GPU/ОС-зависимостей (правило архитектуры №1, wasm-гейт ADR-0011).
//!
//! Инварианты FR-046:
//! - I-1: значения = текущим константам репозитория (ноль визуального
//!   скачка); каждое значение документировано в `$desc` JSON координатами
//!   источника;
//! - I-5: тест паритета JSON↔Rust (внизу файла) ловит расхождение.
//!
//! Семантические слоты (что каким цветом красится) — не здесь, а в
//! `canvas-render::theme::ThemeColors` и компонентных палитрах; здесь —
//! только примитивы.

// ---------------------------------------------------------------------------
// Цвета (design/tokens/colors.json)
// ---------------------------------------------------------------------------

/// Акцент — единый источник акцентного семейства (G4): выделение, фокус,
/// драфт-связь, хром виджетов, drop-ghost, select-рамка, группы.
/// Источник: `SELECTION_BORDER` cards.rs:24.
pub const ACCENT: [f32; 4] = [0.396, 0.612, 0.969, 1.0];

/// Альфа-ступени акцентных вариантов (набор фиксируется в colors.json
/// `opacity_steps`; значение подставляется в канал A при сборке варианта).
pub const ALPHA_08: f32 = 0.08;
pub const ALPHA_10: f32 = 0.10;
pub const ALPHA_22: f32 = 0.22;
pub const ALPHA_30: f32 = 0.30;
pub const ALPHA_35: f32 = 0.35;
pub const ALPHA_40: f32 = 0.40;
pub const ALPHA_50: f32 = 0.50;
pub const ALPHA_60: f32 = 0.60;
pub const ALPHA_70: f32 = 0.70;
pub const ALPHA_90: f32 = 0.90;

/// Нейтральная связь. Источник: `EDGE_COLOR` cards.rs:643.
pub const EDGE_DEFAULT: [f32; 4] = [0.52, 0.58, 0.66, 1.0];
/// Value-ребро (поток значений, FR-014). Источник: `FLOW_EDGE_COLOR` cards.rs:646.
pub const EDGE_FLOW: [f32; 4] = [0.13, 0.66, 0.55, 1.0];
/// Резиновая линия (drag новой связи) — акцент α0.70. Источник: `DRAFT_COLOR` cards.rs:649.
pub const EDGE_DRAFT: [f32; 4] = [0.396, 0.612, 0.969, 0.70];

/// Рамка битой ссылки. Источник: `BROKEN_BORDER` cards.rs:26.
pub const BROKEN_BORDER: [f32; 4] = [0.45, 0.45, 0.45, 1.0];
/// Фон-подсветка `==текст==`. Источник: `HIGHLIGHT_FILL` renderer.rs:43.
pub const HIGHLIGHT_FILL: [f32; 4] = [0.85, 0.75, 0.30, 0.30];
/// Фон what-if строки (FR-017). Источник: `WHATIF_FILL` renderer.rs:46.
pub const WHATIF_FILL: [f32; 4] = [0.30, 0.55, 0.95, 0.22];
/// Заливка чипа what-if (FR-017). Источник: app.rs:2507.
pub const WHATIF_CHIP: [f32; 4] = [0.30, 0.55, 0.95, 1.0];
/// Приглушённый чип what-if. Источник: app.rs:2562.
pub const WHATIF_CHIP_DIM: [f32; 4] = [0.14, 0.16, 0.20, 1.0];
/// Дельта-бейдж what-if (#dfa63e, FR-017). Источник: `WHATIF_BADGE_COLOR` renderer.rs:49.
pub const WHATIF_BADGE: [u8; 3] = [223, 166, 62];
/// Красный строки результата с ошибкой (#e55c5c, FR-013). Источник: `RESULT_ERROR_COLOR` text.rs:108.
pub const ERROR: [u8; 3] = [229, 92, 92];
/// HUD F3 (#659cf8). Источник: `HUD_COLOR` text.rs:140.
pub const HUD: [u8; 3] = [101, 156, 248];
/// Рамка пульса результата (альфа анимируется). Источник: app.rs:10863.
pub const PULSE_RESULT: [f32; 4] = [1.0, 0.85, 0.35, 1.0];
/// PRD-0007 (F-4): лист-константа в explain-дереве — цвет полосы карточки
/// и веток к листьям (прототип v4 `--leaf: #9fd6ff`, тёмная тема). Светлая
/// тема — затемнённый слот [`crate::tokens::EXPLAIN_LEAF_LIGHT`] (контраст
/// ≥ 3:1 на обеих темах, AC-3.4/CR-007).
pub const EXPLAIN_LEAF: [f32; 4] = [0.624, 0.839, 1.0, 1.0];
/// Светлая тема: #1c6ea8 — тот же синий тон, контраст к #f5f5f7 ≈ 4.6:1.
pub const EXPLAIN_LEAF_LIGHT: [f32; 4] = [0.11, 0.43, 0.66, 1.0];

/// Панель диалога (T21). Источник: app.rs:10726.
pub const DIALOG_FILL: [f32; 4] = [0.09, 0.11, 0.15, 0.97];
/// Рамка диалога. Источник: app.rs:10727.
pub const DIALOG_BORDER: [f32; 4] = [0.23, 0.51, 0.96, 1.0];
/// Кнопка primary. Источник: app.rs:10737.
pub const DIALOG_BUTTON_PRIMARY: [f32; 4] = [0.16, 0.32, 0.60, 1.0];
/// Кнопка secondary. Источник: app.rs:10738.
pub const DIALOG_BUTTON_SECONDARY: [f32; 4] = [0.20, 0.23, 0.29, 1.0];
/// Рамка кнопок диалога (и чипов). Источник: app.rs:10739.
pub const DIALOG_BUTTON_BORDER: [f32; 4] = [0.35, 0.40, 0.50, 1.0];
/// Текст заголовка/кнопок (#e8ecf4). Источник: app.rs:10740-10770.
pub const DIALOG_TEXT: [u8; 3] = [232, 236, 244];
/// Приглушённый текст тела (#b6bece). Источник: app.rs:10773.
pub const DIALOG_TEXT_MUTED: [u8; 3] = [182, 190, 206];

/// Текст тоста (#f0e6c2). Источник: app.rs:10797.
pub const TOAST_TEXT: [u8; 3] = [240, 230, 194];

/// Wheel-меню (FR-022): заливка затемнением. Источник: `FILL_DIM` app.rs:4952.
pub const WHEEL_DIM: [f32; 4] = [0.0, 0.0, 0.0, 0.35];
/// Wheel: сектор категории. Источник: `FILL_CATEGORY` app.rs:4953.
pub const WHEEL_CATEGORY: [f32; 4] = [0.17, 0.18, 0.22, 0.92];
/// Wheel: сектор шаблона. Источник: `FILL_TEMPLATE` app.rs:4954.
pub const WHEEL_TEMPLATE: [f32; 4] = [0.20, 0.22, 0.27, 0.92];
/// Wheel: hover сектора. Источник: `FILL_HOVER` app.rs:4955.
pub const WHEEL_HOVER: [f32; 4] = [0.18, 0.29, 0.48, 0.95];
/// Wheel: активный хаб. Источник: `FILL_HUB_ACTIVE` app.rs:4956.
pub const WHEEL_HUB_ACTIVE: [f32; 4] = [0.18, 0.29, 0.48, 0.95];
/// Wheel: рамка сектора. Источник: `BORDER` app.rs:4957.
pub const WHEEL_BORDER: [f32; 4] = [0.22, 0.24, 0.30, 0.90];

/// Severity-таблица FR-016 (warning/danger/critical), тёмная тема.
/// Источник: `SEVERITY_DARK` cards.rs:42-50 (CR-007: контраст ≥ 3:1).
pub const SEVERITY_DARK: [[f32; 4]; 3] = [
    [0.961, 0.651, 0.137, 1.0],
    [0.898, 0.282, 0.302, 1.0],
    [1.0, 0.271, 0.188, 1.0],
];
/// Severity-таблица FR-016, светлая тема. Источник: `SEVERITY_LIGHT` cards.rs:51-56.
pub const SEVERITY_LIGHT: [[f32; 4]; 3] = [
    [0.702, 0.42, 0.0, 1.0],
    [0.761, 0.106, 0.106, 1.0],
    [0.478, 0.0, 0.063, 1.0],
];
/// Тексты бейджей severity (warning/danger/critical), тёмная тема.
/// Источник: `severity_text` cards.rs:75-102.
pub const SEVERITY_TEXT_DARK: [[u8; 3]; 3] = [[245, 166, 35], [242, 107, 115], [255, 102, 85]];
/// Тексты бейджей severity, светлая тема. Источник: `severity_text` cards.rs:75-102.
pub const SEVERITY_TEXT_LIGHT: [[u8; 3]; 3] = [[138, 90, 0], [160, 21, 21], [122, 0, 16]];
/// Текст бейджа для severity=None. Источник: `severity_text` cards.rs:84.
pub const SEVERITY_TEXT_NONE: [u8; 3] = [154, 154, 162];

/// Тень текста HUD (#101012). Источник: text.rs:1884.
pub const HUD_SHADOW: [u8; 3] = [16, 16, 18];

/// Минимапа — нода-файл (RGBA). Источник: `NODE_COLOR_FILE` minimap.rs:34.
pub const MINIMAP_NODE_FILE: [u8; 4] = [96, 148, 228, 255];
/// Минимапа — текстовая нода. Источник: `NODE_COLOR_TEXT` minimap.rs:37.
pub const MINIMAP_NODE_TEXT: [u8; 4] = [228, 196, 96, 255];
/// Минимапа — группа. Источник: `NODE_COLOR_GROUP` minimap.rs:39.
pub const MINIMAP_NODE_GROUP: [u8; 4] = [150, 150, 158, 255];
/// Минимапа — битая ссылка. Источник: `NODE_COLOR_BROKEN` minimap.rs:41.
pub const MINIMAP_NODE_BROKEN: [u8; 4] = [214, 92, 92, 255];
/// Минимапа — рамка viewport. Источник: `VIEWPORT_COLOR` minimap.rs:43.
pub const MINIMAP_VIEWPORT: [u8; 4] = [255, 255, 255, 255];
/// Минимапа — линии рёбер. Источник: `EDGE_COLOR` minimap.rs:45.
pub const MINIMAP_EDGE: [u8; 4] = [170, 176, 188, 255];
/// Минимапа — фон (полупрозрачный). Источник: `BG_COLOR` minimap.rs:47.
pub const MINIMAP_BG: [u8; 4] = [30, 32, 38, 184];

// ---------------------------------------------------------------------------
// Размеры (design/tokens/dimensions.json)
// ---------------------------------------------------------------------------

/// Радиус скругления карточки, world px. Источник: `CORNER_RADIUS` cards.rs:21.
pub const CARD_CORNER_RADIUS: f32 = 8.0;
/// Высота шапки карточки, world px (FR-023). Источник: `HEADER_HEIGHT` cards.rs:19.
pub const CARD_HEADER_HEIGHT: f32 = 34.0;

// ---------------------------------------------------------------------------
// Spacing/radius-scale UI-оверлеев (design/tokens/dimensions.json — FR-053
// U3 PRD-0009 F-9): значения = текущим константам пилотов (галерея схем,
// what-if бар — I-1 ноль скачка); потребители берут из scale.
// ---------------------------------------------------------------------------

/// Базовый зазор чипов/строк. Источник: `BAR_GAP` whatif_ui.rs, зазор чипов
/// галереи, `LIST_MARGIN` whatif_ui.rs.
pub const SPACING_S: f32 = 6.0;
/// Поля таблицы сравнения. Источник: `TABLE_MARGIN` whatif_ui.rs.
pub const SPACING_SM: f32 = 8.0;
/// Поля бара и empty-кнопок. Источник: `BAR_PADDING` whatif_ui.rs, поля
/// кнопок empty-state scheme_gallery_ui.rs.
pub const SPACING_MD: f32 = 10.0;
/// Маржа бара, поля чипов/кнопок/панели. Источник: `BAR_MARGIN`/`CHIP_PAD_X`/
/// `BTN_PAD_X` whatif_ui.rs, `PANEL_PAD` scheme_gallery_ui.rs.
pub const SPACING_LG: f32 = 12.0;
/// Поля панели/карточки к вьюпорту. Источник: маржа панели галереи.
pub const SPACING_XL: f32 = 24.0;

/// Скругление чипа/кнопки what-if и empty/фильтра галереи (первый параметр
/// CardInstance.params). Источник: квад чипа app.rs.
pub const RADIUS_CHIP: f32 = 6.0;
/// Скругление панели галереи. Источник: квад панели app.rs.
pub const RADIUS_PANEL: f32 = 10.0;
/// Скругление чипов-пилюль категорий галереи. Источник: квад чипа app.rs.
pub const RADIUS_PILL: f32 = 12.0;

// ---------------------------------------------------------------------------
// Слоты состояний контролов (design/tokens/colors.json группа `control` —
// FR-053 U3 PRD-0009 F-9): hover/selected/disabled пилотов. Значения =
// прежним вычислениям hover_fill (app.rs:1042-1049: c·1.3+0.04/0.06) и
// локальной dim (I-1 ноль скачка); дифференциация selected — v2.
// ---------------------------------------------------------------------------

/// Hover строки списка/вторичной кнопки, тёмная тема. Источник:
/// hover_fill(menu_fill dark [0.11,0.11,0.13,0.97]).
pub const CONTROL_HOVER_FILL_DARK: [f32; 4] = [0.183, 0.183, 0.229, 0.97];
/// Hover строки списка/вторичной кнопки, светлая тема. Источник:
/// hover_fill(menu_fill light [0.98,0.98,0.99,0.97]) — формула насыщает до 1.0.
pub const CONTROL_HOVER_FILL_LIGHT: [f32; 4] = [1.0, 1.0, 1.0, 0.97];
/// Hover primary-кнопки (обе темы: primary не дифференцирован). Источник:
/// hover_fill(DIALOG_BUTTON_PRIMARY).
pub const CONTROL_PRIMARY_HOVER_FILL: [f32; 4] = [0.248, 0.456, 0.84, 1.0];
/// Выбранная строка списка, тёмная тема (= hover: сегодня selected и hover
/// неразличимы — ноль скачка; семантика разделена слотами).
pub const CONTROL_SELECTED_FILL_DARK: [f32; 4] = CONTROL_HOVER_FILL_DARK;
/// Выбранная строка списка, светлая тема (= hover light).
pub const CONTROL_SELECTED_FILL_LIGHT: [f32; 4] = CONTROL_HOVER_FILL_LIGHT;
/// Текст disabled-кнопок (обе темы). Источник: бывшая локальная `dim`
/// app.rs:2808 (#8a909c).
pub const CONTROL_DISABLED_TEXT: [u8; 3] = [138, 144, 156];

/// Диаметр бусины связи. Источник: `EDGE_DOT` cards.rs:627.
pub const EDGE_DOT: f32 = 2.5;
/// Длина уса стрелки. Источник: `ARROW_LEN` cards.rs:651.
pub const EDGE_ARROW_LEN: f32 = 10.0;

/// Кегль заголовка карточки, world px (FR-023). Источник: `TITLE_FONT_SIZE` text.rs:66.
pub const TYPE_TITLE: f32 = 16.0;
/// Высота строки заголовка. Источник: `TITLE_LINE_HEIGHT` text.rs:68.
pub const TYPE_TITLE_LINE: f32 = 22.0;
/// Кегль тела заметки. Источник: `BODY_FONT_SIZE` text.rs:88.
pub const TYPE_BODY: f32 = 14.0;
/// Высота строки тела. Источник: `BODY_LINE_HEIGHT` text.rs:90.
pub const TYPE_BODY_LINE: f32 = 20.0;
/// Кегль лейбла связи. Источник: `EDGE_LABEL_FONT_SIZE` text.rs:97.
pub const TYPE_EDGE_LABEL: f32 = 12.0;
/// Высота строки лейбла связи. Источник: `EDGE_LABEL_LINE_HEIGHT` text.rs:99.
pub const TYPE_EDGE_LABEL_LINE: f32 = 16.0;
/// Кегль строки результата (FR-013). Источник: `RESULT_FONT_SIZE` text.rs:102.
pub const TYPE_RESULT: f32 = 12.0;
/// Высота строки результата. Источник: `RESULT_LINE_HEIGHT` text.rs:105.
pub const TYPE_RESULT_LINE: f32 = 16.0;
/// Кегль HUD, физические px. Источник: `HUD_FONT_SIZE` text.rs:134.
pub const TYPE_HUD: f32 = 14.0;
/// Высота строки HUD. Источник: `HUD_LINE_HEIGHT` text.rs:136.
pub const TYPE_HUD_LINE: f32 = 18.0;
/// Кегль бейджа «=», физические px. Источник: `BADGE_FONT_SIZE` text.rs:129.
pub const TYPE_BADGE: f32 = 10.0;
/// Высота строки бейджа. Источник: `BADGE_LINE_HEIGHT` text.rs:131.
pub const TYPE_BADGE_LINE: f32 = 12.0;

// ---------------------------------------------------------------------------
// Движение (design/tokens/motion.json)
// ---------------------------------------------------------------------------

/// Затухание не-фокусных элементов (T23). Источник: animate.rs.
pub const FOCUS_FADE_MS: u64 = 150;
/// Перелёт камеры, ease-out. Источник: animate.rs:9,61-79.
pub const CAMERA_FLIGHT_MS: u64 = 300;
/// Пульс результата. Источник: app.rs:10853-10867.
pub const RESULT_PULSE_MS: u64 = 1200;
/// Дыхание фокусной связи. Источник: cards.rs:925-931.
pub const FOCUS_BREATH_MS: u64 = 1600;

// ---------------------------------------------------------------------------
// Тест паритета JSON↔Rust (инвариант I-5): расхождение = красный тест.
// JSON читается только в тестах — рантайм ядра константы не парсит
// (wasm-гейт не страдает), serde_json — уже зависимость canvas-core.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod parity_tests {
    use super::*;
    use serde_json::Value;

    const COLORS: &str = include_str!("../../../design/tokens/colors.json");
    const DIMENSIONS: &str = include_str!("../../../design/tokens/dimensions.json");
    const MOTION: &str = include_str!("../../../design/tokens/motion.json");

    fn f32_arr4(v: &Value, path: &str) -> [f32; 4] {
        let a = v.as_array().unwrap_or_else(|| panic!("{path}: не массив"));
        assert_eq!(a.len(), 4, "{path}: длина != 4");
        [
            a[0].as_f64().unwrap() as f32,
            a[1].as_f64().unwrap() as f32,
            a[2].as_f64().unwrap() as f32,
            a[3].as_f64().unwrap() as f32,
        ]
    }

    fn u8_arr3(v: &Value, path: &str) -> [u8; 3] {
        let a = v.as_array().unwrap_or_else(|| panic!("{path}: не массив"));
        assert_eq!(a.len(), 3, "{path}: длина != 3");
        [
            a[0].as_u64().unwrap() as u8,
            a[1].as_u64().unwrap() as u8,
            a[2].as_u64().unwrap() as u8,
        ]
    }

    fn color<'a>(root: &'a Value, path: &str) -> &'a Value {
        path.split('.').fold(root, |acc, k| {
            acc.get(k).unwrap_or_else(|| panic!("{path}: нет ключа"))
        })
    }

    #[test]
    fn json_colors_match_rust_mirror() {
        let root: Value = serde_json::from_str(COLORS).expect("colors.json валиден");

        assert_eq!(f32_arr4(color(&root, "accent.$value"), "accent"), ACCENT);

        assert_eq!(
            f32_arr4(color(&root, "edge.default.$value"), "edge.default"),
            EDGE_DEFAULT
        );
        assert_eq!(
            f32_arr4(color(&root, "edge.flow.$value"), "edge.flow"),
            EDGE_FLOW
        );
        assert_eq!(
            f32_arr4(color(&root, "edge.draft.$value"), "edge.draft"),
            EDGE_DRAFT
        );

        assert_eq!(
            f32_arr4(color(&root, "state.broken.$value"), "state.broken"),
            BROKEN_BORDER
        );
        assert_eq!(
            f32_arr4(color(&root, "state.highlight.$value"), "state.highlight"),
            HIGHLIGHT_FILL
        );
        assert_eq!(
            f32_arr4(
                color(&root, "state.whatif_fill.$value"),
                "state.whatif_fill"
            ),
            WHATIF_FILL
        );
        assert_eq!(
            f32_arr4(
                color(&root, "state.whatif_chip.$value"),
                "state.whatif_chip"
            ),
            WHATIF_CHIP
        );
        assert_eq!(
            f32_arr4(
                color(&root, "state.whatif_chip_dim.$value"),
                "state.whatif_chip_dim"
            ),
            WHATIF_CHIP_DIM
        );
        assert_eq!(
            u8_arr3(
                color(&root, "state.whatif_badge.$value"),
                "state.whatif_badge"
            ),
            WHATIF_BADGE
        );
        assert_eq!(
            u8_arr3(color(&root, "state.error.$value"), "state.error"),
            ERROR
        );
        assert_eq!(u8_arr3(color(&root, "state.hud.$value"), "state.hud"), HUD);
        assert_eq!(
            f32_arr4(
                color(&root, "state.pulse_result.$value"),
                "state.pulse_result"
            ),
            PULSE_RESULT
        );

        assert_eq!(
            f32_arr4(color(&root, "dialog.fill.$value"), "dialog.fill"),
            DIALOG_FILL
        );
        assert_eq!(
            f32_arr4(color(&root, "dialog.border.$value"), "dialog.border"),
            DIALOG_BORDER
        );
        assert_eq!(
            f32_arr4(
                color(&root, "dialog.button_primary_fill.$value"),
                "dialog.btn.primary"
            ),
            DIALOG_BUTTON_PRIMARY
        );
        assert_eq!(
            f32_arr4(
                color(&root, "dialog.button_secondary_fill.$value"),
                "dialog.btn.secondary"
            ),
            DIALOG_BUTTON_SECONDARY
        );
        assert_eq!(
            f32_arr4(
                color(&root, "dialog.button_border.$value"),
                "dialog.btn.border"
            ),
            DIALOG_BUTTON_BORDER
        );
        assert_eq!(
            u8_arr3(color(&root, "dialog.text.$value"), "dialog.text"),
            DIALOG_TEXT
        );
        assert_eq!(
            u8_arr3(
                color(&root, "dialog.text_muted.$value"),
                "dialog.text_muted"
            ),
            DIALOG_TEXT_MUTED
        );

        assert_eq!(
            u8_arr3(color(&root, "toast.text.$value"), "toast.text"),
            TOAST_TEXT
        );

        assert_eq!(
            f32_arr4(color(&root, "wheel.dim.$value"), "wheel.dim"),
            WHEEL_DIM
        );
        assert_eq!(
            f32_arr4(color(&root, "wheel.category.$value"), "wheel.category"),
            WHEEL_CATEGORY
        );
        assert_eq!(
            f32_arr4(color(&root, "wheel.template.$value"), "wheel.template"),
            WHEEL_TEMPLATE
        );
        assert_eq!(
            f32_arr4(color(&root, "wheel.hover.$value"), "wheel.hover"),
            WHEEL_HOVER
        );
        assert_eq!(
            f32_arr4(color(&root, "wheel.hub_active.$value"), "wheel.hub_active"),
            WHEEL_HUB_ACTIVE
        );
        assert_eq!(
            f32_arr4(color(&root, "wheel.border.$value"), "wheel.border"),
            WHEEL_BORDER
        );

        for (i, _name) in ["0", "1", "2"].iter().enumerate() {
            let d = &color(&root, "severity.dark.$value").as_array().unwrap()[i];
            assert_eq!(f32_arr4(d, "severity.dark"), SEVERITY_DARK[i]);
            let l = &color(&root, "severity.light.$value").as_array().unwrap()[i];
            assert_eq!(f32_arr4(l, "severity.light"), SEVERITY_LIGHT[i]);
            let td = &color(&root, "severity.text_dark.$value")
                .as_array()
                .unwrap()[i];
            assert_eq!(u8_arr3(td, "severity.text_dark"), SEVERITY_TEXT_DARK[i]);
            let tl = &color(&root, "severity.text_light.$value")
                .as_array()
                .unwrap()[i];
            assert_eq!(u8_arr3(tl, "severity.text_light"), SEVERITY_TEXT_LIGHT[i]);
        }
        assert_eq!(
            u8_arr3(
                color(&root, "severity.text_none.$value"),
                "severity.text_none"
            ),
            SEVERITY_TEXT_NONE
        );
        assert_eq!(
            u8_arr3(color(&root, "state.hud_shadow.$value"), "state.hud_shadow"),
            HUD_SHADOW
        );

        // FR-053 (U3 F-9): группа control — слоты состояний контролов.
        assert_eq!(
            f32_arr4(
                color(&root, "control.hover_fill.dark.$value"),
                "control.hover_fill.dark"
            ),
            CONTROL_HOVER_FILL_DARK
        );
        assert_eq!(
            f32_arr4(
                color(&root, "control.hover_fill.light.$value"),
                "control.hover_fill.light"
            ),
            CONTROL_HOVER_FILL_LIGHT
        );
        assert_eq!(
            f32_arr4(
                color(&root, "control.primary_hover_fill.$value"),
                "control.primary_hover_fill"
            ),
            CONTROL_PRIMARY_HOVER_FILL
        );
        assert_eq!(
            f32_arr4(
                color(&root, "control.selected_fill.dark.$value"),
                "control.selected_fill.dark"
            ),
            CONTROL_SELECTED_FILL_DARK
        );
        assert_eq!(
            f32_arr4(
                color(&root, "control.selected_fill.light.$value"),
                "control.selected_fill.light"
            ),
            CONTROL_SELECTED_FILL_LIGHT
        );
        assert_eq!(
            u8_arr3(
                color(&root, "control.disabled_text.$value"),
                "control.disabled_text"
            ),
            CONTROL_DISABLED_TEXT
        );

        let map = color(&root, "minimap");
        let rgba = |key: &str| -> [u8; 4] {
            let a = map
                .get(key)
                .and_then(|v| v.get("$value"))
                .unwrap()
                .as_array()
                .unwrap();
            [
                a[0].as_u64().unwrap() as u8,
                a[1].as_u64().unwrap() as u8,
                a[2].as_u64().unwrap() as u8,
                a[3].as_u64().unwrap() as u8,
            ]
        };
        assert_eq!(rgba("node_file"), MINIMAP_NODE_FILE);
        assert_eq!(rgba("node_text"), MINIMAP_NODE_TEXT);
        assert_eq!(rgba("node_group"), MINIMAP_NODE_GROUP);
        assert_eq!(rgba("node_broken"), MINIMAP_NODE_BROKEN);
        assert_eq!(rgba("viewport"), MINIMAP_VIEWPORT);
        assert_eq!(rgba("edge"), MINIMAP_EDGE);
        assert_eq!(rgba("bg"), MINIMAP_BG);
    }

    #[test]
    fn json_opacity_steps_cover_alpha_constants() {
        let root: Value = serde_json::from_str(COLORS).expect("colors.json валиден");
        let steps: Vec<f32> = color(&root, "opacity_steps.$value")
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        for a in [
            ALPHA_08, ALPHA_10, ALPHA_22, ALPHA_30, ALPHA_35, ALPHA_40, ALPHA_50, ALPHA_60,
            ALPHA_70, ALPHA_90,
        ] {
            assert!(steps.contains(&a), "альфа {a} отсутствует в opacity_steps");
        }
    }

    #[test]
    fn json_dimensions_match_rust_mirror() {
        let root: Value = serde_json::from_str(DIMENSIONS).expect("dimensions.json валиден");
        let dim = |path: &str| -> f32 { color(&root, path).as_f64().unwrap() as f32 };
        assert_eq!(dim("card.corner_radius.$value"), CARD_CORNER_RADIUS);
        assert_eq!(dim("card.header_height.$value"), CARD_HEADER_HEIGHT);
        assert_eq!(dim("edge.dot.$value"), EDGE_DOT);
        assert_eq!(dim("edge.arrow_len.$value"), EDGE_ARROW_LEN);
        assert_eq!(dim("typography.title_size.$value"), TYPE_TITLE);
        assert_eq!(dim("typography.title_line.$value"), TYPE_TITLE_LINE);
        assert_eq!(dim("typography.body_size.$value"), TYPE_BODY);
        assert_eq!(dim("typography.body_line.$value"), TYPE_BODY_LINE);
        assert_eq!(dim("typography.edge_label_size.$value"), TYPE_EDGE_LABEL);
        assert_eq!(
            dim("typography.edge_label_line.$value"),
            TYPE_EDGE_LABEL_LINE
        );
        assert_eq!(dim("typography.result_size.$value"), TYPE_RESULT);
        assert_eq!(dim("typography.result_line.$value"), TYPE_RESULT_LINE);
        assert_eq!(dim("typography.hud_size.$value"), TYPE_HUD);
        assert_eq!(dim("typography.hud_line.$value"), TYPE_HUD_LINE);
        assert_eq!(dim("typography.badge_size.$value"), TYPE_BADGE);
        assert_eq!(dim("typography.badge_line.$value"), TYPE_BADGE_LINE);
        // FR-053 (U3 F-9): spacing/radius-scale UI-оверлеев.
        assert_eq!(dim("spacing.s.$value"), SPACING_S);
        assert_eq!(dim("spacing.sm.$value"), SPACING_SM);
        assert_eq!(dim("spacing.md.$value"), SPACING_MD);
        assert_eq!(dim("spacing.lg.$value"), SPACING_LG);
        assert_eq!(dim("spacing.xl.$value"), SPACING_XL);
        assert_eq!(dim("radius.chip.$value"), RADIUS_CHIP);
        assert_eq!(dim("radius.card.$value"), CARD_CORNER_RADIUS);
        assert_eq!(dim("radius.panel.$value"), RADIUS_PANEL);
        assert_eq!(dim("radius.pill.$value"), RADIUS_PILL);
    }

    #[test]
    fn json_motion_match_rust_mirror() {
        let root: Value = serde_json::from_str(MOTION).expect("motion.json валиден");
        let ms = |path: &str| -> u64 { color(&root, path).as_u64().unwrap() };
        assert_eq!(ms("focus_fade_ms.$value"), FOCUS_FADE_MS);
        assert_eq!(ms("camera_flight_ms.$value"), CAMERA_FLIGHT_MS);
        assert_eq!(ms("result_pulse_ms.$value"), RESULT_PULSE_MS);
        assert_eq!(ms("focus_breath_ms.$value"), FOCUS_BREATH_MS);
    }
}
