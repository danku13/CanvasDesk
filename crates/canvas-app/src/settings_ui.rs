//! FR-039: модалка настроек в стиле Obsidian — чистая модель
//! (образец [`crate::hints_ui`]/`template_ui`): табы ([`SETTINGS_TABS`]),
//! род строки ([`row_kind`]), перечень значений многозначной настройки
//! ([`dropdown_options`]) с чистым применением выбора
//! ([`apply_dropdown_value`]), геометрия модалки ([`modal_layout`], прокрутка
//! контента W-a — [`modal_layout_scrolled`]) и hit-тесты.
//!
//! Волна W-c (аудит ui-kit §10): контролы строк — примитивы кита
//! `canvas-ui`. Тумблер — kit `switch` (геометрия трека/бегунка —
//! [`control_rect`]/[`pill_knob_rect`], слоты стиля — `track_style`/`knob_fill`
//! кита); выпадающее меню — kit `dropdown_menu` («якорь + flip», кламп к
//! окну — [`dropdown_layout`]); логика опций/применения и hit-поведение
//! строк не тронуты.
//!
//! Отличия от панели FR-026 (ревизия владельцем 2026-09-20): модалка по
//! центру окна над затемнением (не панель у угла кнопки); строки
//! «лейбл + описание + контрол» (kit-switch / dropdown-кнопка);
//! таб «Внешний вид» — карточки темы + dropdown языка (FR-040); размер
//! адаптивный с потолками (расчёт на Full HD+); брейкпоинты ширины
//! [`MODAL_BP_COMPACT`]/[`MODAL_BP_MOBILE`] реализованы (вариант «A»,
//! решение владельца 03.10.2026, волна W-e аудита ui-kit §10):
//! [`modal_mode`] → [`ModalMode`] — Desktop (≥1280: двухколоночная
//! раскладка прежняя), Compact (768..1280: одноколоночная, горизонтальный
//! таб-бар, ширина клампится к вьюпорту минус поля), Mobile (<768:
//! полноэкранный лист вьюпорт минус внешние поля). Скролл контента
//! (W-a) работает во всех трёх режимах; поиск по настройкам — отклонён
//! владельцем (не реализуем).
//!
//! Все тексты настроек — через таблицу строк [`crate::i18n`] (ключи,
//! без хардкода — база локализации FR-040); значения из `canvas-core`
//! (`Corner::label` и др.) в рендер не идут. Схема `config.toml` не
//! меняется — это реорганизация UI.

use canvas_core::{
    theme_presets, Corner, GridDensity, GridStyle, IconStyle, Language, Settings, Theme,
    DRAG_PUSH_GAP_PRESETS, DRAG_PUSH_HALO_PRESETS, PORT_ZONE_PRESETS, SNAP_COARSE_ZOOM_PRESETS,
    SNAP_SUB_ZOOM_PRESETS, SNAP_TOLERANCE_PRESETS,
};

use crate::i18n::{self, keys};
use crate::ui::{point_in_rect, SETTINGS_MARGIN};

/// Высота пункта выпадающего меню — kit-метрика строки списка
/// (`canvas_ui::kit::LIST_ROW_H`, значение прежнего литерала 26).
pub const DROPDOWN_ROW_H: f32 = canvas_ui::kit::LIST_ROW_H;
/// Внутренние поля выпадающего меню — токен
/// `canvas_core::tokens::SPACING_S` (значение прежнего литерала 6).
pub const DROPDOWN_MARGIN: f32 = canvas_core::tokens::SPACING_S;

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
// Брейкпоинты ширины (W-e, вариант «A», решение владельца 03.10.2026)
// объявлены В РЕЕСТРЕ поверхностей — LAY8.2 (`design/rules/11-layouts.md`):
// «брейкпоинт зарегистрирован там же, где рисуется поверхность», т.е. рядом
// с декларацией SETTINGS (app/ui_registry.rs), а не в теле отрисовки.
// Здесь — ре-экспорт под прежними именами: доки, тесты и
// [`modal_mode`] собираются без правки тел.
pub use crate::app::ui_registry::{
    SETTINGS_BP_COMPACT as MODAL_BP_COMPACT, SETTINGS_BP_MOBILE as MODAL_BP_MOBILE,
};
/// Ширина левой колонки навигации (клампится на узких окнах).
pub const MODAL_NAV_WIDTH: f32 = 180.0;
/// Высота пункта левой навигации.
pub const MODAL_NAV_ITEM_H: f32 = 34.0;
/// Высота горизонтального таб-бара компакт/мобайл-режимов (W-e) — та же
/// kit-метрика пункта, что у вертикальной навигации ([`MODAL_NAV_ITEM_H`]):
/// слоты крупнее иконки 16px, «таб-бар компактен» без нового литерала.
pub const MODAL_TABBAR_H: f32 = MODAL_NAV_ITEM_H;
/// Высота заголовка раздела в правой панели.
pub const MODAL_TITLE_HEIGHT: f32 = 30.0;
/// Внутренние поля модалки и её панелей — токен
/// `canvas_core::tokens::SPACING_LG` (значение прежнего литерала 12).
pub const MODAL_PADDING: f32 = canvas_core::tokens::SPACING_LG;
/// Высота строки настройки (лейбл + описание в 2 строки, контрол справа).
/// 44 → 52 (wasm-аудит 2026-09-25): однострочное описание при ширине
/// «строка − контрол» рвалось у кромки dropdown'а («…прижата лета|»);
/// теперь описание переносится на 2 строки тем же 11px кеглем.
pub const MODAL_ROW_HEIGHT: f32 = 52.0;
/// Высота карточки темы (таб «Внешний вид»).
pub const MODAL_THEME_CARD_H: f32 = 56.0;
/// Зазор между карточками темы и следующей секцией — 14 px, вне шкалы S1
/// (исключение LAY7 — «Исключения» 11-layouts.md, «Пады/маргины 14–16 px»,
/// LAY-W16: до 24 — удвоение воздуха модалки, до 12 — сжатие устоявшейся
/// плотности; кандидата миграции нет).
pub const MODAL_THEME_GAP: f32 = 14.0;
/// Зазор заголовок/контент модалки — 4 px hairline, вне шкалы S1 (исключение
/// LAY7 — «Исключения» 11-layouts.md, «Hairline-микрозначения 2–4 px»,
/// LAY-W16; прежний inline-литерал `Fixed { h: 4.0 }`, значение бит-в-бит).
pub const MODAL_TITLE_CONTENT_GAP: f32 = 4.0;
/// Высота строки-подсказки внизу левой колонки.
pub const MODAL_HINT_HEIGHT: f32 = 24.0;
// W-c: метрики тумблера — kit (`canvas_ui::kit::SWITCH_W`/`SWITCH_H`/
// `SWITCH_KNOB_PAD`), прежние константы PILL_* удалены; см. [`control_rect`].
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
    /// PRD-0007 (FR-048 X6, F-12): индикатор покрытия цепочками «Цепочки:
    /// N%» в углу канваса — opt-in тумблер в табе «Канвас».
    ExplainCoverage,

    /// FR-079 (S3): мастер-тумблер ИИ-подсказок (попап C1 + карточки C3)
    /// — таб «Подсказки».
    SuggestEnabled,
    /// FR-079 (S3): движок подсказок (off/lex/lex+laya) — dropdown.
    SuggestEngine,

    /// FR-073: мастер-тумблер расталкивания при драге.
    DragPushEnabled,
    /// FR-073: сейф-зазор между нодами — цикл по пресетам.
    DragPushSafeGap,
    /// FR-073: ореол активной ноды — цикл по пресетам.
    DragPushHalo,
    /// FR-073: предиктивное упреждение ореола.
    DragPushPredictive,
    /// FR-073: перезакрепление якорей накрытых нод на drop.
    DragPushRebase,
    /// FR-ICONS: набор иконок UI (Glyph/Lucide/Material/Feather/Bootstrap) —
    /// dropdown в табе «Внешний вид».
    IconStyle,

    /// FR-087: рабочая роль (class) — dropdown реестра
    /// `canvas_core::roles::ROLES` в табе «Профиль».
    Role,
    /// FR-089: согласие на анонимный счётчик использования (web) —
    /// тумблер в табе «Профиль», зеркало предвыбранного чекбокса
    /// первого запуска. На нативе поле персистится, отправки нет.
    TelemetryCounter,
    /// FR-089: согласие на продуктовые метрики и отчёты об ошибках
    /// (PostHog, web) — тумблер в табе «Профиль».
    TelemetryAnalytics,
    /// FR-087: тумблеры вывода типов шаблонных нод (палитра/wheel).
    TplCatBackend,
    TplCatNetwork,
    TplCatUnitEconomics,
    TplCatProductAnalytics,
    /// FR-087: тумблеры вывода типов шаблонных схем (галерея Ctrl+T).
    SchemeCatArchitecture,
    SchemeCatBusiness,
    SchemeCatFramework,
    SchemeCatPlanning,
    SchemeCatOnboarding,

    // FR-LLM-B / PRD-0010 F-7 (Q2): per-feature провайдеры для 9-го таба
    // «AI и модели». Dropdown-строки (RowKind::Dropdown), опции —
    // LlmProviderId для соответствующей фичи (Suggest без ChatGPT OAuth).
    /// Suggest — LLM-подсказки шаблонов и custom-ноды.
    AiProvSuggest,
    /// Graph Builder — генерация графов из текста.
    AiProvGraph,
    /// Agent Panel — агентные операции через MCP.
    AiProvAgent,
    // FR-LLM-FIX: per-feature BYOK-модель (3 dropdown'а вместо одного).
    // Каждый показывается только когда соответствующий провайдер = BYOK
    // (см. `ai_settings_extra_overlay` / рендер таба «AI и модели»).
    /// BYOK-модель для Suggest (dropdown из списка /v1/models после health-check).
    AiModelSuggest,
    /// BYOK-модель для Graph Builder.
    AiModelGraph,
    /// BYOK-модель для Agent Panel.
    AiModelAgent,
    // FR-LLM-OAUTH-APP / PRD-0010 F-5.8: строка «Вход ChatGPT» (OAuth-флоу).
    // Видна только когда provider_graph или provider_agent == ChatGptOAuth;
    // род — RowKind::Button (кнопка действия справа + бейдж состояния), клик
    // по кнопке — apply_button_row → App::oauth_button_click (dispatch по
    // состоянию OAuthUiState: Войти/Отменить/Выйти/Повторить).
    AiOAuth,
    // FR-LLM-FIX: API-ключ BYOK + self-hosted endpoint (отдельные текстовые
    // строки-поля, не dropdown). Кнопка «Проверить» делает mock health-check
    // и переключает бейдж (реальный health-check — Stream C/D TODO).
    /// BYOK API-ключ (password-поле + кнопка «Проверить ключ» + бейдж).
    /// Видна только когда ANY per-feature провайдер = BYOK.
    AiApiKey,
    /// URL self-hosted endpoint (текстовое поле + кнопка «Проверить» + бейдж).
    /// Видна только когда data_residency = SelfHosted.
    AiSelfhostUrl,
    /// API-ключ self-hosted endpoint (текстовое поле). Видна только когда
    /// data_residency = SelfHosted.
    AiSelfhostKey,
    /// Data residency — radio Local/Cloud/SelfHosted (dropdown-цикл).
    AiResidency,
    /// Confidence threshold (dropdown-цикл по пресетам 0..1).
    AiConfidenceThreshold,
    /// Cost limit (dropdown-цикл по пресетам $0.25..$5.00).
    AiCostLimit,
    /// Telemetry opt-in (тумблер).
    AiTelemetry,
}

/// Плоский список всех строк настроек (инвариант полноты: union строк
/// табов == этот список без дублей). Тема — вне списка (карточки,
/// отдельное поле `settings.theme`).
pub const SETTINGS_ROWS: [SettingsRow; 57] = [
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
    SettingsRow::ExplainCoverage,
    SettingsRow::DragPushEnabled,
    SettingsRow::DragPushSafeGap,
    SettingsRow::DragPushHalo,
    SettingsRow::DragPushPredictive,
    SettingsRow::DragPushRebase,
    SettingsRow::IconStyle,
    // FR-079 (S3): таб «Подсказки»
    SettingsRow::SuggestEnabled,
    SettingsRow::SuggestEngine,
    // FR-087: таб «Профиль» — роль + фильтры категорий подсказок
    SettingsRow::Role,
    // FR-089: согласия телеметрии (сразу за ролью — видны без прокрутки)
    SettingsRow::TelemetryCounter,
    SettingsRow::TelemetryAnalytics,
    SettingsRow::TplCatBackend,
    SettingsRow::TplCatNetwork,
    SettingsRow::TplCatUnitEconomics,
    SettingsRow::TplCatProductAnalytics,
    SettingsRow::SchemeCatArchitecture,
    SettingsRow::SchemeCatBusiness,
    SettingsRow::SchemeCatFramework,
    SettingsRow::SchemeCatPlanning,
    SettingsRow::SchemeCatOnboarding,
    // FR-LLM-B / PRD-0010 F-7: 9-й таб «AI и модели»
    SettingsRow::AiProvSuggest,
    SettingsRow::AiProvGraph,
    SettingsRow::AiProvAgent,
    SettingsRow::AiModelSuggest,
    SettingsRow::AiModelGraph,
    SettingsRow::AiModelAgent,
    // FR-LLM-OAUTH-APP: строка «Вход ChatGPT» (видна при ChatGptOAuth в
    // graph/agent — см. ai_tab_visible_rows).
    SettingsRow::AiOAuth,
    // FR-LLM-FIX: API-ключ BYOK + self-hosted endpoint.
    SettingsRow::AiApiKey,
    SettingsRow::AiSelfhostUrl,
    SettingsRow::AiSelfhostKey,
    SettingsRow::AiResidency,
    SettingsRow::AiConfidenceThreshold,
    SettingsRow::AiCostLimit,
    SettingsRow::AiTelemetry,
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
pub const SETTINGS_TABS: [SettingsTab; 9] = [
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
            // PRD-0007 (FR-048 X6, F-12): индикатор покрытия цепочками
            // (opt-in — решение владельца, раунд 2 (в)).
            SettingsRow::ExplainCoverage,
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
        // FR-073: поведение драга нод — расталкивание и его параметры
        title_key: keys::TAB_DRAG,
        icon: "✥",
        theme_cards: false,
        rows: &[
            SettingsRow::DragPushEnabled,
            SettingsRow::DragPushSafeGap,
            SettingsRow::DragPushHalo,
            SettingsRow::DragPushPredictive,
            SettingsRow::DragPushRebase,
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
        // FR-079 (S3): ИИ-карточки шаблонов (master-тумблер + движок).
        // CR-022: попап при вводе ИИ-строки не показывает (c1_in_popup
        // default false) — тумблер управляет C3-карточками «что дальше»
        title_key: keys::TAB_SUGGEST,
        icon: "✦",
        theme_cards: false,
        rows: &[SettingsRow::SuggestEnabled, SettingsRow::SuggestEngine],
    },
    SettingsTab {
        // FR-087: роль + фильтры типов шаблонных нод и схем; FR-089 —
        // согласия телеметрии сразу за ролью
        title_key: keys::TAB_PROFILE,
        icon: "◉",
        theme_cards: false,
        rows: &[
            SettingsRow::Role,
            SettingsRow::TelemetryCounter,
            SettingsRow::TelemetryAnalytics,
            SettingsRow::TplCatBackend,
            SettingsRow::TplCatNetwork,
            SettingsRow::TplCatUnitEconomics,
            SettingsRow::TplCatProductAnalytics,
            SettingsRow::SchemeCatArchitecture,
            SettingsRow::SchemeCatBusiness,
            SettingsRow::SchemeCatFramework,
            SettingsRow::SchemeCatPlanning,
            SettingsRow::SchemeCatOnboarding,
        ],
    },
    SettingsTab {
        title_key: keys::TAB_APPEARANCE,
        icon: "◐",
        theme_cards: true,
        rows: &[
            SettingsRow::ThemePreset,
            SettingsRow::IconStyle,
            SettingsRow::Language,
        ],
    },
    SettingsTab {
        // FR-LLM-B / PRD-0010 F-7: 9-й таб «AI и модели» — per-feature
        // провайдеры, BYOK-модель (per-feature), лимиты, data residency,
        // телеметрия. FR-LLM-FIX: 3 модели (per-feature) вместо одной;
        // + строка API-ключа BYOK (видна при ANY BYOK) + self-hosted
        // endpoint (виден при SelfHosted). Строки ниже — упорядочены по
        // секциям прототипа (Q2 → Q4/Q7 → Q5 → Q7 telemetry).
        title_key: keys::TAB_AI,
        icon: "✦",
        theme_cards: false,
        rows: &[
            // §1 Per-feature провайдер (Q2) + BYOK-модель (per-feature)
            SettingsRow::AiProvSuggest,
            SettingsRow::AiModelSuggest,
            SettingsRow::AiProvGraph,
            SettingsRow::AiModelGraph,
            SettingsRow::AiProvAgent,
            SettingsRow::AiModelAgent,
            // FR-LLM-OAUTH-APP: «Вход ChatGPT» — бейдж состояния + кнопка
            // «Войти/Отменить/Выйти/Повторить» (видна при ChatGptOAuth в
            // graph/agent — см. ai_tab_visible_rows).
            SettingsRow::AiOAuth,
            // FR-LLM-FIX: §2 BYOK — API ключ и модель (видно при ANY BYOK)
            SettingsRow::AiApiKey,
            // §4 Лимиты и качество (Q7)
            SettingsRow::AiCostLimit,
            SettingsRow::AiConfidenceThreshold,
            // §5 Data residency (Q5)
            SettingsRow::AiResidency,
            // FR-LLM-FIX: §5a Self-hosted endpoint (видно при SelfHosted)
            SettingsRow::AiSelfhostUrl,
            SettingsRow::AiSelfhostKey,
            // §6 Телеметрия (opt-in, Q7)
            SettingsRow::AiTelemetry,
        ],
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
        // FR-079 (S3): таб «Подсказки»
        SettingsRow::SuggestEnabled => keys::ROW_SUGGEST_ENABLED,
        SettingsRow::SuggestEngine => keys::ROW_SUGGEST_ENGINE,
        SettingsRow::ExplainCoverage => keys::ROW_EXPLAIN_COVERAGE,
        SettingsRow::DragPushEnabled => keys::ROW_DRAG_PUSH_ENABLED,
        SettingsRow::DragPushSafeGap => keys::ROW_DRAG_PUSH_GAP,
        SettingsRow::DragPushHalo => keys::ROW_DRAG_PUSH_HALO,
        SettingsRow::DragPushPredictive => keys::ROW_DRAG_PUSH_PREDICTIVE,
        SettingsRow::DragPushRebase => keys::ROW_DRAG_PUSH_REBASE,
        SettingsRow::IconStyle => keys::ROW_ICON_STYLE,
        // FR-087: таб «Профиль» — роль и фильтры подсказок; FR-089 — согласия
        SettingsRow::Role => keys::ROW_ROLE,
        SettingsRow::TelemetryCounter => keys::ROW_TELEMETRY_COUNTER,
        SettingsRow::TelemetryAnalytics => keys::ROW_TELEMETRY_ANALYTICS,
        SettingsRow::TplCatBackend => keys::ROW_TPLCAT_BACKEND,
        SettingsRow::TplCatNetwork => keys::ROW_TPLCAT_NETWORK,
        SettingsRow::TplCatUnitEconomics => keys::ROW_TPLCAT_UNIT_ECONOMICS,
        SettingsRow::TplCatProductAnalytics => keys::ROW_TPLCAT_PRODUCT_ANALYTICS,
        SettingsRow::SchemeCatArchitecture => keys::ROW_SCHEMECAT_ARCHITECTURE,
        SettingsRow::SchemeCatBusiness => keys::ROW_SCHEMECAT_BUSINESS,
        SettingsRow::SchemeCatFramework => keys::ROW_SCHEMECAT_FRAMEWORK,
        SettingsRow::SchemeCatPlanning => keys::ROW_SCHEMECAT_PLANNING,
        SettingsRow::SchemeCatOnboarding => keys::ROW_SCHEMECAT_ONBOARDING,
        // FR-LLM-B / PRD-0010 F-7: таб «AI и модели» (Q2+Q5+Q7)
        SettingsRow::AiProvSuggest => keys::AI_ROW_PROV_SUGGEST,
        SettingsRow::AiProvGraph => keys::AI_ROW_PROV_GRAPH,
        SettingsRow::AiProvAgent => keys::AI_ROW_PROV_AGENT,
        SettingsRow::AiModelSuggest => keys::AI_ROW_MODEL_SUGGEST,
        SettingsRow::AiModelGraph => keys::AI_ROW_MODEL_GRAPH,
        SettingsRow::AiModelAgent => keys::AI_ROW_MODEL_AGENT,
        // FR-LLM-OAUTH-APP: строка «Вход ChatGPT».
        SettingsRow::AiOAuth => keys::AI_OAUTH_ROW,
        // FR-LLM-FIX: API-ключ BYOK + self-hosted endpoint.
        SettingsRow::AiApiKey => keys::AI_ROW_API_KEY,
        SettingsRow::AiSelfhostUrl => keys::AI_ROW_SELFHOST_URL,
        SettingsRow::AiSelfhostKey => keys::AI_ROW_SELFHOST_KEY,
        SettingsRow::AiResidency => keys::AI_GRP_RESIDENCY,
        SettingsRow::AiConfidenceThreshold => keys::AI_CONF_THR_LABEL,
        SettingsRow::AiCostLimit => keys::AI_COST_LIMIT_LABEL,
        SettingsRow::AiTelemetry => keys::AI_ROW_TELEMETRY,
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
        // FR-079 (S3): таб «Подсказки»
        SettingsRow::SuggestEnabled => keys::DESC_SUGGEST_ENABLED,
        SettingsRow::SuggestEngine => keys::DESC_SUGGEST_ENGINE,
        SettingsRow::ExplainCoverage => keys::DESC_EXPLAIN_COVERAGE,
        SettingsRow::DragPushEnabled => keys::DESC_DRAG_PUSH_ENABLED,
        SettingsRow::DragPushSafeGap => keys::DESC_DRAG_PUSH_GAP,
        SettingsRow::DragPushHalo => keys::DESC_DRAG_PUSH_HALO,
        SettingsRow::DragPushPredictive => keys::DESC_DRAG_PUSH_PREDICTIVE,
        SettingsRow::DragPushRebase => keys::DESC_DRAG_PUSH_REBASE,
        SettingsRow::IconStyle => keys::DESC_ICON_STYLE,
        // FR-087: описания — общие на группу категорий (лейблы — конкретные)
        SettingsRow::Role => keys::DESC_ROLE,
        SettingsRow::TelemetryCounter => keys::DESC_TELEMETRY_COUNTER,
        SettingsRow::TelemetryAnalytics => keys::DESC_TELEMETRY_ANALYTICS,
        SettingsRow::TplCatBackend
        | SettingsRow::TplCatNetwork
        | SettingsRow::TplCatUnitEconomics
        | SettingsRow::TplCatProductAnalytics => keys::DESC_TPLCAT,
        SettingsRow::SchemeCatArchitecture
        | SettingsRow::SchemeCatBusiness
        | SettingsRow::SchemeCatFramework
        | SettingsRow::SchemeCatPlanning
        | SettingsRow::SchemeCatOnboarding => keys::DESC_SCHEMECAT,
        // FR-LLM-B / PRD-0010 F-7: описания строк таба «AI и модели»
        SettingsRow::AiProvSuggest => keys::AI_DESC_PROV_SUGGEST,
        SettingsRow::AiProvGraph => keys::AI_DESC_PROV_GRAPH,
        SettingsRow::AiProvAgent => keys::AI_DESC_PROV_AGENT,
        SettingsRow::AiModelSuggest => keys::AI_DESC_MODEL_SUGGEST,
        SettingsRow::AiModelGraph => keys::AI_DESC_MODEL_GRAPH,
        SettingsRow::AiModelAgent => keys::AI_DESC_MODEL_AGENT,
        // FR-LLM-OAUTH-APP: описание строки «Вход ChatGPT».
        SettingsRow::AiOAuth => keys::AI_OAUTH_DESC,
        // FR-LLM-FIX: API-ключ BYOK + self-hosted endpoint.
        SettingsRow::AiApiKey => keys::AI_DESC_API_KEY,
        SettingsRow::AiSelfhostUrl => keys::AI_DESC_SELFHOST_URL,
        SettingsRow::AiSelfhostKey => keys::AI_DESC_SELFHOST_KEY,
        SettingsRow::AiResidency => keys::AI_DESC_RESIDENCY,
        SettingsRow::AiConfidenceThreshold => keys::AI_DESC_CONF_THRESHOLD,
        SettingsRow::AiCostLimit => keys::AI_DESC_COST_LIMIT,
        SettingsRow::AiTelemetry => keys::AI_DESC_TELEMETRY,
    }
}

/// Род строки: тумблер (kit-switch, клик переключает) или dropdown
/// (клик открывает меню значений). Инвариант (юнит-тест): `Toggle` — ровно
/// для `bool`-полей `Settings`, `Dropdown` — для остальных.
///
/// FR-LLM-FIX: `Button` — текстовое поле + кнопка «Проверить» + бейдж
/// (API-ключ BYOK, self-hosted endpoint). Клик по строке триггерит
/// mock health-check и переключает бейдж; реальный health-check —
/// Stream C/D TODO.
///
/// FR-LLM-FIX (task FIX-TEXT-INPUT): `TextInput` — редактируемое текстовое
/// поле (имя модели BYOK, API-ключ, URL endpoint'а, self-hosted ключ).
/// Клик по текстовому полю — фокус и начало редактирования (append +
/// backspace, как у inline-поля подмены `explain`); клик по кнопке
/// (опциональной, для строк с проверкой) — `apply_button_row`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    /// Булева настройка: клик — переключить (kit-switch, меню избыточно).
    Toggle,
    /// Многозначная настройка: клик — открыть выпадающее меню.
    Dropdown,
    // FR-LLM-FIX: текстовое поле + кнопка действия (health-check mock).
    /// Клик — вызвать кнопку строки (mock health-check, переключить бейдж).
    Button,
    // FR-LLM-FIX (task FIX-TEXT-INPUT): редактируемое текстовое поле (имя
    // модели / API-ключ / URL / self-hosted ключ). Клик по полю — фокус и
    // ввод; клик по кнопке (если есть) — `apply_button_row`.
    /// Редактируемое текстовое поле (имя модели, API-ключ, URL).
    TextInput,
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
        | SettingsRow::SnapCoarseZoom
        | SettingsRow::DragPushSafeGap
        | SettingsRow::DragPushHalo
        | SettingsRow::IconStyle
        // FR-087: роль — dropdown реестра ролей
        | SettingsRow::Role
        | SettingsRow::SuggestEngine
        // FR-LLM-B: dropdown-строки таба «AI и модели»
        | SettingsRow::AiProvSuggest
        | SettingsRow::AiProvGraph
        | SettingsRow::AiProvAgent
        | SettingsRow::AiResidency
        | SettingsRow::AiConfidenceThreshold
        | SettingsRow::AiCostLimit => RowKind::Dropdown,
        // FR-LLM-OAUTH-APP: строка «Вход ChatGPT» — кнопка действия справа
        // («Войти/Отменить/Выйти/Повторить» по состоянию) + бейдж состояния;
        // текстового поля нет (рендер — спец-ветка в RowKind::Button).
        SettingsRow::AiOAuth => RowKind::Button,
        // FR-LLM-FIX (task FIX-TEXT-INPUT): per-feature BYOK-модель —
        // редактируемое текстовое поле (пользователь вводит имя модели
        // вручную, а не выбирает из списка; дефолт «glm-5.3-flash»).
        SettingsRow::AiModelSuggest
        | SettingsRow::AiModelGraph
        | SettingsRow::AiModelAgent => RowKind::TextInput,
        // FR-LLM-FIX (task FIX-TEXT-INPUT): API-ключ BYOK + self-hosted
        // endpoint — текстовое поле + кнопка «Проверить» (split hit-test:
        // клик по текстовому полю — фокус/ввод, по кнопке — health-check).
        | SettingsRow::AiApiKey
        | SettingsRow::AiSelfhostUrl
        | SettingsRow::AiSelfhostKey => RowKind::TextInput,
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
        | SettingsRow::ExplainCoverage
        | SettingsRow::AutolinkEnabled
        | SettingsRow::SuggestEnabled
        | SettingsRow::DragPushEnabled
        | SettingsRow::DragPushPredictive
        // FR-087: тумблеры категорий шаблонов/схем (bool-видимость)
        | SettingsRow::TplCatBackend
        | SettingsRow::TplCatNetwork
        | SettingsRow::TplCatUnitEconomics
        | SettingsRow::TplCatProductAnalytics
        | SettingsRow::SchemeCatArchitecture
        | SettingsRow::SchemeCatBusiness
        | SettingsRow::SchemeCatFramework
        | SettingsRow::SchemeCatPlanning
        | SettingsRow::SchemeCatOnboarding
        // FR-089: согласия телеметрии — bool-поля Settings
        | SettingsRow::TelemetryCounter
        | SettingsRow::TelemetryAnalytics
        | SettingsRow::DragPushRebase
        // FR-LLM-B: тумблер телеметрии AI (bool-поле LlmSettings)
        | SettingsRow::AiTelemetry => RowKind::Toggle,
    }
}

/// Текущее значение настройки для dropdown-кнопки (контрол на строке
/// показывает значение — HIG «Pop-Up Buttons»; тексты из таблицы
/// [`crate::i18n`], язык — `settings.language`). Для тумблеров — `None`
/// (состояние видно по позиции бегунка kit-switch).
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
        // FR-087: текущая роль — имя в языке интерфейса (неизвестный id —
        // подписка default, мягкая деградация реестра).
        SettingsRow::Role => Some(canvas_core::roles::display_name(
            &settings.role,
            language,
        )
        .to_owned()),
        // FR-079 (S3): движок подсказок (лексика/гибрид/выкл)
        SettingsRow::SuggestEngine => Some(
            i18n::tr(
                language,
                match settings.suggest.engine {
                    canvas_core::SuggestEngineKind::Off => keys::SUGGEST_ENGINE_OFF,
                    canvas_core::SuggestEngineKind::Lex => keys::SUGGEST_ENGINE_LEX,
                    canvas_core::SuggestEngineKind::LexLaya => keys::SUGGEST_ENGINE_LEXLAYA,
                },
            )
            .to_owned(),
        ),
        SettingsRow::ThemePreset => Some(match settings.active_preset() {
            Some(id) => theme_presets::find(id).unwrap().label.to_owned(),
            None => i18n::tr(language, keys::THEME_PRESET_CLASSIC).to_owned(),
        }),
        SettingsRow::SnapTolerance => Some(format!("{} px", settings.snap_tolerance_px as i32)),
        // FR-073: зазор/ореол — пресеты в px сцены; 0 показываем как «0 px»
        // (старое поведение «вплотную» описано в описании строки)
        SettingsRow::DragPushSafeGap => {
            Some(format!("{} px", settings.drag_push_gap_px as i32))
        }
        SettingsRow::DragPushHalo => {
            Some(format!("{} px", settings.drag_push_halo_px as i32))
        }
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
        | SettingsRow::ExplainCoverage
        | SettingsRow::HudOnStart
        // FR-079 (S3): тумблер — состояние видно по позиции pill-ручки
        | SettingsRow::SuggestEnabled
        // FR-073: тумблеры — состояние видно по позиции pill-ручки
        | SettingsRow::DragPushEnabled
        | SettingsRow::DragPushPredictive
        // FR-087: тумблеры категорий — состояние видно по pill-ручке
        | SettingsRow::TplCatBackend
        | SettingsRow::TplCatNetwork
        | SettingsRow::TplCatUnitEconomics
        | SettingsRow::TplCatProductAnalytics
        | SettingsRow::SchemeCatArchitecture
        | SettingsRow::SchemeCatBusiness
        | SettingsRow::SchemeCatFramework
        | SettingsRow::SchemeCatPlanning
        | SettingsRow::SchemeCatOnboarding
        // FR-089: согласия — состояние видно по pill-ручке
        | SettingsRow::TelemetryCounter
        | SettingsRow::TelemetryAnalytics
        | SettingsRow::DragPushRebase
        // FR-LLM-B: тумблер AI-телеметрии — состояние по pill-ручке
        | SettingsRow::AiTelemetry => None,
        // FR-LLM-OAUTH-APP: строка «Вход ChatGPT» — статус показывает бейдж,
        // dropdown-кнопки нет.
        | SettingsRow::AiOAuth
        // FR-LLM-FIX (task FIX-TEXT-INPUT): TextInput-строки (API-ключ /
        // self-hosted URL / self-hosted key / per-feature BYOK-модель) —
        // значение показывает текстовое поле и бейдж, dropdown_value
        // не нужен (нет dropdown-кнопки).
        | SettingsRow::AiApiKey
        | SettingsRow::AiSelfhostUrl
        | SettingsRow::AiSelfhostKey => None,
        // FR-ICONS: текущий набор — локализованное имя варианта.
        SettingsRow::IconStyle => Some(i18n::tr(language, icon_style_key(settings.icon_style)).to_owned()),
        // FR-LLM-B / PRD-0010 F-7: значения dropdown-строк таба «AI и модели».
        // Локализованные имена провайдеров / моделей / режимов residency.
        SettingsRow::AiProvSuggest => Some(ai_provider_label(language, settings.llm.provider_suggest)),
        SettingsRow::AiProvGraph => Some(ai_provider_label(language, settings.llm.provider_graph)),
        SettingsRow::AiProvAgent => Some(ai_provider_label(language, settings.llm.provider_agent)),
        // FR-LLM-FIX (task FIX-TEXT-INPUT): per-feature BYOK-модель теперь —
        // редактируемое текстовое поле (RowKind::TextInput), не dropdown;
        // dropdown_value не вызывается. Сохраняем arm для полноты match
        // (возвращает сам model_id, как раньше).
        SettingsRow::AiModelSuggest => {
            Some(ai_model_current_label(&settings.llm.model_suggest))
        }
        SettingsRow::AiModelGraph => {
            Some(ai_model_current_label(&settings.llm.model_graph))
        }
        SettingsRow::AiModelAgent => {
            Some(ai_model_current_label(&settings.llm.model_agent))
        }
        SettingsRow::AiResidency => Some(ai_residency_label(language, settings.llm.data_residency)),
        SettingsRow::AiConfidenceThreshold => {
            Some(format!("{:.2}", settings.llm.confidence_threshold))
        }
        SettingsRow::AiCostLimit => Some(format!("${:.2} / день", settings.llm.cost_limit_daily)),
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
        // FR-087: реестр ролей (порядок = порядок показа; «текущий» — по id,
        // неизвестный id не отмечается ни в одном пункте). Порядок опций =
        // порядку apply_dropdown_value (инвариант, тест).
        SettingsRow::Role => canvas_core::roles::ROLES
            .iter()
            .map(|role| {
                (
                    role.display_name(language).to_owned(),
                    role.id == settings.role,
                )
            })
            .collect(),
        // FR-079 (S3): порядок = apply_dropdown_value (инвариант, тест)
        SettingsRow::SuggestEngine => [
            (
                i18n::tr(language, keys::SUGGEST_ENGINE_LEX).to_owned(),
                settings.suggest.engine == canvas_core::SuggestEngineKind::Lex,
            ),
            (
                i18n::tr(language, keys::SUGGEST_ENGINE_LEXLAYA).to_owned(),
                settings.suggest.engine == canvas_core::SuggestEngineKind::LexLaya,
            ),
            (
                i18n::tr(language, keys::SUGGEST_ENGINE_OFF).to_owned(),
                settings.suggest.engine == canvas_core::SuggestEngineKind::Off,
            ),
        ]
        .to_vec(),
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
        // FR-073: пресеты сейф-зазора и ореола (px сцены); «текущий» —
        // последний пресет ≤ значения (кламп к пресету-кандидату)
        SettingsRow::DragPushSafeGap => {
            let current = DRAG_PUSH_GAP_PRESETS
                .iter()
                .rposition(|preset| *preset <= settings.drag_push_gap_px)
                .unwrap_or(0);
            DRAG_PUSH_GAP_PRESETS
                .iter()
                .enumerate()
                .map(|(i, preset)| (format!("{} px", *preset as i32), i == current))
                .collect()
        }
        SettingsRow::DragPushHalo => {
            let current = DRAG_PUSH_HALO_PRESETS
                .iter()
                .rposition(|preset| *preset <= settings.drag_push_halo_px)
                .unwrap_or(0);
            DRAG_PUSH_HALO_PRESETS
                .iter()
                .enumerate()
                .map(|(i, preset)| (format!("{} px", *preset as i32), i == current))
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
        | SettingsRow::ExplainCoverage
        | SettingsRow::HudOnStart
        // FR-079 (S3): тумблер — dropdown не открывает (RowKind::Toggle)
        | SettingsRow::SuggestEnabled
        // FR-073: тумблеры — dropdown не открывает (RowKind::Toggle)
        | SettingsRow::DragPushEnabled
        | SettingsRow::DragPushPredictive
        // FR-087: тумблеры категорий — dropdown не открывает (Toggle)
        | SettingsRow::TplCatBackend
        | SettingsRow::TplCatNetwork
        | SettingsRow::TplCatUnitEconomics
        | SettingsRow::TplCatProductAnalytics
        | SettingsRow::SchemeCatArchitecture
        | SettingsRow::SchemeCatBusiness
        | SettingsRow::SchemeCatFramework
        | SettingsRow::SchemeCatPlanning
        | SettingsRow::SchemeCatOnboarding
        // FR-089: согласия — dropdown не открывает (Toggle)
        | SettingsRow::TelemetryCounter
        | SettingsRow::TelemetryAnalytics
        | SettingsRow::DragPushRebase
        // FR-LLM-B: тумблер телеметрии AI — dropdown не открывает (Toggle)
        | SettingsRow::AiTelemetry => Vec::new(),
        // FR-LLM-OAUTH-APP: строка «Вход ChatGPT» — меню не открывает
        // (RowKind::Button; клик по кнопке — apply_button_row).
        | SettingsRow::AiOAuth
        // FR-LLM-FIX: Button-строки (API-ключ / self-hosted URL / self-hosted
        // key) — dropdown не открывает (RowKind::Button); клик триггерит
        // mock health-check (см. apply_button_row в overlays.rs).
        | SettingsRow::AiApiKey
        | SettingsRow::AiSelfhostUrl
        | SettingsRow::AiSelfhostKey => Vec::new(),
        // FR-ICONS: порядок опций = порядок IconStyle::ALL (инвариант, тест) =
        // порядку apply_dropdown_value (тест). Локализованные имена наборов.
        SettingsRow::IconStyle => IconStyle::ALL
            .iter()
            .map(|&style| {
                (
                    i18n::tr(language, icon_style_key(style)).to_owned(),
                    settings.icon_style == style,
                )
            })
            .collect(),
        // FR-LLM-B / PRD-0010 F-7: опции dropdown-строк таба «AI и модели».
        // Порядок = apply_dropdown_value (инвариант, тест). Suggest без
        // ChatGPT (Q2); graph/agent без Laya (LlmProviderId constraint).
        SettingsRow::AiProvSuggest => [
            canvas_llm::LlmProviderId::Byok,
            canvas_llm::LlmProviderId::Ollama,
            canvas_llm::LlmProviderId::Laya,
            canvas_llm::LlmProviderId::Off,
        ]
        .into_iter()
        .map(|p| (ai_provider_label(language, p), settings.llm.provider_suggest == p))
        .collect(),
        SettingsRow::AiProvGraph => [
            canvas_llm::LlmProviderId::ChatGptOAuth,
            canvas_llm::LlmProviderId::Byok,
            canvas_llm::LlmProviderId::Ollama,
            canvas_llm::LlmProviderId::Off,
        ]
        .into_iter()
        .map(|p| (ai_provider_label(language, p), settings.llm.provider_graph == p))
        .collect(),
        SettingsRow::AiProvAgent => [
            canvas_llm::LlmProviderId::ChatGptOAuth,
            canvas_llm::LlmProviderId::Byok,
            canvas_llm::LlmProviderId::Ollama,
            canvas_llm::LlmProviderId::Off,
        ]
        .into_iter()
        .map(|p| (ai_provider_label(language, p), settings.llm.provider_agent == p))
        .collect(),
        // FR-LLM-FIX (task FIX-TEXT-INPUT): per-feature BYOK-модель —
        // теперь редактируемое текстовое поле (RowKind::TextInput), не
        // dropdown. Список опций пуст (меню не открывается). Раньше список
        // подгружался из `/v1/models` после health-check ключа — теперь
        // пользователь вводит имя модели вручную (дефолт «glm-5.3-flash»).
        SettingsRow::AiModelSuggest => Vec::new(),
        SettingsRow::AiModelGraph => Vec::new(),
        SettingsRow::AiModelAgent => Vec::new(),
        SettingsRow::AiResidency => [
            canvas_llm::DataResidency::Local,
            canvas_llm::DataResidency::Cloud,
            canvas_llm::DataResidency::SelfHosted,
        ]
        .into_iter()
        .map(|r| (ai_residency_label(language, r), settings.llm.data_residency == r))
        .collect(),
        SettingsRow::AiConfidenceThreshold => AI_CONF_PRESETS
            .iter()
            .map(|&v| (format!("{:.2}", v), (settings.llm.confidence_threshold - v).abs() < 1e-6))
            .collect(),
        SettingsRow::AiCostLimit => AI_COST_LIMIT_PRESETS
            .iter()
            .map(|&v| (format!("${:.2} / день", v), (settings.llm.cost_limit_daily - v).abs() < 1e-6))
            .collect(),
    }
}

// FR-LLM-B / PRD-0010 F-7: вспомогательные типы/функции для таба «AI и модели».

// FR-LLM-FIX: хардкод-список BYOK-моделей (AiByokModel/AI_BYOK_MODELS) удалён —
// список моделей подгружается из `/v1/models` после health-check ключа.
// До проверки ключа список пуст (Vec::new()), dropdown_options для
// AiModelSuggest/Graph/Agent возвращает пустой Vec, apply_dropdown_value —
// no-op. Реальный health-check — Stream C/D TODO (`// FR-LLM-FIX-TODO:`).

/// Пресеты confidence threshold (F-7.5): 0.0 / 0.25 / 0.5 / 0.75 / 1.0.
/// Шаг слайдера прототипа — 0.05, но пресеты в dropdown выбраны по краям и
/// середине, чтобы давать понятные «режимы» (показывать всё / только
/// уверенные / только очень уверенные). Порядок = apply_dropdown_value.
pub const AI_CONF_PRESETS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

/// Пресеты дневного лимита cost (F-7.4): $0.25 / $0.50 / $1.00 / $2.00 /
/// $5.00. Шаг слайдера прототипа — $0.25; пресеты выбраны как опорные точки
/// «минимум / экономно / по умолчанию / активно / без ограничений по сути».
pub const AI_COST_LIMIT_PRESETS: [f64; 5] = [0.25, 0.50, 1.00, 2.00, 5.00];

/// Локализованное имя провайдера (LlmProviderId → i18n-ключ → таблица).
/// Используется в dropdown_value и dropdown_options (инвариант: та же
/// функция для кнопки и пунктов меню — «ввод = тому, что видно»).
pub fn ai_provider_label(language: Language, provider: canvas_llm::LlmProviderId) -> String {
    let key = match provider {
        canvas_llm::LlmProviderId::Off => keys::AI_OPT_OFF,
        canvas_llm::LlmProviderId::Laya => keys::AI_OPT_LAYA,
        canvas_llm::LlmProviderId::Ollama => keys::AI_OPT_OLLAMA,
        canvas_llm::LlmProviderId::Byok => keys::AI_OPT_BYOK,
        canvas_llm::LlmProviderId::ChatGptOAuth => keys::AI_OPT_CHATGPT,
    };
    i18n::tr(language, key).to_owned()
}

/// Локализованное имя режима data residency.
pub fn ai_residency_label(language: Language, residency: canvas_llm::DataResidency) -> String {
    let key = match residency {
        canvas_llm::DataResidency::Local => keys::AI_RESIDENCY_LOCAL,
        canvas_llm::DataResidency::Cloud => keys::AI_RESIDENCY_CLOUD,
        canvas_llm::DataResidency::SelfHosted => keys::AI_RESIDENCY_SELFHOST,
    };
    i18n::tr(language, key).to_owned()
}

/// FR-LLM-FIX: отображаемое имя текущей BYOK-модели (per-feature). Раньше
/// искала модель в хардкод-списке `AI_BYOK_MODELS` и падала на первую модель
/// (glm-5.3-flash); теперь возвращает сам `model_id` (это идентификатор из
/// `/v1/models`, в UI отображается как есть). Пустая строка — «не выбрана»
/// (UI показывает плейсхолдер `AI_MODEL_PH_EMPTY`/`AI_MODEL_PH_NO_KEY`).
fn ai_model_current_label(model_id: &str) -> String {
    model_id.to_owned()
}

/// FR-ICONS: i18n-ключ локализованного имени варианта `IconStyle`.
pub fn icon_style_key(style: IconStyle) -> &'static str {
    match style {
        IconStyle::Glyph => keys::ICON_STYLE_GLYPH,
        IconStyle::Lucide => keys::ICON_STYLE_LUCIDE,
        IconStyle::Material => keys::ICON_STYLE_MATERIAL,
        IconStyle::Feather => keys::ICON_STYLE_FEATHER,
        IconStyle::Bootstrap => keys::ICON_STYLE_BOOTSTRAP,
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
        // FR-079 (S3): движок подсказок — порядок = dropdown_options
        SettingsRow::SuggestEngine => {
            let engines = [
                canvas_core::SuggestEngineKind::Lex,
                canvas_core::SuggestEngineKind::LexLaya,
                canvas_core::SuggestEngineKind::Off,
            ];
            if let Some(engine) = engines.get(index) {
                settings.suggest.engine = *engine;
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
        // FR-073: пресеты сейф-зазора и ореола
        SettingsRow::DragPushSafeGap => {
            if let Some(preset) = DRAG_PUSH_GAP_PRESETS.get(index) {
                settings.drag_push_gap_px = *preset;
            }
        }
        SettingsRow::DragPushHalo => {
            if let Some(preset) = DRAG_PUSH_HALO_PRESETS.get(index) {
                settings.drag_push_halo_px = *preset;
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
        | SettingsRow::ExplainCoverage
        | SettingsRow::HudOnStart
        | SettingsRow::DragPushEnabled
        | SettingsRow::DragPushPredictive
        | SettingsRow::DragPushRebase
        // FR-079 (S3): тумблер применяется apply_toggle_row, не dropdown
        | SettingsRow::SuggestEnabled
        // FR-087: тумблеры категорий — apply_toggle_row, не dropdown
        | SettingsRow::TplCatBackend
        | SettingsRow::TplCatNetwork
        | SettingsRow::TplCatUnitEconomics
        | SettingsRow::TplCatProductAnalytics
        | SettingsRow::SchemeCatArchitecture
        | SettingsRow::SchemeCatBusiness
        | SettingsRow::SchemeCatFramework
        | SettingsRow::SchemeCatPlanning
        | SettingsRow::SchemeCatOnboarding
        // FR-089: согласия — apply_toggle_row (App), не dropdown
        | SettingsRow::TelemetryCounter
        | SettingsRow::TelemetryAnalytics
        // FR-LLM-B: тумблер телеметрии AI — apply_toggle_row (App), не dropdown
        | SettingsRow::AiTelemetry => {}
        // FR-LLM-OAUTH-APP: строка «Вход ChatGPT» — apply_button_row (App),
        // не dropdown; no-op здесь.
        | SettingsRow::AiOAuth
        // FR-LLM-FIX (task FIX-TEXT-INPUT): TextInput-строки (API-ключ /
        // self-hosted URL / self-hosted key / per-feature BYOK-модель) —
        // apply_button_row/apply_text_input (App), не dropdown; no-op здесь.
        | SettingsRow::AiApiKey
        | SettingsRow::AiSelfhostUrl
        | SettingsRow::AiSelfhostKey => {}
        // FR-ICONS: индекс в `IconStyle::ALL` (порядок = dropdown_options,
        // инвариант теста). Вне диапазона — без изменений (как остальные).
        SettingsRow::IconStyle => {
            if let Some(&style) = IconStyle::ALL.get(index) {
                settings.icon_style = style;
            }
        }
        // FR-087: выбор роли — прямой id из реестра (порядок =
        // dropdown_options). Смена роли СБРАСЫВАЕТ материализованные
        // фильтры категорий (None = наследовать дефолт новой роли) —
        // ручные тумблеры не переживают смену профиля (как Obsidian).
        SettingsRow::Role => {
            if let Some(role) = canvas_core::roles::ROLES.get(index) {
                settings.role = role.id.to_string();
                settings.template_categories = None;
                settings.scheme_categories = None;
            }
        }
        // FR-LLM-B / PRD-0010 F-7: применение выбора в табе «AI и модели».
        // Порядок индексов = dropdown_options (инвариант, тест).
        SettingsRow::AiProvSuggest => {
            let order = [
                canvas_llm::LlmProviderId::Byok,
                canvas_llm::LlmProviderId::Ollama,
                canvas_llm::LlmProviderId::Laya,
                canvas_llm::LlmProviderId::Off,
            ];
            if let Some(&p) = order.get(index) {
                settings.llm.provider_suggest = p;
            }
        }
        SettingsRow::AiProvGraph => {
            let order = [
                canvas_llm::LlmProviderId::ChatGptOAuth,
                canvas_llm::LlmProviderId::Byok,
                canvas_llm::LlmProviderId::Ollama,
                canvas_llm::LlmProviderId::Off,
            ];
            if let Some(&p) = order.get(index) {
                settings.llm.provider_graph = p;
            }
        }
        SettingsRow::AiProvAgent => {
            let order = [
                canvas_llm::LlmProviderId::ChatGptOAuth,
                canvas_llm::LlmProviderId::Byok,
                canvas_llm::LlmProviderId::Ollama,
                canvas_llm::LlmProviderId::Off,
            ];
            if let Some(&p) = order.get(index) {
                settings.llm.provider_agent = p;
            }
        }
        // FR-LLM-FIX (task FIX-TEXT-INPUT): per-feature BYOK-модель — теперь
        // редактируемое текстовое поле (RowKind::TextInput), не dropdown.
        // Применение — через текстовый ввод (settings_text_edit), не через
        // выбор пункта. no-op здесь (инвариант apply_dropdown_value: при
        // пустом списке выбора нет).
        SettingsRow::AiModelSuggest => {}
        SettingsRow::AiModelGraph => {}
        SettingsRow::AiModelAgent => {}
        SettingsRow::AiResidency => {
            let order = [
                canvas_llm::DataResidency::Local,
                canvas_llm::DataResidency::Cloud,
                canvas_llm::DataResidency::SelfHosted,
            ];
            if let Some(&r) = order.get(index) {
                settings.llm.data_residency = r;
            }
        }
        SettingsRow::AiConfidenceThreshold => {
            if let Some(&v) = AI_CONF_PRESETS.get(index) {
                settings.llm.confidence_threshold = v;
            }
        }
        SettingsRow::AiCostLimit => {
            if let Some(&v) = AI_COST_LIMIT_PRESETS.get(index) {
                settings.llm.cost_limit_daily = v;
            }
        }
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
    /// Режим раскладки по брейкпоинтам ширины (W-e): Desktop — навигация
    /// слева; Compact/Mobile — одноколоночная с горизонтальным таб-баром
    /// (Mobile — полноэкранный лист). Рисование ветвит по нему —
    /// «ввод = тому, что видно»: hit-тесты работают от тех же rect'ов
    /// ([`modal_nav_at`] не различает ориентацию — пункты в `nav_items`).
    pub mode: ModalMode,
    /// Ширина левой колонки навигации (кламп на узких окнах; 0 — колонки
    /// нет: Compact/Mobile).
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
    /// Применённая прокрутка контента правой панели (после клампа; 0 —
    /// верх списка). Потребители (рисование/ввод) читают одно и то же
    /// значение из одной функции раскладки — согласованность рендера и
    /// ввода, паттерн `RowsLayout` autolink_ui.
    pub scroll: f32,
    /// Предел прокрутки контента правой панели (0 — всё влезает);
    /// см. [`modal_scroll_max`].
    pub scroll_max: f32,
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

// --- W-c: контролы строк — kit-примитивы -------------------------------------

/// Нейтральный срез палитры для ЧИСТО-геометрических вызовов kit `switch`:
/// геометрия трека/бегунка слотов не читает (слоты идут только в
/// `track_style`/`knob_fill` — заливки, выбор которых остаётся за рисующим
/// слоем, контракт F-8 «цвета — только слоты»). Нули в рендер не попадают.
const SWITCH_GEOMETRY_PALETTE: canvas_ui::kit::KitPalette = canvas_ui::kit::KitPalette {
    panel_fill: [0.0; 4],
    panel_border: [0.0; 4],
    control_fill: [0.0; 4],
    control_border: [0.0; 4],
    control_primary: [0.0; 4],
    control_danger: [0.0; 4],
    hover_fill: [0.0; 4],
    primary_hover_fill: [0.0; 4],
    selected_fill: [0.0; 4],
    text: [0.0; 4],
    text_title: [0.0; 4],
    text_muted: [0.0; 4],
    disabled_text: [0.0; 4],
    accent: [0.0; 4],
    // FR-070 (волна W-d): новые семантические слоты `control_success`/
    // `control_warning`/`stage_dim`/`scrollbar_thumb`/`rule_color` не
    // читаются геометрическим вызовом `kit::switch`, но значения берём из
    // `KitPalette::dark()` (Agent B) — чтобы test fixture не расходился с
    // production-коридором (single source of truth — AGENTS.md §UI-кит).
    control_success: [0.30, 0.75, 0.55, 1.0],
    control_warning: [0.95, 0.65, 0.30, 1.0],
    stage_dim: [0.02, 0.02, 0.04, 0.6],
    scrollbar_thumb: [0.35, 0.38, 0.46, 0.7],
    rule_color: [0.30, 0.33, 0.40, 0.8],
};

/// Kit-раскладка тумблера строки настроек — единственный источник геометрии
/// трека/бегунка (`canvas_ui::kit::switch`: трек SWITCH_W×SWITCH_H, радиус
/// RADIUS_PILL; бегунок SWITCH_H−2·SWITCH_KNOB_PAD, отступ SWITCH_KNOB_PAD,
/// позиция — по `on`).
fn kit_switch_layout(slot: canvas_ui::geometry::UiRect, on: bool) -> canvas_ui::kit::SwitchLayout {
    canvas_ui::kit::switch(
        slot,
        on,
        canvas_ui::kit::KitState::Normal,
        &SWITCH_GEOMETRY_PALETTE,
    )
}

/// Rect контрола внутри строки: kit-switch (трек) у тумблера, dropdown-кнопка
/// у выпадающего списка (справа от строки, вертикально по центру).
pub fn control_rect(row_rect: [f32; 4], kind: RowKind) -> [f32; 4] {
    match kind {
        // W-c: трек kit-switch в зоне справа — та же выкладка `stack` по
        // центру зоны, что внутри kit::switch (позиция бегунка на трек
        // не влияет, поэтому здесь `on: false`).
        RowKind::Toggle => {
            let zone = canvas_ui::geometry::UiRect::new(
                row_rect[0] + row_rect[2] - MODAL_PADDING - canvas_ui::kit::SWITCH_W,
                row_rect[1],
                canvas_ui::kit::SWITCH_W,
                row_rect[3],
            );
            let track = kit_switch_layout(zone, false).track;
            [track.x, track.y, track.w, track.h]
        }
        RowKind::Dropdown => [
            row_rect[0] + row_rect[2] - MODAL_PADDING - DROPDOWN_BTN_W,
            row_rect[1] + (row_rect[3] - DROPDOWN_BTN_H) / 2.0,
            DROPDOWN_BTN_W,
            DROPDOWN_BTN_H,
        ],
        // FR-LLM-FIX: кнопка действия в Button-строке (API-ключ / self-hosted
        // URL / self-hosted key) — справа, как dropdown; ширина/высота — те
        // же DROPDOWN_BTN_W/H (единый визуальный ритм контрола справа).
        // Текстовое поле и бейдж рисуются отдельно (см. overlays.rs).
        RowKind::Button => [
            row_rect[0] + row_rect[2] - MODAL_PADDING - DROPDOWN_BTN_W,
            row_rect[1] + (row_rect[3] - DROPDOWN_BTN_H) / 2.0,
            DROPDOWN_BTN_W,
            DROPDOWN_BTN_H,
        ],
        // FR-LLM-FIX (task FIX-TEXT-INPUT): контрол справа — кнопка действия
        // «Проверить» (для строк AiApiKey/AiSelfhostUrl/AiSelfhostKey); для
        // модель-строк кнопок нет, но control_rect не вызывается в их
        // hit-test (клик по всему полю текстового ввода). Геометрия та же,
        // что у Button/Dropdown — единый визуальный ритм контрола справа.
        RowKind::TextInput => [
            row_rect[0] + row_rect[2] - MODAL_PADDING - DROPDOWN_BTN_W,
            row_rect[1] + (row_rect[3] - DROPDOWN_BTN_H) / 2.0,
            DROPDOWN_BTN_W,
            DROPDOWN_BTN_H,
        ],
    }
}

/// Rect бегунка тумблера по треку и состоянию (W-c: геометрия —
/// `canvas_ui::kit::switch`): квадрат SWITCH_H−2·SWITCH_KNOB_PAD с отступом
/// SWITCH_KNOB_PAD; включён — справа, выключен — слева. Цвета (слоты
/// заливки) — за рисующим слоем (F-8), на геометрию они не влияют.
pub fn pill_knob_rect(track: [f32; 4], on: bool) -> [f32; 4] {
    let kit = kit_switch_layout(
        canvas_ui::geometry::UiRect::new(track[0], track[1], track[2], track[3]),
        on,
    );
    [kit.knob.x, kit.knob.y, kit.knob.w, kit.knob.h]
}

/// FR-LLM-FIX (task FIX-TEXT-INPUT): есть ли у TextInput-строки кнопка
/// действия справа («Проверить ключ»/«Проверить»). У модель-строк кнопки
/// нет — всё поле текстового ввода; у API-ключа/URL/self-hosted ключа —
/// есть (mock health-check). Решает hit-test: клик в `control_rect` строки
/// с кнопкой → `apply_button_row`, клик в `text_input_field_rect` → фокус.
pub fn text_input_has_button(row: SettingsRow) -> bool {
    matches!(
        row,
        SettingsRow::AiApiKey | SettingsRow::AiSelfhostUrl | SettingsRow::AiSelfhostKey
    )
}

/// FR-LLM-FIX (task FIX-TEXT-INPUT): rect текстового поля TextInput-строки
/// (под лейблом, слева; ширина = всё доступное место минус кнопка+бейдж для
/// строк с кнопкой). Геометрия — та же, что в overlays.rs (RowKind::Button),
/// перенесена в функцию-источник: hit-test и рисование читают одно значение
/// (детерминизм pick'а и кадра, паттерн `control_rect`/`pill_knob_rect`).
///
/// FR-LLM-FIX (task FIX-TEXTINPUT-KIT): высота поля — `kit::TEXT_FIELD_HEIGHT`
/// (30.0) — единый источник метрики кита (FR-055), прежний литерал 16.0
/// делал поле визуально меньше кнопок/dropdown'ов строки (нарушение rhythm
/// controls). Поле (y=22, h=30) занимает остаток `MODAL_ROW_HEIGHT` (52)
/// целиком — лейбл сверху, поле снизу, без пустого нижнего поля.
pub fn text_input_field_rect(row: SettingsRow, row_rect: [f32; 4]) -> [f32; 4] {
    let field_y = row_rect[1] + 22.0;
    let field_h = canvas_ui::kit::TEXT_FIELD_HEIGHT;
    let avail_w = (row_rect[2] - MODAL_ROW_LABEL_W).max(40.0);
    let field_x = row_rect[0] + 2.0;
    let field_w = if text_input_has_button(row) {
        // Кнопка «Проверить» справа + бейдж (110 px) + зазор 6 px.
        let badge_w = 110.0;
        let gap = 6.0;
        (avail_w - badge_w - gap).max(40.0)
    } else {
        // Модель-строки: всё доступное место под текст (кнопки/бейджа нет).
        avail_w
    };
    [field_x, field_y, field_w, field_h]
}

/// Режим раскладки модалки по ширине вьюпорта (W-e, вариант «A», решение
/// владельца 03.10.2026): брейкпоинты [`MODAL_BP_COMPACT`]/[
/// `MODAL_BP_MOBILE`] — единственный источник ветвления, символьных
/// эвристик ширины нет (запрет CR-015).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalMode {
    /// viewport ≥ [`MODAL_BP_COMPACT`] (1280): прежняя двухколоночная
    /// раскладка — навигация слева, строки в правой панели.
    Desktop,
    /// [`MODAL_BP_MOBILE`] ≤ viewport < [`MODAL_BP_COMPACT`] (компакт):
    /// одноколоночная композиция — строки на всю ширину, навигация —
    /// горизонтальный таб-бар над заголовком; ширина модалки клампится к
    /// вьюпорту минус поля (дизайн-формула и окно-кламп прежние).
    Compact,
    /// viewport < [`MODAL_BP_MOBILE`] (мобайл): полноэкранный лист —
    /// модалка занимает вьюпорт минус внешние поля [`SETTINGS_MARGIN`]
    /// (== токен `SPACING_LG`); скелет — как в компакт-режиме.
    Mobile,
}

/// Режим раскладки по ширине вьюпорта (чистая функция; высота не участвует —
/// обе контрольные точки заданы по ширине окна, FR-039 §2).
pub fn modal_mode(viewport: [f32; 2]) -> ModalMode {
    if viewport[0] < MODAL_BP_MOBILE {
        ModalMode::Mobile
    } else if viewport[0] < MODAL_BP_COMPACT {
        ModalMode::Compact
    } else {
        ModalMode::Desktop
    }
}

/// Адаптивный размер модалки — два яруса `constrain` (FR-054, примитивы U3):
/// Desktop/Compact — 1) желаемый (45%/60% вьюпорта) в дизайн-границах
/// `[MODAL_MIN_*, MODAL_MAX_*]`; 2) итог — в пределах окна с полями
/// [`SETTINGS_MARGIN`] (инвариант: модалка целиком в окне при любом
/// viewport, 320×240 включительно; окно-кламп приоритетен над
/// дизайн-минимумом). Mobile (W-e) — полноэкранный лист: вьюпорт минус
/// внешние поля [`SETTINGS_MARGIN`] (`SETTINGS_MARGIN` == токен
/// `canvas_core::tokens::SPACING_LG`, значение 12 — новых литералов нет).
fn modal_size(viewport: [f32; 2]) -> [f32; 2] {
    if modal_mode(viewport) == ModalMode::Mobile {
        return [
            (viewport[0] - SETTINGS_MARGIN * 2.0).max(1.0),
            (viewport[1] - SETTINGS_MARGIN * 2.0).max(1.0),
        ];
    }
    use canvas_ui::geometry::UiVec2;
    use canvas_ui::layout::constrain;
    let design = constrain(
        UiVec2::new(MODAL_MIN_W, MODAL_MIN_H),
        UiVec2::new(MODAL_MAX_W, MODAL_MAX_H),
        UiVec2::new(viewport[0] * 0.45, viewport[1] * 0.6),
    );
    let window_max = UiVec2::new(
        (viewport[0] - SETTINGS_MARGIN * 2.0).max(1.0),
        (viewport[1] - SETTINGS_MARGIN * 2.0).max(1.0),
    );
    let size = constrain(UiVec2::new(0.0, 0.0), window_max, design);
    [size.x, size.y]
}

/// Геометрия модалки с позициями навигации, заголовка, строк активного
/// таба и карточек темы. Активный таб — индекс в [`SETTINGS_TABS`]
/// (вне диапазона — первый таб; состояние `App::settings_tab` клампится
/// на вызывающей стороне).
///
/// Раскладка — measured-семейством `canvas_ui` (FR-054 миграция U5; W3.2 —
/// каталог `docs/plans/fr-068-w3-consumer-migration.md`): размер —
/// `constrain`, центрирование — `stack`, скелет — `Row::lay_out_measured`
/// [навигация | контент] (Desktop), пункты навигации и строки —
/// `Column::lay_out_measured`, карточки тем — `Row::lay_out_measured`
/// (зазор — токен `SPACING_MD`, значение прежнего литерала 10); дети
/// выражаются [`MeasuredItem::Fixed`] (константы скелета, Text-детей нет —
/// замерщик не участвует в геометрии). Числа — дословно прежние (тесты
/// фиксируют структуру и клампы).
///
/// W-e (вариант «A», решение владельца 03.10.2026): режим по
/// [`modal_mode`] — Compact/Mobile складывают двухколоночную композицию
/// в одноколоночную: строки на всю ширину контента, навигация —
/// горизонтальный таб-бар над заголовком (те же kit-метрики пункта и
/// measured-примитивы; Mobile — лист вьюпорт минус поля). Скролл (W-a)
/// работает во всех режимах — [`modal_layout_scrolled_with`] над той же
/// базовой раскладкой.
pub fn modal_layout(tab: usize, viewport: [f32; 2]) -> ModalLayout {
    // W3.2: замерщик — канонические shared-точки на вызов (прецедент
    // `ui_registry`/`template_panel_layout`); Fixed-дети его не читают.
    let mut m = canvas_ui::measure::TextMeasurer::new();
    let mut fs = canvas_render::text::measure_font_system();
    // FR-LLM-FIX: None → без фильтрации (тесты / legacy / floating-button
    // rect не зависит от фильтра — берёт только `.rect`).
    modal_layout_with(tab, viewport, &mut m, &mut fs, None)
}

/// FR-LLM-FIX: App-side обёртка с настройками для фильтрации AI-таба.
/// Семантика та же, что у [`modal_layout`], но AI-таб фильтрует BYOK-модель
/// (строка видна только когда провайдер = BYOK). Для остальных табов
/// `settings` игнорируется (фильтрации нет — все строки таба видны).
pub fn modal_layout_with_settings(
    tab: usize,
    viewport: [f32; 2],
    settings: &Settings,
) -> ModalLayout {
    let mut m = canvas_ui::measure::TextMeasurer::new();
    let mut fs = canvas_render::text::measure_font_system();
    modal_layout_with(tab, viewport, &mut m, &mut fs, Some(settings))
}

/// FR-LLM-FIX: видимые строки таба «AI и модели» (индекс 8) с учётом
/// настроек провайдеров и data residency. Модель-строка
/// (AiModelSuggest/Graph/Agent) показывается только когда соответствующий
/// провайдер = BYOK. AiApiKey — когда ANY per-feature провайдер = BYOK.
/// AiSelfhostUrl/AiSelfhostKey — когда data_residency = SelfHosted.
/// FR-LLM-OAUTH-APP: AiOAuth («Вход ChatGPT») — когда provider_graph или
/// provider_agent == ChatGptOAuth (для suggest OAuth недоступен, Q2).
/// Для не-BYOK провайдеров модель подгружается из /v1/models после
/// подключения (ChatGPT/Ollama) или фиксирована (Laya = laya-1.13), пользователь
/// не может её выбрать — строка скрыта. Для других табов возвращаем
/// статический `tab_def.rows` без фильтрации (как раньше).
///
/// **Контракт**: вызывается `modal_layout_with` когда передан `Some(&Settings)`;
/// тесты и legacy-вызовы (`modal_layout(tab, viewport)`) передают `None`
/// и получают все строки (для проверок `tabs_cover_all_rows` и т.п.).
pub fn ai_tab_visible_rows(settings: &Settings) -> Vec<SettingsRow> {
    // FR-LLM-FIX: ANY per-feature провайдер = BYOK → API-ключ виден.
    let any_byok = settings.llm.provider_suggest == canvas_llm::LlmProviderId::Byok
        || settings.llm.provider_graph == canvas_llm::LlmProviderId::Byok
        || settings.llm.provider_agent == canvas_llm::LlmProviderId::Byok;
    // FR-LLM-FIX: data_residency = SelfHosted → self-hosted endpoint виден.
    let selfhosted = settings.llm.data_residency == canvas_llm::DataResidency::SelfHosted;
    // FR-LLM-OAUTH-APP: ChatGptOAuth в graph/agent → блок «Вход ChatGPT».
    let any_chatgpt = settings.llm.provider_graph == canvas_llm::LlmProviderId::ChatGptOAuth
        || settings.llm.provider_agent == canvas_llm::LlmProviderId::ChatGptOAuth;
    let mut out = Vec::new();
    for &row in SETTINGS_TABS[8].rows {
        let visible = match row {
            // FR-LLM-FIX: BYOK-модель видна только когда провайдер = BYOK.
            SettingsRow::AiModelSuggest => {
                settings.llm.provider_suggest == canvas_llm::LlmProviderId::Byok
            }
            SettingsRow::AiModelGraph => {
                settings.llm.provider_graph == canvas_llm::LlmProviderId::Byok
            }
            SettingsRow::AiModelAgent => {
                settings.llm.provider_agent == canvas_llm::LlmProviderId::Byok
            }
            // FR-LLM-OAUTH-APP: блок «Вход ChatGPT» — при ChatGptOAuth в
            // graph/agent (F-5.8; прототип aiAuthBlock).
            SettingsRow::AiOAuth => any_chatgpt,
            // FR-LLM-FIX: API-ключ BYOK виден когда ANY per-feature провайдер = BYOK.
            SettingsRow::AiApiKey => any_byok,
            // FR-LLM-FIX: self-hosted endpoint виден только при SelfHosted.
            SettingsRow::AiSelfhostUrl | SettingsRow::AiSelfhostKey => selfhosted,
            _ => true,
        };
        if visible {
            out.push(row);
        }
    }
    out
}

/// FR-LLM-OAUTH-APP / PRD-0010 F-5.8: UI-состояние блока «Вход ChatGPT»
/// (строка `SettingsRow::AiOAuth`). Снимок состояния OAuth-флоу для рендера
/// и ввода (`App::oauth_ui_state`): натив + feature `l1-llm` — из
/// `app::oauth_flow`; wasm/без фичи — только Idle/Connected (по
/// персистентному флагу), кнопка при этом недоступна.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum OAuthUiState {
    /// Не выполнен вход: кнопка «Войти через ChatGPT» + hint «откроется браузер».
    #[default]
    Idle,
    /// Ожидание подтверждения в браузере: бейдж + кнопка «Отменить».
    Waiting,
    /// Вход выполнен: бейдж ok «вход выполнен · email» (пустая строка —
    /// claim email не выдан, UI показывает «аккаунт») + кнопка «Выйти».
    Connected(String),
    /// Ошибка флоу: бейдж err с текстом + кнопка «Повторить».
    Failed(String),
}

/// [`modal_layout_with`] с ЯВНЫМ замерщиком (для потребителей, уже держащих
/// `measure_font_system` — двойной лок глобального FontSystem невозможен).
///
/// FR-LLM-FIX: `settings_filter` — необязательные настройки для фильтрации
/// строк AI-таба (BYOK-модель показывается только когда провайдер = BYOK).
/// `None` → все строки (тесты / legacy). Возвращает тот же `ModalLayout`.
pub fn modal_layout_with(
    tab: usize,
    viewport: [f32; 2],
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    settings_filter: Option<&Settings>,
) -> ModalLayout {
    use canvas_ui::geometry::{EdgeInsets, UiRect, UiVec2};
    use canvas_ui::layout::{stack, Column, HAlign, MeasuredItem, Row, VAlign};

    let tab_def = SETTINGS_TABS.get(tab).unwrap_or(&SETTINGS_TABS[0]);
    // FR-LLM-FIX: видимые строки (для AI-таба — с фильтром BYOK-модели).
    // Храним в Vec, чтобы пережить borrow cycle (tab_def.rows живёт в
    // статике; vec — на стеке, итерируем безопасно).
    let filtered_rows: Vec<SettingsRow> = if tab == 8 {
        if let Some(s) = settings_filter {
            ai_tab_visible_rows(s)
        } else {
            tab_def.rows.to_vec()
        }
    } else {
        tab_def.rows.to_vec()
    };
    let effective_rows: &[SettingsRow] = &filtered_rows;
    let mode = modal_mode(viewport);
    let single_column = mode != ModalMode::Desktop;
    let [w, h] = modal_size(viewport);
    // Панель — по центру вьюпорта.
    let panel = stack(
        UiRect::new(0.0, 0.0, viewport[0].max(0.0), viewport[1].max(0.0)),
        UiVec2::new(w, h),
        HAlign::Center,
        VAlign::Center,
    );
    let rect = [panel.x, panel.y, panel.w, panel.h];
    let family = crate::admin_ui::FONT_FAMILY;
    // Скелет и навигация:
    // — Desktop: [навигация | контент] (Row gap 0 — колонки вплотную);
    //   левая колонка на узких окнах сжимается (40% ширины модалки), но не
    //   исчезает — инвариант различимости навигации при клампе 320×240.
    // — Compact/Mobile (W-e): одна колонка на всю панель, навигация —
    //   горизонтальный таб-бар из равных слотов (зазор — токен SPACING_S);
    //   слоты считаются от ширины панели — целиком видимы при любом
    //   вьюпорте, переноса/скролла таб-бара нет.
    let (nav_w, content_area, nav_items, hint_rect) = if single_column {
        let tabbar_slot = panel.inset(&EdgeInsets {
            left: MODAL_PADDING,
            top: MODAL_PADDING,
            right: MODAL_PADDING,
            bottom: 0.0,
        });
        let gap = canvas_core::tokens::SPACING_S;
        let count = SETTINGS_TABS.len().max(1);
        let item_w = ((tabbar_slot.w - gap * SETTINGS_TABS.len().saturating_sub(1) as f32)
            / count as f32)
            .max(0.0);
        let items: Vec<[f32; 4]> = Row {
            gap,
            ..Row::default()
        }
        .lay_out_measured(
            tabbar_slot,
            &SETTINGS_TABS
                .iter()
                .map(|_| MeasuredItem::Fixed {
                    w: item_w,
                    h: MODAL_TABBAR_H,
                })
                .collect::<Vec<_>>(),
            m,
            fs,
            family,
            12.0,
        )
        .iter()
        .map(|r| [r.x, r.y, r.w, r.h])
        .collect();
        // Левой колонки нет — подсказка Ctrl+, (rect пуст: hit-тест молчит,
        // рисование ветвится по [`ModalLayout::mode`]).
        (0.0, panel, items, [0.0; 4])
    } else {
        let nav_w = canvas_ui::layout::constrain(
            UiVec2::new(0.0, 0.0),
            UiVec2::new(panel.w * 0.4, f32::INFINITY),
            UiVec2::new(MODAL_NAV_WIDTH, 1.0),
        )
        .x;
        let columns = Row {
            gap: 0.0,
            ..Row::default()
        }
        .lay_out_measured(
            panel,
            &[
                MeasuredItem::Fixed {
                    w: nav_w,
                    h: panel.h,
                },
                MeasuredItem::Fixed {
                    w: panel.w - nav_w,
                    h: panel.h,
                },
            ],
            m,
            fs,
            family,
            12.0,
        );
        let nav_area = columns[0];
        // Пункты навигации: колонка от верхнего паддинга (x — левый край
        // панели, как прежде).
        let nav_slot = nav_area.inset(&EdgeInsets {
            left: 0.0,
            top: MODAL_PADDING,
            right: 0.0,
            bottom: MODAL_PADDING,
        });
        let nav_items: Vec<[f32; 4]> = Column {
            gap: 0.0,
            ..Column::default()
        }
        .lay_out_measured(
            nav_slot,
            &SETTINGS_TABS
                .iter()
                .map(|_| MeasuredItem::Fixed {
                    w: nav_w,
                    h: MODAL_NAV_ITEM_H,
                })
                .collect::<Vec<_>>(),
            m,
            fs,
            family,
            12.0,
        )
        .iter()
        .map(|r| [r.x, r.y, r.w, r.h])
        .collect();
        // Подсказка — к низу навигационной колонки (stack Start/End).
        let hint_slot = nav_area.inset(&EdgeInsets {
            left: MODAL_PADDING,
            top: 0.0,
            right: 0.0,
            bottom: MODAL_PADDING,
        });
        let hint = stack(
            hint_slot,
            UiVec2::new(nav_w - MODAL_PADDING, MODAL_HINT_HEIGHT),
            HAlign::Start,
            VAlign::End,
        );
        let hint_rect = [hint.x, hint.y, hint.w, hint.h];
        (nav_w, columns[1], nav_items, hint_rect)
    };
    // Правая панель: (Compact/Mobile (W-e) — сперва горизонтальный таб-бар
    // и зазор SPACING_MD), затем заголовок раздела (с внутренним паддингом)
    // и зона контента: колонка [паддинг, (таб-бар, зазор), заголовок,
    // зазор 4, контент].
    let content_w = content_area.w;
    let content_h = modal_content_h(panel.h, mode);
    let mut panel_flow: Vec<MeasuredItem> = Vec::new();
    panel_flow.push(MeasuredItem::Fixed {
        w: 0.0,
        h: MODAL_PADDING,
    });
    if single_column {
        // Таб-бар — элемент потока той же ширины, что контент (слоты уже
        // разложены выше — rect'ы в nav_items, здесь занимает место).
        panel_flow.push(MeasuredItem::Fixed {
            w: content_w,
            h: MODAL_TABBAR_H,
        });
        // Зазор таб-бар/заголовок — токен SPACING_MD (масштаб отступов).
        panel_flow.push(MeasuredItem::Fixed {
            w: 0.0,
            h: canvas_core::tokens::SPACING_MD,
        });
    }
    let title_index = panel_flow.len();
    panel_flow.push(MeasuredItem::Fixed {
        w: 0.0,
        h: MODAL_TITLE_HEIGHT,
    });
    // Зазор заголовок/контент ([`MODAL_TITLE_CONTENT_GAP`], вне spacing-scale):
    // вертикальный зазор — именно Fixed{w: 0, h} (Spacer в колонке места не
    // занимает — main-ось колонки высота, см. тест оракула
    // measured_column_matches_manual_fixed_oracle).
    panel_flow.push(MeasuredItem::Fixed {
        w: 0.0,
        h: MODAL_TITLE_CONTENT_GAP,
    });
    let content_index = panel_flow.len();
    panel_flow.push(MeasuredItem::Fixed {
        w: content_w,
        h: content_h,
    });
    let content_flow = Column {
        gap: 0.0,
        ..Column::default()
    }
    .lay_out_measured(content_area, &panel_flow, m, fs, family, 12.0);
    let content = content_flow[content_index];
    let content_rect = [content.x, content.y, content.w, content.h];
    let title_rect = if single_column {
        // Слот заголовка — элемент той же раскладки под таб-баром (один
        // источник y-позиций); ширина — контент минус паддинги, как в
        // Desktop (stack Start/Start от слота потока).
        let title = stack(
            content_flow[title_index],
            UiVec2::new(content_w - MODAL_PADDING * 2.0, MODAL_TITLE_HEIGHT),
            HAlign::Start,
            VAlign::Start,
        );
        [title.x, title.y, title.w, title.h]
    } else {
        // Desktop — прежняя формула: слот от inset контент-зоны.
        let title_slot = content_area.inset(&EdgeInsets {
            left: MODAL_PADDING,
            top: MODAL_PADDING,
            right: MODAL_PADDING,
            bottom: 0.0,
        });
        let title = stack(
            title_slot,
            UiVec2::new(content_w - MODAL_PADDING * 2.0, MODAL_TITLE_HEIGHT),
            HAlign::Start,
            VAlign::Start,
        );
        [title.x, title.y, title.w, title.h]
    };
    // Строки и карточки тем — колонка от зоны контента (внутренний паддинг):
    // в табе с карточками темы строки начинаются ниже карточек (распорка —
    // MODAL_THEME_GAP).
    let inner = content.inset(&EdgeInsets::uniform(MODAL_PADDING));
    let row_w = inner.w;
    let mut flow: Vec<MeasuredItem> = Vec::new();
    let mut card_strip = [[0.0f32; 4]; 2];
    if tab_def.theme_cards {
        // Карточки темы: две рядом («тёмная»/«светлая» — паттерн Obsidian
        // «Base theme»); зазор — токен SPACING_MD (значение прежнего литерала).
        let gap = canvas_core::tokens::SPACING_MD;
        let card_w = (row_w - gap) / 2.0;
        let cards = Row {
            gap,
            ..Row::default()
        }
        .lay_out_measured(
            inner,
            &[
                MeasuredItem::Fixed {
                    w: card_w,
                    h: MODAL_THEME_CARD_H,
                },
                MeasuredItem::Fixed {
                    w: card_w,
                    h: MODAL_THEME_CARD_H,
                },
            ],
            m,
            fs,
            family,
            12.0,
        );
        card_strip = [
            [cards[0].x, cards[0].y, cards[0].w, cards[0].h],
            [cards[1].x, cards[1].y, cards[1].w, cards[1].h],
        ];
        flow.push(MeasuredItem::Fixed {
            w: row_w,
            h: MODAL_THEME_CARD_H,
        });
        flow.push(MeasuredItem::Fixed {
            w: 0.0,
            h: MODAL_THEME_GAP,
        });
    }
    flow.extend(effective_rows.iter().map(|_| MeasuredItem::Fixed {
        w: row_w,
        h: MODAL_ROW_HEIGHT,
    }));
    let rows = Column {
        gap: 0.0,
        ..Column::default()
    }
    .lay_out_measured(inner, &flow, m, fs, family, 12.0)
    .iter()
    .skip(if tab_def.theme_cards { 2 } else { 0 })
    .enumerate()
    .map(|(i, r)| (effective_rows[i], [r.x, r.y, r.w, r.h]))
    .collect();
    ModalLayout {
        rect,
        mode,
        nav_w,
        nav_items,
        title_rect,
        content_rect,
        rows,
        theme_cards: card_strip,
        hint_rect,
        // W-a: предел честный и для legacy-входа (сам вход скролл не
        // применяет — offset 0, клипа нет; см. modal_layout_scrolled_with)
        scroll: 0.0,
        scroll_max: modal_scroll_max(tab, viewport),
    }
}

/// Высота зоны контента правой панели (content_rect) по высоте модалки:
/// модалка минус паддинги, (Compact/Mobile — таб-бар и зазор SPACING_MD),
/// заголовок раздела и зазор под ним. Единый источник формулы для потока
/// зоны в [`modal_layout_with`] и для предела прокрутки — расхождение двух
/// копий формулы сломало бы кламп скролла.
fn modal_content_h(modal_h: f32, mode: ModalMode) -> f32 {
    // Компакт/мобайл: таб-бар MODAL_TABBAR_H + зазор до заголовка SPACING_MD.
    let tabbar = match mode {
        ModalMode::Desktop => 0.0,
        ModalMode::Compact | ModalMode::Mobile => MODAL_TABBAR_H + canvas_core::tokens::SPACING_MD,
    };
    (modal_h - MODAL_PADDING * 2.0 - tabbar - MODAL_TITLE_HEIGHT - 4.0).max(0.0)
}

/// Полная высота потока контента таба (карточки темы + зазор? + ряды).
/// Те же константы скелета, из которых [`modal_layout_with`] строит
/// `flow` (все дети — [`MeasuredItem::Fixed`], замерщик в геометрии не
/// участвует), поэтому сумма сходится с раскладкой дословно.
fn modal_content_flow_h(tab_def: &SettingsTab) -> f32 {
    let prefix = if tab_def.theme_cards {
        MODAL_THEME_CARD_H + MODAL_THEME_GAP
    } else {
        0.0
    };
    prefix + tab_def.rows.len() as f32 * MODAL_ROW_HEIGHT
}

/// FR-LLM-FIX: полная высота потока контента AI-таба с учётом фильтрации
/// BYOK-моделей (строка скрыта → высота 0). Используется в
/// [`modal_layout_scrolled_with`] для клампа скролла, чтобы зона рядов
/// точно отражала видимые строки.
fn modal_content_flow_h_filtered(tab: usize, settings_filter: Option<&Settings>) -> f32 {
    let tab_def = SETTINGS_TABS.get(tab).unwrap_or(&SETTINGS_TABS[0]);
    let prefix = if tab_def.theme_cards {
        MODAL_THEME_CARD_H + MODAL_THEME_GAP
    } else {
        0.0
    };
    let count = if tab == 8 {
        if let Some(s) = settings_filter {
            ai_tab_visible_rows(s).len()
        } else {
            tab_def.rows.len()
        }
    } else {
        tab_def.rows.len()
    };
    prefix + count as f32 * MODAL_ROW_HEIGHT
}

/// Предел вертикальной прокрутки контента правой панели (W-a, дефект
/// адаптива №2: на 800×560 таб «Профиль» переливается за низ модалки):
/// полная высота потока минус высота зоны рядов (content_rect минус
/// внутренний паддинг — прокрученные ряды не наезжают на поля); 0 — всё
/// влезает. Чистая функция без замерщика — для колеса ввода: `scroll_top`
/// потребителя клампится именно в этот предел.
pub fn modal_scroll_max(tab: usize, viewport: [f32; 2]) -> f32 {
    let tab_def = SETTINGS_TABS.get(tab).unwrap_or(&SETTINGS_TABS[0]);
    let mode = modal_mode(viewport);
    let zone_h = (modal_content_h(modal_size(viewport)[1], mode) - MODAL_PADDING * 2.0).max(0.0);
    (modal_content_flow_h(tab_def) - zone_h).max(0.0)
}

/// FR-LLM-FIX: предел прокрутки с учётом фильтрации строк AI-таба
/// (видимые BYOK-модели). App-side обёртка над [`modal_scroll_max`].
pub fn modal_scroll_max_filtered(tab: usize, viewport: [f32; 2], settings: &Settings) -> f32 {
    let mode = modal_mode(viewport);
    let zone_h = (modal_content_h(modal_size(viewport)[1], mode) - MODAL_PADDING * 2.0).max(0.0);
    (modal_content_flow_h_filtered(tab, Some(settings)) - zone_h).max(0.0)
}

/// [`modal_layout_with`] с вертикальной прокруткой контента правой панели
/// (W-a). `scroll_top` клампится через kit [`canvas_ui::kit::ScrollState`]
/// в `[0, modal_scroll_max]`; ряды и карточки сдвигаются на применённый
/// offset и клипаются пересечением с зоной рядов (семантика Table v2 —
/// «list_rows + пересечение»: частичный ряд на краю остаётся видимой
/// частью, элемент целиком вне зоны выпадает из выборки — не рисуется и
/// не пикуется, как в `autolink_ui::rows_layout`).
///
/// Рисование и hit-тест обязаны брать ОДНУ эту функцию — «ввод = тому,
/// что видно» (ui_registry): rect'ы строк/контролов у потребителя
/// согласованы с видимой прокруткой автоматически.
///
/// [`modal_layout_with`] оставлен без сдвига и клипа как вход действующих
/// потребителей: пока ввод/рисование не переведены на прокрутку, клип при
/// offset = 0 «съел» бы переливающий хвост таба без возможности
/// доскроллить. После проводки потребители переходят сюда, и legacy-вход
/// с клипом не нужен.
pub fn modal_layout_scrolled_with(
    tab: usize,
    viewport: [f32; 2],
    scroll_top: f32,
    m: &mut canvas_ui::measure::TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    settings_filter: Option<&Settings>,
) -> ModalLayout {
    use canvas_ui::geometry::{EdgeInsets, UiRect};
    use canvas_ui::kit::ScrollState;

    // База — та же раскладка скелета (навигация/заголовок/зона) без
    // прокрутки: сдвигается и клипается только поток контента правой
    // панели, навигация и заголовок остаются на месте.
    let base = modal_layout_with(tab, viewport, m, fs, settings_filter);
    // Зона рядов: контент-зона минус внутренний паддинг (те же поля, что
    // у потока строк в modal_layout_with).
    let rows_viewport = UiRect::new(
        base.content_rect[0],
        base.content_rect[1],
        base.content_rect[2],
        base.content_rect[3],
    )
    .inset(&EdgeInsets::uniform(MODAL_PADDING));
    // FR-LLM-FIX: высота контента — с учётом фильтрации AI-таба.
    let content_h = modal_content_flow_h_filtered(tab, settings_filter);
    let mut scroll = ScrollState {
        offset: scroll_top,
        content_h,
        viewport_h: rows_viewport.h,
    };
    scroll.clamp();
    // Сдвиг вверх + клип пересечением; пустое пересечение — элемент не
    // виден и из выборки выпадает.
    let clip = |rect: [f32; 4]| -> Option<[f32; 4]> {
        let shifted = UiRect::new(rect[0], rect[1] - scroll.offset, rect[2], rect[3]);
        let visible = shifted.intersection(&rows_viewport)?;
        Some([visible.x, visible.y, visible.w, visible.h])
    };
    let rows = base
        .rows
        .iter()
        .filter_map(|(row, rect)| clip(*rect).map(|clipped| (*row, clipped)))
        .collect();
    // Вынесенная за зону карточка — пустой rect: та же конвенция «карточек
    // нет», что у табов без карточек (hit-тест молчит).
    let no_card = [0.0; 4];
    let theme_cards = [
        clip(base.theme_cards[0]).unwrap_or(no_card),
        clip(base.theme_cards[1]).unwrap_or(no_card),
    ];
    ModalLayout {
        // Применённый (после клампа) offset и предел — потребители ведут
        // состояние по применённому значению, а не по запросу.
        scroll: scroll.offset,
        scroll_max: scroll.max_offset(),
        rows,
        theme_cards,
        ..base
    }
}

/// [`modal_layout_scrolled_with`] с собственным замерщиком (канонические
/// shared-точки на вызов — см. [`modal_layout`]).
pub fn modal_layout_scrolled(tab: usize, viewport: [f32; 2], scroll_top: f32) -> ModalLayout {
    let mut m = canvas_ui::measure::TextMeasurer::new();
    let mut fs = canvas_render::text::measure_font_system();
    // FR-LLM-FIX: None → без фильтрации (тесты / legacy-вызовы).
    modal_layout_scrolled_with(tab, viewport, scroll_top, &mut m, &mut fs, None)
}

/// FR-LLM-FIX: App-side обёртка со скроллом + настройками для фильтрации
/// AI-таба. Семантика та же, что у [`modal_layout_scrolled`], но AI-таб
/// фильтрует BYOK-модель (строка видна только когда провайдер = BYOK).
pub fn modal_layout_scrolled_with_settings(
    tab: usize,
    viewport: [f32; 2],
    scroll_top: f32,
    settings: &Settings,
) -> ModalLayout {
    let mut m = canvas_ui::measure::TextMeasurer::new();
    let mut fs = canvas_render::text::measure_font_system();
    modal_layout_scrolled_with(tab, viewport, scroll_top, &mut m, &mut fs, Some(settings))
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

/// Геометрия выпадающего меню `[x, y, w, h]` (W-c: раскладка и кламп — kit
/// `canvas_ui::kit::dropdown_menu`, «якорь + flip»): ниже контрола-якоря;
/// не влезает снизу — НАД якорем; не влезает и сверху — прижато к низу
/// вьюпорта; по горизонтали зажато во вьюпорт; финальная гарантия — kit
/// `viewport_clamp` (меню не выходит за вьюпорт). Вьюпорт сжат на поля
/// [`DROPDOWN_MARGIN`] — прежние клампы к краям окна дословно (паттерн
/// `hints_ui::popup_rect`/`palette::margin_viewport`). Ширина — параметр
/// с прежним минимумом 80; кит поднимает меню до ширины якоря (width ≥
/// anchor.w) — в модалке параметр равен ширине контрола (FR-039 §1).
/// Высота — прежняя: `count·[`DROPDOWN_ROW_H`] + 2·[`DROPDOWN_MARGIN`]`,
/// пункты внутри меню считаются от верхнего поля ([`dropdown_item_at`]
/// согласован). Логика опций/применения ([`dropdown_options`]/[
/// `apply_dropdown_value`]) не тронута. `count == 0` — пустой rect.
pub fn dropdown_layout(anchor: [f32; 4], viewport: [f32; 2], count: usize, width: f32) -> [f32; 4] {
    if count == 0 {
        return [0.0; 4];
    }
    use canvas_ui::geometry::{UiRect, UiVec2};
    // Вьюпорт, сжатый на поля DROPDOWN_MARGIN, — слот клампов kit dropdown_menu.
    let inset = UiRect::new(
        DROPDOWN_MARGIN,
        DROPDOWN_MARGIN,
        (viewport[0] - DROPDOWN_MARGIN * 2.0).max(0.0),
        (viewport[1] - DROPDOWN_MARGIN * 2.0).max(0.0),
    );
    let height = count as f32 * DROPDOWN_ROW_H + DROPDOWN_MARGIN * 2.0;
    let content = UiVec2::new(width.max(80.0), height);
    let anchor_rect = UiRect::new(anchor[0], anchor[1], anchor[2], anchor[3]);
    let menu = canvas_ui::kit::dropdown_menu(anchor_rect, inset, content).menu;
    [menu.x, menu.y, menu.w, menu.h]
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
                SettingsRow::ExplainDepthLimit,
                SettingsRow::ExplainCoverage
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
        // FR-073: таб «Драг» — расталкивание и параметры физики
        assert_eq!(
            SETTINGS_TABS[3].rows,
            &[
                SettingsRow::DragPushEnabled,
                SettingsRow::DragPushSafeGap,
                SettingsRow::DragPushHalo,
                SettingsRow::DragPushPredictive,
                SettingsRow::DragPushRebase
            ]
        );
        assert_eq!(
            SETTINGS_TABS[4].rows,
            &[
                SettingsRow::EdgesAvoid,
                SettingsRow::PortZone,
                SettingsRow::LinePorts,
                SettingsRow::FocusMode,
                SettingsRow::EdgeAggregation,
                SettingsRow::AutolinkEnabled
            ]
        );
        // FR-079 (S3): таб 5 — «Подсказки» (мастер-тумблер + движок)
        assert_eq!(
            SETTINGS_TABS[5].rows,
            &[SettingsRow::SuggestEnabled, SettingsRow::SuggestEngine]
        );
        // FR-087: таб 6 — «Профиль» (роль + согласия FR-089 + фильтры категорий)
        assert_eq!(
            SETTINGS_TABS[6].rows,
            &[
                SettingsRow::Role,
                SettingsRow::TelemetryCounter,
                SettingsRow::TelemetryAnalytics,
                SettingsRow::TplCatBackend,
                SettingsRow::TplCatNetwork,
                SettingsRow::TplCatUnitEconomics,
                SettingsRow::TplCatProductAnalytics,
                SettingsRow::SchemeCatArchitecture,
                SettingsRow::SchemeCatBusiness,
                SettingsRow::SchemeCatFramework,
                SettingsRow::SchemeCatPlanning,
                SettingsRow::SchemeCatOnboarding
            ]
        );
        assert_eq!(
            SETTINGS_TABS[7].rows,
            &[
                SettingsRow::ThemePreset,
                SettingsRow::IconStyle,
                SettingsRow::Language
            ]
        );
        // FR-LLM-B / PRD-0010 F-7: 9-й таб «AI и модели» — 14 строк (3
        // провайдера + 3 per-feature BYOK-модели + вход ChatGPT + API-ключ +
        // 2 лимита + residency + 2 self-hosted + телеметрия). FR-LLM-FIX:
        // было 10 (без API-ключа и self-hosted), стало 13 (с API-ключом BYOK
        // и self-hosted endpoint — видны по фильтру); FR-LLM-OAUTH-APP:
        // +1 — строка «Вход ChatGPT» (видна при ChatGptOAuth в graph/agent).
        assert_eq!(
            SETTINGS_TABS[8].rows,
            &[
                SettingsRow::AiProvSuggest,
                SettingsRow::AiModelSuggest,
                SettingsRow::AiProvGraph,
                SettingsRow::AiModelGraph,
                SettingsRow::AiProvAgent,
                SettingsRow::AiModelAgent,
                // FR-LLM-OAUTH-APP: «Вход ChatGPT» (видно при ChatGptOAuth).
                SettingsRow::AiOAuth,
                // FR-LLM-FIX: API-ключ BYOK (видно при ANY BYOK).
                SettingsRow::AiApiKey,
                SettingsRow::AiCostLimit,
                SettingsRow::AiConfidenceThreshold,
                SettingsRow::AiResidency,
                // FR-LLM-FIX: self-hosted endpoint (видно при SelfHosted).
                SettingsRow::AiSelfhostUrl,
                SettingsRow::AiSelfhostKey,
                SettingsRow::AiTelemetry,
            ]
        );
    }

    /// FR-LLM-OAUTH-APP / PRD-0010 F-5.8: фильтр AI-таба — строка «Вход
    /// ChatGPT» видна только при ChatGptOAuth в provider_graph/provider_agent
    /// (для suggest OAuth недоступен — Q2). Модель-строки по-прежнему
    /// привязаны к BYOK, API-ключ — к ANY BYOK; комбинации независимы.
    #[test]
    fn ai_tab_shows_oauth_row_only_for_chatgpt_provider() {
        // ChatGptOAuth для agent — строка видна (модель-строка agent скрыта:
        // модель подписки не текстовым вводом).
        let mut settings = Settings::default();
        settings.llm.provider_agent = canvas_llm::LlmProviderId::ChatGptOAuth;
        let rows = ai_tab_visible_rows(&settings);
        assert!(rows.contains(&SettingsRow::AiOAuth));
        assert!(!rows.contains(&SettingsRow::AiModelAgent));
        assert!(!rows.contains(&SettingsRow::AiApiKey));
        // ChatGptOAuth для graph — тоже видна.
        let mut settings = Settings::default();
        settings.llm.provider_graph = canvas_llm::LlmProviderId::ChatGptOAuth;
        assert!(ai_tab_visible_rows(&settings).contains(&SettingsRow::AiOAuth));
        // ChatGptOAuth для suggest НЕ открывает строку (OAuth недоступен
        // для suggest, Q2).
        let mut settings = Settings::default();
        settings.llm.provider_suggest = canvas_llm::LlmProviderId::ChatGptOAuth;
        assert!(!ai_tab_visible_rows(&settings).contains(&SettingsRow::AiOAuth));
        // Без ChatGPT-провайдеров строки нет.
        assert!(!ai_tab_visible_rows(&Settings::default()).contains(&SettingsRow::AiOAuth));
        // Совместно с BYOK: и OAuth-строка, и API-ключ, и модель agent.
        let mut settings = Settings::default();
        settings.llm.provider_agent = canvas_llm::LlmProviderId::ChatGptOAuth;
        settings.llm.provider_graph = canvas_llm::LlmProviderId::Byok;
        let rows = ai_tab_visible_rows(&settings);
        assert!(rows.contains(&SettingsRow::AiOAuth));
        assert!(rows.contains(&SettingsRow::AiApiKey));
        assert!(rows.contains(&SettingsRow::AiModelGraph));
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
                // PRD-0007 (X6, F-12): индикатор покрытия цепочками — булево
                SettingsRow::ExplainCoverage => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.explain_coverage;
                }
                // FR-079 (S3): мастер-тумблер подсказок — булево
                SettingsRow::SuggestEnabled => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.suggest.enabled;
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
                // FR-073: тумблеры расталкивания — булевы поля Settings
                SettingsRow::DragPushEnabled => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.drag_push_enabled;
                }
                SettingsRow::DragPushPredictive => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.drag_push_predictive;
                }
                SettingsRow::DragPushRebase => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.drag_push_rebase;
                }
                // FR-087: тумблеры категорий — булева видимость
                // (`Settings::template/scheme_category_visible`), не
                // прямое bool-поле; рендер читает предикат на кадре
                SettingsRow::TplCatBackend
                | SettingsRow::TplCatNetwork
                | SettingsRow::TplCatUnitEconomics
                | SettingsRow::TplCatProductAnalytics
                | SettingsRow::SchemeCatArchitecture
                | SettingsRow::SchemeCatBusiness
                | SettingsRow::SchemeCatFramework
                | SettingsRow::SchemeCatPlanning
                | SettingsRow::SchemeCatOnboarding => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                }
                // FR-089: согласия телеметрии — прямые bool-поля Settings
                SettingsRow::TelemetryCounter
                | SettingsRow::TelemetryAnalytics => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.telemetry_counter;
                    let _ = defaults.telemetry_analytics;
                }
                // FR-LLM-B: AI-телеметрия — bool-поле LlmSettings (Toggle);
                // остальные строки таба «AI и модели» — Dropdown (проверка
                // ниже в отдельном тесте `ai_tab_dropdowns_roundtrip`).
                SettingsRow::AiTelemetry => {
                    assert_eq!(row_kind(row), RowKind::Toggle);
                    let _ = defaults.llm.telemetry_opt_in;
                }
                // FR-LLM-OAUTH-APP: строка «Вход ChatGPT» — Button (кнопка
                // действия + бейдж состояния, без текстового поля).
                SettingsRow::AiOAuth => {
                    assert_eq!(row_kind(row), RowKind::Button);
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
                | SettingsRow::SnapCoarseZoom
                | SettingsRow::DragPushSafeGap
                | SettingsRow::DragPushHalo
                | SettingsRow::IconStyle
                // FR-079 (S3): dropdown «Движок подсказок»
                | SettingsRow::SuggestEngine
                // FR-087: dropdown «Роль» (реестр canvas_core::roles)
                | SettingsRow::Role
                // FR-LLM-B: dropdown-строки таба «AI и модели»
                | SettingsRow::AiProvSuggest
                | SettingsRow::AiProvGraph
                | SettingsRow::AiProvAgent
                | SettingsRow::AiResidency
                | SettingsRow::AiConfidenceThreshold
                | SettingsRow::AiCostLimit => {
                    assert_eq!(row_kind(row), RowKind::Dropdown);
                }
                // FR-LLM-FIX (task FIX-TEXT-INPUT): per-feature BYOK-модель
                // и API-ключ/self-hosted URL/self-hosted key — редактируемые
                // текстовые поля (RowKind::TextInput), не dropdown/button.
                SettingsRow::AiModelSuggest
                | SettingsRow::AiModelGraph
                | SettingsRow::AiModelAgent
                | SettingsRow::AiApiKey
                | SettingsRow::AiSelfhostUrl
                | SettingsRow::AiSelfhostKey => {
                    assert_eq!(row_kind(row), RowKind::TextInput);
                }
            }
        }
    }

    /// FR-087: инварианты таба «Профиль» — dropdown роли синхронен с
    /// реестром (порядок/отметка/применение), смена роли сбрасывает
    /// материализованные фильтры категорий, тумблеры категорий читают
    /// эффективную видимость (роль > ручной override).
    #[test]
    fn role_dropdown_and_category_filters() {
        // Реестр ролей: dropdown = ROLES по порядку, отметка — по id
        let mut settings = Settings::default();
        assert_eq!(settings.role, canvas_core::roles::DEFAULT_ROLE);
        let options = dropdown_options(SettingsRow::Role, &settings);
        assert_eq!(options.len(), canvas_core::roles::ROLES.len());
        assert!(options[0].1, "отмечен default");
        assert_eq!(options[0].0, "Не выбрана");
        // Применение по индексу (порядок = dropdown_options)
        let architect = canvas_core::roles::ROLES
            .iter()
            .position(|r| r.id == "architect")
            .expect("architect в реестре");
        apply_dropdown_value(&mut settings, SettingsRow::Role, architect);
        assert_eq!(settings.role, "architect");
        assert!(settings.template_category_visible("backend"));
        assert!(!settings.template_category_visible("unit-economics"));
        // Смена роли с материализованным фильтром — фильтр сбрасывается
        settings.template_categories = Some(vec!["network".to_owned()]);
        let developer = canvas_core::roles::ROLES
            .iter()
            .position(|r| r.id == "developer")
            .expect("developer в реестре");
        apply_dropdown_value(&mut settings, SettingsRow::Role, developer);
        assert!(settings.template_categories.is_none(), "override сброшен");
        assert!(settings.template_category_visible("network"));
        // Индекс вне диапазона — без изменений
        apply_dropdown_value(&mut settings, SettingsRow::Role, 99);
        assert_eq!(settings.role, "developer");
        // dropdown_value — имя роли в языке интерфейса
        assert_eq!(
            dropdown_value(SettingsRow::Role, &settings).as_deref(),
            Some("Разработчик")
        );
        let mut en = settings.clone();
        en.language = Language::En;
        assert_eq!(
            dropdown_value(SettingsRow::Role, &en).as_deref(),
            Some("Developer")
        );
        // Категории таба: лейблы/описания непусты (инвариант локализации
        // покрывает их в every_row_has_label_and_description); согласия
        // FR-089 — тумблеры с прямым bool-полем, дефолт вкл (чекбоксы
        // первого запуска предвыбраны, см. telemetry_consent_defaults_and_parse
        // в canvas-core)
        for row in [
            SettingsRow::TelemetryCounter,
            SettingsRow::TelemetryAnalytics,
            SettingsRow::TplCatBackend,
            SettingsRow::TplCatNetwork,
            SettingsRow::TplCatUnitEconomics,
            SettingsRow::TplCatProductAnalytics,
            SettingsRow::SchemeCatArchitecture,
            SettingsRow::SchemeCatBusiness,
            SettingsRow::SchemeCatFramework,
            SettingsRow::SchemeCatPlanning,
            SettingsRow::SchemeCatOnboarding,
        ] {
            assert_eq!(row_kind(row), RowKind::Toggle);
            assert_eq!(dropdown_options(row, &settings), Vec::new());
            assert_eq!(dropdown_value(row, &settings), None);
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
        // 320×240: целиком внутри окна; W-e: это мобайл-режим — лист
        // вьюпорт минус поля, навигация — горизонтальный таб-бар (все табы
        // достижимы hit-тестом), левой колонки и подсказки нет.
        let layout = modal_layout(0, [320.0, 240.0]);
        assert_eq!(layout.mode, ModalMode::Mobile);
        assert!(layout.rect[0] >= SETTINGS_MARGIN - 0.01);
        assert!(layout.rect[1] >= SETTINGS_MARGIN - 0.01);
        assert!(layout.rect[0] + layout.rect[2] <= 320.0 - SETTINGS_MARGIN + 0.01);
        assert!(layout.rect[1] + layout.rect[3] <= 240.0 - SETTINGS_MARGIN + 0.01);
        assert_eq!(layout.nav_w, 0.0, "одноколоночный лист");
        for (i, item) in layout.nav_items.iter().enumerate() {
            assert!(item[2] > 0.0, "слот {i} различим на 320×240");
            assert_eq!(
                modal_nav_at(&layout, [item[0] + item[2] / 2.0, item[1] + item[3] / 2.0]),
                Some(i),
                "таб {i} достижим на 320×240"
            );
        }
        // Вне диапазона таб — первый таб
        let fallback = modal_layout(99, [1920.0, 1080.0]);
        assert_eq!(fallback.rows, modal_layout(0, [1920.0, 1080.0]).rows);
    }

    // --- W-e (вариант «A», решение владельца 03.10.2026): брейкпоинты ----

    /// Пороги режима — ровно на контрольных точках, ветвление только по
    /// ширине окна (обе точки заданы по ширине, FR-039 §2).
    #[test]
    fn modal_mode_thresholds() {
        assert_eq!(modal_mode([1280.0, 800.0]), ModalMode::Desktop);
        assert_eq!(modal_mode([1920.0, 1080.0]), ModalMode::Desktop);
        assert_eq!(modal_mode([1279.0, 800.0]), ModalMode::Compact);
        assert_eq!(modal_mode([1024.0, 640.0]), ModalMode::Compact);
        assert_eq!(modal_mode([800.0, 560.0]), ModalMode::Compact);
        assert_eq!(modal_mode([768.0, 600.0]), ModalMode::Compact);
        assert_eq!(modal_mode([767.0, 600.0]), ModalMode::Mobile);
        assert_eq!(modal_mode([320.0, 240.0]), ModalMode::Mobile);
    }

    /// Desktop ≥ [`MODAL_BP_COMPACT`]: на 1280×800 раскладка прежняя —
    /// двухколоночная (вертикальная навигация слева, подсказка внизу
    /// колонки, строки в правой панели без таб-бара в потоке).
    #[test]
    fn modal_layout_desktop_two_columns_at_1280() {
        let viewport = [1280.0, 800.0];
        let layout = modal_layout(0, viewport);
        assert_eq!(layout.mode, ModalMode::Desktop);
        assert_eq!(layout.nav_w, MODAL_NAV_WIDTH);
        // Навигация вертикальна: один x, y растёт; подсказка на месте.
        let xs: Vec<f32> = layout.nav_items.iter().map(|r| r[0]).collect();
        assert!(xs.iter().all(|&x| (x - xs[0]).abs() < 0.01));
        assert!(layout.nav_items.windows(2).all(|w| w[1][1] > w[0][1]));
        assert!(layout.hint_rect[2] > 0.0, "подсказка Ctrl+, в колонке");
        // Строки — в правой колонке: левый край правее навигации, ширина
        // = колонка контента минус внутренние паддинги.
        for (_, rect) in &layout.rows {
            assert!(rect[0] > layout.rect[0] + layout.nav_w);
            assert!((rect[2] - (layout.rect[2] - layout.nav_w - MODAL_PADDING * 2.0)).abs() < 0.01);
        }
        // Контент-зона — Desktop-формула (без таб-бара).
        assert!(
            (layout.content_rect[3]
                - (layout.rect[3] - MODAL_PADDING * 2.0 - MODAL_TITLE_HEIGHT - 4.0))
                .abs()
                < 0.01
        );
    }

    /// Компакт ([`MODAL_BP_MOBILE`]..[`MODAL_BP_COMPACT`]): одноколоночно —
    /// строки на всю ширину контента, ширина модалки в клампе «вьюпорт
    /// минус поля»; таб-бар горизонтален, целиком виден и все табы
    /// достижимы hit-тестом (в т.ч. на минимальном контрольном окне
    /// 800×560 и на пороге 768).
    #[test]
    fn modal_layout_compact_single_column() {
        for viewport in [
            [1279.0, 800.0],
            [1024.0, 640.0],
            [800.0, 560.0],
            [768.0, 600.0],
        ] {
            let layout = modal_layout(0, viewport);
            assert_eq!(layout.mode, ModalMode::Compact, "{viewport:?}");
            // Одноколоночно: левой колонки нет.
            assert_eq!(layout.nav_w, 0.0, "{viewport:?}");
            assert_eq!(layout.hint_rect, [0.0; 4], "{viewport:?}");
            // Ширина модалки — в клампе «вьюпорт минус внешние поля».
            assert!(
                layout.rect[2] <= viewport[0] - SETTINGS_MARGIN * 2.0 + 0.01
                    && layout.rect[0] >= SETTINGS_MARGIN - 0.01,
                "{viewport:?}: {:?}",
                layout.rect
            );
            // Компакт — НЕ лист: на 800×560 сохраняется дизайн-минимум 560
            // (кламп к вьюпорту — верхняя граница, а не растяжение).
            if viewport[0] == 800.0 {
                assert!((layout.rect[2] - MODAL_MIN_W).abs() < 0.01, "{viewport:?}");
            }
            // Строки на всю ширину контента и внутри модали/вьюпорта
            // (допуск 0.6 ui px — round_layout нативного бэкенда, дробная
            // ширина 0.45·1279 округляется на freeze, контракт F-13).
            for (row, rect) in &layout.rows {
                assert!(
                    (rect[2] - (layout.rect[2] - MODAL_PADDING * 2.0)).abs() < 0.6,
                    "{viewport:?}: {row:?} не на всю ширину {rect:?}"
                );
                assert!(
                    rect[0] >= layout.rect[0] - 0.01
                        && rect[0] + rect[2] <= layout.rect[0] + layout.rect[2] + 0.01,
                    "{viewport:?}: {row:?} шире модалки"
                );
                assert!(
                    rect[1] + rect[3] <= viewport[1] + 0.01,
                    "{viewport:?}: {row:?} за низом вьюпорта"
                );
            }
            // Таб-бар горизонтален: один y, x растёт; заголовок — под ним.
            assert_eq!(layout.nav_items.len(), SETTINGS_TABS.len(), "{viewport:?}");
            assert!(
                layout
                    .nav_items
                    .windows(2)
                    .all(|w| (w[1][1] - w[0][1]).abs() < 0.01 && w[1][0] > w[0][0]),
                "{viewport:?}: не горизонталь"
            );
            assert!(
                layout.title_rect[1] >= layout.rect[1] + MODAL_PADDING + MODAL_TABBAR_H,
                "{viewport:?}: заголовок не под таб-баром"
            );
            // Целиком виден + достижим: слоты внутри модалки, hit-тест
            // находит каждый (тот же modal_nav_at, что у ввода).
            for (i, item) in layout.nav_items.iter().enumerate() {
                assert!(
                    item[0] >= layout.rect[0] - 0.01
                        && item[0] + item[2] <= layout.rect[0] + layout.rect[2] + 0.01
                        && item[1] >= layout.rect[1] - 0.01
                        && item[1] + item[3] <= layout.rect[1] + layout.rect[3] + 0.01,
                    "{viewport:?}: слот {i} за модалкой {item:?}"
                );
                assert!(item[2] > 0.0, "{viewport:?}: слот {i} нулевой");
                assert_eq!(
                    modal_nav_at(&layout, [item[0] + item[2] / 2.0, item[1] + item[3] / 2.0]),
                    Some(i),
                    "{viewport:?}: таб {i} недостижим"
                );
            }
        }
    }

    /// Общий оракул скролла (W-a во всех режимах): при полном offset хвост
    /// потока прижат к низу зоны рядов, каждая видимая строка (частичная —
    /// тоже) внутри зоны, доводится до последнего ряда таба.
    fn assert_full_scroll_tail(tab: usize, viewport: [f32; 2]) {
        let base = modal_layout(tab, viewport);
        let max = modal_scroll_max(tab, viewport);
        assert!(
            max > 0.0,
            "{viewport:?}: таб {tab} влезает — оракул неприменим"
        );
        let scrolled = modal_layout_scrolled(tab, viewport, max);
        assert_eq!(scrolled.mode, base.mode, "{viewport:?}");
        assert!(
            (scrolled.scroll - max).abs() < 0.01,
            "{viewport:?}: офсет зажат в предел"
        );
        let zone = canvas_ui::geometry::UiRect::new(
            base.content_rect[0],
            base.content_rect[1],
            base.content_rect[2],
            base.content_rect[3],
        )
        .inset(&canvas_ui::geometry::EdgeInsets::uniform(MODAL_PADDING));
        assert!(
            !scrolled.rows.is_empty(),
            "{viewport:?}: зона пуста при полном offset"
        );
        for (_, rect) in &scrolled.rows {
            assert!(
                rect[1] >= zone.y - 0.01 && rect[1] + rect[3] <= zone.y + zone.h + 0.01,
                "{viewport:?}: видимая строка вне зоны рядов: {rect:?}"
            );
        }
        let (last_row, last_rect) = scrolled.rows.last().expect("непусто");
        assert_eq!(
            Some(*last_row),
            SETTINGS_TABS[tab].rows.last().copied(),
            "{viewport:?}: скролл доводит до последнего ряда таба"
        );
        assert!(
            (last_rect[1] + last_rect[3] - (zone.y + zone.h)).abs() < 0.01,
            "{viewport:?}: хвост потока не прижат к низу зоны: {last_rect:?}"
        );
    }

    /// Мобайл < [`MODAL_BP_MOBILE`]: полноэкранный лист — модалка занимает
    /// вьюпорт минус внешние поля [`SETTINGS_MARGIN`]; одноколоночный скелет
    /// компакта; все табы достижимы; скролл «Профиля» (12 рядов) достижим.
    #[test]
    fn modal_layout_mobile_sheet() {
        let viewport = [767.0, 600.0];
        let layout = modal_layout(6, viewport);
        assert_eq!(layout.mode, ModalMode::Mobile);
        // Лист: вьюпорт минус внешние поля (SETTINGS_MARGIN == токен
        // SPACING_LG — внешние поля из шкалы, новых литералов нет).
        assert_eq!(
            layout.rect,
            [
                SETTINGS_MARGIN,
                SETTINGS_MARGIN,
                viewport[0] - SETTINGS_MARGIN * 2.0,
                viewport[1] - SETTINGS_MARGIN * 2.0
            ]
        );
        // Одноколоночно: левой колонки нет; строки внутри модали по ширине
        // и не шире вьюпорта (по высоте — скролл, см. оракул ниже).
        assert_eq!(layout.nav_w, 0.0);
        assert_eq!(layout.hint_rect, [0.0; 4]);
        for (row, rect) in &layout.rows {
            assert!(
                (rect[2] - (layout.rect[2] - MODAL_PADDING * 2.0)).abs() < 0.01,
                "{row:?} не на всю ширину"
            );
            assert!(
                rect[0] >= 0.0 && rect[0] + rect[2] <= viewport[0] + 0.01,
                "{row:?} за вьюпортом по ширине"
            );
        }
        // Все табы достижимы (горизонтальный таб-бар листа).
        for (i, item) in layout.nav_items.iter().enumerate() {
            assert!(
                item[0] >= layout.rect[0] - 0.01
                    && item[0] + item[2] <= layout.rect[0] + layout.rect[2] + 0.01
                    && item[1] >= layout.rect[1] - 0.01
                    && item[1] + item[3] <= layout.rect[1] + layout.rect[3] + 0.01,
                "слот {i} за модалкой: {item:?}"
            );
            assert_eq!(
                modal_nav_at(&layout, [item[0] + item[2] / 2.0, item[1] + item[3] / 2.0]),
                Some(i),
                "таб {i} недостижим"
            );
        }
        // Скролл: 12 рядов «Профиля» не влезают в лист 767×600 — офсет
        // достижим, хвост прижат к низу зоны рядов.
        assert_full_scroll_tail(6, viewport);
    }

    /// Скролл в компакт-режиме: на 800×560 «Профиль» требует прокрутки —
    /// офсет достижим, видимые строки внутри зоны (W-a без регресса).
    #[test]
    fn modal_layout_compact_scroll_reachable() {
        assert_full_scroll_tail(6, [800.0, 560.0]);
    }

    /// Скролл в Desktop без регресса: на 1280×800 «Профиль» тоже прокручивается.
    #[test]
    fn modal_layout_desktop_scroll_reachable() {
        assert_full_scroll_tail(6, [1280.0, 800.0]);
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
        // Таб 3 (Драг, FR-073): 3 тумблера + 2 dropdown-пресета
        let layout = modal_layout(3, viewport);
        assert_eq!(layout.rows.len(), 5);
        assert_eq!(layout.rows[0].0, SettingsRow::DragPushEnabled);
        // Таб 4 (Связи и порты): прежний таб 3
        let layout = modal_layout(4, viewport);
        assert_eq!(layout.rows.len(), 6);
        assert_eq!(layout.rows[0].0, SettingsRow::EdgesAvoid);
        // Таб 5 (Подсказки, FR-079): тумблер + движок
        let layout = modal_layout(5, viewport);
        assert_eq!(layout.rows.len(), 2);
        assert_eq!(layout.rows[0].0, SettingsRow::SuggestEnabled);
        assert_eq!(layout.rows[1].0, SettingsRow::SuggestEngine);
        // Таб 6 (Профиль, FR-087/FR-089): роль + 2 согласия телеметрии +
        // 9 тумблеров категорий, без карточек
        let layout = modal_layout(6, viewport);
        assert_eq!(layout.rows.len(), 12);
        assert_eq!(layout.rows[0].0, SettingsRow::Role);
        assert_eq!(layout.rows[1].0, SettingsRow::TelemetryCounter);
        assert_eq!(layout.rows[2].0, SettingsRow::TelemetryAnalytics);
        assert_eq!(layout.rows[11].0, SettingsRow::SchemeCatOnboarding);
        assert!(layout.theme_cards[0][2] <= 0.0, "карточек темы нет");
        // Таб 7 (Внешний вид): карточки темы + строки пресета, иконок и языка ниже
        let layout = modal_layout(7, viewport);
        // FR-ICONS: 3 строки — ThemePreset, IconStyle, Language.
        assert_eq!(layout.rows.len(), 3);
        assert_eq!(layout.rows[0].0, SettingsRow::ThemePreset);
        assert_eq!(layout.rows[1].0, SettingsRow::IconStyle);
        assert_eq!(layout.rows[2].0, SettingsRow::Language);
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

    /// Контролы в строках: kit-switch (трек SWITCH_W×SWITCH_H) у тумблера,
    /// dropdown-кнопка у выпадающего списка; бегунок kit-тумблера отражает
    /// состояние (вкл — справа), стиль трека — kit ControlStyle из слотов.
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
                RowKind::Toggle => assert_eq!(control[2], canvas_ui::kit::SWITCH_W),
                RowKind::Dropdown => assert_eq!(control[2], DROPDOWN_BTN_W),
                // FR-LLM-FIX: Button-строки — кнопка действия справа (та же
                // ширина DROPDOWN_BTN_W, что у dropdown — единый ритм контрола).
                RowKind::Button => assert_eq!(control[2], DROPDOWN_BTN_W),
                // FR-LLM-FIX (task FIX-TEXT-INPUT): TextInput-строки — кнопка
                // действия справа (для строк с проверкой) или пустой слот
                // (для модель-строк); геометрия та же, что у Button/Dropdown.
                RowKind::TextInput => assert_eq!(control[2], DROPDOWN_BTN_W),
            }
        }
        // W-c: трек тумблера — kit-геометрия (SWITCH_W×SWITCH_H по центру
        // строки); бегунок — kit::switch: квадрат SWITCH_H−2·SWITCH_KNOB_PAD
        // с отступом SWITCH_KNOB_PAD; выкл — слева, вкл — справа.
        let track = control_rect([100.0, 10.0, 300.0, MODAL_ROW_HEIGHT], RowKind::Toggle);
        assert_eq!(
            (track[2], track[3]),
            (canvas_ui::kit::SWITCH_W, canvas_ui::kit::SWITCH_H)
        );
        let knob_side = canvas_ui::kit::SWITCH_H - 2.0 * canvas_ui::kit::SWITCH_KNOB_PAD;
        let off = pill_knob_rect(track, false);
        let on = pill_knob_rect(track, true);
        assert_eq!((off[2], off[3]), (knob_side, knob_side));
        assert_eq!((on[2], on[3]), (knob_side, knob_side));
        assert!(off[0] < on[0], "выкл — слева, вкл — справа");
        assert_eq!(off[0], track[0] + canvas_ui::kit::SWITCH_KNOB_PAD);
        assert_eq!(
            on[0] + on[2],
            track[0] + track[2] - canvas_ui::kit::SWITCH_KNOB_PAD
        );
        assert_eq!(on[1], track[1] + canvas_ui::kit::SWITCH_KNOB_PAD);
        // Стиль трека — kit ControlStyle (радиус pill из шкалы токенов);
        // заливка бегунка — слот палитры (нейтральный срез геометрию
        // не меняет — см. SWITCH_GEOMETRY_PALETTE)
        let kit = kit_switch_layout(
            canvas_ui::geometry::UiRect::new(track[0], track[1], track[2], track[3]),
            true,
        );
        assert_eq!(kit.track_style.radius, canvas_core::tokens::RADIUS_PILL);
        assert_eq!(kit.knob_fill, SWITCH_GEOMETRY_PALETTE.text_title);
    }

    /// `dropdown_layout` (kit dropdown_menu): кламп к окну у правого края
    /// модалки и в узком окне; у нижнего края — flip выше контрола; 0
    /// пунктов — пусто; меню не уже якоря (контракт кита) и не перекрывает
    /// якорную строку целиком.
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
        // W-c: меню не уже якоря (контракт kit dropdown_menu — при ширине
        // параметра меньше якоря кит поднимает до ширины якоря); гигантская
        // ширина обрезана вьюпортом (финальная гарантия viewport_clamp)
        let menu = dropdown_layout([50.0, 20.0, 120.0, 28.0], [800.0, 600.0], 3, 100.0);
        assert_eq!(menu[2], 120.0);
        let menu = dropdown_layout([50.0, 20.0, 120.0, 28.0], [800.0, 600.0], 3, 5000.0);
        assert_eq!(menu[2], 800.0 - DROPDOWN_MARGIN * 2.0);
        // У нижнего края — меню выше контрола
        let menu = dropdown_layout([50.0, 200.0, 300.0, 28.0], narrow, 5, DROPDOWN_BTN_W);
        assert!(menu[1] + menu[3] <= 200.0, "меню выше якорного контрола");
        // Меню ниже контрола, когда влезает
        let menu = dropdown_layout([50.0, 20.0, 300.0, 28.0], [800.0, 600.0], 3, DROPDOWN_BTN_W);
        assert!(menu[1] >= 20.0 + 28.0, "меню ниже якорного контрола");
    }

    /// W-c: кламп dropdown у краёв окна — финальная гарантия `viewport_clamp`
    /// кита: якорь у правого края — меню сдвинуто влево (правый край на
    /// поле), у нижнего края — flip вверх (меню выше якоря), якорь в левом
    /// верхнем углу — меню зажато в поле окна, меню шире окна — обрезано
    /// до вьюпорта. Ни один край меню не выходит за [`DROPDOWN_MARGIN`].
    #[test]
    fn dropdown_layout_clamps_at_window_edges() {
        let viewport = [800.0, 600.0];
        let lim = DROPDOWN_MARGIN;
        // Правый край: якорь у кромки — правый край меню не правее поля окна
        let anchor = [viewport[0] - 20.0, 100.0, DROPDOWN_BTN_W, DROPDOWN_BTN_H];
        let menu = dropdown_layout(anchor, viewport, 3, DROPDOWN_BTN_W);
        assert!(menu[0] >= lim - 0.01, "левый край внутри поля");
        assert!(
            menu[0] + menu[2] <= viewport[0] - lim + 0.01,
            "правый край меню — не правее поля окна"
        );
        // Нижний край: вниз не влезает — flip, меню ВЫШЕ якоря и внутри окна
        let anchor = [
            100.0,
            viewport[1] - DROPDOWN_BTN_H,
            DROPDOWN_BTN_W,
            DROPDOWN_BTN_H,
        ];
        let menu = dropdown_layout(anchor, viewport, 3, DROPDOWN_BTN_W);
        assert!(
            menu[1] + menu[3] <= anchor[1] + 0.01,
            "меню выше якорного контрола (flip)"
        );
        assert!(menu[1] >= lim - 0.01 && menu[1] + menu[3] <= viewport[1] - lim + 0.01);
        // Левый-верхний угол: якорь левее поля — меню зажато во вьюпорт,
        // левый край — на поле (обрезка viewport_clamp кита)
        let menu = dropdown_layout(
            [0.0, 0.0, DROPDOWN_BTN_W, DROPDOWN_BTN_H],
            viewport,
            3,
            DROPDOWN_BTN_W,
        );
        assert_eq!(menu[0], lim, "левый край меню — на поле окна");
        assert!(menu[1] >= lim - 0.01);
        // Меню шире окна: обрезано до вьюпорта, края — на полях окна
        let menu = dropdown_layout([50.0, 20.0, 120.0, 28.0], viewport, 3, 5000.0);
        assert_eq!(menu[0], lim);
        assert_eq!(menu[2], viewport[0] - lim * 2.0);
        assert!(menu[1] >= lim - 0.01 && menu[1] + menu[3] <= viewport[1] - lim + 0.01);
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
