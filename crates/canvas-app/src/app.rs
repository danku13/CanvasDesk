//! `canvas_app::app` — `App` и обработчики событий (M8/W2, wasm-port §3.1).
//!
//! Вынесено из `main.rs` чистым перемещением (zero behavior change):
//! структура `App`, все `impl App` (ввод/редактирование/рендер-кадр/MCP),
//! `ApplicationHandler`, `AppEvent` и обслуживающие хелперы (геометрия
//! оверлеев, стресс-сцена, разбор CLI-аргументов, открытие файлов вовне).
//! Нативный `main.rs` — тонкая обёртка: инициализация сервисов + `run_app`
//! (§3.1 п. 2); web-бинарь `canvas-web` (W4-прошивка) соберёт свой набор
//! сервисов вокруг того же `App` — дублирования UI-логики нет.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

// Чистые UI-helpers (геометрия, hit-тесты, меню, двойной клик) — единый
// источник в библиотеке, здесь только платформенно-зависимое состояние.
use crate::docs_ui;
use crate::hints_ui;
use crate::i18n::{self, keys};
use crate::onboarding_ui::{self, OnboardingButton, OnboardingState};
use crate::palette::{
    color_to_rgba, icon_quads, icon_text, palette_bar_size, palette_groups, palette_hit,
    palette_layout, palette_origin, template_update_group, PaletteAction, PaletteHit, PaletteHover,
    PaletteLayout, PaletteTarget, PAL_ICON,
};
use crate::scheme_gallery_ui;
use crate::settings_ui::{
    apply_dropdown_value, control_rect, dropdown_item_at, dropdown_layout, dropdown_options,
    dropdown_value, modal_layout, modal_nav_at, modal_row_at, modal_theme_card_at, pill_knob_rect,
    row_desc_key, row_kind, row_label_key, DropdownState, RowKind, SettingsRow, DROPDOWN_MARGIN,
    DROPDOWN_ROW_H, MODAL_ROW_LABEL_W, SETTINGS_TABS,
};
// FR-038 (T-038.4): snap-движок (T-038.2) — чистая геометрия магнитной
// раскладки; кламп collision публичен для live-клампа кадра драга (п.15);
// T-038.5: batch-операции выделения (п.16-17) — те же чистые функции
use crate::snap::{
    align_centers, clamp_collision, distribute_evenly, effective_grid_step, snap_move, AlignAxis,
    SnapConfig, SnapOutcome, SnapRect, SnapSource,
};
use crate::template_ui;
use crate::template_ui::{
    panel_layout as template_panel_layout, panel_rows as template_panel_rows,
    row_of_ordinal as template_row_of_ordinal, split_two_lines, PanelRow, WheelHit,
};
use crate::ui::{
    button_rect, canvas_menu_label, canvas_menu_visible_items, drag_origins, focus_seed_of,
    help_button_rect, hotkeys_panel_rect_at, in_resize_corner, language_button_rect,
    menu_item_at_for, menu_item_rect, menu_rect_for, next_free_id, nodes_in_rect, paste_nodes,
    plan_group_around, plan_group_around_nodes, plan_group_at, point_in_rect, reassign_ids,
    rubber_band_rect, select_node_hit, submenu_item_at, submenu_origin_next_to, submenu_rect,
    theme_button_rect, toggle_selection_with_primary, CanvasMenuItem, ContextMenu, DoubleClick,
    DragState, EdgeDrag, PastePlacement, Submenu, SubmenuEntry, ALIGN_MIN_SELECTION,
    DUPLICATE_OFFSET, MENU_ITEM_HEIGHT, MENU_LABEL_X, MENU_PADDING, MENU_WIDTH, MIN_NODE_HEIGHT,
    MIN_NODE_WIDTH, SELECT_DRAG_THRESHOLD,
};
use crate::whatif_ui::{self, BarAction};
// PRD-0007 (FR-048 X2): окно проверки цепочки расчёта цифры — модель и
// состояния (Loading/Ready/Stale), рендер/ввод — здесь (паттерн main stage).
use crate::explain_ui::{self, ExplainBuild, ExplainSnapshot, ExplainState};
// PRD-0007 (FR-048 X4): автосвязь по именам — модель диалога ревью,
// рендер/ввод/создание связей — здесь (паттерн explain-окна).
use crate::autolink_ui::{self, ItemState, Review};
// FR-037 MW1: line_kind/NumiLineKind/ExprLineResults/ExprResults и whatif-
// типы использовались только вынесенным кодом; тестовые упоминания —
// импортами внутри mod tests
use canvas_core::expr::{self, ExprOutcome};
// FR-052 (U2 PRD-0009): каркас canvas-ui — HitStack/pick и полосы слоёв
// (ScreenBand) для единого диспетчера ввода/отрисовки
use canvas_core::flow::{self, FlowKind};
use canvas_core::time::Instant;
use canvas_core::{
    analyze, apply_file_events, bundle_thickness, edge_at, focus_set, main_stage_rect,
    nearest_side, path_matches, port_at, resolve_node_path, stage_edge_at_lines,
    stage_edge_geometry, stage_layout, watched_dirs, AnalysisState, Canvas, CanvasStorage,
    ClipboardBackend, DragPushParams, DragPushState, Edge, FileEvent, FocusSeed, FocusSet,
    GridStyle, Language, LineageNodeId, Node, NodeChange, NodeKind, Priority, SearchBackend,
    Settings, Side, SnapAnchor, SpatialIndex, StageLayout, StageMetrics, Theme, ThumbBackend,
    WatchBackend, COLLISION_GAP,
};
use canvas_ui::geometry::UiPoint;
// FR-060 (волна 2 кита): геометрия поверхностей волны 2 — модули кита
// (modal/button_size/list_rows) поверх Painter/WidgetState волны 1
use canvas_ui::kit;
// FR-059 (волна 1 кита): draw-слой Painter (canvas-ui, G7 — данные) +
// машина состояний виджета WidgetState — поверхности волны 1 рисуются
// через Painter, состояния — через KitState (0 ручных матриц)
use canvas_ui::paint::{PaintAlign, PaintItem, Painter};
use canvas_ui::widget::WidgetState;
use canvas_ui::{HitStack, HitTarget, UiLayer};
// FR-044 Р-1 (стык раскладок): лейн-раскладка пилюль подписей веера —
// чистые функции core с инвариантами (без пересечений, кламп в зону).
use canvas_core::bundles::{fan_corridor, stage_fan_label_layout, Rect as StageLocalRect};
// M8/W3 (wasm-port §3.1): протокол поиска переехал в core (натив — FTS5 в
// shell, web/тесты — MemSearch); App общается только через трейт SearchBackend
use canvas_core::search::{SearchCommand, SearchEvent, SearchHit};
// FR-037/ADR-0012 (MW1): модельный слой сцены + MCP-инструменты — вынесены
// из main.rs в крейт canvas-scene (wasm-верификация); UI-поля ввода
// (selected/selected_nodes/dragging) остались здесь, в App
use canvas_render::animate::{
    ease_out_cubic, focus_fade, focus_pulse, pulse_alpha, Flight, FLIGHT_DURATION_MS,
    FOCUS_FADE_MS, FOCUS_PULSE_MS, SHOW_SOURCE_MS, SPILL_WAVE_EDGE_MS, SPILL_WAVE_STEP_MS,
};
use canvas_render::camera::Vec2;
use canvas_render::cards::{
    build_stage_edge_instances_with_alpha, card_instance, drop_ghost, template_band_instance,
    template_icon_quads, title_for, BundleContext, CardInstance, FocusView, SpillWaveView,
    EDGE_COLOR, FLOW_EDGE_COLOR, HEADER_HEIGHT, SELECTION_BORDER, UNMAPPED_EDGE_COLOR,
};
use canvas_render::edit::{
    edge_edit_area, map_key, session_area, EditTarget, EditingSession, KeyCommand,
};
// FR-038: кадр направляющих рендера — конвертация SnapOutcome (T-038.3)
use canvas_render::guides::{GuideSource, GuidesFrame};
use canvas_render::minimap::{Minimap, MINIMAP_H, MINIMAP_W};
// M8/W4 (wasm-port §3.4): стратегия запуска async-инициализации Renderer —
// инъекция (натив: pollster::block_on, web: spawn_local + слот доставки),
// паттерн W3-сервисов: платформенный выбор в точке сборки бинарника
use crate::flowmap_ui;
// FR-044 Р-4/Р-5: панель «Как считается» main stage — модель/раскладка/hit
// (чистый модуль); состояние подсветки — StageCalcFocus в App.
use crate::calc_panel_ui;
use crate::calc_panel_ui::{layout as calc_panel_layout, PanelValues, RowValue, StageCalcFocus};
use canvas_render::renderer_init::{RendererLaunch, RendererLauncher, RendererSlot};
use canvas_render::search_ui::{
    layout as search_layout, scan_scene, PanelAction, SceneEntry, SearchInput, SearchPanel,
    SearchRow,
};
use canvas_render::sectors::SectorInstance;
use canvas_render::text::{
    body_area, measure_body_height, BodyHit, BodyHitKind, LineErrorHit, OverlayText, ScreenText,
    SpillHit, SpillHitKind, TextAlign, BODY_LINE_HEIGHT, BODY_PADDING, BODY_TOP_GAP,
    RESULT_LINE_HEIGHT,
};
use canvas_render::ThemeColors;
use canvas_render::{
    Camera, Color, FrameMeter, FrameOverlay, FrameStats, ParamDropView, SceneView, Selection,
    SpillView, StageTransform,
};
use canvas_scene::{
    fit_template_node_height, formula_line_indices, split_formula_lines, SceneState,
};

/// FR-052 (этап U2 PRD-0009): реестр поверхностей экрана — единый диспетчер.
/// Дочерний модуль `app`: доступ к приватным полям `App` (снимок состояния
/// на кадр). Декларации поверхностей (слой/capture/scope/деградация),
/// сборка `UiFrame` (hit-rect'ы из тех же layout-функций, что у ввода и
/// отрисовки), владелец клавиатуры из `esc_stack`, draw-полосы.
pub mod ui_registry;
// FR-054 (U5 PRD-0009, F-11): сквозной layout-линт полного кадра — CI-гейт G4.
#[cfg(test)]
mod ui_layout_lint;

mod explain;
/// ApplicationHandler (event loop winit) — дочерний модуль (этап 4 рефакторинга 2026-09-24).
mod handler;
/// Обработка ввода — дочерний модуль (этап 3 рефакторинга 2026-09-24).
mod input;
/// Оверлеи приложения — дочерний модуль (этап 2 рефакторинга 2026-09-24).
mod overlays;
mod stage;
/// Чистые helper-функции app — дочерний модуль (этап 1 рефакторинга
/// 2026-09-24): геометрия/текст/снап/линяж explain вынесены из app.rs,
/// поведение без изменений. Обратный импорт — явным списком.
mod support;

// Тултипы живого канваса: непрозрачная подложка + перенос по словам
// (правка владельца 2026-09-26); замер — TextMeasurer/measure_font_system
// в коротком скоупе (контракт «вложенный лок FontSystem запрещён»).
mod tooltip;
pub use support::measured_result_reserve_height;
use support::{
    bezier_samples, centered_box, dim_color4, dim_text_color, distribute_axis_for, drag_bbox,
    drag_from_node, explain_chain_focus, expr_error_hit_at, hit_subtitle, hover_fill,
    infer_param_type, node_display_label, node_subtitle, node_text, node_title, nudge_step_world,
    paint_items_to_band, paint_items_to_stage, rect_xywh, rects_intersect, screen_dot,
    screen_rect_quad, slugify, snap_candidates, snap_tolerance_world, snap_with_anchor,
    spawn_lineage_build, spill_hit_at, spill_hit_target, spill_toast_key, stage_close_button_rect,
    template_card_row, token_color, truncate_chars, unique_custom_id, BatchOp, PortLabelLine,
    PortTarget, SnapFrame,
};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};

use winit::window::{CursorIcon, Window, WindowId};
// Атрибуты окна Windows: отключение своего IDropTarget у winit (T9, план §3)
#[cfg(windows)]
use winit::platform::windows::WindowAttributesExtWindows;
// FR-037 MW1: конверт MCP-приложения (on_mcp_wake) — Windows-pipe-ветка
#[cfg(windows)]
use canvas_scene::{mcp_dispatch, mcp_unwrap_call, Viewport};

/// Множитель зума на одну строку колеса мыши (Ctrl+колесо, SPEC §8).
const ZOOM_STEP_PER_LINE: f32 = 1.1;
/// Пикселей панорамирования на строку колеса без Ctrl (скролл тачпада).
const PAN_PX_PER_LINE: f32 = 40.0;
// Ширина клип-бокса тултипа снята (правка владельца 2026-09-26):
// тултипы — на измеренной непрозрачной подложке с переносом ≤ 10 слов
// в строке (app::tooltip), фиксированный клип 380 px больше не нужен.

// FR-060: измеренная геометрия диалога подтверждения — пады/кегли
// прежней раскладки дословно (см. [`App::dialog_measured_in`])
const DIALOG_PAD_X: f32 = 20.0;
const DIALOG_TITLE_Y: f32 = 16.0;
const DIALOG_BODY_Y: f32 = 46.0;
const DIALOG_BTN_BOTTOM: f32 = 16.0;
const DIALOG_BTN_H: f32 = kit::BUTTON_HEIGHT;
const DIALOG_BTN_GAP: f32 = 16.0;
const DIALOG_TITLE_FS: f32 = 16.0;
const DIALOG_BODY_FS: f32 = 13.0;
const DIALOG_BTN_FS: f32 = 14.0;
const DIALOG_MIN_W: f32 = 280.0;
const DIALOG_MAX_W: f32 = 440.0;
const DIALOG_VIEWPORT_MARGIN: f32 = 40.0;

/// FR-050 Н2 (этап C): высота строки заголовка меню выбора (screen-space,
/// логические px) — пункты сдвинуты ниже заголовка (клик по заголовку —
/// «мимо пункта» — отменяет меню).
const CHOICE_MENU_TITLE_H: f32 = 24.0;

/// Debounce запроса поиска (T14, план §3).
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
/// Лимит строк FTS-запроса (T14): в 2 р больше видимых — запас под скролл.
const SEARCH_RESULTS_LIMIT: usize = 16;

/// Screen-space текст с владеемой строкой (панель настроек): промежуточное
/// представление, конвертируется в `ScreenText` на кадр рендера.
pub(crate) struct OwnedScreenText {
    pub(crate) text: String,
    pub(crate) origin: [f32; 2],
    pub(crate) width: f32,
    pub(crate) font_size: f32,
    pub(crate) color: Color,
    /// Выравнивание в области `width` (иконки кнопок — по центру).
    pub(crate) align: TextAlign,
}

// --- PRD-0007 (FR-048 X2): хелперы кадра окна проверки ----------------------

/// PRD-0007 (FR-048 X4, AC-5.3): тег undo-снапшота пачки автосвязи —
/// по нему undo_action узнаёт, что следующий откат требует подтверждения
/// с подсветкой отменяемого (решение владельца, раунд 2).
const AUTOLINK_UNDO_TAG: &str = "autolink_batch";
/// PRD-0007 (FR-048 X4, AC-5.5): дебаунс фонового скана автосвязи после
/// правок модели (мс) — бурст правок считается одной сессией.
const AUTOLINK_DEBOUNCE_MS: u128 = 700;

/// Правка дрейфа 2026-09-25: квад полосы кадра из screen-прямоугольника — СЫРЫЕ логические
/// px (конвенция полос: единственный screen→world делает рендер —
/// `renderer::screen_instance_to_world` текущей камерой). Зеркало
/// Painter-пути `app::support::paint_items_to_band`; КОНВЕРТАЦИЮ ЗДЕСЬ
/// ДЕЛАТЬ ЗАПРЕЩЕНО — двойной screen→world сдвигает квад на
/// `P + (s − V/2)/z` относительно screen-текстов той же полосы (дрейф
/// панелей при панорамировании/зуме — FR-059 worklog, FR-070 приёмка).
pub(crate) fn band_rect_quad_pub(
    rect: [f32; 4],
    fill: [f32; 4],
    border: [f32; 4],
    radius: f32,
) -> CardInstance {
    CardInstance {
        pos: [rect[0], rect[1]],
        size: [rect[2], rect[3]],
        fill,
        border,
        params: [radius, 0.0, 0.0, 1.0],
        corners: [0.0; 4],
    }
}

// --- FR-038 (T-038.4): интеграция магнитной раскладки ----------------------
//
// Семантика v2: во время drag нода следует курсору свободно (origin+delta),
// snap-движок работает на ПРЕДПРОСМОТР (направляющие + ghost, каждый кадр
// драга) и НА ОТПУСКАНИИ (п.2), collision-avoidance — LIVE (кламп дельты
// каждого кадра, п.15), anchor (п.4) — надстройка над grid-частью движка.

// Буфер обмена ОС (T7, arboard) — M8/W3: за трейтом `ClipboardBackend`
// (нативная реализация — `canvas_shell::clipboard::ArboardClipboard`,
// web — navigator.clipboard); инъекция — в `App::new`.

/// Пользовательские события event loop (T6): worker-потоки ThumbService
/// будят цикл через EventLoopProxy, когда готовы тамбнейлы; shell шлёт
/// события drag-drop (T9) и файлового вотчера (T10).
pub enum AppEvent {
    /// В канале ThumbService появились результаты — забрать и перерисовать.
    ThumbsReady,
    /// Событие drag-drop из IDropTarget (T9): Enter/Over/Leave/Drop.
    Drag(canvas_core::dragdrop::DragEvent),
    /// Батч событий файловой системы от WatchService (T10): debounce 300 мс
    /// уже отработан в shell, здесь — применение к модели и кэшам.
    FileEvents(Vec<FileEvent>),
    /// События поискового индекса (T14): ответы worker-потока FTS5
    /// (результаты запроса / завершение индексации).
    Search(SearchEvent),
    /// События shell-монитора режима десктопа (T15): разрушение WorkerW
    /// (WinEventHook/поллинг) и смена DPI после репарентинга (R10).
    #[cfg(windows)]
    Desktop(canvas_shell::DesktopEvent),
    /// M5 (T20-F): события виджетов — WebView2-колбэки через proxy
    /// (EnvironmentReady/ControllerReady/SnapshotReady/Message) + тик
    /// таймера refresh-снапшотов. Тип кроссплатформенный: на Linux
    /// события не приходят (host нет), матчинг единообразен.
    Widget(canvas_widgets::WidgetEvent),
    /// M8/W6 (wasm-port §4.2): открыть канвас как активную сцену — текст
    /// уже прочитан платформенным слоем (FS Access/OPFS/DOM-drop), App
    /// только парсит и подменяет сцену. `storage` — новое хранилище (диск-
    /// хэндл после «Открыть с диска»); None — оставить текущее (импорт
    /// копии в OPFS). Платформенно-нейтрально: источник — web сейчас,
    /// позже тот же путь пригодится «Недавним» натива.
    OpenScene {
        path: PathBuf,
        json: String,
        storage: Option<Arc<dyn CanvasStorage>>,
    },
    /// Ввод текста через DOM (canvas-web, wasm-аудит 2026-09-25): Chromium
    /// шлёт кириллицу insertText'ом, который winit-web (только keydown)
    /// теряет; web-слой ловит beforeinput и доставляет текст сюда —
    /// маршрут в активный текстовый приёмник тот же, что у Ime::Commit.
    ImeCommit(String),
    /// События шины системных событий (T16): сессия (lock/unlock, R8),
    /// suspend/resume, ExplorerStarted (TaskbarCreated, R7/R11),
    /// shell-hook/clipboard (потребители T18/будущее), SHCNE-мост в
    /// конвейер T10 (корзина → brokenLink, R12).
    #[cfg(windows)]
    Shell(canvas_shell::shell_events::ShellEvent),
    /// В канале MCP pipe-сервера появились запросы (MCP-интеграция):
    /// забрать через `take_request`, ответить через responder.
    #[cfg(windows)]
    McpWake,
    /// T15-relaunch: работающий инстанс получил exit-сигнал от нового
    /// запуска (single-instance handoff, desktop/single_instance) —
    /// штатное завершение: форс-сейв сцены + восстановление иконок
    /// (shutdown). Событие шлёт exit-листенер (поток в main()) через
    /// proxy; на Windows сигналит любой повторный запуск, в т.ч.
    /// перезапуск на --desktop из меню канваса.
    /// На не-Windows листенера нет — вариант не конструируется (dead_code).
    #[cfg_attr(not(windows), allow(dead_code))]
    InstanceExit,
    /// FR-064 P1: воркер потока отдал снимок решений — UI-тред завершает
    /// пересчёт (публикация double buffer + выводка O(N)) в
    /// `SceneState::complete_flow_recompute` (истина — в outcomes-канале
    /// воркера; полезная нагрузка события — информационный снимок для
    /// wake-up, паттерн EventLoopProxy). Desktop-only: на wasm воркера нет
    /// (sync-путь — контракт плана волны S §5.8).
    #[cfg(not(target_arch = "wasm32"))]
    FlowReady {
        solutions: Arc<canvas_core::flow::FlowSolutions>,
        kind: canvas_scene::worker::FlowKind,
    },
}

/// Превью зоны дропа (T9): план вставки от DragEnter, origin следует за
/// курсором на DragOver; живёт до Leave/Drop.
struct DropPreview {
    /// Текущий origin сетки призраков в world-координатах.
    origin: Vec2,
    /// План вставки (id/тип/позиция) — переживает без изменений до Drop.
    plan: Vec<crate::ui::DropInsert>,
}

/// Модальный диалог приложения (T21-B/C: П10/П11): подтверждение
/// установки виджета drag-ом и удаления пакета. Enter — подтвердить,
/// Esc — отменить, клики по кнопкам; остальной ввод глушится.
enum AppDialog {
    /// «Установить виджет <имя> <версия>?»: источник-папка, манифест,
    /// мировая точка дропа (куда встанет нода после install), признак
    /// обновления существующего пакета (П5 — другой заголовок).
    InstallWidget {
        src: PathBuf,
        manifest: canvas_widgets::manifest::WidgetManifest,
        pos: Vec2,
        updating: bool,
    },
    /// «Удалить пакет <имя>? Ноды пакета останутся как заглушки» (П11).
    RemovePackage { widget_id: String, name: String },
    /// FR-014: диалог подтверждения цикла — параметры будущего ребра
    /// (концы и стороны). FR-025: control-фолбэк после диалога всегда
    /// теряет построчную семантику (from_line не сохраняется).
    EdgeCycle {
        from_node: String,
        from_side: Side,
        to_node: String,
        to_side: Side,
    },
    /// FR-050 Н4 (этап C): «Заменить источник?» — drop value-ребра на
    /// занятый параметр. Подтверждение — один undo-шаг (FR-006): старые
    /// рёбра (легаси-дубли — все, инвариант «один вход на параметр»)
    /// удаляются, новое создаётся; отмена — ничего не меняется.
    ReplaceSource {
        from_node: String,
        from_side: Side,
        from_line: Option<usize>,
        to_node: String,
        to_side: Side,
        param: String,
        /// id существующих рёбер, питающих параметр.
        old_edges: Vec<String>,
        /// Подпись текущего источника для тела диалога (заголовок ноды /
        /// имя шаблона — вычислена в момент дропа).
        old_source: String,
    },
    /// PRD-0007 (FR-048 X4, AC-5.3): «Откатить пачку автосвязи?» —
    /// подтверждение отката undo-бата создания связей; связи пачки
    /// подсвечены на канвасе на время диалога (решение владельца, раунд 2).
    AutolinkRollback {
        /// Число связей в пачке (для заголовка).
        count: usize,
        /// id рёбер пачки — подсветка отменяемого на канвасе.
        edge_ids: Vec<String>,
    },
}

impl AppDialog {
    /// Кнопки диалога (screen-space rect'ы считаются от центра окна).
    /// Подписи — таблица i18n (FR-040), `language` — язык интерфейса.
    /// FR-050 Н4: ReplaceSource — «Заменить»/«Отмена» (не Да/Нет);
    /// AutolinkRollback — «Откатить»/«Отмена».
    fn buttons(&self, language: Language) -> [(&'static str, bool); 2] {
        // (подпись, confirm?)
        match self {
            AppDialog::ReplaceSource { .. } => [
                (i18n::tr(language, keys::DIALOG_REPLACE_YES), true),
                (i18n::tr(language, keys::DIALOG_CANCEL), false),
            ],
            AppDialog::AutolinkRollback { .. } => [
                (i18n::tr(language, keys::AUTOLINK_UNDO_YES), true),
                (i18n::tr(language, keys::DIALOG_CANCEL), false),
            ],
            _ => [
                (i18n::tr(language, keys::DIALOG_YES), true),
                (i18n::tr(language, keys::DIALOG_NO), false),
            ],
        }
    }

    /// Заголовок диалога. `canvas` — для имени участников цикла (FR-014).
    fn title(&self, canvas: &Canvas, language: Language) -> String {
        match self {
            AppDialog::InstallWidget {
                manifest, updating, ..
            } => {
                let subs = [
                    ("{name}", manifest.name.as_str()),
                    ("{version}", manifest.version.as_str()),
                ];
                if *updating {
                    i18n::trf(language, keys::DIALOG_UPDATE_TITLE, &subs)
                } else {
                    i18n::trf(language, keys::DIALOG_INSTALL_TITLE, &subs)
                }
            }
            AppDialog::RemovePackage { name, .. } => {
                i18n::trf(language, keys::DIALOG_REMOVE_TITLE, &[("{name}", name)])
            }
            // FR-014: участники цикла — путь по value-рёбрам от стока к
            // истоку + замыкающее ребро (решение открытого вопроса:
            // диалог с фолбэком на control)
            AppDialog::EdgeCycle {
                from_node, to_node, ..
            } => {
                let mut chain = canvas_core::value_path(canvas, to_node, from_node)
                    .unwrap_or_else(|| vec![to_node.clone(), from_node.clone()])
                    .join(" → ");
                chain.push_str(" → ");
                chain.push_str(from_node);
                i18n::trf(language, keys::DIALOG_CYCLE_TITLE, &[("{chain}", &chain)])
            }
            // FR-050 Н4: заголовок без подстановок — «Заменить источник?»
            AppDialog::ReplaceSource { .. } => {
                i18n::tr(language, keys::DIALOG_REPLACE_TITLE).to_owned()
            }
            // PRD-0007 (AC-5.3): «Откатить пачку автосвязи ({n})?»
            AppDialog::AutolinkRollback { count, .. } => i18n::trf(
                language,
                keys::AUTOLINK_UNDO_TITLE,
                &[("{n}", count.to_string().as_str())],
            ),
        }
    }

    /// Пояснение под заголовком (таблица i18n — FR-040).
    fn body(&self, language: Language) -> String {
        match self {
            AppDialog::InstallWidget { manifest, .. } => {
                let perms = if manifest.permissions.is_empty() {
                    i18n::tr(language, keys::DIALOG_PERMS_NONE).to_owned()
                } else {
                    manifest
                        .permissions
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                i18n::trf(language, keys::DIALOG_INSTALL_BODY, &[("{perms}", &perms)])
            }
            AppDialog::RemovePackage { .. } => {
                i18n::tr(language, keys::DIALOG_REMOVE_BODY).to_owned()
            }
            AppDialog::EdgeCycle { .. } => i18n::tr(language, keys::DIALOG_CYCLE_BODY).to_owned(),
            // FR-050 Н4: параметр + текущий источник (подпись вычислена при
            // дропе — здесь только подстановка)
            AppDialog::ReplaceSource {
                param, old_source, ..
            } => i18n::trf(
                language,
                keys::DIALOG_REPLACE_BODY,
                &[("{param}", param), ("{source}", old_source)],
            ),
            // PRD-0007 (AC-5.3): подсветка отменяемого — рендер; здесь текст
            AppDialog::AutolinkRollback { .. } => {
                i18n::tr(language, keys::AUTOLINK_UNDO_BODY).to_owned()
            }
        }
    }
}

/// FR-050 Н2 (этап C): пункт меню выбора.
#[derive(Debug, Clone, PartialEq)]
struct ChoiceItem {
    /// Подпись (имя параметра / строка-источник со значением).
    label: String,
    action: ChoiceAction,
}

/// FR-050 Н2 (этап C): действие пункта меню выбора.
#[derive(Debug, Clone, PartialEq)]
enum ChoiceAction {
    /// Выбран параметр приёмника — создать value-ребро с `toParam`
    /// (drop на якорь или через меню выбора параметра).
    Param {
        from_node: String,
        from_side: Side,
        from_line: Option<usize>,
        to_node: String,
        param: String,
    },
    /// W-AMBIGUOUS-SRC (FR-032): выбрана строка-источник текстовой ноды —
    /// цель уже известна (параметр якоря или выбор из меню параметров).
    SourceLine {
        from_node: String,
        from_side: Side,
        line: usize,
        to_node: String,
        param: Option<String>,
    },
    /// FR-050 Н9-3 (этап E): «Показать источник» — полёт камеры к истоку +
    /// подсветка связи и её концов (токен SHOW_SOURCE_MS).
    ShowSource { edge_id: String },
    /// FR-050 Н9-3/Р-5 (этап E): «Отключить проливание» — удалить ребро
    /// (основной путь ручной правки; Ctrl+Z возвращает связь и значение).
    DisconnectSpill { edge_id: String },
    /// FR-050 Н9-3 (этап E): «Что если…» — вход в what-if режим (FR-017).
    WhatIf,
}

/// FR-050 Н2 (этап C): меню выбора — screen-space (как ContextMenu T7):
/// origin — логические px от угла окна, размер константен при любом зуме.
/// Пункты — параметры приёмника (drop мимо якоря: «меню выбора параметра
/// приёмника либо отмена») либо строки-источники (W-AMBIGUOUS-SRC).
#[derive(Debug, Clone, PartialEq)]
struct ChoiceMenu {
    /// Позиция (логические px) левого верхнего угла меню.
    origin: Vec2,
    /// Ключ i18n заголовка (MENU_PICK_PARAM_TITLE / MENU_PICK_LINE_TITLE).
    title_key: &'static str,
    /// Пункты: подпись + действие.
    items: Vec<ChoiceItem>,
    /// Hover-пункт для подсветки (паттерн аффорданса меню).
    hovered: Option<usize>,
}

/// FR-012: settle-анимация после вставки в группу — (индекс, из, в) для
/// группы и раздвинутых соседей; интерполяция ease_out_cubic ~250 мс.
struct SettleAnim {
    moves: Vec<(usize, [f32; 2], [f32; 2])>,
    start: Instant,
}

/// Длительность settle-анимации вставки в группу (FR-012), мс.
const SETTLE_ANIM_MS: f32 = 250.0;

/// FR-027: меню помощи кнопки «?» — колонка screen-space у кнопки (кламп
/// к окну) + флаг раскрытого подменю разделов документации (двухэтапный
/// Esc: подменю → меню → закрыто — семантика FR-026).
struct HelpMenuState {
    /// Origin колонки меню (логические px).
    origin: Vec2,
    /// Подменю «Документация ▸» раскрыто.
    docs_open: bool,
}

/// FR-027: просмотрщик документации — правый док: индекс страницы, скролл
/// и раскладка (пересчёт при смене страницы/размера — не на каждый кадр).
struct DocsViewer {
    /// Индекс страницы в docs_ui::DOCS_PAGES.
    page: usize,
    /// Раскладка страницы (строки/квады/ссылки) под текущую ширину панели.
    layout: docs_ui::PageLayout,
    /// Скролл контента (кламп).
    scroll: docs_ui::ScrollState,
    /// Ширина контента, под которую собрана раскладка (пересборка при
    /// изменении — окно resize/миграция между мониторами).
    layout_width: f32,
}

/// FR-017 (CP6): строка раскрытого списка подмен нижнего бара
/// (`нода → строка i: было → стало` + маркер протухания Q5c).
/// UI-тип панели what-if (после FR-037 MW1 остался в App: используется
/// только нижним баром).
#[derive(Debug, Clone)]
struct WhatIfOverrideRow {
    node: String,
    node_label: String,
    line: usize,
    base: String,
    whatif: String,
    /// Протухшая подмена (Q5c) — причина, строка рисуется предупреждающей.
    stale: Option<String>,
}

/// Состояние main stage (FR-042, E3): открытая детализация пучка.
/// Срез канваса — 2 ноды + все рёбра пучка (клон; позиции нод переписаны
/// раскладкой [`canvas_core::stage_layout`], stage-локальные px); рёбра
/// среза сохраняют оригинальные id — выделение отображается в живую
/// модель через `edges` (индексы среза ↔ live-индексы canvas.edges).
/// Геометрия веера (якоря на строках значений + кривые) — чистая функция
/// [`canvas_core::stage_edge_geometry`] на кадре: состояние не дублируется.
struct MainStageState {
    /// Упорядоченная пара концов пучка `(from_node, to_node)`.
    key: (String, String),
    /// Live-индексы рёбер пучка в порядке рёбер среза.
    edges: Vec<usize>,
    /// Срез: 2 ноды (раскладка stage) + рёбра пучка.
    slice: Canvas,
    /// Масштаб сжатия раскладки (обновляется на кадре в `relayout`).
    scale: f32,
}

/// FR-044 Р-1/Р-4: совместный контекст кадра main stage — один расчёт
/// для рендера и hit-теста (детерминизм, инвариант 3): геометрия веера,
/// зона клампа пилюль с учётом панели «Как считается», модель панели
/// и активная подсветка зависимостей (Р-5).
struct StageFrameCtx {
    /// Геометрия линий веера (те же, что у точек портов и hit-теста).
    lines: Vec<canvas_core::StageEdgeLine>,
    /// Модель панели «Как считается» (все входы приёмника, Р-8).
    model: calc_panel_ui::CalcPanelModel,
    /// Раскладка панели (None — нет ни переменных, ни формул).
    panel: Option<calc_panel_ui::CalcPanelLayout>,
    /// FR-059: скроллы колонок панели, синхронизированные раскладкой
    /// (копии состояния App на кадр — рисование бегунка/детерминизм).
    vars_scroll: canvas_ui::kit::ScrollState,
    formulas_scroll: canvas_ui::kit::ScrollState,
    /// Зона клампа пилюль (stage-локальные px; низ — верх панели − 8).
    zone: StageLocalRect,
    /// Активная подсветка (hover-превью перекрывает фиксированную).
    focus: Option<StageCalcFocus>,
    /// FR-044 Q3: коэффициент затемнения [0..1] — анимация перехода
    /// подсветки (токен focus_fade_ms); 0 — без приглушения.
    dim: f32,
    /// FR-044 Q2: режим зоны пилюль (Full/Compact/Scroll — эшелоны
    /// переполнения стопки).
    pill_mode: calc_panel_ui::PillZoneMode,
}

impl MainStageState {
    /// Открыть main stage для пучка: срез + раскладка + веер. None —
    /// пучок вырожден (нет доминанты/нод) или рёбра < 2.
    fn open(
        canvas: &Canvas,
        index: &canvas_core::EdgeBundleIndex,
        edge_index: usize,
    ) -> Option<Self> {
        let bundle = index.bundle_of_edge(edge_index)?;
        if bundle.weight < 2 {
            return None;
        }
        let key = index.bundle_key_of_edge(edge_index)?;
        let key = (key.0.to_owned(), key.1.to_owned());
        let from = canvas.node(&key.0)?;
        let to = canvas.node(&key.1)?;
        // Доминанта: её цвет/стиль/геометрия несут агрегированную линию
        // (проверка валидности: доминанта должна существовать)
        index.dominant_edge(canvas, bundle)?;
        // Срез: клон нод + рёбер пучка; позиции раскладкой stage.
        let mut slice = Canvas::default();
        slice.nodes.push(from.clone());
        slice.nodes.push(to.clone());
        let mut edges = Vec::with_capacity(bundle.edges.len());
        for &edge_idx in &bundle.edges {
            let edge = canvas.edges.get(edge_idx)?.clone();
            slice.edges.push(edge);
            edges.push(edge_idx);
        }
        // Раскладка среза — на КАДРЕ (rect зависит от вьюпорта):
        // MainStageState::relayout; здесь — нейтральные позиции, чтобы
        // якоря/кривые были согласованы до первого кадра.
        slice.nodes[0].x = 0.0;
        slice.nodes[0].y = 0.0;
        slice.nodes[1].x = slice.nodes[0].width + 200.0;
        slice.nodes[1].y = 0.0;
        Some(Self {
            key,
            edges,
            slice,
            scale: 1.0,
        })
    }

    /// Переложить ноды среза по раскладке текущего вьюпорта (кадр):
    /// rect зависит от размеров окна, масштаб ≤ 1 гарантирует умещение.
    ///
    /// Срез хранится в stage-локальных px БЕЗ масштаба: раскладка
    /// [`canvas_core::stage_layout`] возвращает уже сжатые rect-относительные
    /// px, а [`StageTransform`] применяет scale к позициям/размерам/полилиниям
    /// ровно один раз — поэтому позиции делятся на scale. Иначе (дефект
    /// «каши» в stage) позиции сжимались дважды, карточки рисовались в
    /// натуральном размере и расходились с веером и подписями тем сильнее,
    /// чем меньше scale.
    fn relayout(&mut self, viewport: [f32; 2]) -> StageLayout {
        let rect = main_stage_rect(viewport);
        let layout = stage_layout(
            [self.slice.nodes[0].width, self.slice.nodes[0].height],
            [self.slice.nodes[1].width, self.slice.nodes[1].height],
            &rect,
        );
        let s = layout.scale.max(f32::EPSILON);
        self.slice.nodes[0].x = layout.source_pos[0] / s;
        self.slice.nodes[0].y = layout.source_pos[1] / s;
        self.slice.nodes[1].x = layout.target_pos[0] / s;
        self.slice.nodes[1].y = layout.target_pos[1] / s;
        self.scale = layout.scale;
        layout
    }

    /// Валидность среза живой модели (инвариант 9 FR-042): все рёбра среза
    /// живы и связывают те же концы; обе ноды живы. Фоновые мутации
    /// (MCP graph_apply/undo) закрывают stage на следующем кадре.
    fn valid(&self, canvas: &Canvas) -> bool {
        for (slice_i, &live) in self.edges.iter().enumerate() {
            let Some(edge) = canvas.edges.get(live) else {
                return false;
            };
            let slice_edge = &self.slice.edges[slice_i];
            if edge.from_node != slice_edge.from_node || edge.to_node != slice_edge.to_node {
                return false;
            }
        }
        if self.edges.len() != self.slice.edges.len() {
            return false;
        }
        canvas.node(&self.key.0).is_some() && canvas.node(&self.key.1).is_some()
    }

    /// Live-индекс по индексу среза.
    fn live_edge(&self, slice_index: usize) -> Option<usize> {
        self.edges.get(slice_index).copied()
    }
}

/// Состояние приложения: окно и рендерер создаются в `resumed`
/// (идиома winit 0.30 — окно создаётся только на активном event loop).
pub struct App {
    window: Option<Arc<Window>>,
    renderer: Option<canvas_render::Renderer>,
    /// M8/W4 (wasm-port §3.4): стратегия запуска инициализации Renderer —
    /// натив `BlockOnRendererLaunch` (pollster, поведение как до W4), web
    /// `SpawnLocalRendererLaunch` (браузерный главный поток блокировать
    /// нельзя — план §7). Инъекция в App::new, паттерн W3-сервисов.
    renderer_launcher: Box<dyn RendererLauncher>,
    /// M8/W4: слот доставки async-результата (web): футура spawn_local
    /// кладёт Renderer и будит цикл; забирается первым RedrawRequested
    /// после готовности GPU. Натив — всегда None (ветка мертва).
    renderer_slot: Option<RendererSlot>,
    camera: Camera,
    scene: SceneState,
    /// Первичное выделение: нода или связь (T8) — якорь для контекстного
    /// меню, редактирования, фокуса (T23), перепривязки (CR-002).
    /// FR-037/ADR-0012: UI-поля ввода вынесены из SceneState в App.
    selected: Option<Selection>,
    /// Множественное выделение нод (CR-001): рамка drag или Ctrl/Shift+клик.
    /// Порядок — порядок добавления (клики) или индексы (рамка).
    selected_nodes: Vec<usize>,
    /// Drag ноды (T7/CR-001): захваченная нода + исходные позиции всех
    /// перемещаемых (выделение или одна + дети групп).
    dragging: Option<DragState>,
    /// Пул системных тамбнейлов (T6): заказы по видимым нодам, ответы в канал.
    /// M8/W3: backend за трейтом (натив — ThumbService, web — W10); инъекция в App::new.
    thumbs: Box<dyn ThumbBackend>,
    modifiers: ModifiersState,
    /// Позиция курсора в логических пикселях.
    cursor: Vec2,
    /// Текущая иконка курсора (аффорданс: пан — Grabbing, редактор — Text,
    /// resize-угол — NwseResize). Практики UI: форма курсора подсказывает
    /// жест; хранится, чтобы set_cursor вызывать только при смене.
    cursor_icon: CursorIcon,
    middle_pressed: bool,
    space_pressed: bool,
    left_pressed: bool,
    /// HUD с fps/p95/счётчиком видимых нод (F3, T5).
    hud_visible: bool,
    /// Замер интервалов между кадрами (окно 300 кадров).
    frame_meter: FrameMeter,
    /// Момент предыдущего отрисованного кадра.
    last_frame: Option<Instant>,
    /// Счётчики последнего кадра (для HUD).
    last_stats: FrameStats,
    /// FR-ICONS: инстансы SVG-иконок текущего кадра (screen-space px).
    /// Собираются в `KitDraw::icon` (kit_ui.rs) при отрисовке полос; drains
    /// в `FrameOverlay::icons` каждого кадра (`std::mem::take` — App мутабелен
    /// между сборкой полос и рендером). Активный набор читается каждый кадр
    /// из `settings.icon_style` через `App::icon_set_active` (overlays.rs) —
    /// смена набора применяется на следующем кадре без инвалидации кэшей.
    icon_instances: Vec<canvas_render::IconInstance>,
    /// Ноды, чей тамбнейл не удалось получить (битая ссылка и т.п.) —
    /// не перезаказывать каждый кадр; ретрай — при изменении файла вотчером (T10).
    thumbs_failed: std::collections::HashSet<usize>,
    /// Активная сессия инлайн-редактирования заметки (T7).
    editing: Option<EditingSession>,
    /// Драг внутри редактора (расширение выделения мышью, T7).
    editor_dragging: bool,
    /// Детектор двойного клика ЛКМ (T7).
    double_click: DoubleClick,
    /// Буфер обмена ОС (T7). M8/W3: backend за трейтом ClipboardBackend
    /// (натив — ArboardClipboard, web — navigator.clipboard).
    clipboard: Box<dyn ClipboardBackend>,
    /// Открытое контекстное меню ноды (ПКМ, T7).
    menu: Option<ContextMenu>,
    /// Hover-раскрытие групп палитры (FR-009): hover-intent открытие,
    /// отсрочка закрытия, пин по клику — практики фронтенда для дропдаунов.
    palette_hover: PaletteHover,
    /// Цель палитры с прошлого кадра: смена (клик по другой ноде/связи,
    /// изменение выделения) сбрасывает раскрытие — индекс группы не должен
    /// переживать смену цели (состав групп у нод и связей разный).
    palette_seen: Option<PaletteTarget>,
    /// Ручной resize ноды за правый нижний угол (T7): индекс ноды.
    resizing: Option<usize>,
    /// Нода под курсором (T8): показываются порты для начала drag связи.
    hovered: Option<usize>,
    /// FR-042 (E3): открытый main stage (детализация пучка). None — режим
    /// выключен; Q6 — взаимоисключителен с полноэкранными оверлеями.
    main_stage: Option<MainStageState>,
    /// FR-044 Р-5: зафиксированная подсветка зависимостей «формула ⇄
    /// переменные ⇄ рёбра» (клик по строке панели/пилюле/ребру веера).
    /// Сброс — Esc (Р-7 — первое Esc), клик по фону stage, закрытие stage.
    /// Runtime-состояние: не пишется в undo и `.canvas` (инвариант 8).
    stage_calc_focus: Option<StageCalcFocus>,
    /// FR-044 Р-5: hover-превью подсветки (живой отклик без фиксации;
    /// визуально перекрывает фиксированную, на оставлении курсора гаснет).
    stage_calc_hover: Option<StageCalcFocus>,
    /// FR-044 Q3: рендер-состояние подсветки — (множество, коэффициент
    /// затемнения [0..1]); обновляется тиком анимации ([`Self::
    /// tick_stage_calc_fade`]) до сборки кадра. В фейд-ауте множество —
    /// снимок последнего активного (гаснет вместе с коэффициентом).
    stage_calc_render: (Option<StageCalcFocus>, f32),
    /// FR-044 Q3: активный переход подсветки — (откуда, куда, момент
    /// старта); длительность — токен `focus_fade_ms` (animate.rs).
    stage_calc_fade: Option<(f32, f32, Instant)>,
    /// FR-044 Q2 (v2): смещение окна пилюль веера (режим Scroll —
    /// второй эшелон [`calc_panel_ui::pill_zone_mode`]); сброс при
    /// открытии/закрытии stage.
    stage_pill_scroll: usize,
    /// Тестовый оверрайд logical-вьюпорта: unit-тесты кликов по
    /// screen-space UI без winit-окна (viewport_logical читает первым).
    #[cfg(test)]
    test_viewport: Option<[f32; 2]>,
    /// PRD-0007 (FR-048 X2): открытое окно проверки цепочки расчёта
    /// (Loading/Ready; Stale — чип внутри). None — окно закрыто. Взаимо-
    /// исключителен с main stage (F-10) — открытие закрывает stage и наоборот.
    explain: Option<ExplainState>,
    /// PRD-0007 (AC-3.3, §9.2): сессионный кэш снапшота объяснения —
    /// переживает закрытие окна; переоткрытие того же корня — мгновенно
    /// (≤ 1 с, G1), без перестройки; чип — если модель изменилась.
    explain_cache: Option<ExplainSnapshot>,
    /// PRD-0007 (FR-048 X4, AC-5.5): предложения фонового детектора
    /// автосвязи (обновляются с дебаунсом после правок — `about_to_wait`).
    autolink_proposals: Vec<canvas_core::AutolinkProposal>,
    /// Момент последней правки модели для дебаунса скана (None — скан не
    /// отложен). Стартует после каждой смены ревизии.
    autolink_scan_due: Option<Instant>,
    /// Ревизия модели последнего выполненного скана (детектор — чистая
    /// функция над канвасом; скан повторяется только при изменении).
    autolink_scanned_rev: u64,
    /// PRD-0007 (FR-048 X4, AC-5.2): открытый диалог ревью автосвязи.
    /// None — закрыт; панель объяснения прячется на время диалога (§6.5).
    autolink_review: Option<Review>,
    /// Прокрутка тела диалога ревью (D10: 12+ предложений — скролл).
    autolink_scroll: f32,
    /// PRD-0007 (AC-5.3): id рёбер последнего undo-бата автосвязи — для
    /// подсветки отменяемого при подтверждении отката. Инвалидация — по
    /// тегу верхнего undo-снапшота (любое другое действие снимает тег).
    autolink_batch: Option<Vec<String>>,
    /// PRD-0007 (FR-048 X6, F-12): кэш индикатора покрытия цепочками —
    /// `(ревизия, процент)`: пересчёт — O(цифры × дерево), не на кадр;
    /// None — кэш пуст (первый кадр/смена настройки). Процент `None`
    /// внутри — цифр нет (индикатор скрыт).
    coverage_cache: Option<(u64, Option<u8>)>,
    /// FR-042 (E2): ребро пучка под курсором (live-индекс) — hover-бамп
    /// агрегированной линии; вычисляется на каждый кадр ввода (паттерн
    /// `hovered`), в кэш не пишется.
    bundle_hover: Option<usize>,
    /// FR-013 (правка 4): зоны наведения бейджей ошибок формульных строк с
    /// прошлого кадра (логические px + текст ошибки). Заполняется после
    /// рендера, используется в сборке оверлея кадра (тултип у курсора —
    /// паттерн тултипа битой ссылки T10). Отставание в кадр незаметно.
    expr_error_hits: Vec<LineErrorHit>,
    /// FR-050 Н9-2 (этап D): зоны наведения пролитых строк с прошлого кадра
    /// (логические px + данные тултипа источника). Заполняется после
    /// рендера (паттерн expr_error_hits), оверлей показывает «пролито: …».
    spill_hits: Vec<SpillHit>,
    /// FR-061 коммит 3: зоны усечённых формул с прошлого кадра (лестница
    /// §3.4, Q8) — тултип строки показывает полную формулу.
    ellipsis_hits: Vec<LineErrorHit>,
    /// FR-061 хвосты (D-7/D-8): кликабельные зоны тела (заголовок блока,
    /// экспандер описания; логические px) — с прошлого кадра (паттерн
    /// spill_hits; отставание в кадр незаметно).
    body_hits: Vec<BodyHit>,
    /// Drag резиновой линии новой связи (T8): от порта до отпускания ЛКМ.
    edge_drag: Option<EdgeDrag>,
    /// FR-050 Н2 (этап C): цель value-drag — шаблонная нода под курсором +
    /// совместимость её параметров с источником по единицам (Н5/E-UNIT).
    /// Пересчитывается на кадр ввода (паттерн `bundle_hover`), рендер
    /// подсвечивает якоря допустимых целей.
    param_drop: Option<(usize, Vec<(String, bool)>)>,
    /// FR-050 Н2 (этап C): меню выбора — параметры приёмника (drop
    /// value-ребра мимо якоря) либо строка-источник (W-AMBIGUOUS-SRC,
    /// FR-032). Screen-space, глушит ввод канваса (паттерн ContextMenu).
    choice_menu: Option<ChoiceMenu>,
    /// Рамка выделения (CR-001): (start world, current world, press screen)
    /// — тянется от пустого места; отпускание > порога = выделение.
    select_rect: Option<(Vec2, Vec2, Vec2)>,
    /// Буфер нодов (FR-003, Ctrl+C/Ctrl+V): внутренний, НЕ системный
    /// clipboard (там текст редактора); вставка — с новыми id.
    node_clipboard: Vec<Node>,
    /// Панель горячих клавиш открыта (FR-004, F1): слева по центру.
    hotkeys_open: bool,
    /// FR-016 (CP5): авто-включение оверлея узких мест уже сработало (или
    /// отключено ручным тогглом) в этом запуске — повторных тостов нет.
    bottleneck_auto_enabled: bool,
    /// Отложенный undo-снапшот (FR-006): «до» растянутого действия —
    /// drag/resize/редактирование. Ставится на старте, пушится в историю
    /// при фактическом изменении (клик без движения шага не создаёт).
    pending_undo: Option<Canvas>,
    /// Настройки приложения (config.toml).
    settings: Settings,
    /// Путь конфига (None — не сохраняем, работаем на дефолтах).
    /// FR-040 v2: на wasm32 поле не используется (persist через localStorage
    /// в `persist_settings_web`); сохранено в API для нативных callers —
    /// `App::new` вызывается одинаково с веб и натива, `Some(path)`/`None`
    /// передаётся caller'ом.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    config_path: Option<PathBuf>,
    /// Панель настроек открыта.
    settings_open: bool,
    /// FR-039: активный таб модалки настроек (индекс в SETTINGS_TABS).
    /// Хранится между открытиями модалки (как в Obsidian), в конфиг не
    /// пишется.
    settings_tab: usize,
    /// Выпадающее меню строки настроек (FR-026): какая строка открыта;
    /// пункты вычисляются на кадр, состояние не устаревает.
    settings_dropdown: DropdownState,
    /// FR-027: меню помощи кнопки «?» (открыто — поверх канваса, клики
    /// глотаются до закрытия; подменю — двухэтапный Esc).
    help_menu: Option<HelpMenuState>,
    /// FR-027: просмотрщик документации (правый док): открыт — колесо над
    /// панелью скроллит, клики по внутренним ссылкам ведут на страницы.
    docs: Option<DocsViewer>,
    /// FR-028: онбординг-тур (автозапуск при первом запуске либо пункт
    /// «Пройти онбординг» из меню «?»). Открытый тур блокирует ввод канваса.
    onboarding: Option<OnboardingState>,
    /// Превью зоны дропа (T9): план вставки на время DragOver.
    drop_preview: Option<DropPreview>,
    /// Модальный диалог T21 (установка/удаление пакета): глушит ввод канваса.
    dialog: Option<AppDialog>,
    /// Toast-строка (T21-A: bridge-toast, ошибки установки): живёт 3 с.
    toast: Option<(String, Instant)>,
    /// Файловый вотчер (T10): события ФС → AppEvent::FileEvents;
    /// набор директорий синхронизируется с моделью (sync_watch_dirs).
    /// M8/W3: backend за трейтом (натив — notify, web — NoopWatch).
    watcher: Box<dyn WatchBackend>,
    /// Регистрация IDropTarget (T9), Windows.
    #[cfg(windows)]
    drag_watcher: Option<canvas_shell::dragdrop::DropWatcher>,
    /// Отправитель drag-событий в event loop (T9). Читается только в
    /// cfg(windows)-ветке resumed(): единственный источник drag-событий —
    /// Windows IDropTarget (SPEC §7.3), на других ОС не читается.
    /// M8/W3: тип события — нейтральный canvas_core::dragdrop::DragEvent.
    #[cfg_attr(not(windows), allow(dead_code))]
    drag_sender: Arc<dyn Fn(canvas_core::dragdrop::DragEvent) + Send + Sync>,
    /// Отправитель событий WebView2-хоста виджетов в event loop (M5/T20).
    /// Прокси создаётся один раз в main() у `EventLoop` — у доступного в
    /// resumed() `ActiveEventLoop` метода create_proxy в winit 0.30 нет;
    /// тот же паттерн, что drag_sender. Читается только в cfg(windows)-ветке
    /// resumed(), на других ОС не читается.
    #[cfg_attr(not(windows), allow(dead_code))]
    widget_sender: canvas_widgets::WidgetEventSender,
    /// Миникарта (T13): снимок сцены + подгонка (CPU, SPEC §6.1).
    minimap: Option<Minimap>,
    /// Сигнатура состояния последней растеризации миникарты:
    /// (центр камеры, зум, размер буфера). Сцена отслеживается через
    /// dirty_since — правки/перемещения пересобирают снимок.
    minimap_sig: Option<([f32; 2], f32, u32, u32)>,
    /// Drag по миникарте (T13): world-точка под курсором следует за ним
    /// (клик без движения = мгновенное центрирование).
    minimap_drag: bool,
    /// M5 (T20-F): менеджер виджетов — реестр пакетов, LOD-план, host.
    widgets: crate::widgets::WidgetManager,
    /// Панель поиска (T14): поле, строки, выбор, скролл.
    search: SearchPanel,
    /// Сервис FTS-индекса (T14): команды в worker-поток, ответы —
    /// AppEvent::Search через proxy. M8/W3: backend за трейтом
    /// SearchBackend (натив — FTS5 в shell, web/тесты — MemSearch).
    search_service: Box<dyn SearchBackend>,
    /// Ноды результатов поиска — параллельно search.rows (T14).
    search_nodes: Vec<usize>,
    /// Debounce запроса (T14): (текст, момент последней правки) — отправка
    /// через 200 мс покоя в about_to_wait.
    search_pending: Option<(String, Instant)>,
    /// Полёт камеры к результату поиска (T14): (полёт, старт).
    flight: Option<(Flight, Instant)>,
    /// Пульс подсветки ноды-результата (T14): (нода, старт).
    pulse: Option<(usize, Instant)>,
    /// FR-019: реестр шаблонов — built-in библиотека из assets/templates
    /// (15 шаблонов, include_dir) + custom из ~/.canvasdesk/templates
    /// (FR-020, решение владельца — единый корень с виджетами).
    templates: canvas_core::templates::TemplateRegistry,
    /// FR-020: корень custom-шаблонов (`~/.canvasdesk/templates`).
    templates_root: std::path::PathBuf,
    /// FR-018: боковая палитра шаблонов (Ctrl+P): фильтр/категории/выбор.
    /// FR-025 — постоянный левый док (не модальна): open — развёрнутость,
    /// focused — клавиатурный фокус.
    template_panel: template_ui::TemplatePanel,
    /// FR-049: галерея готовых схем (модальная) — реестр встроенных
    /// пакетов `assets/canvas-schemes` (canvas-core::schemes).
    scheme_gallery: scheme_gallery_ui::SchemeGalleryState,
    /// FR-055 (этап U4 PRD-0009): витрина кита открыта (поверхность
    /// kit_gallery, Modals/Block) — пункт «?» «О интерфейсе» (Q5-a).
    pub(crate) kit_gallery_open: bool,
    /// FR-070: UI-админпанель открыта (поверхность admin_panel,
    /// Modals/Block) — пункт «?» «UI-консоль».
    pub(crate) admin_open: bool,
    /// FR-070: активная секция сайдбара админпанели.
    pub(crate) admin_section: crate::admin_ui::AdminSection,
    /// FR-070: скролл демо-зоны админпанели.
    admin_scroll: canvas_ui::kit::ScrollState,
    /// FR-070: live-переопределение слотов палитры (None — палитра темы;
    /// «Сброс»/смена темы очищают; сохранение в конфиг — вне рамок v1).
    admin_palette_override: Option<canvas_ui::kit::KitPalette>,
    /// FR-055 (этап U4, F-10): DebugOverlay виден (тогл F9 / `?ui=debug`).
    pub(crate) debug_overlay: bool,
    /// FR-049: empty-state скрыт кнопкой «Пустой холст» до следующего
    /// опустошения канваса (сброс при появлении первой ноды).
    empty_state_dismissed: bool,
    /// FR-049 (US-5): отложенная схема `?template=<id>` (web) — применяется
    /// на первом кадре, когда вьюпорт известен (zoom-to-fit корректен).
    pub pending_scheme: Option<String>,
    /// FR-025: drag карточки шаблона из палитры в точку канваса (нажатие
    /// на строку; отпускание решает — клик: в центр viewport, drag: в
    /// точку курсора с ghost-превью).
    template_drag: Option<template_ui::PanelDrag>,
    /// FR-025 (ревизия 2026-09-16): hover-раскрытие категорий свёрнутой
    /// полосы палитры (hover-intent 150 мс / grace 300 мс, пин по клику,
    /// прокрутка flyout колесом). None — к полосе ещё не обращались.
    template_hover: Option<template_ui::StripHover>,
    /// FR-018: радиальное wheel-меню шаблонов (Shift+клик по пустому
    /// месту): screen-центр + world-точка инстанциации + категория.
    wheel_menu: Option<template_ui::WheelMenu>,
    /// FR-021: popup контекстных подсказок Numi-ввода (состояние + якорь).
    hints: hints_ui::HintPopup,
    /// FR-017 (CP6): раскрытый список подмен активного сценария
    /// (клик по счётчику нижнего бара).
    whatif_list_open: bool,
    /// FR-017: таблица сравнения сценариев открыта (кнопка «Сравнить»).
    whatif_compare_open: bool,
    /// FR-017: строка ноды, открытая в override-поле (индекс строки текста).
    /// `Some` — commit сессии редактирования идёт в подмены активного
    /// сценария, а не в текст ноды (инвариант 2: база не мутируется).
    whatif_override_line: Option<usize>,
    /// FR-072: индекс только что созданной заметки, чей заголовок коммитится
    /// первым: после commit заголовка автоматически открывается правка тела
    /// (создание заметки = заголовок → Enter → тело). Esc — флаг снимается.
    title_then_body: Option<usize>,
    /// T23 (brainstorm-focus): затемнение сцены 0..1 (анимируется фейдом
    /// 150 мс при вкл/выкл и при появлении/исчезновении семени).
    focus_dim: f32,
    /// T23: активный фейд затемнения (от, к, старт).
    focus_fade: Option<(f32, f32, Instant)>,
    /// T23: «дыхание» подсвеченных связей: (семя, старт) — рестарт при
    /// смене семени, один цикл FOCUS_PULSE_MS, затем статика.
    focus_pulse: Option<(FocusSeed, Instant)>,
    /// T23: подсвеченные ноды (семя + соседи + выделенная) — данные
    /// FocusView кадра (пересчёт в update_focus_state).
    focus_nodes: Vec<usize>,
    /// T23: подсвеченные связи (инцидентные семени).
    focus_edges: Vec<usize>,
    /// FR-050 Н9-1 (этап E): активная волна каскада — value-рёбра с
    /// порядком топологического расстояния от изменённых нод
    /// (пульс подсветки вниз по потоку, `flow::spill_wave`); None — волна
    /// не идёт (нет изменений/закончилась). Ревизия сцены отслеживается
    /// отдельно (`seen_flow_revision`) — перестройка на каждом пересчёте.
    spill_wave: Option<(Vec<(usize, u32)>, Instant)>,
    /// Ревизия потока, на которой волна последний раз перестраивалась
    /// (дётект: `scene.revision` изменился → прочитать
    /// `scene.flow_changed_nodes`).
    seen_flow_revision: u64,
    /// FR-050 Н9-3 (этап E): «Показать источник» — подсветка истока,
    /// связи и приёмника с затемнением остального (машинерия фокуса
    /// PRD-0007/FR-048, без дублирования): (ноды, рёбра, старт); живёт
    /// SHOW_SOURCE_MS (токен motion.json), затем фейд обратно.
    show_source: Option<(Vec<usize>, Vec<usize>, Instant)>,
    /// FR-050 Н9-4 (этап E): панель «Карта проливаний» открыта — оверлей
    /// всех проливаний канваса (источник → параметр → значение), клик по
    /// строке — переход к истоку (камера + подсветка Н9-3).
    flow_map_open: bool,
    /// FR-059 (волна 1 кита): скролл списка карты проливаний (кит
    /// список+скролл — замена капа «… ещё N»; сброс при закрытии панели).
    flow_map_scroll: canvas_ui::kit::ScrollState,
    /// FR-059: скролл контента витрины кита (секции v2 — контент выше
    /// панели; сброс при открытии).
    kit_gallery_scroll: canvas_ui::kit::ScrollState,
    /// FR-062 F-17: Tab-кольцо фокус-секции витрины (живёт в
    /// КОНТЕНТ-координатах раскладки — `GalleryLayout::focus_targets` в kit_ui;
    /// сброс при открытии витрины).
    kit_gallery_focus: canvas_ui::keyboard::FocusRing,
    /// FR-059: скроллы колонок панели «Как считается» (кит список+скролл —
    /// замена среза «… ещё N»; живут с stage, сбрасываются при открытии).
    stage_calc_vars_scroll: canvas_ui::kit::ScrollState,
    stage_calc_formulas_scroll: canvas_ui::kit::ScrollState,
    /// FR-068 этап M2 (Table v2, `docs/plans/fr-068-table-v2.md` §8):
    /// retained-таблицы панели «Как считается» (vars, formulas) —
    /// создаются при ПЕРВОМ кадре панели
    /// ([`canvas_ui::kit::Table::new`] поднимает `FontSystem` — дорого,
    /// один раз, не на кадр; §4.6 дизайна). `RefCell`, т.к. `stage_frame`
    /// работает на `&self` (mut-заём среза `main_stage` живёт весь кадр —
    /// второй mut-заём `App` несовместим); Props (кегль/палитра/opts) —
    /// кадровые, синхронизируются в `paint_calc_panel_rows` (stage.rs).
    stage_calc_tables: std::cell::RefCell<Option<(canvas_ui::kit::Table, canvas_ui::kit::Table)>>,
    /// FR-012: цель «втягивания» во время drag — группа под центром
    /// перетаскиваемой ноды (зона подсвечивается, отпускание — вставка).
    group_drop_target: Option<usize>,
    /// FR-012: settle-анимация после вставки в группу — плавный проезд
    /// группы и раздвинутых соседей к целевым позициям (~250 мс).
    settle_anim: Option<SettleAnim>,
    /// FR-073: состояние физики расталкивания (якоря + скорость курсора).
    drag_push: DragPushState,
    /// FR-073: живая сессия физики — true от старта drag до полного
    /// расселения после drop; между драгами физика не тикает (внешние
    /// сдвиги нод — автораскладка/MCP — якоря не трогают).
    drag_push_live: bool,
    /// Режим десктопа (T15, флаг --desktop): окно встраивается в WorkerW
    /// (Windows; на других ОС — warn и обычный оконный режим, SPEC §9).
    desktop_mode: bool,
    /// Найденная иерархия десктопа (T15) после успешного attach: хэндлы
    /// Progman/DefView/WorkerW + стратегия. None — не встроены/фолбэк.
    #[cfg(windows)]
    desktop_hierarchy: Option<canvas_shell::desktop::hierarchy::DesktopHierarchy>,
    /// Последний достоверный DPI окна в desktop-режиме (T15, R10): снят
    /// GetDpiForWindow сразу после attach и обновляется поллингом монитора.
    /// None — оконный режим/до attach: scale берётся из window.scale_factor().
    #[cfg(windows)]
    desktop_dpi: Option<u32>,
    /// Shell-монитор (T15): WinEventHook на WorkerW + DPI-поллинг; события —
    /// AppEvent::Desktop через proxy. Спавнится в main() при --desktop,
    /// слежка (Watch) устанавливается в resumed() после attach.
    #[cfg(windows)]
    desktop_monitor: Option<canvas_shell::desktop::monitor::DesktopMonitorService>,
    /// Счётчик подряд неудач re-attach (T15): ≥3 — стоп автоматики + warn
    /// (анти-флуд упрощённый; полный по PID Shell_TrayWnd — T17).
    #[cfg(windows)]
    desktop_recover_failures: u32,
    /// WS_EX_NOACTIVATE уже снят первым кликом (T15, идемпотентный флаг).
    #[cfg(windows)]
    desktop_activation_enabled: bool,
    /// Шина системных событий (T16): message-only окно на отдельном потоке;
    /// события — AppEvent::Shell через proxy. Спавнится в main()
    /// без привязки к --desktop (события сессии/сна/shell-файлы полезны в
    /// любом режиме, план §8.7); провал — warn + деградация (R14).
    #[cfg(windows)]
    shell_events: Option<canvas_shell::shell_events::window::ShellEventService>,
    /// Последнее известное состояние гейта интерактивной сессии (T16, R8):
    /// лог/диагностика; потребление — T18. Старт true (как и атомик в шине).
    #[cfg(windows)]
    session_interactive: bool,
    /// Владелец скрытия системных иконок (T17-A, R5): capture+hide в
    /// attach_desktop; restore при штатном выходе, Drop-страховка от
    /// паник, sentinel — краш-сейф kill -9.
    #[cfg(windows)]
    icon_guard: Option<canvas_shell::desktop::icons::IconGuard>,
    /// Детект рестартов Explorer (T17-B, R7): PID Shell_TrayWnd
    /// до/после TaskbarCreated (из шины T16) + анти-флуд 30 с.
    #[cfg(windows)]
    explorer_tracker: canvas_shell::desktop::explorer::RestartTracker,
    /// MCP pipe-сервер (MCP-интеграция): worker-поток \\.\pipe\canvasdesk;
    /// запросы забираются по AppEvent::McpWake. None — MCP недоступен (деградация).
    #[cfg(windows)]
    mcp_server: Option<canvas_shell::mcp_pipe::McpPipeServer>,
}

impl App {
    // 11 аргументов — гейт-конфигурация сессии (сцена, сервисы, флаги);
    // группировать в структуру ради clippy — лишний слой на единственном
    // месте создания (main/canvas-web). M8/W3: все платформенные сервисы —
    // нейтральные трейты (инъекция), shell-типов в сигнатуре нет.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scene: SceneState,
        thumbs: Box<dyn ThumbBackend>,
        settings: Settings,
        config_path: Option<PathBuf>,
        cache_dir: Option<PathBuf>,
        drag_sender: Arc<dyn Fn(canvas_core::dragdrop::DragEvent) + Send + Sync>,
        widget_sender: canvas_widgets::WidgetEventSender,
        watcher: Box<dyn WatchBackend>,
        search_service: Box<dyn SearchBackend>,
        clipboard: Box<dyn ClipboardBackend>,
        widget_state: Option<Box<dyn canvas_core::WidgetStateBackend>>,
        desktop_mode: bool,
        renderer_launcher: Box<dyn RendererLauncher>,
    ) -> Self {
        // M5 (T20-F): менеджер виджетов; реестр инициализируется в
        // main() (init_widgets) после настройки трейсинга. M8/W3: каталог
        // кэша инъектируется (натив — shell::default_cache_dir, web — W6).
        // M8/W11: реестр виджетов выбирается по каталогу кэша — есть ФС-
        // каталог → файловый реестр (натив, как сегодня); нет (web) →
        // реестр в памяти (встроенные пакеты из include_dir; решение
        // «OPFS или память» — память, план §5: пакетные файлы в волне 1
        // никто не читает — live-хост отсутствует, манифесты нужны
        // меню/LOD/permissions).
        let widgets_registry = match &cache_dir {
            Some(dir) => canvas_widgets::registry::WidgetRegistry::new(dir.join("widgets")),
            None => canvas_widgets::registry::WidgetRegistry::in_memory(),
        };
        let widgets = crate::widgets::WidgetManager::new(
            widgets_registry,
            // FR-047: виджетам флаг темности ЭФФЕКТИВНОЙ темы (пресет из
            // config.toml перекрывает классику)
            ThemeColors::from_settings(settings.theme, &settings.theme_preset).is_dark(),
            widget_state,
        );
        // FR-027/FR-028: помощь/документация закрыты; тур при первом
        // запуске открывает should_show_onboarding (прецедент
        // hud_on_start, FR-028) — флаг считается ДО переноса settings в Self
        let show_onboarding = onboarding_ui::should_show_onboarding(&settings);
        Self {
            widgets,
            window: None,
            renderer: None,
            renderer_launcher,
            renderer_slot: None,
            camera: Camera::default(),
            scene,
            selected: None,
            selected_nodes: Vec::new(),
            dragging: None,
            thumbs,
            modifiers: ModifiersState::empty(),
            cursor: [0.0, 0.0],
            cursor_icon: CursorIcon::Default,
            middle_pressed: false,
            space_pressed: false,
            left_pressed: false,
            hud_visible: settings.hud_on_start,
            frame_meter: FrameMeter::new(),
            last_frame: None,
            last_stats: FrameStats::default(),
            // FR-ICONS: пустой буфер инстансов; активный набор читается каждый
            // кадр из `settings.icon_style` (через `App::icon_set_active`).
            icon_instances: Vec::new(),
            thumbs_failed: std::collections::HashSet::new(),
            editing: None,
            // FR-072: автопереход «заголовок → тело» выключен по умолчанию
            title_then_body: None,
            editor_dragging: false,
            double_click: DoubleClick::new(),
            clipboard,
            menu: None,
            palette_hover: PaletteHover::new(),
            palette_seen: None,
            resizing: None,
            hovered: None,
            // FR-042: main stage закрыт; hover пучка пуст
            main_stage: None,
            stage_calc_focus: None,
            stage_calc_hover: None,
            // FR-044 Q3: подсветки нет — коэффициент затемнения 0
            stage_calc_render: (None, 0.0),
            stage_calc_fade: None,
            // FR-044 Q2: окно пилюль — с начала
            stage_pill_scroll: 0,
            #[cfg(test)]
            test_viewport: None,
            // PRD-0007 (X2): окно проверки закрыто, сессионный кэш пуст
            explain: None,
            explain_cache: None,
            // PRD-0007 (FR-048 X4): фон автосвязи — пусто до первого скана
            // (оный стартует в about_to_wait с дебаунсом после загрузки)
            autolink_proposals: Vec::new(),
            autolink_scan_due: None,
            autolink_scanned_rev: 0,
            autolink_review: None,
            autolink_scroll: 0.0,
            autolink_batch: None,
            coverage_cache: None,
            bundle_hover: None,
            expr_error_hits: Vec::new(),
            spill_hits: Vec::new(),
            ellipsis_hits: Vec::new(),
            body_hits: Vec::new(),
            edge_drag: None,
            param_drop: None,
            choice_menu: None,
            select_rect: None,
            node_clipboard: Vec::new(),
            hotkeys_open: false,
            bottleneck_auto_enabled: false,
            pending_undo: None,
            // FR-025 (ревизия): палитра — постоянная, но примарно СВЁРНУТАЯ
            // (полоса категорий); развёрнутость из конфига (дефолт —
            // свёрнута). Поле инициализируется до move `settings` ниже.
            template_panel: {
                let mut panel = template_ui::TemplatePanel::new();
                panel.open = settings.template_palette_open;
                panel
            },
            scheme_gallery: scheme_gallery_ui::SchemeGalleryState::default(),
            // FR-055 U4: витрина кита закрыта, DebugOverlay выключен
            // (в web включается параметром `?ui=debug` — url_params).
            kit_gallery_open: false,
            admin_open: false,
            admin_section: crate::admin_ui::AdminSection::Components,
            admin_palette_override: None,
            debug_overlay: false,
            empty_state_dismissed: false,
            pending_scheme: None,
            settings,
            config_path,
            settings_open: false,
            settings_tab: 0,
            settings_dropdown: DropdownState::default(),
            // FR-027/FR-028: помощь/документация закрыты; тур при первом
            // запуске открывает should_show_onboarding (прецедент
            // hud_on_start, FR-028)
            help_menu: None,
            docs: None,
            onboarding: show_onboarding.then(OnboardingState::default),
            drop_preview: None,
            dialog: None,
            toast: None,
            watcher,
            #[cfg(windows)]
            drag_watcher: None,
            drag_sender,
            widget_sender,
            minimap: None,
            minimap_sig: None,
            minimap_drag: false,
            search: SearchPanel::default(),
            search_service,
            search_nodes: Vec::new(),
            search_pending: None,
            flight: None,
            pulse: None,
            templates_root: cache_dir.clone().unwrap_or_default().join("templates"),
            templates: {
                let root = cache_dir.unwrap_or_default().join("templates");
                canvas_core::templates::TemplateRegistry::all_with_custom(&root)
            },
            template_drag: None,
            template_hover: None,
            wheel_menu: None,
            hints: hints_ui::HintPopup::default(),
            whatif_list_open: false,
            whatif_compare_open: false,
            whatif_override_line: None,
            focus_dim: 0.0,
            focus_fade: None,
            focus_pulse: None,
            focus_nodes: Vec::new(),
            focus_edges: Vec::new(),
            spill_wave: None,
            seen_flow_revision: 0,
            show_source: None,
            flow_map_open: false,
            flow_map_scroll: canvas_ui::kit::ScrollState::default(),
            kit_gallery_scroll: canvas_ui::kit::ScrollState::default(),
            admin_scroll: canvas_ui::kit::ScrollState::default(),
            kit_gallery_focus: canvas_ui::keyboard::FocusRing::default(),
            stage_calc_vars_scroll: canvas_ui::kit::ScrollState::default(),
            stage_calc_formulas_scroll: canvas_ui::kit::ScrollState::default(),
            // FR-068 M2: retained-таблицы панели — lazy при первом кадре
            // панели (см. поле); Props синхронизируются кадром
            stage_calc_tables: std::cell::RefCell::new(None),
            group_drop_target: None,
            settle_anim: None,
            drag_push: DragPushState::new(),
            drag_push_live: false,
            desktop_mode,
            #[cfg(windows)]
            desktop_hierarchy: None,
            #[cfg(windows)]
            desktop_dpi: None,
            #[cfg(windows)]
            desktop_monitor: None,
            #[cfg(windows)]
            desktop_recover_failures: 0,
            #[cfg(windows)]
            desktop_activation_enabled: false,
            #[cfg(windows)]
            shell_events: None,
            // Старт true — зеркалит атомик IS_INTERACTIVE_SESSION в шине
            // (T16-A, план §8.3): залоченная до старта сессия события не
            // пришлёт до unlock
            #[cfg(windows)]
            session_interactive: true,
            #[cfg(windows)]
            icon_guard: None,
            #[cfg(windows)]
            explorer_tracker: Default::default(),
            #[cfg(windows)]
            mcp_server: None,
        }
    }

    /// FR-061 этап D (D-8): снимок описаний манифестов шаблонов
    /// (id → описание, Q3) — в сцену и рендер. Вызывается при построении
    /// App и после импорта/обновления шаблонов.
    fn sync_template_descs(&mut self) {
        let descs = self.template_desc_map();
        self.scene.template_descs = descs.clone();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_template_descs(descs);
        }
    }

    /// Подключить shell-монитор десктопа (T15): спавнится в main() при
    /// --desktop (responder через EventLoopProxy), слежка — в resumed().
    #[cfg(windows)]
    pub fn set_desktop_monitor(
        &mut self,
        monitor: canvas_shell::desktop::monitor::DesktopMonitorService,
    ) {
        self.desktop_monitor = Some(monitor);
    }

    /// Подключить шину системных событий (T16; сеттер-паттерн T15 —
    /// сервис спавнится в main() до входа в event loop).
    #[cfg(windows)]
    pub fn set_shell_events(
        &mut self,
        service: canvas_shell::shell_events::window::ShellEventService,
    ) {
        self.shell_events = Some(service);
    }

    /// Подключить MCP pipe-сервер (MCP-интеграция; сеттер-паттерн T15/T16).
    #[cfg(windows)]
    pub fn set_mcp_server(&mut self, server: Option<canvas_shell::mcp_pipe::McpPipeServer>) {
        self.mcp_server = server;
    }

    /// HWND окна приложения через raw-window-handle (T15; тот же приём,
    /// что dragdrop::install в T9: winit 0.30 публично HWND не отдаёт).
    #[cfg(windows)]
    fn window_hwnd(&self) -> Option<isize> {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let window = self.window.as_ref()?;
        match window.window_handle() {
            Ok(handle) => match handle.as_raw() {
                RawWindowHandle::Win32(win32) => Some(win32.hwnd.get()),
                _ => None,
            },
            Err(_) => None,
        }
    }

    /// Конвертация raw-window-handle → HWND (идиома dragdrop/com.rs:
    /// Win32 HWND — указатель без внутренней структуры).
    #[cfg(windows)]
    fn hwnd(raw: isize) -> canvas_shell::desktop::HWND {
        canvas_shell::desktop::HWND(raw as *mut core::ffi::c_void)
    }

    /// Встройка в десктоп (T15, resumed): детект иерархии → идемпотентный
    /// спавн WorkerW → attach с верификацией стилей → слежка монитора.
    /// Любая ошибка — warn + MessageBox + фолбэк: окно остаётся обычным
    /// borderless top-level (R14; это и есть «обычное окно» — пересоздавать
    /// после winit-инициализации нельзя).
    #[cfg(windows)]
    fn attach_desktop(&mut self, raw: isize) {
        use canvas_shell::desktop::{attach, hierarchy};
        let hwnd = Self::hwnd(raw);
        let screen = hierarchy::virtual_screen_rect().unwrap_or_else(|| {
            tracing::warn!("виртуальный экран недоступен — экран 1280x720");
            canvas_shell::ScreenRect::from_ltrb(0, 0, 1280, 720)
        });
        let result = (|| -> Result<(hierarchy::DesktopHierarchy, ()), attach::AttachError> {
            let progman = hierarchy::find_progman()?;
            let hier = hierarchy::ensure_worker_w(progman)?;
            attach::attach(hwnd, &hier, screen).map(|_| (hier, ()))
        })();
        match result {
            Ok((hier, _)) => {
                tracing::info!(
                    strategy = ?hier.strategy,
                    worker_w = hier.worker_w.is_some(),
                    screen = ?(screen.left, screen.top, screen.right, screen.bottom),
                    "канвас встроен в рабочий стол (T15)"
                );
                // T17 (R5, SPEC §7.4 п.5): скрыть системные иконки — на
                // ОБЕИХ стратегиях. На Classic канвас встал НА МЕСТО слоя
                // иконок (без скрытия они невидимы, но кликабельны). На
                // Raised DefView непрозрачен для hit-test даже со скрытыми
                // иконками (проверено WindowFromPoint), поэтому канвас
                // поднят Z-order НАД DefView (attach шаг 4) и перекрывает
                // иконки опаком — скрытие 0x7402 держит поведение стратегий
                // одинаковым (канвас заменяет десктоп, M4) и синхронизирует
                // пункт меню «показать иконки» с фактом.
                let mut guard = canvas_shell::desktop::icons::IconGuard::capture(hier.def_view);
                guard.hide();
                self.icon_guard = Some(guard);
                // R10: зафиксировать достоверный DPI ДО первого тика
                // монитора (его первый замер — молчаливый бейзлайн): без
                // этого viewport_logical()/кнопки ещё один тик (500 мс) и
                // дольше — при стабильном DPI навсегда — считались бы от
                // врущего window.scale_factor() после репарентинга.
                let dpi = attach::window_dpi(hwnd);
                if dpi != 0 {
                    self.desktop_dpi = Some(dpi);
                    // R10: рендерер продолжает считать от врущего
                    // window.scale_factor() после репарентинга — передаём
                    // достоверный DPI из поллинга (как в DpiChanged ниже)
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.set_scale_factor(canvas_shell::dpi_to_scale(dpi));
                    }
                }
                self.desktop_hierarchy = Some(hier);
                self.watch_worker_w(hwnd, &hier);
            }
            Err(err) => {
                tracing::warn!(%err, "встройка в десктоп не удалась — оконный режим");
                attach::fallback_message_box(&err.to_string());
                // R14-деградация: desktop-окно создано borderless на весь
                // виртуальный экран — как top-level оно перекрывает Пуск и
                // иконки. Ужимаем до рабочей области (экран без таскбара).
                attach::shrink_to_work_area(hwnd);
            }
        }
    }

    /// Включить desktop-режим в рантайме (T15): вызвать `attach_desktop` и
    /// выставить `desktop_mode = true`. Если монитор не запущен (не было
    /// `--desktop` при старте) — запускаем его здесь же, чтобы слежка за
    /// WorkerW и DPI-поллинг работали сразу после встройки. Повторный вызов
    /// (уже в desktop-режиме) — no-op.
    ///
    /// T15: галочка «Режим десктопа» в меню канваса — режим активен И
    /// встойка удалась (иерархия найдена). Поле `desktop_hierarchy` есть
    /// только на Windows; на других ОС `desktop_mode` не поднимается
    /// (attach недоступен), поэтому там достаточно одного флага.
    #[cfg(windows)]
    fn desktop_menu_checked(&self) -> bool {
        self.desktop_mode && self.desktop_hierarchy.is_some()
    }

    /// Не-Windows вариант (см. windows-версию): attach недоступен —
    /// `desktop_mode` никогда не поднимается, галочка всегда снята.
    #[cfg(not(windows))]
    fn desktop_menu_checked(&self) -> bool {
        self.desktop_mode
    }

    /// Вход в desktop-режим в рантайме (T15-relaunch): перезапуск себя с
    /// флагом --desktop. In-place встройка (attach_desktop из меню, ee63b7a)
    /// НЕ работает: рендерер в оконном режиме создан с prefer_dx12=false,
    /// а Vulkan-swapchain не презентует в ребёнка Progman (проверено
    /// экспериментом, см. resumed()) — окно растягивается на виртуальный
    /// экран (Win32-шаги attach проходят), но канвас не рисуется и обои
    /// остаются видимыми. Новый процесс стартует с чистого листа: окно
    /// borderless → attach ДО создания GPU-surface → DX12-рендерер.
    /// Эксклюзивность — single-instance handoff (desktop/single_instance):
    /// новый инстанс сигналит exit-событие, этот инстанс штатно сохранится
    /// и выйдет (AppEvent::InstanceExit → shutdown), новый дождётся
    /// освобождения мьютекса и стартанёт в desktop-режиме. Ошибка спавна —
    /// строка для MessageBox (фолбэк R14: пользователь запустит вручную).
    #[cfg(windows)]
    fn spawn_desktop_relaunch(&self) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|err| format!("current_exe: {err}"))?;
        std::process::Command::new(&exe)
            .arg("--desktop")
            .spawn()
            .map(|_| ())
            .map_err(|err| format!("spawn {:?}: {err}", exe))
    }

    /// Выключить desktop-режим в рантайме (T15): обратная к встройке —
    /// `attach::detach` (SetParent(None) + scrub-план «обычного» окна +
    /// shrink_to_work_area), восстановление иконок (IconGuard::restore),
    /// сброс `desktop_hierarchy` / `desktop_dpi` / `desktop_mode`. Монитор
    /// НЕ останавливаем (переживёт выход процесса — поток умрёт вместе с
    /// процессом при завершении).
    ///
    /// Здесь in-place detach корректен (в отличие от входа): рендерер
    /// создан с prefer_dx12=true и в обычном окне презентует нормально —
    /// пересоздавать его не нужно. Любая ошибка detach — не-фатальная:
    /// внутреннее состояние всё равно сбрасывается (канвас остаётся
    /// интерактивным в оконном режиме, даже если визуально окно «застряло»
    /// fullscreen — пользователь может перезапустить приложение).
    #[cfg(windows)]
    fn leave_desktop(&mut self) {
        if !self.desktop_mode {
            tracing::debug!("leave_desktop: не в desktop-режиме — no-op");
            return;
        }
        if let Some(raw) = self.window_hwnd() {
            let hwnd = Self::hwnd(raw);
            if let Err(err) = canvas_shell::desktop::attach::detach(hwnd) {
                tracing::warn!(%err, "leave_desktop: detach провален — состояние сброшено, окно м.б. некорректным");
            }
        } else {
            tracing::warn!("leave_desktop: нет HWND — только сброс состояния");
        }
        // Восстановить системные иконки (T17, R5): IconGuard::drop делает
        // restore, но мы явно вызываем restore() для ясности и сбрасываем
        // поле, чтобы Drop не сработал повторно при завершении приложения.
        if let Some(mut guard) = self.icon_guard.take() {
            guard.show();
            // guard тут drop-нется — restore через Drop страховкой не повторяем
            drop(guard);
        }
        // Снять слежку монитора (Watch с пустой иерархией — монитор переходит
        // в режим «ждать новой иерархии», не падает).
        self.desktop_hierarchy = None;
        self.desktop_dpi = None;
        self.desktop_mode = false;
        tracing::info!("desktop-режим выключен через меню (T15 runtime toggle)");
    }

    /// Установить/перенавесить слежку монитора на иерархию (T15):
    /// WinEventHook на поток WorkerW + DPI-поллинг нашего окна. WorkerW=None
    /// (фон без обоев) — hook не вешается, поллинг следит только за Progman.
    #[cfg(windows)]
    fn watch_worker_w(
        &self,
        ours: canvas_shell::desktop::HWND,
        hier: &canvas_shell::desktop::hierarchy::DesktopHierarchy,
    ) {
        if let Some(monitor) = &self.desktop_monitor {
            monitor.command(canvas_shell::desktop::monitor::MonitorCommand::Watch {
                progman: hier.progman,
                worker_w: hier.worker_w,
                ours,
            });
        }
    }

    /// R7-реакция на TaskbarCreated (T17, план §3): PID Shell_TrayWnd
    /// до/после; настоящий рестарт Explorer → FullReattach + репоинт
    /// иконок на новый DefView; DPI-смена/тема → игнор (поллинг T15
    /// догонит, R10); анти-флуд — crash-loop не утащит в бесконечный
    /// re-attach (R7 п.3).
    #[cfg(windows)]
    fn on_explorer_started(&mut self) {
        use canvas_shell::desktop::explorer::ExplorerRestart;
        // PID опрашиваем ДО register (совет T17-B worklog): окна может
        // не быть в момент события — это не рестарт
        let Some(pid) = canvas_shell::desktop::explorer::tray_pid() else {
            tracing::debug!("TaskbarCreated без Shell_TrayWnd — игнор");
            return;
        };
        match self.explorer_tracker.register(pid, Instant::now()) {
            ExplorerRestart::SameProcess => {
                tracing::info!(pid, "Explorer жив (TaskbarCreated от DPI/темы)");
            }
            ExplorerRestart::Restarted => {
                tracing::warn!(pid, "Explorer перезапущен — восстановление встройки");
                if self.explorer_tracker.suppress_automatic() {
                    // Недостижимо для семантики register (флуд →
                    // FloodStop), страховка от дрейфа
                    tracing::warn!("автоматика восстановления остановлена");
                    return;
                }
                self.recover_desktop(canvas_shell::RecoveryAction::FullReattach);
                // Иерархия пересоздана — guard уже репоинтнут внутри
                // recover_desktop (Ok-ветка); скрытие до-скрыто там же
            }
            ExplorerRestart::FloodStop => {
                // crash-loop: >1 рестарта за 30 с — автоматика стоп,
                // сообщение пользователю в лог (R7 п.3)
                tracing::warn!(
                    "crash-loop Explorer (>1 рестарта за 30 с) — автоматика восстановления остановлена"
                );
            }
        }
    }

    /// Обработка событий shell-монитора (T15): разрушение WorkerW →
    /// recovery по стратегии (R2-симметрия); смена DPI → переградуировка
    /// рендера (R10: winit-события после репарентинга не приходят).
    #[cfg(windows)]
    fn on_desktop_event(&mut self, event: canvas_shell::DesktopEvent) {
        match event {
            canvas_shell::DesktopEvent::WorkerWDestroyed => {
                let action =
                    canvas_shell::recovery_action(self.desktop_hierarchy.map(|h| h.strategy));
                match action {
                    canvas_shell::RecoveryAction::None => {}
                    canvas_shell::RecoveryAction::ReZOrder
                    | canvas_shell::RecoveryAction::FullReattach => self.recover_desktop(action),
                }
            }
            canvas_shell::DesktopEvent::DpiChanged { dpi } => {
                let scale = canvas_shell::dpi_to_scale(dpi);
                tracing::info!(dpi, scale, "DPI десктоп-окна изменился (поллинг R10)");
                self.desktop_dpi = Some(dpi);
                if let Some(renderer) = self.renderer.as_mut() {
                    // Пересоздание surface не нужно: размер HWND не менялся;
                    // минимап пересоберётся по сигнатуре кадра
                    renderer.set_scale_factor(scale);
                }
                self.request_redraw();
            }
        }
    }

    /// Обработка событий шины системных событий (T16, план §3): сессия —
    /// гейт интерактивности (лог/диагностика, R8; потребление — T18),
    /// Suspending — форс-сейв .canvas ДО ухода системы в сон (критерий
    /// TASKS T16; бэкап — внутри save_with_backup, SPEC §9), Resumed —
    /// прогрев кадра, файл-события SHCNE-моста (R12) — конвейер T10.
    #[cfg(windows)]
    fn on_shell_event(&mut self, event: canvas_shell::shell_events::ShellEvent) {
        use canvas_shell::shell_events::ShellEvent;
        match event {
            // classify_wts в шине уже отсёк чужие сессии (R8): дошли только
            // свои — гейт актуален; атомик шины обновлён там же (wndproc)
            ShellEvent::Session { event, session_id } => {
                if let Some(value) = event.gate_value() {
                    self.session_interactive = value;
                }
                tracing::info!(
                    ?event,
                    session_id,
                    interactive = self.session_interactive,
                    "событие сессии (R8)"
                );
            }
            // Безусловно (не только dirty): дебаунс-сейв может не успеть —
            // запись ДО сна обязательна (критерий TASKS T16)
            ShellEvent::Suspending => {
                tracing::info!("система уходит в сон — форс-сейв канваса");
                self.scene.save_now();
            }
            // Прогрев кадра: после сна первый кадр мог не прийти от winit
            ShellEvent::Resumed { kind } => {
                tracing::info!(?kind, "система проснулась");
                self.request_redraw();
            }
            // Потребитель — T17 (R7): PID Shell_TrayWnd отличает краш
            // от DPI-смены (TaskbarCreated приходит и на смену темы);
            // настоящий рестарт → FullReattach, анти-флуд 30 с
            ShellEvent::ExplorerStarted => self.on_explorer_started(),
            // Декод HSHELL_* — T18 (план §3); лог не спамим — debug
            ShellEvent::ShellHook { code, hwnd } => {
                tracing::debug!(code, hwnd, "shell-hook (декод — T18)");
            }
            // Задел «вставить как ноду» (SPEC §7.6) — потребитель будущего
            ShellEvent::ClipboardUpdated => {
                tracing::debug!("буфер обмена обновлён");
            }
            // SHCNE-мост (R12): тот же конвейер, что у вотчера T10 —
            // идемпотентен к дублям notify (§8.8); коалессер шины уже
            // сгладил шквал (150 мс, WM_TIMER-флаш)
            ShellEvent::FileEvents(events) => self.on_file_events(events),
        }
    }

    /// Восстановление встройки после разрушения WorkerW (T15): ReZOrder —
    /// перевыполнить только Z-order (raised, R2); FullReattach — полный
    /// re-attach (classic: SetParent на новый WorkerW). Анти-флуд:
    /// 3 подряд неудачи → стоп автоматики (полный анти-флуд — T17).
    #[cfg(windows)]
    fn recover_desktop(&mut self, action: canvas_shell::RecoveryAction) {
        use canvas_shell::desktop::{attach, hierarchy};
        if self.desktop_recover_failures >= 3 {
            // уже остановлены: не логировать спам — события могут идти потоком
            return;
        }
        let Some(raw) = self.window_hwnd() else {
            return;
        };
        let hwnd = Self::hwnd(raw);
        let result = (|| -> Result<(hierarchy::DesktopHierarchy, ()), attach::AttachError> {
            let progman = hierarchy::find_progman()?;
            let hier = hierarchy::ensure_worker_w(progman)?;
            match action {
                canvas_shell::RecoveryAction::ReZOrder => {
                    attach::refresh_z_order(hwnd, &hier).map(|_| (hier, ()))
                }
                canvas_shell::RecoveryAction::FullReattach => {
                    let screen = hierarchy::virtual_screen_rect()
                        .unwrap_or_else(|| canvas_shell::ScreenRect::from_ltrb(0, 0, 1280, 720));
                    attach::attach(hwnd, &hier, screen).map(|_| (hier, ()))
                }
                canvas_shell::RecoveryAction::None => Ok((hier, ())),
            }
        })();
        match result {
            Ok((hier, _)) => {
                tracing::info!(?action, strategy = ?hier.strategy, "встройка восстановлена");
                self.desktop_recover_failures = 0;
                // T17 (R7): DefView мог пересоздаться вместе с WorkerW —
                // guard репоинтится на живой слой иконок; hide
                // идемпотентен (SHELLSTATE персистентен) — до-скроет при
                // расхождении
                if let Some(guard) = self.icon_guard.as_mut() {
                    guard.repoint(hier.def_view);
                    guard.hide();
                }
                self.desktop_hierarchy = Some(hier);
                self.watch_worker_w(hwnd, &hier);
            }
            Err(err) => {
                self.desktop_recover_failures += 1;
                if self.desktop_recover_failures >= 3 {
                    tracing::warn!(
                        %err,
                        failures = self.desktop_recover_failures,
                        "re-attach не удаётся — автоматика восстановления остановлена"
                    );
                    self.desktop_hierarchy = None;
                } else {
                    tracing::warn!(%err, failures = self.desktop_recover_failures, "re-attach не удался");
                }
            }
        }
    }

    /// Scale factor окна (1.0 до создания окна). В desktop-режиме после
    /// attach — из desktop_dpi (GetDpiForWindow, R10: winit врёт после
    /// репарентинга), иначе — scale_factor окна.
    fn scale_factor(&self) -> f32 {
        let window_scale = self
            .window
            .as_ref()
            .map(|w| w.scale_factor() as f32)
            .unwrap_or(1.0);
        #[cfg(windows)]
        {
            crate::ui::effective_scale(window_scale, self.desktop_dpi)
        }
        #[cfg(not(windows))]
        {
            window_scale
        }
    }

    /// zoom * scale_factor — перевод world-px в физические (для буфера редактора).
    fn zoom_px(&self) -> f32 {
        self.camera.zoom() * self.scale_factor()
    }

    /// Начать редактирование текстовой ноды (T7) или подписи группы:
    /// text-нода редактирует `text`, группа — `label` (двойной клик).
    /// Прочие ноды игнорируются.
    fn begin_editing(&mut self, index: usize) {
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        let is_group = node.kind() == NodeKind::Group;
        if node.kind() != NodeKind::Text && !is_group {
            return;
        }
        let text = if is_group {
            node.label.clone().unwrap_or_default()
        } else {
            node.text.clone().unwrap_or_default()
        };
        let (_, width, height) = body_area(node);
        // FR-061 хвосты (D-8, «Раскрыть+авто»): начало правки сворачивает
        // раскрытые описания (тело ноды рисует буфер редактора — I-5).
        self.scene.collapse_descs_except(None);
        // FR-006: отложенный снапшот «до» правки — шаг закроется на commit
        // с фактическим изменением текста (finish_editing)
        self.begin_pending_undo();
        let zoom_px = self.zoom_px();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let session = EditingSession::new(
            renderer.font_system_mut(),
            EditTarget::Node(index),
            &text,
            width * zoom_px,
            height * zoom_px,
            zoom_px,
        );
        self.editing = Some(session);
        // FR-021: popup подсказок — с чистого листа на каждую правку
        self.hints.reset();
        self.selected = Some(Selection::Node(index));
        self.dragging = None;
        // Давняя заметка могла переполниться до нас (загрузка из файла) —
        // подгоняем размер сразу при входе в редактирование. Группу под
        // текст не подгоняем: рамку ресайзит только пользователь.
        if !is_group {
            self.fit_note_size();
        }
        self.sync_cursor_icon();
        self.request_redraw();
    }

    /// FR-072: двойной клик по text-ноде — выбор цели по точке: шапка
    /// (верхние HEADER_HEIGHT world-px карточки) — правка заголовка,
    /// тело — правка текста (как раньше). Группы — подпись label,
    /// файлы/виджеты — прежнее поведение begin_editing (no-op/фильтры).
    fn begin_edit_node(&mut self, index: usize, world: Vec2) {
        let title_hit =
            self.scene.canvas.nodes.get(index).is_some_and(|node| {
                node.kind() == NodeKind::Text && world[1] < node.y + HEADER_HEIGHT
            });
        if title_hit {
            self.begin_editing_title(index);
        } else {
            self.begin_editing(index);
        }
    }

    /// FR-072: начать правку ЗАГОЛОВКА text-ноды (EditTarget::NodeTitle):
    /// однострочный редактор в шапке карточки. Prefill — текущий заголовок:
    /// явный (canvasdesk.title как есть) либо производный legacy (title_for —
    /// стрипнутая первая строка); «—» → пустое поле. Группы правят label
    /// прежним begin_editing; файлы/виджеты не редактируются.
    fn begin_editing_title(&mut self, index: usize) {
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        if node.kind() != NodeKind::Text {
            return;
        }
        let text = match node.title() {
            Some(title) => title.to_owned(),
            None => {
                let derived = title_for(node);
                if derived == "—" {
                    String::new()
                } else {
                    derived
                }
            }
        };
        let (_, width, height) = canvas_render::text::title_edit_area(node);
        // FR-061 хвосты (D-8): начало правки сворачивает раскрытые описания
        self.scene.collapse_descs_except(None);
        // FR-006: отложенный снапшот «до» правки — шаг закроется на commit
        // (finish_editing) с фактическим изменением заголовка/тела
        self.begin_pending_undo();
        let zoom_px = self.zoom_px();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let session = EditingSession::new_title(
            renderer.font_system_mut(),
            index,
            &text,
            width * zoom_px,
            height * zoom_px,
            zoom_px,
        );
        self.editing = Some(session);
        // FR-021: popup подсказок не для заголовка — держим закрытым
        self.hints.reset();
        self.selected = Some(Selection::Node(index));
        self.dragging = None;
        self.sync_cursor_icon();
        self.request_redraw();
    }

    /// Начать редактирование лейбла связи (T8): двойной клик по линии.
    /// Бокс редактирования — по центру дуги связи (edge_edit_area).
    fn begin_editing_edge(&mut self, index: usize) {
        let Some(edge) = self.scene.canvas.edges.get(index) else {
            return;
        };
        let text = edge.label.clone().unwrap_or_default();
        let Some((_, width, height)) =
            edge_edit_area(&self.scene.canvas, index, self.settings.edges_avoid_nodes)
        else {
            return;
        };
        // FR-006: отложенный снапшот «до» правки лейбла (паттерн begin_editing)
        self.begin_pending_undo();
        let zoom_px = self.zoom_px();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let session = EditingSession::new(
            renderer.font_system_mut(),
            EditTarget::Edge(index),
            &text,
            width * zoom_px,
            height * zoom_px,
            zoom_px,
        );
        self.editing = Some(session);
        self.hints.reset();
        self.selected = Some(Selection::Edge(index));
        self.dragging = None;
        self.sync_cursor_icon();
        self.request_redraw();
    }

    /// Подрастить высоту редактируемой заметки под контент (T7): текст
    /// переносится по ширине карточки (Wrap::WordOrGlyph), за край не
    /// уходит — растёт только высота. Ширина карточки за пользователем:
    /// авто-растягивание по самой длинной строке убрано (правило «перенос
    /// даже одной строки»). Только рост. Только text-ноды: группы под текст
    /// не подгоняются (рамку ресайзит пользователь). Для лейблов связей (T8)
    /// не применяется — бокс фиксированный.
    /// CR-012 (правка 2): высота тела — измерением (`measure_body_height`) —
    /// теми же шрифтами и mono-сегментацией формульных строк, что у рендера:
    /// буфер редактора шейпит текст без GFM-разбивки и mono-флага, переносы
    /// Numi-строк занижались, футер налезал на тело.
    fn fit_note_size(&mut self) {
        let Some(session) = self.editing.as_mut() else {
            return;
        };
        // FR-017: override-поле редактирует ОДНУ строку, а не весь текст —
        // фит по ней исказил бы высоту ноды.
        if self.whatif_override_line.is_some() {
            return;
        };
        let EditTarget::Node(index) = session.target() else {
            return;
        };
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        if node.kind() != NodeKind::Text {
            return;
        }
        // FR-013: резерв под строку результата формулы (футер карточки),
        // чтобы подрезка тела результатом не прятала последнюю строку.
        // При активном редактировании — живой текст сессии.
        let live_text = session.text();
        // FR-013 (правка 2): резерв футера — только для программного итога
        // (MCP-expr без формульных строк в тексте). Построчные результаты
        // Numi-стиля места не требуют — ложатся на свои строки.
        // FR-023: у шаблонной ноды итог формулы шаблона показывается ВСЕГДА
        // (text.rs: правило `is_template_node`), поэтому резерв — всегда,
        // независимо от построчных результатов листа параметров.
        // CR-012 (правка 2): formula_lines — из живых построчных исходов
        // текста сессии; у шаблонной ноды это все строки-формулы листа
        // параметров (присваивания дают результат Ok/Err — все Some).
        let line_results = expr::eval_lines(&live_text);
        let has_line_results = line_results.iter().any(Option::is_some);
        let formula_lines = formula_line_indices(&line_results);
        let is_template = node.template().is_some();
        let program_footer = node.expr().is_some() && !has_line_results;
        let expr_footer = if is_template || program_footer {
            RESULT_LINE_HEIGHT + 2.0
        } else {
            0.0
        };
        let body_width = (node.width - BODY_PADDING * 2.0).max(0.0);
        // D-8: при правке тело И зона описания скрыты (I-5 деградация) —
        // мера без desc; после commit высоту догонит refit сцены.
        let body_h = measure_body_height(&live_text, body_width, &formula_lines, "", false, "");
        let needed_h = HEADER_HEIGHT + BODY_TOP_GAP + body_h + BODY_PADDING + expr_footer;
        let Some(node) = self.scene.canvas.nodes.get_mut(index) else {
            return;
        };
        if needed_h > node.height + 0.5 {
            node.height = needed_h;
            self.scene.spatial.update(index, node);
            self.scene.mark_dirty();
        }
    }

    /// Завершить редактирование (T7/T8): commit — записать текст в модель и
    /// пометить канвас грязным (автосейв); cancel — откат, модель не менялась.
    /// Для связи (T8) и подписи группы пустой лейбл при commit сбрасывается
    /// в None.
    fn finish_editing(&mut self, commit: bool) {
        let Some(session) = self.editing.take() else {
            return;
        };
        // FR-021: сессия закрыта — popup подсказок больше не нужен
        self.hints.reset();
        self.editor_dragging = false;
        // FR-017: override-поле строки — commit идёт в подмены активного
        // сценария, а НЕ в текст ноды (инвариант 2: база не мутируется).
        if let Some(line) = self.whatif_override_line.take() {
            if commit && session.changed() {
                if !self.scene.whatif_active {
                    self.scene.whatif_active = true;
                }
                if self.scene.active_scenario.is_none() {
                    // Подмене нужен сценарий: автосоздание (персистентно,
                    // один undo-шаг — паттерн MCP whatif_set_override).
                    // CR-016: имя — пустое, scene выбирает первый свободный
                    // номер (иначе len+1 коллидирует с существующими).
                    let snapshot = self.scene.canvas.clone();
                    match self.scene.whatif_create_scenario("") {
                        Ok(index) => {
                            self.scene.active_scenario = Some(index);
                            canvas_core::whatif::scenarios_to_canvas(
                                &mut self.scene.canvas,
                                &self.scene.scenarios,
                            );
                            if self.scene.canvas != snapshot {
                                self.scene.push_undo(snapshot);
                                self.scene.mark_dirty();
                            }
                        }
                        Err(err) => {
                            self.pending_undo = None;
                            self.show_toast(err);
                            self.sync_cursor_icon();
                            self.request_redraw();
                            return;
                        }
                    }
                }
                let index = self.scene.active_scenario.expect("сценарий активен");
                let node_id = match session.target() {
                    EditTarget::Node(index) => self
                        .scene
                        .canvas
                        .nodes
                        .get(index)
                        .map(|node| node.id.clone()),
                    EditTarget::Edge(_) | EditTarget::NodeTitle(_) => None,
                };
                if let Some(node_id) = node_id {
                    let expr = session.text();
                    if let Some(scenario) = self.scene.scenarios.get_mut(index) {
                        scenario.line_exprs.insert((node_id, line), expr);
                    }
                    self.whatif_list_open = true;
                    self.scene.recompute_flow();
                }
            }
            // Снапшот «до» begin_editing не нужен: текст ноды не менялся
            // (undo-шаг — только автосоздание сценария выше).
            self.pending_undo = None;
            self.sync_cursor_icon();
            self.request_redraw();
            return;
        }
        if commit && session.changed() {
            // FR-006: правка состоялась — отложенный снапшот «до» в историю
            // (мутация ниже); cancel-ветка дропнет его
            if let Some(snapshot) = self.pending_undo.take() {
                self.scene.push_undo(snapshot);
            }
            match session.target() {
                EditTarget::Node(index) => {
                    let node_id = self.scene.canvas.nodes.get(index).map(|n| n.id.clone());
                    if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                        if node.kind() == NodeKind::Group {
                            // Подпись группы: пустая — сброс в None
                            let text = session.text();
                            let text = text.trim();
                            node.label = if text.is_empty() {
                                None
                            } else {
                                Some(text.to_owned())
                            };
                        } else {
                            let text = session.text();
                            node.text = Some(text.clone());
                            // FR-013: строки «= …» — формула (смешанный
                            // редактор); commit выводит canvasdesk.expr и
                            // пересчитывает строку результата (один undo-шаг
                            // вместе с текстом — паттерн FR-006 выше)
                            node.set_expr(split_formula_lines(&text));
                            // FR-018: у шаблонной ноды текст — Numi-лист
                            // параметров; правка синхронизирует
                            // canvasdesk.template.params (id/version/expr
                            // сохраняются), propagator пересчитает
                            // формулу шаблона с новыми значениями.
                            // FR-023: слияние вместо замены — параметры,
                            // чьих строк нет в правке (удалены/переименованы/
                            // временно сломаны), сохраняются: формула
                            // шаблона остаётся вычислимой, итог (единица,
                            // напр. sec) не пропадает
                            if node.template().is_some() {
                                let fresh = canvas_core::templates::params_from_text(&text);
                                let params = canvas_core::templates::merge_params(
                                    node.template_params(),
                                    fresh,
                                );
                                node.set_template_params(params);
                            }
                        }
                    }
                    // FR-013: пересчёт результата (после мутации модели);
                    // FR-014: живой пересчёт downstream (весь граф — дёшево)
                    if node_id.is_some() {
                        self.scene.recompute_flow();
                    }
                }
                EditTarget::Edge(index) => {
                    if let Some(edge) = self.scene.canvas.edges.get_mut(index) {
                        let text = session.text();
                        let text = text.trim();
                        edge.label = if text.is_empty() {
                            None
                        } else {
                            Some(text.to_owned())
                        };
                        // Кэш лейблов в TextSystem перешейпится сам:
                        // ключ свежести — равенство текста (text.rs)
                    }
                }
                // FR-072: коммит заголовка. Пустая строка — осознанное
                // Some("") (плейсхолдер, утечки первой строки нет), НЕ None.
                // Настоящее переименование legacy-ноды (заголовка явного не
                // было, коммит непустой): первая строка тела — если она
                // проза (не Numi-формула) и нода не шаблонная — переезжает в
                // заголовок (remove_first_line), дубли в теле не остаётся;
                // один undo-шаг вместе с заголовком (снапшот «до» общий).
                EditTarget::NodeTitle(index) => {
                    let text = session.text();
                    let text = text.trim().to_owned();
                    if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                        let migrated = node.title().is_none()
                            && !text.is_empty()
                            && node.template().is_none()
                            && node
                                .text
                                .as_deref()
                                .and_then(|body| body.lines().next())
                                .filter(|line| !line.is_empty())
                                .is_some_and(|first| {
                                    !matches!(
                                        canvas_core::expr::line_kind(first),
                                        canvas_core::expr::NumiLineKind::Assignment { .. }
                                            | canvas_core::expr::NumiLineKind::Expression
                                    )
                                });
                        node.set_title(Some(text));
                        if migrated {
                            node.remove_first_line();
                            let body = node.text.clone().unwrap_or_default();
                            node.set_expr(split_formula_lines(&body));
                        }
                    }
                    // Тело могло измениться (перенос первой строки) —
                    // пересчёт потока значений, как после правки текста
                    self.scene.recompute_flow();
                }
            }
            self.scene.mark_dirty();
        } else {
            // FR-006: отмена правки — модель не менялась, отложенный
            // снапшот «до» дропается (no-op шагов в истории нет)
            self.pending_undo = None;
        }
        // FR-072: после коммита заголовка новой заметки — сразу правка тела
        // (создание = заголовок → Enter → тело); Esc тело не открывает.
        if commit {
            let chain = self.title_then_body.take();
            if let Some(index) = chain {
                if self.scene.canvas.nodes.get(index).is_some() {
                    self.begin_editing(index);
                    return;
                }
            }
        } else {
            self.title_then_body = None;
        }
        self.sync_cursor_icon();
        self.request_redraw();
    }

    // --- FR-017 (CP6): what-if режим — вход/выход, нижний бар, override ---

    /// Вход в what-if режим (пилюля бара / Ctrl+Shift+I / меню канваса).
    fn enter_whatif_mode(&mut self) {
        if self.scene.whatif_active {
            return;
        }
        self.scene.whatif_active = true;
        self.scene.recompute_flow();
        self.request_redraw();
    }

    /// Выход из режима (Esc / ✕): подмены НЕ теряются — они в персистентных
    /// сценариях `.canvas` (Q3b: подтверждение не требуется). Активный
    /// сценарий сбрасывается на «Базу», панели гаснут.
    fn exit_whatif_mode(&mut self) {
        if !self.scene.whatif_active {
            return;
        }
        if self.whatif_override_line.is_some() {
            self.finish_editing(false);
        }
        self.scene.whatif_active = false;
        self.whatif_list_open = false;
        self.whatif_compare_open = false;
        self.scene.whatif_activate(None);
        self.request_redraw();
    }

    /// Геометрия нижнего бара режима (имена сценариев + счётчик подмен).
    /// FR-053 (U3): ширины — измеренные TextMeasurer'ом (шейпинг теми же
    /// метриками, что рендер); счётчик — i18n-строка приложения (ширина
    /// считается по ТОЙ ЖЕ строке, что рисуется).
    fn whatif_bar_layout(&self) -> whatif_ui::BarLayout {
        let viewport = self.viewport_logical();
        // FR-064 P2: замороженные сценарии — маркер «❄» в подписи чипа
        // (ширина чипа измеряется по той же строке, что рисуется).
        let names: Vec<String> = self
            .scene
            .scenarios
            .iter()
            .map(|scenario| {
                if self.scene.whatif_is_frozen(&scenario.name) {
                    format!("{} ❄", scenario.name)
                } else {
                    scenario.name.clone()
                }
            })
            .collect();
        let count = self.scene.whatif_override_count();
        let counter_label = self.trf(keys::WHATIF_OVERRIDES, &[("{count}", &count.to_string())]);
        // FR-064 P2: лейбл кнопки заморозки — по состоянию активного сценария
        // (измеряется та же строка, что рисуется — фикс FR-053).
        let freeze_label = match self.scene.active_scenario {
            Some(index)
                if self
                    .scene
                    .scenarios
                    .get(index)
                    .is_some_and(|scenario| self.scene.whatif_is_frozen(&scenario.name)) =>
            {
                self.tr(keys::WHATIF_UNFREEZE)
            }
            _ => self.tr(keys::WHATIF_FREEZE),
        };
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        whatif_ui::bar_layout(
            &names,
            &counter_label,
            freeze_label,
            viewport,
            &mut measurer,
            &mut fs,
        )
    }

    /// Подпись ноды для панелей what-if: первая строка текста (обрезка),
    /// фолбэк — id (для нод без текста).
    fn whatif_node_label(&self, node_id: &str) -> String {
        self.scene
            .canvas
            .node(node_id)
            .map(|node| {
                node.text
                    .as_deref()
                    .and_then(|text| text.lines().next())
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .unwrap_or(&node.id)
                    .chars()
                    .take(24)
                    .collect()
            })
            .unwrap_or_else(|| node_id.to_owned())
    }

    /// FR-017: строки раскрытого списка подмен активного сценария
    /// (`нода → строка i: было → стало` + маркер протухания Q5c).
    fn whatif_override_rows(&self) -> Vec<WhatIfOverrideRow> {
        let Some(index) = self.scene.active_scenario else {
            return Vec::new();
        };
        let Some(scenario) = self.scene.scenarios.get(index) else {
            return Vec::new();
        };
        let stale = canvas_core::whatif::validate_scenario(&self.scene.canvas, scenario);
        let mut entries: Vec<(&(String, usize), &String)> = scenario.line_exprs.iter().collect();
        entries.sort();
        entries
            .into_iter()
            .map(|((node_id, line), expr)| {
                let base = self
                    .scene
                    .canvas
                    .node(node_id)
                    .and_then(|node| node.text.as_deref())
                    .and_then(|text| text.split('\n').nth(*line))
                    .unwrap_or_default()
                    .trim()
                    .to_owned();
                WhatIfOverrideRow {
                    node: node_id.clone(),
                    node_label: self.whatif_node_label(node_id),
                    line: *line,
                    base,
                    whatif: expr.clone(),
                    stale: stale
                        .iter()
                        .find(|entry| &entry.node == node_id && entry.line == *line)
                        .map(|entry| entry.reason.clone()),
                }
            })
            .collect()
    }

    /// Снять подмену строки списка (runtime-only, `.canvas` не трогаем —
    /// инвариант 2; пересчёт — живой).
    fn whatif_remove_override(&mut self, row: usize) {
        let rows = self.whatif_override_rows();
        let Some(entry) = rows.get(row) else {
            return;
        };
        let key = (entry.node.clone(), entry.line);
        if let Some(index) = self.scene.active_scenario {
            if let Some(scenario) = self.scene.scenarios.get_mut(index) {
                scenario.line_exprs.remove(&key);
            }
        }
        self.scene.recompute_flow();
    }

    /// FR-017: индекс строки расчёта под world-точкой (двойной клик в
    /// режиме → override-поле). Измерение — тем же `measure_body_height`,
    /// что у фита высоты и рендера (переносы учтены): кумулятивная высота
    /// первых k строк против y в теле ноды. Строка должна быть расчётной
    /// (иначе проза — toast по месту вызова).
    fn calc_line_at(&self, index: usize, world: Vec2) -> Option<usize> {
        let node = self.scene.canvas.nodes.get(index)?;
        let text = node.text.as_deref()?.trim_end_matches('\n');
        if text.is_empty() {
            return None;
        }
        let (origin, width, _) = body_area(node);
        // FR-061 этап D (D-8): зона описания — НАД телом (первая зона);
        // hit-тест строк смещается на её высоту (та же формула стека).
        let desc = node
            .canvasdesk
            .as_ref()
            .and_then(|ext| ext.desc.clone())
            .or_else(|| {
                node.template()
                    .and_then(|t| self.templates.find(&t.id).map(|m| m.description.clone()))
            })
            .unwrap_or_default();
        let desc_zone_h = if desc.trim().is_empty() {
            0.0
        } else {
            // FR-069: раскрытое описание — полная вёрстка (hit-зоны строк
            // смещаются на фактическую высоту зоны, I-2)
            let expanded = self.scene.desc_expanded.contains(&node.id);
            measure_body_height("", width, &[], &desc, expanded, "")
        };
        let rel_y = world[1] - origin[1] - desc_zone_h;
        if rel_y < 0.0 {
            return None;
        }
        let line_count = text.split('\n').count();
        let results = expr::eval_lines(text);
        let formula = formula_line_indices(&results);
        for k in 0..line_count {
            let prefix = text.split('\n').take(k + 1).collect::<Vec<_>>().join("\n");
            let formula_prefix: Vec<usize> = formula.iter().copied().filter(|i| *i <= k).collect();
            let cumulative = measure_body_height(&prefix, width, &formula_prefix, "", false, "");
            let is_last = k == line_count - 1;
            if rel_y < cumulative || is_last {
                let calc = results.get(k).is_some_and(Option::is_some)
                    || text
                        .split('\n')
                        .nth(k)
                        .map(|line| line.trim().contains('='))
                        .unwrap_or(false);
                return calc.then_some(k);
            }
        }
        None
    }

    /// FR-017: открыть override-поле строки (двойной клик по строке
    /// расчёта в режиме). Поле предзаполнено текущей подменой (или
    /// исходником строки); commit идёт в активный сценарий, а не в текст
    /// ноды (инвариант 2: база не мутируется). Подсказки FR-021 работают
    /// без правок (контекст тот же).
    fn begin_whatif_override(&mut self, index: usize, line: usize) {
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        let text = node.text.clone().unwrap_or_default();
        let source = text
            .split('\n')
            .nth(line)
            .unwrap_or_default()
            .trim()
            .to_owned();
        let preset = self
            .scene
            .active_scenario
            .and_then(|i| self.scene.scenarios.get(i))
            .and_then(|scenario| scenario.line_exprs.get(&(node.id.clone(), line)))
            .cloned()
            .unwrap_or(source);
        let (_, width, height) = body_area(node);
        let zoom_px = self.zoom_px();
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let session = EditingSession::new(
            renderer.font_system_mut(),
            EditTarget::Node(index),
            &preset,
            width * zoom_px,
            height * zoom_px,
            zoom_px,
        );
        self.editing = Some(session);
        self.whatif_override_line = Some(line);
        self.hints.reset();
        self.selected = Some(Selection::Node(index));
        self.dragging = None;
        self.sync_cursor_icon();
        self.request_redraw();
    }

    /// FR-017: обработка клика по what-if поверхностям (пилюля/бар/список/
    /// таблица). true — клик поглощён (канвасу не достаётся).
    fn whatif_bar_click(&mut self) -> bool {
        let viewport = self.viewport_logical();
        if !self.scene.whatif_active {
            if point_in_rect(whatif_ui::enter_pill_rect(viewport), self.cursor) {
                self.enter_whatif_mode();
                return true;
            }
            return false;
        }
        let layout = self.whatif_bar_layout();
        // Раскрытый список подмен — первый приоритет (над баром).
        if self.whatif_list_open {
            let rows = self.whatif_override_rows();
            let list = whatif_ui::overrides_list_layout(layout.rect, rows.len(), viewport);
            if let Some(row) = whatif_ui::override_row_at(list, rows.len(), self.cursor) {
                if point_in_rect(whatif_ui::remove_button_rect(list, row), self.cursor) {
                    self.whatif_remove_override(row);
                }
                self.request_redraw();
                return true;
            }
            if point_in_rect(list, self.cursor) {
                return true;
            }
        }
        if self.whatif_compare_open {
            let (columns, rows) = self.whatif_compare_table();
            let table = whatif_ui::table_layout(&columns, rows.len(), layout.rect, viewport);
            if point_in_rect(table.rect, self.cursor) {
                return true;
            }
        }
        if let Some(action) = whatif_ui::bar_action_at(&layout, self.cursor) {
            self.apply_whatif_bar_action(action);
            return true;
        }
        // Клик по телу бара (между элементами) — глотается, канвасу не уходит
        point_in_rect(layout.rect, self.cursor)
    }

    /// Вставить готовые ноды в модель (FR-003, паттерн insert_group):
    /// push + spatial index; `select` — выделить вставленные пачкой
    /// (CR-001). Возвращает индексы вставленных (порядок сохранён).
    fn insert_nodes(&mut self, nodes: Vec<Node>, select: bool) -> Vec<usize> {
        // FR-006: вставка (paste/duplicate/drop-планы) — undo-шаг
        self.push_undo();
        let mut indices = Vec::with_capacity(nodes.len());
        for node in nodes {
            self.scene.canvas.nodes.push(node);
            let index = self.scene.canvas.nodes.len() - 1;
            let node_ref = &self.scene.canvas.nodes[index];
            self.scene.spatial.insert(index, node_ref);
            indices.push(index);
        }
        if select {
            self.selected = None;
            self.selected_nodes = indices.clone();
        }
        self.scene.mark_dirty();
        // Файловые копии — вотчер/поиск должны увидеть директории (T10)
        self.sync_watch_dirs();
        self.request_redraw();
        indices
    }

    /// Push undo-снапшота текущего состояния (FR-006): вызывать
    /// непосредственно ПЕРЕД мутацией модели.
    fn push_undo(&mut self) {
        let snapshot = self.scene.canvas.clone();
        self.scene.push_undo(snapshot);
    }

    /// Начать отложенное действие (FR-006): drag/resize/редактирование —
    /// снапшот «до» запоминается на старте; пуш в историю только при
    /// фактическом изменении (см. finish_interaction_undo / finish_editing).
    fn begin_pending_undo(&mut self) {
        self.pending_undo = Some(self.scene.canvas.clone());
    }

    /// Закрыть отложенное действие drag/resize (FR-006): вызывается на
    /// отпускании ЛКМ и при прерывании drag отпусканием Space. Push только
    /// если геометрия реально изменилась — клик без движения не шаг.
    fn finish_interaction_undo(&mut self) {
        let Some(snapshot) = self.pending_undo.take() else {
            return;
        };
        // Drag: позиции нод отличаются от исходных (origins хранит «до»)
        let moved = self.dragging.as_ref().is_some_and(|drag| {
            drag.origins.iter().any(|(index, origin)| {
                self.scene
                    .canvas
                    .nodes
                    .get(*index)
                    .is_some_and(|node| node.x != origin[0] || node.y != origin[1])
            })
        });
        // Resize: размеры отличаются от снапшотных (кламп мог дать те же)
        let resized = self.resizing.is_some_and(|index| {
            self.scene
                .canvas
                .nodes
                .get(index)
                .zip(snapshot.nodes.get(index))
                .is_some_and(|(now, before)| {
                    now.width != before.width || now.height != before.height
                })
        });
        if moved || resized {
            self.scene.push_undo(snapshot);
        }
    }

    // --- FR-038 (T-038.4): магнитная раскладка — методы App ----------------

    /// Snap-расчёт кадра драга (п.2/4/15): bbox набора по origins, кандидаты
    /// (видимые ноды вне перемещаемого набора, с запасом на допуск/зазор),
    /// конфиг из настроек + зума; live-collision клампит дельту кадра (п.15),
    /// полный outcome (grid/guides/anchor + collision) — для предпросмотра и
    /// отпускания. Мастер-тумблер выключен или bbox пуст — None (снапа нет).
    fn compute_snap_frame(&self, drag: &DragState, delta: [f32; 2]) -> Option<SnapFrame> {
        if !self.settings.snap_enabled {
            return None;
        }
        let bbox = drag_bbox(&self.scene.canvas, &drag.origins)?;
        let zoom = self.camera.zoom();
        let (minor, major) = self.settings.grid_density.steps();
        let cfg = SnapConfig {
            to_grid: self.settings.snap_to_grid,
            to_guides: self.settings.snap_to_guides,
            grid_minor: minor,
            grid_major: major,
            tolerance_px: self.settings.snap_tolerance_px,
            zoom,
            sub_zoom: self.settings.snap_grid_sub_zoom,
            coarse_zoom: self.settings.snap_grid_coarse_zoom,
            collision_gap: if self.settings.snap_collision {
                COLLISION_GAP
            } else {
                0.0
            },
        };
        let tolerance_world = snap_tolerance_world(cfg.tolerance_px, zoom);
        // Кандидаты — видимые ноды с запасом на допуск и зазор collision:
        // за пределами запаса ни направляющая, ни кламп задеть не могут
        let viewport = self.viewport_logical();
        let mut visible = self.camera.visible_world_rect(viewport);
        let margin = tolerance_world.max(cfg.collision_gap);
        visible[0] -= margin;
        visible[1] -= margin;
        visible[2] += margin;
        visible[3] += margin;
        let moving: Vec<usize> = drag.origins.iter().map(|(index, _)| *index).collect();
        let candidates = snap_candidates(
            &self.scene.canvas,
            &self.scene.spatial.query_rect(visible),
            &moving,
        );

        // Live-collision (п.15): grid/guides остаются release-time, поэтому
        // клампим ТОЛЬКО дельту кадра — движение останавливается на границе
        // зазора, свободный курсор не «уводит» ноду сквозь соседей
        let eff_delta = if cfg.collision_gap > 0.0 {
            let (fx, fy) = clamp_collision(
                bbox,
                bbox.x + delta[0],
                bbox.y + delta[1],
                &candidates,
                cfg.collision_gap,
                delta[0],
                delta[1],
            );
            [fx, fy]
        } else {
            delta
        };

        // Полный расчёт: отпускание применит outcome.dx/dy как итоговую дельту
        let outcome = snap_with_anchor(
            bbox,
            eff_delta[0],
            eff_delta[1],
            &candidates,
            &cfg,
            self.settings.snap_anchor,
        );
        let bbox_current = [
            bbox.x + eff_delta[0],
            bbox.y + eff_delta[1],
            bbox.x + bbox.w + eff_delta[0],
            bbox.y + bbox.h + eff_delta[1],
        ];
        Some(SnapFrame {
            eff_delta,
            outcome,
            bbox_current,
            tolerance_world,
        })
    }

    /// Предпросмотр снапа в кадре драга (п.2/8/9): направляющие + ghost
    /// snapped-позиции в рендере; снапа нет — слой гасится (п.9: за допуском
    /// и у выключенного мастера линий нет). Коррекция относительно позиции
    /// кадра: ghost = текущий bbox + коррекция, интенсивность (п.8) — по
    /// модулю коррекции (не по полной дельте от захвата).
    fn update_snap_preview(&mut self, frame: Option<&SnapFrame>) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        let Some(frame) = frame else {
            renderer.clear_guides();
            return;
        };
        let corr_x = frame.outcome.dx - frame.eff_delta[0];
        let corr_y = frame.outcome.dy - frame.eff_delta[1];
        if frame.outcome.guides_x.is_empty()
            && frame.outcome.guides_y.is_empty()
            && corr_x == 0.0
            && corr_y == 0.0
        {
            renderer.clear_guides();
            return;
        }
        let source = match frame.outcome.source {
            SnapSource::Grid => GuideSource::Grid,
            SnapSource::Guide => GuideSource::Guide,
        };
        renderer.set_guides(GuidesFrame::from_snap(
            frame.bbox_current,
            corr_x,
            corr_y,
            frame.outcome.guides_x.clone(),
            frame.outcome.guides_y.clone(),
            source,
            frame.tolerance_world,
        ));
    }

    /// Snap-at-release (п.2/21): на отпускании drag применить snap-коррекцию
    /// к свободной позиции. Возвращает true, если коррекция применена —
    /// тогда drag-шаг undo уже закрыт здесь; false — вызывающий делает
    /// обычный одиночный [`Self::finish_interaction_undo`].
    ///
    /// Undo (п.21): при сработавшем снапе — ДВА шага: (1) `finish_interaction_undo`
    /// пушит снапшот начала драга (шаг «drag: старт → свободная позиция»),
    /// (2) `push_undo` фиксирует состояние до коррекции и шаг «snap-коррекция».
    /// Снап не сработал (коррекция нулевая) — один шаг, как раньше; клик без
    /// движения — ни одного (телепорт по сетке на клике был бы сюрпризом).
    fn apply_snap_at_release(&mut self) -> bool {
        let Some(drag) = self.dragging.clone() else {
            return false;
        };
        if !self.settings.snap_enabled {
            return false;
        }
        // Фактическое перемещение — тот же предикат, что в finish_interaction_undo
        let moved = drag.origins.iter().any(|(index, origin)| {
            self.scene
                .canvas
                .nodes
                .get(*index)
                .is_some_and(|node| node.x != origin[0] || node.y != origin[1])
        });
        if !moved {
            return false;
        }
        // Финальная дельта — от той же геометрии, что последний кадр драга:
        // результат бит-в-бит совпадает с предпросмотром (п.20)
        let world = self.cursor_world();
        let delta = [world[0] - drag.grab_world[0], world[1] - drag.grab_world[1]];
        let Some(frame) = self.compute_snap_frame(&drag, delta) else {
            return false;
        };
        let corr_x = frame.outcome.dx - frame.eff_delta[0];
        let corr_y = frame.outcome.dy - frame.eff_delta[1];
        if corr_x == 0.0 && corr_y == 0.0 {
            // Снап не сработал — один push_undo как сейчас (п.21)
            return false;
        }
        // Шаг 1 (FR-006): drag — снапшот начала → состояние до коррекции
        self.finish_interaction_undo();
        // Шаг 2 (п.21): snap-коррекция — отдельная undo-операция
        self.push_undo();
        for (index, origin) in &drag.origins {
            self.scene.move_node(
                *index,
                origin[0] + frame.outcome.dx,
                origin[1] + frame.outcome.dy,
            );
        }
        self.scene.mark_dirty();
        // п.9: направляющие скрываются сразу на отпускании
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.clear_guides();
        }
        self.request_redraw();
        true
    }

    /// Клавиатурный nudge выделения (п.22): стрелки — 1 px, Shift+стрелки —
    /// 10 px, ЭКРАННЫЕ px (обоснование — [`nudge_step_world`]). RAW-дельты
    /// поверх включённого снапа: snap_move НЕ вызывается — точная корректировка
    /// обязана обходить магниты (п.22 v2). Каждый шаг — отдельная undo-
    /// операция (FR-006: push_undo ДО мутации). Направляющие не показываются.
    fn nudge_selection(&mut self, dir: [f32; 2], screen_px: f32) {
        // Nudge не конкурирует с drag/resize: там origin+delta пересчитал бы
        // позиции поверх клавиатурного сдвига
        if self.dragging.is_some() || self.resizing.is_some() {
            return;
        }
        // Набор — как у drag: выделенная нода или весь набор (+ дети групп).
        // Рамка выделения оставляет selected = None при непустом наборе —
        // берём первичным первый из набора
        let primary = match self.selected {
            Some(Selection::Node(index)) => Some(index),
            _ => self.selected_nodes.first().copied(),
        };
        let Some(primary) = primary else {
            return;
        };
        let origins = drag_origins(&self.scene.canvas, primary, &self.selected_nodes);
        if origins.is_empty() {
            return;
        }
        let step = nudge_step_world(screen_px, self.camera.zoom());
        let dx = dir[0] * step;
        let dy = dir[1] * step;
        // Каждый nudge — отдельная undo-операция (п.21/22, FR-006)
        self.push_undo();
        for (index, origin) in &origins {
            self.scene.move_node(*index, origin[0] + dx, origin[1] + dy);
        }
        self.scene.mark_dirty();
        self.request_redraw();
    }

    // --- FR-038 (T-038.5): batch-операции выравнивания (п.16-17) ------------

    /// Видимость batch-пунктов в меню канваса (п.16: ТОЛЬКО при N≥3
    /// выделенных нодах; иначе скрыты).
    fn align_menu_visible(&self) -> bool {
        self.selected_nodes.len() >= ALIGN_MIN_SELECTION
    }

    /// Набор batch-операции (п.16): (юниты, ведомые по юнитам).
    ///
    /// Юнит — выделенная нода, НЕ являющаяся ребёнком другой выделенной
    /// группы; rect юнита — собственный bbox ноды (группа — её рамка:
    /// семантика «группа — обычная нода», как у кандидатов снапа, п.14).
    /// Ведомые — дети выделенных групп (механизм групп FR-012: дети следуют
    /// за родителем, как в `drag_origins`): прямой ребёнок юнита-группы и
    /// транзитивно дети выделенных групп среди ведомых — все с дельтой
    /// своего верхнего выделенного предка; ребёнок НЕвыделенной группы не
    /// двигается (v1-семантика drag без рекурсии в модель). Дубль ведомого
    /// (ребёнок двух выделенных групп) закрепляется за первым родителем
    /// в порядке выделения (детерминизм п.20). Чистое чтение модели.
    fn align_units(&self) -> (Vec<(usize, SnapRect)>, Vec<Vec<usize>>) {
        let canvas = &self.scene.canvas;
        let mut selection = self.selected_nodes.clone();
        selection.sort_unstable();
        selection.dedup();
        let is_group = |index: usize| {
            canvas
                .nodes
                .get(index)
                .is_some_and(|node| node.kind() == NodeKind::Group)
        };
        // Прямые дети выделенных групп — не самостоятельные юниты
        let mut following: Vec<usize> = Vec::new();
        for &index in &selection {
            if is_group(index) {
                for child in canvas_core::group_children(canvas, index) {
                    if !following.contains(&child) {
                        following.push(child);
                    }
                }
            }
        }
        let units: Vec<(usize, SnapRect)> = selection
            .iter()
            .filter(|index| !following.contains(index))
            .filter_map(|&index| {
                canvas.nodes.get(index).map(|node| {
                    (
                        index,
                        SnapRect {
                            x: node.x,
                            y: node.y,
                            w: node.width,
                            h: node.height,
                        },
                    )
                })
            })
            .collect();
        // Ведомые по юнитам: обход от юнита — прямые дети группы-юнита +
        // дети выделенных групп среди ведомых (та же дельта, что у предка)
        let mut followers: Vec<Vec<usize>> = Vec::with_capacity(units.len());
        for (unit_index, _) in &units {
            let mut children: Vec<usize> = Vec::new();
            let mut queue: Vec<usize> = if is_group(*unit_index) {
                canvas_core::group_children(canvas, *unit_index)
            } else {
                Vec::new()
            };
            while let Some(child) = queue.pop() {
                if child == *unit_index || children.contains(&child) {
                    continue;
                }
                children.push(child);
                if selection.contains(&child) && is_group(child) {
                    queue.extend(canvas_core::group_children(canvas, child));
                }
            }
            followers.push(children);
        }
        (units, followers)
    }

    /// FR-038 п.16-17 (T-038.5): применить batch-операцию к выделению.
    ///
    /// ОДНА undo-операция независимо от числа нод (п.17): один `push_undo`
    /// (снапшот «до») непосредственно перед мутациями — НЕ pending_undo-
    /// модель драга. Перемещения — через `scene.move_node` (spatial-индекс
    /// обновляется на каждую ноду, паттерн nudge/delete_selected);
    /// `mark_dirty` — за ним автосейв. Ничего не сдвинулось — без
    /// undo-шага (паттерн `finish_interaction_undo`: пустых шагов нет).
    /// `axis: None` — только для распределения: ось выводится из контекста
    /// выделения ([`distribute_axis_for`], решение об одной кнопке меню).
    fn run_batch_op(&mut self, op: BatchOp, axis: Option<AlignAxis>) {
        // Страховка: пункты меню скрыты при N<3 — команды не доходят
        if self.selected_nodes.len() < ALIGN_MIN_SELECTION {
            return;
        }
        let (units, followers) = self.align_units();
        if units.len() < 2 {
            // Например, выделение «группа + её дети»: самостоятельный юнит
            // один — выравнивать/распределять нечего (п.16 про N≥3 нод)
            return;
        }
        let mut rects: Vec<SnapRect> = units.iter().map(|(_, rect)| *rect).collect();
        let axis = match axis {
            Some(axis) => axis,
            None => {
                debug_assert!(
                    matches!(op, BatchOp::Distribute),
                    "авто-ось — только у распределения"
                );
                distribute_axis_for(&rects)
            }
        };
        match op {
            BatchOp::Align => align_centers(&mut rects, axis),
            BatchOp::Distribute => distribute_evenly(&mut rects, axis),
        }
        // Дельты юнитов: новая позиция минус старая (x/y меняются, w/h нет)
        let deltas: Vec<[f32; 2]> = units
            .iter()
            .zip(&rects)
            .map(|((_, old), new)| [new.x - old.x, new.y - old.y])
            .collect();
        if deltas.iter().all(|[dx, dy]| *dx == 0.0 && *dy == 0.0) {
            return; // раскладка уже такая — без пустого undo-шага
        }
        // План перемещений: юниты + их ведомые (дельта родителя); дубль
        // ведомого двух юнитов — первый в порядке юнитов (детерминизм п.20)
        let mut moves: Vec<(usize, [f32; 2])> = Vec::new();
        for (u, ((index, _), delta)) in units.iter().zip(&deltas).enumerate() {
            moves.push((*index, *delta));
            for &child in &followers[u] {
                if !moves.iter().any(|(moved, _)| moved == &child) {
                    moves.push((child, *delta));
                }
            }
        }
        // ОДИН undo-шаг на всю batch-операцию (п.17, FR-006)
        self.push_undo();
        for (index, [dx, dy]) in moves {
            if let Some(node) = self.scene.canvas.nodes.get(index) {
                let (x, y) = (node.x, node.y);
                self.scene.move_node(index, x + dx, y + dy);
            }
        }
        self.scene.mark_dirty();
        self.request_redraw();
    }

    /// Отменить последнее действие (FR-006, Ctrl+Z): модель «до» из
    /// undo-стека, текущее состояние — в redo. PRD-0007 (AC-5.3): если
    /// верхний снапшот — пачка автосвязи, откат требует подтверждения:
    /// открывается диалог, связи пачки подсвечиваются на канвасе.
    fn undo_action(&mut self) {
        if self.scene.peek_undo_tag() == Some(AUTOLINK_UNDO_TAG) {
            // Подтверждение отката пачки: подсветка отменяемого (AC-5.3) —
            // id рёбер последнего бата (создание фиксирует их в
            // autolink_batch); тег гарантирует, что следующий undo — этот бат.
            let edge_ids = self.autolink_batch.clone().unwrap_or_default();
            let count = edge_ids.len();
            self.highlight_autolink_batch(&edge_ids);
            self.dialog = Some(AppDialog::AutolinkRollback { count, edge_ids });
            self.request_redraw();
            return;
        }
        self.autolink_batch = None;
        if let Some(before) = self.scene.take_undo() {
            self.restore_canvas(before);
            tracing::debug!(depth = self.scene.undo_stack.len(), "undo");
        }
    }

    /// Вернуть отменённое (FR-006, Ctrl+Y / Ctrl+Shift+Z).
    fn redo_action(&mut self) {
        if let Some(after) = self.scene.take_redo() {
            self.restore_canvas(after);
            tracing::debug!(depth = self.scene.redo_stack.len(), "redo");
        }
    }

    /// Восстановить снапшот (FR-006): модель + spatial + сброс кэшей
    /// (индексы из прошлых состояний недостоверны — паттерн
    /// delete_selected). Интеракции и редактирование прерываются без
    /// коммита; автосейв следует за mark_dirty.
    fn restore_canvas(&mut self, canvas: Canvas) {
        self.pending_undo = None;
        self.editing = None;
        self.editor_dragging = false;
        self.settle_anim = None; // FR-012: анимация не валидна после отката
                                 // FR-073: якоря не валидны после отката — сессия физики закрывается
        self.drag_push_live = false;
        self.drag_push.anchors.clear();
        self.group_drop_target = None;
        self.scene.canvas = canvas;
        self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
        // Динамический перерасчёт MeasuredReserveFn (T9-сессия 2026-09-24):
        // снапшот восстановил высоты нод; состояние (hash, height) хранит
        // значения ДО отката. Механизм самовосстанавливается через проверку
        // height_at_measurement, но явный сброс дешевле (не сравнивать
        // высоты каждой ноды) и безопаснее (undo/redo — частый путь).
        self.scene.reset_content_height_state();
        // FR-017: сценарии персистентны в снапшоте (undo whatif_apply
        // возвращает удалённый сценарий, undo create — убирает) —
        // синхронизируем runtime-список с восстановленным канвасом
        self.scene.scenarios = canvas_core::whatif::scenarios_from_canvas(&self.scene.canvas);
        if self
            .scene
            .active_scenario
            .is_some_and(|i| i >= self.scene.scenarios.len())
        {
            self.scene.active_scenario = None;
        }
        // FR-013: снапшот мог изменить формулы и топологию — живой
        // пересчёт графа потока (FR-014)
        self.scene.recompute_flow();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.invalidate_node_caches();
            // FR-038 (п.9): undo/redo прерывают drag — предпросмотр направляющих
            // не должен переживать восстановление состояния
            renderer.clear_guides();
        }
        self.thumbs_failed.clear();
        self.selected = None;
        self.selected_nodes.clear();
        self.dragging = None;
        self.resizing = None;
        self.menu = None;
        self.hovered = None;
        self.edge_drag = None;
        self.select_rect = None;
        self.scene.mark_dirty();
        // Файловый состав мог измениться — вотчер и SHCNE-подписки (T10)
        self.sync_watch_dirs();
        self.request_redraw();
    }

    /// Индексы выделенных нод (FR-003): набор мультивыделения ∪ primary,
    /// по возрастанию без дубликатов; пусто — ничего не выделено.
    fn selection_node_indices(&self) -> Vec<usize> {
        let mut indices = self.selected_nodes.clone();
        if let Some(Selection::Node(index)) = self.selected {
            if !indices.contains(&index) {
                indices.push(index);
            }
        }
        indices.sort_unstable();
        indices.dedup();
        indices
    }

    /// Скопировать выделенные ноды в буфер (FR-003, Ctrl+C): первичное
    /// взаимное расположение сохраняется — вставка центром bbox на курсор.
    fn copy_selection(&mut self) {
        let indices = self.selection_node_indices();
        if indices.is_empty() {
            return;
        }
        self.node_clipboard = indices
            .into_iter()
            .filter_map(|index| self.scene.canvas.nodes.get(index).cloned())
            .collect();
        tracing::debug!(
            count = self.node_clipboard.len(),
            "ноды скопированы в буфер"
        );
    }

    /// Вставить буфер (FR-003, Ctrl+V): копии с новыми id — центром bbox
    /// в позицию курсора; вставленное становится мультивыделением (CR-001).
    fn paste_clipboard(&mut self) {
        if self.node_clipboard.is_empty() {
            return;
        }
        let copies = reassign_ids(&self.scene.canvas, &self.node_clipboard);
        let placement = PastePlacement::AtCursor(self.cursor_world());
        let nodes = paste_nodes(&copies, placement);
        self.insert_nodes(nodes, true);
    }

    /// Дублировать выделение (FR-003, Ctrl+D): копии с новыми id со
    /// сдвигом DUPLICATE_OFFSET; копии становятся мультивыделением.
    fn duplicate_selection(&mut self) {
        let indices = self.selection_node_indices();
        if indices.is_empty() {
            return;
        }
        let originals = indices
            .iter()
            .filter_map(|&index| self.scene.canvas.nodes.get(index))
            .cloned()
            .collect::<Vec<Node>>();
        let copies = reassign_ids(&self.scene.canvas, &originals);
        let nodes = paste_nodes(
            &copies,
            PastePlacement::Offset([DUPLICATE_OFFSET, DUPLICATE_OFFSET]),
        );
        self.insert_nodes(nodes, true);
    }

    /// Сгруппировать выделенные ноды (Ctrl+G): группа с bbox по всему
    /// набору (мультивыделение ∪ primary, CR-001) + GROUP_PADDING; дети —
    /// явный список id (FR-012). Пустое выделение — no-op (семантика
    /// Figma/PowerPoint: группировать нечего — действие не срабатывает).
    /// Undo-шаг, spatial index и перенос выделения на новую группу —
    /// внутри insert_group (паттерн «Сгруппировать» палитры).
    fn group_selection(&mut self) {
        let indices = self.selection_node_indices();
        if indices.is_empty() {
            return;
        }
        if let Some(mut group) =
            plan_group_around_nodes(&self.scene.canvas, &indices, crate::ui::GROUP_PADDING)
        {
            // FR-040: подпись по умолчанию — через таблицу i18n
            group.label = Some(self.tr(keys::GROUP_DEFAULT_LABEL).to_owned());
            self.insert_group(group);
            self.request_redraw();
        }
    }

    /// Вырезать выделенные ноды (FR-007, Ctrl+X): копирование в буфер
    /// (FR-003) + удаление (undo-шаг — внутри delete_selected, FR-006).
    /// Вставка — обычным Ctrl+V: копии с новыми id (правила FR-003).
    fn cut_selection(&mut self) {
        let indices = self.selection_node_indices();
        if indices.is_empty() {
            return;
        }
        self.copy_selection();
        self.delete_selected();
        tracing::debug!(count = self.node_clipboard.len(), "ноды вырезаны в буфер");
    }

    /// Удалить выделенное (T8, Del; CR-001 — мультивыделение): набор нод —
    /// пачкой (canvas-core remove_nodes); связь — по id; одиночную ноду —
    /// каскадно со связями. После удаления нод индексы в canvas.nodes
    /// сдвигаются, поэтому spatial index перестраивается, а все кэши,
    /// ключованные usize (текст, атлас тамбнейлов, негативный кэш),
    /// сбрасываются полностью.
    fn delete_selected(&mut self) {
        // CR-001: мультивыделение — удаляем весь набор (рамка/Ctrl+клик)
        if !self.selected_nodes.is_empty() {
            // FR-006: удаление набора — undo-шаг
            self.push_undo();
            let indices = std::mem::take(&mut self.selected_nodes);
            let removed = self.scene.canvas.remove_nodes(&indices);
            if removed.is_empty() {
                return;
            }
            self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.invalidate_node_caches();
            }
            self.thumbs_failed.clear();
            self.selected = None;
            self.dragging = None;
            self.resizing = None;
            self.editing = None;
            self.menu = None;
            self.hovered = None;
            self.edge_drag = None;
            self.scene.mark_dirty();
            self.sync_watch_dirs();
            // FR-014: downstream удалённых нод — «вход отсутствует»
            self.scene.recompute_flow();
            self.request_redraw();
            // Редактирование прервано удалением — отложенный снапшот (FR-006)
            // больше не актуален: правки умрут вместе с нодой
            self.pending_undo = None;
            return;
        }
        match self.selected {
            Some(Selection::Edge(index)) => {
                let Some(edge) = self.scene.canvas.edges.get(index) else {
                    return;
                };
                let id = edge.id.clone();
                // FR-006: удаление связи — undo-шаг
                self.push_undo();
                self.scene.canvas.remove_edge(&id);
                self.selected = None;
                self.scene.mark_dirty();
                // FR-014: downstream этой связи — «вход отсутствует»
                self.scene.recompute_flow();
                self.request_redraw();
            }
            Some(Selection::Node(index)) => {
                // FR-006: удаление ноды — undo-шаг (снапшот ДО мутации)
                if self.scene.canvas.nodes.get(index).is_some() {
                    self.push_undo();
                }
                if self.scene.canvas.remove_node(index).is_none() {
                    return;
                }
                self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.invalidate_node_caches();
                }
                self.thumbs_failed.clear();
                self.selected = None;
                self.selected_nodes.clear();
                self.dragging = None;
                self.resizing = None;
                self.editing = None;
                self.menu = None;
                self.hovered = None;
                self.edge_drag = None;
                self.scene.mark_dirty();
                // Директории удалённых нод больше не нужны вотчеру (T10)
                self.sync_watch_dirs();
                // FR-014: downstream удалённой ноды — «вход отсутствует»
                self.scene.recompute_flow();
                self.request_redraw();
            }
            None => {}
        }
    }

    /// Создать пустую заметку в world-точке (T7): модель + spatial index.
    /// Возвращает индекс новой ноды.
    fn create_note_at(&mut self, world: Vec2) -> usize {
        // FR-006: создание заметки — undo-шаг
        self.push_undo();
        let id = next_free_id(&self.scene.canvas, "note");
        self.scene
            .canvas
            .nodes
            .push(Node::text(id, "", world[0], world[1]));
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        self.selected = Some(Selection::Node(index));
        self.selected_nodes.clear();
        self.scene.mark_dirty();
        index
    }

    /// Выборочный hit-test под world-точкой: сначала не-group ноды
    /// (меньшая площадь в приоритете — ребёнок группы раньше группы),
    /// затем группы. Кандидаты — точечный запрос spatial index.
    /// FR-011: скрытые ноды (свернутые поддеревья) из hit-test исключены.
    fn selective_hit(&self, world: Vec2) -> Option<usize> {
        let candidates = self
            .scene
            .spatial
            .query_rect([world[0], world[1], world[0], world[1]]);
        let hidden = self.hidden_subtree_nodes();
        let visible: Vec<usize> = candidates
            .into_iter()
            .filter(|index| hidden.binary_search(index).is_err())
            .collect();
        select_node_hit(&self.scene.canvas, &visible)
    }

    /// FR-025 (правка 2): построчная точка выхода под world-точкой.
    /// Кандидаты — ноды из spatial-индекса в прямоугольнике допуска зоны
    /// портов (CR-003) вокруг курсора; хост под курсором проверяется первым.
    /// Правка по проверке владельца: раньше hit-test был привязан к
    /// `hovered`, а кружки сидят НА краю ноды — половина каждого кружка
    /// торчит наружу, курсор правее края давал `hovered = None` и drag от
    /// строки не начинался вовсе (пользователь получал либо сторону-порт с
    /// узловым значением, либо ничего). Группы и скрытые поддеревья — мимо.
    fn line_port_hit(&self, world: Vec2) -> Option<(usize, canvas_core::LinePort)> {
        if !self.settings.line_ports {
            return None;
        }
        let renderer = self.renderer.as_ref()?;
        let zoom = self.camera.zoom();
        let tolerance = self.settings.port_zone_px / zoom.max(1e-3);
        let expanded = [
            world[0] - tolerance,
            world[1] - tolerance,
            world[0] + tolerance,
            world[1] + tolerance,
        ];
        let hidden = self.hidden_subtree_nodes();
        let mut candidates: Vec<usize> = self
            .scene
            .spatial
            .query_rect(expanded)
            .into_iter()
            .filter(|index| hidden.binary_search(index).is_err())
            .collect();
        if let Some(hovered) = self.hovered {
            if let Some(pos) = candidates.iter().position(|&index| index == hovered) {
                candidates.swap(0, pos);
            }
        }
        for index in candidates {
            let Some(node) = self.scene.canvas.nodes.get(index) else {
                continue;
            };
            if node.kind() == NodeKind::Group {
                continue;
            }
            let ports = renderer.line_ports(index, node);
            if let Some(port) =
                canvas_core::line_port_at(&ports, world, zoom, self.settings.port_zone_px)
            {
                return Some((index, port));
            }
        }
        None
    }

    /// FR-050 Н2 (этап C): якорь параметра шаблонной ноды под world-точкой
    /// (зеркало `line_port_hit`: кандидаты — spatial-индекс в прямоугольнике
    /// допуска зоны портов CR-003, хост под курсором первым; якоря — только
    /// у шаблонных нод, на ЛЕВОМ краю). Группы и скрытые поддеревья — мимо.
    fn param_port_hit(&self, world: Vec2) -> Option<(usize, canvas_core::ParamPort)> {
        let renderer = self.renderer.as_ref()?;
        let zoom = self.camera.zoom();
        let tolerance = self.settings.port_zone_px / zoom.max(1e-3);
        let expanded = [
            world[0] - tolerance,
            world[1] - tolerance,
            world[0] + tolerance,
            world[1] + tolerance,
        ];
        let hidden = self.hidden_subtree_nodes();
        let mut candidates: Vec<usize> = self
            .scene
            .spatial
            .query_rect(expanded)
            .into_iter()
            .filter(|index| hidden.binary_search(index).is_err())
            .collect();
        if let Some(hovered) = self.hovered {
            if let Some(pos) = candidates.iter().position(|&index| index == hovered) {
                candidates.swap(0, pos);
            }
        }
        for index in candidates {
            let Some(node) = self.scene.canvas.nodes.get(index) else {
                continue;
            };
            if node.template().is_none() {
                continue;
            }
            let ports = renderer.param_ports(index, node);
            if let Some(port) =
                canvas_core::param_port_at(&ports, world, zoom, self.settings.port_zone_px)
            {
                return Some((index, port.clone()));
            }
        }
        None
    }

    /// FR-045 F-5 v1: чистая сборка лейбла порта (без hit-теста —
    /// тестируема без рендера; правило владельца — данные отдельно от
    /// геометрии). Адресация qualified (R-5): полный путь в тултипе,
    /// объект — единая точка сборки имён [`canvas_core::dataref`]
    /// (коллизия имён — «Имя (node_id)», §Q2). None — лейбла нет.
    fn port_label_for(&self, node_index: usize, target: &PortTarget) -> Option<String> {
        let canvas = &self.scene.canvas;
        let node_id = canvas.nodes.get(node_index)?.id.clone();
        let obj = canvas_core::dataref::qualified_obj_name(
            canvas,
            &node_id,
            &canvas_core::dataref::display_name_counts(canvas),
        );
        Some(match target {
            // «Объект.строка N» — 1-based для отображения, как в dataref
            // (поле собирается i18n-ключом: RU «строка N», EN «line N»)
            PortTarget::Line(Some(line)) => {
                let n = (line + 1).to_string();
                format!("{obj}.{}", self.trf(keys::STAGE_LINE_LABEL, &[("{n}", &n)]))
            }
            // Футер шаблонной ноды — значение ноды целиком (F-5 «out:»)
            PortTarget::Line(None) => self.trf(keys::STAGE_OUT_LABEL, &[("{name}", &obj)]),
            // «to: Объект.Параметр» — входной якорь (F-5 «to:<param>»)
            PortTarget::Param(param) => self.trf(
                keys::TOOLTIP_PORT_PARAM,
                &[("{path}", &format!("{obj}.{param}"))],
            ),
            PortTarget::Out => self.trf(keys::STAGE_OUT_LABEL, &[("{name}", &obj)]),
        })
    }

    /// FR-045 F-5 v2: лейблы входных слотов стороны (PRD-0004 N3, R-5/R-3)
    /// — qualified-пути истоков value-рёбер, прикреплённых к этой стороне
    /// (порядок `canvas.edges` — детерминирован, как `inbound_slots`;
    /// перечисление — единая точка `dataref::input_refs`, FR-044 Р-4).
    /// Поле «строка N» — i18n-ключ (RU/EN, как в v1: display_ref несёт
    /// дословное RU — для тултипов локализуем поле поверх единой точки).
    /// Unmapped (Р-3: `scene.unmapped_edges`) — маркер «не подставлено».
    /// Больше 3 — первые 3 + свёртка «+N ещё» (полный список — в stage).
    /// None — входящих value-рёбер на стороне нет (fallback — «out:»).
    fn inbound_label_lines(&self, node_index: usize, side: Side) -> Option<Vec<PortLabelLine>> {
        let canvas = &self.scene.canvas;
        let node_id = canvas.nodes.get(node_index)?.id.clone();
        let refs = canvas_core::dataref::input_refs(canvas, &node_id);
        if refs.is_empty() {
            return None;
        }
        let mut lines: Vec<PortLabelLine> = Vec::new();
        for r in refs {
            let edge = canvas.edges.get(r.edge_index)?;
            // Сторона ребра — эффективная (CR-008: пин/авто, как в рендере);
            // висячий исток пропускается (лейбл стороны — по живым рёбрам).
            let (Some(from), Some(to)) = (canvas.node(&edge.from_node), canvas.node(&node_id))
            else {
                continue;
            };
            let (_, eff_to) = canvas_core::effective_sides(edge, from, to);
            if eff_to != side {
                continue;
            }
            // Поле: from_line → i18n «строка N»/«line N» (1-based);
            // from_output/fallback edge.id — поле из единой точки dataref.
            let field = match edge.from_line {
                Some(line) => {
                    let n = (line + 1).to_string();
                    self.trf(keys::STAGE_LINE_LABEL, &[("{n}", &n)])
                }
                None => r.r.field.clone(),
            };
            let path = format!("{}.{}", r.r.obj, field);
            // Р-3: исток без значения — маркер «не подставлено» (и янтарный
            // тон строки на вызове); подстановка значения снимает состояние.
            let unmapped = self.scene.unmapped_edges.iter().any(|id| id == &edge.id);
            let text = if unmapped {
                let marker = self.tr(keys::TOOLTIP_PORT_UNMAPPED);
                format!("from: {path} · {marker}")
            } else {
                format!("from: {path}")
            };
            lines.push(PortLabelLine { text, unmapped });
        }
        if lines.is_empty() {
            return None;
        }
        // Свёртка: 3 строки + «+N ещё» — тултип у курсора не разрастается
        // (полный список входов — панель stage, FR-044 Р-4)
        if lines.len() > 3 {
            let rest = lines.len() - 3;
            let n = rest.to_string();
            lines.truncate(3);
            lines.push(PortLabelLine {
                text: self.trf(keys::TOOLTIP_PORT_MORE, &[("{n}", &n)]),
                unmapped: false,
            });
        }
        Some(lines)
    }

    /// FR-045 F-5 v1/v2: лейблы порта канваса под курсором — приоритет как
    /// у drag-старта (CR-003/FR-050): построчный порт → якорь параметра →
    /// сторонный порт. Сторонный порт читается по стороне (P1 «входы
    /// слева»): есть входящие value-рёбра — qualified-истоки (v2,
    /// In-чтение), нет — «out:» (drag-исток, v1). None — порта нет.
    fn port_tooltip_at(&self, world: Vec2) -> Option<Vec<PortLabelLine>> {
        if let Some((node_index, port)) = self.line_port_hit(world) {
            return self
                .port_label_for(node_index, &PortTarget::Line(port.line))
                .map(|text| {
                    vec![PortLabelLine {
                        text,
                        unmapped: false,
                    }]
                });
        }
        if let Some((node_index, anchor)) = self.param_port_hit(world) {
            return self
                .port_label_for(node_index, &PortTarget::Param(anchor.param.clone()))
                .map(|text| {
                    vec![PortLabelLine {
                        text,
                        unmapped: false,
                    }]
                });
        }
        let node_index = self.hovered?;
        let node = self.scene.canvas.nodes.get(node_index)?;
        if node.kind() == NodeKind::Group {
            return None;
        }
        if let Some(side) =
            canvas_core::port_at(node, world, self.camera.zoom(), self.settings.port_zone_px)
        {
            if let Some(lines) = self.inbound_label_lines(node_index, side) {
                return Some(lines);
            }
            return self
                .port_label_for(node_index, &PortTarget::Out)
                .map(|text| {
                    vec![PortLabelLine {
                        text,
                        unmapped: false,
                    }]
                });
        }
        None
    }

    /// FR-050 Н2 (этап C): значение, которое несёт активный value-drag —
    /// для подсветки совместимости параметров (Н5/E-UNIT). Построчный
    /// исток — значение строки; нода целиком — узловое значение (data-нода
    /// значения в потоке не имеет — None, совместимость нейтральна).
    fn drag_source_value(&self) -> Option<expr::Value> {
        let EdgeDrag::New {
            from_node,
            from_port,
            ..
        } = self.edge_drag.as_ref()?
        else {
            return None;
        };
        // FR-064 P1: double buffer — снимок активных решений (рендер
        // читает через read()-гард вместо owned-поля).
        let solutions = canvas_scene::read_flow(&self.scene.flow_active);
        if let Some(port) = from_port {
            // Построчный исток — значение строки; футер шаблона
            // (line = None) — узловое значение (ниже, как у ноды целиком)
            if let Some(line) = port.line {
                return solutions.lines.get(&(from_node.clone(), line)).cloned();
            }
        }
        solutions
            .outputs
            .get(from_node)
            .cloned()
            .and_then(|outcome| outcome.ok())
    }

    /// FR-050 Н2 (этап C): цель value-drag под курсором — шаблонная нода с
    /// совместимостью параметров по единицам (Н5). Пересчёт на кадр ввода
    /// (паттерн `bundle_hover`): None — drag не активен / цель не шаблонная.
    fn compute_param_drop(&self) -> Option<(usize, Vec<(String, bool)>)> {
        let value_drag = matches!(
            self.edge_drag,
            Some(EdgeDrag::New {
                value_flow: true,
                ..
            })
        );
        if !value_drag {
            return None;
        }
        let world = self.cursor_world();
        let index = self.selective_hit(world)?;
        let node = self.scene.canvas.nodes.get(index)?;
        let template = node.template()?;
        // Цель = исток drag — подсветки нет (self-edge не создаётся)
        if drag_from_node(self.edge_drag.as_ref()).is_some_and(|from| from == node.id) {
            return None;
        }
        let source_value = self.drag_source_value();
        let params: Vec<(String, bool)> = template
            .params
            .iter()
            .map(|(name, spec)| {
                // Значение неизвестно (data-нода/без результата) —
                // нейтральная совместимость (подсветка не отсекает)
                let compatible = source_value
                    .as_ref()
                    .map(|value| {
                        canvas_core::flow::value_param_compatible(value, spec.unit.as_deref())
                    })
                    .unwrap_or(true);
                (name.clone(), compatible)
            })
            .collect();
        Some((index, params))
    }

    /// FR-050 Н2 (этап C): формульные строки текстовой ноды-источника для
    /// меню выбора строки (W-AMBIGUOUS-SRC, FR-032): (индекс строки,
    /// подпись «имя = значение»). Пусто — источник не текстовая нода или
    /// строк без результата (ambiguity нет).
    fn source_formula_lines(&self, from_node: &str) -> Vec<(usize, String)> {
        // FR-064 P1: double buffer — снимок активных решений через read()-гард.
        let solutions = canvas_scene::read_flow(&self.scene.flow_active);
        let Some(node) = self.scene.canvas.node(from_node) else {
            return Vec::new();
        };
        if node.template().is_some() {
            // Шаблонная нода: drag от ноды целиком несёт узловое значение
            // (формулу шаблона) — выбора строки нет
            return Vec::new();
        }
        let text = node.text.as_deref().unwrap_or("");
        let mut result = Vec::new();
        for ((node_id, line), value) in &solutions.lines {
            if node_id != from_node {
                continue;
            }
            let raw = text.lines().nth(*line).unwrap_or("");
            let label = match expr::line_kind(raw) {
                expr::NumiLineKind::Assignment { name } => format!("{name} = {value}"),
                _ => format!("строка {} = {value}", line + 1),
            };
            result.push((*line, label));
        }
        result.sort_by_key(|(line, _)| *line);
        result
    }

    /// FR-050 Н2 (этап C): drop value-drag на параметр приёмника — создать
    /// value-ребро с `toParam` (занятый параметр — диалог «Заменить
    /// источник?» Н4; цикл — диалог FR-014 с control-фолбэком: toParam
    /// семантики не переносит, как from_line у FR-025). Неоднозначный
    /// исток (drag от текстовой ноды целиком, >1 формульных строк) —
    /// меню выбора строки (W-AMBIGUOUS-SRC).
    fn drop_to_param(
        &mut self,
        from_node: String,
        from_side: Side,
        from_port: Option<&canvas_core::LinePort>,
        to_node: String,
        param: String,
    ) {
        let from_line = from_port.and_then(|port| port.line);
        // W-AMBIGUOUS-SRC (FR-032): исток не адресован, строк с значением
        // больше одной — сначала выбор строки-источника
        if from_port.is_none() && self.source_formula_lines(&from_node).len() > 1 {
            let items = self
                .source_formula_lines(&from_node)
                .into_iter()
                .map(|(line, label)| ChoiceItem {
                    label,
                    action: ChoiceAction::SourceLine {
                        from_node: from_node.clone(),
                        from_side,
                        line,
                        to_node: to_node.clone(),
                        param: Some(param.clone()),
                    },
                })
                .collect();
            self.open_choice_menu(keys::MENU_PICK_LINE_TITLE, items);
            return;
        }
        self.connect_to_param(from_node, from_side, from_line, to_node, param);
    }

    /// FR-050 Н2/Н4 (этап C): создать value-ребро с `toParam` — с
    /// проверками занятости (диалог замены) и цикла (диалог FR-014).
    fn connect_to_param(
        &mut self,
        from_node: String,
        from_side: Side,
        from_line: Option<usize>,
        to_node: String,
        param: String,
    ) {
        // Н4: параметр уже запитан — диалог «Заменить источник?»
        let old = canvas_core::flow::occupying_param_edges(&self.scene.canvas, &to_node, &param);
        if !old.is_empty() {
            // Победитель — последнее ребро; подпись источника для тела
            // диалога (имя шаблона / первая строка / label / id)
            let winner = old
                .last()
                .and_then(|edge| self.scene.canvas.node(&edge.from_node))
                .map(node_display_label)
                .unwrap_or_else(|| param.clone());
            let old_edges = old.iter().map(|edge| edge.id.clone()).collect();
            self.dialog = Some(AppDialog::ReplaceSource {
                from_node,
                from_side,
                from_line,
                to_node,
                to_side: Side::Left,
                param,
                old_edges,
                old_source: winner,
            });
            self.request_redraw();
            return;
        }
        // FR-014: цикл value-потока — диалог с control-фолбэком
        if canvas_core::creates_value_cycle(&self.scene.canvas, &from_node, &to_node) {
            self.dialog = Some(AppDialog::EdgeCycle {
                from_node,
                from_side,
                to_node,
                to_side: Side::Left,
            });
            self.request_redraw();
            return;
        }
        self.create_param_edge(from_node, from_side, from_line, to_node, Side::Left, param);
    }

    /// FR-050 Н2 (этап C): меню выбора параметра приёмника (drop
    /// value-ребра на шаблонную ноду мимо якоря). Пункты — параметры
    /// снапшота шаблона; выбор → `connect_to_param` (занятость/цикл —
    /// как у drop на якорь).
    fn open_param_choice_menu(
        &mut self,
        from_node: String,
        from_side: Side,
        from_port: Option<&canvas_core::LinePort>,
        to_node: String,
    ) {
        let Some(template) = self
            .scene
            .canvas
            .node(&to_node)
            .and_then(|node| node.template())
        else {
            return;
        };
        let from_line = from_port.and_then(|port| port.line);
        let items = template
            .params
            .keys()
            .map(|param| ChoiceItem {
                label: param.clone(),
                action: ChoiceAction::Param {
                    from_node: from_node.clone(),
                    from_side,
                    from_line,
                    to_node: to_node.clone(),
                    param: param.clone(),
                },
            })
            .collect();
        self.open_choice_menu(keys::MENU_PICK_PARAM_TITLE, items);
    }

    /// FR-050 Н2 (этап C): применить пункт меню выбора — создать ребро
    /// (параметр — с toParam; строка-источник — from_line + уже выбранная
    /// цель; проверки занятости/цикла — в connect_to_param). Этап E:
    /// действия контекст-меню параметра (Н9-3) идут тем же путём.
    fn apply_choice_action(&mut self, action: ChoiceAction) {
        match action {
            ChoiceAction::Param {
                from_node,
                from_side,
                from_line,
                to_node,
                param,
            } => {
                self.connect_to_param(from_node, from_side, from_line, to_node, param);
            }
            ChoiceAction::SourceLine {
                from_node,
                from_side,
                line,
                to_node,
                param,
            } => match param {
                Some(param) => {
                    self.connect_to_param(from_node, from_side, Some(line), to_node, param);
                }
                None => {
                    // Строка выбрана, цель — нода целиком (позиционное
                    // ребро, как до FR-050)
                    self.create_edge(
                        from_node,
                        from_side,
                        to_node,
                        Side::Left,
                        FlowKind::Value,
                        Some(line),
                    );
                }
            },
            // Н9-3: полёт + подсветка истока (машинария фокуса FR-048)
            ChoiceAction::ShowSource { edge_id } => self.show_spill_source(&edge_id),
            // Н9-3/Р-5: удалить ребро — один undo-шаг
            ChoiceAction::DisconnectSpill { edge_id } => self.disconnect_spill(&edge_id),
            // Н9-3: what-if режим (уже включён — просто панель на виду)
            ChoiceAction::WhatIf => self.enter_whatif_mode(),
        }
    }

    /// FR-050 Н2 (этап C): создать value-ребро с `toParam` (общий путь
    /// drop на якорь / меню выбора / замены источника). Undo-шаг (FR-006),
    /// живой пересчёт потока — значение сразу перекрывает локальное (Р-1).
    /// Этап E (Н9-6): тост «подтянулся из …» — паттерн CR-016.
    fn create_param_edge(
        &mut self,
        from_node: String,
        from_side: Side,
        from_line: Option<usize>,
        to_node: String,
        to_side: Side,
        param: String,
    ) {
        let mut edge = Edge::new(
            self.scene.canvas.next_edge_id(),
            from_node,
            Some(from_side),
            to_node.clone(),
            Some(to_side),
        );
        edge.set_flow_kind(FlowKind::Value);
        edge.from_line = from_line;
        edge.to_param = Some(param.clone());
        self.push_undo();
        self.scene.canvas.add_edge(edge);
        self.scene.mark_dirty();
        self.scene.recompute_flow();
        self.spill_connected_toast(&to_node, &param);
        self.request_redraw();
    }

    /// FR-050 Н9-6 (этап E): тост подключения проливания — по представлению
    /// сцены после пересчёта: строка присваивания была — «Параметр {param}
    /// подтянулся из {path}»; параметра не было (строка-проекция Р-4) —
    /// «Значение подтянулось из {path}»; оба с «— Ctrl+Z отменит» (один
    /// undo-шаг только что созданного ребра). Проливание не собралось
    /// (unmapped/источник без значения) — тоста нет (диагностика Р-3 на
    /// месте: пунктир + тултип).
    fn spill_connected_toast(&mut self, to_node: &str, param: &str) {
        let Some(view) = self
            .scene
            .param_spills
            .get(to_node)
            .and_then(|views| views.iter().find(|view| view.param == param))
        else {
            return;
        };
        let key = spill_toast_key(view);
        let subs: &[(&str, &str)] = match view.line.is_some() {
            true => &[("{param}", param), ("{path}", view.path.as_str())],
            false => &[("{path}", view.path.as_str())],
        };
        self.show_toast(self.trf(key, subs));
    }

    /// Хэндл конца выделенной связи под world-точкой (CR-002): конец, чей
    /// порт ближе к курсору в допуске зоны портов (CR-003, экранные px →
    /// world делением на zoom). None — мимо обоих концов/связь висячая.
    fn edge_handle_at(&self, edge_index: usize, world: Vec2) -> Option<canvas_core::EdgeEnd> {
        let tolerance = self.settings.port_zone_px / self.camera.zoom().max(1e-3);
        let dist = |p: &[f32; 2]| ((p[0] - world[0]).powi(2) + (p[1] - world[1]).powi(2)).sqrt();
        [canvas_core::EdgeEnd::From, canvas_core::EdgeEnd::To]
            .into_iter()
            .filter_map(|end| {
                canvas_core::edge_endpoint(&self.scene.canvas, edge_index, end)
                    .map(|(_, point)| (end, dist(&point)))
            })
            .filter(|(_, distance)| *distance <= tolerance)
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(end, _)| end)
    }

    /// Центр видимого мира (мировые координаты) — для «Создать группу».
    fn viewport_center_world(&self) -> Vec2 {
        let rect = self.camera.visible_world_rect(self.viewport_logical());
        [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0]
    }

    /// Вставить готовую ноду-группу в модель (паттерн create_note_at):
    /// spatial index + выделение новой группы. Возвращает индекс.
    fn insert_group(&mut self, group: Node) -> usize {
        // FR-006: создание группы — undo-шаг
        self.push_undo();
        self.scene.canvas.nodes.push(group);
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        self.selected = Some(Selection::Node(index));
        self.selected_nodes.clear();
        self.scene.mark_dirty();
        index
    }

    /// Активно ли панорамирование (средняя кнопка или Space+drag, SPEC §8).
    fn panning(&self) -> bool {
        self.middle_pressed || (self.space_pressed && self.left_pressed)
    }

    /// Аффорданс курсора (практики UI: форма курсора подсказывает жест):
    /// пан — Grabbing, текстовый редактор — Text, resize-угол ноды —
    /// NwseResize, иначе Arrow. set_cursor вызывается только при смене.
    fn sync_cursor_icon(&mut self) {
        let desired = if self.panning() {
            CursorIcon::Grabbing
        } else if self.editing.is_some() {
            CursorIcon::Text
        } else {
            let world = self.cursor_world();
            let resize = self
                .hovered
                .and_then(|i| self.scene.canvas.nodes.get(i))
                .is_some_and(|node| crate::ui::in_resize_corner(node, world));
            // FR-061 хвосты (D-7/D-8): курсор Pointer над кликабельными
            // зонами тела — заголовок блока-ведомости (строка+chevron,
            // решение владельца) и экспандер описания (прототип .blk-hdr).
            let body_pointer = !resize && self.body_hit_at(self.cursor).is_some();
            if resize {
                CursorIcon::NwseResize
            } else if body_pointer {
                CursorIcon::Pointer
            } else {
                CursorIcon::Default
            }
        };
        if self.cursor_icon != desired {
            self.cursor_icon = desired;
            if let Some(window) = &self.window {
                window.set_cursor(desired);
            }
        }
    }

    /// Сброс залипших pointer-transient состояний при потере фокуса окна
    /// (alt-tab во время drag оставлял «прилипшую» ноду/пан — кнопка
    /// Released приходит в другое окно). Практики UI: модальные переходы
    /// гасят активные жесты.
    fn cancel_pointer_transients(&mut self) {
        self.space_pressed = false;
        self.middle_pressed = false;
        self.left_pressed = false;
        self.minimap_drag = false;
        self.editor_dragging = false;
        self.resizing = None;
        self.edge_drag = None;
        self.select_rect = None;
        self.group_drop_target = None;
        if self.dragging.is_some() {
            // FR-006: движение до потери фокуса — undo-шаг
            self.finish_interaction_undo();
            // FR-038 (п.9): drag прерван потерей фокуса — предпросмотр
            // направляющих не должен переживать жест
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.clear_guides();
            }
            self.dragging = None;
        }
        self.sync_cursor_icon();
    }

    /// Курсор над открытой screen-space поверхностью (кнопки/панель
    /// настроек, поиск, меню+подменю, палитра, хоткеи, миникарта, диалог).
    /// Практика canvas-приложений (Miro/Figma): колесо/пинч над плавающим
    /// UI холст не двигают.
    fn cursor_over_screen_surface(&self) -> bool {
        // FR-052 (U2 PRD-0009): колесо/пинч глушатся над экранной
        // поверхностью — решение из реестра (HitStack::absorbs по кадру),
        // а не из ручного списка rect'ов. Панели Capture — в своих rect'ах,
        // Block-модали — везде (инвариант 8: при stage колесо глушится
        // ранним return в on_mouse_wheel/on_pinch).
        let frame = ui_registry::build_frame(self);
        HitStack::absorbs(&frame, UiPoint::new(self.cursor[0], self.cursor[1]))
    }

    /// Размер viewport в логических пикселях. Делитель — effective scale
    /// (R10: в desktop-режиме window.scale_factor() после репарентинга
    /// недостоверен — кнопки улетали за видимую область).
    fn viewport_logical(&self) -> Vec2 {
        // Тестовый оверрайд: клики по screen-space UI без окна
        #[cfg(test)]
        if let Some(viewport) = self.test_viewport {
            return viewport;
        }
        match &self.window {
            Some(window) => {
                let size = window.inner_size();
                let scale = self.scale_factor();
                [size.width as f32 / scale, size.height as f32 / scale]
            }
            None => [0.0, 0.0],
        }
    }

    /// Позиция курсора в world-координатах.
    fn cursor_world(&self) -> Vec2 {
        self.camera
            .screen_to_world(self.cursor, self.viewport_logical())
    }

    /// FR-013 (правка 4): зона наведения бейджа ошибки формульной строки
    /// под курсором (логические px окна), None — мимо всех бейджей. Зоны —
    /// с прошлого кадра (`expr_error_hits`); отставание в кадр незаметно.
    fn expr_error_hit_at(&self, cursor: [f32; 2]) -> Option<&LineErrorHit> {
        expr_error_hit_at(&self.expr_error_hits, cursor)
    }

    /// FR-061 коммит 3: зона наведения усечённой формулы под курсором
    /// (лестница §3.4, Q8), None — мимо. Зоны — с прошлого кадра
    /// (`ellipsis_hits`); отставание в кадр незаметно.
    fn formula_ellipsis_hit_at(&self, cursor: [f32; 2]) -> Option<&LineErrorHit> {
        expr_error_hit_at(&self.ellipsis_hits, cursor)
    }

    /// FR-050 Н9-2 (этап D): зона наведения пролитой строки под курсором
    /// (параметр с toParam / авто-строка приёмника), None — мимо. Зоны — с
    /// прошлого кадра (`spill_hits`); отставание в кадр незаметно.
    fn spill_hit_at(&self, cursor: [f32; 2]) -> Option<&SpillHit> {
        spill_hit_at(&self.spill_hits, cursor)
    }

    /// FR-061 хвосты (D-7/D-8): кликабельная зона тела под курсором
    /// (заголовок блока-ведомости / экспандер описания), None — мимо.
    /// Зоны — с прошлого кадра (`body_hits`); отставание в кадр незаметно.
    fn body_hit_at(&self, cursor: [f32; 2]) -> Option<&BodyHit> {
        self.body_hits.iter().find(|hit| {
            let [x, y, w, h] = hit.rect;
            cursor[0] >= x && cursor[0] <= x + w && cursor[1] >= y && cursor[1] <= y + h
        })
    }

    /// FR-061 хвосты (D-7/D-8 runtime v1): обработать клик по телу ноды —
    /// тоггл свёрнутости блока (заголовок, вся строка — решение владельца)
    /// и раскрытости описания (экспандер; «Раскрыть+авто»). true — клик
    /// поглощён тогглом (не доходит до выделения/драга).
    fn handle_body_hit_click(&mut self) -> bool {
        let Some(hit) = self.body_hit_at(self.cursor) else {
            return false;
        };
        let (kind, node) = (hit.kind, hit.node);
        let Some(node_id) = self.scene.canvas.nodes.get(node).map(|n| n.id.clone()) else {
            return false;
        };
        match kind {
            BodyHitKind::BlockHeader => {
                self.scene.toggle_block_collapsed(&node_id);
                // FR-069 (этап F): тоггл запускает refit — разворот блока
                // растит контент, высота подгоняется сразу (growth-only:
                // свёртывание не усаживает — I-6).
                self.scene.ensure_reserve_at(node);
            }
            BodyHitKind::DescExpander => {
                self.scene.toggle_desc_expanded(&node_id);
                // FR-069: раскрытое описание длиннее клампа растит ноду —
                // мера уровня 2 знает состояние (desc_expanded).
                self.scene.ensure_reserve_at(node);
            }
        }
        true
    }

    /// FR-050 Н9-3 (этап E): открыть контекст-меню проливания по hit-зоне
    /// пролитой строки (параметр с toParam / авто-строка приёмника) —
    /// пункты «Показать источник» / «Отключить проливание» / «Что если…».
    /// Паттерн ChoiceMenu (screen-space, Popups Block, Esc/клик мимо —
    /// отмена). Заголовок — «Проливание в параметр»/«Входящее значение».
    fn open_param_menu(&mut self, hit: &SpillHit) -> bool {
        let Some(target) = spill_hit_target(&self.scene.canvas, hit) else {
            return false;
        };
        let items = vec![
            ChoiceItem {
                label: self.tr(keys::MENU_PARAM_SOURCE).to_owned(),
                action: ChoiceAction::ShowSource {
                    edge_id: target.edge_id.clone(),
                },
            },
            ChoiceItem {
                label: self.tr(keys::MENU_PARAM_DISCONNECT).to_owned(),
                action: ChoiceAction::DisconnectSpill {
                    edge_id: target.edge_id.clone(),
                },
            },
            ChoiceItem {
                label: self.tr(keys::MENU_PARAM_WHATIF).to_owned(),
                action: ChoiceAction::WhatIf,
            },
        ];
        self.open_choice_menu(target.title_key, items);
        true
    }

    /// FR-050 Н9-3 (этап E): «Показать источник» — полёт камеры к истоку
    /// (300 мс ease-out, зум не ниже читаемого — паттерн поиска T14) +
    /// пульс ноды-истока + подсветка истока/связи/приёмника с затемнением
    /// остального до SHOW_SOURCE_MS (машинерия фокуса PRD-0007/FR-048 —
    /// та же, что у карты потока Н9-4; update_focus_state уважает окно).
    fn show_spill_source(&mut self, edge_id: &str) {
        let Some(index) = self
            .scene
            .canvas
            .edges
            .iter()
            .position(|edge| edge.id == edge_id)
        else {
            return;
        };
        let edge = &self.scene.canvas.edges[index];
        let Some(from_index) = self
            .scene
            .canvas
            .nodes
            .iter()
            .position(|node| node.id == edge.from_node)
        else {
            return;
        };
        let to_index = self
            .scene
            .canvas
            .nodes
            .iter()
            .position(|node| node.id == edge.to_node);
        let from = &self.scene.canvas.nodes[from_index];
        let center = [from.x + from.width / 2.0, from.y + from.height / 2.0];
        let flight = Flight::new(
            self.camera.position(),
            self.camera.zoom(),
            center,
            self.camera.zoom().max(0.8),
            FLIGHT_DURATION_MS,
        );
        self.flight = Some((flight, Instant::now()));
        self.pulse = Some((from_index, Instant::now()));
        // Подсветка: исток + приёмник + ребро; «дыхание» — один цикл
        let mut nodes = vec![from_index];
        if let Some(to_index) = to_index {
            nodes.push(to_index);
        }
        nodes.sort_unstable();
        nodes.dedup();
        self.show_source = Some((nodes, vec![index], Instant::now()));
        self.focus_pulse = Some((FocusSeed::Edge(index), Instant::now()));
        self.request_redraw();
    }

    /// FR-050 Н9-3/Р-5 (этап E): «Отключить проливание» — удалить ребро
    /// (основной путь ручной правки пролитого значения): один undo-шаг,
    /// Ctrl+Z возвращает связь — значение откатывается к локальному.
    fn disconnect_spill(&mut self, edge_id: &str) {
        if !self
            .scene
            .canvas
            .edges
            .iter()
            .any(|edge| edge.id == edge_id)
        {
            return;
        }
        self.push_undo();
        self.scene.canvas.remove_edge(edge_id);
        self.scene.mark_dirty();
        self.scene.recompute_flow();
        self.request_redraw();
    }

    /// Клиентские ФИЗИЧЕСКИЕ px от shell (DragEvent) -> world-координаты:
    /// делим на scale_factor (масштаб учтён), затем через камеру (T9).
    fn drag_world_pt(&self, pt: (f32, f32)) -> Vec2 {
        let scale = self.scale_factor();
        let logical = [pt.0 / scale, pt.1 / scale];
        self.camera
            .screen_to_world(logical, self.viewport_logical())
    }

    /// Синхронизировать вотчер с моделью (T10): родительские директории всех
    /// файловых нод → WatchService::sync_dirs (diff, повторный вызов — no-op),
    /// а на Windows — зеркало того же набора в SHChangeNotify-подписки шины
    /// T16 (R12). Вызывается после загрузки, дропа (T9), удаления нод и
    /// rename-событий.
    pub fn sync_watch_dirs(&mut self) {
        let dirs = watched_dirs(&self.scene.canvas, &self.scene.canvas_dir());
        self.watcher.sync_dirs(&dirs);
        // T16 (R12): зеркало того же набора в SHChangeNotify-подписки шины —
        // ЕДИНАЯ точка зеркалирования (план §3, все вызовы остаются как
        // есть); деградация шины — команды уходят впустую, молча (R14)
        #[cfg(windows)]
        if let Some(service) = &self.shell_events {
            service.command(canvas_shell::shell_events::window::ShellCommand::SyncFileDirs(dirs));
        }
    }

    /// Ответ поискового индекса (T14): результаты FTS + заметки → строки.
    fn on_search_event(&mut self, event: SearchEvent) {
        match event {
            SearchEvent::Ready(hits) => self.apply_search_hits(hits),
            SearchEvent::Indexed(count) => tracing::debug!(count, "поисковый индекс обновлён"),
        }
    }

    /// Склейка результатов (T14): FTS-хиты (bm25, путь → нода через
    /// path_matches) + in-memory substring по заметкам и именам нод
    /// (заметок без файла в индексе нет). Дедуп — по ноде.
    /// FR-011: ноды свернутых поддеревьев из результатов исключены.
    fn apply_search_hits(&mut self, hits: Vec<SearchHit>) {
        let canvas_dir = self.scene.canvas_dir();
        let hidden = self.hidden_subtree_nodes();
        let mut nodes: Vec<usize> = Vec::new();
        let mut rows: Vec<SearchRow> = Vec::new();
        for hit in &hits {
            let index = self.scene.canvas.nodes.iter().position(|node| {
                node.file
                    .as_ref()
                    .is_some_and(|file| path_matches(file, &canvas_dir, &hit.path))
            });
            let Some(index) = index else {
                continue; // файл не на канвасе — строка не показывается
            };
            if hidden.binary_search(&index).is_ok() {
                continue; // FR-011: свернутая ветка не ищется
            }
            if nodes.contains(&index) {
                continue;
            }
            nodes.push(index);
            rows.push(SearchRow {
                title: hit.display_name.clone(),
                subtitle: hit_subtitle(&hit.path),
            });
        }
        // In-memory: заметки и имена нод вне FTS-индекса (T14 §3)
        let query = self.search.input.query().to_owned();
        if !query.is_empty() {
            let entries: Vec<SceneEntry<'_>> = self
                .scene
                .canvas
                .nodes
                .iter()
                .enumerate()
                .filter(|(index, _)| !nodes.contains(index))
                .map(|(index, node)| SceneEntry {
                    node: index,
                    title: node_title(node),
                    text: node_text(node),
                })
                .collect();
            for hit in scan_scene(&query, &entries) {
                // FR-011: свернутые ветки в поиске не участвуют
                if hidden.binary_search(&hit).is_ok() {
                    continue;
                }
                let Some(node) = self.scene.canvas.nodes.get(hit) else {
                    continue;
                };
                nodes.push(hit);
                rows.push(SearchRow {
                    title: node_title(node).to_owned(),
                    subtitle: node_subtitle(node).to_owned(),
                });
            }
        }
        // W7 (web-приёмка): итог склейки FTS+scan_scene в лог (DEBUG — на
        // нативе под дефолтным фильтром не виден; на web виден с ?log=debug
        // и служит оракулом браузерного дыма: поиск по 5000-нод сцене)
        tracing::debug!(
            rows = rows.len(),
            "поиск завершён: склейка FTS и scan_scene"
        );
        self.search.set_results(rows);
        self.search_nodes = nodes;
        self.request_redraw();
    }

    /// Правка поля запроса (T14): любое изменение перезапускает debounce.
    fn edit_search_input(&mut self, apply: impl FnOnce(&mut SearchInput) -> bool) {
        let changed = apply(&mut self.search.input);
        if changed {
            self.search_pending = Some((self.search.input.query().to_owned(), Instant::now()));
            self.request_redraw();
        }
    }

    /// Ввод через IME (WindowEvent::Ime, фикс 2026-09-25): текст коммита
    /// уходит в АКТИВНЫЙ текстовый приёмник — приоритет как у клавиатурного
    /// владельца реестра: редактор заметки → панель поиска → поле подмены
    /// окна проверки. Вне текстовых приёмников коммит игнорируется
    /// (IME-статусы/предильт — не текст, панорамирование не трогаем).
    pub(crate) fn on_ime(&mut self, ime: winit::event::Ime) {
        let winit::event::Ime::Commit(text) = ime else {
            return; // Enabled/Disabled/Preedit — v1 не обрабатывает
        };
        self.insert_committed_text(&text);
    }

    /// Общий маршрут текста коммита (Ime::Commit + web-мост AppEvent::ImeCommit):
    /// 1) редактор заметки (EditingSession) — та же вставка, что Paste;
    /// 2) панель поиска; 3) inline-поле подмены окна проверки (FR-048 X3).
    pub fn insert_committed_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // 1) Редактор заметки (EditingSession)
        if let (Some(session), Some(renderer)) = (self.editing.as_mut(), self.renderer.as_mut()) {
            session.insert_text(renderer.font_system_mut(), text);
            self.fit_note_size();
            self.update_hints();
            self.request_redraw();
            return;
        }
        // 2) Панель поиска (открыта — она владеет клавиатурой)
        if self.search.is_open() {
            let text = text.to_owned();
            self.edit_search_input(move |input| {
                input.insert_str(&text);
                true
            });
            return;
        }
        // 3) Inline-поле подмены окна проверки цепочки (FR-048 X3)
        if let Some(state) = self.explain.as_mut() {
            if let Some(edit) = state.edit.as_mut() {
                edit.type_str(text);
                self.request_redraw();
            }
        }
    }

    /// Клавиатура открытой панели поиска (T14): ввод, каретка, выбор, прыжок.
    fn on_search_key(&mut self, event: &KeyEvent) {
        let ctrl = self.modifiers.control_key();
        let shift = self.modifiers.shift_key();
        // Повторное Ctrl+F — очистить поле (первое — открытие с прошлым
        // запросом, ввод замещает его только после очистки)
        if ctrl
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("f") || c.eq_ignore_ascii_case("а"))
        {
            self.search.input.set_query("");
            self.search_pending = Some((String::new(), Instant::now()));
            self.request_redraw();
            return;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.search.close();
                self.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                if let Some(PanelAction::Jump(row)) = self.search.confirm() {
                    self.jump_to_search_row(row);
                }
            }
            Key::Named(NamedKey::F3) => {
                self.cycle_search(if shift { -1 } else { 1 });
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.search.move_selection(-1);
                self.search.ensure_selection_visible();
                self.request_redraw();
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.search.move_selection(1);
                self.search.ensure_selection_visible();
                self.request_redraw();
            }
            Key::Named(NamedKey::Backspace) => {
                self.edit_search_input(|input| input.backspace(ctrl));
            }
            Key::Named(NamedKey::Delete) => {
                self.edit_search_input(SearchInput::delete);
            }
            Key::Named(NamedKey::ArrowLeft) if !ctrl => {
                self.search.input.move_left();
                self.request_redraw();
            }
            Key::Named(NamedKey::ArrowRight) if !ctrl => {
                self.search.input.move_right();
                self.request_redraw();
            }
            Key::Named(NamedKey::Home) => {
                self.search.input.move_to_start();
                self.request_redraw();
            }
            Key::Named(NamedKey::End) => {
                self.search.input.move_to_end();
                self.request_redraw();
            }
            Key::Character(text) => {
                self.edit_search_input(|input| {
                    input.insert_str(text);
                    true
                });
            }
            _ => {}
        }
    }

    /// F3/Shift+F3 (T14): цикл по результатам с прыжком; работает и после
    /// закрытия панели (rows сохранены).
    fn cycle_search(&mut self, delta: i32) {
        if self.search.rows.is_empty() {
            return;
        }
        self.search.move_selection(delta);
        self.search.ensure_selection_visible();
        if let Some(PanelAction::Jump(row)) = self.search.confirm() {
            self.jump_to_search_row(row);
        } else {
            self.request_redraw();
        }
    }

    /// Прыжок к строке результата (T14): полёт камеры 300 мс ease-out,
    /// целевой зум не ниже 0.8 (нода читаема), пульс подсветки.
    fn jump_to_search_row(&mut self, row: usize) {
        let Some(&node) = self.search_nodes.get(row) else {
            return;
        };
        let Some(target) = self.scene.canvas.nodes.get(node) else {
            return;
        };
        let center = [
            target.x + target.width / 2.0,
            target.y + target.height / 2.0,
        ];
        let target_zoom = self.camera.zoom().max(0.8);
        self.flight = Some((
            Flight::new(
                self.camera.position(),
                self.camera.zoom(),
                center,
                target_zoom,
                FLIGHT_DURATION_MS,
            ),
            Instant::now(),
        ));
        self.pulse = Some((node, Instant::now()));
        self.request_redraw();
    }

    /// FR-050 Н9-4 (этап E): тогл панели «Карта проливаний» — оверлей всех
    /// проливаний канваса; входы — пункт меню канваса и Ctrl+Shift+M,
    /// выход — повторный тогл/Esc/клик мимо панели/«✕».
    fn toggle_flow_map(&mut self) {
        self.flow_map_open = !self.flow_map_open;
        // FR-059: скролл списка — на каждое открытие с начала
        self.flow_map_scroll = canvas_ui::kit::ScrollState::default();
        self.request_redraw();
    }

    /// Н9-4: строки карты — чистый сбор из результатов пересчёта сцены
    /// (проливания + авто-строки; чистая функция — flowmap_ui).
    fn flow_map_rows(&self) -> Vec<flowmap_ui::FlowMapRow> {
        flowmap_ui::flow_map_rows(
            &self.scene.canvas,
            &self.scene.param_spills,
            &self.scene.auto_rows,
        )
    }

    /// Н9-4: раскладка панели по текущему вьюпорту и числу строк.
    /// FR-059: скролл списка — состояние App; раскладка синхронизирует
    /// КОПИЮ с контентом/вьюпортом (идемпотентно, resize-паттерн docs_ui)
    /// — детерминизм рендер/hit (одни rect'ы на кадр).
    fn flow_map_layout(&self) -> flowmap_ui::FlowMapLayout {
        let mut scroll = self.flow_map_scroll.clone();
        flowmap_ui::flow_map_layout(
            self.viewport_logical(),
            self.flow_map_rows().len(),
            &mut scroll,
        )
    }

    /// FR-059: скролл списка карты колесом (кит список+скролл).
    fn flow_map_scroll_by(&mut self, dy: f32) {
        let mut scroll = self.flow_map_scroll.clone();
        flowmap_ui::flow_map_layout(
            self.viewport_logical(),
            self.flow_map_rows().len(),
            &mut scroll,
        );
        scroll.scroll_by(dy);
        scroll.clamp();
        self.flow_map_scroll = scroll;
        self.request_redraw();
    }

    /// Н9-4: текст строки карты «путь → адрес · значение» — адрес параметра
    /// или позиционного входа («вход N» = слот + 1, тултип-нотация FR-025).
    fn flow_map_row_text(&self, row: &flowmap_ui::FlowMapRow) -> String {
        let target = match &row.param {
            Some(param) => param.clone(),
            None => self.trf(
                keys::FLOW_MAP_INPUT,
                &[("{n}", &(row.slot + 1).to_string())],
            ),
        };
        let value = row.value.clone().unwrap_or_else(|| "—".to_owned());
        format!("{} → {} · {}", row.path, target, value)
    }

    /// Н9-4: клик по панели карты — «✕» закрывает; строка — переход к
    /// истоку (полёт + подсветка Н9-3, панель остаётся — можно пройти все
    /// проливания подряд); мимо элементов — глотается (Block, панель жива).
    fn click_flow_map(&mut self) {
        let lay = self.flow_map_layout();
        if flowmap_ui::flow_map_close_at(&lay, self.cursor) {
            self.flow_map_open = false;
            self.request_redraw();
            return;
        }
        if let Some(index) = flowmap_ui::flow_map_row_at(&lay, self.cursor) {
            let rows = self.flow_map_rows();
            if let Some(row) = rows.get(index) {
                self.show_spill_source(&row.edge_id);
            }
        }
        self.request_redraw();
    }

    // --- FR-018: шаблоны ---

    /// Инстанцировать шаблон в world-точке (undo-шаг, выделение, пересчёт
    /// потока). Возвращает индекс новой ноды. Дефолты манифеста всегда в
    /// границах — Result разворачивается (ошибка границ возможна только для
    /// переопределений MCP).
    fn instantiate_template_at(
        &mut self,
        manifest: &canvas_core::templates::TemplateManifest,
        world: Vec2,
    ) -> usize {
        self.push_undo();
        let id = next_free_id(&self.scene.canvas, "tpl");
        let mut node = canvas_core::templates::instantiate_with_language(
            manifest,
            &BTreeMap::new(),
            id.clone(),
            world[0],
            world[1],
            self.settings.language,
        )
        .expect("дефолты манифеста в границах");
        // FR-023: авто-высота шаблонной ноды при инстанциации — по числу
        // строк листа параметров: шапка + тело + футер результата. Новая
        // нода сразу влезает целиком (без «подгонки правкой»).
        fit_template_node_height(&mut node);
        self.scene.canvas.nodes.push(node);
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        self.selected = Some(Selection::Node(index));
        self.selected_nodes.clear();
        self.scene.mark_dirty();
        self.scene.recompute_flow();
        // W9 (web-приёмка): оракул браузерного дыма — вставка шаблонной
        // группы дошла до модели (критерий приёмки W9, wasm-port §6)
        tracing::debug!(
            template = %manifest.id,
            node = %id,
            "шаблон вставлен"
        );
        index
    }

    // --- FR-049: галерея схем и empty-state ---

    /// FR-049 (US-5): отложенная схема `?template=<id>` для web-порта
    /// (поле private — сеттер для canvas-web; применяется на первом кадре).
    pub fn set_pending_scheme(&mut self, id: Option<String>) {
        self.pending_scheme = id;
    }

    /// FR-055 (этап U4, F-10): включить/выключить DebugOverlay извне
    /// (web-старт с `?ui=debug`; натив — тогл F9 в on_key).
    pub fn set_debug_overlay(&mut self, on: bool) {
        self.debug_overlay = on;
        self.request_redraw();
    }

    /// Empty-state пустого канваса виден: 0 нод и нет конкурирующих
    /// модальных поверхностей (US-1 AC-1.1).
    fn empty_state_visible(&self) -> bool {
        self.scene.canvas.nodes.is_empty()
            && !self.scheme_gallery.open
            && self.onboarding.is_none()
            && !self.settings_open
            // FR-054: развёрнутый док палитры накрывает левую треть
            // карточки (тот класс постоянных панелей, что settings/menu —
            // карточка прячетcя, клики под доком не теряются)
            && !self.template_panel.open
            // FR-054: панель хоткеев — transient-оверлей того же класса
            // (конкурирует с карточкой за лево-центр/центр)
            && !self.hotkeys_open
            && self.main_stage.is_none()
            && self.menu.is_none()
            && !self.empty_state_dismissed
    }

    /// Открыть схему из галереи: чистый инстансер → один undo-шаг →
    /// вставка → recompute_flow → zoom-to-fit (US-3, G3/G7).
    /// FR-040 v2: язык контента — `settings.language` (En → `content_en`,
    /// при отсутствии — фолбэк на `content` RU).
    fn apply_scheme(&mut self, manifest: &canvas_core::schemes::SchemeManifest) {
        let center = self.viewport_center_world();
        let instance = match canvas_scene::scheme_apply::instantiate_scheme_with_language(
            manifest,
            &self.scene.canvas,
            center,
            self.settings.language,
        ) {
            Ok(instance) => instance,
            Err(err) => {
                tracing::warn!(scheme = %manifest.id, %err, "инстанс схемы не удался");
                return;
            }
        };
        self.push_undo();
        for node in instance.nodes {
            self.scene.canvas.nodes.push(node);
            let index = self.scene.canvas.nodes.len() - 1;
            let node = &self.scene.canvas.nodes[index];
            self.scene.spatial.insert(index, node);
        }
        for edge in instance.edges {
            self.scene.canvas.add_edge(edge);
        }
        self.selected = None;
        self.selected_nodes.clear();
        self.scene.mark_dirty();
        self.scene.recompute_flow();
        // Zoom-to-fit содержимого схемы (F-3 PRD-0008): bbox → вьюпорт.
        let bbox = instance.bbox;
        let viewport = self.viewport_logical();
        let bw = (bbox[2] - bbox[0]).max(160.0);
        let bh = (bbox[3] - bbox[1]).max(120.0);
        let zoom = ((viewport[0] - 96.0) / bw)
            .min((viewport[1] - 160.0) / bh)
            .clamp(0.15, 1.5);
        self.camera.set_zoom(zoom);
        self.camera
            .set_center([(bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0]);
        self.empty_state_dismissed = false;
        self.scheme_gallery.close();
        let name = manifest.display_name(self.settings.language == canvas_core::Language::Ru);
        self.show_toast(i18n::trf(
            self.settings.language,
            keys::GALLERY_APPLIED,
            &[("name", name)],
        ));
        self.request_redraw();
    }

    /// Клавиатура галереи: true — нужен redraw (нажатия глотаются).
    fn on_gallery_key(&mut self, event: &KeyEvent) -> bool {
        // Ctrl+T — закрыть (той же клавишей, что открытие)
        if self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("t") || c == "е" || c == "Е")
        {
            self.scheme_gallery.close();
            return true;
        }
        let registry = canvas_core::schemes::SchemeRegistry::embedded();
        let list = scheme_gallery_ui::rows(registry, &self.scheme_gallery);
        let lay = scheme_gallery_ui::layout(self.viewport_logical(), &list, &self.scheme_gallery);
        let visible = lay.visible_rows.len().max(1);
        match &event.logical_key {
            Key::Named(NamedKey::Escape) if !event.repeat => {
                self.scheme_gallery.close();
                true
            }
            Key::Named(NamedKey::Enter) if !event.repeat => {
                if let Some(scheme) = list.get(self.scheme_gallery.selected) {
                    let manifest = (*scheme).clone();
                    self.apply_scheme(&manifest);
                }
                true
            }
            Key::Named(NamedKey::ArrowDown) if !event.repeat => {
                if self.scheme_gallery.selected + 1 < list.len() {
                    self.scheme_gallery.selected += 1;
                }
                scheme_gallery_ui::clamp_scroll(&mut self.scheme_gallery, visible);
                true
            }
            Key::Named(NamedKey::ArrowUp) if !event.repeat => {
                self.scheme_gallery.selected = self.scheme_gallery.selected.saturating_sub(1);
                scheme_gallery_ui::clamp_scroll(&mut self.scheme_gallery, visible);
                true
            }
            Key::Named(NamedKey::Backspace) if !event.repeat => {
                self.scheme_gallery.filter.pop();
                self.scheme_gallery.selected = 0;
                self.scheme_gallery.scroll_top = 0;
                true
            }
            Key::Character(text) if !event.repeat && !self.modifiers.control_key() => {
                self.scheme_gallery.filter.push_str(text.as_str());
                self.scheme_gallery.selected = 0;
                self.scheme_gallery.scroll_top = 0;
                true
            }
            // Прочие нажатия глотаются молча — канвасу не достаются
            _ => false,
        }
    }

    /// Клик по открытой галерее: элементы панели, мимо — закрыть.
    fn on_gallery_click(&mut self) {
        let registry = canvas_core::schemes::SchemeRegistry::embedded();
        let list = scheme_gallery_ui::rows(registry, &self.scheme_gallery);
        let lay = scheme_gallery_ui::layout(self.viewport_logical(), &list, &self.scheme_gallery);
        if scheme_gallery_ui::point_in_rect(lay.close_rect, self.cursor) {
            self.scheme_gallery.close();
            return;
        }
        // Поле фильтра — глотаем (клавиатура уже маршрутизируется галереей)
        if scheme_gallery_ui::point_in_rect(lay.input_rect, self.cursor) {
            return;
        }
        if let Some(category) = scheme_gallery_ui::chip_at(&lay, self.cursor) {
            self.scheme_gallery.category = if self.scheme_gallery.category == category {
                None
            } else {
                category
            };
            self.scheme_gallery.selected = 0;
            self.scheme_gallery.scroll_top = 0;
            return;
        }
        if let Some(index) = scheme_gallery_ui::row_at(&lay, self.cursor) {
            self.scheme_gallery.selected = index;
            if let Some(scheme) = list.get(index) {
                let manifest = (*scheme).clone();
                self.apply_scheme(&manifest);
            }
            return;
        }
        if scheme_gallery_ui::point_in_rect(lay.panel_rect, self.cursor) {
            return; // внутри панели, мимо элементов — глотаем
        }
        self.scheme_gallery.close();
    }

    /// FR-062 F-17: Tab/Shift+Tab — переход по фокус-секции витрины кита.
    /// Кольцо живёт в КОНТЕНТ-координатах раскладки (offset 0 — без сдвига
    /// и фильтра видимости: секция может быть за окном скролла); при
    /// перестроении контента (язык/вьюпорт) `retain_order` сохраняет
    /// позицию, вне диапазона — сброс. Геометрия — та же `gallery_layout`
    /// (одна раскладка для ввода и отрисовки); вызов — только на нажатие
    /// Tab (стоимость раскладки — микросекунды).
    fn gallery_focus_step(&mut self, backwards: bool) {
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return;
        }
        let lang = self.settings.language;
        let zero_scroll = canvas_ui::kit::ScrollState::default();
        let palette = self.effective_palette().kit_palette();
        let mut m = crate::kit_ui::new_measurer();
        let mut fs = canvas_render::text::measure_font_system();
        let lay =
            crate::kit_ui::gallery_layout(viewport, lang, &zero_scroll, &palette, &mut m, &mut fs);
        self.kit_gallery_focus.retain_order(&lay.focus_targets);
        if backwards {
            self.kit_gallery_focus.prev();
        } else {
            self.kit_gallery_focus.next();
        }
        self.request_redraw();
    }

    /// FR-021: принять выбранную подсказку — заменить токен слева от
    /// каретки текстом вставки. НЕ коммитит заметку; после вставки
    /// пересчитать высоту и список подсказок.
    fn accept_hint(&mut self) {
        let Some(item) = self.hints.selected_item().cloned() else {
            return;
        };
        let token = self.hints.token.clone();
        let applied = if let (Some(session), Some(renderer)) =
            (self.editing.as_mut(), self.renderer.as_mut())
        {
            session.replace_token_before_caret(renderer.font_system_mut(), &token, &item.insert);
            true
        } else {
            false
        };
        if applied {
            self.fit_note_size();
            self.update_hints();
            self.request_redraw();
        }
    }

    /// Батч событий файловой системы (T10): применение к модели — в чистой
    /// canvas_core::apply_file_events, здесь — платформенные реакции: сброс
    /// тамбнейл-кэшей и негативного кэша, автосейв, пересборка вотчеров.
    fn on_file_events(&mut self, events: Vec<FileEvent>) {
        if events.is_empty() {
            return;
        }
        let canvas_dir = self.scene.canvas_dir();
        let changes = apply_file_events(&mut self.scene.canvas, &canvas_dir, &events);
        if changes.is_empty() {
            return; // чужие файлы в наблюдаемых папках — частый случай
        }
        tracing::debug!(
            events = events.len(),
            changes = changes.len(),
            "события файловой системы применены"
        );
        let mut invalidate_thumbs = false;
        let mut dirty = false;
        let mut resync = false;
        for change in changes {
            match change {
                // Modify (и atomic-save): атлас и SQLite-кэш перезапросятся,
                // неудавшийся тамбнейл — перезапросить
                NodeChange::ThumbStale(index) => {
                    invalidate_thumbs = true;
                    self.thumbs_failed.remove(&index);
                }
                // Путь обновлён: автосейв + возможно новая директория вотчинга
                NodeChange::PathUpdated(_) => {
                    dirty = true;
                    resync = true;
                }
                NodeChange::Broken(_) => {}
                // Восстановление: неудавшийся тамбнейл можно перезапросить
                NodeChange::Restored(index) => {
                    invalidate_thumbs = true;
                    self.thumbs_failed.remove(&index);
                }
            }
        }
        if invalidate_thumbs {
            if let Some(renderer) = self.renderer.as_mut() {
                // Полный сброс: ключ атласа — индекс ноды, точечного удаления
                // нет; SQLite промахнётся по mtime сам (ключ — путь+mtime)
                renderer.invalidate_node_caches();
            }
        }
        if dirty {
            self.scene.mark_dirty();
        }
        if resync {
            self.sync_watch_dirs();
        }
        // Поисковый индекс (T14): события ФС — только по путям нод канваса
        // (чужие файлы в наблюдаемых папках в индекс не попадают)
        {
            let canvas_dir = self.scene.canvas_dir();
            let node_path_matches = |path: &Path| {
                self.scene.canvas.nodes.iter().any(|node| {
                    node.file
                        .as_ref()
                        .is_some_and(|file| path_matches(file, &canvas_dir, path))
                })
            };
            for event in &events {
                match event {
                    FileEvent::Create(path) | FileEvent::Modify(path) => {
                        if node_path_matches(path) {
                            self.search_service.command(SearchCommand::IndexFile {
                                path: path.clone(),
                                display_name: Path::new(path)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                            });
                        }
                    }
                    FileEvent::Rename(from, to) => {
                        if node_path_matches(from) || node_path_matches(to) {
                            self.search_service
                                .command(SearchCommand::RemoveFile { path: from.clone() });
                            self.search_service.command(SearchCommand::IndexFile {
                                path: to.clone(),
                                display_name: Path::new(to)
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_default(),
                            });
                        }
                    }
                    FileEvent::Remove(path) => {
                        if node_path_matches(path) {
                            self.search_service
                                .command(SearchCommand::RemoveFile { path: path.clone() });
                        }
                    }
                }
            }
        }
        self.request_redraw();
    }

    /// M8/W6 (wasm-port §4.2): смена активной сцены — «Открыть с диска»,
    /// reopen из недавних, DOM-drop `.canvas`-файла. Текст уже прочитан
    /// платформенным слоем; здесь: форс-сохранение прежней сцены (незакрытые
    /// правки не теряются — SPEC §9), парсинг, подмена SceneState и сброс
    /// переходного UI-состояния (индексы нод новой сцены несовместимы со
    /// старой — селекция/drag/редактор/поиск/миникарта устаревают мгновенно).
    /// Камера — дефолт старта (viewport новой сцены дефолтный, ср. App::new).
    fn on_open_scene(
        &mut self,
        path: PathBuf,
        json: String,
        storage: Option<Arc<dyn CanvasStorage>>,
    ) {
        // Незакрытые правки прежней сцены — в её хранилище до подмены
        if self.scene.dirty_since.is_some() {
            self.scene.save_now();
        }
        let opened = path.display().to_string();
        let canvas = match Canvas::from_str(&json) {
            Ok(canvas) => canvas,
            Err(err) => {
                // Битый файл/текст — прежняя сцена продолжает жить (деградация,
                // не паника; правило обёртки — SPEC §7.5-стиль тоста)
                self.show_toast(
                    self.trf(keys::TOAST_CANVAS_NOT_OPEN, &[("{err}", &err.to_string())]),
                );
                self.request_redraw();
                return;
            }
        };
        let next = match storage {
            Some(storage) => SceneState::with_storage(canvas, path, storage),
            None => SceneState::with_storage(canvas, path, Arc::clone(&self.scene.storage)),
        };
        self.scene = next;
        // Сброс переходного UI: всё, что ссылалось на ноды/геометрию старой
        // сцены. Панели-оверлеи (настройки/хоткеи/помощь/доки/онбординг)
        // сознательно НЕ трогаем — они про приложение, не про сцену.
        self.selected = None;
        self.selected_nodes.clear();
        self.dragging = None;
        self.editing = None;
        self.editor_dragging = false;
        self.menu = None;
        self.edge_drag = None;
        self.select_rect = None;
        self.drop_preview = None;
        self.dialog = None;
        self.pending_undo = None;
        self.resizing = None;
        self.hovered = None;
        self.node_clipboard.clear();
        self.search = SearchPanel::default();
        self.search_nodes.clear();
        self.search_pending = None;
        self.flight = None;
        self.pulse = None;
        self.minimap = None;
        self.minimap_drag = false;
        self.template_drag = None;
        // FR-038 (п.9): сцена заменена — оси направляющих/ghost старой сцены
        // указывали на исчезнувшую геометрию, слой гасится
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.clear_guides();
        }
        self.camera = Camera::default();
        self.show_toast(self.trf(keys::TOAST_CANVAS_OPENED, &[("{name}", &opened)]));
        self.request_redraw();
    }

    fn request_redraw(&self) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// M8/W4 (wasm-port §3.4): конфигурация свежего Renderer и первый кадр —
    /// тело Ok-ветки инициализации до W4, выделено для синхронного (натив,
    /// pollster) и async (web, слот) путей.
    fn install_renderer(&mut self, mut renderer: canvas_render::Renderer) {
        renderer.set_grid_visible(self.settings.grid_visible);
        renderer.set_grid_dots(self.settings.grid_style == GridStyle::Dots);
        let (minor, major) = self.settings.grid_density.steps();
        renderer.set_grid_steps(minor, major);
        // FR-038 (п.3): пороги zoom-адаптивной сетки — пользовательские
        // настройки (старт: sub 150% / coarse 50% из v2)
        renderer.set_grid_zoom_thresholds(
            self.settings.snap_grid_sub_zoom,
            self.settings.snap_grid_coarse_zoom,
        );
        renderer.set_theme(self.effective_palette());
        if let Some(window) = &self.window {
            tracing::info!(
                width = window.inner_size().width,
                height = window.inner_size().height,
                scale_factor = window.scale_factor(),
                "окно создано"
            );
        }
        self.renderer = Some(renderer);
        // FR-061 этап D (D-8): снимок описаний шаблонов — в новый рендер
        // (в сцену установлен при построении App / sync_template_descs).
        let descs = self.template_desc_map();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_template_descs(descs);
        }
        self.request_redraw();
    }

    /// FR-061 этап D (D-8): снимок описаний манифестов шаблонов
    /// (id → описание, Q3) — источник зоны описания шаблонных нод.
    fn template_desc_map(&self) -> std::collections::HashMap<String, String> {
        self.templates
            .list()
            .iter()
            .map(|m| (m.id.clone(), m.description.clone()))
            .collect()
    }

    /// Строка HUD (F3): fps, p95 frame time, счётчик culling последнего кадра.
    /// Рендер идёт по request_redraw, поэтому fps осмыслен во время активного
    /// пан/зума; в простое кадры не рисуются и замер не обновляется.
    fn hud_text(&self) -> Option<String> {
        if !self.hud_visible {
            return None;
        }
        let fps = self
            .frame_meter
            .fps()
            .map(|v| format!("{v:.0}"))
            .unwrap_or_else(|| "—".into());
        let p95 = self
            .frame_meter
            .p95_ms()
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "—".into());
        let thumbs = self
            .renderer
            .as_ref()
            .map(|r| r.thumbnail_count())
            .unwrap_or(0);
        Some(format!(
            "{fps} fps | p95 {p95} мс | кадр {:.1} мс | нод видно {}/{} | связей видно {}/{} | инстансов {} | тамбнейлов {} (очередь {})",
            self.last_stats.cpu_ms,
            self.last_stats.visible_nodes,
            self.last_stats.total_nodes,
            self.last_stats.visible_edges,
            self.last_stats.total_edges,
            self.last_stats.instances,
            thumbs,
            self.thumbs.queue_len()
        ))
    }

    /// FR-050 Н2 (этап C): пункт меню выбора под курсором — геометрия
    /// отрисовки (сдвиг на заголовок); None — заголовок/мимо (отмена).
    fn choice_menu_item_at(&self, cursor: Vec2) -> Option<usize> {
        let menu = self.choice_menu.as_ref()?;
        let shifted = [menu.origin[0], menu.origin[1] + CHOICE_MENU_TITLE_H];
        crate::ui::menu_item_at_for(shifted, cursor, menu.items.len())
    }

    // --- Палитра выделения (FR-009/FR-010) ---

    /// Цель палитры из текущего выделения; None — палитра скрыта
    /// (нет выделения, drag/редактирование/поиск/диалог/рамка выделения).
    fn palette_target(&self) -> Option<PaletteTarget> {
        if self.dialog.is_some()
            || self.search.is_open()
            || self.editing.is_some()
            || self.dragging.is_some()
            || self.edge_drag.is_some()
            || self.select_rect.is_some()
            // Взаимоисключение поповеров: открытое меню канваса прячет
            // палитру (практика UI: transient-поповер один за раз;
            // обратное направление — RMB по ноде закрывает меню)
            || self.menu.is_some()
        {
            return None;
        }
        // Список выделенных нод: primary — одиночное/составное выделение
        // или первый из мультивыделения рамкой (CR-001, там selected = None)
        let selected: Vec<usize> = match self.selected {
            Some(Selection::Node(primary)) => {
                let mut selected = vec![primary];
                for &index in &self.selected_nodes {
                    if !selected.contains(&index) && self.scene.canvas.nodes.get(index).is_some() {
                        selected.push(index);
                    }
                }
                selected
            }
            None if !self.selected_nodes.is_empty() => self
                .selected_nodes
                .iter()
                .copied()
                .filter(|&index| self.scene.canvas.nodes.get(index).is_some())
                .collect(),
            _ => Vec::new(),
        };
        if selected.is_empty() {
            return match self.selected {
                Some(Selection::Edge(edge_index))
                    if self.scene.canvas.edges.get(edge_index).is_some() =>
                {
                    Some(PaletteTarget::Edge(edge_index))
                }
                _ => None,
            };
        }
        // Primary — первый из списка (для одиночного выделения он и есть
        // единственный; для рамки — первый по порядку выделения)
        let primary = selected[0];
        Some(PaletteTarget::Nodes { primary, selected })
    }

    /// Screen-якорь палитры: низ bbox выделенных нод (или середина связи),
    /// в логических px; None — цель без геометрии.
    fn palette_anchor_screen(&self, target: &PaletteTarget) -> Option<[f32; 2]> {
        let viewport = self.viewport_logical();
        let gap = crate::palette::PAL_ANCHOR_GAP;
        match target {
            PaletteTarget::Nodes { selected, .. } => {
                // bbox всех выделенных (первичный + мультивыделение)
                let mut bbox: Option<[f32; 4]> = None;
                for index in selected {
                    let node = self.scene.canvas.nodes.get(*index)?;
                    let rect = [node.x, node.y, node.width, node.height];
                    bbox = Some(match bbox {
                        None => rect,
                        Some(b) => [
                            b[0].min(rect[0]),
                            b[1].min(rect[1]),
                            (b[0] + b[2]).max(rect[0] + rect[2]) - b[0].min(rect[0]),
                            (b[1] + b[3]).max(rect[1] + rect[3]) - b[1].min(rect[1]),
                        ],
                    });
                }
                let b = bbox?;
                let bottom_center = [b[0] + b[2] / 2.0, b[1] + b[3]];
                let screen = self.camera.world_to_screen(bottom_center, viewport);
                Some([screen[0], screen[1] + gap])
            }
            PaletteTarget::Edge(edge_index) => {
                let edge = self.scene.canvas.edges.get(*edge_index)?;
                let avoid = self.settings.edges_avoid_nodes;
                let mid = canvas_core::edge_midpoint(&self.scene.canvas, edge, avoid)?;
                let screen = self.camera.world_to_screen(mid, viewport);
                Some([screen[0], screen[1] + gap])
            }
        }
    }

    /// Чистая геометрия палитры: (layout, группы, цель). Без обновления
    /// hover-состояния — используется и для отрисовки, и для проверки
    /// «курсор над screen-space поверхностью» (колесо над UI холст
    /// не двигает).
    fn palette_geometry(
        &self,
    ) -> Option<(
        PaletteLayout,
        Vec<crate::palette::PaletteGroup>,
        PaletteTarget,
    )> {
        let target = self.palette_target()?;
        // FR-042 (правка 2026-09-24): вес пучка рёбер цели — для группы
        // «Пучок» в edge_groups (явный вход в main stage + удаление ребра).
        // У нод и одиночных рёбер — None (группа не показывается).
        let bundle_weight = match &target {
            PaletteTarget::Edge(edge_index) => self
                .scene
                .bundles
                .bundle_of_edge(*edge_index)
                .map(|bundle| bundle.weight),
            PaletteTarget::Nodes { .. } => None,
        };
        let mut groups = palette_groups(
            &self.scene.canvas,
            &target,
            bundle_weight,
            self.settings.language,
        );
        // FR-019: linked-связь с шаблоном — при несовпадении версии ноды
        // с реестром группа «Шаблон» с ручным update
        if let PaletteTarget::Nodes { primary, .. } = &target {
            if let Some(group) = template_update_group(
                &self.scene.canvas,
                *primary,
                &self.templates,
                self.settings.language,
            ) {
                groups.push(group);
            }
        }
        if groups.is_empty() {
            return None;
        }
        let viewport = self.viewport_logical();
        let anchor = self.palette_anchor_screen(&target)?;
        let origin = palette_origin(anchor, palette_bar_size(&groups), viewport);
        let lay = palette_layout(origin, &groups, viewport);
        Some((lay, groups, target))
    }

    /// Вид палитры на кадр: (layout, группы, открытая hover'ом группа).
    /// Побочно обновляет `palette_hover` (hover-intent/отсрочка закрытия)
    /// и сбрасывает его при смене цели — вызывается и на кликах, и на кадрах.
    fn palette_view(
        &mut self,
    ) -> Option<(
        PaletteLayout,
        Vec<crate::palette::PaletteGroup>,
        Option<usize>,
    )> {
        let (lay, groups, target) = self.palette_geometry()?;
        let open = {
            if self.palette_seen.as_ref() != Some(&target) {
                // Смена цели: раскрытая группа прежней цели недействительна
                self.palette_hover.reset();
                self.palette_seen = Some(target);
            }
            self.palette_hover.update(&lay, self.cursor)
        };
        Some((lay, groups, open))
    }

    /// CR-008: закрепить/освободить конец связи. Закрепление фиксирует
    /// текущую эффективную сторону конца в `fromSide`/`toSide` (файл
    /// отражает то, что видно на экране); освобождение оставляет
    /// сохранённую сторону в файле, но визуально возвращает авто.
    fn set_edge_port_pin(&mut self, edge_index: usize, end: canvas_core::EdgeEnd, pin: bool) {
        if !pin {
            let Some(edge) = self.scene.canvas.edges.get(edge_index) else {
                return;
            };
            let (pin_from, pin_to) = edge.port_pins();
            let currently = match end {
                canvas_core::EdgeEnd::From => pin_from,
                canvas_core::EdgeEnd::To => pin_to,
            };
            if !currently {
                return; // no-op — шаг не копится
            }
            let mut snapshot = self.scene.canvas.clone();
            if let Some(edge) = snapshot.edges.get_mut(edge_index) {
                edge.set_port_pin(end, false);
            }
            self.scene.push_undo(snapshot);
            self.scene.mark_dirty();
            self.show_toast(self.tr(keys::TOAST_PORT_FREED));
            self.request_redraw();
            return;
        }
        // Закрепление: текущая эффективная сторона конца (та же геометрия,
        // по которой рисуется линия)
        let Some((side, _)) = canvas_core::edge_endpoint(&self.scene.canvas, edge_index, end)
        else {
            return;
        };
        let mut snapshot = self.scene.canvas.clone();
        let Some(edge) = snapshot.edges.get_mut(edge_index) else {
            return;
        };
        match end {
            canvas_core::EdgeEnd::From => edge.from_side = Some(side),
            canvas_core::EdgeEnd::To => edge.to_side = Some(side),
        }
        edge.set_port_pin(end, true);
        self.scene.push_undo(snapshot);
        self.scene.mark_dirty();
        self.show_toast(self.tr(keys::TOAST_PORT_PINNED));
        self.request_redraw();
    }

    /// FR-040: перевод ключа по языку настроек этого приложения
    /// (обёртка [`i18n::tr`] — короткая форма для мест рендера/ввода).
    fn tr(&self, key: &'static str) -> &'static str {
        i18n::tr(self.settings.language, key)
    }

    /// FR-040: перевод с подстановками (обёртка [`i18n::trf`]).
    fn trf(&self, key: &'static str, subs: &[(&str, &str)]) -> String {
        i18n::trf(self.settings.language, key, subs)
    }

    /// FR-040 v2: персист конфига (натив — `config.toml` через
    /// `Settings::save`; web — TOML-текст в localStorage
    /// `canvasdesk.config`). 4 точки изменения настроек (тема/язык/
    /// палитра/apply_dropdown) вызывают этот метод — единый источник
    /// правды; до этого в каждой точке был свой блок `if let Some(path)`.
    fn persist_settings(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(path) = &self.config_path {
                if let Err(err) = self.settings.save(path) {
                    tracing::warn!(%err, "не удалось сохранить конфиг");
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.persist_settings_web();
        }
    }

    /// Web (wasm32): TOML-сериализация `Settings` и запись в localStorage
    /// браузера (`canvasdesk.config` — тот же ключ/формат, что читается
    /// в `canvas_web::app_spawn::load_settings`). Фолбэк на дефолт при
    /// ошибке — никогда: любой браузер без localStorage (приватный режим)
    /// молча игнорируется, страница не падает.
    #[cfg(target_arch = "wasm32")]
    fn persist_settings_web(&self) {
        let Some(window) = web_sys::window() else {
            tracing::warn!("localStorage: нет window (вне браузера)");
            return;
        };
        let Ok(Some(storage)) = window.local_storage() else {
            tracing::warn!("localStorage недоступен (приватный режим?)");
            return;
        };
        match toml::to_string(&self.settings) {
            Ok(text) => {
                if let Err(err) = storage.set_item("canvasdesk.config", &text) {
                    tracing::warn!("set_item localStorage упал: {}", wasm_js_err_display(&err));
                }
            }
            Err(err) => {
                tracing::warn!(%err, "сериализация настроек в TOML упала");
            }
        }
    }

    /// Переключить тему (кнопка-иконка рядом с кнопкой настроек) и сохранить конфиг.
    fn toggle_theme(&mut self) {
        self.settings.theme = self.settings.theme.next();
        // FR-047: явный выбор классики тумблером сбрасывает пресет
        self.settings.theme_preset.clear();
        // M5: смена темы уходит виджетам (themeChanged — T21 доведёт
        // рассылку до инстансов, пока обновляется init-данные будущих нод)
        self.widgets.set_theme(self.settings.theme == Theme::Dark);
        let palette = self.effective_palette();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_theme(palette);
        }
        self.persist_settings();
    }

    /// FR-040 v2: переключить язык интерфейса (Ru ↔ En) кнопкой-иконкой в
    /// угловом кластере. Сохранение в `config.toml`/localStorage (поле
    /// `language`), применение — на лету: тексты читаются по кадру через
    /// `tr()`, имена шаблонов — `TemplateManifest::display_name(language)`
    /// в палитре/wheel-меню; ноды-шаблоны, уже созданные ранее, сохраняют
    /// снапшот имени (FR-023: snapshot переживает правки; выбор языка в
    /// момент инстанциации зафиксирован в `TemplateRef.name`).
    fn toggle_language(&mut self) {
        self.settings.language = self.settings.language.next();
        // Toast-подтверждение на новом языке (как Obsidian/VS Code — язык
        // применён сразу, индикатор — собственная локаль).
        self.show_toast(i18n::trf(
            self.settings.language,
            keys::TOAST_LANGUAGE_TOGGLED,
            &[("{lang}", self.settings.language.native_label())],
        ));
        self.persist_settings();
    }

    /// FR-025: сохранить развёрнутость палитры-дока в конфиг
    /// (сворачивание по Esc/кнопке «‹», разворачивание по ручке/Ctrl+P).
    fn persist_palette_dock(&mut self) {
        self.settings.template_palette_open = self.template_panel.open;
        self.persist_settings();
    }

    /// Ревизия FR-025: имена категорий реестра шаблонов (порядок реестра) —
    /// вход свёрнутой полосы палитры.
    fn template_category_names(&self) -> Vec<String> {
        self.templates
            .categories()
            .into_iter()
            .map(|category| category.to_owned())
            .collect()
    }

    /// Ревизия FR-025: геометрия flyout раскрытой категории свёрнутой полосы
    /// (None — палитра развёрнута/nothing раскрыто/индекс протух). Единый
    /// источник для рендера, hit-test и прокрутки: расхождений быть не может.
    fn template_flyout_geometry(
        &self,
        viewport: Vec2,
        strip: &template_ui::StripLayout,
    ) -> Option<template_ui::FlyoutLayout> {
        let hover = self.template_hover.as_ref()?;
        let category = hover.open?;
        let (row_rect, name) = strip.rows.get(category)?;
        let count = self.templates.by_category(name).len();
        Some(template_ui::flyout_layout(
            *row_rect,
            count,
            viewport[0],
            viewport[1],
            hover.scroll_top,
        ))
    }

    /// Ревизия FR-025: кадровое обновление hover-состояния свёрнутой полосы
    /// категорий: строка под курсором, удержание внутри flyout, hover-intent
    /// открытие / grace-закрытие. true — раскрытие или строка под курсором
    /// изменились (нужна перерисовка); при неподвижном курсоре стабильно
    /// false — `about_to_wait` не крутит пустые кадры.
    fn update_template_hover(&mut self) -> bool {
        if self.template_panel.open {
            return false;
        }
        let viewport = self.viewport_logical();
        let categories = self.template_category_names();
        // FR-054: ширины чипов — измеренные (measurer на вызов, паттерн U3).
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let strip =
            template_ui::dock_strip_layout(&categories, viewport[1], &mut measurer, &mut fs);
        let hovered = strip
            .rows
            .iter()
            .position(|(rect, _)| point_in_rect(*rect, self.cursor));
        if hovered.is_none() && self.template_hover.is_none() {
            return false;
        }
        let in_flyout = self
            .template_flyout_geometry(viewport, &strip)
            .is_some_and(|fly| point_in_rect(fly.rect, self.cursor));
        let hover = self
            .template_hover
            .get_or_insert_with(template_ui::StripHover::new);
        hover.update_at(hovered, in_flyout, Instant::now())
    }

    /// T23 (brainstorm-focus): пересчёт состояния фокуса на кадр —
    /// фейд затемнения, пульс «дыхания» и окрестность семени
    /// (hover → выделенная нода → выделенная связь; O(V+E) — на 5k нод
    /// ~0.3–0.5 мс, кадры вне изменений не генерируются). Выделенная нода
    /// добавляется в яркий набор: выделение не гаснет (приоритет над фокусом).
    /// Фейд затемнения фокуса к цели (общий механизм для T23-семени и
    /// подсветки цепочки explain F-4 — один рендер-путь затемнения).
    fn advance_focus_dim(&mut self, target: f32) {
        let needs_new_fade = match self.focus_fade {
            Some((_, to, _)) => (to - target).abs() > 1e-3,
            None => (self.focus_dim - target).abs() > 1e-3,
        };
        if needs_new_fade {
            self.focus_fade = Some((self.focus_dim, target, Instant::now()));
        }
        if let Some((from, to, start)) = self.focus_fade {
            let elapsed = start.elapsed().as_millis() as u32;
            if elapsed >= FOCUS_FADE_MS {
                self.focus_dim = to;
                self.focus_fade = None;
            } else {
                self.focus_dim = from + (to - from) * focus_fade(elapsed);
            }
        }
    }

    fn update_focus_state(&mut self) {
        // PRD-0007 (X4, AC-5.3): пока открыт диалог подтверждения отката
        // пачки, focus_edges держит подсветку отменяемого — не перетирать
        if matches!(self.dialog, Some(AppDialog::AutolinkRollback { .. })) {
            return;
        }
        // FR-050 Н9-3 (этап E): «Показать источник» — открытое окно
        // подсветки истока/связи/приёмника (машинерия фокуса PRD-0007/
        // FR-048, без дублирования): затемнение и рёбра — из снапшота,
        // «дыхание» тикает своим полем; живёт SHOW_SOURCE_MS, затем —
        // фейд к обычному состоянию (провал в штатную логику ниже).
        if let Some((nodes, edges, start)) = &self.show_source {
            if (start.elapsed().as_millis() as u32) < SHOW_SOURCE_MS {
                self.focus_nodes = nodes.clone();
                self.focus_edges = edges.clone();
                self.advance_focus_dim(1.0);
                return;
            }
            // Время вышло: окно гаснет, штатная логика (fade → 0)
            self.show_source = None;
        }
        // PRD-0007 (F-4/AC-3.1): открытое окно Ready — подсветка цепочки
        // из снапшота дерева (F-5: одна модель для окна и подсветки),
        // затемнение прочего — тем же фейдом. Loading не затемняет (У5:
        // затемнение появляется атомарно с деревом). Режим фокуса T23
        // не нужен — семя не участвует.
        if let Some(tree) = self.explain.as_ref().and_then(|s| s.tree()) {
            let set = explain_chain_focus(&self.scene.canvas, tree);
            self.focus_nodes = set.nodes;
            self.focus_edges = set.edges;
            self.advance_focus_dim(1.0);
            self.focus_pulse = None;
            return;
        }
        // CR-001: мультивыделение без primary — семя из первой выделенной
        // (фокус живёт и после сброса одиночного клика)
        let selected = self
            .selected
            .or_else(|| self.selected_nodes.first().copied().map(Selection::Node));
        let seed = focus_seed_of(self.hovered, selected);
        let focus_on = self.settings.focus_mode;
        // Цель затемнения: 1 — режим включён и семя есть; иначе всё гаснет
        let target = f32::from(focus_on && seed.is_some());
        self.advance_focus_dim(target);
        // «Дыхание»: рестарт при смене семени (режим включён), один цикл,
        // затем поле очищается — кадры для статики не нужны
        if focus_on {
            if let Some(seed) = seed {
                // is_some_and (не is_none_or): MSRV проекта 1.80
                if !self.focus_pulse.as_ref().is_some_and(|(s, _)| *s == seed) {
                    self.focus_pulse = Some((seed, Instant::now()));
                }
                if let Some((_, start)) = self.focus_pulse {
                    if start.elapsed().as_millis() as u32 >= FOCUS_PULSE_MS {
                        self.focus_pulse = None;
                    }
                }
            } else {
                self.focus_pulse = None;
            }
        } else {
            self.focus_pulse = None;
        }
        // Окрестность: пересчёт только при включённом режиме (иначе пусто)
        if focus_on {
            if let Some(seed) = seed {
                let mut set = focus_set(&self.scene.canvas, seed);
                // Выделенная нода (кроме семени-связи — у неё свои концы)
                // не гаснет вместе с остальными (план T23 §7); CR-001 —
                // весь набор мультивыделения тоже остаётся ярким
                let seed_is_edge = matches!(seed, FocusSeed::Edge(_));
                if let (Some(Selection::Node(index)), false) = (self.selected, seed_is_edge) {
                    if !set.contains_node(index) {
                        set.nodes.push(index);
                    }
                }
                if !seed_is_edge {
                    for index in &self.selected_nodes {
                        if !set.contains_node(*index) {
                            set.nodes.push(*index);
                        }
                    }
                }
                set.nodes.sort_unstable();
                set.nodes.dedup();
                self.focus_nodes = set.nodes;
                self.focus_edges = set.edges;
            } else {
                self.focus_nodes.clear();
                self.focus_edges.clear();
            }
        } else if !self.focus_nodes.is_empty() || !self.focus_edges.is_empty() {
            self.focus_nodes.clear();
            self.focus_edges.clear();
        }
    }

    /// T23: анимации фокуса ещё идут (кадры держит about_to_wait)?
    fn focus_animating(&self) -> bool {
        self.focus_fade.is_some()
            || self
                .focus_pulse
                .as_ref()
                .is_some_and(|(_, start)| start.elapsed().as_millis() < u128::from(FOCUS_PULSE_MS))
            // FR-050 Н9-3 (этап E): окно «Показать источник» — затемнение
            // держится до истечения токена (затем фейд обратно)
            || self.show_source.is_some()
    }

    /// FR-050 Н9-1 (этап E): волна каскада — перестройка и тик.
    /// Перестройка: ревизия потока изменилась → seeds = ноды с изменившимся
    /// итогом (`scene.flow_changed_nodes`) → `flow::spill_wave` (рёбра
    /// вниз с порядком топологического расстояния); пустой диф — волна
    /// гаснет (мутация без изменения значений: сдвиг, переименование).
    /// Тик: за последним порядком + длительность ребра волна снимается
    /// (кадры для статики не нужны). Вызывается ДО сборки SceneView кадра.
    fn update_spill_wave(&mut self) {
        if self.scene.revision != self.seen_flow_revision {
            self.seen_flow_revision = self.scene.revision;
            let seeds: std::collections::HashSet<String> =
                self.scene.flow_changed_nodes.iter().cloned().collect();
            let edges = if seeds.is_empty() {
                Vec::new()
            } else {
                canvas_core::flow::spill_wave(&self.scene.canvas, &seeds)
            };
            self.spill_wave = (!edges.is_empty()).then(|| (edges, Instant::now()));
        } else if let Some((edges, start)) = &self.spill_wave {
            let max_order = edges.iter().map(|(_, order)| *order).max().unwrap_or(0);
            let total = max_order
                .saturating_mul(SPILL_WAVE_STEP_MS)
                .saturating_add(SPILL_WAVE_EDGE_MS);
            if start.elapsed().as_millis() as u32 >= total {
                self.spill_wave = None;
            }
        }
    }

    /// Н9-1: волна ещё анимируется (кадры держит about_to_wait)?
    fn spill_wave_animating(&self) -> bool {
        self.spill_wave.as_ref().is_some_and(|(edges, start)| {
            let max_order = edges.iter().map(|(_, order)| *order).max().unwrap_or(0);
            let total = max_order
                .saturating_mul(SPILL_WAVE_STEP_MS)
                .saturating_add(SPILL_WAVE_EDGE_MS);
            start.elapsed().as_millis() < u128::from(total)
        })
    }

    // --- FR-073: расталкивание при драге -----------------------------------

    /// FR-073: параметры физики из настроек (мост Settings → drag_push).
    fn drag_push_params(&self) -> DragPushParams {
        DragPushParams {
            halo: self.settings.drag_push_halo_px,
            gap: self.settings.drag_push_gap_px,
            ret: self.settings.drag_push_ret,
            push_frac: self.settings.drag_push_push_frac,
            pair_frac: self.settings.drag_push_pair_frac,
            iters: self.settings.drag_push_iters,
            predictive: self.settings.drag_push_predictive,
            rebase: self.settings.drag_push_rebase,
        }
    }

    /// FR-073: индексы активных нод текущего drag (мультивыделение тянется
    /// жёстко — физика их не двигает).
    fn drag_push_active(&self) -> Vec<usize> {
        match self.dragging.as_ref() {
            Some(drag) => {
                let mut active = Vec::with_capacity(drag.origins.len() + 1);
                active.push(drag.primary);
                active.extend(drag.origins.iter().map(|(index, _)| *index));
                active
            }
            None => Vec::new(),
        }
    }

    /// FR-073: открыть сессию физики — якоря всех нод = текущие позиции
    /// (вызывается на старте drag; поглощает внешние сдвиги между драгами).
    fn drag_push_begin(&mut self) {
        self.drag_push.reanchor_all(&self.scene.canvas);
        self.drag_push_live = true;
        // Оракул браузерного дыма (по образцу W7/W9): старт сессии — якоря,
        // камера и viewport (screen↔world для headless-проверок, ?log=debug)
        tracing::debug!(
            target: "canvas_app",
            anchors = ?self.scene.canvas.nodes.iter().map(|n| (n.id.as_str(), [n.x, n.y])).collect::<Vec<_>>(),
            center = ?self.camera.position(),
            zoom = self.camera.zoom(),
            viewport = ?self.viewport_logical(),
            "drag_push: сессия открыта (якоря = текущие позиции)"
        );
    }

    /// FR-073: кадр физики — true, если были сдвиги (spatial/перерисовка).
    /// Тикает только в живой сессии (от старта drag до расселения после
    /// drop) — внешние сдвиги нод между драгами якоря не тревожат.
    fn tick_drag_push(&mut self) -> bool {
        if !self.drag_push_live || !self.settings.drag_push_enabled {
            return false;
        }
        let params = self.drag_push_params();
        let active = self.drag_push_active();
        let touched = canvas_core::drag_push::step(
            &mut self.scene.canvas,
            &active,
            &mut self.drag_push,
            &params,
        );
        if touched.is_empty() {
            if self.dragging.is_none() {
                self.drag_push_live = false; // расселилось — сессия закрыта
            }
            return false;
        }
        // Оракул браузерного дыма: кто и где сдвинулся на этом шаге
        // (id/позиции пассивных нод; группы в touched не должны попадать
        // никогда — рамки непрозрачны для физики, см. drag_push::step)
        tracing::debug!(
            target: "canvas_app",
            moved = ?touched.iter().filter_map(|&i| self.scene.canvas.nodes.get(i).map(|n| (n.id.as_str(), n.x, n.y))).collect::<Vec<_>>(),
            "drag_push: шаг физики"
        );
        for index in touched {
            if let Some(node) = self.scene.canvas.nodes.get(index) {
                self.scene.spatial.update(index, node);
            }
        }
        self.scene.mark_dirty();
        true
    }

    /// FR-073: максимальное отклонение нод от якорей, world px.
    fn drag_push_displacement(&self) -> f32 {
        self.scene.canvas.nodes.iter().fold(0.0_f32, |max, node| {
            let anchor = self
                .drag_push
                .anchors
                .get(&node.id)
                .copied()
                .unwrap_or([node.x, node.y]);
            let d = ((anchor[0] - node.x).powi(2) + (anchor[1] - node.y).powi(2)).sqrt();
            max.max(d)
        })
    }

    /// FR-073: физика ещё анимируется (кадры держит about_to_wait)? Во
    /// время drag — всегда (ореол давит, упреждение живёт); после drop —
    /// пока ноды не расселились по якорям.
    fn drag_push_animating(&self) -> bool {
        self.drag_push_live
            && self.settings.drag_push_enabled
            && (self.dragging.is_some() || self.drag_push_displacement() > 0.5)
    }

    /// T23: переключить режим фокуса связей (хоткей F / ПКМ-меню / панель
    /// настроек — панель сохраняет конфиг общим хвостом apply_settings_row,
    /// хоткей и меню — рантайм-переключение без записи).
    fn toggle_focus_mode(&mut self) {
        self.settings.focus_mode = !self.settings.focus_mode;
        tracing::info!(
            вкл = self.settings.focus_mode,
            "режим фокуса связей (brainstorm-focus)"
        );
        self.request_redraw();
    }

    /// FR-016 (CP5): переключить оверлей узких мест (Ctrl+B / пункт меню
    /// канваса / панель настроек). Персистентная настройка: сохранение —
    /// вызывающим (меню/панель — save_settings, хоткей — сам сохраняет).
    /// Рендер читает флаг на кадре (SceneView.analysis_overlay).
    fn toggle_bottleneck_overlay(&mut self) {
        self.settings.bottleneck_overlay = !self.settings.bottleneck_overlay;
        tracing::info!(
            вкл = self.settings.bottleneck_overlay,
            "оверлей узких мест (FR-016)"
        );
        // Ручной тогл выключает авто-включение до конца запуска: решение
        // владельца важнее эвристики (FR-016 «Открытые вопросы»).
        self.bottleneck_auto_enabled = true;
        self.request_redraw();
    }

    /// FR-026: применить выбор значения из выпадающего меню — та же чистая
    /// функция значений, что в тестах инвариантов; побочные эффекты рендера
    /// те же, что были в ветках apply_settings_row. Конфиг сохраняется
    /// общим хвостом. Смена угла кнопки перепривязывает панель — открытое
    /// меню закрывает вызывающий (состояние привязано к строке, не к точке).
    fn apply_dropdown_choice(&mut self, row: SettingsRow, index: usize) {
        apply_dropdown_value(&mut self.settings, row, index);
        // FR-047: смена пресета темы — рендер + виджеты сразу
        if row == SettingsRow::ThemePreset {
            self.apply_effective_theme();
        }
        self.sync_settings_row(row);
        self.save_settings();
    }

    /// FR-047: эффективная палитра настроек — активный пресет
    /// (`settings.theme_preset`) или классическая тема (`settings.theme`).
    fn effective_palette(&self) -> ThemeColors {
        ThemeColors::from_settings(self.settings.theme, &self.settings.theme_preset)
    }

    /// FR-047: применить эффективную тему к рендеру и виджетам
    /// (паттерн toggle_theme/клика по карточке — единая точка).
    fn apply_effective_theme(&mut self) {
        let palette = self.effective_palette();
        self.widgets.set_theme(palette.is_dark());
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_theme(palette);
        }
        self.request_redraw();
    }

    /// FR-026: синхронизация рендера с настройками после изменения строки
    /// (то, что раньше делали ветки apply_settings_row по месту).
    fn sync_settings_row(&mut self, row: SettingsRow) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };
        match row {
            SettingsRow::Grid => renderer.set_grid_visible(self.settings.grid_visible),
            SettingsRow::GridStyle => {
                renderer.set_grid_dots(self.settings.grid_style == GridStyle::Dots)
            }
            SettingsRow::GridDensity => {
                let (minor, major) = self.settings.grid_density.steps();
                renderer.set_grid_steps(minor, major);
            }
            // FR-038 (п.3/19): пороги zoom-адаптивной сетки — в рендер сразу
            // (линии sub/coarse перерисовываются с новыми порогами)
            SettingsRow::SnapSubZoom | SettingsRow::SnapCoarseZoom => renderer
                .set_grid_zoom_thresholds(
                    self.settings.snap_grid_sub_zoom,
                    self.settings.snap_grid_coarse_zoom,
                ),
            _ => {}
        }
    }

    /// FR-026: общий хвост применения настроек — сохранение config.toml /
    /// localStorage (FR-040 v2: web persist через `persist_settings`).
    fn save_settings(&self) {
        self.persist_settings();
    }

    /// FR-027: открыть просмотрщик документации на странице (подменю «?»
    /// или внутренняя ссылка): раскладка собирается под текущую ширину,
    /// скролл сверху; меню помощи закрывается (одна «панель помощи» активна).
    fn open_docs_page(&mut self, page: usize) {
        let viewport = self.viewport_logical();
        let panel = docs_ui::viewer_rect(viewport);
        let content = docs_ui::viewer_content_rect(panel);
        // FR-054: раскладка страницы — измеренным текстом (measurer
        // создаётся на перекомпоновку, не на кадр; страница кэшируется).
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let layout = docs_ui::layout_page(page, content[2], &mut measurer, &mut fs);
        let scroll = docs_ui::ScrollState::new(layout.content_height, content[3]);
        self.docs = Some(DocsViewer {
            page,
            layout,
            scroll,
            layout_width: content[2],
        });
        self.help_menu = None;
        self.request_redraw();
    }

    /// FR-028: «Готово» на последнем шаге — тур пройден до конца:
    /// `onboarding_done = true` + сохранение + закрытие (авто-показ молчит
    /// навсегда, ручной вход из меню «?» остаётся).
    fn complete_onboarding(&mut self) {
        self.settings.onboarding_done = true;
        self.onboarding = None;
        self.save_settings();
        self.request_redraw();
    }

    /// FR-028: «Пропустить»/Esc — отложить тур до следующего запуска:
    /// инкремент счётчика с клампом 3 (после третьего подряд авто-показ
    /// замолкает) + сохранение + закрытие. Ручной запуск из меню «?» идёт
    /// МИМО этого метода — счётчик не трогается (явное намерение).
    fn defer_onboarding(&mut self) {
        self.settings.onboarding_defers =
            canvas_core::clamp_onboarding_defers(self.settings.onboarding_defers.saturating_add(1));
        self.onboarding = None;
        self.save_settings();
        self.request_redraw();
    }

    /// FR-054 (гейт G4): панель хоткеев и полоса палитры — соседи
    /// «лево-центр» одного слоя `Panels`; при свёрнутой палитре панель
    /// хоткеев смещается правее полосы (налезание интерактивных rect'ов
    /// одного слоя запрещено; прежде полоса рисовалась поверх панели).
    fn hotkeys_left_offset(&self, viewport: Vec2) -> f32 {
        if self.template_panel.open {
            return 0.0; // док развёрнут — полосы нет, панель у левого края
        }
        let categories = self.template_category_names();
        let mut measurer = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        let strip =
            template_ui::dock_strip_layout(&categories, viewport[1], &mut measurer, &mut fs);
        strip.rect[2] + crate::ui::SETTINGS_GAP
    }

    /// Заказать тамбнейлы видимых файловых нод (T6): приоритет High,
    /// дедупликация — в ThumbService, по наличию в атласе и по негативному кэшу.
    fn order_thumbnails(&self) {
        let Some(renderer) = &self.renderer else {
            return;
        };
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return;
        }
        let visible = self.camera.visible_world_rect(viewport);
        for index in self.scene.spatial.query_rect(visible) {
            let node = &self.scene.canvas.nodes[index];
            let Some(file) = node.file.as_deref() else {
                continue;
            };
            if renderer.has_thumbnail(index) || self.thumbs_failed.contains(&index) {
                continue;
            }
            let path = self.scene.resolve_file_path(file);
            self.thumbs.request(Priority::High, index, path);
        }
    }
}

impl App {
    /// Забрать накопившиеся MCP-запросы из pipe-сервера и ответить на каждый
    /// (MCP-интеграция): tools/call → `mcp_dispatch`, ошибки валидации и
    /// «не найдено» — в MCP-идиоме isError (не JSON-RPC error); битый
    /// конверт — JSON-RPC error с null-id. Красим окно при любом обращении.
    #[cfg(windows)]
    fn on_mcp_wake(&mut self) {
        let Some(server) = self.mcp_server.as_ref() else {
            return;
        };
        let mut handled = false;
        while let Some((line, respond)) = server.take_request() {
            handled = true;
            let reply = match canvas_mcp::parse_envelope(&line) {
                // Notification (id == None) — отвечать нечему
                Ok(request) if request.id.is_none() => continue,
                Ok(request) => {
                    let id = request.id.unwrap_or(serde_json::Value::Null);
                    let (method, params) = mcp_unwrap_call(&request.method, &request.params);
                    // ADR-0012 (viewport-зеркало): до диспетчера камера
                    // снимается в scene.viewport, после — применяется
                    // обратно; числа идентичны, viewport_get/set не меняется
                    let position = self.camera.position();
                    self.scene.viewport = Viewport {
                        x: position[0],
                        y: position[1],
                        zoom: self.camera.zoom(),
                    };
                    let result = mcp_dispatch(&mut self.scene, &self.templates, &method, &params);
                    self.camera
                        .set_center([self.scene.viewport.x, self.scene.viewport.y]);
                    self.camera.set_zoom(self.scene.viewport.zoom);
                    match result {
                        Ok(value) => {
                            // node_delete — единственный инструмент, чистивший
                            // выделение в старом SceneState (индексы сдвинулись):
                            // UI-поля теперь в App (ADR-0012), чистим здесь
                            if method == "node_delete" {
                                self.selected = None;
                                self.selected_nodes.clear();
                                // FR-038 (п.9): MCP удалил ноду под активным
                                // drag — оси предпросмотра стали устаревшими
                                if let Some(renderer) = self.renderer.as_mut() {
                                    renderer.clear_guides();
                                }
                                self.dragging = None;
                            }
                            canvas_mcp::build_result(&id, &value)
                        }
                        Err(message) => canvas_mcp::build_call_error(&id, &message),
                    }
                }
                Err(err) => canvas_mcp::build_error(err.id.as_ref(), err.code, &err.message),
            };
            respond(reply);
        }
        if handled {
            self.request_redraw();
        }
    }
}

impl App {
    /// FR-052 (U2): dismiss-транзиенты при клике мимо ВСЕХ поверхностей
    /// (клик уходит в канвас): фокус дока палитры, flyout свёрнутой полосы,
    /// hover-intent палитры выделения (обновлялся на каждом клике и в
    /// прежней цепочке). Commit редактора делает редакторная ветка
    /// canvas-цепочки — как раньше (9877–9907).
    fn dismiss_transients_on_miss(&mut self) {
        if self.template_panel.open {
            self.template_panel.unfocus();
        }
        if self
            .template_hover
            .as_ref()
            .is_some_and(|h| h.open.is_some() || h.pending())
        {
            self.template_hover = None;
        }
        let _ = self.palette_view();
    }

    /// Empty-state: кнопки карточки; мимо — канвас жив (AC-1.1).
    fn click_empty_state(&mut self) {
        // FR-049: empty-state пустого канваса — кнопки карточки;
        // мимо карточки канвас жив (двойной клик создаёт заметку,
        // empty-state исчезает при первой ноде — AC-1.1)
        if self.empty_state_visible() {
            let card = scheme_gallery_ui::empty_card_rect(self.viewport_logical());
            let (open_btn, dismiss_btn) = scheme_gallery_ui::empty_buttons(card);
            if scheme_gallery_ui::point_in_rect(open_btn, self.cursor) {
                self.scheme_gallery.open();
                self.request_redraw();
                return;
            }
            if scheme_gallery_ui::point_in_rect(dismiss_btn, self.cursor) {
                self.empty_state_dismissed = true;
                self.request_redraw();
                return;
            }
            if scheme_gallery_ui::point_in_rect(card, self.cursor) {
                self.request_redraw();
            }
        }
    }

    /// Панель поиска: строка — прыжок, панель — глотается.
    fn click_search(&mut self) {
        // Панель поиска (T14): клик по строке — прыжок, мимо панели —
        // закрыть; канвасу клик не достаётся. Проверяется первой —
        // панель висит поверх всех оверлеев
        if self.search.is_open() {
            let viewport = self.viewport_logical();
            let lay = search_layout(viewport[0], viewport[1], &self.search);
            let mut handled = false;
            for (visible, rect) in lay.row_rects.iter().enumerate() {
                let row_rect = rect_xywh(*rect);
                if point_in_rect(row_rect, self.cursor) {
                    let row = self.search.scroll_top + visible;
                    self.search.selected = Some(row);
                    self.jump_to_search_row(row);
                    handled = true;
                    break;
                }
            }
            if !handled && !point_in_rect(rect_xywh(lay.panel_rect), self.cursor) {
                self.search.close();
            }
            self.request_redraw();
        }
    }

    /// Панель хоткеев: клик по панели глотается.
    fn click_hotkeys_panel(&mut self) {
        let viewport = self.viewport_logical();
        // Панель хоткеев (FR-004.1, тогл): панель «видно/не видно»
        // устойчива — клик мимо НЕ закрывает (переключение: F1,
        // пункт меню канваса, Esc) и проходит в канвас; клик по
        // самой панели — глотается (строки не интерактивны)
        if self.hotkeys_open {
            let panel = hotkeys_panel_rect_at(viewport, self.hotkeys_left_offset(viewport));
            if point_in_rect(panel, self.cursor) {
                self.request_redraw();
            }
        }
    }

    /// Угловые кнопки: тема / помощь «?» / настройки ⚙ — диспетчер по
    /// имени hit-элемента кадра.
    fn click_corner_button(&mut self, element: &str) -> bool {
        let viewport = self.viewport_logical();
        match element {
            "autolink-badge" => {
                // PRD-0007 (X4, AC-5.5): клик по бейджу — открыть ревью
                if point_in_rect(crate::autolink_ui::badge_rect(viewport), self.cursor) {
                    self.open_autolink_review();
                    return true;
                }
                false
            }
            "theme-button" => {
                if point_in_rect(
                    theme_button_rect(self.settings.button_corner, viewport),
                    self.cursor,
                ) {
                    self.toggle_theme();
                    self.request_redraw();
                    return true;
                }
                false
            }
            // FR-040 v2: кнопка переключения языка — циклический toggle
            // Ru→En→Ru с toast-подтверждением и сохранением в config.toml
            // (паттерн toggle_theme). Применение — на лету (тексты читаются
            // по кадру; шаблоны — display_name(language) в палитре/wheel).
            "language-button" => {
                if point_in_rect(
                    language_button_rect(self.settings.button_corner, viewport),
                    self.cursor,
                ) {
                    self.toggle_language();
                    self.request_redraw();
                    return true;
                }
                false
            }
            "help-button" => {
                if point_in_rect(
                    help_button_rect(self.settings.button_corner, viewport),
                    self.cursor,
                ) {
                    // Q6 FR-042: открытие оверлея закрывает main stage
                    self.close_main_stage();
                    self.help_menu = match self.help_menu.take() {
                        Some(_) => None,
                        None => {
                            let button = help_button_rect(self.settings.button_corner, viewport);
                            Some(HelpMenuState {
                                origin: docs_ui::help_menu_origin(button, viewport),
                                docs_open: false,
                            })
                        }
                    };
                    self.request_redraw();
                    return true;
                }
                false
            }
            "settings-button" => {
                if point_in_rect(
                    button_rect(self.settings.button_corner, viewport),
                    self.cursor,
                ) {
                    // Q6 FR-042: открытие настроек закрывает main stage
                    self.close_main_stage();
                    self.settings_open = !self.settings_open;
                    self.settings_dropdown.reset();
                    self.request_redraw();
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    // --- PRD-0007 (FR-048 X2): окно проверки цепочки расчёта цифры -------

    /// Закрыть окно (Esc/✕/клик по фону/ошибка сборки): готовое дерево —
    /// в сессионный кэш (AC-3.3, переход в Closed снапшот не уничтожает);
    /// подсветка цепочки гаснет фейдом (F-4, цель 0 в update_focus_state).
    fn close_explain(&mut self) {
        if let Some(mut state) = self.explain.take() {
            state.cancel_edit();
            // База забирается до move поля root (частичный move Rust)
            let base_tree = state.take_base_tree();
            if let Some(tree) = state.take_tree() {
                self.explain_cache = Some(ExplainSnapshot {
                    root: state.root,
                    revision: state.revision,
                    tree,
                    base_tree,
                });
            }
        }
        self.focus_nodes.clear();
        self.focus_edges.clear();
        self.request_redraw();
    }

    // --- PRD-0007 (FR-048 X4): автосвязь по именам (F-7) --------------------

    /// Немедленный скан детектора (AC-5.1/AC-5.5): детектор — чистая
    /// функция над канвасом; результат кэшируется до следующей ревизии.
    fn autolink_scan_now(&mut self) {
        self.autolink_proposals = canvas_core::find_proposals(&self.scene.canvas);
        self.autolink_scanned_rev = self.scene.revision;
        self.autolink_scan_due = None;
    }

    /// Открыть диалог ревью по свежему скану (команда меню AC-5.1 / клик
    /// по бейджу AC-5.5). Пустой результат — тост (честный ответ вместо
    /// пустого диалога). Stage закрывается (F-10 — взаимоисключимость).
    fn open_autolink_review(&mut self) {
        self.close_main_stage();
        self.autolink_scan_now();
        if self.autolink_proposals.is_empty() {
            self.show_toast(self.tr(keys::AUTOLINK_TOAST_NONE).to_owned());
            self.request_redraw();
            return;
        }
        let proposals = self.autolink_proposals.clone();
        self.autolink_review = Some(Review::build(&self.scene.canvas, proposals));
        self.autolink_scroll = 0.0;
        self.request_redraw();
    }

    /// Закрыть диалог (Esc/✕/создание связей): решения сеанса (отклонённые)
    /// не запоминаются — возврат отклонённых после закрытия происходит
    /// фоновой перепроверкой (AC-5.2, решение владельца У8).
    fn close_autolink_review(&mut self) {
        self.autolink_review = None;
        self.autolink_scroll = 0.0;
        self.request_redraw();
    }

    /// PRD-0007 (FR-048 X6, F-12): индикатор покрытия цепочками виден —
    /// настройка opt-in включена, поверхность канваса не перекрыта
    /// модалками (та же дисциплина, что у бейджа автосвязи — §6.5).
    fn coverage_indicator_visible(&self) -> bool {
        self.settings.explain_coverage
            && self.explain.is_none()
            && self.main_stage.is_none()
            && self.autolink_review.is_none()
            && self.dialog.is_none()
            && !self.settings_open
            && self.menu.is_none()
            && self.help_menu.is_none()
            && self.docs.is_none()
            && self.onboarding.is_none()
            && !self.scheme_gallery.open
    }

    /// PRD-0007 (FR-048 X6, F-12): процент покрытия цепочками из кэша;
    /// при смене ревизии модели — пересчёт ([`canvas_core::chain_coverage`],
    /// та же семантика деревьев, что окно проверки — F-5). `None` — цифр
    /// нет (индикатор скрыт). Стоимость не на кадр — только по ревизии.
    fn coverage_percent(&mut self) -> Option<u8> {
        if !self.settings.explain_coverage {
            return None;
        }
        let revision = self.scene.revision;
        if let Some((rev, percent)) = self.coverage_cache {
            if rev == revision {
                return percent;
            }
        }
        let whatif = self.scene.fresh_whatif_overrides();
        let stat = match flow::propagate_with_lines(&self.scene.canvas, &whatif) {
            Ok(solutions) => canvas_core::chain_coverage(
                &self.scene.canvas,
                canvas_core::LineageFlow::Ready {
                    solutions: &solutions,
                    data: &canvas_core::DataSnapshots::new(),
                },
            ),
            Err(cycle) => canvas_core::chain_coverage(
                &self.scene.canvas,
                canvas_core::LineageFlow::Cycled(&cycle),
            ),
        };
        let percent = stat.percent();
        self.coverage_cache = Some((revision, percent));
        percent
    }

    /// Видимость бейджа-индикатора (AC-5.5, «ненавязчивый»): фон включён,
    /// предложения есть, ни одного модального оверлея не открыто (клик по
    /// бейджу — открыть ревью). Диалог ревью бейдж скрывает (открыт сам).
    fn autolink_badge_visible(&self) -> bool {
        self.settings.autolink_enabled
            && !self.autolink_proposals.is_empty()
            && self.autolink_review.is_none()
            && self.explain.is_none()
            && self.main_stage.is_none()
            && self.dialog.is_none()
            && !self.settings_open
            && self.menu.is_none()
            && self.help_menu.is_none()
            && self.docs.is_none()
            && self.onboarding.is_none()
            && !self.scheme_gallery.open
            && self.wheel_menu.is_none()
            && !self.search.is_open()
            && self.editing.is_none()
    }

    /// Подсветить рёбра пачки автосвязи (AC-5.3, «подсветка отменяемого»):
    /// индексы живых рёбер по id — в focus_edges (механизм F-4).
    fn highlight_autolink_batch(&mut self, edge_ids: &[String]) {
        self.focus_edges = self
            .scene
            .canvas
            .edges
            .iter()
            .enumerate()
            .filter(|(_, edge)| edge_ids.contains(&edge.id))
            .map(|(idx, _)| idx)
            .collect();
        self.request_redraw();
    }

    /// X3 (AC-4.1/AC-4.2): применить подмену из inline-поля листа через
    /// рантайм FR-017. Канвас пересчитывается сразу (recompute_flow —
    /// живая модель, дельты на карточках показывает whatif_node_map),
    /// панель остаётся на кэшированном снапшоте — чип «Данные изменены»
    /// появляется по новой ревизии (AC-3.3). Сценарий автосоздаётся при
    /// необходимости (персистентный, один undo-шаг — паттерн
    /// finish_editing FR-017/MCP whatif_set_override).
    fn commit_explain_edit(&mut self, node_id: String, line: usize, expr: String) {
        if !self.scene.whatif_active {
            self.scene.whatif_active = true;
        }
        if self.scene.active_scenario.is_none() {
            // Подмене нужен сценарий: автосоздание (CR-016: имя — пустое,
            // scene выбирает первый свободный номер)
            let snapshot = self.scene.canvas.clone();
            match self.scene.whatif_create_scenario("") {
                Ok(index) => {
                    self.scene.active_scenario = Some(index);
                    canvas_core::whatif::scenarios_to_canvas(
                        &mut self.scene.canvas,
                        &self.scene.scenarios,
                    );
                    if self.scene.canvas != snapshot {
                        self.scene.push_undo(snapshot);
                        self.scene.mark_dirty();
                    }
                }
                Err(err) => {
                    self.show_toast(err);
                    self.request_redraw();
                    return;
                }
            }
        }
        let index = self.scene.active_scenario.expect("сценарий активен");
        if let Some(scenario) = self.scene.scenarios.get_mut(index) {
            scenario.line_exprs.insert((node_id, line), expr);
        }
        self.scene.recompute_flow();
        self.request_redraw();
    }

    /// Закрыть inline-поле с коммитом (Enter/клик мимо, AC-4.1): finish →
    /// commit; без открытого поля/без изменений — no-op.
    fn finish_explain_edit(&mut self) {
        let Some((node_id, line, text)) =
            self.explain.as_mut().and_then(|state| state.finish_edit())
        else {
            return;
        };
        self.commit_explain_edit(node_id, line, text);
    }

    /// Preset для inline-поля листа (X3): текущая подмена активного
    /// сценария (правка существующей подмены); None — исходник строки
    /// (start_edit возьмёт formula узла).
    fn explain_leaf_preset(&self, idx: usize) -> Option<String> {
        let state = self.explain.as_ref()?;
        let tree = state.tree()?;
        let node = tree.nodes.get(idx)?;
        let line = node.line?;
        let scenario = self
            .scene
            .active_scenario
            .and_then(|i| self.scene.scenarios.get(i));
        scenario
            .and_then(|s| s.line_exprs.get(&(node.node_id.clone(), line)))
            .cloned()
    }

    /// Клик при открытом окне (§6.4): ✕/чип «Данные изменены»/мета-крошки/
    /// узлы дерева; клик по фону (мимо окна) — закрытие, внутри окна мимо
    /// элементов — глотается (канвас клик не получает).
    /// Вид/лейаут/масштаб кадра окна проверки — единые для рендера и
    /// hit-тестов (детерминизм рендер/ввод). X5: в режиме защиты видимость
    /// управляется `defense_reveal` (шаги AC-6.3), масштаб — укрупнение
    /// ×1.5 с вписыванием (AC-6.2) вместо обычного fit ≤ 1.0.
    fn explain_view(
        &self,
        state: &ExplainState,
        body: [f32; 4],
    ) -> (explain_ui::Visibility, explain_ui::TreeLayout, f32) {
        let tree = state.tree().expect("Ready: дерево есть");
        let auto = if state.is_defense() {
            state.defense_reveal
        } else {
            self.settings.explain_depth_limit
        };
        let vis = explain_ui::visibility(tree, state.view_root(), auto, &state.expanded);
        let layout = explain_ui::layout_tree(tree, &vis, state.view_root());
        let scale = if state.is_defense() {
            explain_ui::defense_fit_scale(layout.bounds, body)
        } else {
            explain_ui::fit_scale(layout.bounds, body)
        };
        (vis, layout, scale)
    }

    /// Полоса результата D под world-точкой (F-1/AC-1.1): Some — корень
    /// explain-дерева (итог ноды, AC-1.4: константа — панель одного узла).
    /// У ноды без вычисленного результата зоны нет (триггер не глушит
    /// редактирование прозы). Геометрия — как в рендере (text.rs):
    /// нижняя полоса карточки высотой RESULT_LINE_HEIGHT.
    fn result_band_root_at(&self, world: Vec2) -> Option<LineageNodeId> {
        let index = self.hovered?;
        let node = self.scene.canvas.nodes.get(index)?;
        match self.scene.expr_results.get(&node.id) {
            Some(ExprOutcome::Ok(_)) => {}
            _ => return None,
        }
        let band = [
            node.x,
            node.y + node.height - BODY_PADDING - RESULT_LINE_HEIGHT,
            node.width,
            RESULT_LINE_HEIGHT,
        ];
        point_in_rect(band, world).then(|| LineageNodeId::total(node.id.clone()))
    }

    /// Системное контекстное меню десктопа (T17, план §3): нативное
    /// Win32-меню через TrackPopupMenu(TPM_RETURNCMD) — команда приходит
    /// return'ом, воронка WM_COMMAND не строится (отступление §8.1).
    /// Меню T7 (цвета нод) не затрагивается — зоны не пересекаются
    /// (§8.2). Отказы всех Win32-шагов — warn + деградация (R14).
    #[cfg(windows)]
    fn desktop_menu(&mut self, event_loop: &ActiveEventLoop) {
        use canvas_shell::desktop::menu::DesktopMenuCommand as Cmd;
        let Some(raw) = self.window_hwnd() else {
            return;
        };
        let hwnd = Self::hwnd(raw);
        // Галочки меню: иконки (сейчас скрыты — инверсная семантика
        // пункта «Показать») и автозапуск (факт реестра HKCU Run)
        let icons_hidden = self
            .icon_guard
            .as_ref()
            .is_some_and(|guard| guard.hidden_by_us());
        let autostart_on = canvas_shell::desktop::interop::autostart_enabled();
        let Some(command) = canvas_shell::desktop::menu::popup(hwnd, icons_hidden, autostart_on)
        else {
            return; // отмена — клик мимо/Esc
        };
        match command {
            // Второй экземпляр в оконном режиме с текущим канвасом
            // (план §8.3): редактирование не ломает десктоп-встройку
            Cmd::OpenCanvas => {
                canvas_shell::desktop::interop::spawn_window_instance(&self.scene.path);
            }
            // Файл в каталоге канваса + нода в точке ПКМ (план §8.4):
            // вотчер T10/поиск T14 подхватят автоматически
            Cmd::NewTextFile => self.create_text_file_node(),
            // Toggle иконок: show — «показать» независимо от того, кто
            // скрывал (ПКМ Explorer в --desktop перехвачен канвасом)
            Cmd::ToggleIcons => {
                if let Some(guard) = self.icon_guard.as_mut() {
                    if guard.hidden_by_us() {
                        guard.show();
                    } else {
                        guard.hide();
                    }
                }
            }
            // Toggle автозапуска (HKCU Run) — галочка перечитается при
            // следующем открытии меню
            Cmd::ToggleAutostart => {
                if let Err(err) = canvas_shell::desktop::interop::set_autostart(!autostart_on) {
                    tracing::warn!(%err, "не удалось переключить автозапуск");
                }
            }
            // Штатный выход — единая точка с CloseRequested
            Cmd::Exit => self.shutdown(event_loop),
        }
    }

    /// «Новый текстовый файл» из десктоп-меню (T17, план §8.4): файл в
    /// каталоге канваса (уникальное имя) + file-нода в позиции ПКМ —
    /// образец вставки дропа T9; вотчер T10 и поиск T14 подхватят
    /// автоматически.
    #[cfg(windows)]
    fn create_text_file_node(&mut self) {
        let world = self.cursor_world();
        let dir = self.scene.canvas_dir();
        // Уникальное имя: «Новая заметка.txt», при коллизии — « 2», « 3»…
        let base = self.tr(keys::FILE_NEW_NOTE);
        let mut name = format!("{base}.txt");
        let mut counter = 1u32;
        while dir.join(&name).exists() {
            counter += 1;
            name = format!("{base} {counter}.txt");
        }
        let path = dir.join(&name);
        if let Err(err) = std::fs::write(&path, "") {
            tracing::warn!(%err, path = %path.display(), "не удалось создать файл");
            return;
        }
        // FR-006: файловая нода из десктоп-меню — undo-шаг (снапшот до —
        // файл на диске остаётся, откатывается только карточка)
        self.push_undo();
        let id = next_free_id(&self.scene.canvas, "file");
        let node = Node::file(
            id,
            &name,
            world[0],
            world[1],
            crate::ui::DROP_CARD_W,
            crate::ui::DROP_CARD_H,
        );
        self.scene.canvas.nodes.push(node);
        let index = self.scene.canvas.nodes.len() - 1;
        let node_ref = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node_ref);
        self.selected = Some(Selection::Node(index));
        self.scene.mark_dirty();
        // Поисковый индекс (T14): новая нода — сразу в FTS
        self.search_service.command(SearchCommand::IndexFile {
            path: path.clone(),
            display_name: name,
        });
        // Каталог канваса мог не быть под слежкой (первая file-нода) —
        // синхронизируем вотчер и SHCNE-подписки (T17-E единая точка)
        self.sync_watch_dirs();
        tracing::info!(path = %path.display(), "создан текстовый файл + нода");
    }

    /// Штатный выход (T17): форс-сейв + восстановление системных иконок
    /// (R5) + завершение — единая точка для CloseRequested и пункта
    /// меню «Выход»; Drop-страховка guard'а остаётся на паниках, sentinel
    /// — на kill -9.
    fn shutdown(&mut self, event_loop: &ActiveEventLoop) {
        if self.scene.dirty_since.is_some() {
            self.scene.save_now();
        }
        #[cfg(windows)]
        if let Some(guard) = self.icon_guard.as_mut() {
            guard.restore();
        }
        event_loop.exit();
    }
}

/// Аргументы командной строки: `canvasdesk [--stress N] [path]`.
pub struct CliArgs {
    /// Нагрузочный режим (T5): сцена из N случайных нод вместо загрузки файла.
    pub stress: Option<usize>,
    /// M5 (T20): добавить N виджет-нод (встроенные часы) в открытую сцену —
    /// нагрузочная приёмка «10 виджетов не роняют fps» (SPEC §10 M5).
    pub stress_widgets: Option<usize>,
    /// Режим десктопа (T15, SPEC §7.4): встройка канваса в WorkerW за
    /// иконками рабочего стола. На не-Windows — warn и оконный режим.
    pub desktop: bool,
    pub path: PathBuf,
}

/// FR-040 v2 (web persist): человекочитаемая строка из `wasm_bindgen::JsValue`
/// для `tracing::warn` — `JsValue` не реализует `Display` (трейт запрещён
/// политикой wasm-bindgen: значение может быть любым JS-типом). Берём debug-
/// представление (`JsValue::debug_string`) — короткое строковое описание.
#[cfg(target_arch = "wasm32")]
fn wasm_js_err_display(err: &wasm_bindgen::JsValue) -> String {
    format!("{err:?}")
}

/// Разбор аргументов вручную — две опции не оправдывают зависимость от clap.
pub fn parse_args(args: &[String]) -> anyhow::Result<CliArgs> {
    let mut stress = None;
    let mut stress_widgets = None;
    let mut desktop = false;
    let mut path = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--stress" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow::anyhow!("--stress требует число нод"))?;
            stress = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("--stress: не число: {value}"))?,
            );
        } else if let Some(value) = arg.strip_prefix("--stress=") {
            stress = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("--stress: не число: {value}"))?,
            );
        } else if arg == "--stress-widgets" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow::anyhow!("--stress-widgets требует число виджетов"))?;
            stress_widgets = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("--stress-widgets: не число: {value}"))?,
            );
        } else if let Some(value) = arg.strip_prefix("--stress-widgets=") {
            stress_widgets = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| anyhow::anyhow!("--stress-widgets: не число: {value}"))?,
            );
        } else if arg == "--desktop" {
            // Булев флаг: повтор допустим (идемпотентен)
            desktop = true;
        } else if arg == "--help" || arg == "-h" {
            println!(
                "Использование: canvasdesk [mcp [--no-spawn]] [--stress N] [--stress-widgets N] [--desktop] [путь к .canvas]\n\
                 \x20 mcp — режим MCP-посредника (stdio; автостарт сервиса, --no-spawn — отключить)\n\
                 \x20 --stress-widgets N — добавить N виджет-нод (нагрузочная приёмка M5)"
            );
            std::process::exit(0);
        } else if path.is_none() {
            path = Some(PathBuf::from(arg));
        } else {
            anyhow::bail!("лишний аргумент: {arg}");
        }
    }
    // В stress-режиме по умолчанию пишем в stress.canvas, чтобы не затирать default.canvas
    let default_path = if stress.is_some() {
        "stress.canvas"
    } else {
        "default.canvas"
    };
    Ok(CliArgs {
        stress,
        stress_widgets,
        desktop,
        path: path.unwrap_or_else(|| PathBuf::from(default_path)),
    })
}

/// Открыть файл/путь в системном приложении (T21-A: openFile моста,
/// permission shell:open). Windows — тот же ShellExecuteEx-путь, что у
/// файловых нод (interop::open_file); Linux/macOS — xdg-open/open
/// (M7: мост виджетов кроссплатформенен, host появится на T22+).
fn open_path_externally(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        canvas_shell::desktop::interop::open_file(path)
    }
    #[cfg(not(windows))]
    {
        let program = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(program)
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("{program}: {e}"))
    }
}

/// Детерминированный PRNG (xorshift32) — генератор стресс-сцены без зависимостей.
struct Xorshift(u32);

impl Xorshift {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    /// Случайное f32 в [0, 1).
    fn unit(&mut self) -> f32 {
        (self.next() % 10_000) as f32 / 10_000.0
    }
}

/// Слова для правдоподобных заголовков стресс-нод.
const STRESS_WORDS: [&str; 8] = [
    "отчёт",
    "смета",
    "презентация",
    "договор",
    "спецификация",
    "заметка",
    "план",
    "архив",
];

/// Нагрузочная сцена (T5): N текстовых нод со случайными rect/цветом/заголовком,
/// раскиданных по области, растущей как sqrt(N) — плотность стабильна.
/// Детерминирована: один и тот же N даёт одну и ту же сцену.
/// M5 (T20-F): добавить N виджет-нод (встроенные часы) детерминированной
/// сеткой — нагрузочная приёмка SPEC §10 («10 виджетов не роняют fps»).
/// Id — `widget-N` по порядку; возвращается число добавленных.
pub fn add_stress_widgets(canvas: &mut Canvas, n: usize) -> usize {
    let mut existing = 0u32;
    for node in &canvas.nodes {
        if let Some(tail) = node.id.strip_prefix("widget-") {
            if let Ok(k) = tail.parse::<u32>() {
                existing = existing.max(k);
            }
        }
    }
    let cols = 5;
    for i in 0..n {
        let col = (i % cols) as f32;
        let row = (i / cols) as f32;
        let ext = canvas_core::CanvasdeskExt {
            widget_id: Some("com.canvasdesk.clock".to_owned()),
            props: serde_json::Map::new(),
            expr: None,
            template: None,
            desc: None,
            data: None,
            title: None,
        };
        canvas.nodes.push(Node::widget(
            format!("widget-{}", existing + i as u32 + 1),
            ext,
            "Clock",
            400.0 + col * 360.0,
            400.0 + row * 260.0,
            320.0,
            200.0,
        ));
    }
    n
}

pub fn stress_canvas(n: usize) -> Canvas {
    let mut canvas = Canvas::default();
    let mut rng = Xorshift(0x9E37_79B9);
    let extent = (n.max(1) as f32).sqrt() * 400.0;
    for i in 0..n {
        let x = rng.unit() * extent * 2.0 - extent;
        let y = rng.unit() * extent * 2.0 - extent;
        let width = 120.0 + rng.unit() * 300.0;
        let height = 80.0 + rng.unit() * 220.0;
        let word = STRESS_WORDS[i % STRESS_WORDS.len()];
        let mut node = Node::text(
            format!("stress-{i}"),
            format!("{word} #{i}\nнагрузочный тест"),
            x,
            y,
        );
        node.width = width;
        node.height = height;
        if rng.unit() < 0.3 {
            node.color = Some((1 + rng.next() % 6).to_string());
        }
        canvas.nodes.push(node);
    }
    canvas
}

impl App {
    /// M5 (T20-F): стартовая инициализация виджетов (реестр + встроенные).
    /// Отдельно от App::new — после настройки трейсинга в main().
    pub fn init_widgets(&mut self) {
        self.widgets.set_theme(self.settings.theme == Theme::Dark);
        self.widgets.init_registry();
    }

    /// M5: события host'а виджетов (из user_event).
    fn on_widget_event(&mut self, event: canvas_widgets::WidgetEvent) {
        self.widgets.on_event(&event);
        match event {
            canvas_widgets::WidgetEvent::SnapshotReady { node_id, snapshot } => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_widget_snapshot(
                        &node_id,
                        snapshot.width,
                        snapshot.height,
                        &snapshot.rgba,
                    );
                }
                self.request_redraw();
            }
            canvas_widgets::WidgetEvent::Message {
                node_id,
                message,
                id,
            } => {
                self.on_widget_message(&node_id, message, id);
            }
            canvas_widgets::WidgetEvent::EnvironmentReady { ok } => {
                tracing::info!(ok, "виджеты: {}", self.widgets.runtime_status());
                self.request_redraw();
            }
            canvas_widgets::WidgetEvent::ControllerReady { .. } => {
                self.request_redraw();
            }
            canvas_widgets::WidgetEvent::Tick => {
                // refresh-расписание вычисляется в update_frame по времени;
                // тик только будит цикл
            }
        }
    }

    /// M5 (T21-A): сообщения моста — enforcement на каждый вызов.
    /// Разрешения берутся из манифеста ПАКЕТА ноды (не из сообщения!),
    /// отказ — warn + JSON-RPC error (для запросов с id). `id`
    /// передаётся из host'а для ответа на запросы readDir/state*.
    fn on_widget_message(
        &mut self,
        node_id: &str,
        message: canvas_widgets::WidgetToHost,
        id: Option<serde_json::Value>,
    ) {
        use canvas_widgets::WidgetToHost;
        // Enforcement (П-таблица §4.6): без permission — отказ + лог.
        // Пакет мог исчезнуть (удалён) — тоже отказ, не паника.
        let permissions = match self
            .widgets
            .permissions_of_node(&self.scene.canvas, node_id)
        {
            Some(p) => p,
            None => {
                tracing::warn!(
                    node_id,
                    msg = message.method(),
                    "мост: пакет ноды не установлен"
                );
                self.reply_err(node_id, id, "пакет виджета не установлен");
                return;
            }
        };
        if let Err(required) = permissions.check_call(&message) {
            tracing::warn!(
                node_id,
                method = message.method(),
                required = required.as_str(),
                "мост: вызов заблокирован — нет permission"
            );
            self.reply_err(
                node_id,
                id,
                format!("нет permission: {}", required.as_str()),
            );
            return;
        }
        match message {
            WidgetToHost::Ready => {
                let init = self
                    .scene
                    .canvas
                    .node(node_id)
                    .and_then(|node| self.widgets.init_message(node));
                if let Some(init) = init {
                    self.widgets.post_message(node_id, &init);
                }
            }
            WidgetToHost::SetProps { props } => {
                // Undo-шаг до мутации (коалесценция — риски M5 §8)
                if self.widgets.should_push_props_undo(node_id) {
                    self.push_undo();
                }
                let index = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .position(|node| node.id == node_id);
                let changed = index
                    .and_then(|i| self.scene.canvas.nodes[i].canvasdesk.as_mut())
                    .map(|ext| {
                        let changed = ext.props != props;
                        ext.props = props.clone();
                        changed
                    })
                    .unwrap_or(false);
                if changed {
                    self.scene.mark_dirty();
                    // Подтверждение виджету (propsChanged) — замкнутый цикл
                    // без эхо-повтора: повторный setProps тех же props не
                    // меняет модель (changed=false).
                    self.widgets.post_message(
                        node_id,
                        &canvas_widgets::HostToWidget::PropsChanged { props },
                    );
                }
            }
            WidgetToHost::OpenFile { path } => {
                // Путь — как дала нода/канвас: резолв от корня канваса,
                // произвольные системные пути виджету недоступны (П4-дух).
                let resolved = self.scene.canvas_dir().join(path.trim_end_matches('/'));
                if let Err(e) = open_path_externally(&resolved) {
                    tracing::warn!(node_id, path = %resolved.display(), error = %e, "openFile не удался");
                    self.show_toast(self.trf(
                        keys::TOAST_WIDGET_OPEN_FAILED,
                        &[("{path}", &resolved.display().to_string())],
                    ));
                }
            }
            WidgetToHost::ReadDir { path } => {
                // Allowlist П4: папки файловых нод + корень канваса
                let canvas_dir = self.scene.canvas_dir();
                let roots = canvas_core::watched_dirs(&self.scene.canvas, &canvas_dir);
                match canvas_widgets::permissions::resolve_fs_request(&path, &canvas_dir, &roots)
                    .and_then(|dir| canvas_widgets::bridge::read_dir_entries(&dir))
                {
                    Ok(entries) => self.widgets.reply(
                        node_id,
                        &canvas_widgets::bridge::Reply::ok(
                            id.clone().unwrap_or(serde_json::Value::Null),
                            entries,
                        ),
                    ),
                    Err(e) => {
                        tracing::warn!(node_id, path, error = %e, "readDir отказан");
                        self.reply_err(node_id, id, e);
                    }
                }
            }
            WidgetToHost::Toast { text } => {
                tracing::info!(node_id, %text, "widget toast");
                self.show_toast(text);
            }
            WidgetToHost::Resize { w, h } => {
                // Кламп манифестных границ (160..2000, SPEC §7.6)
                let w = w.clamp(160.0, 2000.0);
                let h = h.clamp(160.0, 2000.0);
                let index = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .position(|node| node.id == node_id);
                if let Some(node) = index.map(|i| &mut self.scene.canvas.nodes[i]) {
                    if node.width != w || node.height != h {
                        node.width = w;
                        node.height = h;
                        // Геометрия изменилась — обновляем пространственный индекс
                        // пересборкой (как при ручном resize)
                        self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
                        self.scene.mark_dirty();
                    }
                }
            }
            WidgetToHost::StateGet { key } => {
                let value = self.widgets.state_get(node_id, &key);
                let result = serde_json::json!({ "value": value });
                self.widgets.reply(
                    node_id,
                    &canvas_widgets::bridge::Reply::ok(
                        id.clone().unwrap_or(serde_json::Value::Null),
                        result,
                    ),
                );
            }
            WidgetToHost::StateSet { key, value } => {
                self.widgets.state_set(node_id, &key, &value);
                self.widgets.reply(
                    node_id,
                    &canvas_widgets::bridge::Reply::ok(
                        id.clone().unwrap_or(serde_json::Value::Null),
                        serde_json::json!({ "ok": true }),
                    ),
                );
            }
        }
    }

    /// Ответ-ошибка на запрос моста (T21-A): уведомления без id — только warn.
    fn reply_err(
        &mut self,
        node_id: &str,
        id: Option<serde_json::Value>,
        message: impl Into<String>,
    ) {
        if let Some(id) = id {
            self.widgets
                .reply(node_id, &canvas_widgets::bridge::Reply::err(id, message));
        }
    }

    /// Toast (T21-A): строка внизу центра на 3 с + перерисовка.
    fn show_toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
        self.request_redraw();
    }

    /// Rect модального диалога (screen-space, логические px): центр окна.
    ///
    /// FR-060 (волна 2 кита): `kit::modal` + измеренный текст — панель
    /// адаптируется под заголовок/тело/подписи кнопок (фикс класса дефекта
    /// «фиксированные геометрии», PRD-0009 §2: прежние 440×150 не зависели
    /// от текста; 150 содержало ~17 px мёртвого слэка). Пады/якоря прежней
    /// раскладки дословно — см. [`App::dialog_measured`].
    fn dialog_rect(&self) -> [f32; 4] {
        self.dialog_measured().0
    }

    /// Rect кнопок диалога: [Да][Нет] внизу панели (индексы как в buttons()).
    ///
    /// FR-060: ширины кнопок — от измеренного текста (`kit::button_size`:
    /// текст + 2·[`kit::BUTTON_PAD_H`], пол [`kit::BUTTON_HEIGHT`] = 30 —
    /// прежняя высота дословно); прежние 110 были запасом под самую
    /// длинную подпись. Пара — по центру с прежним зазором 16.
    fn dialog_button_rects(&self) -> [[f32; 4]; 2] {
        self.dialog_measured().1
    }

    /// Измеренная раскладка диалога (FR-060): (панель, кнопки, показанные
    /// тексты [заголовок, тело]). Чистая функция замера —
    /// [`App::dialog_measured_in`]; `None` у диалога — прежние константы
    /// 440×150 (вызовы без открытого диалога — airspace-гейт).
    fn dialog_measured(&self) -> ([f32; 4], [[f32; 4]; 2], [String; 2]) {
        let Some(dialog) = &self.dialog else {
            let viewport = self.viewport_logical();
            let w = 440.0_f32.min(viewport[0] - 40.0).max(280.0);
            let h = 150.0;
            let rect = [(viewport[0] - w) / 2.0, (viewport[1] - h) / 2.0, w, h];
            return (rect, [[0.0; 4]; 2], [String::new(), String::new()]);
        };
        let buttons: [String; 2] = dialog
            .buttons(self.settings.language)
            .map(|(label, _)| label.to_owned());
        let mut m = canvas_ui::measure::TextMeasurer::new();
        let mut fs = canvas_render::text::measure_font_system();
        Self::dialog_measured_in(
            self.viewport_logical(),
            &dialog.title(&self.scene.canvas, self.settings.language),
            &dialog.body(self.settings.language),
            &buttons,
            &mut m,
            &mut fs,
        )
    }

    /// Тело [`App::dialog_measured`] — чистая функция от текстов и вьюпорта
    /// (headless-тесты). Геометрия: ширина — от замера заголовка/тела
    /// (пол/потолок/маржа прежние 280/440/40; переполнение потолка —
    /// `ellipsis`, видимая деградация вместо молчаливого клипа — G5/G8);
    /// высота — пады прежней раскладки дословно (верх 16, тело с 46,
    /// высота кнопок 30, низ 16) + измеренная строка тела + зазор
    /// `SPACING_LG` (12); панель — `kit::modal` (центр вьюпорта);
    /// кнопки — `kit::button_size`, пара по центру, зазор 16 (прежний).
    fn dialog_measured_in(
        viewport: [f32; 2],
        title: &str,
        body: &str,
        buttons: &[String; 2],
        m: &mut canvas_ui::measure::TextMeasurer,
        fs: &mut cosmic_text::FontSystem,
    ) -> ([f32; 4], [[f32; 4]; 2], [String; 2]) {
        const FAMILY: &str = canvas_render::text::SANS_FAMILY;
        let title_w = m.width_of(fs, title, FAMILY, DIALOG_TITLE_FS);
        let body_w = m.width_of(fs, body, FAMILY, DIALOG_BODY_FS);
        // Ширина: от измеренного текста; пол/потолок/кламп к вьюпорту прежние
        let w = (title_w.max(body_w) + DIALOG_PAD_X * 2.0)
            .clamp(DIALOG_MIN_W, DIALOG_MAX_W)
            .min((viewport[0] - DIALOG_VIEWPORT_MARGIN).max(DIALOG_MIN_W));
        // Деградация длинных текстов — ellipsis по фактической ширине
        // (прежний рендер молча клиповал — G5/G8: деградация видима)
        let avail = w - DIALOG_PAD_X * 2.0;
        let title_shown = m.ellipsis(fs, title, FAMILY, DIALOG_TITLE_FS, avail);
        let body_shown = m.ellipsis(fs, body, FAMILY, DIALOG_BODY_FS, avail);
        // Высота: якоря прежней раскладки + измеренная строка тела
        let body_h = m
            .measure(
                fs,
                &canvas_ui::measure::TextSpec {
                    text: body,
                    family: FAMILY,
                    size: DIALOG_BODY_FS,
                    max_width: f32::INFINITY,
                    // Диалоги — sans (паритет sans_attrs, вес 500).
                    weight: cosmic_text::Weight::MEDIUM,
                },
            )
            .height;
        let h = DIALOG_BODY_Y
            + body_h
            + canvas_core::tokens::SPACING_LG
            + DIALOG_BTN_H
            + DIALOG_BTN_BOTTOM;
        // Панель — kit::modal (центр вьюпорта; min=max=измеренный размер)
        let slot = canvas_ui::geometry::UiRect::new(0.0, 0.0, viewport[0], viewport[1]);
        let size = canvas_ui::geometry::UiVec2::new(w, h);
        let panel = kit::modal(slot, size, size, size).panel;
        // Кнопки: ширина от измеренного текста; пара по центру, зазор 16
        let gap = DIALOG_BTN_GAP;
        let bw0 = kit::button_size(&buttons[0], m, fs, FAMILY, DIALOG_BTN_FS).x;
        let bw1 = kit::button_size(&buttons[1], m, fs, FAMILY, DIALOG_BTN_FS).x;
        let total = bw0 + gap + bw1;
        let start = panel.x + ((panel.w - total).max(0.0)) / 2.0;
        let by = panel.y + panel.h - DIALOG_BTN_H - DIALOG_BTN_BOTTOM;
        (
            [panel.x, panel.y, panel.w, panel.h],
            [
                [start, by, bw0, DIALOG_BTN_H],
                [start + bw0 + gap, by, bw1, DIALOG_BTN_H],
            ],
            [title_shown, body_shown],
        )
    }

    /// Отмена диалога (Esc/клик «Нет»): ничего не меняется. Откат пачки
    /// отменён — подсветка отменяемого гаснет (AC-5.3).
    fn cancel_dialog(&mut self) {
        if matches!(self.dialog, Some(AppDialog::AutolinkRollback { .. })) {
            self.autolink_batch = None;
            self.focus_edges.clear();
        }
        self.dialog = None;
        self.request_redraw();
    }

    /// FR-014: создать связь заданного типа потока (общий путь drop
    /// резиновой линии и подтверждения диалога цикла). Undo-шаг (FR-006),
    /// mark_dirty + живой пересчёт потока: value-ребро сразу переносит
    /// значение в downstream. FR-025: `from_line` — построчный исток
    /// (Some(i) — значение строки i источника; None — значение ноды).
    fn create_edge(
        &mut self,
        from_node: String,
        from_side: Side,
        to_node: String,
        to_side: Side,
        kind: FlowKind,
        from_line: Option<usize>,
    ) {
        let mut edge = Edge::new(
            self.scene.canvas.next_edge_id(),
            from_node,
            Some(from_side),
            to_node,
            Some(to_side),
        );
        edge.set_flow_kind(kind);
        edge.from_line = from_line;
        self.push_undo();
        self.scene.canvas.add_edge(edge);
        self.scene.mark_dirty();
        self.scene.recompute_flow();
        self.request_redraw();
    }

    /// M5: установка виджет-ноды из подменю (центр viewport, defaultSize).
    fn insert_widget_from_menu(&mut self, widget_id: &str) {
        let center = self.viewport_center_world();
        let id = self.widgets.next_node_id(&self.scene.canvas);
        let Some(node) = self.widgets.build_widget_node(widget_id, id, center) else {
            tracing::warn!(widget_id, "пакет виджета не найден");
            return;
        };
        self.insert_nodes(vec![node], true);
    }

    // --- FR-010: авто-раскладка связанных карточек ---

    /// Применить план авто-раскладки (FR-010) от ноды-семени. Один undo-шаг
    /// (FR-006); ноды без связи с семенем не трогаются; spatial index
    /// обновляется точечно (паттерн drag группы).
    fn apply_related_layout(&mut self, seed: usize, mode: canvas_core::LayoutMode) {
        let plan = canvas_core::plan_related_layout(&self.scene.canvas, seed, mode);
        if plan.is_empty() {
            self.show_toast(self.tr(keys::TOAST_NO_RELATED_CARDS));
            return;
        }
        // FR-006: раскладка — один undo-шаг
        self.push_undo();
        for (index, [x, y]) in plan {
            if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                node.x = x;
                node.y = y;
            }
            if let Some(node) = self.scene.canvas.nodes.get(index) {
                self.scene.spatial.update(index, node);
            }
        }
        self.scene.mark_dirty();
        self.show_toast(self.tr(keys::TOAST_RELATED_ALIGNED));
    }

    // --- FR-011: mindmap (Tab / Enter / сворачивание ветки) ---

    /// Прямые дети ноды по исходящим рёбрам (в порядке рёбер модели).
    fn mindmap_direct_children(canvas: &Canvas, parent_index: usize) -> Vec<usize> {
        let Some(parent) = canvas.nodes.get(parent_index) else {
            return Vec::new();
        };
        let parent_id = parent.id.as_str();
        let index_of: std::collections::HashMap<&str, usize> = canvas
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        canvas
            .edges
            .iter()
            .filter(|edge| edge.from_node == parent_id)
            .filter_map(|edge| index_of.get(edge.to_node.as_str()).copied())
            .filter(|&i| i != parent_index)
            .collect()
    }

    /// Создать дочернюю ветку (FR-011, Tab): text-нода правее родителя
    /// (под существующими детьми) + ребро родитель→новая + вход в
    /// редактирование. Один undo-шаг (нода + ребро + разворот свёрнутого).
    fn mindmap_add_child(&mut self, parent_index: usize) {
        let canvas = &self.scene.canvas;
        let Some(parent) = canvas.nodes.get(parent_index) else {
            return;
        };
        let parent_id = parent.id.clone();
        let x = parent.x + parent.width + canvas_core::LEVEL_GAP;
        let mut y_bottom: Option<f32> = None;
        for child in Self::mindmap_direct_children(canvas, parent_index) {
            if let Some(node) = canvas.nodes.get(child) {
                y_bottom =
                    Some(y_bottom.map_or(node.y + node.height, |b| b.max(node.y + node.height)));
            }
        }
        let y = match y_bottom {
            Some(bottom) => bottom + canvas_core::SIBLING_GAP,
            None => parent.y,
        };
        // Один undo-шаг на всю операцию (нода + ребро + возможный разворот)
        self.push_undo();
        let id = next_free_id(&self.scene.canvas, "note");
        self.scene.canvas.nodes.push(Node::text(id, "", x, y));
        let index = self.scene.canvas.nodes.len() - 1;
        let node = &self.scene.canvas.nodes[index];
        self.scene.spatial.insert(index, node);
        let edge = Edge::new(
            self.scene.canvas.next_edge_id(),
            parent_id,
            Some(Side::Right),
            self.scene.canvas.nodes[index].id.clone(),
            Some(Side::Left),
        );
        self.scene.canvas.add_edge(edge);
        // Свёрнутая ветка разворачивается: новая нода должна быть видна
        if let Some(parent) = self.scene.canvas.nodes.get_mut(parent_index) {
            if parent.collapsed == Some(true) {
                parent.collapsed = None;
            }
        }
        self.scene.mark_dirty();
        self.begin_editing(index);
    }

    /// Создать сиблинга (FR-011, Enter): та же родительская нода, что у
    /// текущей. У корня (нет входящих рёбер) — no-op (зафиксировано в
    /// FR-011). Один undo-шаг.
    fn mindmap_add_sibling(&mut self, node_index: usize) {
        let Some(parent) = canvas_core::parent_index(&self.scene.canvas, node_index) else {
            self.show_toast(self.tr(keys::TOAST_NO_LEVEL));
            return;
        };
        self.mindmap_add_child(parent);
    }

    /// Свернуть/развернуть ветку (FR-011): флаг `collapsed` ноды; поддерево
    /// скрывается из рендера/hit-test/миникарты/поиска (см.
    /// `hidden_subtree_nodes`). Undo-шаг — как изменение модели.
    fn mindmap_set_collapsed(&mut self, node_index: usize, collapsed: bool) {
        let Some(node) = self.scene.canvas.nodes.get(node_index) else {
            return;
        };
        if node.kind() != NodeKind::Text
            || node.collapsed == Some(collapsed)
            || canvas_core::subtree_ids(&self.scene.canvas, node_index).is_empty()
        {
            return; // нет детей — сворачивать нечего
        }
        self.push_undo();
        if let Some(node) = self.scene.canvas.nodes.get_mut(node_index) {
            node.collapsed = Some(collapsed);
        }
        self.scene.mark_dirty();
        self.show_toast(if collapsed {
            self.tr(keys::TOAST_BRANCH_COLLAPSED)
        } else {
            self.tr(keys::TOAST_BRANCH_EXPANDED)
        });
    }

    /// Индексы скрытых нод (свернутые поддеревья, FR-011): объединение
    /// поддеревьев всех нод с collapsed = Some(true); отсортирован —
    /// binary_search в горячих путях.
    fn hidden_subtree_nodes(&self) -> Vec<usize> {
        let mut hidden: Vec<usize> = Vec::new();
        for (index, node) in self.scene.canvas.nodes.iter().enumerate() {
            if node.collapsed == Some(true) {
                for id in canvas_core::subtree_ids(&self.scene.canvas, index) {
                    if !hidden.contains(&id) {
                        hidden.push(id);
                    }
                }
            }
        }
        hidden.sort_unstable();
        hidden.dedup();
        hidden
    }

    // --- FR-009: диспетчер «Настройки ▸» ---

    // --- FR-012: жест «втягивания» в группу ---

    /// Цель втягивания при активном drag: верхняя группа (макс. индекс),
    /// чей rect содержит центр перетаскиваемой первичной ноды, при условии,
    /// что нода ещё НЕ ребёнок этой группы (иначе жест бессмысленен).
    fn group_drop_target(&self, dragging: &DragState) -> Option<usize> {
        let node = self.scene.canvas.nodes.get(dragging.primary)?;
        let center = [node.x + node.width / 2.0, node.y + node.height / 2.0];
        let dragged: Vec<usize> = std::iter::once(dragging.primary)
            .chain(dragging.origins.iter().map(|(i, _)| *i))
            .collect();
        let candidates = self
            .scene
            .spatial
            .query_rect([center[0], center[1], center[0], center[1]]);
        candidates
            .into_iter()
            .rev() // верхняя по z — последняя
            .find(|&index| {
                self.scene.canvas.nodes.get(index).is_some_and(|group| {
                    group.kind() == NodeKind::Group
                        && !dragged.contains(&index)
                        && center[0] >= group.x
                        && center[0] <= group.x + group.width
                        && center[1] >= group.y
                        && center[1] <= group.y + group.height
                        // уже ребёнок (явный список) — не «втягиваем» повторно
                        && !group
                            .children
                            .as_ref()
                            .is_some_and(|list| {
                                self.scene.canvas.nodes.get(dragging.primary).is_some_and(|n| {
                                    list.contains(&n.id)
                                })
                            })
                })
            })
    }

    /// Вставить перетаскиваемые ноды в группу (FR-012, отпускание над
    /// зоной): membership + авторасширение rect до bbox+padding + мягкое
    /// раздвигание пересекаемых соседей (с анимацией). Один undo-шаг.
    fn group_insert_dragged(&mut self, group_index: usize) {
        let Some(drag) = self.dragging.as_ref() else {
            return;
        };
        // Вставляются: первичная нода + весь drag-набор (CR-001), кроме
        // самой группы-цели. Группа в наборе — вставляется ТОЛЬКО она
        // (вложенная группа едет как нода; её дети — через translate_group)
        let primary_is_group = self
            .scene
            .canvas
            .nodes
            .get(drag.primary)
            .is_some_and(|n| n.kind() == NodeKind::Group);
        let mut ids: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let dragged: Vec<usize> = if primary_is_group {
            vec![drag.primary]
        } else {
            std::iter::once(drag.primary)
                .chain(drag.origins.iter().map(|(i, _)| *i))
                .collect()
        };
        for index in dragged {
            if index == group_index || !seen.insert(index) {
                continue;
            }
            if let Some(node) = self.scene.canvas.nodes.get(index) {
                ids.push(node.id.clone());
            }
        }
        if ids.is_empty() {
            return;
        }
        self.push_undo();
        canvas_core::group_add_children(&mut self.scene.canvas, group_index, &ids);
        // Авторасширение: rect группы = bbox(дети) + GROUP_PADDING
        let old = self
            .scene
            .canvas
            .nodes
            .get(group_index)
            .map(|g| [g.x, g.y])
            .unwrap_or([0.0, 0.0]);
        canvas_core::group_expand_to_children(
            &mut self.scene.canvas,
            group_index,
            crate::ui::GROUP_PADDING,
        );
        let new_rect = self
            .scene
            .canvas
            .nodes
            .get(group_index)
            .map(|g| [g.x, g.y, g.width, g.height])
            .unwrap_or([0.0, 0.0, 0.0, 0.0]);
        self.scene
            .spatial
            .update(group_index, &self.scene.canvas.nodes[group_index]);
        // Мягкое раздвигание: не-дети, чьи bbox пересеклись с новым rect,
        // сдвигаются на минимальный осевой вектор; группа и раздвинутые
        // соседи едут плавно (settle-анимация ~250 мс)
        let children: Vec<usize> = canvas_core::group_children(&self.scene.canvas, group_index);
        let others: Vec<(usize, [f32; 4])> = self
            .scene
            .canvas
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, node)| {
                *i != group_index && !children.contains(i) && node.kind() != NodeKind::Group
            })
            .map(|(i, node)| (i, [node.x, node.y, node.width, node.height]))
            .collect();
        let push_plan = canvas_core::plan_push_out(new_rect, &others);
        let mut moves: Vec<(usize, [f32; 2], [f32; 2])> = Vec::new();
        let new_pos = [new_rect[0], new_rect[1]];
        if (new_pos[0] - old[0]).abs() > f32::EPSILON || (new_pos[1] - old[1]).abs() > f32::EPSILON
        {
            moves.push((group_index, old, new_pos));
        }
        for (index, [dx, dy]) in push_plan {
            let (Some(from), Some(to_target)) = (
                self.scene.canvas.nodes.get(index).map(|n| [n.x, n.y]),
                self.scene
                    .canvas
                    .nodes
                    .get(index)
                    .map(|n| [n.x + dx, n.y + dy]),
            ) else {
                continue;
            };
            if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                node.x += dx;
                node.y += dy;
            }
            if let Some(node) = self.scene.canvas.nodes.get(index) {
                self.scene.spatial.update(index, node);
            }
            moves.push((index, from, to_target));
        }
        if !moves.is_empty() {
            // FR-073: settle-анимация ведёт ноды к целям — их якоря =
            // целевые позиции, чтобы пружина расталкивания не боролась
            // с анимацией
            for (index, _, to_target) in &moves {
                if let Some(node) = self.scene.canvas.nodes.get(*index) {
                    self.drag_push.anchors.insert(node.id.clone(), *to_target);
                }
            }
            self.settle_anim = Some(SettleAnim {
                moves,
                start: Instant::now(),
            });
        }
        self.scene.mark_dirty();
        self.show_toast(self.tr(keys::TOAST_NODE_INSERTED));
    }

    /// Вынос детей из групп после drag (FR-012): нода, отпущенная вне rect
    /// своей ЯВНОЙ группы, удаляется из её детей. Легаси-группы (без
    /// списка) не участвуют — их геометрический фолбэк не меняется.
    fn group_drag_out_released(&mut self) {
        let Some(drag) = self.dragging.as_ref() else {
            return;
        };
        let dragged: Vec<usize> = std::iter::once(drag.primary)
            .chain(drag.origins.iter().map(|(i, _)| *i))
            .collect();
        // (группа → [id нод к выносу])
        let mut removals: Vec<(usize, String)> = Vec::new();
        for index in dragged {
            let Some(node) = self.scene.canvas.nodes.get(index) else {
                continue;
            };
            let center = [node.x + node.width / 2.0, node.y + node.height / 2.0];
            for (gi, group) in self.scene.canvas.nodes.iter().enumerate() {
                if gi == index || group.kind() != NodeKind::Group {
                    continue;
                }
                let Some(children) = &group.children else {
                    continue; // легаси-группа: геометрия, жеста нет
                };
                if !children.contains(&node.id) {
                    continue;
                }
                let outside = center[0] < group.x
                    || center[0] > group.x + group.width
                    || center[1] < group.y
                    || center[1] > group.y + group.height;
                if outside {
                    removals.push((gi, node.id.clone()));
                }
            }
        }
        if removals.is_empty() {
            return;
        }
        // FR-006: вынос — undo-шаг (один на все удаления)
        self.push_undo();
        for (gi, id) in removals {
            canvas_core::group_remove_child(&mut self.scene.canvas, gi, &id);
        }
        self.scene.mark_dirty();
        self.show_toast(self.tr(keys::TOAST_NODE_EXTRACTED));
    }

    /// Rect открытого меню канваса (логические px) — airspace для виджетов.
    /// Число пунктов — то же видимое множество, что в отрисовке/хит-тесте
    /// (batch-пункты FR-038 добавляют высоту только при N≥3).
    fn menu_open_rect(&self) -> Option<[f32; 4]> {
        let menu = self.menu.as_ref()?;
        Some(menu_rect_for(
            menu.origin,
            canvas_menu_visible_items(self.align_menu_visible()).len(),
        ))
    }

    /// FR-050 Н2 (этап C): rect панели меню выбора (пункты + строка
    /// заголовка) — для hit-rect'а реестра поверхностей (FR-052 U2).
    fn choice_menu_rect(&self) -> Option<[f32; 4]> {
        let menu = self.choice_menu.as_ref()?;
        let [x, y, w, h] = menu_rect_for(menu.origin, menu.items.len());
        Some([x, y, w, h + CHOICE_MENU_TITLE_H])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    // Этап 1 рефакторинга: anchor_grid_delta живёт в support (используется
    // только в тестах — здесь точечный импорт вместо родительского).
    use support::anchor_grid_delta;

    // --- PRD-0007 (FR-048 X2): чистые хелперы окна проверки ----------------

    /// explain_chain_focus (F-4): узлы дерева → индексы канваса, рёбра
    /// via → индексы; невалидные id пропускаются; дубли (ромб) дедупятся.
    #[test]
    fn explain_chain_focus_maps_tree_to_canvas() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "", 100.0, 0.0));
        canvas.nodes.push(Node::text("c", "", 200.0, 0.0));
        canvas.edges.push(Edge {
            id: "e1".into(),
            from_node: "a".into(),
            from_side: None,
            to_node: "b".into(),
            to_side: None,
            label: None,
            color: None,
            style: None,
            thickness: None,
            from_line: None,
            from_output: None,
            to_param: None,
            extra: Default::default(),
        });
        canvas.edges.push(Edge {
            id: "e2".into(),
            from_node: "b".into(),
            from_side: None,
            to_node: "c".into(),
            to_side: None,
            label: None,
            color: None,
            style: None,
            thickness: None,
            from_line: None,
            from_output: None,
            to_param: None,
            extra: Default::default(),
        });
        // Дерево: корень c → b (e1!) → a (ребро e2 — чужой id), лист
        // «ghost» — ноды нет на канвасе (молча пропускается)
        let tree = canvas_core::LineageTree {
            root: canvas_core::LineageNodeId::total("c"),
            nodes: vec![
                canvas_core::LineageNode {
                    node_id: "c".into(),
                    line: None,
                    kind: canvas_core::LineageNodeKind::Calc,
                    value: None,
                    formula: None,
                    title: "C".into(),
                    label: None,
                    children: vec![canvas_core::LineageChild {
                        child: 1,
                        via: Some(canvas_core::LineageVia {
                            edge_id: "e1".into(),
                            from_node: "b".into(),
                            to_node: "c".into(),
                            from_line: None,
                            from_output: None,
                            to_param: None,
                        }),
                    }],
                },
                canvas_core::LineageNode {
                    node_id: "b".into(),
                    line: None,
                    kind: canvas_core::LineageNodeKind::Calc,
                    value: None,
                    formula: None,
                    title: "B".into(),
                    label: None,
                    children: vec![canvas_core::LineageChild {
                        child: 2,
                        // Ромб-дубль ребра + несуществующее ребро
                        via: Some(canvas_core::LineageVia {
                            edge_id: "e1".into(),
                            from_node: "a".into(),
                            to_node: "b".into(),
                            from_line: None,
                            from_output: None,
                            to_param: None,
                        }),
                    }],
                },
                canvas_core::LineageNode {
                    node_id: "ghost".into(),
                    line: None,
                    kind: canvas_core::LineageNodeKind::Leaf,
                    value: None,
                    formula: None,
                    title: "G".into(),
                    label: None,
                    children: Vec::new(),
                },
            ],
        };
        let set = explain_chain_focus(&canvas, &tree);
        // Узлы: b=1, c=2 (ghost пропущен — ноды нет на канвасе; «a» в
        // дерево не входит), отсортированы
        assert_eq!(set.nodes, vec![1, 2]);
        // Рёбра: e1 дважды → один индекс 0; e2 не встречался — нет
        assert_eq!(set.edges, vec![0]);
    }

    // FR-037 MW1: перенесённые в canvas-scene сущности (модель сцены,
    // двухуровневый refit) — здесь остались только canvas-render-зависимые
    // тесты точного измерения (уровень 2)
    use canvas_core::expr::{line_kind, NumiLineKind};
    use canvas_render::text::BODY_LINE_HEIGHT;
    use canvas_scene::{ensure_result_reserve, wrapped_body_rows};

    /// Локальный диспетчер для остающихся в canvas-app тестов (зависят от
    /// canvas-render — точное измерение); MCP-тесты переехали в
    /// canvas-scene (имена/ассерты сохранены).
    fn dispatch(
        scene: &mut SceneState,
        method: &str,
        params: &str,
    ) -> Result<serde_json::Value, String> {
        let params: serde_json::Value = serde_json::from_str(params).expect("params — JSON");
        let registry = canvas_core::templates::TemplateRegistry::builtin();
        canvas_scene::mcp_dispatch(scene, &registry, method, &params)
    }

    /// CR-015: область центрируемого screen-текста лежит внутри rect с
    /// двусторонним инсетом (origin — левый край области по контракту
    /// `ScreenText`; подпись центрируется рендером внутри [origin, origin+width]).
    #[test]
    fn centered_box_stays_inside_rect() {
        let rect = [100.0, 20.0, 90.0, 26.0];
        let (origin, width) = centered_box(rect, 3.0);
        assert_eq!(origin[0], rect[0] + 3.0);
        assert_eq!(origin[1], rect[1]);
        // Область симметрична: центр области == центр rect.
        assert!((origin[0] + width / 2.0 - (rect[0] + rect[2] / 2.0)).abs() < 0.01);
        assert!(origin[0] + width <= rect[0] + rect[2] - 3.0 + 0.01);
        // Узкий rect — ширина не уходит в минус.
        let (_, narrow_w) = centered_box([0.0, 0.0, 4.0, 10.0], 3.0);
        assert_eq!(narrow_w, 0.0);
    }

    // --- FR-038 (T-038.4): чистые хелперы интеграции магнитной раскладки ----

    /// Краткая форма прямоугольника для snap-тестов.
    fn sr(x: f32, y: f32, w: f32, h: f32) -> SnapRect {
        SnapRect { x, y, w, h }
    }

    /// drag_bbox: одиночный набор — rect ноды по origin; групповой — union.
    #[test]
    fn drag_bbox_unions_origins() {
        let mut canvas = Canvas::default();
        // Размеры заданы явно: drag_bbox берёт размеры из текущих нод
        let mut a = Node::text("a", "", 10.0, 20.0);
        a.width = 30.0;
        a.height = 60.0;
        canvas.nodes.push(a);
        let mut wide = Node::text("b", "", 0.0, 0.0);
        wide.x = 40.0;
        wide.y = 5.0;
        wide.width = 30.0;
        wide.height = 60.0;
        canvas.nodes.push(wide);
        let origins = vec![(0usize, [1.0, 2.0]), (1usize, [3.0, 4.0])];
        let bbox = drag_bbox(&canvas, &origins).expect("bbox");
        assert_eq!((bbox.x, bbox.y), (1.0, 2.0));
        assert_eq!((bbox.right(), bbox.bottom()), (33.0, 64.0));
        // Битый индекс — None (снап не применяется)
        assert!(drag_bbox(&canvas, &[(99, [0.0, 0.0])]).is_none());
    }

    /// snap_candidates: перемещаемые исключены (в т.ч. дети групп — они в
    /// origins), группы попадают своим bbox.
    #[test]
    fn snap_candidates_exclude_moving_set() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("moving", "", 0.0, 0.0));
        let mut group = Node::text("g", "", 0.0, 0.0);
        group.node_type = "group".into();
        group.x = 100.0;
        group.width = 200.0;
        canvas.nodes.push(group);
        canvas.nodes.push(Node::text("other", "", 300.0, 0.0));
        let candidates = snap_candidates(&canvas, &[0, 1, 2], &[0]);
        assert_eq!(candidates.len(), 2, "moving исключён, группа и сосед — нет");
        assert_eq!((candidates[0].x, candidates[0].w), (100.0, 200.0));
        assert_eq!((candidates[1].x, candidates[1].w), (300.0, 260.0));
    }

    /// anchor_grid_delta: в допуске — дельта до ближайшей линии, за допуском
    /// — 0; отрицательные координаты корректны.
    #[test]
    fn anchor_grid_delta_rules() {
        assert_eq!(anchor_grid_delta(103.0, 20.0, 8.0), -3.0);
        assert_eq!(anchor_grid_delta(117.0, 20.0, 8.0), 3.0);
        assert_eq!(anchor_grid_delta(110.0, 20.0, 8.0), 0.0, "за допуском");
        // Отрицательные координаты: −3 тянется к линии 0 (+3)
        assert_eq!(anchor_grid_delta(-3.0, 20.0, 8.0), 3.0);
        assert_eq!(anchor_grid_delta(-17.0, 20.0, 8.0), -3.0);
    }

    /// snap_with_anchor: BoundingBox — точный passthrough движка.
    #[test]
    fn snap_with_anchor_bounding_box_passthrough() {
        let cfg = SnapConfig::default();
        let candidates = [sr(160.0, 500.0, 50.0, 50.0)];
        let outcome = snap_with_anchor(
            sr(0.0, 0.0, 40.0, 40.0),
            162.0,
            0.0,
            &candidates,
            &cfg,
            SnapAnchor::BoundingBox,
        );
        let direct = snap_move(sr(0.0, 0.0, 40.0, 40.0), 162.0, 0.0, &candidates, &cfg);
        assert_eq!(outcome, direct);
    }

    /// snap_with_anchor Corner: левый-верхний угол тянется к пересечению
    /// линий; ось за допуском не двигается.
    #[test]
    fn snap_with_anchor_corner_aligns_corner() {
        let cfg = SnapConfig::default();
        // Угол (3, 7): X до линии 0 (дельта -3 в допуске), Y до линии 0
        // (дельта -7 в допуске 8) — угол встаёт на пересечение (0, 0)
        let outcome = snap_with_anchor(
            sr(3.0, 7.0, 43.0, 30.0),
            0.0,
            0.0,
            &[],
            &cfg,
            SnapAnchor::Corner,
        );
        assert_eq!(outcome.dx, -3.0);
        assert_eq!(outcome.dy, -7.0);
        assert!(outcome.guides_x.is_empty() && outcome.guides_y.is_empty());
        assert_eq!(outcome.source, SnapSource::Grid);
        // Сетка выключена — anchor-grid молчит (п.4: anchor меняет только grid)
        let no_grid = SnapConfig {
            to_grid: false,
            ..SnapConfig::default()
        };
        let outcome = snap_with_anchor(
            sr(3.0, 7.0, 43.0, 30.0),
            0.0,
            0.0,
            &[],
            &no_grid,
            SnapAnchor::Corner,
        );
        assert_eq!((outcome.dx, outcome.dy), (0.0, 0.0));
    }

    /// snap_with_anchor Center: центр bbox — к пересечению линий.
    #[test]
    fn snap_with_anchor_center_aligns_center() {
        let cfg = SnapConfig::default();
        // Центр (21.5, 15): до линий 20 — дельты -1.5/+5, обе в допуске
        let outcome = snap_with_anchor(
            sr(0.0, 0.0, 43.0, 30.0),
            0.0,
            0.0,
            &[],
            &cfg,
            SnapAnchor::Center,
        );
        assert_eq!(outcome.dx, -1.5);
        assert_eq!(outcome.dy, 5.0);
    }

    /// snap_with_anchor: арбитраж anchor-grid vs guide (п.11) — меньшая
    /// |дельта| побеждает; проигравшая guide-ось линию не оставляет (п.9).
    #[test]
    fn snap_with_anchor_arbitrates_grid_vs_guide() {
        let cfg = SnapConfig::default();
        // Guide ближе (0.25 < 0.5 у grid; значения кратны 0.25 — точны в
        // f32, дельты бит-в-бит): guide выигрывает, линия остаётся
        let closer = [sr(100.75, 500.0, 50.0, 50.0)];
        let outcome = snap_with_anchor(
            sr(100.5, 0.0, 40.0, 40.0),
            0.0,
            0.0,
            &closer,
            &cfg,
            SnapAnchor::Corner,
        );
        assert_eq!(outcome.dx, 0.25);
        assert_eq!(outcome.guides_x, vec![100.75]);
        assert_eq!(outcome.source, SnapSource::Guide);
        // Grid ближе (0.5 < 1.5 у guide): grid выигрывает, линия гасится
        let farther = [sr(102.0, 500.0, 50.0, 50.0)];
        let outcome = snap_with_anchor(
            sr(100.5, 0.0, 40.0, 40.0),
            0.0,
            0.0,
            &farther,
            &cfg,
            SnapAnchor::Corner,
        );
        assert_eq!(outcome.dx, -0.5);
        assert!(outcome.guides_x.is_empty());
        assert_eq!(outcome.source, SnapSource::Grid);
    }

    /// snap_with_anchor: collision (п.15) — финальный кламп поверх
    /// anchor-grid: движение останавливается на границе зазора.
    #[test]
    fn snap_with_anchor_collision_clamps_last() {
        let cfg = SnapConfig {
            collision_gap: 8.0,
            ..SnapConfig::default()
        };
        // Движение вправо на 95: anchor-grid тянет угол к линии 100, но
        // зона зазора кандидата (120−8) останавливает правый край на 112 →
        // дельта 72
        let candidate = [sr(120.0, 0.0, 40.0, 40.0)];
        let outcome = snap_with_anchor(
            sr(0.0, 0.0, 40.0, 40.0),
            95.0,
            0.0,
            &candidate,
            &cfg,
            SnapAnchor::Corner,
        );
        assert_eq!(outcome.dx, 72.0);
        assert!(outcome.guides_x.is_empty());
        // Без collision anchor-grid дотянул бы до 100
        let free = SnapConfig::default();
        let outcome = snap_with_anchor(
            sr(0.0, 0.0, 40.0, 40.0),
            95.0,
            0.0,
            &candidate,
            &free,
            SnapAnchor::Corner,
        );
        assert_eq!(outcome.dx, 100.0);
    }

    /// nudge_step_world (п.22): экранные px → world делением на зум — шаг
    /// «1 px экрана» одинаково выглядит на любом зуме; зум ≤ 0 — фолбэк.
    #[test]
    fn nudge_step_world_scales_with_zoom() {
        assert_eq!(nudge_step_world(1.0, 2.0), 0.5);
        assert_eq!(nudge_step_world(1.0, 0.5), 2.0);
        assert_eq!(nudge_step_world(10.0, 1.0), 10.0);
        assert_eq!(nudge_step_world(1.0, 0.0), 1.0);
        assert_eq!(nudge_step_world(1.0, -2.0), 1.0);
    }

    /// FR-037 MW1: паритет констант canvas-scene с canvas-render (метрики
    /// раскладки refit, зум viewport) и дефолтов файловой карточки MCP
    /// (crate::ui::DROP_CARD_*) — вынос не расшатывает синхронность.
    #[test]
    fn measure_layout_consts_match_render() {
        assert_eq!(
            canvas_scene::measure::HEADER_HEIGHT,
            canvas_render::cards::HEADER_HEIGHT
        );
        assert_eq!(
            canvas_scene::measure::BODY_FONT_SIZE,
            canvas_render::text::BODY_FONT_SIZE
        );
        assert_eq!(
            canvas_scene::measure::BODY_LINE_HEIGHT,
            canvas_render::text::BODY_LINE_HEIGHT
        );
        assert_eq!(
            canvas_scene::measure::BODY_PADDING,
            canvas_render::text::BODY_PADDING
        );
        assert_eq!(
            canvas_scene::measure::BODY_TOP_GAP,
            canvas_render::text::BODY_TOP_GAP
        );
        assert_eq!(
            canvas_scene::measure::RESULT_LINE_HEIGHT,
            canvas_render::text::RESULT_LINE_HEIGHT
        );
        // FR-069 (этап F): высота ряда подписи секции (зеркало ZONE_LABEL_LINE_HEIGHT).
        assert_eq!(
            canvas_scene::measure::ZONE_LABEL_LINE_HEIGHT,
            canvas_render::text::ZONE_LABEL_LINE_HEIGHT
        );
        assert_eq!(canvas_scene::MIN_ZOOM, canvas_render::camera::MIN_ZOOM);
        assert_eq!(canvas_scene::MAX_ZOOM, canvas_render::camera::MAX_ZOOM);
        assert_eq!(canvas_scene::DEFAULT_FILE_CARD_W, crate::ui::DROP_CARD_W);
        assert_eq!(canvas_scene::DEFAULT_FILE_CARD_H, crate::ui::DROP_CARD_H);
        // FR-038: дефолты порогов zoom-адаптивной сетки core ↔ render —
        // дублируются (зависимости core→render нет, ADR-0012), расхождение
        // ловим паритетом
        assert_eq!(
            canvas_core::SNAP_SUB_ZOOM_DEFAULT,
            canvas_render::grid::DEFAULT_SUB_ZOOM
        );
        assert_eq!(
            canvas_core::SNAP_COARSE_ZOOM_DEFAULT,
            canvas_render::grid::DEFAULT_COARSE_ZOOM
        );
    }

    /// CR-010: оценка рядов тела с переносами — длинная строка даёт
    /// несколько визуальных рядов (рендер шейпит Wrap::WordOrGlyph),
    /// пустой текст — минимум один ряд.
    #[test]
    fn wrapped_body_rows_counts_visual_rows() {
        // Ширина тела 240 (нода 260 − 2·BODY_PADDING): ~34 юнита на ряд
        let width = 260.0 - BODY_PADDING * 2.0;
        assert_eq!(wrapped_body_rows("a", width), 1);
        assert_eq!(wrapped_body_rows("a\nb", width), 2);
        // 200 символов — больше одного ряда
        let long = "x".repeat(200);
        assert!(wrapped_body_rows(&long, width) >= 5, "переносы недооценены");
        // Короткие строки суммируются по рядам
        let two = format!("{}\n{}", "y".repeat(100), "z");
        assert!(wrapped_body_rows(&two, width) >= 4);
        assert_eq!(wrapped_body_rows("", width), 1);
    }

    /// CR-012: строки-присваивания Numi-листа рендерятся моноширинным
    /// Noto Sans Mono (mono-флаг source_line, text.rs) — аванс шире
    /// пропорциональной оценки 7 px/символ. При ширине тела 240 px
    /// 32-символьное присваивание старой метрикой давало 1 ряд, рендер
    /// переносит на 2 — оценка обязана считать mono-строки по метрике
    /// моноширинного шрифта.
    #[test]
    fn wrapped_body_rows_mono_assignment_uses_mono_metric() {
        let width = 260.0 - BODY_PADDING * 2.0;
        let line = format!("{} = 5", "a".repeat(28)); // 32 символа
        assert!(
            matches!(line_kind(&line), NumiLineKind::Assignment { .. }),
            "строка должна распознаваться как присваивание Numi"
        );
        assert_eq!(
            wrapped_body_rows(&line, width),
            2,
            "моно-строка из 32 символов при ширине 240 переносится на 2 ряда"
        );
        // Прекондition регресса: старая метрика (7 px/символ) давала бы 1 ряд.
        assert!(
            32.0 / (width / 7.0).floor() <= 1.0,
            "старая пропорциональная метрика занижала до 1 ряда"
        );
        // Длинное значение (~51 символ) — не менее 2 рядов по mono-метрике.
        let long = format!("{} = 100 rps", "a".repeat(40));
        assert!(wrapped_body_rows(&long, width) >= 2);
        // Прозаическая строка той же длины — прежняя метрика (1 ряд).
        let prose = format!("{} встреча в офисе", "a".repeat(18));
        assert_eq!(line_kind(&prose), NumiLineKind::Prose);
        assert_eq!(wrapped_body_rows(&prose, width), 1);
    }

    /// CR-012: двухуровневый ленивый refit — оценка ворота, рост по
    /// измеренной высоте. Заниженная высота растёт минимум до измеренного
    /// резерва футера, достаточная не трогается; повторный вызов
    /// идемпотентен (growth-only — без осцилляций при частых пересчётах).
    #[test]
    fn ensure_result_reserve_grows_only() {
        // FR-037 MW1: уровень 2 (точное измерение) — как до выноса из main.rs
        canvas_scene::install_measured_reserve(measured_result_reserve_height);
        let line = format!("{} = 5", "a".repeat(28));
        let mut low = Node::text("n", line.clone(), 0.0, 0.0);
        low.width = 260.0;
        low.height = 80.0; // занижено: 2 ряда тела + резерв футера не влезают
                           // CR-012 (правка 2): formula_lines для присваивания — [0].
        ensure_result_reserve(&mut low, &line, &[0], None, false, true, "", 0);
        let needed = measured_result_reserve_height(&line, low.width, &[0], "", false, true, "", 0);
        assert!(
            low.height >= needed - 1e-3,
            "высота {} выросла минимум до измеренного резерва футера {needed}",
            low.height
        );
        let grown = low.height;
        ensure_result_reserve(&mut low, &line, &[0], None, false, true, "", 0);
        assert_eq!(low.height, grown, "повторный вызов — no-op (growth-only)");
        let mut tall = Node::text("n2", line.clone(), 0.0, 0.0);
        tall.width = 260.0;
        tall.height = 1000.0;
        ensure_result_reserve(&mut tall, &line, &[0], None, false, true, "", 0);
        assert_eq!(tall.height, 1000.0, "достаточная высота не сжимается");
    }

    /// CR-010/CR-012: стартовая высота новой шаблонной ноды покрывает
    /// переносы — длинное значение параметра не вылезает за низ карточки.
    /// CR-012: строка-присваивание считается моноширинной метрикой:
    /// 32-символьное присваивание при ширине тела 240 рендерится в 2 ряда,
    /// а пропорциональная оценка давала 1 — высота занижалась на ряд.
    /// CR-012 (правка 2): высота — по измеренной высоте тела (реальный
    /// шейпинг теми же шрифтами, что у рендера).
    #[test]
    fn fit_template_height_covers_wrapped_lines() {
        // FR-037 MW1: уровень 2 (точное измерение) — как до выноса из main.rs
        canvas_scene::install_measured_reserve(measured_result_reserve_height);
        let mono_line = format!("{} = 5", "a".repeat(28));
        let mut node = Node::text("tpl", mono_line.clone(), 0.0, 0.0);
        node.width = 260.0;
        node.height = 80.0; // занижено (как дефолт манифеста при длинном листе)
        fit_template_node_height(&mut node);
        let body_width = node.width - BODY_PADDING * 2.0;
        assert!(
            wrapped_body_rows(&mono_line, body_width) >= 2,
            "моно-строка должна занимать минимум 2 ряда"
        );
        // CR-012 (правка 2): высота покрывает измеренную высоту тела
        // (formula_lines — из eval_lines, тот же источник, что у refit).
        let formula_lines = formula_line_indices(&expr::eval_lines(&mono_line));
        let needed = measured_result_reserve_height(
            &mono_line,
            node.width,
            &formula_lines,
            "",
            false,
            true,
            "",
            0,
        );
        assert!(
            node.height >= needed - 1e-3,
            "высота {} меньше измеренной нужной {needed}",
            node.height
        );
        // Регресс самой заниженной оценки: высота минимум на ряд больше
        // «старой» формулы с одним рядом тела.
        let old_one_row = HEADER_HEIGHT
            + BODY_TOP_GAP
            + BODY_LINE_HEIGHT
            + BODY_PADDING
            + RESULT_LINE_HEIGHT
            + 2.0;
        assert!(
            node.height >= old_one_row + BODY_LINE_HEIGHT,
            "высота должна покрывать второй ряд моно-переноса"
        );
        // Короткий лист — высота скромная (рост, не раздувание)
        let mut short = Node::text("tpl2", "rps = 10 rps", 0.0, 0.0);
        short.width = 260.0;
        short.height = 80.0;
        fit_template_node_height(&mut short);
        assert!(short.height < node.height);
    }

    /// CR-012: ленивый refit при пересчёте — ноды, которым рендер покажет
    /// футер результата (шаблонные всегда; обычные — без построчных
    /// результатов), получают резерв по высоте; прочие не трогаются.
    /// Покрывает загрузку .canvas и MCP node_update_text (оба идут через
    /// recompute_flow).
    #[test]
    fn recompute_grows_result_reserve_for_footer_nodes() {
        // FR-037 MW1: уровень 2 (точное измерение) — как до выноса из main.rs
        canvas_scene::install_measured_reserve(measured_result_reserve_height);
        use canvas_core::templates::{TemplateParam, TemplateRef};
        let mut canvas = Canvas::default();
        // Шаблонная нода: футер результата показывается всегда; высота
        // занижена, тело — моноширинное присваивание на 2 ряда.
        let mut tpl = Node::text("tpl1", format!("{} = 5", "a".repeat(28)), 0.0, 0.0);
        tpl.width = 260.0;
        tpl.height = 80.0;
        tpl.set_expr(Some("$rps".to_owned()));
        tpl.set_template(Some(TemplateRef {
            id: "t".to_owned(),
            version: "1".to_owned(),
            expr: "$rps".to_owned(),
            params: BTreeMap::from([(
                "rps".to_owned(),
                TemplateParam {
                    num: 1000.0,
                    unit: Some("rps".to_owned()),
                },
            )]),
            icon: "custom".to_owned(),
            color: "#9B9B9B".to_owned(),
            name: None,
            outputs: Vec::new(),
        }));
        canvas.nodes.push(tpl);
        // Обычная прозаическая нода — футера нет, высота не трогается.
        let mut plain = Node::text("p1", "Просто заметка без формул", 0.0, 400.0);
        plain.width = 260.0;
        plain.height = 120.0;
        canvas.nodes.push(plain);
        let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/cr012.canvas"));
        // CR-012 (правка 2): порог — измеренная высота (реальный шейпинг),
        // а не оценка рядов: formula_lines — из построчных результатов ноды.
        let needed_for = |scene: &SceneState, node: &Node| {
            let text = node.text.as_deref().unwrap_or("");
            let formula_lines = scene
                .expr_line_results
                .get(&node.id)
                .map(|lines| formula_line_indices(lines))
                .unwrap_or_default();
            measured_result_reserve_height(text, node.width, &formula_lines, "", false, true, "", 0)
        };
        let tpl_node = scene.canvas.node("tpl1").expect("нода tpl1");
        assert!(
            tpl_node.height >= needed_for(&scene, tpl_node),
            "шаблонная нода выросла под резерв футера"
        );
        match scene.expr_results.get("tpl1").expect("результат tpl1") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1\u{a0}000 rps"),
            other => panic!("ожидалось значение: {other:?}"),
        }
        assert_eq!(
            scene.canvas.node("p1").expect("нода p1").height,
            120.0,
            "прозаическая нода без футера не тронута"
        );
        // MCP-мутация текста шаблонной ноды (длиннее — рядов больше):
        // рост под новый резерв применяется тем же пересчётом.
        let text = format!("{} = 5\n= $rps", "b".repeat(55));
        dispatch(
            &mut scene,
            "node_update_text",
            &serde_json::json!({ "id": "tpl1", "text": text }).to_string(),
        )
        .expect("node_update_text");
        let tpl_node = scene.canvas.node("tpl1").expect("нода tpl1");
        assert!(
            tpl_node.height >= needed_for(&scene, tpl_node),
            "после MCP-правки высота покрывает новый резерв футера"
        );
    }

    /// CR-012 (правка 2): двухуровневый refit при загрузке — шаблонная нода
    /// со старым дефолтом высоты (120, как до CR-010) вырастает РОВНО до
    /// измеренного резерва (реальный шейпинг, без фантомного ряда);
    /// повторная загрузка с подогнанной высотой не растит дальше
    /// (нет осцилляций/ползучести).
    #[test]
    fn recompute_result_reserve_matches_measured_height() {
        // FR-037 MW1: уровень 2 (точное измерение) — как до выноса из main.rs
        canvas_scene::install_measured_reserve(measured_result_reserve_height);
        use canvas_core::templates::{TemplateParam, TemplateRef};
        let mut canvas = Canvas::default();
        // Скриншотный Numi-лист из CR-012: три параметра при ширине 280 —
        // тело 3 mono-ряда, при высоте 120 футер налезал на последнюю строку.
        let text = "rps = 200 rps\ntoken_verify = 2 ms\ncache_ttl = 5 min".to_owned();
        let mut tpl = Node::text("tpl1", text.clone(), 0.0, 0.0);
        tpl.width = 280.0;
        tpl.height = 120.0;
        tpl.set_expr(Some("$rps".to_owned()));
        tpl.set_template(Some(TemplateRef {
            id: "t".to_owned(),
            version: "1".to_owned(),
            expr: "$rps".to_owned(),
            params: BTreeMap::from([(
                "rps".to_owned(),
                TemplateParam {
                    num: 200.0,
                    unit: Some("rps".to_owned()),
                },
            )]),
            icon: "custom".to_owned(),
            color: "#9B9B9B".to_owned(),
            name: None,
            outputs: Vec::new(),
        }));
        canvas.nodes.push(tpl);
        let path = PathBuf::from("target/tmp/cr012-measured.canvas");
        let scene = SceneState::new(canvas, path.clone());
        let tpl_node = scene.canvas.node("tpl1").expect("нода tpl1");
        let formula_lines = formula_line_indices(
            scene
                .expr_line_results
                .get("tpl1")
                .expect("построчные результаты tpl1"),
        );
        assert_eq!(
            formula_lines,
            vec![0, 1, 2],
            "все три строки-присваивания — формульные"
        );
        let needed = measured_result_reserve_height(
            &text,
            tpl_node.width,
            &formula_lines,
            "",
            false,
            true,
            "",
            0,
        );
        assert!(
            (tpl_node.height - needed).abs() < 1e-3,
            "высота ровно измеренная: {} vs {needed} (growth-only от 120)",
            tpl_node.height
        );
        assert!(
            tpl_node.height > 120.0,
            "старый дефолт 120 занижен — нода выросла"
        );
        // Повторная загрузка с уже подогнанной высотой — без дальнейшего роста.
        let scene2 = SceneState::new(scene.canvas.clone(), path);
        let tpl2 = scene2.canvas.node("tpl1").expect("нода tpl1");
        assert_eq!(
            tpl2.height, tpl_node.height,
            "повторный refit не растит дальше (нет осцилляций)"
        );
    }

    /// Стартовый канвас непустой и переживает round-trip.
    #[test]
    fn seed_canvas_is_valid() {
        let canvas = canvas_scene::seed_canvas();
        assert!(!canvas.nodes.is_empty());
        let json = canvas.to_json().expect("сериализация seed");
        let restored = Canvas::from_str(&json).expect("seed парсится обратно");
        assert_eq!(canvas, restored);
    }

    /// Стресс-генератор (T5): ровно N нод, детерминизм, размеры в пределах.
    #[test]
    fn stress_canvas_is_deterministic_and_bounded() {
        let a = stress_canvas(5000);
        let b = stress_canvas(5000);
        assert_eq!(a.nodes.len(), 5000);
        assert_eq!(a, b, "одинаковый N должен давать одинаковую сцену");
        for node in &a.nodes {
            assert!((120.0..=420.0).contains(&node.width));
            assert!((80.0..=300.0).contains(&node.height));
            assert_eq!(node.kind(), canvas_core::NodeKind::Text);
            assert!(node.id.starts_with("stress-"));
        }
    }

    /// Резолв путей файловых нод (T6): относительные — от каталога канваса,
    /// результат всегда абсолютный (shell-API иначе отказывает).
    /// Windows-only: тест оперирует Windows-путями (диск `C:`), на Unix
    /// они не абсолютны — семантика проверяется на CI (windows-latest).
    #[cfg(windows)]
    #[test]
    fn resolve_file_path_is_absolute() {
        let scene = SceneState::new(Canvas::default(), PathBuf::from("target/tmp/thumbs.canvas"));
        let abs = scene.resolve_file_path("C:/abs/photo.png");
        assert_eq!(abs, PathBuf::from("C:/abs/photo.png"));
        let rel = scene.resolve_file_path("thumbtest/photo1.png");
        assert!(
            rel.is_absolute(),
            "относительный путь не абсолютизирован: {rel:?}"
        );
        assert!(rel.ends_with(PathBuf::from("target/tmp/thumbtest/photo1.png")));
    }

    /// Парсинг аргументов: --stress N, --stress=N, --desktop, путь, дефолты.
    #[test]
    fn cli_args_parsing() {
        let args = parse_args(&[]).expect("пустые аргументы");
        assert_eq!(args.stress, None);
        assert!(!args.desktop);
        assert_eq!(args.path, PathBuf::from("default.canvas"));

        let args = parse_args(&["--stress".into(), "5000".into()]).expect("--stress N");
        assert_eq!(args.stress, Some(5000));
        assert!(!args.desktop);
        assert_eq!(args.path, PathBuf::from("stress.canvas"));

        let args =
            parse_args(&["--stress=100".into(), "my.canvas".into()]).expect("--stress=N path");
        assert_eq!(args.stress, Some(100));
        assert_eq!(args.path, PathBuf::from("my.canvas"));

        // T15: флаг --desktop (булев, повтор идемпотентен, порядок любой)
        let args = parse_args(&["--desktop".into()]).expect("--desktop");
        assert!(args.desktop);
        assert_eq!(args.path, PathBuf::from("default.canvas"));

        let args =
            parse_args(&["--desktop".into(), "board.canvas".into()]).expect("--desktop path");
        assert!(args.desktop);
        assert_eq!(args.path, PathBuf::from("board.canvas"));

        let args = parse_args(&["--stress=7".into(), "--desktop".into(), "--desktop".into()])
            .expect("--stress + двойной --desktop");
        assert_eq!(args.stress, Some(7));
        assert!(args.desktop);
        assert_eq!(args.path, PathBuf::from("stress.canvas"));

        assert!(parse_args(&["--stress".into()]).is_err());
        assert!(parse_args(&["--stress".into(), "abc".into()]).is_err());
        assert!(parse_args(&["a.canvas".into(), "b.canvas".into()]).is_err());
    }

    // --- MCP-интеграция: mcp_dispatch (15 инструментов) ---

    // --- FR-006: undo/redo ---

    // --- FR-013: Numi-формулы (canvasdesk.expr) ---

    /// Смешанный редактор: строки «= …» — формула без префикса; пустые
    /// формульные строки пропускаются; без «=» — None.
    #[test]
    fn split_formula_lines_extracts_equal_prefixed() {
        assert_eq!(split_formula_lines("= 5 ms"), Some("5 ms".to_owned()));
        assert_eq!(split_formula_lines("=  5 ms  "), Some("5 ms".to_owned()));
        assert_eq!(
            split_formula_lines("Gateway\n= rps = 1000\n= latency = 50 ms"),
            Some("rps = 1000\nlatency = 50 ms".to_owned()),
            "несколько утверждений соединяются переводом строки"
        );
        // Пустая формула («=» без содержимого) — не утверждение
        assert_eq!(split_formula_lines("=\n= 5 ms"), Some("5 ms".to_owned()));
        // Нет формульных строк — None
        assert_eq!(split_formula_lines("Просто текст"), None);
        assert_eq!(split_formula_lines(""), None);
        // «=» внутри строки не формула — только начало строки
        assert_eq!(split_formula_lines("a = b"), None);
    }

    /// FR-018: параметры шаблона из текста ноды — присваивания вычисляются
    /// Numi-eval'ом (единицы, суффиксы `2k`); проза и пустые строки
    /// пропускаются.
    #[test]
    fn template_params_from_text_parses_assignments() {
        let params = canvas_core::templates::params_from_text(
            "rps = 1000 rps\nservice_rate = 1200 rps\nservers = 2k\nКомментарий-проза\n\nempty = ",
        );
        assert_eq!(params.len(), 3, "проза/пустые — мимо");
        assert_eq!(params["rps"].num, 1000.0);
        assert_eq!(params["rps"].unit.as_deref(), Some("rps"));
        assert_eq!(params["service_rate"].num, 1200.0);
        assert_eq!(params["servers"].num, 2000.0, "суффикс k разворачивается");
        assert_eq!(params["servers"].unit, None);
        // Скаляр без единицы
        let scalar = canvas_core::templates::params_from_text("k = 3");
        assert_eq!(scalar["k"].num, 3.0);
        assert_eq!(scalar["k"].unit, None);
        // Проза с «=» не парсится в значение — мимо
        let prose = canvas_core::templates::params_from_text("Server load = high");
        assert!(prose.is_empty(), "не-Numi-значение пропущено");
    }

    /// FR-020: slug имени шаблона — латиница/цифры/дефисы; кириллица
    /// транслитерируется («Нагрузка» → «nagruzka»).
    #[test]
    fn slugify_transliterates_cyrillic() {
        assert_eq!(slugify("My Custom LB"), "my-custom-lb");
        assert_eq!(slugify("Нагрузка"), "nagruzka");
        assert_eq!(slugify("БД SQL (мастер)"), "bd-sql-master");
        assert_eq!(slugify("  --weird name--  "), "weird-name");
        assert_eq!(slugify("!!!"), "custom", "нет символов — фолбэк custom");
    }

    /// FR-020: тип параметра по токену единицы.
    #[test]
    fn infer_param_type_maps_units() {
        use canvas_core::templates::ParamType;
        assert_eq!(infer_param_type(Some("rps")), ParamType::Rate);
        assert_eq!(infer_param_type(Some("ms")), ParamType::Time);
        assert_eq!(infer_param_type(Some("KB")), ParamType::Bytes);
        assert_eq!(infer_param_type(Some("req")), ParamType::Count);
        assert_eq!(infer_param_type(Some("%")), ParamType::Percent);
        assert_eq!(infer_param_type(None), ParamType::Scalar);
        assert_eq!(infer_param_type(Some("unknown")), ParamType::Scalar);
    }

    /// FR-020: уникальный id — без конфликтов; конфликт получает суффикс -2.
    #[test]
    fn unique_custom_id_avoids_conflicts() {
        let registry = canvas_core::templates::TemplateRegistry::empty();
        let root = std::env::temp_dir().join(format!(
            "canvasdesk-fr20-uid-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("root");
        let first = unique_custom_id("my-lb", &registry, &root);
        assert_eq!(first, "my-lb");
        // «Занято» в реестре → суффикс
        let manifest = canvas_core::templates::TemplateManifest {
            id: "my-lb".to_owned(),
            name: "x".to_owned(),
            name_ru: None,
            version: "1.0.0".to_owned(),
            category: "custom".to_owned(),
            description: String::new(),
            description_en: None,
            params: Vec::new(),
            outputs: Vec::new(),
            expr: "1".to_owned(),
            color: "#9B9B9B".to_owned(),
            icon: "custom".to_owned(),
            source: canvas_core::templates::TemplateSource::Custom,
        };
        canvas_core::templates::save_custom(&manifest, &root).expect("save");
        let registry = canvas_core::templates::TemplateRegistry::all_with_custom(&root);
        let second = unique_custom_id("my-lb", &registry, &root);
        assert_eq!(second, "my-lb-2");
        std::fs::remove_dir_all(&root).ok();
    }

    /// FR-013 (правка 5): РЕАЛЬНЫЙ флоу набора — EditingSession (как
    /// begin_editing), ввод через insert_text (путь вставки/набора), живые
    /// результаты — ТОЧНО та же формула, что в RedrawRequested; затем
    /// commit (текст сессии в модель + recompute_expr). Регресс корневого
    /// бага «выражения так ничего и не показывают»: emit экранировал
    /// одиночное `=` (`x = 200` → `x \= 200`), expr-парсер падал на `\`,
    /// переменные не объявлялись — лист владельца молчал ЦЕЛИКОМ и в
    /// редакторе, и на карточке. MCP-тесты (node_update_text) этого не
    /// ловили: они кладут в модель чистый текст, минуя markdown-канонику.
    #[test]
    fn live_typing_flow_owner_sheets() {
        use cosmic_text::FontSystem;
        fn run_session(text: &str) -> (String, Vec<Option<ExprOutcome>>) {
            let mut fs = FontSystem::new();
            let mut session =
                EditingSession::new(&mut fs, EditTarget::Node(0), "", 360.0, 228.0, 1.0);
            session.insert_text(&mut fs, text);
            let canonical = session.text();
            // Та же формула, что в RedrawRequested для живых результатов
            let live = expr::eval_lines(&canonical);
            (canonical, live)
        }
        fn expect_ok(line: &Option<ExprOutcome>, expected: &str, context: &str) {
            match line {
                Some(ExprOutcome::Ok(value)) => {
                    assert_eq!(value.to_string(), expected, "{context}")
                }
                other => panic!("{context}: ожидалось {expected}, получено {other:?}"),
            }
        }
        fn expect_err(line: &Option<ExprOutcome>, name: &str, context: &str) {
            match line {
                Some(ExprOutcome::Err(msg)) => {
                    assert!(msg.contains(name), "{context}: имя {name} в ошибке: {msg}")
                }
                other => panic!("{context}: ожидалась ошибка про {name}: {other:?}"),
            }
        }

        // Лист 1 владельца (латиница): c = a + b — ссылки на необъявленные
        // a/b — ВИДИМАЯ ошибка строки (правка 5), остальные — значения
        let (canonical, live) = run_session("x = 200\nc = a + b\n200 + x");
        assert_eq!(canonical, "x = 200\nc = a + b\n200 + x", "emit без \\=");
        expect_ok(&live[0], "200", "присваивание x");
        expect_err(&live[1], "a", "необъявленная a в присваивании");
        expect_ok(&live[2], "400", "ссылка на x");

        // Та же раскладка кириллицей (русская раскладка владельца)
        let (_, live) = run_session("х = 200\nс = а + б\n200 + х");
        expect_ok(&live[0], "200", "кириллическое присваивание");
        expect_err(&live[1], "а", "необъявленная а");
        expect_ok(&live[2], "400", "ссылка на х");

        // Лист 2 владельца: x-умножение и ссылки
        let (canonical, live) = run_session("a=25+35x20\nb = 2\na+b");
        assert_eq!(canonical, "a=25+35x20\nb = 2\na+b");
        expect_ok(&live[0], "725", "a = 25+35x20");
        expect_ok(&live[1], "2", "b = 2");
        expect_ok(&live[2], "727", "a+b");

        // COMMIT: канонический текст сессии попадает в модель, построчные
        // результаты совпадают с живыми (карточка после клика мимо ноды)
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("n1", "", 0.0, 0.0));
        let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/live-flow.canvas"));
        let (canonical, live) =
            run_session("123 + 5123 = a\n235 + 2323 = b\nx = 200\nc = a + b\n200 + x");
        scene.canvas.nodes[0].text = Some(canonical.clone());
        scene.recompute_expr("n1");
        let committed = scene
            .expr_line_results
            .get("n1")
            .expect("построчные результаты после commit");
        assert_eq!(committed.len(), live.len());
        expect_ok(
            &committed[0],
            "5\u{a0}246",
            "commit: хвостовое присваивание",
        );
        expect_ok(&committed[1], "2\u{a0}558", "commit: b");
        expect_ok(&committed[2], "200", "commit: x");
        expect_ok(&committed[3], "7\u{a0}804", "commit: c = a + b");
        expect_ok(&committed[4], "400", "commit: 200 + x");
        // Совместимость: заметка прежних сборок с `\=` в модели оживает
        scene.canvas.nodes[0].text = Some("x \\= 200\n200 + x".to_owned());
        scene.recompute_expr("n1");
        let committed = scene
            .expr_line_results
            .get("n1")
            .expect("результаты для старой каноники");
        expect_ok(&committed[0], "200", "старая заметка: присваивание с \\=");
        expect_ok(&committed[1], "400", "старая заметка: ссылка");
    }

    /// FR-013 (правка 4): hit-тест зон наведения бейджей ошибок.
    #[test]
    fn expr_error_tooltip_hit_test() {
        let hits = vec![
            LineErrorHit {
                rect: [360.0, 34.0, 25.0, 36.0],
                message: "единицы не совместимы: 1 sec и 2 req".to_owned(),
            },
            LineErrorHit {
                rect: [360.0, 54.0, 25.0, 36.0],
                message: "деление на ноль".to_owned(),
            },
        ];
        // Внутри первой зоны
        assert_eq!(
            expr_error_hit_at(&hits, [372.0, 40.0]).map(|hit| &*hit.message),
            Some("единицы не совместимы: 1 sec и 2 req"),
        );
        // Внутри второй зоны (первая кончается на y=70 — берём точку ниже)
        assert_eq!(
            expr_error_hit_at(&hits, [378.0, 80.0]).map(|hit| &*hit.message),
            Some("деление на ноль"),
        );
        // Мимо всех зон
        assert!(
            expr_error_hit_at(&hits, [100.0, 40.0]).is_none(),
            "мимо по x"
        );
        assert!(
            expr_error_hit_at(&hits, [372.0, 200.0]).is_none(),
            "мимо по y"
        );
        // Пустой набор зон
        assert!(expr_error_hit_at(&[], [372.0, 40.0]).is_none());
    }

    /// FR-050 Н9-2 (этап D): hit-тест зон пролитых строк (параметр с
    /// toParam / авто-строка приёмника) — данные тултипа источника.
    #[test]
    fn spill_hit_at_picks_rect() {
        use canvas_render::text::SpillHitKind;
        let hits = vec![
            SpillHit {
                rect: [360.0, 30.0, 25.0, 18.0],
                kind: SpillHitKind::Param {
                    param: "rps".to_owned(),
                    path: "Трафик.peak_rps".to_owned(),
                    value: Some("1389 rps".to_owned()),
                    local: Some("50 rps".to_owned()),
                },
                node: 3,
            },
            SpillHit {
                rect: [360.0, 54.0, 25.0, 18.0],
                kind: SpillHitKind::AutoRow {
                    path: "Курсы.usd".to_owned(),
                    slot: 1,
                    value: Some("92.5".to_owned()),
                    template: false,
                    edge_id: "e-9".to_owned(),
                },
                node: 4,
            },
        ];
        // Внутри первой зоны — данные параметра
        assert!(matches!(
            spill_hit_at(&hits, [372.0, 38.0]).map(|hit| &hit.kind),
            Some(SpillHitKind::Param { ref param, .. }) if param == "rps"
        ));
        // Внутри второй зоны — данные авто-строки
        assert!(matches!(
            spill_hit_at(&hits, [378.0, 60.0]).map(|hit| &hit.kind),
            Some(SpillHitKind::AutoRow { ref path, slot, .. }) if path == "Курсы.usd" && *slot == 1
        ));
        // Мимо всех зон
        assert!(spill_hit_at(&hits, [100.0, 40.0]).is_none(), "мимо по x");
        assert!(spill_hit_at(&hits, [372.0, 200.0]).is_none(), "мимо по y");
        // Пустой набор зон
        assert!(spill_hit_at(&[], [372.0, 40.0]).is_none());
    }

    // --- FR-014: поток значений по рёбрам ---

    /// Сцена потока: A «1200 + 480», B «$in / 3», ребро A→B (control).
    fn flow_scene() -> SceneState {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("fa", "A\n1200 + 480", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::text("fb", "B\n$in / 3", 500.0, 0.0));
        canvas.add_edge(Edge::new("e-ab", "fa", None, "fb", Some(Side::Left)));
        SceneState::new(canvas, PathBuf::from("target/tmp/flow.canvas"))
    }

    /// Правка 3 (регрессия UI-переключателя): тогл через палитру
    /// (SceneState::toggle_edge_flow) применяет flow.kind к ЖИВОМУ канвасу
    /// (раньше мутировался клон-снимок — переключатель не работал),
    /// копит undo-шаг «до» и пересчитывает downstream.
    #[test]
    fn palette_flow_toggle_applies_to_live_canvas() {
        let mut scene = flow_scene();
        // Тогл Control → Value применён к живому канвасу
        let applied = scene
            .toggle_edge_flow(0, FlowKind::Value)
            .expect("тогл без цикла");
        assert!(applied, "тогл должен примениться");
        assert_eq!(scene.canvas.edges[0].flow_kind(), FlowKind::Value);
        // Живой пересчёт: B = 1680 / 3 = 560 (вход пришёл по value-ребру)
        let b = scene
            .expr_results
            .get("fb")
            .expect("результат B после тогла");
        match b {
            ExprOutcome::Ok(value) => assert!((value.num - 560.0).abs() < 1e-9, "{value:?}"),
            ExprOutcome::Err(err) => panic!("B не должен иметь ошибку: {err}"),
        }
        // Undo-шаг «до» тогла (FR-006): снят ДО мутации, а не после
        assert_eq!(scene.undo_stack.len(), 1, "один undo-шаг");
        let before = scene.undo_stack[0].clone();
        assert_eq!(
            before.edges[0].flow_kind(),
            FlowKind::Control,
            "снимок «до» — control"
        );
        // Round-trip в файл: kind=value сохраняется
        let json = scene.canvas.to_json().expect("сериализация");
        assert!(json.contains("\"value\""), "flow.kind в файле: {json}");
    }

    /// Правка 3: обратный тогл Value → Control — поле удаляется целиком,
    /// downstream теряет вход (MissingInbound), undo-шаг копится.
    #[test]
    fn palette_flow_toggle_back_removes_field_and_breaks_input() {
        let mut scene = flow_scene();
        scene
            .toggle_edge_flow(0, FlowKind::Value)
            .expect("первый тогл");
        let applied = scene
            .toggle_edge_flow(0, FlowKind::Control)
            .expect("обратный тогл");
        assert!(applied);
        assert_eq!(scene.canvas.edges[0].flow_kind(), FlowKind::Control);
        assert!(
            scene.canvas.edges[0].extra.get("canvasdesk").is_none(),
            "control удаляет расширение целиком (как MCP)"
        );
        // Downstream потерял вход — у B ошибка MissingInbound
        match scene.expr_results.get("fb") {
            Some(ExprOutcome::Err(msg)) => {
                assert!(msg.contains("вход"), "ожидался missing inbound: {msg}");
            }
            other => panic!("ожидалась ошибка входа у B, получено: {other:?}"),
        }
        assert_eq!(scene.undo_stack.len(), 2, "два undo-шага (туда-обратно)");
    }

    /// Правка 3: no-op-варианты не копят шаг и не меняют модель — связи
    /// нет, тип уже такой.
    #[test]
    fn palette_flow_toggle_noop_cases() {
        let mut scene = flow_scene();
        // Несуществующая связь
        let applied = scene
            .toggle_edge_flow(7, FlowKind::Value)
            .expect("нет связи — Ok(false)");
        assert!(!applied);
        assert!(scene.undo_stack.is_empty());
        // Повторный тогл в тот же тип — no-op
        scene
            .toggle_edge_flow(0, FlowKind::Value)
            .expect("первый тогл");
        let snapshot = scene.canvas.clone();
        let applied = scene
            .toggle_edge_flow(0, FlowKind::Value)
            .expect("уже value — Ok(false)");
        assert!(!applied);
        assert_eq!(scene.canvas, snapshot, "модель не изменилась");
        assert_eq!(scene.undo_stack.len(), 1, "второй шаг не копится");
    }

    /// Правка 3: тогл в Value, замыкающий цикл, отклонён с участниками
    /// (Err), модель не меняется (DAG-инвариант в UI-пути).
    #[test]
    fn palette_flow_toggle_rejects_cycle() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("fa", "5", 0.0, 0.0));
        canvas.nodes.push(Node::text("fb", "$in", 500.0, 0.0));
        // A → B уже value, обратная связь B → A — control
        canvas.add_edge(Edge::new("e-ab", "fa", None, "fb", Some(Side::Left)));
        canvas.add_edge(Edge::new("e-ba", "fb", None, "fa", Some(Side::Right)));
        canvas.edges[0].set_flow_kind(FlowKind::Value);
        let mut scene = SceneState::new(canvas, PathBuf::from("target/tmp/flow-cycle.canvas"));
        // Тогл B → A в value замкнул бы цикл A → B → A
        let result = scene.toggle_edge_flow(1, FlowKind::Value);
        let participants = result.expect_err("цикл должен быть отклонён");
        assert_eq!(participants, vec!["fa".to_owned(), "fb".to_owned()]);
        // Модель не изменилась
        assert_eq!(scene.canvas.edges[1].flow_kind(), FlowKind::Control);
        assert!(scene.undo_stack.is_empty(), "отклонённый тогл без шага");
    }

    /// FR-042/FR-044 (регрессия «каши» в stage, дефект скриншота): срез
    /// хранится в stage-локальных px БЕЗ масштаба — [`StageTransform`]
    /// применяет scale ровно ОДИН раз: позиция ноды на экране совпадает с
    /// раскладкой `stage_layout`, размеры сжаты тем же коэффициентом,
    /// обе колонки умещаются в rect, а веер стартует у порта истока
    /// (рендер и hit-test — в одной системе координат).
    #[test]
    fn stage_slice_is_stage_local_and_fits_rect() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("fa", "исток", 0.0, 0.0);
        a.width = 420.0;
        a.height = 240.0;
        let mut b = Node::text("fb", "приёмник", 600.0, 0.0);
        b.width = 360.0;
        b.height = 200.0;
        canvas.nodes.push(a);
        canvas.nodes.push(b);
        canvas.add_edge(Edge::new("e1", "fa", None, "fb", Some(Side::Left)));
        canvas.add_edge(Edge::new("e2", "fa", None, "fb", Some(Side::Left)));
        canvas.add_edge(Edge::new("e3", "fa", None, "fb", Some(Side::Left)));
        let index = canvas_core::EdgeBundleIndex::build(&canvas);
        assert!(index.bundle_of_edge(0).is_some(), "пучок собран");
        let mut stage = MainStageState::open(&canvas, &index, 0).expect("stage открыт");
        // Тесный вьюпорт → масштаб сжатия < 1 (инвариант умещения)
        let viewport = [900.0_f32, 600.0];
        let layout = stage.relayout(viewport);
        assert!(layout.scale < 1.0, "узкий viewport требует сжатия");
        let rect = main_stage_rect(viewport);
        let transform = StageTransform::new([rect.x, rect.y], layout.scale);
        // Позиция истока на экране = раскладке (масштаб применён один раз)
        let p = transform.map_point([stage.slice.nodes[0].x, stage.slice.nodes[0].y]);
        assert!(
            (p[0] - (rect.x + layout.source_pos[0])).abs() < 0.01,
            "x истока"
        );
        assert!(
            (p[1] - (rect.y + layout.source_pos[1])).abs() < 0.01,
            "y истока"
        );
        // Размеры сжаты тем же масштабом, что и позиции
        assert!(
            (transform.map_size(stage.slice.nodes[0].width) - layout.scale * 420.0).abs() < 0.01,
            "ширина карточки сжата тем же scale"
        );
        // Правый край приёмника внутри rect main stage
        let right = transform.map_point([stage.slice.nodes[1].x, 0.0])[0]
            + transform.map_size(stage.slice.nodes[1].width);
        assert!(right <= rect.x + rect.w + 0.01, "приёмник умещается в rect");
        // Веер согласован с карточками: полилиния стартует у правого края
        // истока (якоря на строках значений — та же чистая функция, что
        // в рендере)
        let metrics = StageMetrics::default();
        let lines = stage_edge_geometry(&stage.slice, &metrics, [false, false], 24);
        let p0 = transform.map_point(lines[0].points[0]);
        let src_port_x = stage.slice.nodes[0].x + stage.slice.nodes[0].width;
        assert!(
            (p0[0] - transform.map_point([src_port_x, 0.0])[0]).abs() < 2.0,
            "веер стартует у правого порта истока"
        );
    }

    /// Хелпер FR-044: App на заглушках с готовым канвасом и тестовым
    /// вьюпортом (клики по screen-space UI без winit-окна).
    #[cfg(test)]
    fn stub_app_with_canvas(canvas: Canvas) -> App {
        let scene = SceneState::new(canvas, PathBuf::from("target/tmp/fr044-stage.canvas"));
        let cache_dir =
            std::env::temp_dir().join(format!("canvasdesk-fr044-{}", std::process::id()));
        std::fs::create_dir_all(&cache_dir).expect("tmp cache dir");
        let (search_responder, _rx) = {
            let (tx, rx) = std::sync::mpsc::channel();
            let responder: canvas_core::SearchResponder = std::sync::Arc::new(move |event| {
                let _ = tx.send(event);
            });
            (responder, rx)
        };
        let mut app = App::new(
            scene,
            Box::new(canvas_core::NoopThumbs),
            Settings::default(),
            None,
            Some(cache_dir),
            std::sync::Arc::new(|_event: canvas_core::DragEvent| {}),
            std::sync::Arc::new(|_event: canvas_widgets::WidgetEvent| {}),
            Box::new(canvas_core::NoopWatch),
            Box::new(canvas_core::MemSearch::new(search_responder)),
            Box::new(canvas_core::NoopClipboard),
            Some(Box::new(canvas_core::MemWidgetState::default())),
            false,
            Box::new(canvas_render::renderer_init::NoopRendererLaunch),
        );
        app.test_viewport = Some([1200.0_f32, 800.0]);
        app
    }

    /// Демо-канвас FR-044 (оракул инварианта 6 CR): пучок Заявки→Отчёт ×2
    /// (users, conv) + внешний вход Цены.usd; формулы приёмника с
    /// qualified-путями именованного синтаксиса (FR-050 Р-6).
    #[cfg(test)]
    fn fr044_demo_canvas() -> (Canvas, usize, usize) {
        let mut canvas = Canvas::default();
        let mut src = Node::text("src", "Заявки\nusers = 10\nconv = 0.2", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        let mut ext = Node::text("ext", "Цены\nusd = 90", 0.0, 300.0);
        ext.width = 300.0;
        ext.height = 140.0;
        let mut dst = Node::text(
            "dst",
            "Отчёт\nx = Заявки.users * Заявки.conv * Цены.usd\ny = Заявки.users + Цены.usd",
            700.0,
            0.0,
        );
        dst.width = 420.0;
        dst.height = 220.0;
        canvas.nodes.push(src);
        canvas.nodes.push(ext);
        canvas.nodes.push(dst);
        let mut e1 = Edge::new("e1", "src", None, "dst", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_output = Some("users".to_owned());
        let mut e2 = Edge::new("e2", "src", None, "dst", None);
        e2.set_flow_kind(FlowKind::Value);
        e2.from_output = Some("conv".to_owned());
        let mut e3 = Edge::new("e3", "ext", None, "dst", None);
        e3.set_flow_kind(FlowKind::Value);
        e3.from_output = Some("usd".to_owned());
        canvas.edges.push(e1);
        canvas.edges.push(e2);
        canvas.edges.push(e3);
        (canvas, 0, 1) // пучок — рёбра 0/1; внешнее — 2
    }

    /// FR-044 Р-5/инвариант 6 (интеграция): клик по строке формулы в
    /// панели «Как считается» — подсветка ровно множества её операнд-
    /// рёбер и переменных (3 ребра + 3 переменные в демо-оракуле CR);
    /// значения в модели — из активного потока (named-выходы).
    #[test]
    fn stage_calc_panel_click_formula_focuses_operands() {
        let (canvas, _e1, _e2) = fr044_demo_canvas();
        let mut app = stub_app_with_canvas(canvas);
        app.scene.recompute_flow();
        let index = canvas_core::EdgeBundleIndex::build(&app.scene.canvas);
        app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
        let viewport = app.viewport_logical();
        let rect = main_stage_rect(viewport);
        let stage = app.main_stage.as_ref().expect("stage открыт");
        let s = stage.scale.max(f32::EPSILON);
        let ctx = app.stage_frame_ctx(stage, &rect, s);
        // Р-8: внешний вход Цены.usd в модели (панель полная)
        assert_eq!(ctx.model.vars.len(), 3);
        assert!(!ctx.model.vars[2].in_bundle);
        assert_eq!(ctx.model.ext_sources.len(), 1);
        // Значение по именованному выходу (исправление stage_edge_value_text)
        assert_eq!(
            ctx.model.vars[0].value,
            calc_panel_ui::RowValue::Ok("10".to_owned())
        );
        let panel = ctx.panel.as_ref().expect("панель построена");
        assert_eq!(panel.formula_rows.len(), 2);
        // Клик по строке формулы 0
        let f = panel.formula_rows[0].1;
        app.cursor = [rect.x + f[0] + 30.0, rect.y + f[1] + 5.0];
        app.click_main_stage();
        let focus = app.stage_calc_focus.as_ref().expect("фокус зафиксирован");
        assert_eq!(
            focus.edges,
            std::collections::BTreeSet::from([0, 1, 2]),
            "ровно операнд-рёбра формулы (инвариант 6)"
        );
        assert_eq!(
            focus.rows,
            std::collections::BTreeSet::from([0, 1, 2, ctx.model.vars.len()]),
            "формула + её три переменные"
        );
        assert!(
            app.selected.is_none(),
            "клик по строке панели не выделяет ребро"
        );
    }

    /// FR-044 Р-5/инвариант 7 (интеграция): клик по переменной — обратная
    /// навигация: переменная + её ребро + все формулы, где она участвует;
    /// клик по пилюле веера — выделение ребра + синхронная подсветка.
    #[test]
    fn stage_calc_click_variable_and_pill_sync() {
        let (canvas, _e1, _e2) = fr044_demo_canvas();
        let mut app = stub_app_with_canvas(canvas);
        app.scene.recompute_flow();
        let index = canvas_core::EdgeBundleIndex::build(&app.scene.canvas);
        app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
        let viewport = app.viewport_logical();
        let rect = main_stage_rect(viewport);
        let stage = app.main_stage.as_ref().expect("stage открыт");
        let s = stage.scale.max(f32::EPSILON);
        let ctx = app.stage_frame_ctx(stage, &rect, s);
        let panel = ctx.panel.as_ref().expect("панель построена");
        // Геометрия кликов — до мутаций (заимствование stage)
        let var_click = {
            let v = panel.var_rows[0].1;
            [rect.x + v[0] + 20.0, rect.y + v[1] + 5.0]
        };
        let (pill_item, pill_center) = {
            let pills = app.stage_pill_rects(stage, &ctx);
            let (item, pill_rect) = pills
                .iter()
                .copied()
                .find(|(item, _)| stage.edges[*item] == 1)
                .expect("пилюля ребра 1");
            let transform = StageTransform::new([rect.x, rect.y], stage.scale);
            let center = transform.map_point([
                pill_rect.x + pill_rect.w / 2.0,
                pill_rect.y + pill_rect.h / 2.0,
            ]);
            (item, center)
        };
        // Обратная навигация: users (строка 0) участвует в обеих формулах
        app.cursor = var_click;
        app.click_main_stage();
        let focus = app.stage_calc_focus.as_ref().expect("фокус зафиксирован");
        assert_eq!(focus.edges, std::collections::BTreeSet::from([0]));
        assert_eq!(
            focus.rows,
            std::collections::BTreeSet::from([0, ctx.model.vars.len(), ctx.model.vars.len() + 1]),
            "переменная + обе формулы (инвариант 7)"
        );
        // Клик по пилюле ребра 1 (conv): выделение + синхронная подсветка
        app.cursor = pill_center;
        app.click_main_stage();
        assert_eq!(
            app.selected,
            Some(Selection::Edge(1)),
            "пилюля = выделение ребра (live-индекс, item = {pill_item})"
        );
        let focus = app.stage_calc_focus.as_ref().expect("фокус синхронен");
        assert_eq!(focus.edges, std::collections::BTreeSet::from([1]));
        assert!(
            focus.rows.contains(&(ctx.model.vars.len())),
            "связанная строка формулы подсвечена"
        );
    }

    /// FR-044 Р-7 (интеграция): Esc-каскад — первое Esc гасит подсветку
    /// (stage открыт), второе закрывает stage; клик по фону stage —
    /// сброс подсветки без закрытия; закрытие stage гасит подсветку
    /// (инвариант 8: состояние не переживает stage).
    #[test]
    fn stage_calc_esc_cascade_and_background_reset() {
        let (canvas, _e1, _e2) = fr044_demo_canvas();
        let mut app = stub_app_with_canvas(canvas);
        app.scene.recompute_flow();
        let index = canvas_core::EdgeBundleIndex::build(&app.scene.canvas);
        app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
        let viewport = app.viewport_logical();
        let rect = main_stage_rect(viewport);
        let stage = app.main_stage.as_ref().expect("stage открыт");
        let s = stage.scale.max(f32::EPSILON);
        let ctx = app.stage_frame_ctx(stage, &rect, s);
        let panel = ctx.panel.as_ref().expect("панель построена");
        // Фиксация подсветки кликом по строке формулы
        let f = panel.formula_rows[0].1;
        app.cursor = [rect.x + f[0] + 30.0, rect.y + f[1] + 5.0];
        app.click_main_stage();
        assert!(app.stage_calc_focus.is_some(), "подсветка активна");
        // Первое Esc — гасит подсветку, stage остаётся
        assert!(app.dispatch_esc(ui_registry::id::STAGE), "Esc поглощён");
        assert!(app.stage_calc_focus.is_none(), "подсветка сброшена");
        assert!(app.main_stage.is_some(), "stage открыт (Р-7)");
        // Второе Esc — закрывает stage
        assert!(app.dispatch_esc(ui_registry::id::STAGE), "Esc поглощён");
        assert!(app.main_stage.is_none(), "stage закрыт");
        assert!(app.stage_calc_hover.is_none(), "превью тоже погасло");
        // Снова: подсветка → клик по фону stage (не панель, не пилюля,
        // не линия — верх stage над веером) — сброс БЕЗ закрытия
        app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
        let stage = app.main_stage.as_ref().expect("stage открыт");
        let ctx = app.stage_frame_ctx(stage, &rect, s);
        let panel = ctx.panel.as_ref().expect("панель");
        let f = panel.formula_rows[0].1;
        app.cursor = [rect.x + f[0] + 30.0, rect.y + f[1] + 5.0];
        app.click_main_stage();
        assert!(app.stage_calc_focus.is_some());
        app.cursor = [rect.x + 60.0, rect.y + 70.0];
        app.click_main_stage();
        assert!(
            app.stage_calc_focus.is_none(),
            "фон stage — сброс подсветки"
        );
        assert!(
            app.main_stage.is_some(),
            "stage не закрывается фоном внутри"
        );
        // Закрытие stage гасит подсветку (инвариант 8)
        app.stage_calc_focus = Some(StageCalcFocus::default());
        app.close_main_stage();
        assert!(app.stage_calc_focus.is_none() && app.stage_calc_hover.is_none());
    }

    /// FR-073 (интеграция): драг через соседа — вытеснение с жёстким
    /// зазором, drop на место соседа перезакрепляет его якорь, расселение
    /// закрывает сессию физики.
    #[test]
    fn drag_push_displaces_rebases_and_settles() {
        let mut canvas = Canvas::default();
        for (id, x) in [("a", 0.0), ("b", 260.0)] {
            let mut n = Node::text(id, id, x, 0.0);
            n.width = 200.0;
            n.height = 80.0;
            canvas.nodes.push(n);
        }
        let mut app = stub_app_with_canvas(canvas);
        // Старт drag (путь on_cursor_pressed): якоря = текущие позиции
        app.drag_push_begin();
        app.dragging = Some(DragState {
            primary: 0,
            grab_world: [0.0, 0.0],
            origins: vec![(0, [0.0, 0.0])],
        });
        // Кадр ввода привёл активную внахлёст с "b"
        app.scene.canvas.nodes[0].x = 240.0;
        let params = app.drag_push_params();
        // Кадры физики: b вытесняется (MTV — кратчайший выход, любая ось),
        // 2D-зазор активная↔пассивная не ниже halo+gap
        for _ in 0..30 {
            app.tick_drag_push();
            let (a, b) = (&app.scene.canvas.nodes[0], &app.scene.canvas.nodes[1]);
            let gx = (b.x - (a.x + a.width)).max(a.x - (b.x + b.width));
            let gy = (b.y - (a.y + a.height)).max(a.y - (b.y + b.height));
            let clearance = gx.max(gy);
            assert!(
                clearance >= params.halo + params.gap - 0.5,
                "зазор {clearance} < {}",
                params.halo + params.gap
            );
        }
        // Drop на место "b": активная закреплена где брошена, накрытая b
        // получает новый якорь
        let active = app.drag_push_active();
        canvas_core::drag_push::commit_drop(
            &app.scene.canvas,
            &active,
            &mut app.drag_push,
            &params,
        );
        app.dragging = None;
        assert_eq!(app.drag_push.anchors["a"], [240.0, 0.0]);
        assert!(
            app.drag_push.anchors["b"][0] > 260.0 || app.drag_push.anchors["b"][1] != 0.0,
            "накрытая b перезакреплена: {:?}",
            app.drag_push.anchors["b"]
        );
        // Расселение: кадры до успокоения, сессия закрывается
        let mut guard = 0;
        while app.drag_push_animating() {
            app.tick_drag_push();
            guard += 1;
            assert!(guard < 600, "расселение не завершается");
        }
        // Завершающий тик: если расселось само (animating=false сразу),
        // сессия закрывается следующим холостым кадром физики
        assert!(!app.tick_drag_push(), "тик вне сессии — no-op");
        assert!(!app.drag_push_live, "сессия закрыта после расселения");
    }

    /// FR-044 Q3 (интеграция): анимация перехода подсветки — включение
    /// наращивает коэффициент затемнения (кадры до завершения), фейд-аут
    /// гасит его до нуля, снимок множества живёт до конца перехода;
    /// закрытие stage гасит мгновенно (инвариант 8).
    #[test]
    fn stage_calc_focus_fade_animation() {
        let (canvas, _e1, _e2) = fr044_demo_canvas();
        let mut app = stub_app_with_canvas(canvas);
        app.scene.recompute_flow();
        let index = canvas_core::EdgeBundleIndex::build(&app.scene.canvas);
        app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
        let viewport = app.viewport_logical();
        let rect = main_stage_rect(viewport);
        let focus_set = |app: &App| -> StageCalcFocus {
            let stage = app.main_stage.as_ref().expect("stage открыт");
            let ctx = app.stage_frame_ctx(stage, &rect, stage.scale.max(f32::EPSILON));
            let _panel = ctx.panel.as_ref().expect("панель построена");
            StageCalcFocus::for_formula(&ctx.model, 0)
        };
        // Включение: тик стартует переход — коэффициент в [0, 1)
        app.stage_calc_focus = Some(focus_set(&app));
        assert!(app.tick_stage_calc_fade(), "переход идёт — кадры нужны");
        let (render_set, dim) = app.stage_calc_render.clone();
        assert!(render_set.is_some(), "множество применено с первого тика");
        assert!((0.0..1.0).contains(&dim), "коэффициент анимируется: {dim}");
        // Догоняем переход (реальное время focus_fade_ms = 150 мс)
        let mut guard = 0;
        while app.tick_stage_calc_fade() {
            guard += 1;
            assert!(guard < 100_000, "переход не завершается");
        }
        let (_, dim) = app.stage_calc_render.clone();
        assert!((dim - 1.0).abs() < f32::EPSILON, "устаканилось на 1.0");
        assert!(!app.stage_calc_fade_animating());
        // Сброс: фейд-аут — снимок множества держится до конца перехода
        app.stage_calc_focus = None;
        app.stage_calc_hover = None;
        assert!(app.tick_stage_calc_fade(), "фейд-аут идёт");
        let (snapshot, dim) = app.stage_calc_render.clone();
        assert!(snapshot.is_some(), "снимок множества живёт в фейд-ауте");
        assert!(dim <= 1.0, "коэффициент уходит от 1.0: {dim}");
        let mut guard = 0;
        while app.tick_stage_calc_fade() {
            guard += 1;
            assert!(guard < 100_000, "фейд-аут не завершается");
        }
        let (snapshot, dim) = app.stage_calc_render.clone();
        assert!(snapshot.is_none(), "множество очищено после фейд-аута");
        assert!(dim.abs() < f32::EPSILON, "коэффициент нулевой: {dim}");
        // Закрытие stage — мгновенный сброс рендер-состояния
        app.stage_calc_focus = Some(focus_set(&app));
        let _ = app.tick_stage_calc_fade();
        app.close_main_stage();
        assert_eq!(app.stage_calc_render, (None, 0.0));
        assert!(!app.stage_calc_fade_animating());
    }

    /// FR-044 Р-3-а (интеграция): лейблы слотов — у value-ребра с адресацией
    /// подпись истока = «out: <имя>» + значение, у приёмника — путь;
    /// control-ребро — «управление» у приёмника, без значения/подписи у
    /// истока и без value-пути в пилюле (инвариант 5).
    #[test]
    fn stage_slot_labels_output_and_control() {
        let mut canvas = Canvas::default();
        let mut src = Node::text("src", "Заявки\nusers = 10", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        let mut dst = Node::text("dst", "Отчёт\nx = 1", 700.0, 0.0);
        dst.width = 420.0;
        dst.height = 200.0;
        canvas.nodes.push(src);
        canvas.nodes.push(dst);
        let mut e1 = Edge::new("e1", "src", None, "dst", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_output = Some("users".to_owned());
        let mut e2 = Edge::new("e2", "src", None, "dst", None);
        e2.set_flow_kind(FlowKind::Control);
        canvas.edges.push(e1);
        canvas.edges.push(e2);
        let mut app = stub_app_with_canvas(canvas);
        app.scene.recompute_flow();
        let index = canvas_core::EdgeBundleIndex::build(&app.scene.canvas);
        app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
        assert!(app.main_stage.is_some(), "пучок value+control открыт");
        // Value-ребро: подпись истока — слот выхода + значение; приёмник — путь
        let e1 = &app.scene.canvas.edges[0];
        let (slot, value) = app.stage_src_label_lines(e1);
        assert_eq!(slot.as_deref(), Some("out: users"), "лейбл слота выхода");
        assert_eq!(value, "10", "значение по именованному выходу");
        assert_eq!(
            app.stage_dst_label_text(e1, "Заявки"),
            "Заявки.users",
            "лейбл слота входа — полный путь (Р-3)"
        );
        // Control-ребро: у истока подписи нет; у приёмника — «управление»;
        // в пилюле — «to: <приёмник>», значения нет (инвариант 5)
        let e2 = &app.scene.canvas.edges[1];
        assert_eq!(
            app.stage_src_label_lines(e2),
            (None, String::new()),
            "control не несёт значения/подписи у истока"
        );
        assert_eq!(
            app.stage_dst_label_text(e2, "Заявки"),
            "управление",
            "control-ребро не отображается value-путём"
        );
        assert_eq!(app.stage_edge_value_text(e2), "", "control без значения");
        assert_eq!(
            app.stage_edge_addr_text(e2),
            "to: Отчёт",
            "адрес control-ребра — «to: <приёмник>»"
        );
    }

    /// FR-045 F-5 v1: лейблы портов канваса — qualified-адресация (R-5):
    /// построчный порт — «Объект.строка N» (1-based), сторонный порт и
    /// футер шаблона — «out: Объект»; объект — единая точка dataref
    /// (первая строка текста, дословно).
    #[test]
    fn port_label_line_and_out_qualified() {
        let mut canvas = Canvas::default();
        let mut src = Node::text("src", "Заявки\nusers = 10\nconv = 0.2", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        canvas.nodes.push(src);
        let app = stub_app_with_canvas(canvas);
        assert_eq!(
            app.port_label_for(0, &PortTarget::Line(Some(0))),
            Some("Заявки.строка 1".to_owned()),
            "построчный порт — полный путь (R-5: полный путь в тултипе)"
        );
        assert_eq!(
            app.port_label_for(0, &PortTarget::Line(Some(1))),
            Some("Заявки.строка 2".to_owned())
        );
        assert_eq!(
            app.port_label_for(0, &PortTarget::Out),
            Some("out: Заявки".to_owned()),
            "сторонный порт — «out: <объект>» (F-5 «out:<имя>»)"
        );
        assert_eq!(
            app.port_label_for(0, &PortTarget::Line(None)),
            Some("out: Заявки".to_owned()),
            "футер шаблона — значение ноды целиком"
        );
    }

    /// FR-045 F-5 v1: якорь параметра шаблонной ноды — «to: Объект.Параметр»;
    /// EN-язык — «line N» в построчном лейбле (i18n FR-040, «out:»/«to:»
    /// одинаковы в обоих языках).
    #[test]
    fn port_label_param_and_english() {
        let mut canvas = Canvas::default();
        let mut tpl = Node::text("tpl", "", 0.0, 0.0);
        tpl.width = 420.0;
        tpl.height = 200.0;
        tpl.set_template(Some(canvas_core::templates::TemplateRef {
            id: "mock.lb".to_owned(),
            version: "1.0".to_owned(),
            expr: "mm1($rps)".to_owned(),
            params: std::collections::BTreeMap::new(),
            icon: "lb".to_owned(),
            color: "#4A90E2".to_owned(),
            name: Some("Балансировщик".to_owned()),
            outputs: Vec::new(),
        }));
        canvas.nodes.push(tpl);
        let app = stub_app_with_canvas(canvas);
        assert_eq!(
            app.port_label_for(0, &PortTarget::Param("rps".to_owned())),
            Some("to: Балансировщик.rps".to_owned()),
            "якорь параметра — «to: <полный путь>» (F-5 «to:<param>»)"
        );
        // EN: поле строки локализовано, объект — дословно
        let mut canvas_en = Canvas::default();
        let mut src = Node::text("src", "Заявки\nusers = 10", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        canvas_en.nodes.push(src);
        let mut app_en = stub_app_with_canvas(canvas_en);
        app_en.settings.language = Language::En;
        assert_eq!(
            app_en.port_label_for(0, &PortTarget::Line(Some(0))),
            Some("Заявки.line 1".to_owned()),
            "EN — «line N» (STAGE_LINE_LABEL)"
        );
        assert_eq!(
            app_en.port_label_for(0, &PortTarget::Out),
            Some("out: Заявки".to_owned())
        );
    }

    /// FR-045 F-5 v1: коллизия отображаемых имён — дискриминатор
    /// «Имя (node_id)» из dataref (§Q2, единая точка сборки).
    #[test]
    fn port_label_name_collision_fallback() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("a", "Заявки\nx = 1", 0.0, 0.0);
        a.width = 420.0;
        a.height = 200.0;
        let mut b = Node::text("b", "Заявки\ny = 2", 500.0, 0.0);
        b.width = 420.0;
        b.height = 200.0;
        canvas.nodes.push(a);
        canvas.nodes.push(b);
        let app = stub_app_with_canvas(canvas);
        assert_eq!(
            app.port_label_for(0, &PortTarget::Line(Some(0))),
            Some("Заявки (a).строка 1".to_owned()),
            "коллизия имён — дискриминатор node_id (§Q2)"
        );
        assert_eq!(
            app.port_label_for(1, &PortTarget::Out),
            Some("out: Заявки (b)".to_owned())
        );
    }

    /// FR-045 F-5 v2: входные слоты стороны — qualified-истоки (R-5):
    /// `fromLine` — «строка N» (i18n, 1-based), `fromOutput` — имя выхода,
    /// без адресации — fallback edge.id; unmapped-исток (Р-3,
    /// `scene.unmapped_edges`) — маркер «не подставлено» и янтарный тон.
    #[test]
    fn port_in_label_lines_qualified_and_unmapped() {
        let mut canvas = Canvas::default();
        let mut src = Node::text("src", "Заявки\nusers = 10\nconv = 0.2", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        let mut dst = Node::text("dst", "Отчёт\nx = 1", 700.0, 0.0);
        dst.width = 420.0;
        dst.height = 200.0;
        canvas.nodes.push(src);
        canvas.nodes.push(dst);
        let mut e1 = Edge::new("e1", "src", None, "dst", None);
        e1.set_flow_kind(FlowKind::Value);
        e1.from_line = Some(0);
        let mut e2 = Edge::new("e2", "src", None, "dst", None);
        e2.set_flow_kind(FlowKind::Value);
        e2.from_output = Some("users".to_owned());
        let mut e3 = Edge::new("e3", "src", None, "dst", None);
        e3.set_flow_kind(FlowKind::Value);
        canvas.edges.push(e1);
        canvas.edges.push(e2);
        canvas.edges.push(e3);
        let mut app = stub_app_with_canvas(canvas);
        app.scene.unmapped_edges = vec!["e3".to_owned()];
        let lines = app
            .inbound_label_lines(1, Side::Left)
            .expect("левый вход dst — 3 истока");
        assert_eq!(lines.len(), 3, "каждое входящее value-ребро — строка");
        assert_eq!(
            lines[0].text, "from: Заявки.строка 1",
            "fromLine — i18n «строка N», 1-based (R-5, как в v1)"
        );
        assert!(!lines[0].unmapped);
        assert_eq!(
            lines[1].text, "from: Заявки.users",
            "fromOutput — имя выхода (единая точка dataref)"
        );
        assert_eq!(
            lines[2].text, "from: Заявки.e3 · не подставлено",
            "без адресации — fallback edge.id; unmapped — маркер Р-3"
        );
        assert!(lines[2].unmapped, "unmapped-исток — янтарный тон строки");
    }

    /// FR-045 F-5 v2: свёртка длинного списка входов — 3 строки + «+N ещё»
    /// (тултип у курсора не разрастается; полный список — панель stage).
    #[test]
    fn port_in_label_lines_more_summary() {
        let mut canvas = Canvas::default();
        let mut src = Node::text("src", "Заявки\nusers = 10", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        let mut dst = Node::text("dst", "Отчёт\nx = 1", 700.0, 0.0);
        dst.width = 420.0;
        dst.height = 200.0;
        canvas.nodes.push(src);
        canvas.nodes.push(dst);
        for i in 0..4 {
            let mut edge = Edge::new(format!("e{i}"), "src", None, "dst", None);
            edge.set_flow_kind(FlowKind::Value);
            // Существующий выход «users» — рёбра пролитые (unmapped-маркер
            // не мешает свёртке; вычисляется recompute сцены при stub)
            edge.from_output = Some("users".to_owned());
            canvas.edges.push(edge);
        }
        let app = stub_app_with_canvas(canvas);
        let lines = app
            .inbound_label_lines(1, Side::Left)
            .expect("левый вход dst — 4 истока");
        assert_eq!(lines.len(), 4, "3 строки + свёртка «+N ещё»");
        assert_eq!(lines[0].text, "from: Заявки.users");
        assert_eq!(lines[2].text, "from: Заявки.users");
        assert_eq!(lines[3].text, "+1 ещё", "свёртка — счётчик скрытых строк");
        assert!(!lines[3].unmapped, "свёртка — нейтральный тон");
    }

    /// FR-045 F-5 v2: сторонный порт читается по стороне (P1 «входы
    /// слева») — с входами from-чтение (In-лейблы), без входов —
    /// «out:» (drag-исток, v1); hit-тест — левый порт приёмника.
    #[test]
    fn port_tooltip_at_in_precedes_out_fallback() {
        let mut canvas = Canvas::default();
        let mut src = Node::text("src", "Заявки\nusers = 10", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        let mut dst = Node::text("dst", "Отчёт\nx = 1", 700.0, 0.0);
        dst.width = 420.0;
        dst.height = 200.0;
        canvas.nodes.push(src);
        canvas.nodes.push(dst);
        let mut edge = Edge::new("e1", "src", None, "dst", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = Some("users".to_owned());
        canvas.edges.push(edge);
        let mut app = stub_app_with_canvas(canvas);
        app.hovered = Some(1);
        let lines = app
            .port_tooltip_at([700.0, 100.0])
            .expect("левый порт dst под курсором");
        assert_eq!(
            lines[0].text, "from: Заявки.users",
            "входящая сторона — from-чтение (R-5 qualified-адрес)"
        );
        // Одинокая нода без входов — прежнее v1-чтение («out:»)
        let mut canvas2 = Canvas::default();
        let mut solo = Node::text("src", "Заявки\nusers = 10", 0.0, 0.0);
        solo.width = 420.0;
        solo.height = 200.0;
        canvas2.nodes.push(solo);
        let mut app2 = stub_app_with_canvas(canvas2);
        app2.hovered = Some(0);
        let lines2 = app2
            .port_tooltip_at([0.0, 100.0])
            .expect("левый порт src под курсором");
        assert_eq!(
            lines2[0].text, "out: Заявки",
            "без входящих value-рёбер — drag-исток (v1)"
        );
        assert!(
            app2.port_tooltip_at([5000.0, 5000.0]).is_none(),
            "мимо порта — None"
        );
    }

    /// FR-045 F-5 v2: EN-язык — «line N» (i18n STAGE_LINE_LABEL),
    /// «not mapped» и «+N more» (FR-040, инвариант полноты).
    #[test]
    fn port_in_label_lines_english() {
        let mut canvas = Canvas::default();
        let mut src = Node::text("src", "Заявки\nusers = 10", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        let mut dst = Node::text("dst", "Отчёт\nx = 1", 700.0, 0.0);
        dst.width = 420.0;
        dst.height = 200.0;
        canvas.nodes.push(src);
        canvas.nodes.push(dst);
        for i in 0..4 {
            let mut edge = Edge::new(format!("e{i}"), "src", None, "dst", None);
            edge.set_flow_kind(FlowKind::Value);
            edge.from_line = Some(0);
            edge.from_output = Some(format!("f{i}"));
            canvas.edges.push(edge);
        }
        let mut app = stub_app_with_canvas(canvas);
        app.settings.language = Language::En;
        app.scene.unmapped_edges = vec!["e0".to_owned()];
        let lines = app
            .inbound_label_lines(1, Side::Left)
            .expect("левый вход dst — 4 истока");
        assert_eq!(
            lines[0].text, "from: Заявки.line 1 · not mapped",
            "EN: поле строки и маркер unmapped (Р-5 + Р-3)"
        );
        assert!(lines[0].unmapped);
        assert_eq!(lines[3].text, "+1 more", "EN: свёртка");
    }

    /// FR-044 Q2 (интеграция): переполненная стопка пилюль — режим Scroll
    /// с индикаторами «↑ ещё N»/«ещё N ↓»; клик по нижнему индикатору
    /// листает окно (счётчики above/below пересчитываются).
    #[test]
    fn stage_pill_zone_scroll_window() {
        let mut canvas = Canvas::default();
        let mut src = Node::text("src", "Заявки", 0.0, 0.0);
        src.width = 420.0;
        src.height = 200.0;
        let mut dst = Node::text("dst", "Отчёт\nx = 1", 700.0, 0.0);
        dst.width = 420.0;
        dst.height = 200.0;
        canvas.nodes.push(src);
        canvas.nodes.push(dst);
        // Пучок ×16 — в зоне stage обычного окна это Scroll (Q2)
        for i in 0..16 {
            let mut edge = Edge::new(format!("e{i}"), "src", None, "dst", None);
            edge.set_flow_kind(FlowKind::Value);
            edge.from_output = Some(format!("f{i}"));
            canvas.edges.push(edge);
        }
        let mut app = stub_app_with_canvas(canvas);
        app.scene.recompute_flow();
        let index = canvas_core::EdgeBundleIndex::build(&app.scene.canvas);
        app.main_stage = MainStageState::open(&app.scene.canvas, &index, 0);
        let stage = app.main_stage.as_ref().expect("stage открыт");
        let viewport = app.viewport_logical();
        let rect = main_stage_rect(viewport);
        let s = stage.scale.max(f32::EPSILON);
        let ctx = app.stage_frame_ctx(stage, &rect, s);
        let (pill_rects, mode) = app.stage_pill_state(stage, &ctx);
        let (above, below) = match mode {
            calc_panel_ui::PillZoneMode::Scroll { above, below, .. } => (above, below),
            other => panic!(
                "ожидался Scroll для пучка ×16 (zone.h = {}, rect'ов = {}): {other:?}",
                ctx.zone.h,
                pill_rects.len()
            ),
        };
        assert_eq!(above, 0, "окно с начала");
        assert!(below > 0, "есть скрытые снизу");
        assert_eq!(pill_rects.len() + above + below, 16, "окно + скрытые");
        // Индикатор «ещё N ↓» — клик листает окно
        let (top_ind, bottom_ind) = app.stage_pill_scroll_indicators(stage, &ctx);
        assert!(top_ind.is_none(), "выше окна пусто — индикатора нет");
        let bottom_ind = bottom_ind.expect("индикатор снизу есть");
        let transform = StageTransform::new([rect.x, rect.y], stage.scale);
        app.cursor = transform.map_point([
            bottom_ind.x + bottom_ind.w / 2.0,
            bottom_ind.y + bottom_ind.h / 2.0,
        ]);
        app.click_main_stage();
        assert!(app.stage_pill_scroll > 0, "клик по индикатору листает окно");
        // После листания: выше есть скрытые, снизу — упор
        let stage = app.main_stage.as_ref().expect("stage открыт");
        let ctx = app.stage_frame_ctx(stage, &rect, s);
        match ctx.pill_mode {
            calc_panel_ui::PillZoneMode::Scroll { above, below, .. } => {
                assert!(above > 0, "окно сместилось вниз");
                assert_eq!(below, 0, "клик страницей дошёл до упора");
            }
            other => panic!("режим должен остаться Scroll: {other:?}"),
        }
        // Закрытие stage сбрасывает окно
        app.close_main_stage();
        assert_eq!(app.stage_pill_scroll, 0, "окно сброшено (Q2)");
    }

    /// M8/W3 (wasm-port §6, приёмка «трейты покрыты тестами на заглушках»):
    /// `App::new` собирается целиком на заглушках (NoopThumbs/NoopWatch/
    /// NoopClipboard/MemSearch/MemWidgetState) без окна и GPU — сервисы
    /// инъектируются, нативных shell-типов в сигнатуре нет. Проверяем
    /// проводку: sync_watch_dirs (NoopWatch) без паник, state виджетов
    /// ходит через MemWidgetState, NoopClipboard молча пуст.
    #[test]
    fn app_assembles_on_stub_backends() {
        let scene = SceneState::new(
            Canvas::default(),
            PathBuf::from("target/tmp/w3-stubs.canvas"),
        );
        let cache_dir =
            std::env::temp_dir().join(format!("canvasdesk-w3-app-{}", std::process::id()));
        std::fs::create_dir_all(&cache_dir).expect("tmp cache dir");

        let (search_responder, _rx) = {
            let (tx, rx) = std::sync::mpsc::channel();
            let responder: canvas_core::SearchResponder = std::sync::Arc::new(move |event| {
                let _ = tx.send(event);
            });
            (responder, rx)
        };
        let widget_state = Box::new(canvas_core::MemWidgetState::default());

        let mut app = App::new(
            scene,
            Box::new(canvas_core::NoopThumbs),
            Settings::default(),
            None,
            Some(cache_dir.clone()),
            std::sync::Arc::new(|_event: canvas_core::DragEvent| {}),
            std::sync::Arc::new(|_event: canvas_widgets::WidgetEvent| {}),
            Box::new(canvas_core::NoopWatch),
            Box::new(canvas_core::MemSearch::new(search_responder)),
            Box::new(canvas_core::NoopClipboard),
            Some(widget_state),
            false,
            Box::new(canvas_render::renderer_init::NoopRendererLaunch),
        );

        // Вотчер-заглушка: синхронизация директорий — no-op без паник
        app.sync_watch_dirs();
        // Буфер обмена-заглушка: пусто и без паник
        assert_eq!(app.clipboard.get_text(), None);
        app.clipboard.set_text("тест".into());
        assert_eq!(app.clipboard.get_text(), None, "NoopClipboard не хранит");
        // Состояние виджетов через MemWidgetState: set/get roundtrip
        app.widgets.state_set("node-1", "note", "значение");
        assert_eq!(
            app.widgets.state_get("node-1", "note"),
            Some("значение".into())
        );
        std::fs::remove_dir_all(&cache_dir).ok();
    }

    /// Хелпер W6: App на заглушках с явным хранилищем сцены (MemStorage)
    /// — для проверки подмены сцены/флаша правок без окна и GPU.
    fn stub_app_on_storage(
        path: &str,
        storage: Arc<dyn CanvasStorage>,
    ) -> (App, Arc<dyn CanvasStorage>) {
        let scene =
            SceneState::with_storage(Canvas::default(), PathBuf::from(path), Arc::clone(&storage));
        // Уникальный каталог на вызов хелпера: тесты зовут его параллельно,
        // общий путь «по pid» гонял create_dir_all/remove_dir_all между
        // потоками — на Windows это PermissionDenied (флак CI gates
        // windows, merge 77cbb98). Счётчик убирает пересечение путей.
        static NEXT_CACHE_DIR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let cache_dir = std::env::temp_dir().join(format!(
            "canvasdesk-w6-app-{}-{}",
            std::process::id(),
            NEXT_CACHE_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&cache_dir).expect("tmp cache dir");
        let (search_responder, _rx) = {
            let (tx, rx) = std::sync::mpsc::channel();
            let responder: canvas_core::SearchResponder = std::sync::Arc::new(move |event| {
                let _ = tx.send(event);
            });
            (responder, rx)
        };
        let app = App::new(
            scene,
            Box::new(canvas_core::NoopThumbs),
            Settings::default(),
            None,
            Some(cache_dir.clone()),
            std::sync::Arc::new(|_event: canvas_core::DragEvent| {}),
            std::sync::Arc::new(|_event: canvas_widgets::WidgetEvent| {}),
            Box::new(canvas_core::NoopWatch),
            Box::new(canvas_core::MemSearch::new(search_responder)),
            Box::new(canvas_core::NoopClipboard),
            Some(Box::new(canvas_core::MemWidgetState::default())),
            false,
            Box::new(canvas_render::renderer_init::NoopRendererLaunch),
        );
        std::fs::remove_dir_all(&cache_dir).ok();
        (app, storage)
    }

    /// M8/W6: OpenScene — сцена подменяется (путь/модель), переходный UI
    /// сбрасывается (селекция/поиск/миникарта ссылались на старые индексы),
    /// камера — дефолт старта.
    #[test]
    fn open_scene_replaces_scene_and_resets_ui() {
        let (mut app, _old_storage) = stub_app_on_storage(
            "target/tmp/w6-a.canvas",
            Arc::new(canvas_core::MemStorage::new()),
        );
        // Переходное состояние «до»: селекция ноды 0, миникарта, поиск
        app.selected = Some(Selection::Node(0));
        app.selected_nodes.push(0);
        app.minimap = None; // поле приватное, но тесты в том же модуле — установим через открытие
        app.camera.set_zoom(3.0);

        let json = r#"{"nodes":[{"id":"n1","type":"text","text":"новая сцена","x":0,"y":0,"width":200,"height":150}]}"#;
        app.on_open_scene(
            PathBuf::from("target/tmp/w6-b.canvas"),
            json.to_string(),
            None,
        );

        assert_eq!(app.scene.path, PathBuf::from("target/tmp/w6-b.canvas"));
        assert_eq!(app.scene.canvas.nodes.len(), 1, "новая модель на месте");
        assert_eq!(app.selected, None, "селекция старой сцены сброшена");
        assert!(app.selected_nodes.is_empty());
        assert!(!app.search.open, "панель поиска закрыта");
        assert!(
            (app.camera.zoom() - Camera::default().zoom()).abs() < 1e-6,
            "камера — дефолт старта"
        );
        // Тост подтверждает открытие
        assert!(app
            .toast
            .as_ref()
            .is_some_and(|(text, _)| text.contains("Открыт")));
    }

    /// M8/W6: битый текст канваса — прежняя сцена продолжает жить (тост
    /// с ошибкой, путь/модель не тронуты) — деградация, не паника.
    #[test]
    fn open_scene_parse_error_keeps_current_scene() {
        let (mut app, _old) = stub_app_on_storage(
            "target/tmp/w6-keep.canvas",
            Arc::new(canvas_core::MemStorage::new()),
        );
        app.on_open_scene(
            PathBuf::from("target/tmp/w6-broken.canvas"),
            "{битый json".to_string(),
            None,
        );
        assert_eq!(app.scene.path, PathBuf::from("target/tmp/w6-keep.canvas"));
        assert!(app.scene.canvas.nodes.is_empty());
        assert!(app
            .toast
            .as_ref()
            .is_some_and(|(text, _)| text.contains("не открыт")));
    }

    /// M8/W6: незакрытые правки прежней сцены — форс-сохранение в её
    /// хранилище ДО подмены (SPEC §9: правки не теряются при смене файла).
    #[test]
    fn open_scene_flushes_dirty_previous_scene() {
        let old = Arc::new(canvas_core::MemStorage::new());
        let (mut app, old_handle) = stub_app_on_storage("target/tmp/w6-dirty.canvas", old);
        app.scene
            .canvas
            .extra
            .insert("name".into(), serde_json::json!("незакрытая правка"));
        app.scene.mark_dirty();
        assert!(app.scene.dirty_since.is_some());

        let json = r#"{"nodes":[]}"#;
        app.on_open_scene(
            PathBuf::from("target/tmp/w6-next.canvas"),
            json.to_string(),
            None,
        );
        // Прежняя сцена записана в СТАРОЕ хранилище под старым путём
        let saved = old_handle
            .load(&PathBuf::from("target/tmp/w6-dirty.canvas"))
            .expect("грязная сцена сохранена до подмены");
        assert_eq!(
            saved.extra.get("name").and_then(|v| v.as_str()),
            Some("незакрытая правка")
        );
        // Новая сцена чистая
        assert!(app.scene.dirty_since.is_none());
    }

    /// M8/W6: явное хранилище в событии (диск-хэндл после «Открыть с
    /// диска») — новая сцена сохраняет именно в него.
    #[test]
    fn open_scene_uses_provided_storage() {
        let (mut app, _old) = stub_app_on_storage(
            "target/tmp/w6-opfs.canvas",
            Arc::new(canvas_core::MemStorage::new()),
        );
        let disk = Arc::new(canvas_core::MemStorage::new());
        let json = r#"{"nodes":[{"id":"d1","type":"text","text":"с диска","x":10,"y":20,"width":200,"height":150}]}"#;
        app.on_open_scene(
            PathBuf::from("disk.canvas"),
            json.to_string(),
            Some(disk as Arc<dyn CanvasStorage>),
        );
        app.scene.save_now();
        let saved = app
            .scene
            .storage
            .load(&PathBuf::from("disk.canvas"))
            .expect("сохранение ушло в подменённое хранилище");
        assert_eq!(saved.nodes.len(), 1);
    }

    // --- FR-038 (T-038.5): batch-операции выравнивания (п.16-17) ------------

    /// distribute_axis_for: ряд (размах центров по X больше) → X, колонна → Y,
    /// равенство размахов и меньше двух элементов → X (детерминизм).
    #[test]
    fn distribute_axis_for_picks_larger_center_span() {
        let row = [
            sr(0.0, 0.0, 40.0, 40.0),
            sr(100.0, 2.0, 40.0, 40.0),
            sr(300.0, 10.0, 40.0, 40.0),
        ];
        assert_eq!(distribute_axis_for(&row), AlignAxis::X);
        let column = [
            sr(0.0, 0.0, 40.0, 40.0),
            sr(2.0, 100.0, 40.0, 40.0),
            sr(10.0, 300.0, 40.0, 40.0),
        ];
        assert_eq!(distribute_axis_for(&column), AlignAxis::Y);
        // Диагональ с равными размахами — X (тай-брейк)
        let tie = [sr(0.0, 0.0, 40.0, 40.0), sr(100.0, 100.0, 40.0, 40.0)];
        assert_eq!(distribute_axis_for(&tie), AlignAxis::X);
        assert_eq!(
            distribute_axis_for(&[sr(0.0, 0.0, 40.0, 40.0)]),
            AlignAxis::X
        );
    }

    /// Хелпер T-038.5: сцена из трёх нод с явными габаритами + выделение.
    fn batch_app(canvas: Canvas, selection: &[usize]) -> App {
        let (mut app, _storage) = stub_app_on_storage(
            "target/tmp/fr-038-batch.canvas",
            Arc::new(canvas_core::MemStorage::new()),
        );
        app.scene.canvas = canvas;
        app.scene.spatial = SpatialIndex::build(&app.scene.canvas);
        app.selected_nodes = selection.to_vec();
        app
    }

    /// Выравнивание в ряд (п.16): центры на общей горизонтали = среднее
    /// центров Y; X/размеры не тронуты.
    #[test]
    fn run_batch_op_align_row_sets_common_y() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("a", "", 0.0, 0.0);
        a.width = 100.0;
        a.height = 50.0;
        let mut b = Node::text("b", "", 200.0, 90.0);
        b.width = 80.0;
        b.height = 60.0;
        let mut c = Node::text("c", "", 400.0, 30.0);
        c.width = 40.0;
        c.height = 20.0;
        canvas.nodes.extend([a, b, c]);
        let expected = (25.0 + 120.0 + 40.0) / 3.0;
        let mut app = batch_app(canvas, &[0, 1, 2]);

        app.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));

        for i in 0..3 {
            let node = &app.scene.canvas.nodes[i];
            assert!(
                (node.y + node.height / 2.0 - expected).abs() < 1e-3,
                "центр ноды {i} на оси"
            );
        }
        assert_eq!(app.scene.canvas.nodes[0].x, 0.0, "X не тронут");
        assert_eq!(app.scene.canvas.nodes[1].x, 200.0);
        assert_eq!(app.scene.canvas.nodes[2].x, 400.0);
        // Выделение сохранено (операция не трогает UI-состояние выделения)
        assert_eq!(app.selected_nodes, vec![0, 1, 2]);
    }

    /// П.17: batch-операция — ОДИН undo-шаг на всё выделение; Ctrl+Z
    /// возвращает исходные позиции ровно; redo возвращает выравненные.
    #[test]
    fn run_batch_op_is_single_undo_step() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("a", "", 0.0, 0.0);
        a.width = 100.0;
        a.height = 50.0;
        let mut b = Node::text("b", "", 200.0, 90.0);
        b.width = 80.0;
        b.height = 60.0;
        let mut c = Node::text("c", "", 400.0, 30.0);
        c.width = 40.0;
        c.height = 20.0;
        canvas.nodes.extend([a, b, c]);
        let before: Vec<[f32; 2]> = canvas.nodes.iter().map(|n| [n.x, n.y]).collect();
        let mut app = batch_app(canvas, &[0, 1, 2]);

        app.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));
        assert_eq!(app.scene.undo_stack.len(), 1, "один undo-шаг (п.17)");
        let aligned: Vec<[f32; 2]> = app.scene.canvas.nodes.iter().map(|n| [n.x, n.y]).collect();
        assert_ne!(aligned, before, "операция что-то сдвинула");

        app.undo_action();
        assert!(app.scene.undo_stack.is_empty());
        let after_undo: Vec<[f32; 2]> = app.scene.canvas.nodes.iter().map(|n| [n.x, n.y]).collect();
        assert_eq!(after_undo, before, "undo вернул исходные позиции");

        app.redo_action();
        let after_redo: Vec<[f32; 2]> = app.scene.canvas.nodes.iter().map(|n| [n.x, n.y]).collect();
        assert_eq!(after_redo, aligned, "redo вернул выравнивание");
    }

    /// Batch-операция обновляет spatial-индекс (паттерн move_node):
    /// нода находится по НОВОЙ позиции и уже не по старой.
    #[test]
    fn run_batch_op_updates_spatial_index() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("a", "", 0.0, 0.0);
        a.width = 100.0;
        a.height = 50.0;
        let mut b = Node::text("b", "", 200.0, 900.0);
        b.width = 80.0;
        b.height = 60.0;
        let mut c = Node::text("c", "", 400.0, 1800.0);
        c.width = 40.0;
        c.height = 20.0;
        canvas.nodes.extend([a, b, c]);
        let mut app = batch_app(canvas, &[0, 1, 2]);
        app.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));
        let expected = (25.0 + 930.0 + 1810.0) / 3.0;

        // Зона вокруг НОВОЙ оси (среднее центров) видит все три ноды;
        // старые позиции (по краям) — больше ни одной из них
        let near_axis: [f32; 4] = [-1000.0, expected - 10.0, 1000.0, expected + 10.0];
        let hits = app.scene.spatial.query_rect(near_axis);
        assert_eq!(hits, vec![0, 1, 2], "все ноды у новой оси");
        let old_top: [f32; 4] = [-1000.0, 800.0, 100.0, 860.0];
        assert!(
            !app.scene.spatial.query_rect(old_top).contains(&1),
            "нода 1 ушла со старого места в индексе"
        );
    }

    /// Дети групп следуют за родителем (п.6): выделены группа + две ноды —
    /// юниты три, ребёнок группы двигается дельтой группы, не выделяясь.
    #[test]
    fn run_batch_op_group_children_follow_parent() {
        let mut canvas = Canvas::default();
        // Группа-рамка g с явным списком детей [c]
        let mut g = Node::text("g", "", 1000.0, 500.0);
        g.node_type = "group".into();
        g.width = 300.0;
        g.height = 200.0;
        g.children = Some(vec!["c".into()]);
        canvas.nodes.push(g);
        let mut c = Node::text("c", "", 1100.0, 550.0);
        c.width = 60.0;
        c.height = 40.0;
        canvas.nodes.push(c);
        let mut b = Node::text("b", "", 0.0, 0.0);
        b.width = 80.0;
        b.height = 60.0;
        let mut d = Node::text("d", "", 50.0, 900.0);
        d.width = 40.0;
        d.height = 20.0;
        canvas.nodes.push(b);
        canvas.nodes.push(d);
        // Выделение: группа (0) + две ноды (2, 3); ребёнок (1) НЕ выделен
        let mut app = batch_app(canvas, &[0, 2, 3]);
        let group_before = app.scene.canvas.nodes[0].y;
        let child_before = app.scene.canvas.nodes[1].y;

        app.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));

        let expected = (600.0 + 30.0 + 910.0) / 3.0; // центры Y юнитов
        let group = &app.scene.canvas.nodes[0];
        let child = &app.scene.canvas.nodes[1];
        assert!((group.y + group.height / 2.0 - expected).abs() < 1e-3);
        let group_dy = group.y - group_before;
        assert!(
            (child.y - (child_before + group_dy)).abs() < 1e-3,
            "ребёнок сдвинулся дельтой группы"
        );
        assert_eq!(child.x, 1100.0, "поперечное ребёнка не тронуто");
        // undo возвращает и группу, и ребёнка
        app.undo_action();
        assert_eq!(app.scene.canvas.nodes[1].y, child_before);
    }

    /// Распределение через авто-ось (None): колонна (размах центров по Y
    /// больше) распределяется по Y с равными зазорами между краями.
    #[test]
    fn run_batch_op_distribute_auto_axis_column() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("a", "", 0.0, 0.0);
        a.width = 100.0;
        a.height = 40.0;
        let mut b = Node::text("b", "", 20.0, 60.0);
        b.width = 80.0;
        b.height = 80.0;
        canvas.nodes.push(a);
        canvas.nodes.push(b);
        let mut c = Node::text("c", "", 10.0, 300.0);
        c.width = 60.0;
        c.height = 20.0;
        canvas.nodes.push(c);
        let mut app = batch_app(canvas, &[0, 1, 2]);

        app.run_batch_op(BatchOp::Distribute, None);

        // span 0..320, сумма высот 140 → зазор 90; позиции 0, 130, 300
        assert_eq!(app.scene.canvas.nodes[0].y, 0.0);
        assert_eq!(app.scene.canvas.nodes[1].y, 130.0);
        assert_eq!(app.scene.canvas.nodes[2].y, 300.0);
        assert_eq!(app.scene.undo_stack.len(), 1, "один undo-шаг");
    }

    /// Повторная команда на уже выровненном выделении — no-op БЕЗ
    /// undo-шага (пустых шагов в истории нет).
    #[test]
    fn run_batch_op_noop_when_already_aligned() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("a", "", 0.0, 100.0);
        a.width = 100.0;
        a.height = 50.0;
        let mut b = Node::text("b", "", 200.0, 90.0);
        b.width = 80.0;
        b.height = 60.0;
        let mut c = Node::text("c", "", 400.0, 80.0);
        c.width = 40.0;
        c.height = 20.0;
        // Центры Y: 125, 120, 90 — среднее 111.666…, операция сдвинет;
        // сначала выравниваем, потом повторяем
        canvas.nodes.extend([a, b, c]);
        let mut app = batch_app(canvas, &[0, 1, 2]);
        app.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));
        assert_eq!(app.scene.undo_stack.len(), 1);
        app.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));
        assert_eq!(
            app.scene.undo_stack.len(),
            1,
            "повтор без сдвига — без undo-шага"
        );
    }

    /// Выделение «группа + её дети» (N=3) — самостоятельный юнит один:
    /// операция честный no-op (выравнивать нечего), история не растёт.
    #[test]
    fn run_batch_op_group_with_children_only_is_noop() {
        let mut canvas = Canvas::default();
        let mut g = Node::text("g", "", 0.0, 0.0);
        g.node_type = "group".into();
        g.width = 300.0;
        g.height = 200.0;
        g.children = Some(vec!["c1".into(), "c2".into()]);
        canvas.nodes.push(g);
        canvas.nodes.push(Node::text("c1", "", 10.0, 10.0));
        canvas.nodes.push(Node::text("c2", "", 100.0, 20.0));
        let before: Vec<[f32; 2]> = canvas.nodes.iter().map(|n| [n.x, n.y]).collect();
        let mut app = batch_app(canvas, &[0, 1, 2]);

        app.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));

        let after: Vec<[f32; 2]> = app.scene.canvas.nodes.iter().map(|n| [n.x, n.y]).collect();
        assert_eq!(after, before, "ничего не сдвинулось");
        assert!(app.scene.undo_stack.is_empty(), "без undo-шага");
    }

    /// Видимость batch-пунктов (п.16): N≥3 — видны, меньше — скрыты.
    #[test]
    fn align_menu_visible_only_from_three_nodes() {
        let mut canvas = Canvas::default();
        for id in ["a", "b", "c"] {
            canvas.nodes.push(Node::text(id, "", 0.0, 0.0));
        }
        let mut app = batch_app(canvas, &[]);
        assert!(!app.align_menu_visible(), "нет выделения — скрыты");
        app.selected_nodes = vec![0, 1];
        assert!(!app.align_menu_visible(), "N=2 — скрыты");
        app.selected_nodes = vec![0, 1, 2];
        assert!(app.align_menu_visible(), "N=3 — видны");
    }
}

// --- FR-050 (этап C): чистые функции UI-механизма toParam ---

#[cfg(test)]
mod fr050_stage_c_tests {
    use super::*;

    /// Н2: исток активного drag — только вариант New; Rebind/None — нет.
    #[test]
    fn drag_from_node_only_new_variant() {
        assert_eq!(drag_from_node(None), None);
        assert_eq!(
            drag_from_node(Some(&EdgeDrag::Rebind {
                edge_index: 0,
                end: canvas_core::EdgeEnd::From,
            })),
            None,
            "перепривязка — не новый исток"
        );
        assert_eq!(
            drag_from_node(Some(&EdgeDrag::New {
                from_node: "a".to_owned(),
                from_side: Side::Right,
                value_flow: true,
                from_port: None,
            })),
            Some("a".to_owned())
        );
    }

    /// Н4: подпись ноды для диалога замены — приоритет: имя шаблона
    /// (FR-023) → первая непустая строка текста → label → id.
    #[test]
    fn node_display_label_priority() {
        let mut node = Node::text("n1", "rps = 1000 rps\nservers = 2", 0.0, 0.0);
        node.set_template(Some(canvas_core::templates::TemplateRef {
            id: "t".to_owned(),
            version: "1".to_owned(),
            expr: "$rps".to_owned(),
            params: Default::default(),
            icon: String::new(),
            color: String::new(),
            name: Some("Балансировщик".to_owned()),
            outputs: Vec::new(),
        }));
        assert_eq!(node_display_label(&node), "Балансировщик");
        // Проза без шаблона: первая непустая строка (пустые пропускаются)
        let prose = Node::text("n2", "\n\nПривет Мир\nвторой", 0.0, 0.0);
        assert_eq!(node_display_label(&prose), "Привет Мир");
        // Текст пуст → label
        let labeled = Node::text("n3", "", 0.0, 0.0);
        let mut labeled = labeled;
        labeled.label = Some("Метка".to_owned());
        assert_eq!(node_display_label(&labeled), "Метка");
        // Совсем ничего → id
        assert_eq!(node_display_label(&Node::text("n4", "", 0.0, 0.0)), "n4");
    }
}

/// FR-050 этап E: юнит-тесты чистых функций (Н9-1/Н9-3/Н9-6).
#[cfg(test)]
mod fr050_stage_e_tests {
    use super::*;

    /// Н9-6: ключ тоста по представлению проливания — строка присваивания
    /// была → «Параметр…»; параметра не было (строка-проекция Р-4) →
    /// «Значение…».
    #[test]
    fn spill_toast_key_by_line_presence() {
        let with_param = SpillView {
            param: "rps".to_owned(),
            line: Some(0),
            from_label: "Трафик".to_owned(),
            from_output: Some("peak_rps".to_owned()),
            value: Some("1389 rps".to_owned()),
            path: "Трафик.peak_rps".to_owned(),
            local: Some("50 rps".to_owned()),
        };
        assert_eq!(spill_toast_key(&with_param), keys::TOAST_SPILL_PARAM);
        let auto_row = SpillView {
            param: "rps".to_owned(),
            line: None,
            from_label: "Трафик".to_owned(),
            from_output: Some("peak_rps".to_owned()),
            value: Some("1389 rps".to_owned()),
            path: "Трафик.peak_rps".to_owned(),
            local: None,
        };
        assert_eq!(spill_toast_key(&auto_row), keys::TOAST_SPILL_AUTOROW);
    }

    /// Н9-3: цель контекст-меню параметра — ребро-победитель (последнее по
    /// canvas.edges, инвариант Н4) и заголовок «Проливание в параметр»;
    /// авто-строка адресует ребро из hit-зоны, заголовок «Входящее
    /// значение»; протухшие hit-зоны (нода/ребро удалены) — None.
    #[test]
    fn spill_hit_target_resolves_edge() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text(
            "traffic",
            "Трафик\npeak_rps = 1389 rps",
            0.0,
            0.0,
        ));
        canvas
            .nodes
            .push(Node::text("gw", "rps = 50 rps", 400.0, 0.0));
        // Легаси-дубль + победитель: E2 позже по edges
        for (id, from) in [("e1", "traffic"), ("e2", "traffic")] {
            let mut edge = Edge::new(id, from, None, "gw", None);
            edge.set_flow_kind(FlowKind::Value);
            edge.to_param = Some("rps".to_owned());
            edge.from_line = Some(1);
            canvas.add_edge(edge);
        }
        let hit = SpillHit {
            rect: [0.0, 0.0, 10.0, 10.0],
            kind: SpillHitKind::Param {
                param: "rps".to_owned(),
                path: "Трафик.peak_rps".to_owned(),
                value: Some("1389 rps".to_owned()),
                local: Some("50 rps".to_owned()),
            },
            node: 1,
        };
        let target = spill_hit_target(&canvas, &hit).expect("победитель найден");
        assert_eq!(target.edge_id, "e2", "победитель — последнее ребро");
        assert_eq!(target.param.as_deref(), Some("rps"));
        assert_eq!(target.title_key, keys::MENU_PARAM_TITLE);

        // Авто-строка: ребро в hit-зоне
        let auto = SpillHit {
            rect: [0.0, 0.0, 10.0, 10.0],
            kind: SpillHitKind::AutoRow {
                path: "Трафик.peak_rps".to_owned(),
                slot: 0,
                value: Some("1389 rps".to_owned()),
                template: false,
                edge_id: "e2".to_owned(),
            },
            node: 1,
        };
        let target = spill_hit_target(&canvas, &auto).expect("ребро авто-строки");
        assert_eq!(target.edge_id, "e2");
        assert_eq!(target.param, None);
        assert_eq!(target.title_key, keys::MENU_AUTOROW_TITLE);

        // Протухшие зоны: неизвестная нода / удалённое ребро / свободный
        // параметр — меню не открывается (None)
        let ghost_node = SpillHit {
            rect: [0.0, 0.0, 10.0, 10.0],
            kind: SpillHitKind::AutoRow {
                path: String::new(),
                slot: 0,
                value: None,
                template: false,
                edge_id: "e2".to_owned(),
            },
            node: 99,
        };
        assert!(spill_hit_target(&canvas, &ghost_node).is_none());
        let gone_edge = SpillHit {
            rect: [0.0, 0.0, 10.0, 10.0],
            kind: SpillHitKind::AutoRow {
                path: String::new(),
                slot: 0,
                value: None,
                template: false,
                edge_id: "e-missing".to_owned(),
            },
            node: 1,
        };
        assert!(spill_hit_target(&canvas, &gone_edge).is_none());
        let free_param = SpillHit {
            rect: [0.0, 0.0, 10.0, 10.0],
            kind: SpillHitKind::Param {
                param: "unknown".to_owned(),
                path: String::new(),
                value: None,
                local: None,
            },
            node: 1,
        };
        assert!(spill_hit_target(&canvas, &free_param).is_none());
    }

    // --- FR-060: измеренная геометрия диалога подтверждения -----------------

    /// Диалог адаптируется под измеренный текст: ширина — от замера
    /// (пол/потолок прежние), высота — пады прежней раскладки + измеренная
    /// строка тела; кнопки — `kit::button_size`, пара по центру, зазор 16;
    /// длинный заголовок — ellipsis (видимая деградация, не молчаливый клип).
    #[test]
    fn dialog_measured_adapts_to_text() {
        let mut fs = cosmic_text::FontSystem::new();
        let mut m = canvas_ui::measure::TextMeasurer::new();
        const FAMILY: &str = canvas_render::text::SANS_FAMILY;
        let viewport = [1280.0, 800.0];
        let buttons = ["Да".to_owned(), "Нет".to_owned()];
        let (rect, btns, shown) = App::dialog_measured_in(
            viewport,
            "Установить виджет?",
            "Пакет: clock (1.2.0)",
            &buttons,
            &mut m,
            &mut fs,
        );
        // Ширина: измеренный текст + 2·пад X, пол 280 (короткие тексты)
        let title_w = m.width_of(&mut fs, "Установить виджет?", FAMILY, DIALOG_TITLE_FS);
        let body_w = m.width_of(&mut fs, "Пакет: clock (1.2.0)", FAMILY, DIALOG_BODY_FS);
        let expect_w = (title_w.max(body_w) + DIALOG_PAD_X * 2.0).clamp(DIALOG_MIN_W, DIALOG_MAX_W);
        assert!((rect[2] - expect_w).abs() < 0.01, "ширина от замера");
        assert!(rect[2] >= DIALOG_MIN_W);
        // Высота: тело 46 + измеренная строка + SPACING_LG + кнопка 30 + низ 16
        let body_h = m
            .measure(
                &mut fs,
                &canvas_ui::measure::TextSpec {
                    text: "Пакет: clock (1.2.0)",
                    family: FAMILY,
                    size: DIALOG_BODY_FS,
                    max_width: f32::INFINITY,
                    weight: cosmic_text::Weight::MEDIUM,
                },
            )
            .height;
        let expect_h = DIALOG_BODY_Y
            + body_h
            + canvas_core::tokens::SPACING_LG
            + DIALOG_BTN_H
            + DIALOG_BTN_BOTTOM;
        assert!(
            (rect[3] - expect_h).abs() < 0.01,
            "высота от замера, не h=150"
        );
        assert!(rect[3] < 150.0, "мёртвый слэк прежних 150 устранён");
        // Панель центрирована (kit::modal)
        assert!((rect[0] - (viewport[0] - rect[2]) / 2.0).abs() < 0.01);
        assert!((rect[1] - (viewport[1] - rect[3]) / 2.0).abs() < 0.01);
        // Кнопки: kit::button_size, пара по центру, зазор 16, низ 16
        let bw0 = m.width_of(&mut fs, "Да", FAMILY, DIALOG_BTN_FS) + kit::BUTTON_PAD_H * 2.0;
        let bw1 = m.width_of(&mut fs, "Нет", FAMILY, DIALOG_BTN_FS) + kit::BUTTON_PAD_H * 2.0;
        assert!((btns[0][2] - bw0).abs() < 0.01, "ширина кнопки от текста");
        assert!((btns[1][2] - bw1).abs() < 0.01);
        assert!(
            (btns[0][3] - DIALOG_BTN_H).abs() < 0.01,
            "высота кнопки 30 дословно"
        );
        let total = bw0 + DIALOG_BTN_GAP + bw1;
        let start = rect[0] + (rect[2] - total) / 2.0;
        assert!((btns[0][0] - start).abs() < 0.01, "пара по центру");
        assert!((btns[1][0] - (start + bw0 + DIALOG_BTN_GAP)).abs() < 0.01);
        assert!((btns[0][1] - (rect[1] + rect[3] - DIALOG_BTN_H - DIALOG_BTN_BOTTOM)).abs() < 0.01);
        assert_eq!(
            shown,
            [
                "Установить виджет?".to_owned(),
                "Пакет: clock (1.2.0)".to_owned()
            ]
        );
    }

    /// Потолок ширины 440 + ellipsis длинного текста; кламп к узкому окну.
    #[test]
    fn dialog_measured_long_text_ellipsis_and_narrow_viewport() {
        let mut fs = cosmic_text::FontSystem::new();
        let mut m = canvas_ui::measure::TextMeasurer::new();
        let long_title = "Цикл: очень длинная цепочка участников расчёта ".repeat(12);
        let buttons = ["Да".to_owned(), "Нет".to_owned()];
        // Широкий вьюпорт: ширина = потолок 440, заголовок — ellipsis
        let (rect, _, shown) = App::dialog_measured_in(
            [1280.0, 800.0],
            &long_title,
            "тело",
            &buttons,
            &mut m,
            &mut fs,
        );
        assert!((rect[2] - DIALOG_MAX_W).abs() < 0.01, "потолок 440");
        assert!(shown[0].ends_with('…'), "ellipsis вместо молчаливого клипа");
        assert!(shown[0].chars().count() < long_title.chars().count());
        // Узкий вьюпорт: пол инварианта 280 сильнее клампа к окну —
        // прежняя формула дословно (w = min(440, vw−40).max(280))
        let (rect, _, _) = App::dialog_measured_in(
            [300.0, 600.0],
            "Установить виджет?",
            "тело",
            &buttons,
            &mut m,
            &mut fs,
        );
        assert!((rect[2] - DIALOG_MIN_W).abs() < 0.01, "пол 280 ≡ прежнему");
    }
}
