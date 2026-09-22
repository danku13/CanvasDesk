//! FR-039: модалка настроек в стиле Obsidian — чистая модель
//! (образец [`crate::hints_ui`]/`template_ui`): табы ([`SETTINGS_TABS`]),
//! род строки ([`row_kind`]), перечень значений многозначной настройки
//! ([`dropdown_options`]) с чистым применением выбора
//! ([`apply_dropdown_value`]), геометрия модалки и меню с клампом к окну
//! ([`modal_layout`]/[`dropdown_layout`]) и hit-тесты.
//!
//! Отличия от панели FR-026 (ревизия владельцем 2026-09-20): модалка по
//! центру окна над затемнением (не панель у угла кнопки); строки
//! «лейбл + описание + контрол» (pill-тумблер / dropdown-кнопка);
//! таб «Внешний вид» — карточки темы + dropdown языка (FR-040); размер
//! адаптивный с потолками (расчёт на Full HD+, контрольные точки
//! [`MODAL_BP_COMPACT`]/[`MODAL_BP_MOBILE`] — константы без реализации);
//! поиск по настройкам — отклонён владельцем (не реализуем).
//!
//! Все тексты настроек — через таблицу строк [`crate::i18n`] (ключи,
//! без хардкода — база локализации FR-040); значения из `canvas-core`
//! (`Corner::label` и др.) в рендер не идут. Схема `config.toml` не
//! меняется — это реорганизация UI.

use canvas_core::{
    theme_presets, Corner, GridDensity, GridStyle, Language, Settings, Theme, PORT_ZONE_PRESETS,
    SNAP_COARSE_ZOOM_PRESETS, SNAP_SUB_ZOOM_PRESETS, SNAP_TOLERANCE_PRESETS,
};

use crate::i18n::{self, keys};
use crate::ui::{point_in_rect, SETTINGS_MARGIN};

/// Высота пункта выпадающего меню.
pub const DROPDOWN_ROW_H: f32 = 26.0;
/// Внутренние поля выпадающего меню.
pub const DROPDOWN_MARGIN: f32 = 6.0;

// Модалка настроек (FR-039): адаптивный размер с потолками — расчёт на
// десктопы Full HD и выше (1920×1080 → ~864×640).

/// Минимальная ширина модалки (логические px).
pub const MODAL_MIN_W: f32 = 560.0;
/// Потолок ширины модалки (логические px).
pub const MODAL_MAX_W: f32 = 880.0;
/// Минимальная высота модалки (логические px).
pub const MODAL_MIN_H: f32 = 400.0;
/// Потолок высоты модалки (логические px).
pub const MODAL_MAX_H: f32 = 640.0;
/// Контрольная точка «компакт» (ширина окна, логические px) — задел под
/// будущую адаптацию планшетов; поведение ниже точки в v1 не реализуется.
pub const MODAL_BP_COMPACT: f32 = 1280.0;
/// Контрольная точка «мобильный» (ширина окна, логические px) — задел под
/// будущую адаптацию телефонов; поведение ниже точки в v1 не реализуется.
pub const MODAL_BP_MOBILE: f32 = 768.0;
/// Ширина левой колонки навигации (клампится на узких окнах).
pub const MODAL_NAV_WIDTH: f32 = 180.0;
/// Высота пункта левой навигации.
pub const MODAL_NAV_ITEM_H: f32 = 34.0;
/// Высота заголовка раздела в правой панели.
pub const MODAL_TITLE_HEIGHT: f32 = 30.0;
/// Внутренние поля модалки и её панелей.
pub const MODAL_PADDING: f32 = 12.0;
/// Высота строки настройки (лейбл + описание, контрол справа).
pub const MODAL_ROW_HEIGHT: f32 = 44.0;
/// Высота карточки темы (таб «Внешний вид»).
pub const MODAL_THEME_CARD_H: f32 = 56.0;
/// Зазор между карточками темы и следующей секцией.
pub const MODAL_THEME_GAP: f32 = 14.0;
/// Высота строки-подсказки внизу левой колонки.
pub const MODAL_HINT_HEIGHT: f32 = 24.0;
/// Размер pill-тумблера: трек и ручка (логические px).
pub const PILL_TRACK_W: f32 = 34.0;
pub const PILL_TRACK_H: f32 = 18.0;
pub const PILL_KNOB: f32 = 14.0;
/// Размер dropdown-кнопки в строке.
pub const DROPDOWN_BTN_W: f32 = 170.0;
pub const DROPDOWN_BTN_H: f32 = 24.0;
/// Резерв ширины под контрол и поля при расчёте ширины текста лейбла/описания
/// (текст не наезжает на контрол справа).
pub const MODAL_ROW_LABEL_W: f32 = DROPDOWN_BTN_W + MODAL_PADDING + 6.0;

// Строки настроек: порядок плоского списка = прежний (FR-026) + Language
// FR-040. Источник инварианта полноты табов.

/// Строка-переключатель модалки настроек.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRow {
    /// Угол летающей кнопки (цикл по 4 углам).
    ButtonCorner,
    /// Сетка канваса вкл/выкл.
    Grid,
    /// Вид сетки: линии или точки.
    GridStyle,
    /// Плотность сетки (цикл по 3 вариантам).
    GridDensity,
    /// Связи огибают посторонние ноды.
    EdgesAvoid,
    /// Зона захвата портов для drag связи (CR-003): цикл по пресетам.
    PortZone,
    /// FR-025: построчные точки выхода на нодах с расчётами вкл/выкл.
    LinePorts,
    /// FR-016 (CP5): оверлей узких мест (рамка/бейджи по ρ и W) вкл/выкл.
    BottleneckOverlay,
    /// Режим фокуса связей (T23, brainstorm-focus) вкл/выкл.
    FocusMode,
    /// FR-042 (F-13): агрегация связей (LOD-0 пучки + main stage) вкл/выкл.
    EdgeAggregation,
    /// HUD (F3) включён при старте.
    HudOnStart,
    /// FR-040: язык интерфейса (русский/English).
    Language,
    /// FR-047 (PRD-0006 D4/F-8): тема-пресет (Nord, Dracula, Catppuccin,
    /// Solarized, Tokyo Night, Gruvbox) — dropdown в табе «Внешний вид»;
    /// «Классическая» = выбор по карточкам тёмной/светлой.
    ThemePreset,

    /// FR-038 (п.5): мастер-тумблер магнитной раскладки — гасит весь снап
    /// без сброса остальных настроек.
    SnapEnabled,
    /// FR-038 (п.1/19): привязка к сетке на отпускании drag.
    SnapGrid,
    /// FR-038 (п.6/19): направляющие соседей (края/центры/середины).
    SnapGuides,
    /// FR-038 (п.15/19): collision-avoidance — не проходить сквозь ноды.
    SnapCollision,
    /// FR-038 (п.8/12): допуск направляющих — цикл по пресетам.
    SnapTolerance,
    /// FR-038 (п.3): порог sub-сетки — цикл по пресетам.
    SnapSubZoom,
    /// FR-038 (п.3): порог coarse-сетки — цикл по пресетами.
    SnapCoarseZoom,
    /// PRD-0007 (FR-048 X2, AC-2.3): лимит глубины авто-раскрытия
    /// explain-дерева (0 — без ограничения) — dropdown в табе «Канвас».
    ExplainDepthLimit,
    /// PRD-0007 (FR-048 X4, AC-5.5): фоновый детектор автосвязи —
    /// тумблер в табе «Связи и порты».
    AutolinkEnabled,
}

/// Плоский список всех строк настроек (инвариант полноты: union строк
/// табов == этот список без дублей). Тема — вне списка (карточки,
/// отдельное поле `settings.theme`).
pub const SETTINGS_ROWS: [SettingsRow; 22] = [
    SettingsRow::ButtonCorner,
    SettingsRow::Grid,
    SettingsRow::GridStyle,
    SettingsRow::GridDensity,
    SettingsRow::EdgesAvoid,
    SettingsRow::PortZone,
    SettingsRow::LinePorts,
    SettingsRow::BottleneckOverlay,
    SettingsRow::SnapEnabled,
    SettingsRow::SnapGrid,
    SettingsRow::SnapGuides,
    SettingsRow::SnapCollision,
    SettingsRow::SnapTolerance,
    SettingsRow::SnapSubZoom,
    SettingsRow::SnapCoarseZoom,
    SettingsRow::FocusMode,
    SettingsRow::EdgeAggregation,
    SettingsRow::HudOnStart,
    SettingsRow::ThemePreset,
    SettingsRow::Language,
    SettingsRow::ExplainDepthLimit,
    SettingsRow::AutolinkEnabled,
];

/// Таб модалки (FR-039): иконка + ключ заголовка + строки. Тема —
/// отдельный вид контента таба «Внешний вид» (карточки), не строка.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsTab {
    /// Ключ заголовка таба (таблица [`crate::i18n`]).
    pub title_key: &'static str,
    /// Иконка-глиф (набор как у всего UI: ⚙/▾/✓ — системный фолбэк).
    pub icon: &'static str,
    /// Таб содержит карточки темы (ровно один — «Внешний вид»).
    pub theme_cards: bool,
    /// Строки таба в порядке отображения.
    pub rows: &'static [SettingsRow],
}

/// Табы настроек (FR-039 §1, распределение v1): «Общие» — кнопка и HUD;
/// «Канвас» — сетка и оверлей узких мест; «Связи и порты» — связи/порты/
/// фокус + построчные точки выхода FR-025; «Внешний вид» — карточки темы
/// и язык FR-040. FR-038 дополнит модель пятым табом «Snap».
pub const SETTINGS_TABS: [SettingsTab; 5] = [
    SettingsTab {
        title_key: keys::TAB_GENERAL,
        icon: "◎",
        theme_cards: false,
        rows: &[SettingsRow::ButtonCorner, SettingsRow::HudOnStart],
    },
    SettingsTab {
        title_key: keys::TAB_CANVAS,
        icon: "▦",
        theme_cards: false,
        rows: &[
            SettingsRow::Grid,
            SettingsRow::GridStyle,
            SettingsRow::GridDensity,
            SettingsRow::BottleneckOverlay,
            // PRD-0007 (FR-048 X2): лимит глубины explain-дерева (AC-2.3).
            SettingsRow::ExplainDepthLimit,
        ],
    },
    SettingsTab {
        title_key: keys::TAB_SNAP,
        icon: "≡",
        theme_cards: false,
        rows: &[
            SettingsRow::SnapEnabled,
            SettingsRow::SnapGrid,
            SettingsRow::SnapGuides,
            SettingsRow::SnapCollision,
            SettingsRow::SnapTolerance,
            SettingsRow::SnapSubZoom,
            SettingsRow::SnapCoarseZoom,
        ],
    },
    SettingsTab {
        title_key: keys::TAB_EDGES,
        icon: "⇄",
        theme_cards: false,
        rows: &[
            SettingsRow::EdgesAvoid,
            SettingsRow::PortZone,
            SettingsRow::LinePorts,
            SettingsRow::FocusMode,
            SettingsRow::EdgeAggregation,
            // PRD-0007 (FR-048 X4): тумблер фонового детектора автосвязи
            // (AC-5.5 — раздел «Связи и порты»).
            SettingsRow::AutolinkEnabled,
        ],
    },
    SettingsTab {
        title_key: keys::TAB_APPEARANCE,
        icon: "◐",
        theme_cards: true,
        rows: &[SettingsRow::ThemePreset, SettingsRow::Language],
    },
];

/// Ключ лейбла строки (значение показывает контрол — лейбл без «: вкл»,
/// FR-039 §5).
pub fn row_label_key(row: SettingsRow) -> &'static str {
    match row {
        SettingsRow::ButtonCorner => keys::ROW_BUTTON_CORNER,
        SettingsRow::Grid => keys::ROW_GRID,
        SettingsRow::GridStyle => keys::ROW_GRID_STYLE,
        SettingsRow::GridDensity => keys::ROW_GRID_DENSITY,
        SettingsRow::EdgesAvoid => keys::ROW_EDGES_AVOID,
        SettingsRow::PortZone => keys::ROW_PORT_ZONE,
        SettingsRow::LinePorts => keys::ROW_LINE_PORTS,
        SettingsRow::BottleneckOverlay => keys::ROW_BOTTLENECK,
        SettingsRow::FocusMode => keys::ROW_FOCUS_MODE,
        SettingsRow::EdgeAggregation => keys::ROW_EDGE_AGGREGATION,
        SettingsRow::HudOnStart => keys::ROW_HUD_ON_START,
        SettingsRow::ThemePreset => keys::ROW_THEME_PRESET,
        SettingsRow::Language => keys::ROW_LANGUAGE,
        SettingsRow::SnapEnabled => keys::ROW_SNAP_ENABLED,
        SettingsRow::SnapGrid => keys::ROW_SNAP_GRID,
        SettingsRow::SnapGuides => keys::ROW_SNAP_GUIDES,
        SettingsRow::SnapCollision => keys::ROW_SNAP_COLLISION,
        SettingsRow::SnapTolerance => keys::ROW_SNAP_TOLERANCE,
        SettingsRow::SnapSubZoom => keys::ROW_SNAP_SUB_ZOOM,
        SettingsRow::SnapCoarseZoom => keys::ROW_SNAP_COARSE_ZOOM,
        SettingsRow::ExplainDepthLimit => keys::ROW_EXPLAIN_DEPTH,
        SettingsRow::AutolinkEnabled => keys::ROW_AUTOLINK,
    }
}

/// Ключ описания строки (приглушённый текст под лейблом — v1 по решению
/// владельца).
pub fn row_desc_key(row: SettingsRow) -> &'static str {
    match row {
        SettingsRow::ButtonCorner => keys::DESC_BUTTON_CORNER,
        SettingsRow::Grid => keys::DESC_GRID,
        SettingsRow::GridStyle => keys::DESC_GRID_STYLE,
        SettingsRow::GridDensity => keys::DESC_GRID_DENSITY,
        SettingsRow::EdgesAvoid => keys::DESC_EDGES_AVOID,
        SettingsRow::PortZone => keys::DESC_PORT_ZONE,
        SettingsRow::LinePorts => keys::DESC_LINE_PORTS,
        SettingsRow::BottleneckOverlay => keys::DESC_BOTTLENECK,
        SettingsRow::FocusMode => keys::DESC_FOCUS_MODE,
        SettingsRow::EdgeAggregation => keys::DESC_EDGE_AGGREGATION,
        SettingsRow::HudOnStart => keys::DESC_HUD_ON_START,
        SettingsRow::ThemePreset => keys::DESC_THEME_PRESET,
        SettingsRow::Language => keys::DESC_LANGUAGE,
        SettingsRow::SnapEnabled => keys::DESC_SNAP_ENABLED,
        SettingsRow::SnapGrid => keys::DESC_SNAP_GRID,
        SettingsRow::SnapGuides => keys::DESC_SNAP_GUIDES,
        SettingsRow::SnapCollision => keys::DESC_SNAP_COLLISION,
        SettingsRow::SnapTolerance => keys::DESC_SNAP_TOLERANCE,
        SettingsRow::SnapSubZoom => keys::DESC_SNAP_SUB_ZOOM,
        SettingsRow::SnapCoarseZoom => keys::DESC_SNAP_COARSE_ZOOM,
        SettingsRow::ExplainDepthLimit => keys::DESC_EXPLAIN_DEPTH,
        SettingsRow::AutolinkEnabled => keys::DESC_AUTOLINK,
    }
}

/// Род строки: тумблер (pill-тумблер, клик переключает) или dropdown
/// (клик открывает меню значений). Инвариант (юнит-тест): `Toggle` — ровно
/// для `bool`-полей `Settings`, `Dropdown` — для остальных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Булева настройка: клик — переключить (pill-тумблер, меню избыточно).
    Toggle,
    /// Многозначная настройка: клик — открыть выпадающее меню.
    Dropdown,
}

/// Род строки панели.
pub fn row_kind(row: SettingsRow) -> RowKind {
    match row {
        SettingsRow::ButtonCorner
        | SettingsRow::GridStyle
        | SettingsRow::GridDensity
        | SettingsRow::PortZone
        | SettingsRow::Language
        | SettingsRow::ThemePreset
        | SettingsRow::ExplainDepthLimit
        | SettingsRow::SnapTolerance
        | SettingsRow::SnapSubZoom
        | SettingsRow::SnapCoarseZoom => RowKind::Dropdown,
        SettingsRow::Grid
        | SettingsRow::EdgesAvoid
        | SettingsRow::LinePorts
        | SettingsRow::BottleneckOverlay
        | SettingsRow::SnapEnabled
        | SettingsRow::SnapGrid
        | SettingsRow::SnapGuides
        | SettingsRow::SnapCollision
        | SettingsRow::FocusMode
        | SettingsRow::EdgeAggregation
        | SettingsRow::HudOnStart
        | SettingsRow::AutolinkEnabled => RowKind::Toggle,
    }
}

/// Текущее значение настройки для dropdown-кнопки (контрол на строке
/// показывает значение — HIG «Pop-Up Buttons»; тексты из таблицы
/// [`crate::i18n`], язык — `settings.language`). Для тумблеров — `None`
/// (состояние видно по позиции pill-ручки).
pub fn dropdown_value(row: SettingsRow, settings: &Settings) -> Option<String> {
    let language = settings.language;
    match row {
        SettingsRow::ButtonCorner => {
            let key = match settings.button_corner {
                Corner::TopLeft => keys::CORNER_TOP_LEFT,
                Corner::TopRight => keys::CORNER_TOP_RIGHT,
                Corner::BottomLeft => keys::CORNER_BOTTOM_LEFT,
                Corner::BottomRight => keys::CORNER_BOTTOM_RIGHT,
            };
            Some(i18n::tr(language, key).to_owned())
        }
        SettingsRow::GridStyle => {
            let key = match settings.grid_style {
                GridStyle::Lines => keys::GRID_STYLE_LINES,
                GridStyle::Dots => keys::GRID_STYLE_DOTS,
            };
            Some(i18n::tr(language, key).to_owned())
        }
        SettingsRow::GridDensity => {
            let key = match settings.grid_density {
                GridDensity::Dense => keys::GRID_DENSITY_DENSE,
                GridDensity::Medium => keys::GRID_DENSITY_MEDIUM,
                GridDensity::Sparse => keys::GRID_DENSITY_SPARSE,
            };
            Some(i18n::tr(language, key).to_owned())
        }
        SettingsRow::PortZone => Some(format!("{} px", settings.port_zone_px as i32)),
        SettingsRow::Language => Some(settings.language.native_label().to_owned()),
        SettingsRow::ThemePreset => Some(match settings.active_preset() {
            Some(id) => theme_presets::find(id).unwrap().label.to_owned(),
            None => i18n::tr(language, keys::THEME_PRESET_CLASSIC).to_owned(),
        }),
        SettingsRow::SnapTolerance => Some(format!("{} px", settings.snap_tolerance_px as i32)),
        // PRD-0007 (AC-2.3): «0» показывается как «без ограничения».
        SettingsRow::ExplainDepthLimit => Some(if settings.explain_depth_limit == 0 {
            i18n::tr(language, keys::VALUE_EXPLAIN_ALL).to_owned()
        } else {
            format!("{}", settings.explain_depth_limit)
        }),
        SettingsRow::SnapSubZoom => {
            Some(format!("{}%", (settings.snap_grid_sub_zoom * 100.0) as i32))
        }
        SettingsRow::SnapCoarseZoom => Some(format!(
            "{}%",
            (settings.snap_grid_coarse_zoom * 100.0) as i32
        )),
        SettingsRow::Grid
        | SettingsRow::EdgesAvoid
        | SettingsRow::LinePorts
        | SettingsRow::BottleneckOverlay
        | SettingsRow::SnapEnabled
        | SettingsRow::SnapGrid
        | SettingsRow::SnapGuides
        | SettingsRow::SnapCollision
        | SettingsRow::FocusMode
        | SettingsRow::EdgeAggregation
        | SettingsRow::AutolinkEnabled
        | SettingsRow::HudOnStart => None,
    }
}

/// Полный перечень значений многозначной настройки с отметкой текущего
/// (`(значение, текущее)`). Порядок пунктов = порядку цикла `.next()` —
/// выбор пункта `i` эквивалентен соответствующему числу нажатий цикла
/// (инвариант, юнит-тест). Для тумблеров — пустой список (меню избыточно).
///
/// Зона портов: текущим считается пресет, от которого цикл
/// `next_port_zone` шагнул бы дальше (последний пресет ≤ значения) —
/// ручная правка `config.toml` между пресетами всё равно получает отметку.
/// Язык (FR-040): названия — в собственной локали
/// ([`Language::native_label`], конвенция Obsidian/VS Code).
pub fn dropdown_options(row: SettingsRow, settings: &Settings) -> Vec<(String, bool)> {
    let language = settings.language;
    match row {
        SettingsRow::ButtonCorner => [
            (Corner::TopLeft, keys::CORNER_TOP_LEFT),
            (Corner::TopRight, keys::CORNER_TOP_RIGHT),
            (Corner::BottomRight, keys::CORNER_BOTTOM_RIGHT),
            (Corner::BottomLeft, keys::CORNER_BOTTOM_LEFT),
        ]
        .into_iter()
        .map(|(corner, key)| {
            (
                i18n::tr(language, key).to_owned(),
                settings.button_corner == corner,
            )
        })
        .collect(),
        SettingsRow::GridStyle => [
            (GridStyle::Lines, keys::GRID_STYLE_LINES),
            (GridStyle::Dots, keys::GRID_STYLE_DOTS),
        ]
        .into_iter()
        .map(|(style, key)| {
            (
                i18n::tr(language, key).to_owned(),
                settings.grid_style == style,
            )
        })
        .collect(),
        SettingsRow::GridDensity => [
            (GridDensity::Dense, keys::GRID_DENSITY_DENSE),
            (GridDensity::Medium, keys::GRID_DENSITY_MEDIUM),
            (GridDensity::Sparse, keys::GRID_DENSITY_SPARSE),
        ]
        .into_iter()
        .map(|(density, key)| {
            (
                i18n::tr(language, key).to_owned(),
                settings.grid_density == density,
            )
        })
        .collect(),
        SettingsRow::PortZone => {
            let current = PORT_ZONE_PRESETS
                .iter()
                .rposition(|preset| *preset <= settings.port_zone_px)
                .unwrap_or(0);
            PORT_ZONE_PRESETS
                .iter()
                .enumerate()
                .map(|(i, preset)| (format!("{} px", *preset as i32), i == current))
                .collect()
        }
        SettingsRow::Language => [Language::Ru, Language::En]
            .into_iter()
            .map(|lang| (lang.native_label().to_owned(), settings.language == lang))
            .collect(),
        // FR-047: первый пункт — «Классическая» (карточки тёмной/светлой),
        // далее — реестр пресетов в порядке регистрации. Порядок опций =
        // порядку apply_dropdown_value (инвариант, тест).
        SettingsRow::ThemePreset => {
            let active = settings.active_preset();
            let mut options = vec![(
                i18n::tr(language, keys::THEME_PRESET_CLASSIC).to_owned(),
                active.is_none(),
            )];
            options.extend(
                theme_presets::PRESETS
                    .iter()
                    .map(|preset| (preset.label.to_owned(), Some(preset.id) == active)),
            );
            options
        }
        SettingsRow::SnapTolerance => {
            let current = SNAP_TOLERANCE_PRESETS
                .iter()
                .rposition(|preset| *preset <= settings.snap_tolerance_px)
                .unwrap_or(0);
            SNAP_TOLERANCE_PRESETS
                .iter()
                .enumerate()
                .map(|(i, preset)| (format!("{} px", *preset as i32), i == current))
                .collect()
        }
        SettingsRow::SnapSubZoom => {
            let current = SNAP_SUB_ZOOM_PRESETS
                .iter()
                .rposition(|preset| *preset <= settings.snap_grid_sub_zoom)
                .unwrap_or(0);
            SNAP_SUB_ZOOM_PRESETS
                .iter()
                .enumerate()
                .map(|(i, preset)| (format!("{}%", (*preset * 100.0) as i32), i == current))
                .collect()
        }
        SettingsRow::SnapCoarseZoom => {
            let current = SNAP_COARSE_ZOOM_PRESETS
                .iter()
                .rposition(|preset| *preset <= settings.snap_grid_coarse_zoom)
                .unwrap_or(0);
            SNAP_COARSE_ZOOM_PRESETS
                .iter()
                .enumerate()
                .map(|(i, preset)| (format!("{}%", (*preset * 100.0) as i32), i == current))
                .collect()
        }
        // PRD-0007 (AC-2.3): пресеты глубины [2, 3, 4, без ограничения].
        // Порядок опций = порядку apply_dropdown_value (инвариант, тест).
        SettingsRow::ExplainDepthLimit => {
            let current = settings.explain_depth_limit;
            let all = i18n::tr(language, keys::VALUE_EXPLAIN_ALL).to_owned();
            [
                (2u8, "2".to_owned()),
                (3, "3".to_owned()),
                (4, "4".to_owned()),
                (0, all),
            ]
            .into_iter()
            .map(|(value, label)| (label, current == value))
            .collect()
        }
        SettingsRow::Grid
        | SettingsRow::EdgesAvoid
        | SettingsRow::LinePorts
        | SettingsRow::BottleneckOverlay
        | SettingsRow::SnapEnabled
        | SettingsRow::SnapGrid
        | SettingsRow::SnapGuides
        | SettingsRow::SnapCollision
        | SettingsRow::FocusMode
        | SettingsRow::EdgeAggregation
        | SettingsRow::AutolinkEnabled
        | SettingsRow::HudOnStart => Vec::new(),
    }
}

/// Применить выбор пункта dropdown к настройкам (чистая функция — только
/// значение; побочные эффекты рендера — на стороне `App`, сохранение
/// конфига — общий хвост вызывающего). Индекс вне диапазона — без изменений.
pub fn apply_dropdown_value(settings: &mut Settings, row: SettingsRow, index: usize) {
    match row {
        SettingsRow::ButtonCorner => {
            let corners = [
                Corner::TopLeft,
                Corner::TopRight,
                Corner::BottomRight,
                Corner::BottomLeft,
            ];
            if let Some(corner) = corners.get(index) {
                settings.button_corner = *corner;
            }
        }
        SettingsRow::GridStyle => {
            let styles = [GridStyle::Lines, GridStyle::Dots];
            if let Some(style) = styles.get(index) {
                settings.grid_style = *style;
            }
        }
        SettingsRow::GridDensity => {
            let densities = [GridDensity::Dense, GridDensity::Medium, GridDensity::Sparse];
            if let Some(density) = densities.get(index) {
                settings.grid_density = *density;
            }
        }
        SettingsRow::PortZone => {
            if let Some(preset) = PORT_ZONE_PRESETS.get(index) {
                settings.port_zone_px = *preset;
            }
        }
        // FR-040: выбор языка — прямое присваивание (не цикл), применяется
        // на лету; сохранение — общий хвост вызывающего.
        SettingsRow::Language => {
            let languages = [Language::Ru, Language::En];
            if let Some(language) = languages.get(index) {
                settings.language = *language;
            }
        }
        // FR-047: индекс 0 — «Классическая» (сброс пресета, выбор по
        // карточкам тёмной/светлой); далее — реестр PRESETS в порядке
        // регистрации (порядок == dropdown_options, тест).
        SettingsRow::ThemePreset => {
            if index == 0 {
                settings.theme_preset.clear();
            } else if let Some(preset) = theme_presets::PRESETS.get(index - 1) {
                settings.theme_preset = preset.id.to_string();
            }
        }
        SettingsRow::SnapTolerance => {
            if let Some(preset) = SNAP_TOLERANCE_PRESETS.get(index) {
                settings.snap_tolerance_px = *preset;
            }
        }
        SettingsRow::SnapSubZoom => {
            if let Some(preset) = SNAP_SUB_ZOOM_PRESETS.get(index) {
                settings.snap_grid_sub_zoom = *preset;
            }
        }
        SettingsRow::SnapCoarseZoom => {
            if let Some(preset) = SNAP_COARSE_ZOOM_PRESETS.get(index) {
                settings.snap_grid_coarse_zoom = *preset;
            }
        }
        // PRD-0007 (AC-2.3): порядок опций — [2, 3, 4, без ограничения].
        SettingsRow::ExplainDepthLimit => {
            if let Some(value) = [2u8, 3, 4, 0].get(index) {
                settings.explain_depth_limit = *value;
            }
        }
        SettingsRow::Grid
        | SettingsRow::EdgesAvoid
        | SettingsRow::LinePorts
        | SettingsRow::BottleneckOverlay
        | SettingsRow::SnapEnabled
        | SettingsRow::SnapGrid
        | SettingsRow::SnapGuides
        | SettingsRow::SnapCollision
        | SettingsRow::FocusMode
        | SettingsRow::EdgeAggregation
        | SettingsRow::AutolinkEnabled
        | SettingsRow::HudOnStart => {}
    }
}

/// Состояние выпадающего меню настроек (FR-026): какая строка открыта и
/// клавиатурное выделение пункта. Пункты вычисляются на кадр из
/// [`dropdown_options`] — состояние не может устареть. Хранится в `App`,
/// сбрасывается при закрытии модалки/меню.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DropdownState {
    /// Открытое меню строки (`None` — все закрыты).
    pub open_row: Option<SettingsRow>,
    /// Клавиатурное выделение (индекс в списке пунктов).
    pub selected: usize,
}

impl DropdownState {
    /// Открыто ли какое-нибудь меню.
    pub fn is_open(&self) -> bool {
        self.open_row.is_some()
    }

    /// Закрыть меню и сбросить выделение.
    pub fn reset(&mut self) {
        self.open_row = None;
        self.selected = 0;
    }

    /// Открыть меню строки; выделение — на текущем значении (первая
    /// отметка), чтобы Enter сразу применял видимое состояние.
    pub fn open(&mut self, row: SettingsRow, settings: &Settings) {
        self.open_row = Some(row);
        self.selected = dropdown_options(row, settings)
            .iter()
            .position(|(_, current)| *current)
            .unwrap_or(0);
    }

    /// Сдвиг выделения с закольцовыванием; true — было изменение.
    pub fn move_selection(&mut self, delta: i32, count: usize) -> bool {
        if count == 0 {
            return false;
        }
        let len = count as i32;
        let next = (self.selected as i32 + delta).rem_euclid(len);
        if next == self.selected as i32 {
            return false;
        }
        self.selected = next as usize;
        true
    }
}

/// Геометрия модалки настроек (FR-039): rect целиком, пункты левой
/// навигации, заголовок раздела, строки активного таба, карточки темы,
/// подсказка внизу левой колонки.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalLayout {
    /// Rect модалки `[x, y, w, h]` (логические px).
    pub rect: [f32; 4],
    /// Ширина левой колонки навигации (кламп на узких окнах).
    pub nav_w: f32,
    /// Rect'ы пунктов навигации по индексу таба.
    pub nav_items: Vec<[f32; 4]>,
    /// Rect заголовка раздела (правая панель).
    pub title_rect: [f32; 4],
    /// Зона контента правой панели (строки внутри неё).
    pub content_rect: [f32; 4],
    /// Строки активного таба в порядке отображения.
    pub rows: Vec<(SettingsRow, [f32; 4])>,
    /// Карточки темы `[Dark, Light]` (пустые rect'ы вне таба «Внешний вид»).
    pub theme_cards: [[f32; 4]; 2],
    /// Rect подсказки внизу левой колонки.
    pub hint_rect: [f32; 4],
}

impl ModalLayout {
    /// Rect строки настройки активного таба (`None` для отсутствующих).
    pub fn row_rect(&self, row: SettingsRow) -> Option<[f32; 4]> {
        self.rows
            .iter()
            .find_map(|(r, rect)| (*r == row).then_some(*rect))
    }

    /// Rect карточки темы.
    pub fn theme_card_rect(&self, theme: Theme) -> [f32; 4] {
        match theme {
            Theme::Dark => self.theme_cards[0],
            Theme::Light => self.theme_cards[1],
        }
    }
}

/// Rect контрола внутри строки: pill-тумблер у тумблера, dropdown-кнопка
/// у выпадающего списка (справа от строки, вертикально по центру).
pub fn control_rect(row_rect: [f32; 4], kind: RowKind) -> [f32; 4] {
    match kind {
        RowKind::Toggle => [
            row_rect[0] + row_rect[2] - MODAL_PADDING - PILL_TRACK_W,
            row_rect[1] + (row_rect[3] - PILL_TRACK_H) / 2.0,
            PILL_TRACK_W,
            PILL_TRACK_H,
        ],
        RowKind::Dropdown => [
            row_rect[0] + row_rect[2] - MODAL_PADDING - DROPDOWN_BTN_W,
            row_rect[1] + (row_rect[3] - DROPDOWN_BTN_H) / 2.0,
            DROPDOWN_BTN_W,
            DROPDOWN_BTN_H,
        ],
    }
}

/// Rect ручки pill-тумблера по треку и состоянию (включён — справа,
/// акцентный цвет; выключен — слева, приглушённый).
pub fn pill_knob_rect(track: [f32; 4], on: bool) -> [f32; 4] {
    let x = if on {
        track[0] + track[2] - PILL_KNOB - 2.0
    } else {
        track[0] + 2.0
    };
    [
        x,
        track[1] + (track[3] - PILL_KNOB) / 2.0,
        PILL_KNOB,
        PILL_KNOB,
    ]
}

/// Адаптивный размер модалки: ширина `(vw * 0.45).clamp(MIN_W, MAX_W)`,
/// высота `(vh * 0.6).clamp(MIN_H, MAX_H)`, затем кламп в окно с полями
/// [`SETTINGS_MARGIN`] — инвариант: модалка целиком в окне при любом
/// viewport (320×240 включительно).
fn modal_size(viewport: [f32; 2]) -> [f32; 2] {
    let w = (viewport[0] * 0.45).clamp(MODAL_MIN_W, MODAL_MAX_W);
    let h = (viewport[1] * 0.6).clamp(MODAL_MIN_H, MODAL_MAX_H);
    let w = w.min((viewport[0] - SETTINGS_MARGIN * 2.0).max(1.0));
    let h = h.min((viewport[1] - SETTINGS_MARGIN * 2.0).max(1.0));
    [w, h]
}

/// Геометрия модалки с позициями навигации, заголовка, строк активного
/// таба и карточек темы. Активный таб — индекс в [`SETTINGS_TABS`]
/// (вне диапазона — первый таб; состояние `App::settings_tab` клампится
/// на вызывающей стороне).
pub fn modal_layout(tab: usize, viewport: [f32; 2]) -> ModalLayout {
    let tab_def = SETTINGS_TABS.get(tab).unwrap_or(&SETTINGS_TABS[0]);
    let [w, h] = modal_size(viewport);
    let x = (viewport[0] - w) / 2.0;
    let y = (viewport[1] - h) / 2.0;
    let rect = [x, y, w, h];
    // Левая колонка: на узких окнах сжимается (40% ширины модалки), но не
    // исчезает — инвариант различимости навигации при клампе 320×240.
    let nav_w = MODAL_NAV_WIDTH.min(rect[2] * 0.4);
    let nav_items = SETTINGS_TABS
        .iter()
        .enumerate()
        .map(|(i, _)| {
            [
                rect[0],
                rect[1] + MODAL_PADDING + i as f32 * MODAL_NAV_ITEM_H,
                nav_w,
                MODAL_NAV_ITEM_H,
            ]
        })
        .collect();
    let hint_rect = [
        rect[0] + MODAL_PADDING,
        rect[1] + rect[3] - MODAL_PADDING - MODAL_HINT_HEIGHT,
        nav_w - MODAL_PADDING,
        MODAL_HINT_HEIGHT,
    ];
    // Правая панель: заголовок раздела + зона контента
    let content_x = rect[0] + nav_w;
    let content_w = rect[2] - nav_w;
    let title_rect = [
        content_x + MODAL_PADDING,
        rect[1] + MODAL_PADDING,
        content_w - MODAL_PADDING * 2.0,
        MODAL_TITLE_HEIGHT,
    ];
    let content_y = rect[1] + MODAL_PADDING + MODAL_TITLE_HEIGHT + 4.0;
    let content_h = rect[3] - MODAL_PADDING * 2.0 - MODAL_TITLE_HEIGHT - 4.0;
    let content_rect = [content_x, content_y, content_w, content_h];
    // Строки: единая сетка (высота MODAL_ROW_HEIGHT); в табе с карточками
    // темы строки начинаются ниже карточек.
    let cards_top = content_y;
    let rows_top = if tab_def.theme_cards {
        cards_top + MODAL_THEME_CARD_H + MODAL_THEME_GAP
    } else {
        cards_top
    };
    let row_w = content_w - MODAL_PADDING * 2.0;
    let rows = tab_def
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            (
                *row,
                [
                    content_x + MODAL_PADDING,
                    rows_top + i as f32 * MODAL_ROW_HEIGHT,
                    row_w,
                    MODAL_ROW_HEIGHT,
                ],
            )
        })
        .collect();
    // Карточки темы: две рядом («тёмная»/«светлая» — паттерн Obsidian
    // «Base theme»); вне таба «Внешний вид» — пустые rect'ы.
    let theme_cards = if tab_def.theme_cards {
        let gap = 10.0;
        let card_w = (row_w - gap) / 2.0;
        [
            [
                content_x + MODAL_PADDING,
                cards_top,
                card_w,
                MODAL_THEME_CARD_H,
            ],
            [
                content_x + MODAL_PADDING + card_w + gap,
                cards_top,
                card_w,
                MODAL_THEME_CARD_H,
            ],
        ]
    } else {
        [[0.0; 4]; 2]
    };
    ModalLayout {
        rect,
        nav_w,
        nav_items,
        title_rect,
        content_rect,
        rows,
        theme_cards,
        hint_rect,
    }
}

/// Hit-test пункта левой навигации: индекс таба под точкой или `None`
/// (вне пунктов — в том числе зона контента и подсказка).
pub fn modal_nav_at(layout: &ModalLayout, point: [f32; 2]) -> Option<usize> {
    layout
        .nav_items
        .iter()
        .position(|rect| point_in_rect(*rect, point))
}

/// Hit-test строки модалки: какая строка настройки под точкой. Заголовок,
/// зона контента мимо строк, навигация и паддинги — `None` (не кликабельны).
pub fn modal_row_at(layout: &ModalLayout, point: [f32; 2]) -> Option<SettingsRow> {
    layout
        .rows
        .iter()
        .find_map(|(row, rect)| point_in_rect(*rect, point).then_some(*row))
}

/// Hit-test карточки темы: `Some(theme)` — клик по карточке («тёмная»/
/// «светлая»); вне карточек — `None`.
pub fn modal_theme_card_at(layout: &ModalLayout, point: [f32; 2]) -> Option<Theme> {
    if point_in_rect(layout.theme_cards[0], point) {
        return Some(Theme::Dark);
    }
    if point_in_rect(layout.theme_cards[1], point) {
        return Some(Theme::Light);
    }
    None
}

/// Геометрия выпадающего меню `[x, y, w, h]` с клампом к окну: ниже
/// контрола-якоря; не влезает снизу — выше строки. Ширина — параметр
/// (FR-039: в модалке это ширина контрола, у угловой панели была жёстко
/// `PANEL_WIDTH`); кламп по горизонтали и вертикали — паттерн
/// `hints_ui::popup_layout` (FR-021). `count == 0` — пустой rect.
pub fn dropdown_layout(anchor: [f32; 4], viewport: [f32; 2], count: usize, width: f32) -> [f32; 4] {
    if count == 0 {
        return [0.0; 4];
    }
    let height = count as f32 * DROPDOWN_ROW_H + DROPDOWN_MARGIN * 2.0;
    let width = width
        .max(80.0)
        .min((viewport[0] - DROPDOWN_MARGIN * 2.0).max(0.0));
    let mut x = anchor[0];
    if x + width > viewport[0] - DROPDOWN_MARGIN {
        x = viewport[0] - DROPDOWN_MARGIN - width;
    }
    let x = x.max(DROPDOWN_MARGIN);
    // Ниже контрола; не влезает снизу — выше (не перекрывая саму строку)
    let below = anchor[1] + anchor[3] + 2.0;
    let y = if below + height <= viewport[1] - DROPDOWN_MARGIN {
        below
    } else {
        (anchor[1] - height - 2.0).max(DROPDOWN_MARGIN)
    };
    [x, y, width, height]
}

/// Hit-test пункта выпадающего меню: индекс пункта под точкой или `None`
/// (вне меню, в полях-паддингах, за последним пунктом).
pub fn dropdown_item_at(menu: [f32; 4], count: usize, point: [f32; 2]) -> Option<usize> {
    if !point_in_rect(menu, point) {
        return None;
    }
    let rel = point[1] - menu[1] - DROPDOWN_MARGIN;
    if rel < 0.0 {
        return None;
    }
    let index = (rel / DROPDOWN_ROW_H) as usize;
    (index < count).then_some(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::tr;

    /// Инвариант полноты (перенос FR-026 на табы): union строк табов ==
    /// SETTINGS_ROWS — ни одна настройка не потеряна и не задублирована;
    /// карточки темы — ровно у одного таба; ключи/иконки табов непусты.
    #[test]
    fn tabs_cover_all_rows() {
        let mut seen = Vec::new();
        let mut theme_tabs = 0;
        for tab in &SETTINGS_TABS {
            assert!(!tab.title_key.is_empty(), "ключ заголовка таба пуст");
            assert!(!tab.icon.is_empty(), "иконка таба пуста");
            assert!(!tab.rows.is_empty(), "пустой таб: {}", tab.title_key);
            if tab.theme_cards {
                theme_tabs += 1;
            }
            for &row in tab.rows {
                assert!(!seen.contains(&row), "дубль строки в табах: {row:?}");
                seen.push(row);
            }
        }
        assert_eq!(theme_tabs, 1, "карточки темы — ровно один таб");
        for row in SETTINGS_ROWS {
            assert!(seen.contains(&row), "строка вне табов: {row:?}");
        }
        assert_eq!(seen.len(), SETTINGS_ROWS.len());
        // Распределение v1 (FR-039 §1)
        assert_eq!(
            SETTINGS_TABS[0].rows,
            &[SettingsRow::ButtonCorner, SettingsRow::HudOnStart]
        );
        assert_eq!(
            SETTINGS_TABS[1].rows,
            &[
                SettingsRow::Grid,
                SettingsRow::GridStyle,
                SettingsRow::GridDensity,
                SettingsRow::BottleneckOverlay,
                SettingsRow::ExplainDepthLimit
            ]
        );
        assert_eq!(
            SETTINGS_TABS[2].rows,
            &[
                SettingsRow::SnapEnabled,
                SettingsRow::SnapGrid,
                SettingsRow::SnapGuides,
                SettingsRow::SnapCollision,
                SettingsRow::SnapTolerance,
                SettingsRow::SnapSubZoom,
                SettingsRow::SnapCoarseZoom
            ]
        );
        assert_eq!(
            SETTINGS_TABS[3].rows,
            &[
                SettingsRow::EdgesAvoid,
                SettingsRow::PortZone,
                SettingsRow::LinePorts,
                SettingsRow::FocusMode,
                SettingsRow::EdgeAggregation,
                SettingsRow::AutolinkEnabled
            ]
        );
        assert_eq!(
            SETTINGS_TABS[4].rows,
            &[SettingsRow::ThemePreset, SettingsRow::Language]
        );
    }

    /// Инвариант локализации (FR-039 §5): у каждой строки есть ключи
    /// лейбла и описания, значения непусты в обоих языках.
    #[test]
    fn every_row_has_label_and_description() {
        for row in SETTINGS_ROWS {
            for (lang, name) in [(Language::Ru, "RU"), (Language::En, "EN")] {
                assert!(!tr(lang, row_label_key(row)).is_empty(), "{name}: {row:?}");
                assert!(!tr(lang, row_desc_key(row)).is_empty(), "{name}: {row:?}");
            }
        }
        for tab in &SETTINGS_TABS {
            assert!(!tr(Language::Ru, tab.title_key).is_empty());
            assert!(!tr(Language::En, tab.title_key).is_empty());
        }
    }

    /// Инвариант булевых: `Toggle` — ровно для `bool`-полей `Settings`;
    /// многозначные (включая язык FR-040) — `Dropdown`.
    #[test]
    fn row_kind_partitions_bool_and_value_rows() {
        let defaults = Settings::default();
        for row in SETTINGS_ROWS {
            match row {
                SettingsRow::Grid => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.grid_visible;
                }
                SettingsRow::EdgesAvoid => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.edges_avoid_nodes;
                }
                SettingsRow::LinePorts => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.line_ports;
                }
                SettingsRow::BottleneckOverlay => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.bottleneck_overlay;
                }
                SettingsRow::FocusMode => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.focus_mode;
                }
                // FR-042 (F-13): агрегация связей — булево поле Settings
                SettingsRow::EdgeAggregation => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.edge_aggregation;
                }
                // PRD-0007 (X4, AC-5.5): фоновый детектор автосвязи — булево
                SettingsRow::AutolinkEnabled => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.autolink_enabled;
                }
                SettingsRow::HudOnStart => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.hud_on_start;
                }
                // FR-038: тумблеры магнитной раскладки — булевы поля Settings
                SettingsRow::SnapEnabled => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.snap_enabled;
                }
                SettingsRow::SnapGrid => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.snap_to_grid;
                }
                SettingsRow::SnapGuides => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.snap_to_guides;
                }
                SettingsRow::SnapCollision => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.snap_collision;
                }
                SettingsRow::ButtonCorner
                | SettingsRow::GridStyle
                | SettingsRow::GridDensity
                | SettingsRow::PortZone
                | SettingsRow::Language
                | SettingsRow::ThemePreset
                | SettingsRow::ExplainDepthLimit
                | SettingsRow::SnapTolerance
                | SettingsRow::SnapSubZoom
                | SettingsRow::SnapCoarseZoom => {
                    assert_eq!(row_kind(row), RowKind::Dropdown);
                }
            }
        }
    }

    /// FR-047: пресетный dropdown — полный перечень (классика + 7 пресетов),
    /// ровно одна отметка текущего; выбор по индексу согласован с
    /// dropdown_options (порядок apply == порядку опций); сброс на
    /// «Классическую» очищает пресет; карточки Classic Dark/Light
    /// (settings.theme) при активном пресете не отмечены.
    #[test]
    fn theme_preset_options_apply_and_reset() {
        let mut settings = Settings::default();

        // Дефолт: классика отмечена, пресетов в реестре 7
        let options = dropdown_options(SettingsRow::ThemePreset, &settings);
        assert_eq!(options.len(), 1 + theme_presets::PRESETS.len());
        assert_eq!(options.iter().filter(|(_, cur)| *cur).count(), 1);
        assert!(
            options[0].1,
            "классическая отмечена при пустом theme_preset"
        );

        // Выбор пресета по индексу (i-я опция == i-1-й пресет реестра)
        apply_dropdown_value(&mut settings, SettingsRow::ThemePreset, 2);
        assert_eq!(
            settings.theme_preset,
            theme_presets::PRESETS[1].id,
            "индекс 2 == второй пресет реестра"
        );
        assert_eq!(settings.active_preset(), Some(theme_presets::PRESETS[1].id));
        let options = dropdown_options(SettingsRow::ThemePreset, &settings);
        assert_eq!(options.iter().filter(|(_, cur)| *cur).count(), 1);
        assert!(options[2].1, "выбранный пресет отмечен");
        // Значение на контроле — метка пресета (без i18n: имена собственные)
        assert_eq!(
            dropdown_value(SettingsRow::ThemePreset, &settings).as_deref(),
            Some(theme_presets::PRESETS[1].label)
        );

        // Сброс на «Классическую»
        apply_dropdown_value(&mut settings, SettingsRow::ThemePreset, 0);
        assert!(settings.theme_preset.is_empty());
        assert!(
            dropdown_value(SettingsRow::ThemePreset, &settings).is_some(),
            "значение на контроле всегда есть"
        );

        // Некорректный индекс не меняет состояние
        apply_dropdown_value(&mut settings, SettingsRow::ThemePreset, 99);
        assert!(settings.theme_preset.is_empty());

        // Активный пресет гасит отметку классических карточек (апп-инвариант
        // выбора темы: пресет перекрывает theme, см. ThemeColors::from_settings)
        settings.theme_preset = "nord".to_string();
        assert_eq!(settings.active_preset(), Some("nord"));
        settings.theme_preset.clear();
        assert_eq!(settings.active_preset(), None);
        // Неизвестный id из config.toml — мягкая деградация к классике
        settings.theme_preset = "monokai".to_string();
        assert_eq!(settings.active_preset(), None);
    }

    /// Инвариант значений: у каждого dropdown полный перечень значений
    /// (4 угла / 2 вида / 3 плотности / 5 пресетов / 2 языка), текущее
    /// отмечено ровно один раз; у тумблеров список пуст; язык подписан
    /// собственной локалью.
    #[test]
    fn dropdown_options_counts_and_current() {
        let settings = Settings {
            button_corner: Corner::BottomLeft,
            grid_style: GridStyle::Dots,
            grid_density: GridDensity::Sparse,
            port_zone_px: 20.0,
            language: Language::En,
            ..Settings::default()
        };

        let corners = dropdown_options(SettingsRow::ButtonCorner, &settings);
        assert_eq!(corners.len(), 4);
        assert_eq!(corners.iter().filter(|(_, cur)| *cur).count(), 1);
        assert_eq!(
            corners
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("bottom left")
        );

        let styles = dropdown_options(SettingsRow::GridStyle, &settings);
        assert_eq!(styles.len(), 2);
        assert_eq!(
            styles
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("dots")
        );

        let densities = dropdown_options(SettingsRow::GridDensity, &settings);
        assert_eq!(densities.len(), 3);
        assert_eq!(
            densities
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("sparse")
        );

        let zones = dropdown_options(SettingsRow::PortZone, &settings);
        assert_eq!(zones.len(), PORT_ZONE_PRESETS.len());
        assert_eq!(
            zones
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("20 px")
        );
        // Значение между пресетами — отметка на последнем меньшем (цикл)
        let between = Settings {
            port_zone_px: 22.0,
            ..Settings::default()
        };
        let zones = dropdown_options(SettingsRow::PortZone, &between);
        assert_eq!(
            zones
                .iter()
                .find(|(_, cur)| *cur)
                .map(|(label, _)| label.as_str()),
            Some("20 px")
        );

        // Язык — названия в собственной локали независимо от языка UI
        let languages = dropdown_options(SettingsRow::Language, &settings);
        assert_eq!(languages.len(), 2);
        assert_eq!(languages[0].0, "русский");
        assert_eq!(languages[1].0, "English");
        assert_eq!(languages.iter().position(|(_, cur)| *cur), Some(1));

        for row in [
            SettingsRow::Grid,
            SettingsRow::EdgesAvoid,
            SettingsRow::FocusMode,
            SettingsRow::HudOnStart,
        ] {
            assert!(
                dropdown_options(row, &settings).is_empty(),
                "{row:?}: тумблер без меню"
            );
            assert!(
                dropdown_value(row, &settings).is_none(),
                "{row:?}: тумблер без значения на контроле"
            );
        }
        // Значение на контроле dropdown-строк
        assert_eq!(
            dropdown_value(SettingsRow::Language, &settings).as_deref(),
            Some("English")
        );
        assert_eq!(
            dropdown_value(SettingsRow::PortZone, &settings).as_deref(),
            Some("20 px")
        );
    }

    /// Инвариант эквивалентности: выбор пункта `i` == k нажатий цикла
    /// (значение и подпись совпадают с `.next()`/`next_port_zone`);
    /// язык — включён в проверку (2 значения, цикл `Language::next`).
    #[test]
    fn dropdown_choice_matches_value_cycle() {
        /// (строка, цикл значений, число значений) — сценарий эквивалентности.
        type CycleCase = (SettingsRow, fn(&mut Settings, usize), usize);
        let cases: [CycleCase; 5] = [
            (
                SettingsRow::ButtonCorner,
                |s, k| {
                    for _ in 0..k {
                        s.button_corner = s.button_corner.next();
                    }
                },
                4,
            ),
            (
                SettingsRow::GridStyle,
                |s, k| {
                    for _ in 0..k {
                        s.grid_style = s.grid_style.next();
                    }
                },
                2,
            ),
            (
                SettingsRow::GridDensity,
                |s, k| {
                    for _ in 0..k {
                        s.grid_density = s.grid_density.next();
                    }
                },
                3,
            ),
            (
                SettingsRow::PortZone,
                |s, k| {
                    for _ in 0..k {
                        s.port_zone_px = canvas_core::next_port_zone(s.port_zone_px);
                    }
                },
                PORT_ZONE_PRESETS.len(),
            ),
            (
                SettingsRow::Language,
                |s, k| {
                    for _ in 0..k {
                        s.language = s.language.next();
                    }
                },
                2,
            ),
        ];
        for (row, cycle, count) in cases {
            for start in 0..count {
                // Текущее значение = start нажатий цикла от дефолта; его
                // индекс в перечне — по отметке dropdown_options
                let mut base = Settings::default();
                cycle(&mut base, start);
                let current_index = dropdown_options(row, &base)
                    .iter()
                    .position(|(_, current)| *current)
                    .expect("текущее значение отмечено");
                for chosen in 0..count {
                    let mut expected = base.clone();
                    cycle(&mut expected, (chosen + count - current_index) % count);
                    let mut applied = base.clone();
                    apply_dropdown_value(&mut applied, row, chosen);
                    assert_eq!(
                        applied, expected,
                        "{row:?}: current={current_index}, chosen={chosen}"
                    );
                }
            }
        }
    }

    /// Выбор вне диапазона — без изменений; выбор текущего — значение то же.
    #[test]
    fn apply_dropdown_value_out_of_range_is_noop() {
        let mut settings = Settings::default();
        let before = settings.clone();
        apply_dropdown_value(&mut settings, SettingsRow::ButtonCorner, 4);
        apply_dropdown_value(&mut settings, SettingsRow::GridStyle, 99);
        apply_dropdown_value(&mut settings, SettingsRow::GridDensity, usize::MAX);
        apply_dropdown_value(&mut settings, SettingsRow::PortZone, 5);
        apply_dropdown_value(&mut settings, SettingsRow::Language, 2);
        assert_eq!(settings, before);
    }

    /// Инвариант адаптива (FR-039): на Full HD модалка ~864×640, на
    /// 1280×720 — в границах MIN/MAX, на 320×240 — целиком внутри окна;
    /// модалка центрирована на больших окнах.
    #[test]
    fn modal_layout_adaptive_and_clamped() {
        // Full HD: 0.45*1920 = 864, 0.6*1080 = 648 → потолок 640
        let layout = modal_layout(0, [1920.0, 1080.0]);
        assert_eq!(
            layout.rect,
            [(1920.0 - 864.0) / 2.0, (1080.0 - 640.0) / 2.0, 864.0, 640.0]
        );
        assert_eq!(layout.nav_w, MODAL_NAV_WIDTH);
        // 1280×720: в границах MIN/MAX
        let layout = modal_layout(0, [1280.0, 720.0]);
        assert!(layout.rect[2] >= MODAL_MIN_W && layout.rect[2] <= MODAL_MAX_W);
        assert!(layout.rect[3] >= MODAL_MIN_H && layout.rect[3] <= MODAL_MAX_H);
        // 320×240: целиком внутри окна
        let layout = modal_layout(0, [320.0, 240.0]);
        assert!(layout.rect[0] >= SETTINGS_MARGIN - 0.01);
        assert!(layout.rect[1] >= SETTINGS_MARGIN - 0.01);
        assert!(layout.rect[0] + layout.rect[2] <= 320.0 - SETTINGS_MARGIN + 0.01);
        assert!(layout.rect[1] + layout.rect[3] <= 240.0 - SETTINGS_MARGIN + 0.01);
        assert!(
            layout.nav_w > 0.0 && layout.nav_w < layout.rect[2],
            "навигация различима"
        );
        // Вне диапазона таб — первый таб
        let fallback = modal_layout(99, [1920.0, 1080.0]);
        assert_eq!(fallback.rows, modal_layout(0, [1920.0, 1080.0]).rows);
    }

    /// Структура модалки: 4 пункта навигации; строки — только активного
    /// таба; карточки темы только у «Внешнего вида»; hit-тесты навигации,
    /// строк и карточек согласованы с layout.
    #[test]
    fn modal_layout_structure_and_hit_tests() {
        let viewport = [1600.0, 900.0];
        // Таб 0 (Общие): 2 строки, карточек нет
        let layout = modal_layout(0, viewport);
        assert_eq!(layout.nav_items.len(), SETTINGS_TABS.len());
        assert_eq!(layout.rows.len(), 2);
        assert!(layout
            .rows
            .iter()
            .all(|(row, _)| { SETTINGS_TABS[0].rows.contains(row) }));
        assert_eq!(layout.theme_cards, [[0.0; 4]; 2]);
        assert_eq!(
            modal_theme_card_at(&layout, [layout.rect[0] + 10.0, layout.rect[1] + 10.0]),
            None
        );
        // Навигация: клик по пункту 2 — Some(2), клик в контент — None
        let item2 = layout.nav_items[2];
        assert_eq!(
            modal_nav_at(&layout, [item2[0] + 5.0, item2[1] + 5.0]),
            Some(2)
        );
        let inside_content = [layout.content_rect[0] + 5.0, layout.content_rect[1] + 5.0];
        assert_eq!(modal_nav_at(&layout, inside_content), None);
        // Строки: клик по первой строке таба; клик в шапку/подсказку — None
        let (first_row, first_rect) = layout.rows[0];
        assert_eq!(
            modal_row_at(&layout, [first_rect[0] + 10.0, first_rect[1] + 5.0]),
            Some(first_row)
        );
        assert_eq!(
            modal_row_at(
                &layout,
                [layout.title_rect[0] + 10.0, layout.title_rect[1] + 5.0]
            ),
            None
        );
        assert_eq!(
            modal_row_at(
                &layout,
                [layout.hint_rect[0] + 5.0, layout.hint_rect[1] + 5.0]
            ),
            None
        );
        assert_eq!(
            modal_nav_at(
                &layout,
                [layout.hint_rect[0] + 5.0, layout.hint_rect[1] + 5.0]
            ),
            None
        );
        // Таб 2 (Snap, FR-038): 4 тумблера + 3 dropdown-пресета
        let layout = modal_layout(2, viewport);
        assert_eq!(layout.rows.len(), 7);
        assert_eq!(layout.rows[0].0, SettingsRow::SnapEnabled);
        assert_eq!(
            modal_nav_at(
                &layout,
                [layout.hint_rect[0] + 5.0, layout.hint_rect[1] + 5.0]
            ),
            None
        );
        // Таб 4 (Внешний вид): карточки темы + строки пресета и языка ниже
        let layout = modal_layout(4, viewport);
        assert_eq!(layout.rows.len(), 2);
        assert_eq!(layout.rows[0].0, SettingsRow::ThemePreset);
        assert_eq!(layout.rows[1].0, SettingsRow::Language);
        let card_dark = layout.theme_card_rect(Theme::Dark);
        let card_light = layout.theme_card_rect(Theme::Light);
        assert!(card_dark[2] > 0.0 && card_light[2] > 0.0);
        assert!(card_light[0] > card_dark[0], "карточки рядом");
        assert_eq!(
            modal_theme_card_at(&layout, [card_dark[0] + 5.0, card_dark[1] + 5.0]),
            Some(Theme::Dark)
        );
        assert_eq!(
            modal_theme_card_at(&layout, [card_light[0] + 5.0, card_light[1] + 5.0]),
            Some(Theme::Light)
        );
        // Строка языка — ниже карточек (не перекрываются)
        let lang_rect = layout
            .row_rect(SettingsRow::Language)
            .expect("строка языка");
        assert!(lang_rect[1] >= card_dark[1] + card_dark[3]);
        // row_rect согласован с modal_row_at для каждого таба
        for tab in 0..SETTINGS_TABS.len() {
            let layout = modal_layout(tab, viewport);
            for (row, rect) in &layout.rows {
                assert_eq!(
                    modal_row_at(&layout, [rect[0] + 10.0, rect[1] + 5.0]),
                    Some(*row),
                    "таб {tab}, строка {row:?}"
                );
            }
        }
    }

    /// Контролы в строках: pill-тумблер у тумблера, dropdown-кнопка у
    /// выпадающего списка; ручка pill отражает состояние (вкл — справа).
    #[test]
    fn control_rects_follow_row_kind() {
        for row in SETTINGS_ROWS {
            let layout = modal_layout(0, [1600.0, 900.0]);
            let Some(rect) = layout.row_rect(row) else {
                continue;
            };
            let control = control_rect(rect, row_kind(row));
            assert!(control[0] >= rect[0] && control[0] + control[2] <= rect[0] + rect[2] + 0.01);
            assert!(control[1] >= rect[1] && control[1] + control[3] <= rect[1] + rect[3] + 0.01);
            match row_kind(row) {
                RowKind::Toggle => assert_eq!(control[2], PILL_TRACK_W),
                RowKind::Dropdown => assert_eq!(control[2], DROPDOWN_BTN_W),
            }
        }
        // Ручка pill: выкл — слева, вкл — справа
        let track = [100.0, 10.0, PILL_TRACK_W, PILL_TRACK_H];
        let off = pill_knob_rect(track, false);
        let on = pill_knob_rect(track, true);
        assert!(off[0] < on[0]);
        assert_eq!(on[0] + on[2], track[0] + track[2] - 2.0);
    }

    /// `dropdown_layout` с параметрической шириной: кламп к окну у правого
    /// края модалки и в узком окне; у нижнего края — выше контрола; 0
    /// пунктов — пусто; меню не перекрывает якорную строку целиком.
    #[test]
    fn dropdown_layout_clamps_to_window() {
        assert_eq!(
            dropdown_layout([0.0, 0.0, 100.0, 28.0], [800.0, 600.0], 0, DROPDOWN_BTN_W),
            [0.0; 4]
        );
        let viewport = [1600.0, 900.0];
        for tab in 0..SETTINGS_TABS.len() {
            let layout = modal_layout(tab, viewport);
            for row in SETTINGS_ROWS {
                let Some(row_rect) = layout.row_rect(row) else {
                    continue;
                };
                if row_kind(row) != RowKind::Dropdown {
                    continue;
                }
                let anchor = control_rect(row_rect, RowKind::Dropdown);
                let menu = dropdown_layout(anchor, viewport, 5, DROPDOWN_BTN_W);
                assert!(menu[0] >= DROPDOWN_MARGIN - 0.01, "таб {tab} {row:?}");
                assert!(
                    menu[0] + menu[2] <= viewport[0] - DROPDOWN_MARGIN + 0.01,
                    "таб {tab} {row:?}"
                );
                assert!(menu[1] >= DROPDOWN_MARGIN - 0.01, "таб {tab} {row:?}");
                assert!(
                    menu[1] + menu[3] <= viewport[1] - DROPDOWN_MARGIN + 0.01,
                    "таб {tab} {row:?}"
                );
                assert_eq!(menu[2], DROPDOWN_BTN_W);
                // Не перекрывает якорную строку целиком: при открытии вниз
                // верх строки остаётся виден (контрол внутри строки)
                if menu[1] >= anchor[1] + anchor[3] {
                    assert!(menu[1] > row_rect[1], "таб {tab} {row:?}");
                }
            }
        }
        // Узкое окно (320×240): меню клампится внутрь, ширина <= окна
        let narrow = [320.0, 240.0];
        let menu = dropdown_layout([8.0, 100.0, 300.0, 28.0], narrow, 5, DROPDOWN_BTN_W);
        assert!(
            menu[0] >= DROPDOWN_MARGIN - 0.01
                && menu[0] + menu[2] <= 320.0 - DROPDOWN_MARGIN + 0.01
        );
        assert!(
            menu[1] >= DROPDOWN_MARGIN - 0.01
                && menu[1] + menu[3] <= 240.0 - DROPDOWN_MARGIN + 0.01
        );
        // Ширина меньше потолка — ширина якоря; гигантская — кламп к окну
        let menu = dropdown_layout([50.0, 20.0, 120.0, 28.0], [800.0, 600.0], 3, 100.0);
        assert_eq!(menu[2], 100.0);
        let menu = dropdown_layout([50.0, 20.0, 120.0, 28.0], [800.0, 600.0], 3, 5000.0);
        assert!(menu[2] <= 800.0 - DROPDOWN_MARGIN * 2.0);
        // У нижнего края — меню выше контрола
        let menu = dropdown_layout([50.0, 200.0, 300.0, 28.0], narrow, 5, DROPDOWN_BTN_W);
        assert!(menu[1] + menu[3] <= 200.0, "меню выше якорного контрола");
        // Меню ниже контрола, когда влезает
        let menu = dropdown_layout([50.0, 20.0, 300.0, 28.0], [800.0, 600.0], 3, DROPDOWN_BTN_W);
        assert!(menu[1] >= 20.0 + 28.0, "меню ниже якорного контрола");
    }

    /// Hit-тест пунктов меню: пункты 0/средний/последний, паддинги и мимо
    /// меню — None.
    #[test]
    fn dropdown_item_hit_tests() {
        let count = 5;
        let menu = dropdown_layout(
            [100.0, 100.0, 300.0, 28.0],
            [1600.0, 900.0],
            count,
            DROPDOWN_BTN_W,
        );
        let height = count as f32 * DROPDOWN_ROW_H + DROPDOWN_MARGIN * 2.0;
        assert!((menu[3] - height).abs() < 0.01);
        // Пункт 0 и последний
        assert_eq!(
            dropdown_item_at(
                menu,
                count,
                [menu[0] + 30.0, menu[1] + DROPDOWN_MARGIN + 3.0]
            ),
            Some(0)
        );
        assert_eq!(
            dropdown_item_at(
                menu,
                count,
                [
                    menu[0] + 30.0,
                    menu[1] + DROPDOWN_MARGIN + 4.0 * DROPDOWN_ROW_H + 3.0
                ]
            ),
            Some(4)
        );
        // Паддинг сверху и мимо меню — None
        assert_eq!(
            dropdown_item_at(menu, count, [menu[0] + 30.0, menu[1] + 1.0]),
            None
        );
        assert_eq!(
            dropdown_item_at(menu, count, [menu[0] + 30.0, menu[1] + menu[3] + 5.0]),
            None
        );
        assert_eq!(
            dropdown_item_at(menu, count, [menu[0] + 500.0, menu[1] + 20.0]),
            None
        );
    }

    /// Модель состояния меню: открытие ставит выделение на текущее значение,
    /// сдвиг закольцован, reset закрывает; язык открывается на «русский».
    #[test]
    fn dropdown_state_model() {
        let settings = Settings {
            button_corner: Corner::BottomRight,
            ..Settings::default()
        };
        let mut state = DropdownState::default();
        assert!(!state.is_open());
        state.open(SettingsRow::ButtonCorner, &settings);
        assert!(state.is_open());
        assert_eq!(state.open_row, Some(SettingsRow::ButtonCorner));
        // BottomRight — третий пункт цикла (TopLeft, TopRight, BottomRight, …)
        assert_eq!(state.selected, 2);
        assert!(state.move_selection(1, 4));
        assert_eq!(state.selected, 3);
        assert!(state.move_selection(1, 4), "закольцовывание вперёд");
        assert_eq!(state.selected, 0);
        assert!(state.move_selection(-1, 4), "закольцовывание назад");
        assert_eq!(state.selected, 3);
        state.reset();
        assert!(!state.is_open());
        assert_eq!(state.selected, 0);
        assert!(!state.move_selection(1, 0), "пустой список — сдвига нет");
        // Язык: дефолт Ru — выделение на пункте 0
        let mut state = DropdownState::default();
        state.open(SettingsRow::Language, &Settings::default());
        assert_eq!(state.selected, 0);
    }
}
