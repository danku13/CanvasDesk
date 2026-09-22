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
    help_button_rect, hotkeys_panel_rect, in_resize_corner, menu_item_at_for, menu_item_rect,
    menu_rect_for, next_free_id, nodes_in_rect, paste_nodes, plan_group_around,
    plan_group_around_nodes, plan_group_at, point_in_rect, reassign_ids, rubber_band_rect,
    select_node_hit, submenu_item_at, submenu_origin_next_to, submenu_rect, theme_button_rect,
    toggle_selection_with_primary, CanvasMenuItem, ContextMenu, DoubleClick, DragState, EdgeDrag,
    PastePlacement, Submenu, SubmenuEntry, ALIGN_MIN_SELECTION, DUPLICATE_OFFSET, MENU_LABEL_X,
    MENU_PADDING, MENU_WIDTH, MIN_NODE_HEIGHT, MIN_NODE_WIDTH, SELECT_DRAG_THRESHOLD,
};
use crate::whatif_ui::{self, BarAction};
// PRD-0007 (FR-048 X2): окно проверки цепочки расчёта цифры — модель и
// состояния (Loading/Ready/Stale), рендер/ввод — здесь (паттерн main stage).
use crate::explain_ui::{self, ExplainBuild, ExplainSnapshot, ExplainState};
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
    ClipboardBackend, Edge, FileEvent, FocusSeed, FocusSet, GridStyle, Language, LineageNodeId,
    Node, NodeChange, NodeKind, Priority, SearchBackend, Settings, Side, SnapAnchor, SpatialIndex,
    StageLayout, StageMetrics, Theme, ThumbBackend, WatchBackend, COLLISION_GAP,
};
use canvas_ui::geometry::UiPoint;
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
    FOCUS_FADE_MS, FOCUS_PULSE_MS,
};
use canvas_render::camera::Vec2;
use canvas_render::cards::{
    build_stage_edge_instances, card_instance, drop_ghost, template_band_instance,
    template_icon_quads, title_for, BundleContext, CardInstance, FocusView, EDGE_COLOR,
    FLOW_EDGE_COLOR, HEADER_HEIGHT, SELECTION_BORDER,
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
use canvas_render::renderer_init::{RendererLaunch, RendererLauncher, RendererSlot};
use canvas_render::search_ui::{
    layout as search_layout, scan_scene, PanelAction, SceneEntry, SearchInput, SearchPanel,
    SearchRow,
};
use canvas_render::sectors::SectorInstance;
use canvas_render::text::{
    body_area, measure_body_height, LineErrorHit, OverlayText, ScreenText, TextAlign,
    BODY_LINE_HEIGHT, BODY_PADDING, BODY_TOP_GAP, RESULT_LINE_HEIGHT,
};
use canvas_render::ThemeColors;
use canvas_render::{
    Camera, Color, FrameMeter, FrameOverlay, FrameStats, ParamDropView, SceneView, Selection,
    StageTransform,
};
use canvas_scene::{
    fit_template_node_height, formula_line_indices, split_formula_lines, whatif_delta_str,
    SceneState,
};

/// FR-052 (этап U2 PRD-0009): реестр поверхностей экрана — единый диспетчер.
/// Дочерний модуль `app`: доступ к приватным полям `App` (снимок состояния
/// на кадр). Декларации поверхностей (слой/capture/scope/деградация),
/// сборка `UiFrame` (hit-rect'ы из тех же layout-функций, что у ввода и
/// отрисовки), владелец клавиатуры из `esc_stack`, draw-полосы.
pub mod ui_registry;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// FR-046: мост «примитив design-токенов → glyphon::Color» (константный):
/// текстовые цвета диалогов/тостов берутся из `canvas_core::tokens`.
const fn token_color(rgb: [u8; 3]) -> Color {
    Color::rgb(rgb[0], rgb[1], rgb[2])
}
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
/// Ширина клип-бокса тултипа битой ссылки (T10): длинный путь переносится
/// на границы этой области, экран не покидает.
const TOOLTIP_WIDTH: f32 = 380.0;

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

/// Опора для центрируемого screen-текста внутри `rect` (подписи кнопок/чипов).
/// Контракт рендера (`ScreenText`, text.rs): `origin` — ЛЕВЫЙ край области
/// выравнивания, Center центрирует строку в `[origin_x, origin_x + width]`.
/// Поэтому origin ставим на `rect[0] + inset`, а ширину берём с двусторонним
/// инсетом — итоговая область по центру rect. Передача центра rect как
/// origin сдвигает текст вправо на полширины области (дефект CR-015).
fn centered_box(rect: [f32; 4], inset: f32) -> ([f32; 2], f32) {
    let width = (rect[2] - inset * 2.0).max(0.0);
    ([rect[0] + inset, rect[1]], width)
}

/// Усечение строки до `max` символов с многоточием (FR-044: ширина пилюль
/// и строк stage оценивается по числу символов — средняя advance моно 12 px;
/// точное измерение недоступно на стороне приложения — шейпинг в TextSystem).
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

// --- PRD-0007 (FR-048 X2): хелперы кадра окна проверки ----------------------

/// Screen-прямоугольник → world-квад (паттерн stage: позиция через
/// screen_to_world, размер/радиус делятся на зум — константный экранный
/// размер при любом зуме).
fn screen_rect_quad(
    camera: &Camera,
    viewport: Vec2,
    rect: [f32; 4],
    fill: [f32; 4],
    border: [f32; 4],
    radius: f32,
) -> CardInstance {
    let zoom = camera.zoom();
    CardInstance {
        pos: camera.screen_to_world([rect[0], rect[1]], viewport),
        size: [rect[2] / zoom, rect[3] / zoom],
        fill,
        border,
        params: [radius / zoom, 0.0, 0.0, 1.0],
    }
}

/// Кружок с центром в screen-точке → world-инстанс (паттерн `cards::dot`).
fn screen_dot(
    camera: &Camera,
    viewport: Vec2,
    center: [f32; 2],
    diameter: f32,
    fill: [f32; 4],
) -> CardInstance {
    let zoom = camera.zoom();
    let world = camera.screen_to_world(center, viewport);
    let r = diameter / 2.0 / zoom;
    CardInstance {
        pos: [world[0] - r, world[1] - r],
        size: [diameter / zoom, diameter / zoom],
        fill,
        border: [0.0; 4],
        params: [r, 0.0, 0.0, 1.0],
    }
}

/// Точки кубической безье (`n` отсчётов, включая концы) — ветки дерева
/// (прототип v4: от правого порта родителя к левому порту ребёнка).
fn bezier_samples(points: [[f32; 2]; 4], n: usize) -> Vec<[f32; 2]> {
    let [p0, c0, c1, p1] = points;
    (0..=n)
        .map(|i| {
            let t = i as f32 / n.max(1) as f32;
            let u = 1.0 - t;
            [
                u * u * u * p0[0]
                    + 3.0 * u * u * t * c0[0]
                    + 3.0 * u * t * t * c1[0]
                    + t * t * t * p1[0],
                u * u * u * p0[1]
                    + 3.0 * u * u * t * c0[1]
                    + 3.0 * u * t * t * c1[1]
                    + t * t * t * p1[1],
            ]
        })
        .collect()
}

/// Сборка lineage-дерева по снапшоту сцены (общая для фонового потока и
/// фолбэка): Ready по значениям либо Cycled-топология (AC-2.4).
fn build_lineage_snapshot(
    canvas: &Canvas,
    solutions: &flow::FlowSolutions,
    cycle: Option<&flow::CycleError>,
    root: LineageNodeId,
) -> Result<canvas_core::LineageTree, canvas_core::LineageError> {
    match cycle {
        Some(cycle) => canvas_core::build_lineage(
            &canvas.clone(),
            canvas_core::LineageFlow::Cycled(cycle),
            root,
        ),
        None => {
            let data = canvas_core::DataSnapshots::new();
            canvas_core::build_lineage(
                canvas,
                canvas_core::LineageFlow::Ready {
                    solutions,
                    data: &data,
                },
                root,
            )
        }
    }
}

/// PRD-0007 (X2, AC-1.2/G5): запуск сборки дерева — натив: фоновый поток
/// (UI не блокируется, честный лоадер крутится), wasm/сбой потока:
/// синхронный результат (Ready на первом же poll).
fn spawn_lineage_build(
    canvas: &Canvas,
    solutions: &flow::FlowSolutions,
    cycle: Option<&flow::CycleError>,
    root: LineageNodeId,
) -> ExplainBuild {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let worker_canvas = canvas.clone();
        let worker_solutions = solutions.clone();
        let worker_cycle = cycle.cloned();
        let worker_root = root.clone();
        let spawned = std::thread::Builder::new()
            .name("lineage-build".into())
            .spawn(move || {
                let tree = build_lineage_snapshot(
                    &worker_canvas,
                    &worker_solutions,
                    worker_cycle.as_ref(),
                    worker_root,
                );
                let _ = tx.send(tree);
            });
        match spawned {
            Ok(_handle) => ExplainBuild::Native(rx),
            // Поток не поднялся — синхронный фолбэк (окно честно ждёт)
            Err(_) => ExplainBuild::Done(build_lineage_snapshot(canvas, solutions, cycle, root)),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        // Однопоточный рантайм: сборка синхронная (Ready на первом poll)
        ExplainBuild::Done(build_lineage_snapshot(canvas, solutions, cycle, root))
    }
}

/// PRD-0007 (F-4/AC-3.1): набор подсветки цепочки на канвасе из дерева
/// снапшота (F-5: одна модель для окна и подсветки). Узлы — все `node_id`
/// дерева, рёбра — все `via.edge_id`; индексы отсортированы (контракт
/// FocusSet). Пучки подсвечивает OR-семантика рендера (линию пучка рисует
/// доминанта); невалидные id (нода/ребро удалены) молча пропускаются.
fn explain_chain_focus(canvas: &Canvas, tree: &canvas_core::LineageTree) -> FocusSet {
    let mut set = FocusSet::empty();
    let node_index: std::collections::HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let edge_index: std::collections::HashMap<&str, usize> = canvas
        .edges
        .iter()
        .enumerate()
        .map(|(i, e)| (e.id.as_str(), i))
        .collect();
    for node in &tree.nodes {
        if let Some(&i) = node_index.get(node.node_id.as_str()) {
            set.nodes.push(i);
        }
        for child in &node.children {
            let Some(via) = &child.via else { continue };
            if let Some(&i) = edge_index.get(via.edge_id.as_str()) {
                set.edges.push(i);
            }
        }
    }
    set.nodes.sort_unstable();
    set.nodes.dedup();
    set.edges.sort_unstable();
    set.edges.dedup();
    set
}

// --- FR-038 (T-038.4): интеграция магнитной раскладки ----------------------
//
// Семантика v2: во время drag нода следует курсору свободно (origin+delta),
// snap-движок работает на ПРЕДПРОСМОТР (направляющие + ghost, каждый кадр
// драга) и НА ОТПУСКАНИИ (п.2), collision-avoidance — LIVE (кламп дельты
// каждого кадра, п.15), anchor (п.4) — надстройка над grid-частью движка.

/// Данные snap-расчёта одного кадра драга (T-038.4): достаточно и для
/// перемещения (live-collision), и для предпросмотра, и для отпускания —
/// отпускание пересчитывает кадр с теми же входами (детерминизм п.20:
/// результат совпадает с последним предпросмотром).
struct SnapFrame {
    /// Дельта перемещения ЭТОГО кадра: свободная (world−grab) или ужатая
    /// live-collision (п.15, если `snap_collision` включён).
    eff_delta: [f32; 2],
    /// Полный расчёт снапа от bbox-origins с желаемой дельтой `eff_delta`:
    /// итоговая дельта (grid/guides/anchor + collision поверх) и оси
    /// направляющих. Итоговые позиции = origin + outcome.dx/dy.
    outcome: SnapOutcome,
    /// bbox перемещаемого набора в позиции кадра (origins + eff_delta),
    /// [x0, y0, x1, y1] — вход `GuidesFrame::from_snap` для ghost.
    bbox_current: [f32; 4],
    /// Допуск в world px (`snap_tolerance_px / zoom`) — для нелинейного
    /// усиления направляющих (п.8, `GuidesFrame.tolerance`).
    tolerance_world: f32,
}

/// bbox перемещаемого набора ПО ИСХОДНЫМ позициям (origins): размеры берутся
/// из текущих нод — drag размеры не меняет. Групповой drag — union bbox
/// (п.14: дети групп уже в origins, дубликатов нет). Пустой/битый набор —
/// None (снап не применяется).
fn drag_bbox(canvas: &Canvas, origins: &[(usize, Vec2)]) -> Option<SnapRect> {
    let mut acc: Option<SnapRect> = None;
    for (index, origin) in origins {
        let node = canvas.nodes.get(*index)?;
        let rect = SnapRect {
            x: origin[0],
            y: origin[1],
            w: node.width,
            h: node.height,
        };
        acc = Some(match acc {
            None => rect,
            Some(acc) => {
                let left = acc.x.min(rect.x);
                let top = acc.y.min(rect.y);
                let right = (acc.x + acc.w).max(rect.x + rect.w);
                let bottom = (acc.y + acc.h).max(rect.y + rect.h);
                SnapRect {
                    x: left,
                    y: top,
                    w: right - left,
                    h: bottom - top,
                }
            }
        });
    }
    acc
}

/// Кандидаты снапа (T-038.4): bbox видимых нод ВНЕ перемещаемого набора
/// (края/центры/середины движок берёт из bbox сам). Группа — обычная нода
/// в `canvas.nodes`: её bbox и есть кандидат (п.14). Чистая функция.
fn snap_candidates(canvas: &Canvas, visible: &[usize], moving: &[usize]) -> Vec<SnapRect> {
    visible
        .iter()
        .filter(|index| !moving.contains(index))
        .filter_map(|index| canvas.nodes.get(*index))
        .map(|node| SnapRect {
            x: node.x,
            y: node.y,
            w: node.width,
            h: node.height,
        })
        .collect()
}

/// Дельта притяжения точки к ближайшей линии сетки (п.4 anchor-надстройка):
/// в пределах допуска — дельта до ближайшей линии (кратной `step`), иначе 0 —
/// сетка молчит, как в движке (п.1-3). Отрицательные координаты корректны
/// ((pos/step).round() математически одинаков).
fn anchor_grid_delta(pos: f32, step: f32, tol: f32) -> f32 {
    let delta = (pos / step).round() * step - pos;
    if delta.abs() <= tol {
        delta
    } else {
        0.0
    }
}

/// Допуск в world px из экранных (п.6): screen = world·zoom → деление на
/// зум; некорректный зум — фолбэк 1.0 (зеркало движка, snap::tol_world).
fn snap_tolerance_world(tolerance_px: f32, zoom: f32) -> f32 {
    if zoom > 0.0 {
        tolerance_px / zoom
    } else {
        tolerance_px
    }
}

/// FR-038 (п.4): anchor-надстройка над движком — тонкая коррекция grid-части.
/// `BoundingBox` — движок как есть (ближайший край bbox к линии). `Corner`/
/// `Center` — движок вызывается БЕЗ сетки (to_grid=false), grid-дельта
/// считается вручную для левого-верхнего угла / центра bbox (к пересечению
/// линий); направляющие всегда от движка (п.7 — по краям/центрам); арбитраж
/// grid-vs-guide по осям — минимальная |дельта| (п.11), при равенстве —
/// направляющая (как в движке), ось, выигранная grid, направляющей НЕ
/// помечается (п.9). Collision (п.15) — финальный кламп поверх итоговой
/// дельты. Детерминизм (п.20): только арифметика входа.
fn snap_with_anchor(
    moving: SnapRect,
    dx: f32,
    dy: f32,
    candidates: &[SnapRect],
    cfg: &SnapConfig,
    anchor: SnapAnchor,
) -> SnapOutcome {
    if matches!(anchor, SnapAnchor::BoundingBox) {
        return snap_move(moving, dx, dy, candidates, cfg);
    }
    // Движок без сетки и без collision (клампим сами после anchor-дельты —
    // иначе collision «зафиксирует» дельту до арбитража осей)
    let mut outcome = snap_move(
        moving,
        dx,
        dy,
        candidates,
        &SnapConfig {
            collision_gap: 0.0,
            ..*cfg
        },
    );
    let step = effective_grid_step(cfg);
    let tol = snap_tolerance_world(cfg.tolerance_px, cfg.zoom);
    let (ax, ay) = match anchor {
        // Угол bbox — к пересечению линий (п.4)
        SnapAnchor::Corner => (moving.x, moving.y),
        // Центр bbox — аналогично по центру (п.4)
        _ => (moving.cx(), moving.cy()),
    };
    let grid_step_valid = cfg.to_grid && step > 0.0 && step.is_finite();
    let gx = if grid_step_valid {
        anchor_grid_delta(ax + dx, step, tol)
    } else {
        0.0
    };
    let gy = if grid_step_valid {
        anchor_grid_delta(ay + dy, step, tol)
    } else {
        0.0
    };

    // Арбитраж по осям: guide (уже в outcome — там, где сработал, ось
    // помечена линией) против anchor-grid. Дельта guide по оси = outcome −
    // желаемая. Равенство — направляющая (детерминизм, как в движке п.11).
    let guide_won_x = !outcome.guides_x.is_empty();
    let guide_won_y = !outcome.guides_y.is_empty();
    let guide_dx = outcome.dx - dx;
    let guide_dy = outcome.dy - dy;
    let mut out_dx = if !guide_won_x || gx.abs() < guide_dx.abs() {
        dx + gx
    } else {
        outcome.dx
    };
    let mut out_dy = if !guide_won_y || gy.abs() < guide_dy.abs() {
        dy + gy
    } else {
        outcome.dy
    };
    // Ось, выигранная anchor-grid, направляющей не помечается (п.9)
    let mut guides_x = std::mem::take(&mut outcome.guides_x);
    let mut guides_y = std::mem::take(&mut outcome.guides_y);
    if guide_won_x && out_dx != outcome.dx {
        guides_x.clear();
    }
    if guide_won_y && out_dy != outcome.dy {
        guides_y.clear();
    }
    // Collision поверх финальной дельты (п.15) — как в движке
    if cfg.collision_gap > 0.0 {
        let (fx, fy) = clamp_collision(
            moving,
            moving.x + out_dx,
            moving.y + out_dy,
            candidates,
            cfg.collision_gap,
            out_dx,
            out_dy,
        );
        // Кламп, сдвинувший ось с выигранной позиции, гасит её направляющую
        // (п.9: линия — только там, где снап определил финальную позицию;
        // иначе рендер показывает ось, которой позиция больше не касается)
        if guide_won_x && fx != out_dx {
            guides_x.clear();
        }
        if guide_won_y && fy != out_dy {
            guides_y.clear();
        }
        out_dx = fx;
        out_dy = fy;
    }

    // Источник — по выжившим направляющим (кламп мог погасить оси)
    let source = if !guides_x.is_empty() || !guides_y.is_empty() {
        SnapSource::Guide
    } else {
        SnapSource::Grid
    };

    SnapOutcome {
        dx: out_dx,
        dy: out_dy,
        guides_x,
        guides_y,
        source,
    }
}

/// Шаг клавиатурного nudge в world px (п.22): спецификация v2 задаёт шаг
/// «в пикселях экрана» — визуальный шаг одинаков на любом зуме; перевод в
/// world — делением на зум (screen = world·zoom). Детерминировано: шаг
/// зависит только от зума кадра. Некорректный зум — фолбэк к экранным px.
fn nudge_step_world(screen_px: f32, zoom: f32) -> f32 {
    if zoom > 0.0 {
        screen_px / zoom
    } else {
        screen_px
    }
}

/// FR-038 п.16 (T-038.5): ось «Распределить равномерно» по контексту
/// выделения. Правило (детерминизм п.20): ось с БОЛЬШИМ размахом центров —
/// ряд (размах по X больше) распределяется по X, колонна — по Y; равенство
/// размахов — X. Пары с выравниванием: после «Выровнять по горизонтали»
/// (ряд) распределение идёт по X, после «по вертикали» — по Y. Решение в
/// пользу одного пункта меню (вместо двух) — компактность меню и
/// предсказуемая связка с только что выполненным выравниванием; правило
/// выводится из текущей раскладки, вопрос владельцу — на приёмке FR-038.
fn distribute_axis_for(rects: &[SnapRect]) -> AlignAxis {
    if rects.len() < 2 {
        return AlignAxis::X;
    }
    let span = |axis: AlignAxis| -> f32 {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for rect in rects {
            let c = match axis {
                AlignAxis::X => rect.cx(),
                AlignAxis::Y => rect.cy(),
            };
            lo = lo.min(c);
            hi = hi.max(c);
        }
        hi - lo
    };
    if span(AlignAxis::Y) > span(AlignAxis::X) {
        AlignAxis::Y
    } else {
        AlignAxis::X
    }
}

/// Род batch-операции выделения (FR-038 п.16, T-038.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BatchOp {
    /// «Выровнять…» — ряд/колонна по центрам (общая ось из пункта меню).
    Align,
    /// «Распределить равномерно» — равные зазоры вдоль оси раскладки
    /// (ось из контекста выделения, [`distribute_axis_for`]).
    Distribute,
}

// Буфер обмена ОС (T7, arboard) — M8/W3: за трейтом `ClipboardBackend`
// (нативная реализация — `canvas_shell::clipboard::ArboardClipboard`,
// web — navigator.clipboard); инъекция — в `App::new`.

/// Преобразование [x0, y0, x1, y1] → [x, y, w, h] (point_in_rect-конвенция).
fn rect_xywh(rect: [f32; 4]) -> [f32; 4] {
    [
        rect[0],
        rect[1],
        (rect[2] - rect[0]).max(0.0),
        (rect[3] - rect[1]).max(0.0),
    ]
}

/// Карточка строки шаблона палитры (FR-024/FR-025): подложка + плитка
/// квад-иконки + имя + описание. Общий рендер строк развёрнутого дока и
/// flyout свёрнутой полосы — WYSIWYG: клик по нарисованному. `rect` — xywh.
#[allow(clippy::too_many_arguments)]
fn template_card_row(
    manifest: &canvas_core::templates::TemplateManifest,
    rect: [f32; 4],
    fill: [f32; 4],
    border: [f32; 4],
    palette: &ThemeColors,
    icon_tint: [f32; 4],
    instances: &mut Vec<CardInstance>,
    texts: &mut Vec<OwnedScreenText>,
) {
    instances.push(CardInstance {
        pos: [rect[0], rect[1]],
        size: [rect[2], rect[3]],
        fill,
        border,
        params: [6.0, 0.0, 0.0, 1.0],
    });
    // Плитка иконки (скруглённый квадрат) + квад-иконка роли
    let tile = [
        rect[0] + 8.0,
        rect[1] + (rect[3] - template_ui::TEMPLATE_ROW_TILE) / 2.0,
        template_ui::TEMPLATE_ROW_TILE,
        template_ui::TEMPLATE_ROW_TILE,
    ];
    instances.push(CardInstance {
        pos: [tile[0], tile[1]],
        size: [tile[2], tile[3]],
        fill: palette.palette_tile_fill,
        border: [0.0; 4],
        params: [6.0, 0.0, 0.0, 1.0],
    });
    instances.extend(template_icon_quads(
        template_ui::icon_key(manifest),
        [
            tile[0] + (tile[2] - template_ui::TEMPLATE_ROW_ICON) / 2.0,
            tile[1] + (tile[3] - template_ui::TEMPLATE_ROW_ICON) / 2.0,
            template_ui::TEMPLATE_ROW_ICON,
            template_ui::TEMPLATE_ROW_ICON,
        ],
        icon_tint,
    ));
    texts.push(OwnedScreenText {
        text: manifest.display_name().to_owned(),
        origin: [tile[0] + tile[2] + 8.0, rect[1] + 5.0],
        width: rect[2] - (tile[2] + 24.0),
        font_size: 13.0,
        color: palette.title,
        align: TextAlign::Left,
    });
    texts.push(OwnedScreenText {
        text: manifest.description.clone(),
        origin: [tile[0] + tile[2] + 8.0, rect[1] + 21.0],
        width: rect[2] - (tile[2] + 24.0),
        font_size: 11.0,
        color: palette.body,
        align: TextAlign::Left,
    });
}

/// Заголовок ноды для поиска/результатов (T14): имя файла или текст заметки.
fn node_title(node: &Node) -> &str {
    if let Some(file) = node.file.as_ref() {
        return Path::new(file)
            .file_name()
            .map(|name| name.to_str().unwrap_or(file))
            .unwrap_or(file);
    }
    node.text.as_deref().unwrap_or("Заметка")
}

/// Полный текст ноды для in-memory поиска (T14): содержимое заметки.
fn node_text(node: &Node) -> &str {
    node.text.as_deref().unwrap_or("")
}

/// Подзаголовок строки результата (T14): «заметка» или родительский каталог.
fn node_subtitle(node: &Node) -> &str {
    if node.file.is_some() {
        "файл"
    } else {
        "заметка"
    }
}

/// Подзаголовок FTS-хита (T14): имя родительского каталога пути.
fn hit_subtitle(path: &Path) -> String {
    path.parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "файл".to_owned())
}

/// CR-012 (правка 2): точная требуемая высота — тело измеряется реальным
/// шейпингом (`measure_body_height`: те же встроенные Noto-шрифты и
/// mono-сегментация формульных строк, что у рендера). Оценка среднего
/// аванса принципиально хрупка — измерение устраняет класс дефектов
/// «футер налезает на перенос». Формула та же, что и у
/// [`estimated_result_reserve_height`], с измеренной высотой тела.
pub fn measured_result_reserve_height(text: &str, node_width: f32, formula_lines: &[usize]) -> f32 {
    let body_width = (node_width - BODY_PADDING * 2.0).max(BODY_PADDING);
    let body = measure_body_height(text, body_width, formula_lines);
    HEADER_HEIGHT + BODY_TOP_GAP + body + BODY_PADDING + RESULT_LINE_HEIGHT + 2.0
}

/// FR-020: slug из имени шаблона: латиница/цифры/дефисы, кириллица —
/// транслитерация (решение владельца: «Нагрузка» → «nagruzka»).
/// Прочие символы — дефис; сжатие подряд идущих; обрезка краёв.
fn slugify(name: &str) -> String {
    const TRANSLIT: &[(&str, &str)] = &[
        ("а", "a"),
        ("б", "b"),
        ("в", "v"),
        ("г", "g"),
        ("д", "d"),
        ("е", "e"),
        ("ё", "e"),
        ("ж", "zh"),
        ("з", "z"),
        ("и", "i"),
        ("й", "y"),
        ("к", "k"),
        ("л", "l"),
        ("м", "m"),
        ("н", "n"),
        ("о", "o"),
        ("п", "p"),
        ("р", "r"),
        ("с", "s"),
        ("т", "t"),
        ("у", "u"),
        ("ф", "f"),
        ("х", "h"),
        ("ц", "ts"),
        ("ч", "ch"),
        ("ш", "sh"),
        ("щ", "sch"),
        ("ъ", ""),
        ("ы", "y"),
        ("ь", ""),
        ("э", "e"),
        ("ю", "yu"),
        ("я", "ya"),
    ];
    let lower = name.to_lowercase();
    let mut out = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if let Some((_, latin)) = TRANSLIT.iter().find(|(c, _)| *c == ch.to_string()) {
            out.push_str(latin);
        } else if ch == ' ' || ch == '_' || ch == '-' || ch == '.' || ch == '/' {
            out.push('-');
        }
        // прочие символы (эмодзи, знаки) — пропускаются
    }
    let mut collapsed = String::new();
    for ch in out.chars() {
        if ch == '-' && collapsed.ends_with('-') {
            continue;
        }
        collapsed.push(ch);
    }
    let trimmed = collapsed.trim_matches('-');
    if trimmed.is_empty() {
        "custom".to_owned()
    } else {
        trimmed.chars().take(48).collect()
    }
}

/// FR-020: тип параметра по токену единицы (подсказка UI в манифесте).
fn infer_param_type(unit: Option<&str>) -> canvas_core::templates::ParamType {
    use canvas_core::templates::ParamType;
    match unit {
        Some("rps") | Some("req/s") => ParamType::Rate,
        Some("ms") | Some("s") | Some("sec") | Some("secs") | Some("min") | Some("h")
        | Some("hour") => ParamType::Time,
        Some("B") | Some("KB") | Some("MB") | Some("GB") => ParamType::Bytes,
        Some("%") => ParamType::Percent,
        Some("req") | Some("reqs") => ParamType::Count,
        _ => ParamType::Scalar,
    }
}

/// FR-020: уникальный id custom-шаблона: базовый slug; конфликт с
/// существующим id (реестр или файловая система) — суффикс `-2`, `-3`…
fn unique_custom_id(
    base: &str,
    registry: &canvas_core::templates::TemplateRegistry,
    root: &std::path::Path,
) -> String {
    let base = base.chars().take(48).collect::<String>();
    let exists = |id: &str| registry.find(id).is_some() || root.join(id).exists();
    if !exists(&base) {
        return base;
    }
    for n in 2..=1000u32 {
        let candidate = format!("{base}-{n}");
        if !exists(&candidate) {
            return candidate;
        }
    }
    // Практически недостижимо — последний рубеж: метка времени
    format!(
        "{base}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    )
}

/// FR-013 (правка 4): зона наведения бейджа ошибки формульной строки под
/// курсором (все координаты — логические px окна). Вынесено из App для
/// прямого unit-тестирования.
fn expr_error_hit_at(hits: &[LineErrorHit], cursor: [f32; 2]) -> Option<&LineErrorHit> {
    hits.iter().find(|hit| {
        let [x, y, w, h] = hit.rect;
        cursor[0] >= x && cursor[0] <= x + w && cursor[1] >= y && cursor[1] <= y + h
    })
}

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
}

impl AppDialog {
    /// Кнопки диалога (screen-space rect'ы считаются от центра окна).
    /// Подписи — таблица i18n (FR-040), `language` — язык интерфейса.
    /// FR-050 Н4: ReplaceSource — «Заменить»/«Отмена» (не Да/Нет).
    fn buttons(&self, language: Language) -> [(&'static str, bool); 2] {
        // (подпись, confirm?)
        match self {
            AppDialog::ReplaceSource { .. } => [
                (i18n::tr(language, keys::DIALOG_REPLACE_YES), true),
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

/// FR-050 Н2 (этап C): id ноды-истока активного drag (None — drag не
/// активен). Свободная функция — для `compute_param_drop` (self-edge
/// исключается из подсветки целей).
fn drag_from_node(drag: Option<&EdgeDrag>) -> Option<String> {
    match drag {
        Some(EdgeDrag::New { from_node, .. }) => Some(from_node.clone()),
        _ => None,
    }
}

/// FR-050 Н4 (этап C): подпись ноды-источника для диалога «Заменить
/// источник?» — снимок имени шаблона (FR-023) / первая непустая строка
/// текста / label / id (тот же приоритет, что у `spill_source_title`
/// ядра; публичная копия — core не экспортирует ту).
fn node_display_label(node: &Node) -> String {
    if let Some(name) = node
        .template()
        .and_then(|template| template.name)
        .filter(|name| !name.is_empty())
    {
        return name;
    }
    if let Some(first) = node
        .text
        .as_deref()
        .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
    {
        return first.trim().to_owned();
    }
    node.label.clone().unwrap_or_else(|| node.id.clone())
}

/// Ярление заливки для hover-подсветки (кнопки настроек/темы): практика
/// аффорданса — интерактивная кнопка отвечает на курсор.
fn hover_fill(c: [f32; 4]) -> [f32; 4] {
    [
        (c[0] * 1.3 + 0.04).min(1.0),
        (c[1] * 1.3 + 0.04).min(1.0),
        (c[2] * 1.3 + 0.06).min(1.0),
        c[3],
    ]
}

/// FR-026: пересекаются ли два rect `[x, y, w, h]` — куллинг текстов строк
/// панели, перекрытых выпадающим меню (квады рисуются до screen-текстов).
fn rects_intersect(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
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
    /// PRD-0007 (FR-048 X2): открытое окно проверки цепочки расчёта
    /// (Loading/Ready; Stale — чип внутри). None — окно закрыто. Взаимо-
    /// исключителен с main stage (F-10) — открытие закрывает stage и наоборот.
    explain: Option<ExplainState>,
    /// PRD-0007 (AC-3.3, §9.2): сессионный кэш снапшота объяснения —
    /// переживает закрытие окна; переоткрытие того же корня — мгновенно
    /// (≤ 1 с, G1), без перестройки; чип — если модель изменилась.
    explain_cache: Option<ExplainSnapshot>,
    /// FR-042 (E2): ребро пучка под курсором (live-индекс) — hover-бамп
    /// агрегированной линии; вычисляется на каждый кадр ввода (паттерн
    /// `hovered`), в кэш не пишется.
    bundle_hover: Option<usize>,
    /// FR-013 (правка 4): зоны наведения бейджей ошибок формульных строк с
    /// прошлого кадра (логические px + текст ошибки). Заполняется после
    /// рендера, используется в сборке оверлея кадра (тултип у курсора —
    /// паттерн тултипа битой ссылки T10). Отставание в кадр незаметно.
    expr_error_hits: Vec<LineErrorHit>,
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
    /// FR-012: цель «втягивания» во время drag — группа под центром
    /// перетаскиваемой ноды (зона подсвечивается, отпускание — вставка).
    group_drop_target: Option<usize>,
    /// FR-012: settle-анимация после вставки в группу — плавный проезд
    /// группы и раздвинутых соседей к целевым позициям (~250 мс).
    settle_anim: Option<SettleAnim>,
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
            thumbs_failed: std::collections::HashSet::new(),
            editing: None,
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
            // PRD-0007 (X2): окно проверки закрыто, сессионный кэш пуст
            explain: None,
            explain_cache: None,
            bundle_hover: None,
            expr_error_hits: Vec::new(),
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
            group_drop_target: None,
            settle_anim: None,
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
        let body_h = measure_body_height(&live_text, body_width, &formula_lines);
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
                    EditTarget::Edge(_) => None,
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
            }
            self.scene.mark_dirty();
        } else {
            // FR-006: отмена правки — модель не менялась, отложенный
            // снапшот «до» дропается (no-op шагов в истории нет)
            self.pending_undo = None;
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
    fn whatif_bar_layout(&self) -> whatif_ui::BarLayout {
        let viewport = self.viewport_logical();
        let names: Vec<String> = self
            .scene
            .scenarios
            .iter()
            .map(|scenario| scenario.name.clone())
            .collect();
        let count = self.scene.whatif_override_count();
        whatif_ui::bar_layout(&names, count, viewport)
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

    /// FR-017: данные таблицы сравнения (колонки + ячейки). Строки — union
    /// подменённых переменных всех сценариев; значения — прогон
    /// `propagate_with_lines` с подменами каждого сценария (по прогону на
    /// сценарий — 3–5 прогонов <10 мс, допустимо по роадмапу).
    fn whatif_compare_table(&self) -> (Vec<String>, Vec<Vec<String>>) {
        let mut columns = vec![
            self.tr(keys::WHATIF_COLUMN_VAR).to_owned(),
            self.tr(keys::WHATIF_BASE).to_owned(),
        ];
        for scenario in &self.scene.scenarios {
            columns.push(scenario.name.clone());
        }
        // Ключи: union валидных подмен всех сценариев.
        let mut keys: Vec<(String, usize)> = Vec::new();
        for scenario in &self.scene.scenarios {
            for key in canvas_core::whatif::active_line_exprs(&self.scene.canvas, scenario).keys() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
        }
        keys.sort();
        // Одно решение на сценарий (пустой сценарий — None: все ячейки «—»).
        let per_scenario: Vec<Option<flow::FlowSolutions>> = self
            .scene
            .scenarios
            .iter()
            .map(|scenario| {
                let line_exprs =
                    canvas_core::whatif::active_line_exprs(&self.scene.canvas, scenario);
                if line_exprs.is_empty() {
                    return None;
                }
                let overrides = flow::WhatIfOverrides {
                    line_exprs,
                    ..Default::default()
                };
                flow::propagate_with_lines(&self.scene.canvas, &overrides).ok()
            })
            .collect();
        let base_lines = &self.scene.flow_baseline.lines;
        let rows: Vec<Vec<String>> = keys
            .iter()
            .map(|(node_id, line)| {
                let label = format!("{} : стр. {}", self.whatif_node_label(node_id), line + 1);
                let key = (node_id.clone(), *line);
                let base = base_lines.get(&key);
                let mut row = vec![
                    label,
                    base.map(|value| value.to_string())
                        .unwrap_or_else(|| "—".to_owned()),
                ];
                for solutions in &per_scenario {
                    let Some(solutions) = solutions else {
                        row.push("—".to_owned());
                        continue;
                    };
                    let Some(value) = solutions.lines.get(&key) else {
                        row.push("—".to_owned());
                        continue;
                    };
                    let cell = match base {
                        Some(base) => match whatif_delta_str(base, value) {
                            Some(delta) => format!("{value} ({delta})"),
                            None => value.to_string(),
                        },
                        None => value.to_string(),
                    };
                    row.push(cell);
                }
                row
            })
            .collect();
        (columns, rows)
    }

    /// Диспетчер кликов по нижнему бару (пилюля вне режима).
    fn apply_whatif_bar_action(&mut self, action: BarAction) {
        match action {
            BarAction::Enter => self.enter_whatif_mode(),
            BarAction::Close => self.exit_whatif_mode(),
            BarAction::Base => self.scene.whatif_activate(None),
            BarAction::Scenario(index) => self.scene.whatif_activate(Some(index)),
            BarAction::NewScenario => {
                // CR-016: имя — пустое, scene выбирает первый свободный
                // номер (иначе len+1 коллидирует с существующими).
                let snapshot = self.scene.canvas.clone();
                match self.scene.whatif_create_scenario("") {
                    Ok(index) => {
                        // Список сценариев персистентен: мутация
                        // `canvasdesk.whatif` одним undo-шагом (FR-006).
                        canvas_core::whatif::scenarios_to_canvas(
                            &mut self.scene.canvas,
                            &self.scene.scenarios,
                        );
                        if self.scene.canvas != snapshot {
                            self.scene.push_undo(snapshot);
                            self.scene.mark_dirty();
                        }
                        self.scene.whatif_activate(Some(index));
                    }
                    Err(err) => self.show_toast(err),
                }
            }
            BarAction::ToggleOverrides => {
                self.whatif_list_open = !self.whatif_list_open;
                if self.whatif_list_open {
                    self.whatif_compare_open = false;
                }
            }
            BarAction::Apply => {
                // Паттерн MCP whatif_apply: записать подмены в persisted-
                // строки/params, удалить сценарий (Q6b) — один undo-шаг.
                if self.scene.active_scenario.is_none() {
                    return;
                }
                let snapshot = self.scene.canvas.clone();
                let applied = self.scene.whatif_apply_active();
                canvas_core::whatif::scenarios_to_canvas(
                    &mut self.scene.canvas,
                    &self.scene.scenarios,
                );
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                    self.scene.mark_dirty();
                }
                self.whatif_list_open = false;
                self.scene.recompute_flow();
                self.show_toast(
                    self.trf(keys::TOAST_APPLY_DONE, &[("{count}", &applied.to_string())]),
                );
            }
            BarAction::Reset => {
                // Сброс подмен активного сценария — runtime-only.
                if let Some(index) = self.scene.active_scenario {
                    if let Some(scenario) = self.scene.scenarios.get_mut(index) {
                        scenario.line_exprs.clear();
                    }
                    self.scene.recompute_flow();
                }
            }
            BarAction::Compare => {
                self.whatif_compare_open = !self.whatif_compare_open;
                if self.whatif_compare_open {
                    self.whatif_list_open = false;
                }
            }
        }
        self.request_redraw();
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
        let rel_y = world[1] - origin[1];
        if rel_y < 0.0 {
            return None;
        }
        let line_count = text.split('\n').count();
        let results = expr::eval_lines(text);
        let formula = formula_line_indices(&results);
        for k in 0..line_count {
            let prefix = text.split('\n').take(k + 1).collect::<Vec<_>>().join("\n");
            let formula_prefix: Vec<usize> = formula.iter().copied().filter(|i| *i <= k).collect();
            let cumulative = measure_body_height(&prefix, width, &formula_prefix);
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

    /// FR-017: оверлей нижнего бара (пилюля вне режима, полоса, список
    /// подмен, таблица сравнения). Screen-space — константный размер при
    /// любом зуме (паттерн `canvas_menu_overlay`).
    fn whatif_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        let palette = self.effective_palette();
        let accent = Color::rgb(0x4c, 0xa6, 0xff);
        let dim = Color::rgb(0x8a, 0x90, 0x9c);
        if !self.scene.whatif_active {
            // Свёрнутый вид: пилюля входа.
            let pill = whatif_ui::enter_pill_rect(viewport);
            instances.push(CardInstance {
                pos: [pill[0], pill[1]],
                size: [pill[2], pill[3]],
                fill: palette.menu_fill,
                border: [0.35, 0.40, 0.50, 1.0],
                params: [pill[3] / 2.0, 0.0, 0.0, 1.0],
            });
            let (pill_box, pill_width) = centered_box(pill, 4.0);
            texts.push(OwnedScreenText {
                text: self.tr(keys::WHATIF_PILL).to_owned(),
                origin: [pill_box[0], pill[1] + 8.0],
                width: pill_width,
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Center,
            });
            return (instances, texts);
        }
        let layout = self.whatif_bar_layout();
        let chip = |rect: [f32; 4], fill: [f32; 4], border: [f32; 4]| CardInstance {
            pos: [rect[0], rect[1]],
            size: [rect[2], rect[3]],
            fill,
            border,
            params: [6.0, 0.0, 0.0, 1.0],
        };
        instances.push(CardInstance {
            pos: [layout.rect[0], layout.rect[1]],
            size: [layout.rect[2], layout.rect[3]],
            fill: palette.menu_fill,
            border: [0.35, 0.40, 0.50, 1.0],
            params: [8.0, 0.0, 0.0, 1.0],
        });
        // Индикатор режима.
        texts.push(OwnedScreenText {
            text: "WHAT-IF".to_owned(),
            origin: [layout.indicator[0], layout.indicator[1] + 6.0],
            width: layout.indicator[2],
            font_size: 12.0,
            color: accent,
            align: TextAlign::Center,
        });
        // Чипы: «База» + сценарии + «+». Активный — акцентной рамкой.
        let active = self.scene.active_scenario;
        let mut chip_text = |rect: [f32; 4], label: &str, current: bool| {
            instances.push(chip(
                rect,
                if current {
                    canvas_core::tokens::DIALOG_BUTTON_PRIMARY
                } else {
                    canvas_core::tokens::DIALOG_BUTTON_SECONDARY
                },
                if current {
                    canvas_core::tokens::WHATIF_CHIP
                } else {
                    canvas_core::tokens::DIALOG_BUTTON_BORDER
                },
            ));
            let (box_origin, box_width) = centered_box(rect, 3.0);
            texts.push(OwnedScreenText {
                text: label.to_owned(),
                origin: [box_origin[0], rect[1] + 6.0],
                width: box_width,
                font_size: 13.0,
                color: if current {
                    token_color(canvas_core::tokens::DIALOG_TEXT)
                } else {
                    palette.title
                },
                align: TextAlign::Center,
            });
        };
        chip_text(layout.base, self.tr(keys::WHATIF_BASE), active.is_none());
        for (i, rect) in layout.scenarios.iter().enumerate() {
            // CR-015: подпись через `chip_label` — тот же кап «…», что в
            // раскладке чипа (иначе текст шире чипа и переливается).
            let label = self
                .scene
                .scenarios
                .get(i)
                .map(|scenario| whatif_ui::chip_label(&scenario.name))
                .unwrap_or_default();
            chip_text(*rect, &label, active == Some(i));
        }
        chip_text(layout.new_scenario, "+", false);
        // Счётчик подмен (клик — список; раскрыт — акцент).
        let count = self.scene.whatif_override_count();
        let counter_label = self.trf(keys::WHATIF_OVERRIDES, &[("{count}", &count.to_string())]);
        chip_text(
            layout.overrides,
            &counter_label,
            self.whatif_list_open && count > 0,
        );
        // Кнопки. Apply/Сброс — без активного сценария/подмен приглушены.
        let has_overrides = active.is_some() && count > 0;
        let buttons = [
            (layout.apply, self.tr(keys::WHATIF_APPLY), has_overrides),
            (layout.reset, self.tr(keys::WHATIF_RESET), has_overrides),
            (
                layout.compare,
                self.tr(keys::WHATIF_COMPARE),
                !self.scene.scenarios.is_empty(),
            ),
            (layout.close, "✕", true),
        ];
        for (rect, label, enabled) in buttons {
            instances.push(chip(
                rect,
                if enabled {
                    canvas_core::tokens::DIALOG_BUTTON_SECONDARY
                } else {
                    canvas_core::tokens::WHATIF_CHIP_DIM
                },
                canvas_core::tokens::DIALOG_BUTTON_BORDER,
            ));
            let (box_origin, box_width) = centered_box(rect, 3.0);
            texts.push(OwnedScreenText {
                text: label.to_owned(),
                origin: [box_origin[0], rect[1] + 6.0],
                width: box_width,
                font_size: 13.0,
                color: if enabled { palette.title } else { dim },
                align: TextAlign::Center,
            });
        }
        // Раскрытый список подмен.
        if self.whatif_list_open {
            let rows = self.whatif_override_rows();
            let list = whatif_ui::overrides_list_layout(layout.rect, rows.len(), viewport);
            instances.push(CardInstance {
                pos: [list[0], list[1]],
                size: [list[2], list[3]],
                fill: palette.menu_fill,
                border: [0.35, 0.40, 0.50, 1.0],
                params: [6.0, 0.0, 0.0, 1.0],
            });
            if rows.is_empty() {
                texts.push(OwnedScreenText {
                    text: self.tr(keys::WHATIF_NO_OVERRIDES).to_owned(),
                    origin: [
                        list[0] + whatif_ui::LIST_MARGIN,
                        list[1] + whatif_ui::LIST_MARGIN + 4.0,
                    ],
                    width: list[2] - whatif_ui::LIST_MARGIN * 2.0,
                    font_size: 13.0,
                    color: dim,
                    align: TextAlign::Left,
                });
            }
            for (i, row) in rows.iter().enumerate() {
                let rect = whatif_ui::override_row_rect(list, i);
                let mut text = format!(
                    "{} → стр. {}: {} → {}",
                    row.node_label,
                    row.line + 1,
                    row.base,
                    row.whatif
                );
                let color = if row.stale.is_some() {
                    text = format!("{text}  ⚠ {}", row.stale.as_deref().unwrap_or_default());
                    Color::rgb(0xe5, 0x5c, 0x5c)
                } else {
                    palette.title
                };
                texts.push(OwnedScreenText {
                    text,
                    origin: [rect[0] + 4.0, rect[1] + 4.0],
                    width: rect[2] - whatif_ui::REMOVE_WIDTH - 8.0,
                    font_size: 13.0,
                    color,
                    align: TextAlign::Left,
                });
                let remove = whatif_ui::remove_button_rect(list, i);
                texts.push(OwnedScreenText {
                    text: "✕".to_owned(),
                    origin: [remove[0], remove[1] + 2.0],
                    width: remove[2],
                    font_size: 13.0,
                    color: dim,
                    align: TextAlign::Center,
                });
            }
        }
        // Таблица сравнения сценариев.
        if self.whatif_compare_open {
            let (columns, rows) = self.whatif_compare_table();
            let table = whatif_ui::table_layout(&columns, rows.len(), layout.rect, viewport);
            instances.push(CardInstance {
                pos: [table.rect[0], table.rect[1]],
                size: [table.rect[2], table.rect[3]],
                fill: palette.menu_fill,
                border: [0.35, 0.40, 0.50, 1.0],
                params: [6.0, 0.0, 0.0, 1.0],
            });
            for (c, rect) in table.header.iter().enumerate() {
                texts.push(OwnedScreenText {
                    text: columns.get(c).cloned().unwrap_or_default(),
                    origin: [rect[0] + 6.0, rect[1] + 6.0],
                    width: rect[2] - 12.0,
                    font_size: 12.0,
                    color: if c == 0 { dim } else { accent },
                    align: TextAlign::Left,
                });
            }
            for (r, row) in rows.iter().enumerate() {
                for (c, cell) in row.iter().enumerate() {
                    let Some(rect) = table.cells.get(r).and_then(|cells| cells.get(c)) else {
                        continue;
                    };
                    texts.push(OwnedScreenText {
                        text: cell.clone(),
                        origin: [rect[0] + 6.0, rect[1] + 4.0],
                        width: rect[2] - 12.0,
                        font_size: 13.0,
                        color: if c == 0 { palette.title } else { palette.body },
                        align: TextAlign::Left,
                    });
                }
            }
        }
        (instances, texts)
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
    /// undo-стека, текущее состояние — в redo.
    fn undo_action(&mut self) {
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
        self.group_drop_target = None;
        self.scene.canvas = canvas;
        self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
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
        let solutions = &self.scene.flow_active;
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
        let solutions = &self.scene.flow_active;
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

    /// FR-050 Н2 (этап C): открыть меню выбора у курсора (screen-space).
    fn open_choice_menu(&mut self, title_key: &'static str, items: Vec<ChoiceItem>) {
        if items.is_empty() {
            return;
        }
        let viewport = self.viewport_logical();
        // Геометрия — та же, что у контекстного меню (T7): ширина/высота
        // пунктов константны (+ строка заголовка), меню не выезжает за
        // правый/нижний край окна
        let count = items.len();
        let [_, _, _, mh] = crate::ui::menu_rect_for([0.0; 2], count);
        let mh = mh + CHOICE_MENU_TITLE_H;
        let origin = [
            (self.cursor[0] + 8.0).min((viewport[0] - crate::ui::MENU_WIDTH).max(0.0)),
            (self.cursor[1] + 8.0).min((viewport[1] - mh).max(0.0)),
        ];
        self.choice_menu = Some(ChoiceMenu {
            origin,
            title_key,
            items,
            hovered: None,
        });
        self.request_redraw();
    }

    /// FR-050 Н2 (этап C): применить пункт меню выбора — создать ребро
    /// (параметр — с toParam; строка-источник — from_line + уже выбранная
    /// цель; проверки занятости/цикла — в connect_to_param).
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
        }
    }

    /// FR-050 Н2 (этап C): создать value-ребро с `toParam` (общий путь
    /// drop на якорь / меню выбора / замены источника). Undo-шаг (FR-006),
    /// живой пересчёт потока — значение сразу перекрывает локальное (Р-1).
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
            to_node,
            Some(to_side),
        );
        edge.set_flow_kind(FlowKind::Value);
        edge.from_line = from_line;
        edge.to_param = Some(param);
        self.push_undo();
        self.scene.canvas.add_edge(edge);
        self.scene.mark_dirty();
        self.scene.recompute_flow();
        self.request_redraw();
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
            if resize {
                CursorIcon::NwseResize
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

    /// Клиентские ФИЗИЧЕСКИЕ px от shell (DragEvent) -> world-координаты:
    /// делим на scale_factor (масштаб учтён), затем через камеру (T9).
    fn drag_world_pt(&self, pt: (f32, f32)) -> Vec2 {
        let scale = self.scale_factor();
        let logical = [pt.0 / scale, pt.1 / scale];
        self.camera
            .screen_to_world(logical, self.viewport_logical())
    }

    /// События drag-drop (T9): превью зоны на Enter/Over, вставка нод на
    /// Drop. Данные приходят сырыми из shell, план строит crate::ui.
    fn on_drag_event(&mut self, drag: canvas_core::dragdrop::DragEvent) {
        use crate::ui::{plan_drop, DropInsertKind};
        match drag {
            canvas_core::dragdrop::DragEvent::Enter { data, client_pt } => {
                let world = self.drag_world_pt(client_pt);
                // T21-B: дроп одиночной папки с widget.json — призрак
                // установки виджета (перехват ДО plan_drop файлов)
                if let Some(src) = crate::ui::dropped_widget_package(&data) {
                    match canvas_widgets::manifest::WidgetManifest::from_dir(&src) {
                        Ok(manifest) => {
                            self.drop_preview = Some(DropPreview {
                                origin: world,
                                plan: vec![crate::ui::DropInsert {
                                    id: "widget-install".to_owned(),
                                    kind: crate::ui::DropInsertKind::InstallWidget(
                                        src,
                                        manifest.name.clone(),
                                    ),
                                    pos: world,
                                }],
                            });
                            self.request_redraw();
                            return;
                        }
                        // Битый манифест: честный призрак-ошибка + toast,
                        // как «файл недоступен» у битых ссылок (SPEC §7.5)
                        Err(e) => {
                            self.show_toast(self.trf(
                                keys::TOAST_WIDGET_NOT_INSTALLED,
                                &[("{err}", &e.to_string())],
                            ));
                            self.drop_preview = None;
                            self.request_redraw();
                            return;
                        }
                    }
                }
                let plan = plan_drop(&self.scene.canvas, &data, world);
                // Пустой план (нет поддерживаемых форматов) — не подсвечиваем
                self.drop_preview = if plan.is_empty() {
                    None
                } else {
                    Some(DropPreview {
                        origin: world,
                        plan,
                    })
                };
            }
            canvas_core::dragdrop::DragEvent::Over { client_pt } => {
                // Сетка призраков следует за курсором, сам план не меняется
                let world = self.drag_world_pt(client_pt);
                if let Some(preview) = self.drop_preview.as_mut() {
                    preview.origin = world;
                }
            }
            canvas_core::dragdrop::DragEvent::Leave => self.drop_preview = None,
            canvas_core::dragdrop::DragEvent::Drop { data, client_pt } => {
                let world = self.drag_world_pt(client_pt);
                // T21-B: дроп виджет-пакета — диалог П10 (Да/Нет), установка
                // и нода только после подтверждения; невалидный манифест —
                // toast (повторно не парсим успех — уже в призраке)
                if let Some(src) = crate::ui::dropped_widget_package(&data) {
                    match canvas_widgets::manifest::WidgetManifest::from_dir(&src) {
                        Ok(manifest) => {
                            let updating = self.widgets.registry.contains(&manifest.id);
                            self.dialog = Some(AppDialog::InstallWidget {
                                src,
                                manifest,
                                pos: world,
                                updating,
                            });
                        }
                        Err(e) => {
                            self.show_toast(self.trf(
                                keys::TOAST_WIDGET_NOT_INSTALLED,
                                &[("{err}", &e.to_string())],
                            ));
                        }
                    }
                    self.drop_preview = None;
                    self.request_redraw();
                    return;
                }
                // План пересчитываем по СВЕЖИМ данным Drop (не из превью,
                // план T9 §5): источник мог обновить содержимое
                let plan = plan_drop(&self.scene.canvas, &data, world);
                if !plan.is_empty() {
                    // FR-006: дроп файлов/заметок — undo-шаг
                    self.push_undo();
                }
                let mut last: Option<usize> = None;
                for ins in plan {
                    let node = match ins.kind {
                        DropInsertKind::File(path) => Node::file(
                            ins.id,
                            path.to_string_lossy().into_owned(),
                            ins.pos[0],
                            ins.pos[1],
                            crate::ui::DROP_CARD_W,
                            crate::ui::DROP_CARD_H,
                        ),
                        DropInsertKind::Note(text) => {
                            Node::text(ins.id, text, ins.pos[0], ins.pos[1])
                        }
                        // Установка виджета перехвачена выше (T21-B: дроп
                        // открывает диалог, не вставляет ноду напрямую) —
                        // сюда попасть не можем; рамка на случай будущих
                        // прямых вставок (MCP widget_add — T22+)
                        DropInsertKind::InstallWidget(_, _) => {
                            tracing::warn!("дроп виджета прошёл мимо диалога — пропущен");
                            continue;
                        }
                    };
                    // Вставка как в create_note_at: модель + spatial index
                    self.scene.canvas.nodes.push(node);
                    let index = self.scene.canvas.nodes.len() - 1;
                    let node_ref = &self.scene.canvas.nodes[index];
                    self.scene.spatial.insert(index, node_ref);
                    last = Some(index);
                }
                if let Some(index) = last {
                    // Выделяем последнюю ноду группы; тамбнейлы закажет
                    // order_thumbnails в ближайшем кадре, автосейв — сам
                    self.selected = Some(Selection::Node(index));
                    self.scene.mark_dirty();
                }
                // Поисковый индекс (T14): сброшенные файлы — сразу в FTS
                let canvas_dir = self.scene.canvas_dir();
                for node in &self.scene.canvas.nodes {
                    let Some(file) = node.file.as_ref() else {
                        continue;
                    };
                    self.search_service.command(SearchCommand::IndexFile {
                        path: resolve_node_path(file, &canvas_dir),
                        display_name: Path::new(file)
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| file.clone()),
                    });
                }
                // Дроп мог добавить файловые ноды в новые директории —
                // синхронизируем вотчер (T10)
                self.sync_watch_dirs();
                self.drop_preview = None;
            }
        }
        self.request_redraw();
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

    /// Пересобрать/обновить миникарту (T13, SPEC §6.1): не каждый кадр, а по
    /// dirty-условиям — правки сцены (dirty_until save), движение камеры
    /// (пан/зум двигают рамку viewport) или смена размера буфера (DPI/resize).
    fn update_minimap(&mut self) {
        // Вычисления (immutable) — до mutable borrow рендерера
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return;
        }
        let scale = self.scale_factor();
        let width_px = (MINIMAP_W as f32 * scale).round().max(1.0) as u32;
        let height_px = (MINIMAP_H as f32 * scale).round().max(1.0) as u32;
        let sig = (
            self.camera.position(),
            self.camera.zoom(),
            width_px,
            height_px,
        );
        let size_changed = self
            .minimap_sig
            .is_some_and(|prev| prev.2 != width_px || prev.3 != height_px);
        // dirty_until-автосейва: правки сцены пересобирают снимок; между
        // правкой и сейвом (2 с debounce) каждый запрошенный кадр обновляет
        // миникарту — это и есть видимость перемещений в реальном времени
        let scene_dirty = self.scene.dirty_since.is_some();
        if self.minimap.is_some() && self.minimap_sig == Some(sig) && !scene_dirty {
            return;
        }
        let viewport_world = self.camera.visible_world_rect(viewport);
        if self.minimap.is_none() || scene_dirty || size_changed {
            // сцена/размер изменились — полный снимок (T13-A)
            // FR-011: свернутые поддеревья не рисуются на миникарте
            let hidden = self.hidden_subtree_nodes();
            let scene_view = if hidden.is_empty() {
                self.scene.canvas.clone()
            } else {
                let mut filtered = self.scene.canvas.clone();
                filtered.nodes = filtered
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| hidden.binary_search(i).is_err())
                    .map(|(_, node)| node.clone())
                    .collect();
                filtered.edges.retain(|edge| {
                    let from_exists = filtered.nodes.iter().any(|node| node.id == edge.from_node);
                    let to_exists = filtered.nodes.iter().any(|node| node.id == edge.to_node);
                    from_exists && to_exists
                });
                filtered
            };
            self.minimap = Some(Minimap::capture(
                &scene_view,
                viewport_world,
                width_px,
                height_px,
            ));
        } else if let Some(minimap) = self.minimap.as_mut() {
            // только камера — пересчёт подгонки и рамки (дешевле снимка)
            minimap.set_viewport(viewport_world);
        }
        self.minimap_sig = Some(sig);
        // Загрузка текстуры (mutable borrow) — кадр растеризован заранее
        if let Some(minimap) = self.minimap.as_ref() {
            let image = minimap.render();
            if let Some(renderer) = self.renderer.as_mut() {
                renderer.set_minimap(&image);
            }
        }
    }

    /// Прямоугольник миникарты в логических px (T13): None — не задана или
    /// окно меньше 252×172 (квад скрыт).
    fn minimap_rect(&self) -> Option<[f32; 4]> {
        self.renderer
            .as_ref()
            .and_then(|renderer| renderer.minimap_rect_logical())
    }

    /// Центрировать камеру на world-точке под курсором мыши в миникарте
    /// (T13): клик — прыжок, drag — world-точка следует за курсором.
    fn center_camera_on_minimap_cursor(&mut self) {
        let Some(minimap) = self.minimap.as_ref() else {
            return;
        };
        let Some(rect) = self.minimap_rect() else {
            return;
        };
        // rect — логические px, маппинг минимапы — в физических буфера
        let scale = self.scale_factor();
        let px = [
            (self.cursor[0] - rect[0]) * scale,
            (self.cursor[1] - rect[1]) * scale,
        ];
        self.camera.set_center(minimap.map_to_world(px));
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

    /// Оверлей панели поиска (T14): квады + тексты в screen-space
    /// (FrameOverlay), геометрия — search_ui::layout.
    fn search_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if !self.search.is_open() || viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let lay = search_layout(viewport[0], viewport[1], &self.search);
        let palette = self.effective_palette();
        let panel = rect_xywh(lay.panel_rect);
        instances.push(CardInstance {
            pos: [panel[0], panel[1]],
            size: [panel[2], panel[3]],
            fill: palette.menu_fill,
            border: [0.22, 0.24, 0.30, 0.9],
            params: [8.0, 0.0, 0.0, 1.0],
        });
        let input = rect_xywh(lay.input_rect);
        instances.push(CardInstance {
            pos: [input[0], input[1]],
            size: [input[2], input[3]],
            fill: palette.search_input_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        // Каретка — литерал «|» в конце текста (MVP, без мерцания)
        let query_with_caret = format!("{}|", self.search.input.query());
        texts.push(OwnedScreenText {
            text: query_with_caret,
            origin: [input[0] + 10.0, input[1] + 9.0],
            width: (input[2] - 20.0).max(10.0),
            font_size: 14.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        for (visible, rect) in lay.row_rects.iter().enumerate() {
            let row = self.search.scroll_top + visible;
            let Some(entry) = self.search.rows.get(row) else {
                break;
            };
            let selected = self.search.selected == Some(row);
            let row_rect = rect_xywh(*rect);
            // Hover-подсветка результата (не выбранного — выделенный несёт
            // акцент): практика списков результатов (VS Code)
            let row_hover = point_in_rect(row_rect, self.cursor);
            instances.push(CardInstance {
                pos: [row_rect[0], row_rect[1]],
                size: [row_rect[2], row_rect[3]],
                fill: if selected {
                    [0.18, 0.29, 0.48, 0.95]
                } else if row_hover {
                    [0.24, 0.30, 0.42, 0.6]
                } else {
                    palette.search_row_fill
                },
                border: [0.0; 4],
                params: [4.0, 0.0, 0.0, 1.0],
            });
            texts.push(OwnedScreenText {
                text: entry.title.clone(),
                origin: [row_rect[0] + 10.0, row_rect[1] + 4.0],
                width: (row_rect[2] - 20.0).max(10.0),
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: entry.subtitle.clone(),
                origin: [row_rect[0] + 10.0, row_rect[1] + 18.0],
                width: (row_rect[2] - 20.0).max(10.0),
                font_size: 11.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
        (instances, texts)
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
        let mut node = canvas_core::templates::instantiate(
            manifest,
            &BTreeMap::new(),
            id.clone(),
            world[0],
            world[1],
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

    /// Empty-state пустого канваса виден: 0 нод и нет конкурирующих
    /// модальных поверхностей (US-1 AC-1.1).
    fn empty_state_visible(&self) -> bool {
        self.scene.canvas.nodes.is_empty()
            && !self.scheme_gallery.open
            && self.onboarding.is_none()
            && !self.settings_open
            && self.main_stage.is_none()
            && self.menu.is_none()
            && !self.empty_state_dismissed
    }

    /// Открыть схему из галереи: чистый инстансер → один undo-шаг →
    /// вставка → recompute_flow → zoom-to-fit (US-3, G3/G7).
    fn apply_scheme(&mut self, manifest: &canvas_core::schemes::SchemeManifest) {
        let center = self.viewport_center_world();
        let instance = match canvas_scene::scheme_apply::instantiate_scheme(
            manifest,
            &self.scene.canvas,
            center,
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

    /// Отрисовка галереи (screen-space): панель, шапка, фильтр, чипы,
    /// строки схем, футер-подсказка. Цвета — слоты ThemeColors.
    fn scheme_gallery_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let registry = canvas_core::schemes::SchemeRegistry::embedded();
        let list = scheme_gallery_ui::rows(registry, &self.scheme_gallery);
        let lay = scheme_gallery_ui::layout(viewport, &list, &self.scheme_gallery);
        // Подложка панели
        instances.push(CardInstance {
            pos: [lay.panel_rect[0], lay.panel_rect[1]],
            size: [lay.panel_rect[2], lay.panel_rect[3]],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [10.0, 0.0, 0.0, 1.0],
        });
        // Шапка + счётчик
        texts.push(OwnedScreenText {
            text: self.tr(keys::GALLERY_TITLE).to_owned(),
            origin: [lay.header_rect[0], lay.header_rect[1] + 8.0],
            width: lay.header_rect[2] - 40.0,
            font_size: 14.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        texts.push(OwnedScreenText {
            text: format!("{}", list.len()),
            origin: [
                lay.header_rect[0] + lay.header_rect[2] - 36.0,
                lay.header_rect[1] + 10.0,
            ],
            width: 30.0,
            font_size: 11.0,
            color: palette.body,
            align: TextAlign::Center,
        });
        // Кнопка закрытия «×»
        instances.push(CardInstance {
            pos: [lay.close_rect[0], lay.close_rect[1]],
            size: [lay.close_rect[2], lay.close_rect[3]],
            fill: palette.palette_chip_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: "×".to_owned(),
            origin: [lay.close_rect[0], lay.close_rect[1] + 3.0],
            width: lay.close_rect[2],
            font_size: 13.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Поле фильтра
        instances.push(CardInstance {
            pos: [lay.input_rect[0], lay.input_rect[1]],
            size: [lay.input_rect[2], lay.input_rect[3]],
            fill: palette.search_input_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: if self.scheme_gallery.filter.is_empty() {
                self.tr(keys::GALLERY_SEARCH).to_owned()
            } else {
                format!("{}|", self.scheme_gallery.filter)
            },
            origin: [lay.input_rect[0] + 8.0, lay.input_rect[1] + 8.0],
            width: lay.input_rect[2] - 16.0,
            font_size: 12.0,
            color: palette.body,
            align: TextAlign::Left,
        });
        // Чипы категорий («Все» + уникальные категории реестра)
        let ru = self.settings.language == canvas_core::Language::Ru;
        for (rect, category) in &lay.chip_rects {
            let active = self.scheme_gallery.category == *category;
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if active {
                    palette.palette_chip_fill
                } else {
                    palette.menu_fill
                },
                border: palette.palette_border,
                params: [12.0, 0.0, 0.0, 1.0],
            });
            let label = match category {
                None => self.tr(keys::GALLERY_ALL).to_owned(),
                Some(key) => {
                    let manifest = registry.list().iter().find(|s| &s.category == key);
                    match manifest {
                        Some(m) => {
                            if ru {
                                m.category_ru.clone()
                            } else {
                                m.category_en.clone()
                            }
                        }
                        None => key.clone(),
                    }
                }
            };
            texts.push(OwnedScreenText {
                text: label,
                origin: [rect[0] + 8.0, rect[1] + 6.0],
                width: rect[2] - 16.0,
                font_size: 11.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        // Строки схем (окно видимости)
        let hovered = scheme_gallery_ui::row_at(&lay, self.cursor);
        for (rect, index) in lay.row_rects.iter().zip(lay.visible_rows.iter()) {
            let Some(scheme) = list.get(*index) else {
                continue;
            };
            let is_selected = *index == self.scheme_gallery.selected;
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if is_selected || hovered == Some(*index) {
                    hover_fill(palette.menu_fill)
                } else {
                    palette.menu_fill
                },
                border: palette.palette_border,
                params: [8.0, 0.0, 0.0, 1.0],
            });
            texts.push(OwnedScreenText {
                text: scheme.display_name(ru).to_owned(),
                origin: [rect[0] + 10.0, rect[1] + 8.0],
                width: rect[2] - 20.0,
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: if ru {
                    scheme.description_ru.clone()
                } else {
                    scheme.description_en.clone()
                },
                origin: [rect[0] + 10.0, rect[1] + 28.0],
                width: rect[2] - 20.0,
                font_size: 11.0,
                color: palette.body,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: i18n::trf(
                    self.settings.language,
                    keys::GALLERY_META,
                    &[
                        ("nodes", scheme.content.nodes.len().to_string().as_str()),
                        ("edges", scheme.content.edges.len().to_string().as_str()),
                    ],
                ),
                origin: [rect[0] + 10.0, rect[1] + 44.0],
                width: rect[2] - 20.0,
                font_size: 10.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
        // Футер-подсказка
        texts.push(OwnedScreenText {
            text: self.tr(keys::GALLERY_FOOTER).to_owned(),
            origin: [lay.footer_rect[0], lay.footer_rect[1] + 5.0],
            width: lay.footer_rect[2],
            font_size: 10.0,
            color: palette.body,
            align: TextAlign::Left,
        });
        (instances, texts)
    }

    /// Отрисовка empty-state пустого канваса (US-1): карточка с одной
    /// главной кнопкой и альтернативой «Пустой холст».
    fn empty_state_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let card = scheme_gallery_ui::empty_card_rect(viewport);
        instances.push(CardInstance {
            pos: [card[0], card[1]],
            size: [card[2], card[3]],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [10.0, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: self.tr(keys::GALLERY_EMPTY_TITLE).to_owned(),
            origin: [card[0] + 20.0, card[1] + 18.0],
            width: card[2] - 40.0,
            font_size: 16.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        texts.push(OwnedScreenText {
            text: self.tr(keys::GALLERY_EMPTY_BODY).to_owned(),
            origin: [card[0] + 20.0, card[1] + 46.0],
            width: card[2] - 40.0,
            font_size: 12.0,
            color: palette.body,
            align: TextAlign::Left,
        });
        let (open_btn, dismiss_btn) = scheme_gallery_ui::empty_buttons(card);
        for (rect, label_key, accent) in [
            (open_btn, keys::GALLERY_EMPTY_OPEN, true),
            (dismiss_btn, keys::GALLERY_EMPTY_DISMISS, false),
        ] {
            let hovered = scheme_gallery_ui::point_in_rect(rect, self.cursor);
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if accent {
                    if hovered {
                        hover_fill([0.16, 0.32, 0.60, 1.0])
                    } else {
                        [0.16, 0.32, 0.60, 1.0]
                    }
                } else if hovered {
                    hover_fill(palette.menu_fill)
                } else {
                    palette.menu_fill
                },
                border: palette.palette_border,
                params: [6.0, 0.0, 0.0, 1.0],
            });
            texts.push(OwnedScreenText {
                text: self.tr(label_key).to_owned(),
                origin: [rect[0], rect[1] + 9.0],
                width: rect[2],
                font_size: 12.0,
                color: palette.title,
                align: TextAlign::Center,
            });
        }
        (instances, texts)
    }

    /// Popup открывается только на Numi-строках каретки (вердикт
    /// `expr::line_kind`, вне код-фенсов) при непустом списке вариантов;
    /// якорь — низ каретки в логических px окна.
    fn update_hints(&mut self) {
        let Some(session) = self.editing.as_ref() else {
            self.hints.reset();
            return;
        };
        // Подсказки — только в тексте ноды (лейблы связей не Numi-редактор)
        if session.node_index().is_none() {
            self.hints.reset();
            return;
        }
        let (line_i, line_text, caret) = session.caret_line();
        let prefix = &line_text[..caret.min(line_text.len())];
        // Код-фенсы выше строки каретки (``` toggling, как eval_lines)
        let full_text = session.text();
        let in_fence = full_text
            .split('\n')
            .take(line_i)
            .fold(false, |fence, line| {
                fence ^ line.trim_start().starts_with("```")
            });
        if in_fence
            || !matches!(
                expr::line_kind(prefix),
                expr::NumiLineKind::Assignment { .. } | expr::NumiLineKind::Expression
            )
        {
            self.hints.reset();
            return;
        }
        // Контекст ноды: переменные выше, value-входы, параметры шаблона
        let vars: Vec<String> = full_text
            .split('\n')
            .take(line_i)
            .filter_map(|line| match expr::line_kind(line) {
                expr::NumiLineKind::Assignment { name } => Some(name),
                _ => None,
            })
            .collect();
        let ctx = hints_ui::HintContext {
            vars,
            inbound: self
                .scene
                .canvas
                .edges
                .iter()
                .filter(|edge| {
                    edge.to_node
                        == self
                            .scene
                            .canvas
                            .nodes
                            .get(session.node_index().unwrap_or(usize::MAX))
                            .map(|node| node.id.clone())
                            .unwrap_or_default()
                        && edge.flow_kind() == FlowKind::Value
                })
                .count(),
            params: self
                .scene
                .canvas
                .nodes
                .get(session.node_index().unwrap_or(usize::MAX))
                .and_then(|node| node.template())
                .map(|template| template.params.keys().cloned().collect())
                .unwrap_or_default(),
        };
        let items = hints_ui::hint_items(prefix, &ctx, self.settings.language);
        let token = hints_ui::token_before_caret(prefix, prefix.len()).0;
        self.hints.sync(token, items);
        // Якорь — низ каретки (screen logical px): world-область тела ноды
        // + позиция каретки в буфере (физ. px)
        if self.hints.open {
            let caret_rect = if let (Some(session), Some(renderer)) =
                (self.editing.as_mut(), self.renderer.as_mut())
            {
                session.caret_rect(renderer.font_system_mut())
            } else {
                None
            };
            if let (Some(session), Some(rect)) = (self.editing.as_ref(), caret_rect) {
                if let Some((origin, _, _)) =
                    session_area(&self.scene.canvas, session, self.settings.edges_avoid_nodes)
                {
                    let screen = self.camera.world_to_screen(origin, self.viewport_logical());
                    let scale = self.scale_factor();
                    self.hints.anchor = [
                        screen[0] + rect[0] / scale,
                        screen[1] + (rect[1] + rect[3]) / scale,
                    ];
                }
            }
        }
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

    /// FR-021: оверлей popup подсказок — подложка + строки
    /// (имя + серая деталь), выделение акцентом. Паттерн wheel_overlay.
    fn hints_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        if !self.hints.open || self.hints.items.is_empty() {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let viewport = self.viewport_logical();
        let [px, py, pw, ph] =
            hints_ui::popup_layout(self.hints.anchor, viewport, self.hints.items.len());
        if pw <= 0.0 {
            return (instances, texts);
        }
        instances.push(CardInstance {
            pos: [px, py],
            size: [pw, ph],
            fill: palette.menu_fill,
            border: [0.22, 0.24, 0.30, 0.95],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        for (i, item) in self.hints.items.iter().enumerate() {
            let row_y = py + hints_ui::HINT_MARGIN + i as f32 * hints_ui::HINT_ROW_H;
            if i == self.hints.selected {
                instances.push(CardInstance {
                    pos: [px + 4.0, row_y],
                    size: [pw - 8.0, hints_ui::HINT_ROW_H],
                    fill: [0.18, 0.29, 0.48, 0.95],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
            }
            texts.push(OwnedScreenText {
                text: item.label.clone(),
                origin: [px + 10.0, row_y + 4.0],
                width: 118.0,
                font_size: 12.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: item.detail.clone(),
                origin: [px + 134.0, row_y + 6.0],
                width: (pw - 142.0).max(20.0),
                font_size: 10.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
        (instances, texts)
    }

    /// Клавиатура палитры шаблонов в фокусе (Ctrl+P/клик по поиску):
    /// ввод фильтра, стрелки/Enter/Esc. Вызывается из on_key, когда панель
    /// развёрнута и сфокусирована. true — клавиша потреблена панелью.
    fn on_template_panel_key(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return true; // отпускания глотаются — канвасу не достаются
        }
        // Ctrl+P не глотаем: on_key ниже фокусирует поиск развёрнутого
        // дока (FR-025) или разворачивает свёрнутый
        if self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("p") || c.eq_ignore_ascii_case("з"))
        {
            return false;
        }
        if event.logical_key == Key::Named(NamedKey::Escape) && !event.repeat {
            // FR-025: Esc сворачивает постоянный док (не закрывает модал —
            // док не модален; повторный Ctrl+P или ручка слева развернут)
            self.template_panel.close();
            self.persist_palette_dock();
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::Enter) && !event.repeat {
            let rows = template_panel_rows(&self.templates, &self.template_panel);
            // FR-024: выделение — ординал среди строк-шаблонов (секции —
            // заголовки, не цели)
            if let Some(row_idx) = template_row_of_ordinal(&rows, self.template_panel.selected) {
                if let template_ui::PanelRow::Template(index) = rows[row_idx] {
                    let manifest = self.templates.list()[index].clone();
                    let center = self.viewport_center_world();
                    // FR-025: док остаётся развёрнут — только фокус снят
                    self.template_panel.unfocus();
                    self.instantiate_template_at(&manifest, center);
                    self.request_redraw();
                }
            }
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowDown) && !event.repeat {
            let rows = template_panel_rows(&self.templates, &self.template_panel);
            self.template_panel.move_selection(1, &rows);
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowUp) && !event.repeat {
            let rows = template_panel_rows(&self.templates, &self.template_panel);
            self.template_panel.move_selection(-1, &rows);
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::Backspace) && !event.repeat {
            self.template_panel.backspace();
            self.template_panel.selected = 0;
            self.template_panel.scroll_top = 0;
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowLeft) && !event.repeat {
            self.template_panel.move_left();
            self.request_redraw();
            return true;
        }
        if event.logical_key == Key::Named(NamedKey::ArrowRight) && !event.repeat {
            self.template_panel.move_right();
            self.request_redraw();
            return true;
        }
        // Печатаемый символ (включая кириллицу — logical_key уже раскладка)
        if let Key::Character(text) = &event.logical_key {
            if !event.repeat && !self.modifiers.control_key() {
                self.template_panel.insert_str(text.as_str());
                self.template_panel.selected = 0;
                self.template_panel.scroll_top = 0;
                self.request_redraw();
                return true;
            }
        }
        true
    }

    /// Оверлей палитры шаблонов (FR-018, Ctrl+P; FR-024 — стиль Miro
    /// Template picker; FR-025 — постоянная палитра: ПРИМАРНО свёрнутая
    /// вертикальная полоса категорий по центру слева, hover раскрывает
    /// flyout справа; развёрнутый док по Ctrl+P; ghost-превью drag):
    /// свёрнутый режим — [`template_strip_overlay`]; развёрнутый — левый док
    /// во всю высоту (чистая геометрия — `template_ui::panel_layout`), шапка
    /// «Шаблоны», поиск с placeholder, чипы категорий, секции с заголовками,
    /// строки-карточки, hover/выбранное состояние, футер-подсказка.
    fn template_panel_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let icon_tint = color_to_rgba(palette.icon);
        // FR-025 (ревизия): свёрнутый режим — полоса категорий + flyout
        if !self.template_panel.open {
            let (mut strip_instances, mut strip_texts) =
                self.template_strip_overlay(viewport, &palette);
            instances.append(&mut strip_instances);
            texts.append(&mut strip_texts);
        } else {
            let rows = template_panel_rows(&self.templates, &self.template_panel);
            let lay = template_panel_layout(
                viewport[0],
                viewport[1],
                &self.templates,
                &self.template_panel,
                &rows,
            );
            let panel = rect_xywh(lay.panel_rect);
            // Подложка дока: плотная, с рамкой (отделяет панель от канваса).
            // CR-011: рамка палитурная (была захардкожена тёмной — ломала
            // светлую тему).
            instances.push(CardInstance {
                pos: [panel[0], panel[1]],
                size: [panel[2], panel[3]],
                fill: palette.menu_fill,
                border: palette.palette_border,
                params: [8.0, 0.0, 0.0, 1.0],
            });
            // Шапка: название + счётчик шаблонов
            let total = template_ui::template_row_count(&rows);
            texts.push(OwnedScreenText {
                text: self.tr(keys::TEMPLATES_TITLE).to_owned(),
                origin: [lay.header_rect[0], lay.header_rect[1] + 6.0],
                width: lay.header_rect[2] * 0.5,
                font_size: 14.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: format!("{total}"),
                origin: [
                    lay.header_rect[0] + lay.header_rect[2] * 0.5,
                    lay.header_rect[1] + 8.0,
                ],
                width: lay.header_rect[2] * 0.5 - 4.0,
                font_size: 11.0,
                color: palette.body,
                align: TextAlign::Center,
            });
            // FR-025: кнопка сворачивания дока («‹» у правого края шапки)
            let collapse = rect_xywh(lay.collapse_rect);
            instances.push(CardInstance {
                pos: [collapse[0], collapse[1]],
                size: [collapse[2], collapse[3]],
                fill: palette.palette_chip_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
            });
            texts.push(OwnedScreenText {
                text: "‹".to_owned(),
                origin: [collapse[0], collapse[1] + 3.0],
                width: collapse[2],
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Center,
            });
            // Поле фильтра: placeholder при пустом вводе, иначе текст с кареткой
            let input = rect_xywh(lay.input_rect);
            instances.push(CardInstance {
                pos: [input[0], input[1]],
                size: [input[2], input[3]],
                fill: palette.search_input_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
            });
            texts.push(OwnedScreenText {
                text: if self.template_panel.filter.is_empty() {
                    self.tr(keys::TEMPLATES_SEARCH).to_owned()
                } else {
                    format!("{}|", self.template_panel.filter)
                },
                origin: [input[0] + 10.0, input[1] + 8.0],
                width: (input[2] - 20.0).max(10.0),
                font_size: 13.0,
                color: if self.template_panel.filter.is_empty() {
                    palette.body
                } else {
                    palette.title
                },
                align: TextAlign::Left,
            });
            // Чипы категорий (CR-011: заливки палитурные, не хардкод)
            for (rect, name, active) in &lay.category_rects {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1]],
                    size: [rect[2], rect[3]],
                    fill: if *active {
                        palette.palette_selected_fill
                    } else {
                        palette.palette_chip_fill
                    },
                    border: [0.0; 4],
                    params: [11.0, 0.0, 0.0, 1.0],
                });
                texts.push(OwnedScreenText {
                    text: name.clone(),
                    origin: [rect[0] + 10.0, rect[1] + 6.0],
                    width: rect[2] - 12.0,
                    font_size: 12.0,
                    color: palette.title,
                    align: TextAlign::Left,
                });
            }
            // Строки: секции-заголовки и карточки шаблонов (Miro-стиль)
            for (row_i, (rect, row)) in lay.row_rects.iter().zip(lay.rows.iter()).enumerate() {
                match row {
                    PanelRow::Section(name) => {
                        texts.push(OwnedScreenText {
                            text: name.clone(),
                            origin: [rect[0] + 2.0, rect[1] + 5.0],
                            width: rect[2] - 4.0,
                            font_size: 11.0,
                            color: palette.body,
                            align: TextAlign::Left,
                        });
                    }
                    PanelRow::Template(index) => {
                        let Some(manifest) = self.templates.list().get(*index) else {
                            continue;
                        };
                        // Ординал строки среди шаблонов (секции не считаются)
                        let ordinal = lay.rows[..row_i]
                            .iter()
                            .filter(|other| matches!(other, PanelRow::Template(_)))
                            .count();
                        let selected = self.template_panel.selected == ordinal;
                        let row_rect = rect_xywh(*rect);
                        let row_hover = point_in_rect(row_rect, self.cursor);
                        // Подложка-карточка строки (Miro: карточка с фоном;
                        // CR-011: заливки палитурные, не хардкод)
                        let fill = if selected {
                            palette.palette_selected_fill
                        } else if row_hover {
                            palette.palette_hover_fill
                        } else {
                            palette.palette_row_fill
                        };
                        let border = if selected || row_hover {
                            palette.palette_border
                        } else {
                            [0.0; 4]
                        };
                        template_card_row(
                            manifest,
                            row_rect,
                            fill,
                            border,
                            &palette,
                            icon_tint,
                            &mut instances,
                            &mut texts,
                        );
                    }
                }
            }
            // Футер-подсказка (CR-011: позиция из footer_rect чистой геометрии —
            // строки списка в него не заходят; FR-025: Esc сворачивает док)
            let footer = rect_xywh(lay.footer_rect);
            texts.push(OwnedScreenText {
                text: self.tr(keys::TEMPLATES_FOOTER).to_owned(),
                origin: [footer[0], footer[1] + 7.0],
                width: footer[2],
                font_size: 10.0,
                color: palette.body,
                align: TextAlign::Left,
            });
        } // else: развёрнутый док
          // FR-025: ghost-превью drag карточки шаблона — призрак дропа
          // (Т9) в world-точке курсора, отрисованный screen-space поверх
        if let Some(drag) = &self.template_drag {
            if drag.active {
                if let Some(manifest) = self.templates.list().get(drag.index) {
                    if let Ok(mut preview) = canvas_core::templates::instantiate(
                        manifest,
                        &BTreeMap::new(),
                        "preview".to_owned(),
                        0.0,
                        0.0,
                    ) {
                        fit_template_node_height(&mut preview);
                        let world = self.cursor_world();
                        let top_left = [
                            world[0] - preview.width / 2.0,
                            world[1] - preview.height / 2.0,
                        ];
                        let ghost = drop_ghost(top_left, [preview.width, preview.height]);
                        let zoom = self.camera.zoom();
                        instances.push(CardInstance {
                            pos: self.camera.world_to_screen(top_left, viewport),
                            size: [ghost.size[0] * zoom, ghost.size[1] * zoom],
                            fill: ghost.fill,
                            border: ghost.border,
                            params: ghost.params,
                        });
                    }
                }
            }
        }
        (instances, texts)
    }

    /// Оверлей свёрнутой палитры (ревизия FR-025, 2026-09-16): вертикальная
    /// полоса категорий по центру левого края (подложка + строки с именем и
    /// счётчиком + шеврон «развернуть док» внизу) и flyout раскрытой
    /// категории справа — строки шаблонов тем же карточным рендером, что и
    /// развёрнутый док, плюс индикаторы прокрутки при переполнении.
    fn template_strip_overlay(
        &self,
        viewport: Vec2,
        palette: &ThemeColors,
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let categories = self.template_category_names();
        let strip = template_ui::dock_strip_layout(&categories, viewport[1]);
        let icon_tint = color_to_rgba(palette.icon);
        let open_category = self.template_hover.as_ref().and_then(|h| h.open);
        // Подложка полосы
        instances.push(CardInstance {
            pos: [strip.rect[0], strip.rect[1]],
            size: [strip.rect[2], strip.rect[3]],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [8.0, 0.0, 0.0, 1.0],
        });
        // Строки категорий: hover-подсветка под курсором и у раскрытой
        for (i, (rect, name)) in strip.rows.iter().enumerate() {
            let row_hover = point_in_rect(*rect, self.cursor) || open_category == Some(i);
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if row_hover {
                    palette.palette_hover_fill
                } else {
                    palette.palette_chip_fill
                },
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
            });
            let count = self.templates.by_category(name).len();
            texts.push(OwnedScreenText {
                text: format!("{name} · {count}"),
                origin: [rect[0] + 8.0, rect[1] + 6.0],
                width: rect[2] - 12.0,
                font_size: 12.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        // Шеврон «развернуть док» — строка внизу полосы
        instances.push(CardInstance {
            pos: [strip.chevron_rect[0], strip.chevron_rect[1]],
            size: [strip.chevron_rect[2], strip.chevron_rect[3]],
            fill: palette.palette_chip_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: "»".to_owned(),
            origin: [strip.chevron_rect[0], strip.chevron_rect[1] + 5.0],
            width: strip.chevron_rect[2],
            font_size: 13.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Flyout раскрытой категории: строки шаблонов (только видимое окно)
        if let (Some(cat), Some(fly)) = (
            open_category,
            self.template_flyout_geometry(viewport, &strip),
        ) {
            let Some((_, name)) = strip.rows.get(cat) else {
                return (instances, texts);
            };
            let items = self.templates.by_category(name);
            instances.push(CardInstance {
                pos: [fly.rect[0], fly.rect[1]],
                size: [fly.rect[2], fly.rect[3]],
                fill: palette.menu_fill,
                border: palette.palette_border,
                params: [8.0, 0.0, 0.0, 1.0],
            });
            for (v, rect) in fly.row_rects.iter().enumerate() {
                let Some(manifest) = items.get(fly.scroll_top + v) else {
                    break;
                };
                let row_hover = point_in_rect(*rect, self.cursor);
                template_card_row(
                    manifest,
                    *rect,
                    if row_hover {
                        palette.palette_hover_fill
                    } else {
                        palette.palette_row_fill
                    },
                    if row_hover {
                        palette.palette_border
                    } else {
                        [0.0; 4]
                    },
                    palette,
                    icon_tint,
                    &mut instances,
                    &mut texts,
                );
            }
            // Индикаторы прокрутки: стрелки ▲/▼ у правого края flyout
            if fly.max_scroll > 0 {
                if fly.scroll_top > 0 {
                    texts.push(OwnedScreenText {
                        text: "▲".to_owned(),
                        origin: [fly.rect[0] + fly.rect[2] - 20.0, fly.rect[1] + 2.0],
                        width: 16.0,
                        font_size: 10.0,
                        color: palette.body,
                        align: TextAlign::Center,
                    });
                }
                if fly.scroll_top < fly.max_scroll {
                    texts.push(OwnedScreenText {
                        text: "▼".to_owned(),
                        origin: [
                            fly.rect[0] + fly.rect[2] - 20.0,
                            fly.rect[1] + fly.rect[3] - 15.0,
                        ],
                        width: 16.0,
                        font_size: 10.0,
                        color: palette.body,
                        align: TextAlign::Center,
                    });
                }
            }
        }
        (instances, texts)
    }

    /// Оверлей радиального wheel-меню шаблонов (FR-018, Shift+клик):
    /// плашки-мини-карточки (шаблоны + категории) из чистой геометрии
    /// `template_ui::wheel_geometry` — раскладка отталкивается от размера
    /// плашек, зазор гарантирован (правка владельца 2026-09-16). Пайплайн
    /// квадов без поворотов; hover — по тем же плашкам (WYSIWYG).
    /// FR-022 (бест-практики радиальных меню): затемнение фона под
    /// модальным пикером (паттерн Miro Template picker), круглая кнопка
    ///-хаб «назад/закрыть» (Kurtenbach/Buxton — центр отменяет уровень),
    /// крошки глубины в хабе (выбранная категория).
    /// FR-018/FR-022: wheel-меню шаблонов (Shift+клик) — donut-сектора
    /// (`SectorsPipeline`, SDF annular-wedge) + квады иконок/хаба + подписи.
    /// Сектора — screen-space; рендерер конвертирует их в world тем же
    /// способом, что и screen_instances (центр через screen_to_world,
    /// радиусы / zoom — `screen_sector_to_world` в renderer.rs). Возвращает
    /// (сектора, квады, тексты) — квады рисуются ПОВЕРХ секторов.
    fn wheel_overlay(&self) -> (Vec<SectorInstance>, Vec<CardInstance>, Vec<OwnedScreenText>) {
        // Цвета wheel-меню. FR-046: открытый вопрос FR-022 закрыт — значения
        // из design-токенов (wheel.*; дифференциация светлой темы — v2).
        const FILL_DIM: [f32; 4] = canvas_core::tokens::WHEEL_DIM;
        const FILL_CATEGORY: [f32; 4] = canvas_core::tokens::WHEEL_CATEGORY;
        const FILL_TEMPLATE: [f32; 4] = canvas_core::tokens::WHEEL_TEMPLATE;
        const FILL_HOVER: [f32; 4] = canvas_core::tokens::WHEEL_HOVER;
        const FILL_HUB_ACTIVE: [f32; 4] = canvas_core::tokens::WHEEL_HUB_ACTIVE;
        const BORDER: [f32; 4] = canvas_core::tokens::WHEEL_BORDER;

        let mut sectors = Vec::new();
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.wheel_menu else {
            return (sectors, instances, texts);
        };
        let palette = self.effective_palette();
        let icon_tint = color_to_rgba(palette.icon);
        let [vw, vh] = self.viewport_logical();
        let categories = self.templates.categories();
        let templates: Vec<_> = menu
            .category
            .as_deref()
            .map(|c| self.templates.by_category(c))
            .unwrap_or_default();
        let geo =
            template_ui::wheel_geometry(menu.screen, vw, vh, categories.len(), templates.len());
        let hovered = geo.hit(self.cursor);
        // Затемнение фона — диском-полным-кругом: первый инстанс секторного
        // прохода, под ним ничего рисовать не нужно; радиус — до дальнего
        // угла viewport (кламп центра гарантирует покрытие окна)
        let dim_r = geo.center[0]
            .max(vw - geo.center[0])
            .hypot(geo.center[1].max(vh - geo.center[1]))
            + 4.0;
        sectors.push(SectorInstance {
            center: geo.center,
            r0: 0.0,
            r1: dim_r,
            a0: 0.0,
            a1: std::f32::consts::TAU,
            fill: FILL_DIM,
        });
        // Donut-сектора меню: что нарисовано — по тому и клик (geo.hit —
        // тот же полярный тест, что и SDF-шейдер)
        for sector in &geo.sectors {
            let active = hovered.as_ref() == Some(&sector.hit);
            let fill = if active {
                FILL_HOVER
            } else {
                match sector.hit {
                    WheelHit::Category(_) => FILL_CATEGORY,
                    WheelHit::Template(_) => FILL_TEMPLATE,
                }
            };
            sectors.push(SectorInstance {
                center: geo.center,
                r0: sector.r0,
                r1: sector.r1,
                a0: sector.a0,
                a1: sector.a1,
                fill,
            });
            // Иконка + подпись внутри сектора (как circular-menu: вертикально,
            // иконка выше текста; подпись всегда рисуем — минимальная дуга
            // сектора 60 px вмещает две строки 11px по ~10 символов)
            let [px, py] =
                template_ui::sector_point(geo.center, sector.mid_angle(), sector.mid_radius());
            match sector.hit {
                WheelHit::Category(i) => {
                    texts.push(OwnedScreenText {
                        text: categories[i].to_owned(),
                        origin: [px - 40.0, py - 7.0],
                        width: 80.0,
                        font_size: 12.0,
                        color: palette.title,
                        align: TextAlign::Center,
                    });
                }
                WheelHit::Template(i) => {
                    let Some(manifest) = templates.get(i) else {
                        continue;
                    };
                    instances.extend(template_icon_quads(
                        template_ui::icon_key(manifest),
                        [px - 8.0, py - 14.0, 16.0, 16.0],
                        icon_tint,
                    ));
                    let (line1, line2) =
                        split_two_lines(manifest.display_name(), template_ui::WHEEL_TPL_TEXT_CHARS);
                    let push_line = |text: String, dy: f32| OwnedScreenText {
                        text,
                        origin: [px - 32.0, py + dy],
                        width: 64.0,
                        font_size: 11.0,
                        color: palette.title,
                        align: TextAlign::Center,
                    };
                    match line2 {
                        None => texts.push(push_line(line1, 4.0)),
                        Some(line2) => {
                            texts.push(push_line(line1, 2.0));
                            texts.push(push_line(line2, 15.0));
                        }
                    }
                }
            }
        }
        // Хаб: круглая кнопка «назад/закрыть» (FR-022). Без категории —
        // подсказка «закрыть»; с категорией — крошки глубины «← имя»
        let [hx, hy, hw, hh] = geo.hub;
        instances.push(CardInstance {
            pos: [hx, hy],
            size: [hw, hh],
            fill: if menu.category.is_some() {
                FILL_HUB_ACTIVE
            } else {
                FILL_CATEGORY
            },
            border: BORDER,
            params: [hw / 2.0, 0.0, 0.0, 1.0], // круг — радиус = половина стороны
        });
        texts.push(OwnedScreenText {
            text: if let Some(category) = &menu.category {
                format!("← {category}")
            } else {
                "закрыть".to_owned()
            },
            origin: [geo.center[0] - 44.0, geo.center[1] - 6.0],
            width: 88.0,
            font_size: 10.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        (sectors, instances, texts)
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
        self.request_redraw();
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

    /// Оверлей контекстного меню пустого канваса (T7): фон, подписи.
    /// Screen-space — логические px, константный читаемый размер при любом
    /// зуме (уточнение владельца). Меню ноды/связи заменены палитрой.
    /// FR-038 (T-038.5): batch-пункты выравнивания — хвост меню, видны
    /// только при N≥3 выделенных нодах (единый список с хит-тестом).
    /// FR-050 Н2 (этап C): оверлей меню выбора — панель + заголовок +
    /// пункты (имена параметров / строки-источники со значениями) +
    /// hover-подсветка. Screen-space (как контекстное меню T7):
    /// константный размер при любом зуме; клик по пункту — действие,
    /// мимо/Esc — отмена.
    fn choice_menu_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.choice_menu else {
            return (instances, texts);
        };
        let palette = self.effective_palette();
        // Панель: высота пунктов + строка заголовка (геометрия меню T7)
        let [x, y, w, h] = menu_rect_for(menu.origin, menu.items.len());
        instances.push(CardInstance {
            pos: [x, y],
            size: [w, h + CHOICE_MENU_TITLE_H],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 0.0],
        });
        // Заголовок — приглушённым тоном (не пункт, не интерактивен)
        texts.push(OwnedScreenText {
            text: self.tr(menu.title_key).to_owned(),
            origin: [x + MENU_PADDING + 4.0, y + MENU_PADDING + 5.0],
            width: w - MENU_PADDING * 2.0 - 8.0,
            font_size: 12.0,
            color: palette.body,
            align: TextAlign::Left,
        });
        // Пункты — геометрия меню T7, сдвинутая на высоту заголовка;
        // хит-тест — той же геометрией (choice_menu_item_at), клик по
        // заголовку = «мимо пункта» = отмена
        for (i, item) in menu.items.iter().enumerate() {
            let rect = menu_item_rect(menu.origin, i);
            let rect = [rect[0], rect[1] + CHOICE_MENU_TITLE_H, rect[2], rect[3]];
            if menu.hovered == Some(i) {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1]],
                    size: [rect[2], rect[3]],
                    fill: [0.24, 0.30, 0.42, 0.9],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
            }
            texts.push(OwnedScreenText {
                text: item.label.clone(),
                origin: [rect[0] + 8.0, rect[1] + 5.0],
                width: rect[2] - 12.0,
                font_size: 14.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        (instances, texts)
    }

    /// FR-050 Н2 (этап C): пункт меню выбора под курсором — геометрия
    /// отрисовки (сдвиг на заголовок); None — заголовок/мимо (отмена).
    fn choice_menu_item_at(&self, cursor: Vec2) -> Option<usize> {
        let menu = self.choice_menu.as_ref()?;
        let shifted = [menu.origin[0], menu.origin[1] + CHOICE_MENU_TITLE_H];
        crate::ui::menu_item_at_for(shifted, cursor, menu.items.len())
    }

    fn canvas_menu_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.menu else {
            return (instances, texts);
        };
        let items = canvas_menu_visible_items(self.align_menu_visible());
        let palette = self.effective_palette();
        let [x, y, w, h] = menu_rect_for(menu.origin, items.len());
        instances.push(CardInstance {
            pos: [x, y],
            size: [w, h],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 0.0],
        });
        // Hover-подсветка пункта (аффорданс — как строки палитры/поиска:
        // интерактивный элемент отвечает на курсор)
        let hovered_item = menu_item_at_for(menu.origin, self.cursor, items.len());
        for (i, item) in items.iter().enumerate() {
            let rect = menu_item_rect(menu.origin, i);
            if hovered_item == Some(i) {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1]],
                    size: [rect[2], rect[3]],
                    fill: [0.24, 0.30, 0.42, 0.9],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
            }
            texts.push(OwnedScreenText {
                text: canvas_menu_label(
                    *item,
                    self.settings.focus_mode,
                    self.hotkeys_open,
                    self.desktop_menu_checked(),
                    self.settings.bottleneck_overlay,
                    self.scene.whatif_active,
                    self.settings.language,
                ),
                origin: [rect[0] + MENU_LABEL_X, rect[1] + 5.0],
                width: rect[2] - MENU_LABEL_X,
                font_size: 14.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        // M5 (T20-F): колонка подменю «Виджеты ▸» — справа от меню
        if let Some(submenu) = &menu.submenu {
            let [sx, sy, sw, sh] = submenu_rect(submenu);
            instances.push(CardInstance {
                pos: [sx, sy],
                size: [sw, sh],
                fill: palette.menu_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 0.0],
            });
            if submenu.entries.is_empty() {
                texts.push(OwnedScreenText {
                    text: self.tr(keys::WIDGETS_EMPTY).to_owned(),
                    origin: [
                        submenu.origin[0] + MENU_PADDING + 4.0,
                        submenu.origin[1] + MENU_PADDING + 5.0,
                    ],
                    width: MENU_WIDTH - MENU_PADDING * 2.0 - 8.0,
                    font_size: 13.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            } else {
                let hovered_sub = submenu_item_at(submenu, self.cursor);
                for (i, entry) in submenu.entries.iter().enumerate() {
                    let rect = menu_item_rect(submenu.origin, i);
                    if hovered_sub == Some(i) {
                        instances.push(CardInstance {
                            pos: [rect[0], rect[1]],
                            size: [rect[2], rect[3]],
                            fill: [0.24, 0.30, 0.42, 0.9],
                            border: [0.0; 4],
                            params: [4.0, 0.0, 0.0, 1.0],
                        });
                    }
                    texts.push(OwnedScreenText {
                        text: entry.label.clone(),
                        origin: [rect[0] + MENU_LABEL_X, rect[1] + 5.0],
                        width: rect[2] - MENU_LABEL_X,
                        font_size: 13.0,
                        color: palette.title,
                        align: TextAlign::Left,
                    });
                }
            }
        }
        (instances, texts)
    }

    /// FR-027: оверлей меню помощи кнопки «?» — колонка у кнопки и
    /// раскрытое подменю разделов (паттерн контекстного меню T7).
    fn help_menu_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(menu) = &self.help_menu else {
            return (instances, texts);
        };
        let viewport = self.viewport_logical();
        let palette = self.effective_palette();
        let [x, y, w, h] = docs_ui::help_menu_rect(menu.origin);
        instances.push(CardInstance {
            pos: [x, y],
            size: [w, h],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 0.0],
        });
        let hovered = docs_ui::help_menu_item_at(menu.origin, self.cursor);
        for (i, item) in docs_ui::HELP_MENU_ITEMS.iter().enumerate() {
            let rect = docs_ui::help_menu_item_rect(menu.origin, i);
            if hovered == Some(*item) {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1]],
                    size: [rect[2], rect[3]],
                    fill: [0.24, 0.30, 0.42, 0.9],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
            }
            texts.push(OwnedScreenText {
                text: docs_ui::help_menu_item_label(*item, self.settings.language).to_owned(),
                origin: [rect[0] + 8.0, rect[1] + 6.0],
                width: rect[2] - 8.0,
                font_size: 13.0,
                color: palette.title,
                align: TextAlign::Left,
            });
        }
        if menu.docs_open {
            let sub = docs_ui::help_submenu_origin(menu.origin, viewport);
            let [sx, sy, sw, sh] = docs_ui::help_submenu_rect(sub);
            instances.push(CardInstance {
                pos: [sx, sy],
                size: [sw, sh],
                fill: palette.menu_fill,
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 0.0],
            });
            let hovered_sub = docs_ui::help_submenu_item_at(sub, self.cursor);
            for (i, page) in docs_ui::DOCS_PAGES.iter().enumerate() {
                let rect = docs_ui::help_submenu_item_rect(sub, i);
                if hovered_sub == Some(i) {
                    instances.push(CardInstance {
                        pos: [rect[0], rect[1]],
                        size: [rect[2], rect[3]],
                        fill: [0.24, 0.30, 0.42, 0.9],
                        border: [0.0; 4],
                        params: [4.0, 0.0, 0.0, 1.0],
                    });
                }
                texts.push(OwnedScreenText {
                    text: self.tr(page.label_key).to_owned(),
                    origin: [rect[0] + 8.0, rect[1] + 6.0],
                    width: rect[2] - 8.0,
                    font_size: 13.0,
                    color: palette.title,
                    align: TextAlign::Left,
                });
            }
        }
        (instances, texts)
    }

    /// FR-027: оверлей просмотрщика документации — правый док: шапка
    /// (раздел + ×), скроллируемый контент (кламп строк к видимой зоне),
    /// внутренние ссылки — акцент + подчёркивание, скроллбар-аффорданс.
    fn docs_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(viewer) = &self.docs else {
            return (instances, texts);
        };
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let panel = docs_ui::viewer_rect(viewport);
        let content = docs_ui::viewer_content_rect(panel);
        // Затемнение канваса вокруг панели (паттерн wheel FR-022)
        if panel[0] > 0.0 {
            instances.push(CardInstance {
                pos: [0.0, 0.0],
                size: [viewport[0], viewport[1]],
                fill: [0.02, 0.02, 0.04, 0.45],
                border: [0.0; 4],
                params: [0.0, 0.0, 0.0, 0.0],
            });
        }
        // Панель
        instances.push(CardInstance {
            pos: [panel[0], panel[1]],
            size: [panel[2], panel[3]],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 0.0],
        });
        // Шапка: раздел + × (hover-аффорданс)
        let close = docs_ui::viewer_close_rect(panel);
        let close_hovered = point_in_rect(close, self.cursor);
        instances.push(CardInstance {
            pos: [close[0], close[1]],
            size: [close[2], close[3]],
            fill: if close_hovered {
                hover_fill(palette.menu_fill)
            } else {
                palette.menu_fill
            },
            border: [0.0; 4],
            params: [6.0, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: "×".to_owned(),
            origin: [close[0], close[1] + (close[3] - 14.0 * 1.3) / 2.0],
            width: close[2],
            font_size: 14.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        let title = docs_ui::DOCS_PAGES
            .get(viewer.page)
            .map(|p| self.tr(p.label_key).to_owned())
            .unwrap_or_default();
        texts.push(OwnedScreenText {
            text: title,
            origin: [panel[0] + docs_ui::DOCS_PADDING, panel[1] + 11.0],
            width: close[0] - panel[0] - docs_ui::DOCS_PADDING * 2.0,
            font_size: 15.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        // Контент: строки раскладки со сдвигом −offset; строки вне видимой
        // зоны не рендерим (кламп, длинные страницы дешевле кадра)
        let view_top = content[1];
        let view_bottom = content[1] + content[3];
        for line in &viewer.layout.lines {
            let (_, line_h) = docs_ui::kind_metrics(line.kind);
            let y = content[1] + line.y - viewer.scroll.offset;
            if y + line_h < view_top || y > view_bottom {
                continue;
            }
            let (font, _) = docs_ui::kind_metrics(line.kind);
            let color = match line.kind {
                docs_ui::RowKind::Heading(_) => palette.title,
                docs_ui::RowKind::Quote | docs_ui::RowKind::Code => palette.icon,
                docs_ui::RowKind::TableCell { header: true } => palette.title,
                _ => palette.body,
            };
            let mut x = content[0] + line.x;
            for span in &line.spans {
                // Внутренние ссылки — акцент; внешние — обычный текст
                // (v1 не кликабельны, FR-027)
                let span_color = if span.href.as_deref().is_some_and(|href| {
                    matches!(docs_ui::link_target(href), docs_ui::LinkTarget::Page(_))
                }) {
                    palette.link
                } else {
                    color
                };
                texts.push(OwnedScreenText {
                    text: span.text.clone(),
                    origin: [x, y],
                    width: (content[0] + content[2] - x).max(10.0),
                    font_size: font,
                    color: span_color,
                    align: TextAlign::Left,
                });
                x += docs_ui::text_width(&span.text, font);
            }
        }
        // Квады раскладки (линии/подчёркивания шапок таблиц/бары цитат) —
        // видимые по y
        for quad in &viewer.layout.quads {
            let y = content[1] + quad.y - viewer.scroll.offset;
            if y + quad.height < view_top || y > view_bottom {
                continue;
            }
            instances.push(CardInstance {
                pos: [content[0] + quad.x, y],
                size: [quad.width, quad.height.max(1.0)],
                fill: match quad.kind {
                    docs_ui::QuadKind::Rule => [0.30, 0.33, 0.40, 0.8],
                    docs_ui::QuadKind::QuoteBar => color_to_rgba(palette.link),
                },
                border: [0.0; 4],
                params: [0.0, 0.0, 0.0, 0.0],
            });
        }
        // Подчёркивания внутренних ссылок (кликабельный аффорданс)
        for link in &viewer.layout.links {
            let y = content[1] + link.rect[1] + link.rect[3] - 2.0 - viewer.scroll.offset;
            if y < view_top || y > view_bottom {
                continue;
            }
            instances.push(CardInstance {
                pos: [content[0] + link.rect[0], y],
                size: [link.rect[2], 1.0],
                fill: color_to_rgba(palette.link),
                border: [0.0; 4],
                params: [0.0, 0.0, 0.0, 0.0],
            });
        }
        // Скроллбар-аффорданс справа (ползунок по пропорции offset/max)
        if viewer.scroll.max_offset > 0.0 {
            let track_h = content[3];
            let thumb_h = (track_h * track_h / viewer.layout.content_height).clamp(24.0, track_h);
            let free = (track_h - thumb_h).max(0.0);
            let thumb_y = content[1] + free * viewer.scroll.offset / viewer.scroll.max_offset;
            instances.push(CardInstance {
                pos: [
                    panel[0] + panel[2] - docs_ui::DOCS_SCROLLBAR_W - 2.0,
                    thumb_y,
                ],
                size: [docs_ui::DOCS_SCROLLBAR_W, thumb_h],
                fill: [0.35, 0.38, 0.46, 0.7],
                border: [0.0; 4],
                params: [3.0, 0.0, 0.0, 1.0],
            });
        }
        // Футер-подсказка
        texts.push(OwnedScreenText {
            text: self.tr(keys::DOCS_FOOTER).to_owned(),
            origin: [panel[0] + docs_ui::DOCS_PADDING, panel[1] + panel[3] - 18.0],
            width: panel[2] - docs_ui::DOCS_PADDING * 2.0,
            font_size: 11.0,
            color: palette.icon,
            align: TextAlign::Left,
        });
        (instances, texts)
    }

    /// FR-028: оверлей онбординга — затемнение канваса + карточка по центру
    /// (заголовок, тело, прогресс-точки, кнопки Назад/Далее|Готово/Пропустить).
    fn onboarding_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let Some(state) = &self.onboarding else {
            return (instances, texts);
        };
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        let Some(step) = onboarding_ui::ONBOARDING_STEPS.get(state.step) else {
            return (instances, texts);
        };
        // Затемнение (паттерн wheel FR-022): тур поверх неинтерактивного
        // канваса — фокус на карточке
        instances.push(CardInstance {
            pos: [0.0, 0.0],
            size: [viewport[0], viewport[1]],
            fill: [0.02, 0.02, 0.04, 0.55],
            border: [0.0; 4],
            params: [0.0, 0.0, 0.0, 0.0],
        });
        let card = onboarding_ui::card_rect(viewport, state.step, self.settings.language);
        instances.push(CardInstance {
            pos: [card[0], card[1]],
            size: [card[2], card[3]],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [10.0, 0.0, 0.0, 0.0],
        });
        let text_x = card[0] + onboarding_ui::ONBOARDING_PAD;
        let text_w = card[2] - onboarding_ui::ONBOARDING_PAD * 2.0;
        // Заголовок
        texts.push(OwnedScreenText {
            text: self.tr(step.title_key).to_owned(),
            origin: [text_x, card[1] + onboarding_ui::ONBOARDING_PAD],
            width: text_w,
            font_size: onboarding_ui::ONBOARDING_TITLE_FONT,
            color: palette.title,
            align: TextAlign::Left,
        });
        // Прогресс-точки: текущая — акцент, остальные — приглушены
        let (centers, dots_y) = onboarding_ui::progress_dots(card);
        for (i, cx) in centers.iter().enumerate() {
            let current = i == state.step;
            let r = onboarding_ui::ONBOARDING_DOT / 2.0;
            instances.push(CardInstance {
                pos: [cx - r, dots_y],
                size: [r * 2.0, r * 2.0],
                fill: if current {
                    color_to_rgba(palette.link)
                } else {
                    [0.30, 0.33, 0.40, 0.9]
                },
                border: [0.0; 4],
                params: [r, 0.0, 0.0, 1.0],
            });
        }
        // Тело шага (строки переноса — тот же источник, что высота карточки)
        let body_top = card[1] + onboarding_ui::body_top_offset();
        for (i, line) in onboarding_ui::body_lines(state.step, card[2], self.settings.language)
            .iter()
            .enumerate()
        {
            texts.push(OwnedScreenText {
                text: line.clone(),
                origin: [
                    text_x,
                    body_top + i as f32 * onboarding_ui::ONBOARDING_BODY_LINE_H,
                ],
                width: text_w,
                font_size: onboarding_ui::ONBOARDING_BODY_FONT,
                color: palette.body,
                align: TextAlign::Left,
            });
        }
        // Кнопки: Назад (слева, не на первом шаге), Далее/Готово (справа,
        // акцент), Пропустить (правый верх — выход виден всегда, NN/g)
        let buttons = [
            (
                OnboardingButton::Prev,
                state.prev_label_key().map(|key| self.tr(key).to_owned()),
            ),
            (
                OnboardingButton::Next,
                Some(self.tr(state.next_label_key()).to_owned()),
            ),
            (
                OnboardingButton::Skip,
                Some(self.tr(keys::ONBOARDING_SKIP).to_owned()),
            ),
        ];
        for (button, label) in &buttons {
            let Some(label) = label.clone() else {
                continue;
            };
            let rect = onboarding_ui::button_rect(card, *button);
            let hovered = point_in_rect(rect, self.cursor);
            let accent = *button == OnboardingButton::Next;
            instances.push(CardInstance {
                pos: [rect[0], rect[1]],
                size: [rect[2], rect[3]],
                fill: if hovered {
                    hover_fill(if accent {
                        [0.16, 0.32, 0.60, 1.0]
                    } else {
                        palette.menu_fill
                    })
                } else if accent {
                    [0.16, 0.32, 0.60, 1.0]
                } else {
                    [0.20, 0.23, 0.29, 1.0]
                },
                border: [
                    0.35,
                    0.40,
                    0.50,
                    if *button == OnboardingButton::Skip {
                        0.7
                    } else {
                        1.0
                    },
                ],
                params: [6.0, 0.0, 0.0, 1.0],
            });
            texts.push(OwnedScreenText {
                text: label,
                origin: [rect[0], rect[1] + (rect[3] - 13.0 * 1.3) / 2.0],
                width: rect[2],
                font_size: if *button == OnboardingButton::Skip {
                    11.0
                } else {
                    13.0
                },
                color: palette.title,
                align: TextAlign::Center,
            });
        }
        (instances, texts)
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
        let mut groups = palette_groups(&self.scene.canvas, &target, self.settings.language);
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

    /// Оверлей палитры выделения: бар с кнопками групп (иконка + подпись),
    /// открытая hover'ом колонка (строки с иконками и подписями).
    /// Screen-space: константный размер при любом зуме.
    fn palette_overlay(
        &self,
        lay: &PaletteLayout,
        groups: &[crate::palette::PaletteGroup],
        open: Option<usize>,
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let palette = self.effective_palette();
        let tint = color_to_rgba(palette.icon);
        let title = palette.title;
        let accent = [0.18, 0.29, 0.48, 0.95];
        // Фон бара
        instances.push(CardInstance {
            pos: [lay.bar[0], lay.bar[1]],
            size: [lay.bar[2], lay.bar[3]],
            fill: palette.menu_fill,
            border: [0.22, 0.24, 0.30, 0.9],
            params: [8.0, 0.0, 0.0, 1.0],
        });
        for (i, group) in groups.iter().enumerate() {
            let button = lay.groups[i].button;
            let hovered = open == Some(i);
            // Кнопка группы: подсветка при наведении (выпадашка открыта)
            instances.push(CardInstance {
                pos: [button[0], button[1]],
                size: [button[2], button[3]],
                fill: if hovered {
                    [0.24, 0.30, 0.42, 1.0]
                } else {
                    [0.17, 0.18, 0.22, 1.0]
                },
                border: [0.0; 4],
                params: [6.0, 0.0, 0.0, 1.0],
            });
            // Иконка группы (текстовый глиф — ScreenText'ом по центру)
            let icon_rect = [
                button[0] + (button[2] - PAL_ICON) / 2.0,
                button[1] + (button[3] - PAL_ICON) / 2.0,
                PAL_ICON,
                PAL_ICON,
            ];
            instances.extend(icon_quads(group.icon, icon_rect, tint, &palette));
            if let Some(glyph) = icon_text(group.icon) {
                texts.push(OwnedScreenText {
                    text: glyph.to_owned(),
                    origin: [icon_rect[0], icon_rect[1] + 2.0],
                    width: icon_rect[2],
                    font_size: 12.0,
                    color: title,
                    align: TextAlign::Center,
                });
            }
            // Подпись группы под кнопкой
            texts.push(OwnedScreenText {
                text: group.label.clone(),
                origin: lay.groups[i].caption,
                width: button[2],
                font_size: 10.0,
                color: palette.body,
                align: TextAlign::Center,
            });
            // Открытая колонка (hover): фон + строки
            if hovered {
                let drop = lay.groups[i].dropdown;
                instances.push(CardInstance {
                    pos: [drop[0], drop[1]],
                    size: [drop[2], drop[3]],
                    fill: palette.menu_fill,
                    border: [0.22, 0.24, 0.30, 0.9],
                    params: [6.0, 0.0, 0.0, 1.0],
                });
                for (k, entry) in group.entries.iter().enumerate() {
                    let row = lay.groups[i].rows[k];
                    let row_hovered = point_in_rect(row, self.cursor);
                    let fill = if row_hovered {
                        accent
                    } else if entry.current {
                        [0.18, 0.29, 0.48, 0.45]
                    } else {
                        [0.0; 4]
                    };
                    if fill[3] > 0.0 {
                        instances.push(CardInstance {
                            pos: [row[0], row[1]],
                            size: [row[2], row[3]],
                            fill,
                            border: [0.0; 4],
                            params: [4.0, 0.0, 0.0, 1.0],
                        });
                    }
                    if let Some(icon) = entry.icon {
                        let icon_rect = [
                            row[0] + 5.0,
                            row[1] + (row[3] - PAL_ICON) / 2.0,
                            PAL_ICON,
                            PAL_ICON,
                        ];
                        instances.extend(icon_quads(icon, icon_rect, tint, &palette));
                        if let Some(glyph) = icon_text(icon) {
                            texts.push(OwnedScreenText {
                                text: glyph.to_owned(),
                                origin: [icon_rect[0], icon_rect[1] + 2.0],
                                width: icon_rect[2],
                                font_size: 13.0,
                                color: title,
                                align: TextAlign::Center,
                            });
                        }
                        texts.push(OwnedScreenText {
                            text: entry.label.clone(),
                            origin: [row[0] + 28.0, row[1] + 5.0],
                            width: row[2] - 32.0,
                            font_size: 13.0,
                            color: title,
                            align: TextAlign::Left,
                        });
                    } else {
                        // Строка без иконки — текст по всей ширине
                        texts.push(OwnedScreenText {
                            text: entry.label.clone(),
                            origin: [row[0] + 8.0, row[1] + 5.0],
                            width: row[2] - 12.0,
                            font_size: 13.0,
                            color: title,
                            align: TextAlign::Left,
                        });
                    }
                }
            }
        }
        (instances, texts)
    }

    /// Действие палитры: клик по строке выпадашки. Мутирующие действия —
    /// undo-шаг (FR-006); настройки ноды — через apply_node_setting.
    fn apply_palette_action(&mut self, action: PaletteAction) {
        match action {
            PaletteAction::Node {
                node_index,
                setting,
            } => self.apply_node_setting(node_index, setting),
            PaletteAction::NodeColor { targets, preset } => {
                let snapshot = self.scene.canvas.clone();
                for index in targets {
                    if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                        node.color = preset.map(str::to_owned);
                    }
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::NodeGroup(index) => {
                if let Some(mut group) =
                    plan_group_around(&self.scene.canvas, index, crate::ui::GROUP_PADDING)
                {
                    group.label = Some(self.tr(keys::GROUP_DEFAULT_LABEL).to_owned());
                    self.insert_group(group);
                }
            }
            PaletteAction::Layout { seed, mode } => self.apply_related_layout(seed, mode),
            PaletteAction::EdgeStyle { edge_index, style } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.style = Some(style);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::EdgeThickness {
                edge_index,
                thickness,
            } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.thickness = Some(thickness);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            PaletteAction::EdgeColor { edge_index, preset } => {
                let snapshot = self.scene.canvas.clone();
                if let Some(edge) = self.scene.canvas.edges.get_mut(edge_index) {
                    edge.color = preset.map(str::to_owned);
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            // FR-014: тогл типа потока (Value ↔ Control) — undo-шаг +
            // живой пересчёт (downstream может потерять/обрести входы).
            // Тогл в Value, замыкающий цикл, отвергается (isError в MCP;
            // в палитре — toast с участниками). Правка 3: логика тогла —
            // в SceneState::toggle_edge_flow (мутация живого канваса,
            // тестируемо); палитра только показывает toast при цикле.
            PaletteAction::EdgeFlowKind { edge_index, kind } => {
                match self.scene.toggle_edge_flow(edge_index, kind) {
                    Err(participants) => {
                        let participants = participants.join(" → ");
                        self.show_toast(
                            self.trf(keys::TOAST_FLOW_CYCLE, &[("{participants}", &participants)]),
                        );
                        self.request_redraw();
                    }
                    Ok(true) => self.request_redraw(),
                    Ok(false) => {}
                }
            }
            // CR-008: закрепить/освободить конец связи. Закрепление —
            // WYSIWYG: в fromSide/toSide фиксируется текущая эффективная
            // сторона (что видели — то и закрепили). Undo-шаг (FR-006);
            // no-op (состояние не изменилось) шаг не копит.
            PaletteAction::EdgePortsPin {
                edge_index,
                end,
                pin,
            } => self.set_edge_port_pin(edge_index, end, pin),
            PaletteAction::EdgePortsAuto { edge_index } => {
                let Some(edge) = self.scene.canvas.edges.get(edge_index) else {
                    return;
                };
                if !edge.ports_pinned() {
                    return; // no-op — шаг не копится
                }
                let mut snapshot = self.scene.canvas.clone();
                if let Some(edge) = snapshot.edges.get_mut(edge_index) {
                    edge.clear_port_pins();
                }
                self.scene.push_undo(snapshot);
                self.scene.mark_dirty();
                self.show_toast(self.tr(keys::TOAST_PORT_AUTO));
                self.request_redraw();
            }
            // FR-019: ручной update шаблонной ноды (linked-связь):
            // expr/version/icon/color — из манифеста реестра, params — по
            // именам (совпавшие сохраняются, новые — дефолты). Один
            // undo-шаг, пересчёт потока.
            PaletteAction::TemplateUpdate { node_index } => {
                let Some(node) = self.scene.canvas.nodes.get(node_index) else {
                    return;
                };
                let Some(template) = node.template() else {
                    return;
                };
                let Some(manifest) = self.templates.find(&template.id) else {
                    return;
                };
                if manifest.version == template.version {
                    return; // no-op — шаг не копится
                }
                let mut updated = template.clone();
                updated.version = manifest.version.clone();
                updated.expr = manifest.expr.clone();
                updated.icon = manifest.icon.clone();
                updated.color = manifest.color.clone();
                // FR-023: имя шаблона тоже синхронизируется с манифестом
                // (заголовок ноды — актуальное имя из реестра)
                updated.name = Some(manifest.display_name().to_owned());
                let mut params = BTreeMap::new();
                for spec in &manifest.params {
                    let value = template.params.get(&spec.name).cloned().unwrap_or(
                        canvas_core::templates::TemplateParam {
                            num: spec.default,
                            unit: spec.unit.clone(),
                        },
                    );
                    params.insert(spec.name.clone(), value);
                }
                updated.params = params;
                let snapshot = self.scene.canvas.clone();
                if let Some(node) = self.scene.canvas.nodes.get_mut(node_index) {
                    node.set_template(Some(updated));
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
                self.scene.recompute_flow();
                self.show_toast(self.trf(
                    keys::TOAST_TEMPLATE_UPDATED,
                    &[
                        ("{name}", manifest.display_name()),
                        ("{version}", manifest.version.as_str()),
                    ],
                ));
                self.request_redraw();
            }
            // FR-020: «Сохранить как шаблон» — снимок template-ссылки ноды
            // в custom-манифест (~/.canvasdesk/templates/<id>/template.json).
            // Файловая операция — НЕ undo-able; реестр перечитается.
            PaletteAction::SaveAsTemplate { node_index } => {
                let Some(node) = self.scene.canvas.nodes.get(node_index) else {
                    return;
                };
                let Some(template) = node.template() else {
                    return;
                };
                let name = node.label.clone().unwrap_or_else(|| template.id.clone());
                let id = unique_custom_id(&slugify(&name), &self.templates, &self.templates_root);
                let params: Vec<canvas_core::templates::ParamSpec> = template
                    .params
                    .iter()
                    .map(|(name_param, value)| canvas_core::templates::ParamSpec {
                        name: name_param.clone(),
                        // Тип — подсказка UI: выводим по токену единицы
                        kind: infer_param_type(value.unit.as_deref()),
                        default: value.num,
                        unit: value.unit.clone(),
                        min: None,
                        max: None,
                    })
                    .collect();
                let manifest = canvas_core::templates::TemplateManifest {
                    id: id.clone(),
                    name: name.clone(),
                    name_ru: None,
                    version: "1.0.0".to_owned(),
                    category: "custom".to_owned(),
                    description: format!("Сохранено с канваса {}", self.scene.path.display()),
                    description_en: None,
                    params,
                    // FR-029: именованные выходы переезжают в custom-шаблон
                    // вместе со снимком (снапшот переживает пересохранение)
                    outputs: template.outputs.clone(),
                    expr: template.expr.clone(),
                    // FR-046: дефолтный цвет манифеста — константа данных
                    // templates::DEFAULT_TEMPLATE_COLOR (контракт FR-018)
                    color: canvas_core::templates::DEFAULT_TEMPLATE_COLOR.to_owned(),
                    icon: "custom".to_owned(),
                    source: canvas_core::templates::TemplateSource::Custom,
                };
                match canvas_core::templates::save_custom(&manifest, &self.templates_root) {
                    Ok(path) => {
                        // Перезагрузка реестра: custom появится в
                        // палитре/wheel и в MCP template_list
                        self.templates = canvas_core::templates::TemplateRegistry::all_with_custom(
                            &self.templates_root,
                        );
                        self.show_toast(self.trf(
                            keys::TOAST_TEMPLATE_SAVED,
                            &[("{name}", &name), ("{path}", &path.display().to_string())],
                        ));
                    }
                    Err(err) => {
                        self.show_toast(self.trf(
                            keys::TOAST_TEMPLATE_SAVE_FAILED,
                            &[("{err}", &err.to_string())],
                        ));
                    }
                }
                self.request_redraw();
            }
        }
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
        if let Some(path) = &self.config_path {
            if let Err(err) = self.settings.save(path) {
                tracing::warn!(%err, "не удалось сохранить конфиг");
            }
        }
    }

    /// FR-025: сохранить развёрнутость палитры-дока в конфиг
    /// (сворачивание по Esc/кнопке «‹», разворачивание по ручке/Ctrl+P).
    fn persist_palette_dock(&mut self) {
        self.settings.template_palette_open = self.template_panel.open;
        if let Some(path) = &self.config_path {
            if let Err(err) = self.settings.save(path) {
                tracing::warn!(%err, "не удалось сохранить конфиг");
            }
        }
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
        let strip = template_ui::dock_strip_layout(&categories, viewport[1]);
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

    /// FR-026: применить переключение булевой строки панели настроек
    /// (тумблер) и сохранить конфиг. Многозначные строки — через
    /// выпадающее меню ([`App::apply_dropdown_choice`]).
    fn apply_toggle_row(&mut self, row: SettingsRow) {
        match row {
            SettingsRow::Grid => {
                self.settings.grid_visible = !self.settings.grid_visible;
            }
            SettingsRow::EdgesAvoid => {
                self.settings.edges_avoid_nodes = !self.settings.edges_avoid_nodes;
            }
            // FR-025: построчные точки выхода — рендер/hit-тест читают флаг
            // на кадре (SceneView.line_ports), синхронизация рендера не нужна
            SettingsRow::LinePorts => {
                self.settings.line_ports = !self.settings.line_ports;
            }
            // FR-016 (CP5): оверлей узких мест — рендер читает флаг на кадре
            // (SceneView.analysis_overlay), синхронизация рендера не нужна
            SettingsRow::BottleneckOverlay => {
                self.settings.bottleneck_overlay = !self.settings.bottleneck_overlay;
                self.bottleneck_auto_enabled = true;
            }
            // FR-038 (п.19): тумблеры магнитной раскладки — движок и рендер
            // читают флаги на кадр. Мастер-тумблер (п.5) гасит весь снаппинг
            // без сброса остальных настроек — активный предпросмотр убираем
            // сразу (drag с открытой панелью невозможен, но защита дешёвая)
            SettingsRow::SnapEnabled => {
                self.settings.snap_enabled = !self.settings.snap_enabled;
                if !self.settings.snap_enabled {
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.clear_guides();
                    }
                }
            }
            SettingsRow::SnapGrid => {
                self.settings.snap_to_grid = !self.settings.snap_to_grid;
            }
            SettingsRow::SnapGuides => {
                self.settings.snap_to_guides = !self.settings.snap_to_guides;
            }
            SettingsRow::SnapCollision => {
                self.settings.snap_collision = !self.settings.snap_collision;
            }
            // T23: состояние синхронно с settings — сохранение общим хвостом
            SettingsRow::FocusMode => self.toggle_focus_mode(),
            // FR-042 (F-13): агрегация рендер/ввод читают на кадр
            // (SceneView.bundles + гейты открытия stage); при выключении
            // открытый stage закрывается (инвариант согласованности)
            SettingsRow::EdgeAggregation => {
                self.settings.edge_aggregation = !self.settings.edge_aggregation;
                if !self.settings.edge_aggregation {
                    self.close_main_stage();
                    self.bundle_hover = None;
                }
            }
            SettingsRow::HudOnStart => {
                self.settings.hud_on_start = !self.settings.hud_on_start;
                // Мгновенная обратная связь: HUD переключается сразу
                self.hud_visible = self.settings.hud_on_start;
            }
            SettingsRow::ButtonCorner
            | SettingsRow::GridStyle
            | SettingsRow::GridDensity
            | SettingsRow::PortZone
            | SettingsRow::Language
            | SettingsRow::ThemePreset
            | SettingsRow::SnapTolerance
            | SettingsRow::SnapSubZoom
            | SettingsRow::SnapCoarseZoom
            // PRD-0007 (AC-2.3): dropdown «Лимит глубины explain-дерева» —
            // применяется в apply_dropdown_choice, тумблером не является
            | SettingsRow::ExplainDepthLimit => {
                debug_assert!(false, "dropdown-строка не тумблер: {row:?}");
                return;
            }
        }
        self.sync_settings_row(row);
        self.save_settings();
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

    /// FR-026: общий хвост применения настроек — сохранение config.toml.
    fn save_settings(&self) {
        if let Some(path) = &self.config_path {
            if let Err(err) = self.settings.save(path) {
                tracing::warn!(%err, "не удалось сохранить конфиг");
            }
        }
    }

    /// FR-027: открыть просмотрщик документации на странице (подменю «?»
    /// или внутренняя ссылка): раскладка собирается под текущую ширину,
    /// скролл сверху; меню помощи закрывается (одна «панель помощи» активна).
    fn open_docs_page(&mut self, page: usize) {
        let viewport = self.viewport_logical();
        let panel = docs_ui::viewer_rect(viewport);
        let content = docs_ui::viewer_content_rect(panel);
        let layout = docs_ui::layout_page(page, content[2]);
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

    /// Screen-space оверлей настроек: летающая кнопка всегда, панель — когда
    /// открыта. Координаты — логические px от левого верхнего угла окна.
    fn settings_overlay(&self) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut instances = Vec::new();
        let mut texts = Vec::new();
        let viewport = self.viewport_logical();
        if viewport[0] <= 0.0 || viewport[1] <= 0.0 {
            return (instances, texts);
        }
        let palette = self.effective_palette();
        // Вертикальная центровка иконки: лайн-бокс высотой font*1.3 по центру
        // кнопки (так же считает рендер screen-текстов).
        let icon_top = |rect: [f32; 4], font_size: f32| rect[1] + (rect[3] - font_size * 1.3) / 2.0;
        let button = button_rect(self.settings.button_corner, viewport);
        // Hover-аффорданс: курсор над кнопкой — заливка ярче
        let settings_hovered = point_in_rect(button, self.cursor);
        instances.push(CardInstance {
            pos: [button[0], button[1]],
            size: [button[2], button[3]],
            fill: if settings_hovered {
                hover_fill(palette.menu_fill)
            } else {
                palette.menu_fill
            },
            border: [0.0; 4],
            // params.y = рамка выделения: подсветка кнопки при открытой панели
            params: [8.0, self.settings_open as u8 as f32, 0.0, 0.0],
        });
        // Иконка-шестерёнка: горизонтально по центру кнопки (Align::Center
        // в области width = ширине кнопки) — не зависит от метрик глифа.
        texts.push(OwnedScreenText {
            text: "⚙".to_owned(),
            origin: [button[0], icon_top(button, 18.0)],
            width: button[2],
            font_size: 18.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Кнопка переключения темы — рядом с кнопкой настроек (в тот же угол).
        // Иконка показывает ЦЕЛЬ: в тёмной теме «солнце» (клик — светлая).
        let theme_button = theme_button_rect(self.settings.button_corner, viewport);
        let theme_hovered = point_in_rect(theme_button, self.cursor);
        instances.push(CardInstance {
            pos: [theme_button[0], theme_button[1]],
            size: [theme_button[2], theme_button[3]],
            fill: if theme_hovered {
                hover_fill(palette.menu_fill)
            } else {
                palette.menu_fill
            },
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 0.0],
        });
        let theme_icon = match self.settings.theme {
            Theme::Dark => "☀",
            Theme::Light => "🌙",
        };
        texts.push(OwnedScreenText {
            text: theme_icon.to_owned(),
            origin: [theme_button[0], icon_top(theme_button, 16.0)],
            width: theme_button[2],
            font_size: 16.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // FR-027: кнопка «?» — третий элемент кластера (настройки/тема/помощь):
        // вход в меню документации и онбординга; hover-аффорданс как у ⚙
        let help_button = help_button_rect(self.settings.button_corner, viewport);
        let help_hovered = point_in_rect(help_button, self.cursor);
        instances.push(CardInstance {
            pos: [help_button[0], help_button[1]],
            size: [help_button[2], help_button[3]],
            fill: if help_hovered {
                hover_fill(palette.menu_fill)
            } else {
                palette.menu_fill
            },
            border: [0.0; 4],
            params: [8.0, self.help_menu.is_some() as u8 as f32, 0.0, 0.0],
        });
        texts.push(OwnedScreenText {
            text: "?".to_owned(),
            origin: [help_button[0], icon_top(help_button, 18.0)],
            width: help_button[2],
            font_size: 18.0,
            color: palette.title,
            align: TextAlign::Center,
        });
        // Панель горячих клавиш (FR-004): у левого края, по центру;
        // рендерится независимо от панели настроек
        if self.hotkeys_open {
            let panel = hotkeys_panel_rect(viewport);
            instances.push(CardInstance {
                pos: [panel[0], panel[1]],
                size: [panel[2], panel[3]],
                fill: palette.menu_fill,
                border: [0.0; 4],
                params: [8.0, 0.0, 0.0, 0.0],
            });
            let pad = crate::ui::HOTKEYS_PADDING;
            let header_h = crate::ui::HOTKEYS_HEADER_HEIGHT;
            let row_h = crate::ui::HOTKEYS_ROW_HEIGHT;
            let key_w = crate::ui::HOTKEYS_KEY_COLUMN;
            let key_x = panel[0] + pad;
            let desc_x = panel[0] + pad + key_w;
            let desc_w = (panel[2] - pad * 2.0 - key_w).max(10.0);
            texts.push(OwnedScreenText {
                text: self.tr(keys::HOTKEYS_TITLE).to_owned(),
                origin: [key_x, panel[1] + pad + 7.0],
                width: panel[2] - pad * 2.0,
                font_size: 15.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            for (i, (key, description)) in crate::ui::HOTKEYS.iter().enumerate() {
                let y = panel[1] + pad + header_h + i as f32 * row_h + 3.0;
                // Строки ниже кромки панели (кламп высоты) не рисуем
                if y + row_h > panel[1] + panel[3] - 2.0 {
                    break;
                }
                texts.push(OwnedScreenText {
                    text: self.tr(key).to_owned(),
                    origin: [key_x, y],
                    width: key_w,
                    font_size: 12.0,
                    color: palette.link,
                    align: TextAlign::Left,
                });
                texts.push(OwnedScreenText {
                    text: self.tr(description).to_owned(),
                    origin: [desc_x, y],
                    width: desc_w,
                    font_size: 12.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
        }
        if !self.settings_open {
            return (instances, texts);
        }
        // FR-039: затемнение канваса под модалкой (паттерн онбординга
        // FR-028) — фокус на диалоге настроек, ввод под ним глушится
        instances.push(CardInstance {
            pos: [0.0, 0.0],
            size: [viewport[0], viewport[1]],
            fill: [0.02, 0.02, 0.04, 0.45],
            border: [0.0; 4],
            params: [0.0, 0.0, 0.0, 0.0],
        });
        // FR-039: модалка по центру — layout несёт rect'ы навигации,
        // заголовка раздела, строк активного таба и карточек темы
        let layout = modal_layout(self.settings_tab, viewport);
        let modal = layout.rect;
        instances.push(CardInstance {
            pos: [modal[0], modal[1]],
            size: [modal[2], modal[3]],
            fill: palette.menu_fill,
            border: [0.0; 4],
            params: [8.0, 0.0, 0.0, 0.0],
        });
        // Левая колонка: пункты «иконка + название» (Obsidian); активный
        // раздел — акцентная подложка, неактивные — hover-подсветка
        let tab_index = self.settings_tab.min(SETTINGS_TABS.len() - 1);
        for (i, tab) in SETTINGS_TABS.iter().enumerate() {
            let Some(item) = layout.nav_items.get(i) else {
                continue;
            };
            let active = i == tab_index;
            let hovered = point_in_rect(*item, self.cursor);
            if active {
                instances.push(CardInstance {
                    pos: [item[0] + 4.0, item[1] + 2.0],
                    size: [item[2] - 8.0, item[3] - 4.0],
                    fill: palette.palette_selected_fill,
                    border: [0.0; 4],
                    params: [6.0, 0.0, 0.0, 1.0],
                });
            } else if hovered {
                instances.push(CardInstance {
                    pos: [item[0] + 4.0, item[1] + 2.0],
                    size: [item[2] - 8.0, item[3] - 4.0],
                    fill: palette.palette_hover_fill,
                    border: [0.0; 4],
                    params: [6.0, 0.0, 0.0, 1.0],
                });
            }
            texts.push(OwnedScreenText {
                text: tab.icon.to_owned(),
                origin: [item[0] + 12.0, item[1] + (item[3] - 14.0 * 1.3) / 2.0],
                width: 20.0,
                font_size: 14.0,
                color: if active { palette.link } else { palette.icon },
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: self.tr(tab.title_key).to_owned(),
                origin: [item[0] + 36.0, item[1] + (item[3] - 13.0 * 1.3) / 2.0],
                width: item[2] - 36.0 - 6.0,
                font_size: 13.0,
                color: if active { palette.title } else { palette.body },
                align: TextAlign::Left,
            });
        }
        // Подсказка внизу левой колонки (перенос из подвала панели FR-026)
        texts.push(OwnedScreenText {
            text: self.tr(keys::SETTINGS_HINT).to_owned(),
            origin: [layout.hint_rect[0], layout.hint_rect[1] + 4.0],
            width: layout.hint_rect[2],
            font_size: 11.0,
            color: palette.icon,
            align: TextAlign::Left,
        });
        // Заголовок раздела (правая панель)
        let tab_def = &SETTINGS_TABS[tab_index];
        texts.push(OwnedScreenText {
            text: self.tr(tab_def.title_key).to_owned(),
            origin: [layout.title_rect[0], layout.title_rect[1] + 3.0],
            width: layout.title_rect[2],
            font_size: 16.0,
            color: palette.title,
            align: TextAlign::Left,
        });
        // FR-026/FR-039: открытое выпадающее меню — геометрия и пункты
        // (состояние не хранит список — вычисляется из настроек, устареть
        // не может); ширина меню = ширине контрола строки (FR-039 §1)
        let menu = self.settings_dropdown.open_row.map(|row| {
            let items = dropdown_options(row, &self.settings);
            let anchor = layout
                .row_rect(row)
                .map(|rect| control_rect(rect, RowKind::Dropdown))
                .unwrap_or([0.0; 4]);
            let rect = dropdown_layout(anchor, viewport, items.len(), anchor[2]);
            (row, items, rect)
        });
        // Карточки темы (таб «Внешний вид», паттерн Obsidian «Base theme»):
        // активная — акцентная рамка; клик — прямой выбор (логика кнопки
        // ☀/🌙). params.y = рамка выделения (паттерн кнопки ⚙)
        if tab_def.theme_cards {
            for (theme, label_key) in [
                (Theme::Dark, keys::THEME_DARK),
                (Theme::Light, keys::THEME_LIGHT),
            ] {
                let card = layout.theme_card_rect(theme);
                // FR-047: карточка отмечена только при БЕЗАКТИВНОМ пресете
                // (пресет перекрывает классику — отметка была бы ложной)
                let selected =
                    self.settings.theme_preset.is_empty() && self.settings.theme == theme;
                let hovered = point_in_rect(card, self.cursor);
                instances.push(CardInstance {
                    pos: [card[0], card[1]],
                    size: [card[2], card[3]],
                    fill: if selected {
                        palette.palette_selected_fill
                    } else if hovered {
                        palette.palette_hover_fill
                    } else {
                        palette.palette_row_fill
                    },
                    border: if selected {
                        color_to_rgba(palette.link)
                    } else {
                        palette.palette_border
                    },
                    params: [8.0, selected as u8 as f32, 0.0, 1.0],
                });
                texts.push(OwnedScreenText {
                    text: self.tr(label_key).to_owned(),
                    origin: [card[0], card[1] + card[3] / 2.0 - 8.5],
                    width: card[2],
                    font_size: 13.0,
                    color: palette.title,
                    align: TextAlign::Center,
                });
            }
        }
        // Строки единой сетки: лейбл (БЕЗ значения) + описание приглушённым
        // кеглем, контрол справа — pill-тумблер или dropdown-кнопка
        for (row, rect) in &layout.rows {
            // Квады рисуются ДО всех screen-текстов (renderer.rs):
            // строки, перекрытые меню, не рисуем — иначе их текст
            // проступит сквозь фон меню
            if menu
                .as_ref()
                .is_some_and(|(_, _, menu_rect)| rects_intersect(*menu_rect, *rect))
            {
                continue;
            }
            // Hover-подсветка кликабельной строки (аффорданс)
            if point_in_rect(*rect, self.cursor) {
                instances.push(CardInstance {
                    pos: [rect[0], rect[1] + 1.0],
                    size: [rect[2], rect[3] - 2.0],
                    fill: [0.24, 0.30, 0.42, 0.35],
                    border: [0.0; 4],
                    params: [4.0, 0.0, 0.0, 1.0],
                });
            }
            texts.push(OwnedScreenText {
                text: self.tr(row_label_key(*row)).to_owned(),
                origin: [rect[0] + 2.0, rect[1] + 4.0],
                width: rect[2] - MODAL_ROW_LABEL_W,
                font_size: 13.0,
                color: palette.body,
                align: TextAlign::Left,
            });
            texts.push(OwnedScreenText {
                text: self.tr(row_desc_key(*row)).to_owned(),
                origin: [rect[0] + 2.0, rect[1] + 22.0],
                width: rect[2] - MODAL_ROW_LABEL_W,
                font_size: 11.0,
                color: palette.icon,
                align: TextAlign::Left,
            });
            let kind = row_kind(*row);
            let control = control_rect(*rect, kind);
            match kind {
                RowKind::Toggle => {
                    let on = match row {
                        SettingsRow::Grid => self.settings.grid_visible,
                        SettingsRow::EdgesAvoid => self.settings.edges_avoid_nodes,
                        SettingsRow::LinePorts => self.settings.line_ports,
                        SettingsRow::BottleneckOverlay => self.settings.bottleneck_overlay,
                        // FR-038 (п.19): тумблеры магнитной раскладки
                        SettingsRow::SnapEnabled => self.settings.snap_enabled,
                        SettingsRow::SnapGrid => self.settings.snap_to_grid,
                        SettingsRow::SnapGuides => self.settings.snap_to_guides,
                        SettingsRow::SnapCollision => self.settings.snap_collision,
                        SettingsRow::FocusMode => self.settings.focus_mode,
                        SettingsRow::EdgeAggregation => self.settings.edge_aggregation,
                        SettingsRow::HudOnStart => self.settings.hud_on_start,
                        SettingsRow::ButtonCorner
                        | SettingsRow::GridStyle
                        | SettingsRow::GridDensity
                        | SettingsRow::PortZone
                        | SettingsRow::Language
                        | SettingsRow::ThemePreset
                        | SettingsRow::SnapTolerance
                        | SettingsRow::SnapSubZoom
                        | SettingsRow::SnapCoarseZoom
                        // PRD-0007: dropdown-строка в ветку Toggle не
                        // попадает (row_kind = Dropdown), arm — для полноты
                        | SettingsRow::ExplainDepthLimit => false,
                    };
                    // Pill-тумблер: трек (включён — акцент) + ручка-квад,
                    // позиция отражает значение (рисуется квадами)
                    instances.push(CardInstance {
                        pos: [control[0], control[1]],
                        size: [control[2], control[3]],
                        fill: if on {
                            color_to_rgba(palette.link)
                        } else {
                            [0.30, 0.33, 0.40, 0.9]
                        },
                        border: [0.0; 4],
                        params: [control[3] / 2.0, 0.0, 0.0, 1.0],
                    });
                    let knob = pill_knob_rect(control, on);
                    instances.push(CardInstance {
                        pos: [knob[0], knob[1]],
                        size: [knob[2], knob[3]],
                        fill: [0.92, 0.92, 0.94, 1.0],
                        border: [0.0; 4],
                        params: [knob[2] / 2.0, 0.0, 0.0, 1.0],
                    });
                }
                RowKind::Dropdown => {
                    // Кнопка со значением + ▾ (HIG «Pop-Up Buttons»)
                    instances.push(CardInstance {
                        pos: [control[0], control[1]],
                        size: [control[2], control[3]],
                        fill: if point_in_rect(control, self.cursor) {
                            hover_fill(palette.palette_chip_fill)
                        } else {
                            palette.palette_chip_fill
                        },
                        border: palette.palette_border,
                        params: [6.0, 0.0, 0.0, 1.0],
                    });
                    if let Some(value) = dropdown_value(*row, &self.settings) {
                        texts.push(OwnedScreenText {
                            text: value,
                            origin: [control[0] + 10.0, control[1] + 4.0],
                            width: control[2] - 26.0,
                            font_size: 12.0,
                            color: palette.title,
                            align: TextAlign::Left,
                        });
                    }
                    texts.push(OwnedScreenText {
                        text: "▾".to_owned(),
                        origin: [control[0] + control[2] - 16.0, control[1] + 4.0],
                        width: 14.0,
                        font_size: 12.0,
                        color: palette.icon,
                        align: TextAlign::Left,
                    });
                }
            }
        }
        // FR-026: выпадающее меню — поверх модалки: фон чуть ярче панели,
        // hover/клавиатурное выделение пункта, галочка у текущего значения
        if let Some((row, items, menu_rect)) = &menu {
            instances.push(CardInstance {
                pos: [menu_rect[0], menu_rect[1]],
                size: [menu_rect[2], menu_rect[3]],
                fill: hover_fill(palette.menu_fill),
                border: [0.0; 4],
                params: [8.0, 0.0, 0.0, 0.0],
            });
            let hovered_item = dropdown_item_at(*menu_rect, items.len(), self.cursor);
            for (i, (label, current)) in items.iter().enumerate() {
                let y = menu_rect[1] + DROPDOWN_MARGIN + i as f32 * DROPDOWN_ROW_H;
                let highlighted = hovered_item == Some(i)
                    || (self.settings_dropdown.open_row == Some(*row)
                        && self.settings_dropdown.selected == i);
                if highlighted {
                    instances.push(CardInstance {
                        pos: [menu_rect[0] + 3.0, y + 1.0],
                        size: [menu_rect[2] - 6.0, DROPDOWN_ROW_H - 2.0],
                        fill: [0.24, 0.30, 0.42, 0.6],
                        border: [0.0; 4],
                        params: [4.0, 0.0, 0.0, 1.0],
                    });
                }
                if *current {
                    texts.push(OwnedScreenText {
                        text: "✓".to_owned(),
                        origin: [menu_rect[0] + 8.0, y + 5.0],
                        width: 18.0,
                        font_size: 12.0,
                        color: palette.link,
                        align: TextAlign::Left,
                    });
                }
                texts.push(OwnedScreenText {
                    text: label.clone(),
                    origin: [menu_rect[0] + MENU_LABEL_X, y + 4.0],
                    width: menu_rect[2] - MENU_LABEL_X - DROPDOWN_MARGIN,
                    font_size: 13.0,
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
        }
        (instances, texts)
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
    /// FR-052 (U2): Esc-диспетчер реестра — тела прежней лестницы
    /// 8143–8232 дословно, порядок задаёт `SurfaceRegistry::esc_stack`.
    /// `true` — поверхность поглотила Esc (обход стека прекращается).
    fn dispatch_esc(&mut self, surface: &str) -> bool {
        match surface {
            // FR-042 (E3, инвариант 8): первый Esc закрывает открытый
            // main stage (при открытом stage прочие оверлеи закрыты)
            // PRD-0007 (X2): окно проверки закрывается одним Esc
            ui_registry::id::EXPLAIN => {
                if self.explain.is_some() {
                    self.close_explain();
                    true
                } else {
                    false
                }
            }
            ui_registry::id::STAGE => self.main_stage.take().is_some(),
            // FR-027: двухэтапный Esc — подменю → меню → закрыто
            ui_registry::id::HELP_MENU => {
                if let Some(menu) = self.help_menu.take() {
                    // Подменю открыто — первый Esc закрывает только его
                    if menu.docs_open {
                        self.help_menu = Some(HelpMenuState {
                            origin: menu.origin,
                            docs_open: false,
                        });
                    }
                    true
                } else {
                    false
                }
            }
            ui_registry::id::DOCS => self.docs.take().is_some(),
            // Раскрытая колонка палитры закрывается без снятия выделения
            ui_registry::id::PALETTE => {
                if self.palette_hover.open.is_some() || self.palette_hover.pending() {
                    self.palette_hover.reset();
                    true
                } else {
                    false
                }
            }
            // Ревизия FR-025: Esc гасит flyout свёрнутой полосы палитры
            ui_registry::id::TEMPLATE_STRIP => {
                if self
                    .template_hover
                    .as_ref()
                    .is_some_and(|h| h.open.is_some() || h.pending())
                {
                    self.template_hover = None;
                    true
                } else {
                    false
                }
            }
            // FR-025 п.3: Esc сворачивает развёрнутый док и БЕЗ
            // клавиатурного фокуса — мышиный expand() даёт focused=false
            ui_registry::id::TEMPLATE_PANEL => {
                if self.template_panel.open {
                    self.template_panel.close();
                    self.persist_palette_dock();
                    true
                } else {
                    false
                }
            }
            ui_registry::id::MENU => self.menu.take().is_some(),
            // FR-050 Н2 (этап C): Esc закрывает меню выбора (отмена —
            // ребро не создаётся, «либо отмена» в постановке)
            ui_registry::id::CHOICE_MENU => self.choice_menu.take().is_some(),
            ui_registry::id::SETTINGS => {
                // FR-026: Esc при открытом меню закрывает ТОЛЬКО меню
                // (повторный Esc закроет панель — семантика popup FR-021)
                if self.settings_dropdown.is_open() {
                    self.settings_dropdown.reset();
                } else {
                    self.settings_open = false;
                }
                true
            }
            ui_registry::id::HOTKEYS => {
                if self.hotkeys_open {
                    self.hotkeys_open = false;
                    true
                } else {
                    false
                }
            }
            // FR-017: выход из what-if режима (подмены не теряются —
            // они в персистентных сценариях `.canvas`, Q3b)
            ui_registry::id::WHATIF => {
                if self.scene.whatif_active {
                    self.exit_whatif_mode();
                    true
                } else {
                    false
                }
            }
            // FR-018: wheel-меню закрывается Esc (панель шаблонов — раньше)
            ui_registry::id::WHEEL => self.wheel_menu.take().is_some(),
            _ => false,
        }
    }

    fn on_key(&mut self, event: &KeyEvent) {
        // FR-052 (U2 PRD-0009): маршрутизация клавиатуры из реестра —
        // владелец = верх esc_stack активных поверхностей (дословно
        // воспроизводит прежние head-ветки; NUMI-хоткеи канваса не
        // тронуты — Q4 §11 PRD-0009).
        let registry = ui_registry::build_registry(self);
        match ui_registry::key_owner(&registry) {
            ui_registry::KeyOwner::Onboarding => {
                // FR-028: открытый онбординг глушит канвас-хоткеи (тур модален);
                // Esc — «Пропустить» (отложить до следующего запуска)
                if self.onboarding.is_some() {
                    if event.state == ElementState::Pressed
                        && !event.repeat
                        && event.logical_key == Key::Named(NamedKey::Escape)
                    {
                        self.defer_onboarding();
                        self.request_redraw();
                    }
                    return;
                }
                return;
            }
            ui_registry::KeyOwner::Gallery => {
                // FR-049: модальная галерея схем — клавиатура галереи (↑/↓/Enter/
                // Esc/фильтр), остальное глотается (канвас не получает)
                if self.scheme_gallery.open {
                    if event.state == ElementState::Pressed && self.on_gallery_key(event) {
                        self.request_redraw();
                    }
                    return;
                }
                return;
            }
            ui_registry::KeyOwner::Editor => {
                // Активное редактирование (T7): клавиатура уходит в редактор
                if self.editing.is_some() {
                    if event.state != ElementState::Pressed {
                        return;
                    }
                    let ctrl = self.modifiers.control_key();
                    let shift = self.modifiers.shift_key();
                    // FR-021: при открытом popup подсказок навигация/выбор
                    // перехватываются ДО команд редактора: Enter/Tab принимают
                    // подсказку (НЕ коммитят заметку), Esc закрывает только popup
                    // (повторный Esc — откат правки, прежнее поведение)
                    if self.hints.open {
                        match &event.logical_key {
                            Key::Named(NamedKey::ArrowDown) if !event.repeat => {
                                self.hints.move_selection(1);
                                self.request_redraw();
                                return;
                            }
                            Key::Named(NamedKey::ArrowUp) if !event.repeat => {
                                self.hints.move_selection(-1);
                                self.request_redraw();
                                return;
                            }
                            Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Tab)
                                if !event.repeat =>
                            {
                                self.accept_hint();
                                return;
                            }
                            Key::Named(NamedKey::Escape) if !event.repeat => {
                                self.hints.reset();
                                self.request_redraw();
                                return;
                            }
                            _ => {}
                        }
                    }
                    let Some(command) = map_key(&event.logical_key, ctrl, shift) else {
                        return;
                    };
                    match command {
                        KeyCommand::Commit => self.finish_editing(true),
                        KeyCommand::Cancel => self.finish_editing(false),
                        KeyCommand::Copy => {
                            if let Some(text) =
                                self.editing.as_ref().and_then(|s| s.copy_selection())
                            {
                                self.clipboard.set_text(text);
                            }
                        }
                        KeyCommand::Cut => {
                            let text = match (self.editing.as_mut(), self.renderer.as_mut()) {
                                (Some(session), Some(renderer)) => {
                                    session.cut_selection(renderer.font_system_mut())
                                }
                                _ => None,
                            };
                            if let Some(text) = text {
                                self.clipboard.set_text(text);
                                self.request_redraw();
                            }
                        }
                        KeyCommand::Paste => {
                            let text = self.clipboard.get_text();
                            let pasted = if let (Some(text), Some(session), Some(renderer)) =
                                (text, self.editing.as_mut(), self.renderer.as_mut())
                            {
                                session.insert_text(renderer.font_system_mut(), &text);
                                true
                            } else {
                                false
                            };
                            if pasted {
                                self.fit_note_size();
                                self.update_hints();
                                self.request_redraw();
                            }
                        }
                        other => {
                            let applied = if let (Some(session), Some(renderer)) =
                                (self.editing.as_mut(), self.renderer.as_mut())
                            {
                                session.apply(renderer.font_system_mut(), other);
                                true
                            } else {
                                false
                            };
                            if applied {
                                // Текст мог вырасти (wrap/новые строки) — подгоняем
                                // высоту заметки под контент прямо во время набора
                                self.fit_note_size();
                                // FR-021: popup подсказок — следом за правкой текста
                                self.update_hints();
                                self.request_redraw();
                            }
                        }
                    }
                    return;
                }
            }
            ui_registry::KeyOwner::Search => {
                // Панель поиска (T14): открыта — клавиатура уходит в панель
                // (ввод/каретка/Enter/Esc/F3), канвас-хоткеи приглушены
                if self.search.is_open() {
                    if event.state == ElementState::Pressed {
                        self.on_search_key(event);
                    }
                    return;
                }
                return;
            }
            ui_registry::KeyOwner::TemplatePanel => {
                // Прежний гейт 8036: панель без клавиатурного фокуса
                // клавиши не перехватывает — лестница (Ctrl+P и др.) работает
                if self.template_panel.focused && self.on_template_panel_key(event) {
                    return;
                }
            }
            ui_registry::KeyOwner::Dialog => {
                // T21: модальный диалог глушит весь ввод канваса — Enter/Esc —
                // подтвердить/отменить, остальное игнорируется (П10/П11)
                if self.dialog.is_some() && event.state == ElementState::Pressed && !event.repeat {
                    match event.logical_key {
                        Key::Named(NamedKey::Enter) => {
                            self.confirm_dialog();
                        }
                        Key::Named(NamedKey::Escape) => self.cancel_dialog(),
                        _ => {}
                    }
                    return;
                }
                return;
            }
            ui_registry::KeyOwner::Explain => {
                // PRD-0007 (X2): открытое окно проверки — Esc закрывает
                // (§6.4: Ready/Stale → Closed); прочие клавиши — в лестницу
                if event.state == ElementState::Pressed
                    && !event.repeat
                    && event.logical_key == Key::Named(NamedKey::Escape)
                {
                    self.close_explain();
                    self.request_redraw();
                    return;
                }
            }
            ui_registry::KeyOwner::Stage => {
                // Любая клавиша закрывает stage (8233–8239; Esc — 8141):
                // нужный оверлей откроется следующим нажатием
                self.main_stage = None;
                self.request_redraw();
                return;
            }
            ui_registry::KeyOwner::Canvas => {}
        }
        // Esc-лестница из реестра: порядок esc_stack воспроизводит прежнюю
        // ручную лестницу 8143–8232 дословно (первый поглотитель останавливает)
        if event.logical_key == Key::Named(NamedKey::Escape)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            for surface in registry.esc_stack() {
                if self.dispatch_esc(surface.as_str()) {
                    self.request_redraw();
                    return;
                }
            }
        }
        // Ctrl+F — открыть панель поиска (T14; кириллическая раскладка — «а»);
        // активное редактирование сначала фиксируется
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("f") || c.eq_ignore_ascii_case("а"))
        {
            if self.editing.is_some() {
                self.finish_editing(true);
            }
            self.search.open();
            self.request_redraw();
            return;
        }
        // Ctrl+P — фокус в поиск палитры шаблонов (FR-018/FR-025;
        // кириллическая раскладка — «з»). Палитра — постоянный док:
        // Ctrl+P её не закрывает, а фокусирует (по свёрнутой — разворачивает
        // с чистым фильтром). Взаимоисключающе с wheel-меню
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("p") || c.eq_ignore_ascii_case("з"))
        {
            self.wheel_menu = None;
            // Ревизия FR-025: развёрнутый док гасит flyout полосы
            self.template_hover = None;
            if self.template_panel.open {
                self.template_panel.focus_search();
            } else {
                self.template_panel.open();
            }
            // W9 (web-приёмка): оракул браузерного дыма — палитра открылась
            // и реестр не пуст (DEBUG — на нативе под дефолтным фильтром
            // не виден; на web виден с ?log=debug)
            tracing::debug!(
                templates = self.templates.list().len(),
                categories = self.templates.categories().len(),
                "шаблонная палитра: док открыт/сфокусирован"
            );
            self.request_redraw();
            return;
        }
        // FR-017 (CP6): Ctrl+Shift+I — вход/выход из what-if режима
        // (Q3: Ctrl+W отклонён — мышечная память «закрыть вкладку»;
        // Ctrl+I занят курсивом в редакторе; кириллица — «Ш»).
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && self.modifiers.shift_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("i") || c.eq_ignore_ascii_case("ш"))
        {
            if self.scene.whatif_active {
                self.exit_whatif_mode();
            } else {
                self.enter_whatif_mode();
            }
            self.request_redraw();
            return;
        }
        // FR-026: клавиатура выпадающего меню настроек — ↑/↓ сдвигают
        // выделение, Enter применяет (модель popup FR-021); Esc обрабатывается
        // ниже — первым делом закрывает меню, панель остаётся открытой
        if self.settings_open
            && self.settings_dropdown.is_open()
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            let row = self.settings_dropdown.open_row.expect("меню открыто");
            let count = dropdown_options(row, &self.settings).len();
            match &event.logical_key {
                Key::Named(NamedKey::ArrowDown) => {
                    self.settings_dropdown.move_selection(1, count);
                    self.request_redraw();
                    return;
                }
                Key::Named(NamedKey::ArrowUp) => {
                    self.settings_dropdown.move_selection(-1, count);
                    self.request_redraw();
                    return;
                }
                Key::Named(NamedKey::Enter) => {
                    let selected = self.settings_dropdown.selected;
                    self.apply_dropdown_choice(row, selected);
                    self.settings_dropdown.reset();
                    self.request_redraw();
                    return;
                }
                _ => {}
            }
        }
        // F1 — панель горячих клавиш (FR-004): раскладконезависимая
        // функциональная клавиша; внутри редактора/поиска не работает
        // (клавиатура ушла туда раньше — return выше)
        if event.logical_key == Key::Named(NamedKey::F1)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.hotkeys_open = !self.hotkeys_open;
            self.request_redraw();
            return;
        }
        // Ctrl+C/V/D/X — буфер нодов (FR-003/FR-007; кириллица: с/м/в/ч —
        // те же физические клавиши). Ctrl+Z/Y — undo/redo (FR-006;
        // кириллица: я/н). Внутри редактора эти клавиши — текстовые (выше
        // return: Ctrl+X там — вырезание текста), во время поиска — панель
        if event.state == ElementState::Pressed && !event.repeat && self.modifiers.control_key() {
            if let Key::Character(c) = &event.logical_key {
                match c.to_lowercase().as_str() {
                    "c" | "с" => self.copy_selection(),
                    "v" | "м" => self.paste_clipboard(),
                    "d" | "в" => self.duplicate_selection(),
                    "x" | "ч" => self.cut_selection(),
                    "z" | "я" => {
                        // Ctrl+Shift+Z — общепринятый синоним redo
                        if self.modifiers.shift_key() {
                            self.redo_action();
                        } else {
                            self.undo_action();
                        }
                    }
                    "y" | "н" => self.redo_action(),
                    "g" | "п" => self.group_selection(),
                    _ => {}
                }
            }
        }
        // FR-049: Ctrl+T — тогл галереи схем (кириллическая «е» — та же
        // физическая клавиша; во время редактирования не доходим — там
        // нет Ctrl+T-команды редактора)
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("t") || c == "е" || c == "Е")
        {
            if self.scheme_gallery.open {
                self.scheme_gallery.close();
            } else {
                self.scheme_gallery.open();
                self.empty_state_dismissed = false;
            }
            self.request_redraw();
            return;
        }
        // Ctrl+, — toggle панели настроек (кириллическая «б» — та же клавиша;
        // во время редактирования сюда не доходим — там Ctrl+Б это Bold)
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c) if c == "," || c == "б" || c == "Б")
        {
            self.settings_open = !self.settings_open;
            self.settings_dropdown.reset();
            self.request_redraw();
            return;
        }
        // FR-016 (CP5): Ctrl+B — toggle оверлея узких мест (кириллическая
        // «и» — та же физическая клавиша; во время редактирования сюда не
        // доходим — там Ctrl+Б это Bold, конфликт решён комментарием выше).
        // Персистентная настройка — сохраняем конфиг сразу.
        if event.state == ElementState::Pressed
            && !event.repeat
            && self.modifiers.control_key()
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("b") || c == "и" || c == "И")
        {
            self.toggle_bottleneck_overlay();
            self.save_settings();
            return;
        }
        if event.logical_key == Key::Named(NamedKey::Space) && !event.repeat {
            self.space_pressed = event.state == ElementState::Pressed;
            self.sync_cursor_icon();
            if !self.space_pressed {
                // Отпускание Space во время drag не должно оставлять ноду "прилипшей"
                // FR-006: применённое движение — undo-шаг; далее drag прерывается
                self.finish_interaction_undo();
                // FR-038 (п.9): drag прерван — предпросмотр направляющих гасится
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.clear_guides();
                }
                self.dragging = None;
            }
        }
        // T23 (brainstorm-focus): F (русская раскладка — «А») — переключить
        // режим фокуса связей. Конфликтов нет: Ctrl+F — поиск (обработан
        // выше с модификатором), F3 — HUD/цикл поиска (функциональная клавиша)
        if event.state == ElementState::Pressed
            && !event.repeat
            && matches!(&event.logical_key, Key::Character(c)
                if c.eq_ignore_ascii_case("f") || c == "а" || c == "А")
        {
            self.toggle_focus_mode();
            return;
        }
        // F3 — цикл по результатам поиска (T14), если они есть (в т.ч. после
        // закрытия панели — rows сохранены); иначе — HUD с fps/p95 (T5)
        if event.logical_key == Key::Named(NamedKey::F3)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            if !self.search.rows.is_empty() {
                self.cycle_search(if self.modifiers.shift_key() { -1 } else { 1 });
            } else {
                self.hud_visible = !self.hud_visible;
                self.request_redraw();
            }
        }
        // Del — удалить выделенную ноду (каскадно со связями) или связь (T8).
        // Во время редактирования сюда не доходим — там Delete работает в тексте
        if event.logical_key == Key::Named(NamedKey::Delete)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.delete_selected();
        }
        // FR-038 (п.22): клавиатурный nudge выделения — стрелки 1 px,
        // Shift+стрелки 10 px. Глобальный контур (ВНЕ оверлея поиска —
        // стрелки панели уходят в on_search_key с return выше): редактор/
        // поиск/диалог/настройки-меню тоже возвращают раньше. Ctrl не трогаем
        // — Ctrl+←/→ заняты mindmap-сворачиванием (ветка ниже). Повторы
        // клавиши НЕ глотаются: удержание двигает ноду, каждый шаг — своя
        // undo-операция (п.22 «каждый шаг в undo»)
        if event.state == ElementState::Pressed && !self.modifiers.control_key() {
            if let Key::Named(named) = &event.logical_key {
                let dir = match named {
                    NamedKey::ArrowUp => Some([0.0, -1.0]),
                    NamedKey::ArrowDown => Some([0.0, 1.0]),
                    NamedKey::ArrowLeft => Some([-1.0, 0.0]),
                    NamedKey::ArrowRight => Some([1.0, 0.0]),
                    _ => None,
                };
                if let Some(dir) = dir {
                    let screen_px = if self.modifiers.shift_key() {
                        10.0
                    } else {
                        1.0
                    };
                    self.nudge_selection(dir, screen_px);
                }
            }
        }
        // FR-011: mindmap-ветвление — Tab (дочерняя), Enter (сиблинг),
        // Ctrl+← (свернуть ветку), Ctrl+→ (развернуть). Только при выделенной
        // text-ноде; редактор/поиск/диалог приглушают канвас-хоткеи (return
        // выше — клавиатура уходит туда)
        if event.state == ElementState::Pressed && !event.repeat && !self.modifiers.shift_key() {
            let selected_index = match self.selected {
                Some(Selection::Node(index)) => Some(index),
                _ => None,
            };
            if let Some(index) = selected_index {
                let is_text = self
                    .scene
                    .canvas
                    .nodes
                    .get(index)
                    .is_some_and(|node| node.kind() == NodeKind::Text);
                if is_text {
                    let ctrl = self.modifiers.control_key();
                    match &event.logical_key {
                        Key::Named(NamedKey::Tab) => self.mindmap_add_child(index),
                        Key::Named(NamedKey::Enter) if !ctrl => self.mindmap_add_sibling(index),
                        Key::Named(NamedKey::ArrowLeft) if ctrl => {
                            self.mindmap_set_collapsed(index, true);
                        }
                        Key::Named(NamedKey::ArrowRight) if ctrl => {
                            self.mindmap_set_collapsed(index, false);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// FR-042 (E3) + FR-044: кадр main stage — паритет с прототипом
    /// prototype-mainstage-anatomy.html (drawStage). Затемнение фона,
    /// подложка, заголовок «Пучок: A → B · ×N» с кнопкой ✕, веер рёбер
    /// с точками портов, ПОЛНЫЕ карточки среза (заголовок, построчные
    /// результаты, полоса результата), пилюли подписей лейн-стопкой в
    /// коридоре между колонками ([`canvas_core::bundles::stage_fan_label_layout`]).
    /// Возвращает (квады, screen-тексты) — рендерер выводит их модальным
    /// проходом после всего живого контента (инвариант 8: модальность).
    fn stage_frame(
        &self,
        viewport: [f32; 2],
        stage: &MainStageState,
        layout: StageLayout,
    ) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let palette = ThemeColors::from_theme(self.settings.theme);
        let rect = main_stage_rect(viewport);
        let transform = StageTransform::new([rect.x, rect.y], layout.scale);
        let camera = &self.camera;
        let zoom = camera.zoom();
        let s = layout.scale.max(f32::EPSILON);
        let mut quads: Vec<CardInstance> = Vec::new();
        let mut texts: Vec<OwnedScreenText> = Vec::new();
        // Шрифт stage-текста: базовый размер * масштаб раскладки (геометрия
        // сжата тем же коэффициентом — карточка и её текст сжимаются вместе);
        // минимум 8 px — читаемость деградационных режимов.
        let font = |px: f32| (px * s).max(8.0);
        // FR-044 (владелец, 2026-09-22): геометрия веера — якоря рёбер на
        // строках значений (from_line/to_param → строка карточки), группы
        // одинаковых якорей расходятся ±12 px. Метрики — константы рендера,
        // чтобы геометрия и отрисовка строк не разъезжались.
        let metrics = StageMetrics {
            header_h: HEADER_HEIGHT,
            body_top_gap: BODY_TOP_GAP,
            body_line: BODY_LINE_HEIGHT,
            result_line: RESULT_LINE_HEIGHT,
            body_padding: BODY_PADDING,
            strip_extra: 6.0,
        };
        let footers = [
            self.scene
                .expr_results
                .contains_key(&stage.slice.nodes[0].id),
            self.scene
                .expr_results
                .contains_key(&stage.slice.nodes[1].id),
        ];
        let lines = stage_edge_geometry(&stage.slice, &metrics, footers, 24);
        // 1) Затемнение фона (§7.5: тёмная 0.6 / светлая 0.5) — весь вьюпорт
        quads.push(CardInstance {
            pos: camera.screen_to_world([0.0, 0.0], viewport),
            size: [viewport[0] / zoom, viewport[1] / zoom],
            fill: palette.stage_dim,
            border: [0.0; 4],
            params: [0.0, 0.0, 0.0, 1.0],
        });
        // 2) Подложка и рамка stage — стиль модалок FR-039 (радиус 14)
        quads.push(CardInstance {
            pos: camera.screen_to_world([rect.x, rect.y], viewport),
            size: [rect.w / zoom, rect.h / zoom],
            fill: palette.menu_fill,
            border: palette.palette_border,
            params: [14.0 / zoom, 0.0, 0.0, 1.0],
        });
        // 3) Заголовок «Пучок: A → B · ×N», подсказка Esc и кнопка ✕
        let from_title = title_for(&stage.slice.nodes[0]);
        let to_title = title_for(&stage.slice.nodes[1]);
        texts.push(OwnedScreenText {
            text: self.trf(
                keys::STAGE_BUNDLE_TITLE,
                &[
                    ("from", from_title.as_str()),
                    ("to", to_title.as_str()),
                    ("n", stage.slice.edges.len().to_string().as_str()),
                ],
            ),
            origin: transform.map_point([16.0, 12.0]),
            width: transform.map_size(rect.w) - 200.0,
            font_size: font(13.0),
            color: palette.title,
            align: TextAlign::Left,
        });
        // Кнопка ✕ — правый верхний угол (rect пересчитывается в клике —
        // та же формула, состояния не требует)
        let close = [rect.x + rect.w - 36.0, rect.y + 12.0, 24.0, 24.0];
        texts.push(OwnedScreenText {
            text: self.tr(keys::STAGE_HINT).to_owned(),
            origin: [close[0] - 160.0, close[1] + 5.0],
            width: 152.0,
            font_size: font(11.0),
            color: palette.quote,
            align: TextAlign::Left,
        });
        quads.push(CardInstance {
            pos: camera.screen_to_world([close[0], close[1]], viewport),
            size: [close[2] / zoom, close[3] / zoom],
            fill: [0.0; 4],
            border: palette.palette_border,
            params: [7.0 / zoom, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: "×".to_owned(),
            origin: [close[0], close[1] + 3.0],
            width: close[2],
            font_size: font(12.0),
            color: palette.body,
            align: TextAlign::Center,
        });
        // 4) Рёбра среза веером (stage-локальные px → мир); выделение
        // ребра среза — live-индекс из Selection. Геометрия — те же линии,
        // что у точек портов и hit-test'а (единый источник)
        let selected_slice = self.selected.and_then(|sel| match sel {
            Selection::Edge(live) => stage.edges.iter().position(|&e| e == live),
            Selection::Node(_) => None,
        });
        for inst in build_stage_edge_instances(
            &stage.slice,
            &lines,
            stage.slice.edges.len(),
            selected_slice,
            None,
        ) {
            quads.push(transform.instance_to_world(&inst, camera, viewport));
        }
        // 5) Точки портов на концах веера (аффорданс входа/выхода, прототип):
        // цвет — класс потока ребра, выделенное ребро — акцент; точки сидят
        // на строках значений (якоря линий)
        for (i, edge) in stage.slice.edges.iter().enumerate() {
            let Some(line) = lines.get(i) else {
                continue;
            };
            let fill = if selected_slice == Some(i) {
                SELECTION_BORDER
            } else if edge.flow_kind() == FlowKind::Value {
                FLOW_EDGE_COLOR
            } else {
                EDGE_COLOR
            };
            for p in [line.from, line.to] {
                let d = 7.0;
                quads.push(transform.instance_to_world(
                    &CardInstance {
                        pos: [p[0] - d / 2.0, p[1] - d / 2.0],
                        size: [d, d],
                        fill,
                        border: [0.0; 4],
                        params: [d / 2.0, 0.0, 0.0, 1.0],
                    },
                    camera,
                    viewport,
                ));
            }
        }
        // 6) Карточки среза: карточка + полоса категории шаблона (анатомия
        // A/B, PRD-0004); выделенная — рамка выделения
        let is_selected = |node: &Node| {
            self.scene
                .canvas
                .nodes
                .iter()
                .position(|n| n.id == node.id)
                .is_some_and(|idx| {
                    self.selected == Some(Selection::Node(idx))
                        || self.selected_nodes.contains(&idx)
                })
        };
        for node in stage.slice.nodes.iter() {
            quads.push(transform.instance_to_world(
                &card_instance(node, is_selected(node), &palette),
                camera,
                viewport,
            ));
            if let Some(band) = template_band_instance(node) {
                quads.push(transform.instance_to_world(&band, camera, viewport));
            }
        }
        // 7) Контент нод среза (анатомия C/D, PRD-0004): заголовок,
        // построчные результаты Numi-листа, полоса результата в футере —
        // screen-space тексты константного размера (стиль модальностей
        // FR-039), позиции — через transform (scale применён один раз)
        for node in stage.slice.nodes.iter() {
            let title = title_for(node);
            if !title.is_empty() {
                texts.push(OwnedScreenText {
                    text: title,
                    origin: transform.map_point([node.x + 12.0, node.y + 8.0]),
                    width: transform.map_size(node.width) - 24.0,
                    font_size: font(13.0),
                    color: palette.title,
                    align: TextAlign::Left,
                });
            }
            if node.kind() != NodeKind::Text {
                continue;
            }
            let Some(text) = node.text.as_deref() else {
                continue;
            };
            let results = self.scene.expr_line_results.get(&node.id);
            let has_footer = self.scene.expr_results.contains_key(&node.id);
            // Вертикали строк — ритм живой карточки (BODY_LINE_HEIGHT от
            // шапки, инвариант вертикали FR-025); строки сверх высоты тела
            // (минус футер результата) не рисуются
            let body_top = node.y + HEADER_HEIGHT + BODY_TOP_GAP;
            let footer_h = if has_footer {
                RESULT_LINE_HEIGHT + 6.0
            } else {
                0.0
            };
            let avail_h =
                (node.height - HEADER_HEIGHT - BODY_TOP_GAP - BODY_PADDING - footer_h).max(0.0);
            let max_rows = ((avail_h / BODY_LINE_HEIGHT).floor() as usize).max(1);
            // Оценка ширины моно-строки: advance ≈ 0.6·font (stage-локальные px)
            let char_w = 7.2f32;
            for (li, line) in text.lines().enumerate().take(max_rows) {
                let row_y = body_top
                    + li as f32 * BODY_LINE_HEIGHT
                    + (BODY_LINE_HEIGHT - RESULT_LINE_HEIGHT) / 2.0;
                let outcome = results.and_then(|l| l.get(li)).and_then(|o| o.as_ref());
                let (value_text, is_err) = match outcome {
                    Some(ExprOutcome::Ok(value)) => (value.to_string(), false),
                    Some(ExprOutcome::Err(_)) => ("!".to_owned(), true),
                    None => (String::new(), false),
                };
                let value_w = value_text.chars().count() as f32 * char_w;
                // Левая колонка — исходная строка; усечение под зазор до
                // колонки значения (прототип: truncate до ширины карточки)
                let fit =
                    ((node.width - BODY_PADDING * 2.0 - value_w - 14.0) / char_w).max(3.0) as usize;
                let color = if is_err {
                    palette.error
                } else if outcome.is_some() {
                    palette.body
                } else {
                    palette.quote
                };
                texts.push(OwnedScreenText {
                    text: truncate_chars(line.trim_end(), fit),
                    origin: transform.map_point([node.x + BODY_PADDING, row_y + 2.0]),
                    width: transform.map_size(node.width - BODY_PADDING * 2.0),
                    font_size: font(12.0),
                    color,
                    align: TextAlign::Left,
                });
                if !value_text.is_empty() {
                    let vw = value_text.chars().count() as f32 * char_w;
                    texts.push(OwnedScreenText {
                        text: value_text,
                        origin: transform
                            .map_point([node.x + node.width - BODY_PADDING - vw, row_y + 2.0]),
                        width: transform.map_size(vw) + 24.0,
                        font_size: font(12.0),
                        color: if is_err { palette.error } else { palette.body },
                        align: TextAlign::Left,
                    });
                }
            }
            // Полоса результата (D): узловое значение в футере карточки
            if let Some(outcome) = self.scene.expr_results.get(&node.id) {
                let (value_text, color) = match outcome {
                    ExprOutcome::Ok(value) => (format!("= {value}"), palette.body),
                    ExprOutcome::Err(msg) => {
                        (truncate_chars(&format!("! {msg}"), 40), palette.error)
                    }
                };
                let vw = value_text.chars().count() as f32 * char_w;
                let strip_h = RESULT_LINE_HEIGHT + 6.0;
                let strip_y = node.y + node.height - BODY_PADDING - strip_h;
                quads.push(transform.instance_to_world(
                    &CardInstance {
                        pos: [node.x + 6.0, strip_y],
                        size: [(node.width - 12.0).max(0.0), strip_h],
                        fill: palette.search_row_fill,
                        border: [0.0; 4],
                        params: [6.0, 0.0, 0.0, 1.0],
                    },
                    camera,
                    viewport,
                ));
                // Порт выхода (FR-025) — кружок value-цвета у правого края
                let d = 7.0;
                quads.push(transform.instance_to_world(
                    &CardInstance {
                        pos: [
                            node.x + node.width - d - 4.0,
                            strip_y + strip_h / 2.0 - d / 2.0,
                        ],
                        size: [d, d],
                        fill: FLOW_EDGE_COLOR,
                        border: [0.0; 4],
                        params: [d / 2.0, 0.0, 0.0, 1.0],
                    },
                    camera,
                    viewport,
                ));
                let vw = vw.min(node.width - BODY_PADDING * 2.0 - 14.0).max(0.0);
                texts.push(OwnedScreenText {
                    text: value_text,
                    origin: transform.map_point([
                        node.x + node.width - BODY_PADDING - d - 6.0 - vw,
                        strip_y + 3.0,
                    ]),
                    width: transform.map_size(vw) + 24.0,
                    font_size: font(12.0),
                    color,
                    align: TextAlign::Left,
                });
            }
        }
        // 7b) Подписи на концах рёбер (FR-044, владелец 2026-09-22: «на
        // концах edge — подписи значений», прототип R5/R6 — порты с
        // подложкой): у истока — ЗНАЧЕНИЕ ребра (своей строки), у приёмника
        // — квалифицированный адрес «Объект · строка N / Объект.output».
        // Подложка — цвет подложки stage (меню): подписи не сливаются с
        // линиями веера (прототип R6: подложка от рёбер). Оценка ширины —
        // advance ≈ 0.6·шрифта (как у строк карточки).
        let src_title = title_for(&stage.slice.nodes[0]);
        let mut src_label_max = 0.0_f32;
        let mut dst_label_max = 0.0_f32;
        for (i, edge) in stage.slice.edges.iter().enumerate() {
            let Some(line) = lines.get(i) else {
                continue;
            };
            // Исток: значение строки/ноды (подпись значения)
            let value = truncate_chars(&self.stage_edge_value_text(edge), 24);
            if !value.is_empty() {
                let w = value.chars().count() as f32 * 6.3 + 12.0;
                src_label_max = src_label_max.max(w);
                let bx = line.from[0] + 10.0;
                let by = line.from[1] - 9.0;
                quads.push(transform.instance_to_world(
                    &CardInstance {
                        pos: [bx, by],
                        size: [w, 18.0],
                        fill: palette.menu_fill,
                        border: [0.0; 4],
                        params: [4.0, 0.0, 0.0, 1.0],
                    },
                    camera,
                    viewport,
                ));
                texts.push(OwnedScreenText {
                    text: value,
                    origin: transform.map_point([bx + 6.0, by + 13.0]),
                    width: transform.map_size(w),
                    font_size: font(10.5),
                    color: palette.body,
                    align: TextAlign::Left,
                });
            }
            // Приёмник: квалифицированный адрес истока (Объект.Поле)
            let qualified = if let Some(line_no) = edge.from_line {
                format!(
                    "{src_title} · {}",
                    i18n::trf(
                        self.settings.language,
                        keys::STAGE_LINE_LABEL,
                        &[("n", &(line_no + 1).to_string())],
                    )
                )
            } else if let Some(output) = edge.from_output.as_deref() {
                format!("{src_title}.{output}")
            } else {
                src_title.clone()
            };
            let qualified = truncate_chars(&qualified, 26);
            if !qualified.is_empty() {
                let w = qualified.chars().count() as f32 * 6.3 + 12.0;
                dst_label_max = dst_label_max.max(w);
                let bx = line.to[0] - 10.0 - w;
                let by = line.to[1] - 9.0;
                quads.push(transform.instance_to_world(
                    &CardInstance {
                        pos: [bx, by],
                        size: [w, 18.0],
                        fill: palette.menu_fill,
                        border: [0.0; 4],
                        params: [4.0, 0.0, 0.0, 1.0],
                    },
                    camera,
                    viewport,
                ));
                texts.push(OwnedScreenText {
                    text: qualified,
                    origin: transform.map_point([bx + 6.0, by + 13.0]),
                    width: transform.map_size(w),
                    font_size: font(10.5),
                    color: palette.edge_label,
                    align: TextAlign::Left,
                });
            }
        }
        // 8) Пилюли подписей веера (FR-044 Р-1): адресация + значение,
        // лейн-стопка в коридоре между колонками (stage_fan_label_layout:
        // без пересечений, кламп в зону; порядок — по вертикали середин;
        // пилюля тянется к середине СВОЕЙ линии — preferred_x)
        let mut pills_in: Vec<(usize, f32, f32, Option<f32>)> = Vec::new();
        let mut mids: Vec<[f32; 2]> = Vec::new();
        for (i, _edge) in stage.slice.edges.iter().enumerate() {
            let Some(line) = lines.get(i) else {
                continue;
            };
            mids.push(line.mid);
            pills_in.push((i, 0.0, 34.0, Some(line.mid[0]))); // ширина после сортировки
        }
        // Сортировка по вертикали середин (прототип R7: стопка следует
        // геометрии веера) с сохранением индекса ребра
        let mut order: Vec<usize> = (0..mids.len()).collect();
        order.sort_by(|&a, &b| mids[a][1].total_cmp(&mids[b][1]));
        let sorted: Vec<(usize, f32, f32, Option<f32>)> = order
            .iter()
            .map(|&oi| {
                let (item, _, h, pref) = pills_in[oi];
                let addr = truncate_chars(&self.stage_edge_addr_text(&stage.slice.edges[item]), 42);
                let value =
                    truncate_chars(&self.stage_edge_value_text(&stage.slice.edges[item]), 42);
                let w =
                    (addr.chars().count().max(value.chars().count()) as f32 * 7.2 + 24.0).max(56.0);
                (item, w, h, pref)
            })
            .collect();
        // Зона и коридор — stage-локальные px (заголовок сверху, подсказка
        // снизу; коридор — между колонками нод, pad прототипа 70 px),
        // дополнительно сужен на зоны подписей концов рёбер (7b): пилюли
        // не наезжают на подписи значений/адресов (владелец 2026-09-22:
        // «тултипы не залезают на edge и подписи»)
        let zone = StageLocalRect {
            x: 0.0,
            y: 56.0 / s,
            w: rect.w / s,
            h: ((rect.h - 56.0 - 46.0) / s).max(0.0),
        };
        let src = &stage.slice.nodes[0];
        let dst = &stage.slice.nodes[1];
        let src_labels = StageLocalRect {
            x: src.x,
            y: src.y,
            w: src.width,
            h: src.height,
        };
        let dst_labels = StageLocalRect {
            x: dst.x,
            y: dst.y,
            w: dst.width,
            h: dst.height,
        };
        let mut corridor = fan_corridor(src_labels, dst_labels, 70.0);
        let left_needed = src.x + src.width + 10.0 + src_label_max + 6.0;
        let right_limit = dst.x - 10.0 - dst_label_max - 6.0;
        if left_needed > corridor.x {
            let d = left_needed - corridor.x;
            corridor.x += d;
            corridor.w -= d;
        }
        let over = corridor.x + corridor.w - right_limit;
        if over > 0.0 {
            corridor.w -= over;
        }
        corridor.w = corridor.w.max(0.0);
        let axis_y = zone.y + zone.h / 2.0;
        let laid = stage_fan_label_layout(sorted, corridor, zone, axis_y);
        for pill in &laid.pills {
            let edge = &stage.slice.edges[pill.item];
            let addr = truncate_chars(&self.stage_edge_addr_text(edge), 42);
            let value = truncate_chars(&self.stage_edge_value_text(edge), 42);
            let sel = selected_slice == Some(pill.item);
            quads.push(transform.instance_to_world(
                &CardInstance {
                    pos: [pill.rect.x, pill.rect.y],
                    size: [pill.rect.w, pill.rect.h],
                    fill: palette.edge_label_fill,
                    border: if sel { SELECTION_BORDER } else { [0.0; 4] },
                    params: [9.0, 0.0, 0.0, 1.0],
                },
                camera,
                viewport,
            ));
            texts.push(OwnedScreenText {
                text: addr,
                origin: transform.map_point([pill.rect.x + 12.0, pill.rect.y + 5.0]),
                width: transform.map_size(pill.rect.w - 16.0),
                font_size: font(12.0),
                color: palette.title,
                align: TextAlign::Left,
            });
            if !value.is_empty() {
                texts.push(OwnedScreenText {
                    text: value,
                    origin: transform.map_point([pill.rect.x + 12.0, pill.rect.y + 18.0]),
                    width: transform.map_size(pill.rect.w - 16.0),
                    font_size: font(11.0),
                    color: palette.edge_label,
                    align: TextAlign::Left,
                });
            }
        }
        // 9) Подсказка внизу stage (i18n, §7.2)
        texts.push(OwnedScreenText {
            text: self.tr(keys::STAGE_FOOT_HINT).to_owned(),
            origin: [rect.x + 40.0, rect.y + rect.h - 26.0],
            width: rect.w - 80.0,
            font_size: font(11.5),
            color: palette.quote,
            align: TextAlign::Center,
        });
        (quads, texts)
    }

    /// FR-042 (E3): адресная часть подписи ребра в stage — адресация истока
    /// (fromLine «строка N» / fromOutput, FR-025/FR-029) и параметр-приёмник
    /// (toParam). Значение — отдельно, второй строкой пилюли (FR-044).
    fn stage_edge_addr_text(&self, edge: &Edge) -> String {
        let language = self.settings.language;
        let mut parts: Vec<String> = Vec::new();
        if let Some(line) = edge.from_line {
            parts.push(i18n::trf(
                language,
                keys::STAGE_LINE_LABEL,
                &[("n", &(line + 1).to_string())],
            ));
        } else if let Some(output) = edge.from_output.as_deref() {
            parts.push(output.to_owned());
        }
        if let Some(param) = edge.to_param.as_deref() {
            parts.push(i18n::trf(
                language,
                keys::STAGE_PARAM_LABEL,
                &[("param", param)],
            ));
        }
        if parts.is_empty() {
            parts.push("—".to_owned());
        }
        parts.join(" · ")
    }

    /// FR-042 (E3): значение ребра в stage (FR-014/FR-025; для fromLine —
    /// построчный результат Numi-листа источника).
    fn stage_edge_value_text(&self, edge: &Edge) -> String {
        match edge.from_line {
            Some(line) => self
                .scene
                .expr_line_results
                .get(&edge.from_node)
                .and_then(|lines| lines.get(line))
                .and_then(|outcome| outcome.as_ref())
                .map(|outcome| match outcome {
                    ExprOutcome::Ok(value) => value.to_string(),
                    ExprOutcome::Err(msg) => msg.clone(),
                })
                .unwrap_or_default(),
            None => match self.scene.expr_results.get(&edge.from_node) {
                Some(ExprOutcome::Ok(value)) => value.to_string(),
                Some(ExprOutcome::Err(msg)) => msg.clone(),
                None => String::new(),
            },
        }
    }

    /// FR-042 (E3): открыть main stage, если ребро — часть пучка веса ≥ 2
    /// и агрегация включена (F-13). true — stage открыт (клик поглощён);
    /// false — одиночное ребро/агрегация выключена (поведение прежнее).
    fn try_open_main_stage(&mut self, edge_index: usize) -> bool {
        if !self.settings.edge_aggregation {
            return false;
        }
        match MainStageState::open(&self.scene.canvas, &self.scene.bundles, edge_index) {
            Some(stage) => {
                self.main_stage = Some(stage);
                // PRD-0007 (F-10, У10): stage и окно проверки взаимо-
                // исключительны; снапшот остаётся в сессионном кэше —
                // возврат через «?» мгновенный (≤ 1 с)
                self.close_explain();
                self.bundle_hover = None;
                self.request_redraw();
                true
            }
            None => false,
        }
    }

    /// FR-042 (E3): закрыть main stage (Esc/клик по фону/перед открытием
    /// оверлея — Q6). Выделение ребра сохраняется (AC-3.2).
    fn close_main_stage(&mut self) {
        if self.main_stage.take().is_some() {
            self.request_redraw();
        }
    }

    /// FR-052 (U2): диспетчер кликов экрана — `HitStack::pick` решил, что
    /// точка принадлежит поверхности `surface` (элемент `element`).
    /// Тела обработчиков — дословный перенос прежних веток `on_left_button`
    /// (PRD-0009 §13 U2: реестр — единственный диспетчер). `false` —
    /// поверхность не поглотила (клик продолжает путь в канвас).
    fn dispatch_surface_click(&mut self, surface: &str, element: &str) -> bool {
        match surface {
            ui_registry::id::GALLERY => {
                self.on_gallery_click();
                self.request_redraw();
                true
            }
            ui_registry::id::ONBOARDING => {
                self.click_onboarding();
                true
            }
            ui_registry::id::EMPTY => {
                self.click_empty_state();
                true
            }
            ui_registry::id::STAGE => {
                self.click_main_stage();
                true
            }
            ui_registry::id::SEARCH => {
                self.click_search();
                true
            }
            ui_registry::id::WHEEL => {
                self.click_wheel_menu();
                true
            }
            ui_registry::id::TEMPLATE_PANEL => {
                self.click_template_panel();
                true
            }
            ui_registry::id::TEMPLATE_STRIP => {
                self.click_template_strip();
                true
            }
            ui_registry::id::HELP_MENU => {
                self.click_help_menu();
                true
            }
            ui_registry::id::DOCS => {
                self.click_docs();
                true
            }
            ui_registry::id::WHATIF => self.whatif_bar_click(),
            ui_registry::id::CORNER_BUTTONS => self.click_corner_button(element),
            ui_registry::id::SETTINGS => {
                self.click_settings();
                true
            }
            ui_registry::id::HOTKEYS => {
                self.click_hotkeys_panel();
                true
            }
            ui_registry::id::MINIMAP => {
                self.click_minimap();
                true
            }
            ui_registry::id::EXPLAIN => {
                // PRD-0007 (X2): ✕/чип/крошки/узлы; внутри окна мимо
                // элементов — глотается (канвас клик не получает)
                self.on_explain_click();
                true
            }
            ui_registry::id::DIALOG => {
                self.click_dialog();
                true
            }
            ui_registry::id::PALETTE => self.click_palette(),
            ui_registry::id::MENU => {
                self.click_context_menu();
                true
            }
            // FR-050 Н2 (этап C): клик по панели меню выбора — пункт
            // выполняет действие; заголовок/паддинг — отмена («мимо
            // пункта», как клик мимо панели — backdrop)
            ui_registry::id::CHOICE_MENU => {
                self.click_choice_menu();
                true
            }
            _ => false,
        }
    }

    /// FR-052 (U2): контракт «клик мимо Block-поверхности» (backdrop).
    /// gallery/search/settings/docs/help/stage/menu — закрыть и глотнуть;
    /// onboarding/dialog — глотнуть без закрытия (явный выбор); wheel —
    /// near/far логика внутри обработчика.
    fn dispatch_surface_backdrop(&mut self, surface: &str) -> bool {
        match surface {
            ui_registry::id::GALLERY => {
                self.scheme_gallery.close();
                self.request_redraw();
                true
            }
            ui_registry::id::ONBOARDING => {
                self.request_redraw();
                true
            }
            ui_registry::id::STAGE => {
                self.main_stage = None;
                self.request_redraw();
                true
            }
            ui_registry::id::SEARCH => {
                self.search.close();
                self.request_redraw();
                true
            }
            ui_registry::id::SETTINGS => {
                // Двухэтапный dismiss (9533–9541 / 9592–9597): открытое меню —
                // закрывается только оно, модалка остаётся; иначе — модалка.
                if self.settings_dropdown.is_open() {
                    self.settings_dropdown.reset();
                } else {
                    self.settings_open = false;
                }
                self.request_redraw();
                true
            }
            ui_registry::id::MENU => {
                self.menu = None;
                self.request_redraw();
                true
            }
            // FR-050 Н2 (этап C): клик мимо панели меню выбора — отмена
            // («либо отмена» в постановке): закрыть и глотнуть
            ui_registry::id::CHOICE_MENU => {
                self.choice_menu = None;
                self.request_redraw();
                true
            }
            ui_registry::id::HELP_MENU => {
                self.help_menu = None;
                self.request_redraw();
                true
            }
            ui_registry::id::DOCS => {
                self.docs = None;
                self.request_redraw();
                true
            }
            ui_registry::id::DIALOG => {
                self.request_redraw();
                true
            }
            ui_registry::id::EXPLAIN => {
                // Клик по фону (мимо окна) — закрытие (on_explain_click X2)
                self.close_explain();
                true
            }
            ui_registry::id::WHEEL => {
                self.click_wheel_menu();
                true
            }
            _ => false,
        }
    }

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

    /// Онбординг: кнопки карточки, остальное глотается.
    fn click_onboarding(&mut self) {
        // FR-028: онбординг открыт — модальный оверлей: клики по
        // кнопкам карточки, остальное глотается (канвас не
        // реагирует; выход виден всегда — «Пропустить» в углу)
        if let Some(state) = &self.onboarding {
            let viewport = self.viewport_logical();
            let card = onboarding_ui::card_rect(viewport, state.step, self.settings.language);
            match onboarding_ui::button_at(card, state, self.cursor) {
                Some(OnboardingButton::Next) => {
                    if state.is_last() {
                        // «Готово»: тур пройден — флаг + сохранение
                        self.complete_onboarding();
                    } else if onboarding_ui::ONBOARDING_STEPS
                        .get(state.step)
                        .and_then(|step| step.action_key)
                        .is_some()
                    {
                        // FR-049: CTA шага («Попробовать») — тур
                        // пройден, галерея схем открыта
                        self.complete_onboarding();
                        self.scheme_gallery.open();
                    } else if let Some(state) = self.onboarding.as_mut() {
                        state.next();
                    }
                }
                Some(OnboardingButton::Prev) => {
                    if let Some(state) = self.onboarding.as_mut() {
                        state.prev();
                    }
                }
                Some(OnboardingButton::Skip) => self.defer_onboarding(),
                None => {}
            }
            self.request_redraw();
        }
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

    /// Main stage: ✕/внутри — выделение ребра, мимо — закрыть.
    fn click_main_stage(&mut self) {
        // FR-042 (E3, F-8/F-9): модальность main stage — клик вне
        // rect закрывает (канвас клик не получает, инвариант 8:
        // нода не создаётся, выделение не сбрасывается); внутри —
        // выделение ребра среза (живой индекс), без правки (PoC —
        // просмотр и выделение, non-goals PRD).
        if self.main_stage.is_some() {
            let viewport = self.viewport_logical();
            let rect = main_stage_rect(viewport);
            // FR-044 (прототип): кнопка ✕ в правом верхнем углу —
            // закрытие stage; rect по той же формуле, что в рендере
            if point_in_rect(
                [rect.x + rect.w - 36.0, rect.y + 12.0, 24.0, 24.0],
                self.cursor,
            ) {
                self.close_main_stage();
                self.request_redraw();
                return;
            }
            if point_in_rect([rect.x, rect.y, rect.w, rect.h], self.cursor) {
                let stage = self.main_stage.as_ref().expect("stage открыт");
                let transform = StageTransform::new([rect.x, rect.y], stage.scale);
                let local = transform.unmap_point(self.cursor);
                // Допуск от толщины (F-5): max(EDGE_HIT_TOLERANCE, d/2 + 2).
                // Геометрия веера — та же чистая функция, что на кадре
                // (детерминизм: рендер и hit-test совпадают)
                let tolerance = (bundle_thickness(stage.slice.edges.len()) / 2.0 + 2.0)
                    .max(canvas_core::EDGE_HIT_TOLERANCE);
                let metrics = StageMetrics {
                    header_h: HEADER_HEIGHT,
                    body_top_gap: BODY_TOP_GAP,
                    body_line: BODY_LINE_HEIGHT,
                    result_line: RESULT_LINE_HEIGHT,
                    body_padding: BODY_PADDING,
                    strip_extra: 6.0,
                };
                let footers = [
                    self.scene
                        .expr_results
                        .contains_key(&stage.slice.nodes[0].id),
                    self.scene
                        .expr_results
                        .contains_key(&stage.slice.nodes[1].id),
                ];
                let lines = stage_edge_geometry(&stage.slice, &metrics, footers, 24);
                let hit = stage_edge_at_lines(&lines, local, tolerance)
                    .and_then(|slice_i| stage.live_edge(slice_i));
                // Заимствование stage закончено — можно мутировать
                if let Some(live) = hit {
                    self.selected = Some(Selection::Edge(live));
                }
            } else {
                self.main_stage = None;
            }
            self.request_redraw();
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

    /// Wheel-меню: сектор/хаб/near/far — polar-геометрия.
    fn click_wheel_menu(&mut self) {
        // FR-018: wheel-меню шаблонов — клики обрабатываются до
        // канваса (оверлей поверх всего). Сектор категории — выбор
        // категории (растут шаблонные кольца); сектор шаблона —
        // инстанциация в world-точку открытия; мимо секторов, но
        // рядом — глотаем, заметно дальше — закрыть.
        // Любой клик глотается — dismiss не создаёт заметку.
        if let Some(menu) = self.wheel_menu.clone() {
            let [vw, vh] = self.viewport_logical();
            let categories = self.templates.categories();
            let template_count = menu
                .category
                .as_deref()
                .map(|c| self.templates.by_category(c).len())
                .unwrap_or(0);
            let geo =
                template_ui::wheel_geometry(menu.screen, vw, vh, categories.len(), template_count);
            match geo.hit(self.cursor) {
                Some(WheelHit::Category(i)) => {
                    if let Some(menu_mut) = self.wheel_menu.as_mut() {
                        menu_mut.category = Some(categories[i].to_owned());
                    }
                }
                Some(WheelHit::Template(i)) => {
                    let category = menu.category.expect("категория выбрана");
                    let manifest = self.templates.by_category(&category)[i].clone();
                    let world = menu.world;
                    self.wheel_menu = None;
                    self.instantiate_template_at(&manifest, world);
                }
                None => {
                    // FR-022: клик по кнопке-хабу — «назад» (сброс
                    // категории) или «закрыть»; дальше — как раньше:
                    // рядом глотаем, заметно дальше — закрыть.
                    if geo.hub_hit(self.cursor) {
                        if let Some(menu_mut) = self.wheel_menu.as_mut() {
                            menu_mut.category = None;
                        }
                        if menu.category.is_none() {
                            self.wheel_menu = None;
                        }
                    } else {
                        let dx = self.cursor[0] - geo.center[0];
                        let dy = self.cursor[1] - geo.center[1];
                        let outside = (dx * dx + dy * dy).sqrt() > geo.extent + 12.0;
                        if outside {
                            self.wheel_menu = None;
                        }
                    }
                }
            }
            self.request_redraw();
        }
    }

    /// Док палитры: collapse/поиск/категории/строки.
    fn click_template_panel(&mut self) {
        let viewport = self.viewport_logical();
        let rows = template_panel_rows(&self.templates, &self.template_panel);
        let lay = template_panel_layout(
            viewport[0],
            viewport[1],
            &self.templates,
            &self.template_panel,
            &rows,
        );
        let mut handled = false;
        // Кнопка сворачивания дока («‹» в шапке)
        if point_in_rect(rect_xywh(lay.collapse_rect), self.cursor) {
            self.template_panel.close();
            self.persist_palette_dock();
            handled = true;
        }
        // Клик по полю поиска — клавиатурный фокус в панель
        if !handled && point_in_rect(rect_xywh(lay.input_rect), self.cursor) {
            self.template_panel.focus_search();
            handled = true;
        }
        if !handled {
            for (rect, name, _active) in &lay.category_rects {
                if point_in_rect(rect_xywh(*rect), self.cursor) {
                    self.template_panel.category =
                        if self.template_panel.category.as_deref() == Some(name) {
                            None
                        } else {
                            Some(name.clone())
                        };
                    self.template_panel.selected = 0;
                    self.template_panel.scroll_top = 0;
                    handled = true;
                    break;
                }
            }
        }
        if !handled {
            // FR-024: строки панели — секции (заголовки, клик
            // глотается) и карточки шаблонов (FR-025: нажатие
            // — кандидат в drag; вставка — на отпускании: клик —
            // в центр viewport, drag — в точку курсора)
            for (rect, row) in lay.row_rects.iter().zip(lay.rows.iter()) {
                if point_in_rect(rect_xywh(*rect), self.cursor) {
                    if let PanelRow::Template(index) = row {
                        self.template_drag = Some(template_ui::PanelDrag {
                            index: *index,
                            press: self.cursor,
                            active: false,
                        });
                    }
                    handled = true;
                    break;
                }
            }
        }
        if !handled && point_in_rect(rect_xywh(lay.panel_rect), self.cursor) {
            // Внутри дока, мимо элементов — глотаем
            handled = true;
        }
        if handled {
            self.request_redraw();
        }
    }

    /// Свёрнутая полоса + flyout: drag-кандидаты/пин/expand.
    fn click_template_strip(&mut self) {
        // FR-025 (ревизия): свёрнутая палитра — полоса категорий
        // по центру слева; hover/pin раскрывает flyout справа.
        // Клик по строке flyout — drag-кандидат (инстанциация на
        // отпускании); по строке категории — пин-переключение;
        // по шеврону или полосе мимо строк — развернуть док;
        // мимо полосы — закрыть flyout, клик уходит в канвас.
        let viewport = self.viewport_logical();
        let categories = self.template_category_names();
        let strip = template_ui::dock_strip_layout(&categories, viewport[1]);
        // Строка flyout: кандидат в drag (тот же пайплайн, что
        // и у развёрнутого дока — ghost + вставка на отпускании)
        let flyout_hit = self
            .template_flyout_geometry(viewport, &strip)
            .and_then(|fly| {
                self.template_hover.as_ref().and_then(|hover| {
                    hover.open.and_then(|cat| {
                        strip.rows.get(cat).and_then(|(_, name)| {
                            let items = self.templates.by_category(name);
                            fly.row_rects
                                .iter()
                                .enumerate()
                                .find(|(_, rect)| point_in_rect(**rect, self.cursor))
                                .and_then(|(v, _)| {
                                    items.get(fly.scroll_top + v).and_then(|m| {
                                        self.templates.list().iter().position(|lm| lm.id == m.id)
                                    })
                                })
                        })
                    })
                })
            });
        if let Some(index) = flyout_hit {
            self.template_drag = Some(template_ui::PanelDrag {
                index,
                press: self.cursor,
                active: false,
            });
            self.request_redraw();
            return;
        }
        if let Some(i) = strip
            .rows
            .iter()
            .position(|(rect, _)| point_in_rect(*rect, self.cursor))
        {
            // Пин-переключение flyout категории (WAI-ARIA)
            self.template_hover
                .get_or_insert_with(template_ui::StripHover::new)
                .toggle_trigger(i);
            self.request_redraw();
            return;
        }
        if point_in_rect(strip.rect, self.cursor) {
            // Шеврон или полоса мимо строк — развернуть док
            self.template_panel.expand();
            self.persist_palette_dock();
            self.template_hover = None;
            self.request_redraw();
            return;
        }
        // Мимо полосы: flyout закрывается, клик уходит в канвас
        self.template_hover = None;
    }

    /// Меню помощи: подменю первым, паддинг глотается.
    fn click_help_menu(&mut self) {
        // FR-027: меню помощи (кнопка «?») и просмотрщик
        // документации — поповеры поверх канваса: клики
        // обрабатываются до кнопок/панели настроек
        if let Some(menu) = self.help_menu.take() {
            let viewport = self.viewport_logical();
            // Подменю разделов — ПЕРВЫМ (колонка правее/левее меню):
            // выбор открывает просмотрщик, паддинг — глотается
            if menu.docs_open {
                let sub = docs_ui::help_submenu_origin(menu.origin, viewport);
                if let Some(page) = docs_ui::help_submenu_item_at(sub, self.cursor) {
                    self.open_docs_page(page);
                    self.request_redraw();
                    return;
                }
                if point_in_rect(docs_ui::help_submenu_rect(sub), self.cursor) {
                    self.help_menu = Some(menu);
                    self.request_redraw();
                    return;
                }
            }
            match docs_ui::help_menu_item_at(menu.origin, self.cursor) {
                // «Документация ▸» — тогл подменю (7 разделов)
                Some(docs_ui::HelpMenuItem::Docs) => {
                    self.help_menu = Some(HelpMenuState {
                        origin: menu.origin,
                        docs_open: !menu.docs_open,
                    });
                }
                // «Пройти онбординг» (FR-028): явное намерение —
                // счётчик откладываний не трогается
                Some(docs_ui::HelpMenuItem::Onboarding) => {
                    self.onboarding = Some(OnboardingState::default());
                }
                // «Галерея схем» (FR-049): открыть модальную галерею
                Some(docs_ui::HelpMenuItem::Schemes) => {
                    self.help_menu = None;
                    self.scheme_gallery.open();
                }
                None => {
                    // Поверхность меню (паддинг) — глотается, меню
                    // остаётся; мимо — закрыть (клик глотается,
                    // паттерн контекстного меню T7)
                    if point_in_rect(docs_ui::help_menu_rect(menu.origin), self.cursor) {
                        self.help_menu = Some(menu);
                    }
                }
            }
            self.request_redraw();
        }
    }

    /// Просмотрщик документации: ✕/ссылки/панель.
    fn click_docs(&mut self) {
        if self.docs.is_some() {
            let viewport = self.viewport_logical();
            let panel = docs_ui::viewer_rect(viewport);
            // × — закрыть
            if point_in_rect(docs_ui::viewer_close_rect(panel), self.cursor) {
                self.docs = None;
                self.request_redraw();
                return;
            }
            if point_in_rect(panel, self.cursor) {
                // Внутренняя ссылка — переход на страницу
                let content = docs_ui::viewer_content_rect(panel);
                let link = self.docs.as_ref().and_then(|viewer| {
                    docs_ui::link_at(
                        &viewer.layout,
                        viewer.scroll,
                        [content[0], content[1]],
                        self.cursor,
                    )
                    .cloned()
                });
                if let Some(docs_ui::LinkTarget::Page(id)) = link.map(|l| l.target) {
                    if let Some(page) = docs_ui::page_index_by_id(id) {
                        self.open_docs_page(page);
                        return;
                    }
                }
                // Клик по панели без ссылки — глотается
                self.request_redraw();
                return;
            }
            // Клик мимо панели — закрыть (клик глотается)
            self.docs = None;
            self.request_redraw();
        }
    }

    /// Модалка настроек: dropdown/навигация/карточки/строки.
    fn click_settings(&mut self) {
        // Панель настроек (screen-space): клики обрабатываются до
        // канваса — кнопка/панель поверх и «прозрачности» не дают
        let viewport = self.viewport_logical();
        // Кнопка переключения темы — рядом с кнопкой настроек
        if point_in_rect(
            theme_button_rect(self.settings.button_corner, viewport),
            self.cursor,
        ) {
            self.toggle_theme();
            self.request_redraw();
            return;
        }
        // FR-027: кнопка «?» — тогл меню помощи (как ⚙ у настроек)
        // Q6 FR-042: открытие оверлея закрывает main stage
        if point_in_rect(
            help_button_rect(self.settings.button_corner, viewport),
            self.cursor,
        ) {
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
            return;
        }
        if point_in_rect(
            button_rect(self.settings.button_corner, viewport),
            self.cursor,
        ) {
            // Q6 FR-042: открытие настроек закрывает main stage
            self.close_main_stage();
            self.settings_open = !self.settings_open;
            self.settings_dropdown.reset();
            self.request_redraw();
            return;
        }
        if self.settings_open {
            // FR-039: layout модалки — hit-тесты по навигации,
            // строкам и карточкам темы
            let layout = modal_layout(self.settings_tab, viewport);
            // Открытое выпадающее меню — первый приоритет: клик по
            // пункту применяет значение; клик мимо меню закрывает
            // ТОЛЬКО меню (модалка остаётся открытой — двухэтапный
            // dismiss), клик по другой строке обработается ниже
            if let Some(open_row) = self.settings_dropdown.open_row {
                let items = dropdown_options(open_row, &self.settings);
                let anchor = layout
                    .row_rect(open_row)
                    .map(|rect| control_rect(rect, RowKind::Dropdown))
                    .unwrap_or([0.0; 4]);
                let menu_rect = dropdown_layout(anchor, viewport, items.len(), anchor[2]);
                if point_in_rect(menu_rect, self.cursor) {
                    if let Some(index) = dropdown_item_at(menu_rect, items.len(), self.cursor) {
                        self.apply_dropdown_choice(open_row, index);
                    }
                    self.settings_dropdown.reset();
                    self.request_redraw();
                    return;
                }
                self.settings_dropdown.reset();
                if modal_row_at(&layout, self.cursor).is_none()
                    && !point_in_rect(layout.rect, self.cursor)
                {
                    // Клик вне меню, не по строке и не по модалке:
                    // меню закрыто, канвасу клик не достаётся
                    // (иначе создал бы заметку)
                    self.request_redraw();
                    return;
                }
            }
            // Пункт левой навигации — смена таба (+ сброс dropdown);
            // активный таб переживает закрытие модалки (в памяти App)
            if let Some(tab) = modal_nav_at(&layout, self.cursor) {
                self.settings_tab = tab;
                self.settings_dropdown.reset();
                self.request_redraw();
                return;
            }
            // Карточки темы (таб «Внешний вид») — прямой выбор
            // классики; клик по карточке сбрасывает пресет (FR-047:
            // карточки и пресет — взаимоисключающие источники темы)
            if let Some(theme) = modal_theme_card_at(&layout, self.cursor) {
                if self.settings.theme != theme || !self.settings.theme_preset.is_empty() {
                    self.settings.theme = theme;
                    self.settings.theme_preset.clear();
                    self.widgets.set_theme(theme == Theme::Dark);
                    if let Some(renderer) = self.renderer.as_mut() {
                        renderer.set_theme(ThemeColors::from_theme(theme));
                    }
                    self.save_settings();
                }
                self.request_redraw();
                return;
            }
            if let Some(row) = modal_row_at(&layout, self.cursor) {
                match row_kind(row) {
                    RowKind::Toggle => self.apply_toggle_row(row),
                    RowKind::Dropdown => {
                        // Клик по dropdown-строке открывает меню
                        // значений (НЕ меняет значение); повторный
                        // клик по той же строке закрывает
                        if self.settings_dropdown.open_row == Some(row) {
                            self.settings_dropdown.reset();
                        } else {
                            self.settings_dropdown.open(row, &self.settings);
                        }
                    }
                }
            } else if !point_in_rect(layout.rect, self.cursor) {
                // FR-039: клик по затемнению (вне rect модалки) —
                // закрыть; канвасу клик не достаётся (иначе двойной
                // клик мимо создал бы заметку)
                self.settings_open = false;
                self.settings_dropdown.reset();
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
            let panel = hotkeys_panel_rect(viewport);
            if point_in_rect(panel, self.cursor) {
                self.request_redraw();
            }
        }
    }

    /// Миникарта: центрирование + drag.
    fn click_minimap(&mut self) {
        // Миникарта (T13, SPEC §6.1): клик — центрирование камеры,
        // drag — world-точка под курсором следует за ним. Квад
        // рисуется поверх всего — проверка до канвас-хит-тестов
        if let Some(rect) = self.minimap_rect() {
            if point_in_rect(
                [rect[0], rect[1], rect[2] - rect[0], rect[3] - rect[1]],
                self.cursor,
            ) {
                self.center_camera_on_minimap_cursor();
                self.minimap_drag = true;
                self.request_redraw();
            }
        }
    }

    /// Модальный диалог: кнопки Да/Нет, мимо — глотается.
    fn click_dialog(&mut self) {
        // T21: модальный диалог поверх всего — кнопки Да/Нет
        // (клики мимо панели не закрывают: установка — явный выбор)
        if self.dialog.is_some() {
            for (i, rect) in self.dialog_button_rects().iter().enumerate() {
                let [x, y, w, h] = *rect;
                if self.cursor[0] >= x
                    && self.cursor[0] <= x + w
                    && self.cursor[1] >= y
                    && self.cursor[1] <= y + h
                {
                    if i == 0 {
                        self.confirm_dialog();
                    } else {
                        self.cancel_dialog();
                    }
                    break;
                }
            }
            self.request_redraw();
        }
    }

    /// Контекстное меню: подменю/пункты/паддинг/мимо.
    fn click_context_menu(&mut self) {
        // Открытое меню канваса (T7): клик по пункту — действие,
        // клик по поверхности меню (паддинг) — глотается, меню
        // ОСТАЁТСЯ открытым (Radix: клик внутри поверхности меню
        // не закрывает), клик мимо — закрыть (dismiss-клик в канвас
        // не проходит). M5: подменю проверяется ПЕРВЫМ — его колонка
        // правее базового меню. Hit-test — в логических px (курсор).
        if self.menu.is_some() {
            let in_base = self
                .menu_open_rect()
                .is_some_and(|rect| point_in_rect(rect, self.cursor));
            let in_submenu = self
                .menu
                .as_ref()
                .and_then(|m| m.submenu.as_ref())
                .map(submenu_rect)
                .is_some_and(|rect| point_in_rect(rect, self.cursor));
            // 1. Пункт подменю — действие
            if let Some(submenu) = self.menu.as_ref().and_then(|m| m.submenu.as_ref()) {
                if let Some(i) = submenu_item_at(submenu, self.cursor) {
                    let action = submenu.entries[i].action.clone();
                    self.menu = None;
                    match action {
                        crate::ui::SubmenuAction::Insert(widget_id) => {
                            self.insert_widget_from_menu(&widget_id);
                        }
                        // T21-C (П11): удаление пакета — с подтверждением;
                        // меню уже закрыто, модальный диалог поверх
                        crate::ui::SubmenuAction::Remove(widget_id) => {
                            let name = self
                                .widgets
                                .registry
                                .get(&widget_id)
                                .map(|p| p.manifest.name.clone())
                                .unwrap_or(widget_id.clone());
                            self.dialog = Some(AppDialog::RemovePackage { widget_id, name });
                        }
                    }
                    self.request_redraw();
                    return;
                }
            }
            // 2. Поверхность подменю без пункта — глотается, не закрывает
            if in_submenu {
                self.request_redraw();
                return;
            }
            // 3. Пункт или паддинг базового меню (список — тот же,
            // что в отрисовке: batch-пункты видны только при N≥3)
            if let Some(menu) = self.menu.take() {
                let items = canvas_menu_visible_items(self.align_menu_visible());
                if let Some(i) = menu_item_at_for(menu.origin, self.cursor, items.len()) {
                    match items[i] {
                        CanvasMenuItem::NewGroup => {
                            let center = self.viewport_center_world();
                            let mut group = plan_group_at(&self.scene.canvas, center);
                            group.label = Some(self.tr(keys::GROUP_DEFAULT_LABEL).to_owned());
                            self.insert_group(group);
                        }
                        // T23: переключение из меню — рантайм,
                        // без записи конфига (как и хоткей F)
                        CanvasMenuItem::FocusMode => self.toggle_focus_mode(),
                        // FR-004.1: тогл оверлея хоткеев из меню
                        // (панель «видно/не видно», галочка ✓)
                        CanvasMenuItem::Hotkeys => {
                            self.hotkeys_open = !self.hotkeys_open;
                        }
                        // M5 (T20-F): открыть подменю пакетов;
                        // повторный клик — тоггл (закрыть). Пустой
                        // список — честная строка «(нет установленных)».
                        // T21-C: под каждой вставкой — секция
                        // удаления пакетов (П11)
                        CanvasMenuItem::Widgets => {
                            if menu.submenu.is_some() {
                                // Тоггл: подменю уже открыто — закрыть
                                self.menu = Some(ContextMenu {
                                    origin: menu.origin,
                                    submenu: None,
                                });
                            } else {
                                let submenu_origin = submenu_origin_next_to(menu.origin);
                                let mut entries: Vec<SubmenuEntry> = self
                                    .widgets
                                    .menu_entries()
                                    .into_iter()
                                    .map(|(widget_id, label)| SubmenuEntry {
                                        action: crate::ui::SubmenuAction::Insert(widget_id),
                                        label,
                                    })
                                    .collect();
                                entries.extend(self.widgets.menu_entries().into_iter().map(
                                    |(widget_id, label)| SubmenuEntry {
                                        action: crate::ui::SubmenuAction::Remove(widget_id),
                                        label:
                                            self.trf(
                                                keys::WIDGETS_REMOVE_ENTRY,
                                                &[("{name}", &label)],
                                            ),
                                    },
                                ));
                                self.menu = Some(ContextMenu {
                                    origin: menu.origin,
                                    submenu: Some(Submenu {
                                        origin: submenu_origin,
                                        entries,
                                    }),
                                });
                            }
                        }
                        // T15: переключатель desktop-режима. Вход
                        // (runtime, без --desktop): перезапуск себя с
                        // --desktop через single-instance handoff —
                        // in-place SetParent не работает (Vulkan-swapchain
                        // не презентует в ребёнка Progman, Renderer
                        // фиксируется с prefer_dx12 при старте). Выход
                        // (уже встроены): in-place detach — DX12-рендерер
                        // в обычном окне презентует, пересоздание не нужно.
                        // На не-Windows — warn.
                        CanvasMenuItem::DesktopMode => {
                            #[cfg(windows)]
                            {
                                if self.desktop_mode && self.desktop_hierarchy.is_some() {
                                    self.leave_desktop();
                                } else {
                                    match self.spawn_desktop_relaunch() {
                                        Ok(()) => tracing::info!(
                                            "перезапуск на --desktop: новый инстанс \
                                             закроет текущий (single-instance handoff)"
                                        ),
                                        Err(err) => {
                                            tracing::warn!(
                                                %err,
                                                "перезапуск на --desktop не удался"
                                            );
                                            canvas_shell::desktop::attach::fallback_message_box(
                                                &format!(
                                                    "Не удалось перезапустить CanvasDesk \
                                                 в режиме десктопа:\n{err}\n\nЗапустите \
                                                 приложение вручную с флагом --desktop."
                                                ),
                                            );
                                        }
                                    }
                                }
                            }
                            #[cfg(not(windows))]
                            {
                                tracing::warn!("desktop-режим не поддерживается на этой платформе");
                            }
                        }
                        // FR-016 (CP5): тогл оверлея узких мест из меню —
                        // персистентная настройка (как Ctrl+B)
                        CanvasMenuItem::BottleneckOverlay => {
                            self.toggle_bottleneck_overlay();
                        }
                        // FR-017 (CP6): тогл what-if режима из меню
                        // (эквивалент Ctrl+Shift+I; подмены в
                        // сценариях переживают выход — Q3b)
                        CanvasMenuItem::WhatIf => {
                            if self.scene.whatif_active {
                                self.exit_whatif_mode();
                            } else {
                                self.enter_whatif_mode();
                            }
                        }
                        // FR-038 п.16-17 (T-038.5): batch-операции
                        // выделения — ОДНА undo-операция на все ноды;
                        // хоткеи не назначаются (F1 HOTKEYS не трогаем,
                        // кандидат — на приёмку FR-038)
                        CanvasMenuItem::AlignHorizontal => {
                            // ряд: общая ось Y (центры на одной горизонтали)
                            self.run_batch_op(BatchOp::Align, Some(AlignAxis::Y));
                        }
                        CanvasMenuItem::AlignVertical => {
                            // колонна: общая ось X (центры на одной вертикали)
                            self.run_batch_op(BatchOp::Align, Some(AlignAxis::X));
                        }
                        CanvasMenuItem::DistributeEvenly => {
                            // ось раскладки — из контекста выделения
                            // (решение T-038.5: одна кнопка, правило в
                            // distribute_axis_for)
                            self.run_batch_op(BatchOp::Distribute, None);
                        }
                    }
                    self.request_redraw();
                    return;
                }
                if in_base {
                    // Паддинг базового меню — меню остаётся открытым
                    self.menu = Some(menu);
                    self.request_redraw();
                    return;
                }
                // Клик мимо — меню закрыто (take выше), клик глотается
                self.request_redraw();
            }
        }
    }

    /// Угловые кнопки: тема / помощь «?» / настройки ⚙ — диспетчер по
    /// имени hit-элемента кадра.
    fn click_corner_button(&mut self, element: &str) -> bool {
        let viewport = self.viewport_logical();
        match element {
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

    /// Палитра выделения: Entry/Trigger/Bar поглощают, мимо rect'ов —
    /// `false` (клик уходит в канвас). palette_view() обновляет
    /// hover-intent — как в прежней цепочке.
    fn click_palette(&mut self) -> bool {
        if let Some((lay, groups, open)) = self.palette_view() {
            match palette_hit(&lay, self.cursor, open) {
                Some(PaletteHit::Entry { group, entry }) => {
                    let action = groups[group].entries[entry].action.clone();
                    self.apply_palette_action(action);
                    // Действие выполнено — раскрытие закрывается
                    // (состав групп мог измениться; Radix: закрытие
                    // меню по выбору пункта)
                    self.palette_hover.reset();
                    self.request_redraw();
                    true
                }
                Some(PaletteHit::Trigger(group)) => {
                    // Пин: клик открывает без задержки / закрывает
                    // повторным кликом — стабильность для точного
                    // наведения, как у menu-button в вебе
                    self.palette_hover.toggle_trigger(group);
                    self.request_redraw();
                    true
                }
                Some(PaletteHit::Bar) => {
                    self.request_redraw();
                    true
                }
                None => false,
            }
        } else {
            false
        }
    }
    // --- PRD-0007 (FR-048 X2): окно проверки цепочки расчёта цифры -------

    /// Открыть окно проверки (§6.4): сессионный кэш — мгновенный Ready
    /// (AC-3.3, ≤ 1 с), иначе Loading с честным лоадером + фоновая сборка
    /// (AC-1.2/G5). Повторный «?» на другую цифру — перестройка на новый
    /// корень (У6: одна панель — один корень); main stage закрывается
    /// (F-10 — оверлеи взаимоисключительны, снапшот остаётся в кэше).
    fn open_explain(&mut self, root: LineageNodeId) {
        // F-10 (Q6): открытие оверлея закрывает main stage
        self.close_main_stage();
        let revision = self.scene.revision;
        if let Some(snap) = self.explain_cache.take() {
            if snap.root == root && snap.revision == revision {
                // Переоткрытие из кэша: Ready сразу, чип — если модель
                // всё-таки изменилась (from_snapshot сравнивает ревизии)
                self.explain = Some(ExplainState::from_snapshot(snap, revision));
                self.request_redraw();
                return;
            }
            // Чужой/устаревший снапшот не нужен: новый закэшируется при
            // закрытии окна (гигиена памяти — держим только последний)
        }
        let build = spawn_lineage_build(
            &self.scene.canvas,
            &self.scene.flow_active,
            self.scene.flow_cycle.as_ref(),
            root.clone(),
        );
        self.explain = Some(ExplainState::loading(root, revision, build));
        self.request_redraw();
    }

    /// Закрыть окно (Esc/✕/клик по фону/ошибка сборки): готовое дерево —
    /// в сессионный кэш (AC-3.3, переход в Closed снапшот не уничтожает);
    /// подсветка цепочки гаснет фейдом (F-4, цель 0 в update_focus_state).
    fn close_explain(&mut self) {
        if let Some(mut state) = self.explain.take() {
            if let Some(tree) = state.take_tree() {
                self.explain_cache = Some(ExplainSnapshot {
                    root: state.root,
                    revision: state.revision,
                    tree,
                });
            }
        }
        self.focus_nodes.clear();
        self.focus_edges.clear();
        self.request_redraw();
    }

    /// Клик при открытом окне (§6.4): ✕/чип «Данные изменены»/мета-крошки/
    /// узлы дерева; клик по фону (мимо окна) — закрытие, внутри окна мимо
    /// элементов — глотается (канвас клик не получает).
    fn on_explain_click(&mut self) {
        let viewport = self.viewport_logical();
        let win = explain_ui::window_rect(viewport);
        // ✕ — закрыть (снапшот → сессионный кэш)
        if point_in_rect(explain_ui::close_rect(win), self.cursor) {
            self.close_explain();
            return;
        }
        let ready = self.explain.as_ref().is_some_and(|s| s.is_ready());
        if ready {
            let stale_now = self
                .explain
                .as_ref()
                .is_some_and(|s| s.revision != self.scene.revision);
            // Чип «Данные изменены» — единственный путь Stale → Ready
            // (AC-3.3): перестройка из нового снапшота, тот же корень
            if stale_now && point_in_rect(explain_ui::chip_rect(win), self.cursor) {
                let root = self.explain.as_ref().expect("готово").root.clone();
                self.open_explain(root);
                return;
            }
            // Мета-строка с крошками вида (X2: клик — возврат к корню)
            if point_in_rect(explain_ui::meta_rect(win), self.cursor) {
                if let Some(state) = self.explain.as_mut() {
                    state.click_crumb(0);
                }
                self.request_redraw();
                return;
            }
            // Узел дерева: hit по лейауту кадра (та же чистая функция,
            // что в рендере — детерминизм рендер/ввод)
            let body = explain_ui::body_rect(win);
            let hit = self.explain.as_ref().and_then(|state| {
                let tree = state.tree()?;
                let vis = explain_ui::visibility(
                    tree,
                    state.view_root(),
                    self.settings.explain_depth_limit,
                    &state.expanded,
                );
                let layout = explain_ui::layout_tree(tree, &vis, state.view_root());
                let scale = explain_ui::fit_scale(layout.bounds, body);
                explain_ui::node_at(&layout, scale, body, self.cursor)
            });
            if let Some(idx) = hit {
                if let Some(state) = self.explain.as_mut() {
                    let vis = explain_ui::visibility(
                        state.tree().expect("дерево есть"),
                        state.view_root(),
                        self.settings.explain_depth_limit,
                        &state.expanded,
                    );
                    let _click = state.click_node(idx, &vis);
                }
                self.request_redraw();
                return;
            }
        }
        // Клик по фону (мимо окна) — закрытие (§6.4 Ready/Stale → Closed)
        if !point_in_rect(win, self.cursor) {
            self.close_explain();
            return;
        }
        self.request_redraw();
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

    /// Кадр окна проверки (§6.4) — модальный проход кадра (паттерн
    /// stage_frame: квады + screen-тексты, рендерер выводит поверх всего).
    /// Loading: окно + честный лоадер (кольцо + ротация подписей), канвас
    /// НЕ затемняется (У5 — затемнение и подсветка атомарны с деревом).
    /// Ready: дерево (ветки-безье + карточки узлов), чип Stale, футер.
    fn explain_frame(&mut self, viewport: [f32; 2]) -> (Vec<CardInstance>, Vec<OwnedScreenText>) {
        let mut quads: Vec<CardInstance> = Vec::new();
        let mut texts: Vec<OwnedScreenText> = Vec::new();
        // Присваивается в ветке Ready; ранние выходы его не читают
        let cursor_idx;
        {
            let Some(state) = self.explain.as_ref() else {
                return (quads, texts);
            };
            let palette = ThemeColors::from_theme(self.settings.theme);
            let camera = &self.camera;
            let win = explain_ui::window_rect(viewport);
            // Заголовок корня — из живой модели (тот же title_for, что у
            // подписей проливания); в Loading дерева ещё нет
            let root_title = self
                .scene
                .canvas
                .node(&state.root.node_id)
                .map(title_for)
                .unwrap_or_else(|| "—".to_owned());
            // Окно (затемнение фона НЕ рисуем: в Ready затемняет цепочку
            // FocusView (F-4), в Loading затемнения нет вообще — У5)
            quads.push(screen_rect_quad(
                camera,
                viewport,
                win,
                palette.menu_fill,
                palette.palette_border,
                14.0,
            ));
            // Шапка: заголовок + крошки + ✕ + чип Stale
            texts.push(OwnedScreenText {
                text: self.tr(keys::EXPLAIN_TITLE).to_owned(),
                origin: [win[0] + 16.0, win[1] + 10.0],
                width: (win[2] - 240.0).max(120.0),
                font_size: 15.0,
                color: palette.title,
                align: TextAlign::Left,
            });
            let meta = if state.is_ready() {
                let tree = state.tree().expect("готово");
                let path: Vec<String> = state
                    .view_path
                    .iter()
                    .filter_map(|&i| tree.nodes.get(i).map(|n| n.title.clone()))
                    .collect();
                if path.len() > 1 {
                    path.join(" → ")
                } else {
                    self.trf(keys::EXPLAIN_META, &[("title", root_title.as_str())])
                }
            } else {
                self.trf(keys::EXPLAIN_META, &[("title", root_title.as_str())])
            };
            texts.push(OwnedScreenText {
                text: meta,
                origin: [win[0] + 16.0, win[1] + 32.0],
                width: (win[2] - 240.0).max(120.0),
                font_size: 11.5,
                color: palette.quote,
                align: TextAlign::Left,
            });
            let close = explain_ui::close_rect(win);
            quads.push(screen_rect_quad(
                camera,
                viewport,
                close,
                [0.0; 4],
                palette.palette_border,
                7.0,
            ));
            texts.push(OwnedScreenText {
                text: "×".to_owned(),
                origin: [close[0], close[1] + 2.0],
                width: close[2],
                font_size: 14.0,
                color: palette.body,
                align: TextAlign::Center,
            });
            // Чип «Данные изменены» (AC-3.3/F-5): модель изменилась после
            // сборки — канвас и дерево не перерисовываются сами
            if state.is_ready() && state.revision != self.scene.revision {
                let chip = explain_ui::chip_rect(win);
                quads.push(screen_rect_quad(
                    camera,
                    viewport,
                    chip,
                    color_to_rgba(palette.whatif_badge),
                    palette.palette_border,
                    14.0,
                ));
                texts.push(OwnedScreenText {
                    text: self.tr(keys::EXPLAIN_STALE).to_owned(),
                    origin: [chip[0], chip[1] + 5.0],
                    width: chip[2],
                    font_size: 11.5,
                    color: Color::rgb(255, 255, 255),
                    align: TextAlign::Center,
                });
            }
            // --- Loading: честный лоадер (AC-1.2, У5) -------------------
            if state.is_loading() {
                let body = explain_ui::body_rect(win);
                let cx = body[0] + body[2] / 2.0;
                let cy = body[1] + body[3] / 2.0 - 20.0;
                let elapsed = state.opened_at.elapsed().as_millis();
                let spin = (elapsed as f32 / 900.0) * std::f32::consts::TAU;
                for i in 0..10 {
                    let angle = spin + i as f32 * std::f32::consts::TAU / 10.0;
                    let p = [cx + angle.cos() * 14.0, cy + angle.sin() * 14.0];
                    let mut fill = palette.accent;
                    fill[3] = 0.25 + 0.75 * (i as f32 / 10.0);
                    quads.push(screen_dot(camera, viewport, p, 6.0, fill));
                }
                let captions = [
                    self.tr(keys::EXPLAIN_LOADER_1),
                    self.tr(keys::EXPLAIN_LOADER_2),
                    self.tr(keys::EXPLAIN_LOADER_3),
                    self.tr(keys::EXPLAIN_LOADER_4),
                    self.tr(keys::EXPLAIN_LOADER_5),
                    self.tr(keys::EXPLAIN_LOADER_6),
                    self.tr(keys::EXPLAIN_LOADER_7),
                    self.tr(keys::EXPLAIN_LOADER_8),
                    self.tr(keys::EXPLAIN_LOADER_9),
                    self.tr(keys::EXPLAIN_LOADER_10),
                    self.tr(keys::EXPLAIN_LOADER_11),
                    self.tr(keys::EXPLAIN_LOADER_12),
                ];
                texts.push(OwnedScreenText {
                    text: state.loader_caption(&captions).to_owned(),
                    origin: [body[0], cy + 34.0],
                    width: body[2],
                    font_size: 12.5,
                    color: palette.body,
                    align: TextAlign::Center,
                });
                return (quads, texts);
            }
            // --- Ready: дерево (ветки + карточки), F-5 ------------------
            let Some(tree) = state.tree() else {
                return (quads, texts);
            };
            let body = explain_ui::body_rect(win);
            let vis = explain_ui::visibility(
                tree,
                state.view_root(),
                self.settings.explain_depth_limit,
                &state.expanded,
            );
            let layout = explain_ui::layout_tree(tree, &vis, state.view_root());
            let scale = explain_ui::fit_scale(layout.bounds, body);
            let local = |x: f32, y: f32| {
                [
                    body[0] + explain_ui::BODY_PAD + x * scale,
                    body[1] + explain_ui::BODY_PAD + y * scale,
                ]
            };
            // Ветки — под карточками (порядок рисования): безье из локальных
            // px лейаута → screen → мир (паттерн polyline_dots: линия —
            // цепочка перекрывающихся кружков)
            for curve in &layout.curves {
                let samples = bezier_samples(curve.points, 36);
                let mut fill = if curve.to_leaf {
                    palette.explain_leaf
                } else {
                    palette.accent
                };
                fill[3] = if curve.to_leaf { 0.9 } else { 0.55 };
                for p in samples {
                    let sp = local(p[0], p[1]);
                    quads.push(screen_dot(camera, viewport, sp, 4.0, fill));
                }
            }
            // Разделитель футера + статистика видимого дерева
            let footer_line = [win[0], win[1] + win[3] - explain_ui::FOOTER_H, win[2], 1.0];
            quads.push(screen_rect_quad(
                camera,
                viewport,
                footer_line,
                palette.palette_border,
                [0.0; 4],
                0.0,
            ));
            texts.push(OwnedScreenText {
                text: self.trf(
                    keys::EXPLAIN_STATS,
                    &[
                        ("lv", layout.levels.to_string().as_str()),
                        ("n", layout.nodes.len().to_string().as_str()),
                    ],
                ),
                origin: [win[0] + 16.0, win[1] + win[3] - 27.0],
                width: 280.0,
                font_size: 11.0,
                color: palette.quote,
                align: TextAlign::Left,
            });
            // Карточки узлов (F-3: значение + формула + адрес)
            for laid in &layout.nodes {
                let node = &tree.nodes[laid.idx];
                let rect = {
                    let [x, y] = local(laid.rect[0], laid.rect[1]);
                    [x, y, laid.rect[2] * scale, laid.rect[3] * scale]
                };
                let hovered = state.cursor == Some(laid.idx);
                // Цвет полосы рода узла: расчётный — акцент, лист — слот
                // explain_leaf (контраст ≥ 3:1, AC-3.4), терминалы —
                // ошибка/предупреждение
                let strip = match node.kind {
                    canvas_core::LineageNodeKind::Calc => palette.accent,
                    canvas_core::LineageNodeKind::Leaf => palette.explain_leaf,
                    canvas_core::LineageNodeKind::Cycle => color_to_rgba(palette.error),
                    canvas_core::LineageNodeKind::Unmapped
                    | canvas_core::LineageNodeKind::Unlinked
                    | canvas_core::LineageNodeKind::Truncated => {
                        color_to_rgba(palette.whatif_badge)
                    }
                };
                quads.push(screen_rect_quad(
                    camera,
                    viewport,
                    rect,
                    palette.card_fill,
                    if hovered {
                        palette.accent
                    } else {
                        palette.palette_border
                    },
                    8.0,
                ));
                let strip_rect = [rect[0], rect[1], (4.0 * scale).max(2.0), rect[3]];
                quads.push(screen_rect_quad(
                    camera, viewport, strip_rect, strip, [0.0; 4], 0.0,
                ));
                // Шрифт карточки сжимается fit-масштабом вместе с геометрией
                let font = |px: f32| (px * scale).max(8.0);
                let tx = rect[0] + 10.0;
                let text_w = rect[2] - 16.0;
                // 1) Заголовок ноды-таблицы
                texts.push(OwnedScreenText {
                    text: node.title.clone(),
                    origin: [tx, rect[1] + 7.0],
                    width: text_w,
                    font_size: font(12.0),
                    color: palette.title,
                    align: TextAlign::Left,
                });
                // 2) Значение (Ok — цифра; Err — диагностика; None — метка
                // терминального узла: «цикл»/«не связано»/…)
                let (value_str, value_color) = match &node.value {
                    Some(Ok(v)) => (v.to_string(), palette.title),
                    Some(Err(e)) => (e.clone(), palette.error),
                    None => (
                        match node.kind {
                            canvas_core::LineageNodeKind::Cycle => {
                                self.tr(keys::EXPLAIN_CYCLE).to_owned()
                            }
                            canvas_core::LineageNodeKind::Unmapped => {
                                self.tr(keys::EXPLAIN_UNMAPPED).to_owned()
                            }
                            canvas_core::LineageNodeKind::Unlinked => {
                                self.tr(keys::EXPLAIN_UNLINKED).to_owned()
                            }
                            canvas_core::LineageNodeKind::Truncated => {
                                self.tr(keys::EXPLAIN_TRUNCATED).to_owned()
                            }
                            _ => String::new(),
                        },
                        palette.error,
                    ),
                };
                texts.push(OwnedScreenText {
                    text: value_str,
                    origin: [tx, rect[1] + 25.0],
                    width: text_w,
                    font_size: font(13.5),
                    color: value_color,
                    align: TextAlign::Left,
                });
                // 3) Формула узла/строки (у листа и терминалов нет)
                if let Some(formula) = &node.formula {
                    texts.push(OwnedScreenText {
                        text: formula.clone(),
                        origin: [tx, rect[1] + 44.0],
                        width: text_w,
                        font_size: font(10.5),
                        color: palette.body,
                        align: TextAlign::Left,
                    });
                } else if node.kind == canvas_core::LineageNodeKind::Leaf {
                    // Лист-константа: пометка «исходное значение» (AC-1.4)
                    texts.push(OwnedScreenText {
                        text: self.tr(keys::EXPLAIN_LEAF_TAG).to_owned(),
                        origin: [tx, rect[1] + 44.0],
                        width: text_w,
                        font_size: font(10.5),
                        color: palette.quote,
                        align: TextAlign::Left,
                    });
                }
                // 4) Адресная строка ребра (AC-2.1: квалифицированный адрес)
                if let Some(via) = &laid.via {
                    let mut addr = if let Some(name) = &via.from_output {
                        self.trf(keys::EXPLAIN_ADDR_OUTPUT, &[("name", name.as_str())])
                    } else if let Some(line) = via.from_line {
                        self.trf(
                            keys::EXPLAIN_ADDR_SLOT,
                            &[("n", (line + 1).to_string().as_str())],
                        )
                    } else {
                        String::new()
                    };
                    if let Some(param) = &via.to_param {
                        if !addr.is_empty() {
                            addr.push_str(" · ");
                        }
                        addr.push_str(param);
                    }
                    if !addr.is_empty() {
                        texts.push(OwnedScreenText {
                            text: addr,
                            origin: [tx, rect[1] + 58.0],
                            width: text_w,
                            font_size: font(9.5),
                            color: palette.quote,
                            align: TextAlign::Left,
                        });
                    }
                }
                // Бейдж фронтира «+N глубже» (AC-2.3: ручное разворачивание)
                if vis.frontier[laid.idx] {
                    let bw = 66.0;
                    let bh = 15.0;
                    let badge = [
                        rect[0] + rect[2] - bw - 6.0,
                        rect[1] + rect[3] - bh - 5.0,
                        bw,
                        bh,
                    ];
                    quads.push(screen_rect_quad(
                        camera,
                        viewport,
                        badge,
                        palette.accent,
                        [0.0; 4],
                        7.0,
                    ));
                    texts.push(OwnedScreenText {
                        text: self.trf(
                            keys::EXPLAIN_EXPAND_BADGE,
                            &[("n", vis.hidden_descendants[laid.idx].to_string().as_str())],
                        ),
                        origin: [badge[0], badge[1] + 1.5],
                        width: bw,
                        font_size: 9.5,
                        color: Color::rgb(255, 255, 255),
                        align: TextAlign::Center,
                    });
                }
            }
            // Hover узла дерева (кадр) — рамка акцентом
            cursor_idx = explain_ui::node_at(&layout, scale, body, self.cursor);
        }
        if let Some(s) = self.explain.as_mut() {
            s.cursor = cursor_idx;
        }
        (quads, texts)
    }

    /// Hover-«?» у цифры результата (§6.4 Closed → Hover, hover-only —
    /// решение владельца): pill под курсором у полосы D; клик по цифре —
    /// фолбэк-триггер (AC-1.1, единственный путь на таче).
    fn explain_hover_pill(
        &mut self,
        viewport: [f32; 2],
        quads: &mut Vec<CardInstance>,
        texts: &mut Vec<OwnedScreenText>,
    ) {
        // Pill — только когда окно/stage/редактор не перехватывают курсор
        if self.explain.is_some()
            || self.main_stage.is_some()
            || self.scheme_gallery.open
            || self.onboarding.is_some()
            || self.editing.is_some()
        {
            return;
        }
        let world = self.cursor_world();
        let Some(index) = self.hovered else { return };
        let Some(node) = self.scene.canvas.nodes.get(index) else {
            return;
        };
        let Some(ExprOutcome::Ok(_)) = self.scene.expr_results.get(&node.id) else {
            return;
        };
        let band = [
            node.x,
            node.y + node.height - BODY_PADDING - RESULT_LINE_HEIGHT,
            node.width,
            RESULT_LINE_HEIGHT,
        ];
        if !point_in_rect(band, world) {
            return;
        }
        let palette = ThemeColors::from_theme(self.settings.theme);
        let zoom = self.camera.zoom();
        let origin = self
            .camera
            .screen_to_world([self.cursor[0] + 14.0, self.cursor[1] - 30.0], viewport);
        quads.push(CardInstance {
            pos: origin,
            size: [22.0 / zoom, 18.0 / zoom],
            fill: palette.accent,
            border: [0.0; 4],
            params: [5.0 / zoom, 0.0, 0.0, 1.0],
        });
        texts.push(OwnedScreenText {
            text: "?".to_owned(),
            origin: [self.cursor[0] + 14.0, self.cursor[1] - 28.0],
            width: 22.0,
            font_size: 13.0,
            color: Color::rgb(255, 255, 255),
            align: TextAlign::Center,
        });
    }

    fn on_left_button(&mut self, state: ElementState) {
        self.left_pressed = state == ElementState::Pressed;
        // T15: первый клик по канвасу снимает WS_EX_NOACTIVATE — с этого
        // момента окно может получать фокус («WS_EX_NOACTIVATE до первого
        // клика», TASKS T15); ошибки не критичны, флаг ставим до вызова
        // (повторные клики не ретраят). Клавиатурный фокус ставим явно на
        // КАЖДОМ нажатии: в ребёнке Progman клик активирует top-level-предка,
        // а фокус ввода нашему окну системой не передаётся — без SetFocus
        // WM_KEYDOWN не доходят и текст в нодах не редактируется
        // (attach::focus_window, идемпотентен — внутри GetFocus-проверка)
        #[cfg(windows)]
        if self.desktop_mode && state == ElementState::Pressed && self.desktop_hierarchy.is_some() {
            if !self.desktop_activation_enabled {
                self.desktop_activation_enabled = true;
                if let Some(raw) = self.window_hwnd() {
                    let hwnd = Self::hwnd(raw);
                    if let Err(err) = canvas_shell::desktop::attach::enable_activation(hwnd) {
                        tracing::warn!(%err, "не удалось снять WS_EX_NOACTIVATE");
                    }
                }
            }
            if let Some(raw) = self.window_hwnd() {
                let hwnd = Self::hwnd(raw);
                if let Err(err) = canvas_shell::desktop::attach::focus_window(hwnd) {
                    tracing::warn!(%err, "не удалось передать клавиатурный фокус");
                }
            }
        }
        if self.space_pressed {
            return; // Space+drag — панорамирование (SPEC §8)
        }
        match state {
            ElementState::Pressed => {
                // FR-052 (U2 PRD-0009): единый диспетчер поверхностей —
                // HitStack::pick по кадру реестра решает, кто получает клик
                // (порядок = слои/визуальный верх, а не порядок веток).
                // Block-модали глотают backdrop по контракту поверхности;
                // Capture — только в своих rect'ах; None → dismiss
                // транзиентов и прежняя canvas-цепочка (мир L0).
                let ui_frame = ui_registry::build_frame(self);
                let pick = HitStack::pick(&ui_frame, UiPoint::new(self.cursor[0], self.cursor[1]));
                match pick {
                    Some(HitTarget::Element { surface, rect }) => {
                        let surface_id = surface.surface.as_str().to_owned();
                        let element = rect.element.clone();
                        if self.dispatch_surface_click(&surface_id, &element) {
                            return;
                        }
                    }
                    Some(HitTarget::Backdrop { surface }) => {
                        let surface_id = surface.surface.as_str().to_owned();
                        if self.dispatch_surface_backdrop(&surface_id) {
                            return;
                        }
                    }
                    None => self.dismiss_transients_on_miss(),
                }
                let world = self.cursor_world();
                // PRD-0007 (F-1/AC-1.1): клик по цифре результата (полоса D)
                // — фолбэк-триггер окна проверки цепочки; у константы —
                // панель одного узла (AC-1.4)
                if self.explain.is_none() {
                    if let Some(root) = self.result_band_root_at(world) {
                        self.open_explain(root);
                        self.request_redraw();
                        return;
                    }
                }
                // Выборочный hit-test (T5 + группы): ребёнок группы раньше
                // самой группы, не-group с меньшей площадью в приоритете
                let hit = self.selective_hit(world);
                // Активное редактирование (T7/T8): клик внутри области
                // редактирования — в курсор, клик снаружи — commit и обычная
                // обработка
                if let Some(target) = self.editing.as_ref().map(EditingSession::target) {
                    let avoid = self.settings.edges_avoid_nodes;
                    let inside = match target {
                        EditTarget::Node(index) => hit == Some(index),
                        EditTarget::Edge(index) => edge_edit_area(&self.scene.canvas, index, avoid)
                            .is_some_and(|(origin, width, height)| {
                                world[0] >= origin[0]
                                    && world[0] <= origin[0] + width
                                    && world[1] >= origin[1]
                                    && world[1] <= origin[1] + height
                            }),
                    };
                    if inside {
                        let zoom_px = self.zoom_px();
                        if let (Some(session), Some(renderer)) =
                            (self.editing.as_mut(), self.renderer.as_mut())
                        {
                            if let Some((origin, _, _)) =
                                session_area(&self.scene.canvas, session, avoid)
                            {
                                let x = ((world[0] - origin[0]) * zoom_px) as i32;
                                let y = ((world[1] - origin[1]) * zoom_px) as i32;
                                session.click(renderer.font_system_mut(), x, y);
                                self.editor_dragging = true;
                            }
                        }
                        self.request_redraw();
                        return;
                    }
                    self.finish_editing(true);
                }
                // Хэндлы концов выделенной связи (CR-002): захват хэндла —
                // drag перепривязки без удаления. Проверка ДО портов: хэндл
                // сидит на порту, занятом существующей связью. Зона — та же,
                // что у портов (CR-003, из настроек).
                if let Some(Selection::Edge(edge_index)) = self.selected {
                    if let Some(end) = self.edge_handle_at(edge_index, world) {
                        self.edge_drag = Some(EdgeDrag::Rebind { edge_index, end });
                        self.request_redraw();
                        return;
                    }
                }
                // FR-025: ПОСТРОЧНЫЕ точки выхода (флаг line_ports) —
                // приоритет над сторонными портами в пределах своих рядов:
                // drag от кружка строки создаёт value-ребро со значением
                // именно этой строки (from_port, всегда value). Правка 2:
                // hit-test по кандидатам spatial-индекса — не привязан к
                // hovered (кружки наполовину торчат из ноды; см.
                // line_port_hit)
                if let Some((node_index, port)) = self.line_port_hit(world) {
                    let from_node = self.scene.canvas.nodes[node_index].id.clone();
                    self.edge_drag = Some(EdgeDrag::New {
                        from_node,
                        from_side: Side::Right,
                        // Точка выхода расчёта семантически value
                        value_flow: true,
                        from_port: Some(port),
                    });
                    self.request_redraw();
                    return;
                }
                // Порт hover-ноды (T8): начало drag резиновой линии новой
                // связи — drag ноды/resize/двойной клик не начинаются.
                // У групп портов нет: edge-drag с группы не начинается.
                // Зона захвата — из настроек (CR-003).
                if let Some(node_index) = self.hovered {
                    let port = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .filter(|node| node.kind() != NodeKind::Group)
                        .and_then(|node| {
                            port_at(node, world, self.camera.zoom(), self.settings.port_zone_px)
                        });
                    if let Some(side) = port {
                        let from_node = self.scene.canvas.nodes[node_index].id.clone();
                        // FR-014: Shift+drag — value-ребро (поток значений),
                        // обычный drag — контрольная связь (дефолт)
                        let value_flow = self.modifiers.shift_key();
                        self.edge_drag = Some(EdgeDrag::New {
                            from_node,
                            from_side: side,
                            value_flow,
                            from_port: None,
                        });
                        self.request_redraw();
                        return;
                    }
                }
                // FR-018: Shift+клик по пустому месту — wheel-меню шаблонов
                // в точке курсора (мишень инстанциации — world-точка).
                // Нода/связь под курсором — обычная обработка выше.
                if self.modifiers.shift_key()
                    && hit.is_none()
                    && edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes).is_none()
                {
                    self.wheel_menu = Some(template_ui::WheelMenu {
                        screen: self.cursor,
                        world,
                        category: None,
                    });
                    // W9 (web-приёмка): оракул браузерного дыма —
                    // Shift+клик поднял wheel-меню категорий (FR-018)
                    tracing::debug!(
                        categories = self.templates.categories().len(),
                        "wheel-меню шаблонов: категории"
                    );
                    self.request_redraw();
                    return;
                }
                // Двойной клик (winit его не даёт — свой детектор, T7):
                // по пустому месту — новая заметка, по text-ноде —
                // редактирование, по линии связи — лейбл связи (T8)
                if self.double_click.register(Instant::now(), self.cursor) {
                    let avoid = self.settings.edges_avoid_nodes;
                    match hit {
                        None => match edge_at(&self.scene.canvas, world, avoid) {
                            Some(edge_index) => {
                                // FR-042 (E3): двойной клик по пучку — main
                                // stage (лейбл-редактор у пучка неопределён;
                                // подписи отдельных рёбер видны в stage);
                                // одиночное ребро — лейбл, как раньше.
                                if self.try_open_main_stage(edge_index) {
                                    return;
                                }
                                self.begin_editing_edge(edge_index);
                            }
                            None => {
                                let index = self.create_note_at(world);
                                self.begin_editing(index);
                            }
                        },
                        // T17 (SPEC §7.4 п.7): двойной клик по файловой
                        // ноде — открыть ассоциацией «как в Explorer»
                        // (ShellExecuteEx SEE_MASK_INVOKEIDLIST);
                        // text-ноды — редактирование (T7)
                        Some(index) => {
                            // CR-006: у виджет-ноды текстового редактора нет —
                            // двойной клик (по хрому) не открывает его; клики
                            // по контенту до этой ветки не доходят (guard выше)
                            if self.scene.canvas.nodes[index].kind() == NodeKind::Widget {
                                self.request_redraw();
                                return;
                            }
                            // FR-017: в what-if режиме двойной клик по строке
                            // расчёта — override-поле подмены (база не
                            // редактируется — Q8); по прозаической строке —
                            // toast с объяснением.
                            if self.scene.whatif_active
                                && self.scene.canvas.nodes[index].kind() == NodeKind::Text
                            {
                                match self.calc_line_at(index, world) {
                                    Some(line) => {
                                        self.begin_whatif_override(index, line);
                                    }
                                    None => {
                                        self.show_toast(self.tr(keys::WHATIF_ONLY_CALC_LINES));
                                    }
                                }
                                self.request_redraw();
                                return;
                            }
                            #[cfg(windows)]
                            if let Some(file) = self.scene.canvas.nodes[index].file.clone() {
                                let path = resolve_node_path(&file, &self.scene.canvas_dir());
                                if let Err(err) = canvas_shell::desktop::interop::open_file(&path) {
                                    tracing::warn!(
                                        %err,
                                        path = %path.display(),
                                        "не удалось открыть файл"
                                    );
                                }
                            } else {
                                self.begin_editing(index);
                            }
                            #[cfg(not(windows))]
                            self.begin_editing(index);
                        }
                    }
                    self.request_redraw();
                    return;
                }
                // Ручной resize (T7): захват за правый нижний угол ноды
                if let Some(index) = hit {
                    if in_resize_corner(&self.scene.canvas.nodes[index], world) {
                        self.selected = Some(Selection::Node(index));
                        self.resizing = Some(index);
                        // FR-006: отложенный снапшот «до» resize — шаг
                        // закроется на отпускании при изменении размеров
                        self.begin_pending_undo();
                        self.request_redraw();
                        return;
                    }
                }
                match hit {
                    Some(index) => {
                        // CR-006: ЛКМ по КОНТЕНТУ виджет-ноды — ввод принадлежит
                        // виджету (WIDGETS.md §8.5). Ни выделения, ни drag,
                        // ни рамки выделения, ни семени фокуса: в live ввод
                        // и так уходит в HWND WebView2, в snapshot/placeholder
                        // клик глотается канвасом без оверлеев. Хром
                        // (заголовок 28 px / рамка 8 px) — прежнее поведение.
                        if self.scene.canvas.nodes[index].kind() == NodeKind::Widget {
                            let node = &self.scene.canvas.nodes[index];
                            let rect = [node.x, node.y, node.width, node.height];
                            if canvas_widgets::layout::hit_test(&rect, world)
                                == canvas_widgets::layout::WidgetHit::Content
                            {
                                tracing::debug!(
                                    node_id = %node.id,
                                    "клик по контенту виджета — канвас без оверлеев"
                                );
                                self.request_redraw();
                                return;
                            }
                        }
                        // Ctrl/Shift + клик (CR-001.3): уже выделенное
                        // (в т.ч. одиночный якорь) остаётся, клик-нутая
                        // тоглится; drag с модификатором не начинается
                        // (это правка выделения, не перемещение)
                        if self.modifiers.control_key() || self.modifiers.shift_key() {
                            let primary = self.selected.and_then(|selection| match selection {
                                Selection::Node(index) => Some(index),
                                Selection::Edge(_) => None,
                            });
                            let anchor = toggle_selection_with_primary(
                                primary,
                                &mut self.selected_nodes,
                                index,
                            );
                            self.selected = anchor.map(Selection::Node);
                            self.request_redraw();
                            return;
                        }
                        // Обычный клик: нода вне набора — набор сбрасывается
                        // (одиночное выделение); нода В наборе — тянем набор
                        let in_set = self.selected_nodes.contains(&index);
                        self.selected = Some(Selection::Node(index));
                        if !in_set {
                            self.selected_nodes.clear();
                        }
                        // Drag (T7/CR-001): исходные позиции — одна нода или
                        // весь набор (+ дети групп); на движении delta к всем
                        let origins = drag_origins(&self.scene.canvas, index, &self.selected_nodes);
                        // FR-006: отложенный снапшот «до» перемещения — шаг
                        // закроется на отпускании при фактическом сдвиге
                        self.begin_pending_undo();
                        // FR-012: новый drag отменяет settle-анимацию и
                        // сбрасывает цель втягивания
                        self.settle_anim = None;
                        self.group_drop_target = None;
                        self.dragging = Some(DragState {
                            primary: index,
                            grab_world: world,
                            origins,
                        });
                    }
                    // Промах по нодам: hit-test связей (T8) — ближайшая
                    // в допуске EDGE_HIT_TOLERANCE, иначе сброс выделения.
                    // Рамка (CR-001): drag с пустого места тянет выделение —
                    // финал на отпускании (порог клик/драг отсекает клики)
                    // FR-042 (E3, F-5/F-6): клик по агрегированной линии
                    // пучка — открытие main stage; одиночное ребро —
                    // выделение, как раньше (F-10)
                    None => {
                        if let Some(edge_index) =
                            edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes)
                        {
                            if !self.try_open_main_stage(edge_index) {
                                self.selected = Some(Selection::Edge(edge_index));
                            }
                        } else {
                            self.selected = None;
                        }
                        self.selected_nodes.clear();
                        self.select_rect = Some((world, world, self.cursor));
                    }
                }
                self.request_redraw();
            }
            ElementState::Released => {
                // FR-025: отпускание нажатия на строке палитры — вставка:
                // drag (порог пройден) — в world-точку курсора, клик — в
                // центр viewport. Инстанциация на отпускании, а не на
                // нажатии, чтобы отличить drag от клика
                if let Some(drag) = self.template_drag.take() {
                    let manifest = self.templates.list().get(drag.index).cloned();
                    if let Some(manifest) = manifest {
                        if drag.active {
                            let world = self.cursor_world();
                            self.instantiate_template_at(&manifest, world);
                        } else {
                            let center = self.viewport_center_world();
                            self.instantiate_template_at(&manifest, center);
                        }
                    }
                    self.request_redraw();
                }
                // Рамка выделения (CR-001): движение больше порога —
                // выделяем ноды, пересекающие прямоугольник (AABB,
                // частичное вхождение считается); клик без движения уже
                // отработал в Pressed (edge/сброс)
                if let Some((start, _, press)) = self.select_rect.take() {
                    let moved = (self.cursor[0] - press[0]).abs() > SELECT_DRAG_THRESHOLD
                        || (self.cursor[1] - press[1]).abs() > SELECT_DRAG_THRESHOLD;
                    if moved {
                        let rect = rubber_band_rect(start, self.cursor_world());
                        self.selected_nodes = nodes_in_rect(&self.scene.canvas, rect);
                        self.selected = None;
                    }
                    self.request_redraw();
                }
                // Drop резиновой линии: новая связь (T8) или перепривязка
                // конца существующей (CR-002). На другую ноду — применяем,
                // в пустоту/на ту же ноду/на зеркальный конец — отмена
                if let Some(drag) = self.edge_drag.take() {
                    // FR-050 Н2 (этап C): подсветка целей drag гасится на
                    // отпускании (drag завершён)
                    self.param_drop = None;
                    let world = self.cursor_world();
                    match drag {
                        EdgeDrag::New {
                            from_node,
                            from_side,
                            value_flow,
                            from_port,
                        } => {
                            // FR-025: drag от построчного порта всегда
                            // value-ребро (точка выхода расчёта)
                            let value_flow = value_flow || from_port.is_some();
                            // FR-050 Н2 (этап C): drop value-drag на якорь
                            // параметра шаблонной ноды — value-ребро с toParam
                            // (занятость/цикл/W-AMBIGUOUS-SRC — внутри);
                            // приоритет над портом стороны (якорь сидит на
                            // левом краю, как порт — но семантика точнее)
                            if value_flow {
                                if let Some((target, port)) = self.param_port_hit(world) {
                                    let to_id = self.scene.canvas.nodes[target].id.clone();
                                    if to_id != from_node {
                                        self.drop_to_param(
                                            from_node.clone(),
                                            from_side,
                                            from_port.as_ref(),
                                            to_id,
                                            port.param.clone(),
                                        );
                                        self.request_redraw();
                                        return;
                                    }
                                }
                            }
                            if let Some(target) = self.selective_hit(world) {
                                let to_node = &self.scene.canvas.nodes[target];
                                let to_id = to_node.id.clone();
                                if to_id != from_node {
                                    // FR-050 Н2 (этап C): drop value-ребра на
                                    // шаблонную ноду мимо якоря — меню выбора
                                    // параметра приёмника (иначе позиционное
                                    // ребро даст W-UNUSED-SLOT и авто-строку
                                    // вместо проливания в параметр)
                                    if value_flow && to_node.template().is_some() {
                                        self.open_param_choice_menu(
                                            from_node.clone(),
                                            from_side,
                                            from_port.as_ref(),
                                            to_id,
                                        );
                                        self.request_redraw();
                                        return;
                                    }
                                    let to_side = nearest_side(to_node, world);
                                    // FR-025: построчный исток — индекс строки
                                    // (футер шаблонной ноды — None: узловое
                                    // значение)
                                    let from_line = from_port.and_then(|port| port.line);
                                    if value_flow {
                                        // FR-014: value-ребро, замыкающее цикл,
                                        // — диалог (контрольная связь / отмена);
                                        // валидное — создаётся сразу
                                        if canvas_core::creates_value_cycle(
                                            &self.scene.canvas,
                                            &from_node,
                                            &to_id,
                                        ) {
                                            // FR-025: from_line в диалог не
                                            // попадает — фолбэк (control)
                                            // построчную семантику отбрасывает
                                            self.dialog = Some(AppDialog::EdgeCycle {
                                                from_node,
                                                from_side,
                                                to_node: to_id,
                                                to_side,
                                            });
                                        } else {
                                            self.create_edge(
                                                from_node,
                                                from_side,
                                                to_id,
                                                to_side,
                                                FlowKind::Value,
                                                from_line,
                                            );
                                        }
                                    } else {
                                        self.create_edge(
                                            from_node,
                                            from_side,
                                            to_id,
                                            to_side,
                                            FlowKind::Control,
                                            None,
                                        );
                                    }
                                }
                            }
                        }
                        // CR-002: перепривязка конца — id/лейбл/цвет/стиль
                        // сохраняются (retarget_edge), сторона — ближайшая
                        // к курсору сторона целевой ноды
                        EdgeDrag::Rebind { edge_index, end } => {
                            if let Some(target) = self.selective_hit(world) {
                                let target_node = &self.scene.canvas.nodes[target];
                                let target_id = target_node.id.clone();
                                let side = nearest_side(target_node, world);
                                // FR-006: перепривязка — undo-шаг ДО мутации;
                                // retarget сам отклонит бесполезный перенос —
                                // тогда шаг снимается (no-op клики не копятся)
                                self.push_undo();
                                if canvas_core::retarget_edge(
                                    &mut self.scene.canvas,
                                    edge_index,
                                    end,
                                    &target_id,
                                    side,
                                ) {
                                    self.scene.mark_dirty();
                                    // FR-014: перепривязка могла изменить
                                    // топологию value-потока
                                    self.scene.recompute_flow();
                                } else {
                                    self.scene.undo_stack.pop_back();
                                }
                            }
                        }
                    }
                    self.request_redraw();
                }
                // FR-012: отпускание drag — втягивание в группу (зона была
                // подсвечена) или вынос из группы (отпускание вне rect своей
                // явной группы); каждое — свой undo-шаг membership
                let drop_target = self.group_drop_target.take();
                if let Some(group_index) = drop_target {
                    self.group_insert_dragged(group_index);
                } else {
                    self.group_drag_out_released();
                }
                // FR-038 (п.2/21): snap-at-release — коррекция свободной позиции
                // отпускания по сетке/направляющим. При сработавшем снапе — ДВА
                // undo-шага: drag-шаг закрывается внутри (снапшот начала → до
                // коррекции), затем шаг коррекции (до → после); без снапа —
                // обычный одиночный путь FR-006
                if !self.apply_snap_at_release() {
                    // FR-006: закрытие отложенного drag/resize — undo-шаг при
                    // фактическом изменении (клик без движения не шаг)
                    self.finish_interaction_undo();
                }
                // FR-038 (п.9): направляющие не переживают отпускание
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.clear_guides();
                }
                self.dragging = None;
                self.editor_dragging = false;
                self.resizing = None;
                self.minimap_drag = false;
            }
        }
    }

    fn on_right_button(&mut self, state: ElementState, event_loop: &ActiveEventLoop) {
        // Вне Windows параметр не читается (системное меню T17 — Win32);
        // явный let вместо underscore-имени: имя остаётся осмысленным
        #[cfg(not(windows))]
        let _ = event_loop;
        if state != ElementState::Pressed {
            return;
        }
        // FR-028: открытый онбординг модален — ПКМ глотается (меню
        // канваса/палитра не всплывают под оверлеем)
        if self.onboarding.is_some() {
            self.request_redraw();
            return;
        }
        // FR-050 Н2 (этап C): ПКМ закрывает меню выбора (отмена) — канвасное
        // меню не всплывает поверх активного выбора
        if self.choice_menu.take().is_some() {
            self.request_redraw();
            return;
        }
        // FR-027: ПКМ над меню помощи/просмотрщиком — не открывает меню
        // канваса (клик в поповер — его поверхность, паттерн popover)
        {
            let viewport = self.viewport_logical();
            let over_help = self.help_menu.as_ref().is_some_and(|menu| {
                let in_menu = point_in_rect(docs_ui::help_menu_rect(menu.origin), self.cursor);
                let in_sub = menu.docs_open && {
                    let sub = docs_ui::help_submenu_origin(menu.origin, viewport);
                    point_in_rect(docs_ui::help_submenu_rect(sub), self.cursor)
                };
                in_menu || in_sub
            });
            let over_docs =
                self.docs.is_some() && point_in_rect(docs_ui::viewer_rect(viewport), self.cursor);
            if over_help || over_docs {
                self.request_redraw();
                return;
            }
        }
        // ПКМ во время редактирования — сначала commit (T7)
        if self.editing.is_some() {
            self.finish_editing(true);
        }
        let world = self.cursor_world();
        match self.selective_hit(world) {
            // Нода: выделить → палитра выделения под нодой (FR-009/FR-010).
            // Мультивыделение сохраняется при ПКМ по выделенной ноде
            Some(index) => {
                if !self.selected_nodes.contains(&index) {
                    self.selected_nodes.clear();
                }
                self.selected = Some(Selection::Node(index));
                self.menu = None;
            }
            // Связь: выделить → палитра связи (Стиль/Толщина/Цвет);
            // мимо — меню пустого канваса или закрытие (десктоп-меню T17)
            // FR-042 (E3): ПКМ по агрегированной линии — main stage (единый
            // вход AC-3.1; палитра применяется к конкретному ребру изнутри
            // stage); одиночное ребро — выделение + палитра, как раньше
            None => {
                let avoid = self.settings.edges_avoid_nodes;
                match edge_at(&self.scene.canvas, world, avoid) {
                    Some(edge_index) => {
                        if self.try_open_main_stage(edge_index) {
                            self.request_redraw();
                            return;
                        }
                        self.selected = Some(Selection::Edge(edge_index));
                        self.selected_nodes.clear();
                        self.menu = None;
                    }
                    None => {
                        // T17 (SPEC §7.4 п.6): в --desktop ПКМ по пустому месту —
                        // системное меню десктопа (нативное Win32: Открыть
                        // канвас / Новый текстовый файл / иконки / автозапуск /
                        // Выход); вне --desktop — меню пустого канваса
                        // (создание группы), повторный ПКМ мимо закрывает его.
                        // Исключение: Shift+ПКМ в --desktop открывает canvas-меню
                        // (пункт «✓ Режим десктопа» — выход из встройки без
                        // выхода из приложения). Origin — логические px
                        // (screen-space меню).
                        // Взаимоисключение поповеров: открытие меню прячет
                        // палитру и сбрасывает её раскрытие
                        self.palette_hover.reset();
                        #[cfg(windows)]
                        let desktop_menu = self.desktop_mode
                            && self.desktop_hierarchy.is_some()
                            && !self.modifiers.shift_key();
                        #[cfg(not(windows))]
                        let desktop_menu = false;
                        if desktop_menu {
                            self.menu = None;
                            #[cfg(windows)]
                            self.desktop_menu(event_loop);
                        } else {
                            self.menu = match self.menu.take() {
                                // Повторный ПКМ по тому же пустому месту —
                                // закрыть (тоггл, как у ноды/связи)
                                Some(_) => None,
                                _ => Some(ContextMenu {
                                    origin: self.cursor,
                                    submenu: None,
                                }),
                            };
                        }
                    }
                }
            }
        }
        self.request_redraw();
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

    fn on_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        let scale = self.scale_factor();
        let logical = [position.x as f32 / scale, position.y as f32 / scale];
        // Drag по миникарте (T13): пан следует за курсором — раньше
        // канвас-панорамирования, дрги не конкурируют (нажатие перехвачено)
        if self.minimap_drag {
            self.cursor = logical;
            self.center_camera_on_minimap_cursor();
            self.request_redraw();
            return;
        }
        if self.panning() && self.main_stage.is_none() {
            let delta = [logical[0] - self.cursor[0], logical[1] - self.cursor[1]];
            self.camera.pan(delta);
            self.request_redraw();
        }
        self.cursor = logical;
        // FR-050 Н2 (этап C): hover-пункт меню выбора (подсветка следует
        // за курсором — паттерн аффорданса контекстного меню; геометрия —
        // сдвиг пунктов на заголовок, как в отрисовке)
        if let Some(menu) = self.choice_menu.as_mut() {
            let count = menu.items.len();
            let shifted = [menu.origin[0], menu.origin[1] + CHOICE_MENU_TITLE_H];
            let hovered = crate::ui::menu_item_at_for(shifted, logical, count);
            if menu.hovered != hovered {
                menu.hovered = hovered;
                self.request_redraw();
            }
        }
        // FR-025: нажатие на строку палитры — порог переводит его в drag
        // (ghost-превью следует за курсором до отпускания)
        if let Some(drag) = self.template_drag.as_mut() {
            if drag.update(logical) || drag.active {
                self.request_redraw();
            }
        }
        // Ревизия FR-025: hover-раскрытие категорий свёрнутой полосы палитры
        // (hover-intent / grace; подсветка строки следует за курсором)
        if self.update_template_hover() {
            self.request_redraw();
        }
        // Драг внутри редактора — расширение выделения мышью (T7/T8)
        if self.editor_dragging && !self.space_pressed {
            let world = self.cursor_world();
            let zoom_px = self.zoom_px();
            if let (Some(session), Some(renderer)) = (self.editing.as_mut(), self.renderer.as_mut())
            {
                if let Some((origin, _, _)) =
                    session_area(&self.scene.canvas, session, self.settings.edges_avoid_nodes)
                {
                    let x = ((world[0] - origin[0]) * zoom_px) as i32;
                    let y = ((world[1] - origin[1]) * zoom_px) as i32;
                    session.drag(renderer.font_system_mut(), x, y);
                }
            }
            self.request_redraw();
        }
        if !self.space_pressed {
            // Рамка выделения (CR-001): тянется за курсором (перерисовка на
            // каждое движение — квад в оверлее); пан во время рамки —
            // Space недоступен (guard выше), средняя кнопка замораживает
            if self.select_rect.is_some() {
                let world = self.cursor_world();
                if let Some(rect) = self.select_rect.as_mut() {
                    rect.1 = world;
                }
                self.request_redraw();
            }
            // Ручной resize за правый нижний угол (T7): размеры клампятся
            // минимумом, spatial index обновляется инкрементально
            if let Some(index) = self.resizing {
                let world = self.cursor_world();
                if let Some(node) = self.scene.canvas.nodes.get_mut(index) {
                    node.width = (world[0] - node.x).max(MIN_NODE_WIDTH);
                    node.height = (world[1] - node.y).max(MIN_NODE_HEIGHT);
                    self.scene.spatial.update(index, node);
                    self.scene.mark_dirty();
                }
                self.request_redraw();
            } else if let Some(drag) = self.dragging.clone() {
                // Drag (T7/CR-001): каждая перемещаемая нода — в исходную
                // позицию + дельта курсора от захвата (ровно один сдвиг за
                // кадр; дети групп — в origins с старта, дубликатов нет).
                // FR-038: нода следует курсору СВОБОДНО (п.2 v2) — дельта
                // кадра клампится только live-collision (п.15, если включён);
                // grid/guides — предпросмотр + release-time, позиции не трогают
                let world = self.cursor_world();
                let delta = [world[0] - drag.grab_world[0], world[1] - drag.grab_world[1]];
                let snap_frame = self.compute_snap_frame(&drag, delta);
                let eff = snap_frame.as_ref().map_or(delta, |frame| frame.eff_delta);
                for (index, origin) in &drag.origins {
                    self.scene
                        .move_node(*index, origin[0] + eff[0], origin[1] + eff[1]);
                }
                // FR-012: зона втягивания — группа под центром первичной ноды
                let target = self.group_drop_target(&drag);
                if target != self.group_drop_target {
                    self.group_drop_target = target;
                }
                self.scene.mark_dirty();
                // FR-038 (п.2/8/9): предпросмотр — направляющие + ghost
                // snapped-позиции; без снапа слой гасится
                self.update_snap_preview(snap_frame.as_ref());
                self.request_redraw();
            } else if self.edge_drag.is_some() {
                // Резиновая линия (T8) следует за курсором — курсор уже
                // обновлён выше, нужна только перерисовка.
                // FR-050 Н2 (этап C): цель value-drag — пересчёт подсветки
                // якорей параметров (совместимость Н5), смена — перерисовка
                let param_drop = self.compute_param_drop();
                if param_drop != self.param_drop {
                    self.param_drop = param_drop;
                }
                self.request_redraw();
            } else if !self.panning() && !self.editor_dragging && self.editing.is_none() {
                // Hover (T8): порты ноды под курсором; перерисовка — только
                // при смене ноды, чтобы не крутить кадры на каждый пиксель.
                // Выборочный hit: над ребёнком группы hover уходит ему,
                // а не группе (порты групп не рисуются — cards.rs).
                // Модальность (практики UI): над открытым диалогом/поиском/
                // меню канваса hover-порты гасятся — сквозь оверлей
                // не подсвечивают
                let world = self.cursor_world();
                // FR-052 (U2): hover-порты гасятся, если клик в точке
                // курсора перехватила экранная поверхность (pick по кадру
                // реестра) — прежний список dialog/search/menu заменён
                // правилом (модали/панели не подсвечивают мир под собой)
                let hovered = {
                    let frame = ui_registry::build_frame(self);
                    if HitStack::absorbs(&frame, UiPoint::new(self.cursor[0], self.cursor[1])) {
                        None
                    } else {
                        self.selective_hit(world)
                    }
                };
                if hovered != self.hovered {
                    self.hovered = hovered;
                    self.request_redraw();
                } else if self.palette_target().is_some()
                    || self.menu.is_some()
                    || self.search.is_open()
                    || self.settings_open
                {
                    // Палитра/меню/поиск/настройки: hover-подсветка элементов
                    // следует за курсором
                    self.request_redraw();
                }
                // FR-042 (E2): BundleHover — ребро пучка веса ≥ 2 под
                // курсором (hover-бамп агрегированной линии + курсор);
                // вычисляется на кадр ввода, в кэш не пишется. Нода под
                // курсором / открытый stage / drag — hover пучка нет.
                let bundle_hover = if self.main_stage.is_none()
                    && self.edge_drag.is_none()
                    && self.settings.edge_aggregation
                    && hovered.is_none()
                {
                    edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes).filter(
                        |&i| {
                            self.scene
                                .bundles
                                .bundle_of_edge(i)
                                .is_some_and(|b| b.weight >= 2)
                        },
                    )
                } else {
                    None
                };
                if bundle_hover != self.bundle_hover {
                    self.bundle_hover = bundle_hover;
                    self.request_redraw();
                }
            }
        }
        // Аффорданс курсора (Grabbing/Text/NwseResize/Arrow) — после всех
        // смен состояний этого события
        self.sync_cursor_icon();
    }

    fn on_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // FR-042 (E3, F-9): открытое main stage модально — колесо глушится
        // (пан/зум канваса в stage недоступны, инвариант 8)
        if self.main_stage.is_some() {
            return;
        }
        // Ревизия FR-025: колесо над flyout свёрнутой палитры прокручивает
        // список шаблонов, а не панорамирует канвас (знак — как у списков:
        // колесо от себя, y<0, увеличивает scroll_top)
        if !self.template_panel.open {
            let viewport = self.viewport_logical();
            let categories = self.template_category_names();
            let strip = template_ui::dock_strip_layout(&categories, viewport[1]);
            let fly = self.template_flyout_geometry(viewport, &strip);
            if let (Some(hover), Some(fly)) = (self.template_hover.as_mut(), fly) {
                if fly.max_scroll > 0 && point_in_rect(fly.rect, self.cursor) {
                    let lines = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y.round() as i32,
                        MouseScrollDelta::PixelDelta(pos) => (pos.y / 40.0).round() as i32,
                    };
                    hover.scroll_by(-lines, fly.max_scroll);
                    self.request_redraw();
                    return;
                }
            }
        }
        // FR-027: просмотрщик документации открыт — колесо над панелью
        // скроллит его контент (кламп; раскладка пересобирается, если
        // ширина панели изменилась — окно resize/миграция монитора)
        if self.docs.is_some() {
            let viewport = self.viewport_logical();
            let panel = docs_ui::viewer_rect(viewport);
            if point_in_rect(panel, self.cursor) {
                let scale = self.scale_factor();
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * PAN_PX_PER_LINE,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / scale,
                };
                let content = docs_ui::viewer_content_rect(panel);
                if let Some(viewer) = self.docs.as_mut() {
                    if (viewer.layout_width - content[2]).abs() > 0.5 {
                        viewer.layout = docs_ui::layout_page(viewer.page, content[2]);
                        viewer.layout_width = content[2];
                        viewer
                            .scroll
                            .resize(viewer.layout.content_height, content[3]);
                    }
                    if viewer.scroll.wheel(dy) {
                        self.request_redraw();
                    }
                }
                return;
            }
        }
        // Колесо над screen-space UI (панели/меню/палитра/миникарта) холст
        // не двигает — практика canvas-приложений (Miro/Figma)
        if self.cursor_over_screen_surface() {
            return;
        }
        // Тачпады шлют PixelDelta (физические px), колёсики мышей — LineDelta
        let scale = self.scale_factor();
        let (dx, dy) = match delta {
            MouseScrollDelta::LineDelta(x, y) => (x * PAN_PX_PER_LINE, y * PAN_PX_PER_LINE),
            MouseScrollDelta::PixelDelta(pos) => (pos.x as f32 / scale, pos.y as f32 / scale),
        };
        let viewport = self.viewport_logical();
        if self.modifiers.control_key() {
            // Ctrl+колесо — зум к позиции курсора (SPEC §8)
            let factor = match delta {
                MouseScrollDelta::LineDelta(_, y) => ZOOM_STEP_PER_LINE.powf(y),
                MouseScrollDelta::PixelDelta(pos) => (pos.y as f32 * 0.005).exp(),
            };
            self.camera.zoom_at(factor, self.cursor, viewport);
        } else {
            // Двухпальцевый скролл тачпада — панорамирование (SPEC §8)
            self.camera.pan([dx, dy]);
        }
        self.request_redraw();
    }

    fn on_pinch(&mut self, delta: f64) {
        // FR-042 (E3, F-9): пинч при открытом stage глушится
        if self.main_stage.is_some() {
            return;
        }
        // Пинч над screen-space UI — холст не зумит (как колесо выше)
        if self.cursor_over_screen_surface() {
            return;
        }
        let viewport = self.viewport_logical();
        let factor = (delta as f32).exp();
        self.camera.zoom_at(factor, self.cursor, viewport);
        self.request_redraw();
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
    fn dialog_rect(&self) -> [f32; 4] {
        let viewport = self.viewport_logical();
        let w = 440.0_f32.min(viewport[0] - 40.0).max(280.0);
        let h = 150.0;
        [(viewport[0] - w) / 2.0, (viewport[1] - h) / 2.0, w, h]
    }

    /// Rect кнопок диалога: [Да][Нет] внизу панели (индексы как в buttons()).
    fn dialog_button_rects(&self) -> [[f32; 4]; 2] {
        let [x, y, w, h] = self.dialog_rect();
        let bw = 110.0;
        let bh = 30.0;
        let gap = 16.0;
        let total = bw * 2.0 + gap;
        let start = x + (w - total) / 2.0;
        let by = y + h - bh - 16.0;
        [[start, by, bw, bh], [start + bw + gap, by, bw, bh]]
    }

    /// Подтверждение диалога (Enter/клик «Да»): установка или удаление.
    fn confirm_dialog(&mut self) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        match dialog {
            AppDialog::InstallWidget {
                src,
                manifest,
                pos,
                updating,
            } => {
                match self.widgets.install_package(&src) {
                    Ok(canvas_widgets::registry::InstallOutcome::Installed) => {
                        self.show_toast(self.trf(
                            keys::TOAST_WIDGET_INSTALLED,
                            &[("{name}", manifest.name.as_str())],
                        ));
                    }
                    Ok(canvas_widgets::registry::InstallOutcome::Updated) => {
                        self.show_toast(self.trf(
                            keys::TOAST_WIDGET_UPDATED,
                            &[
                                ("{name}", manifest.name.as_str()),
                                ("{version}", manifest.version.as_str()),
                            ],
                        ));
                    }
                    Ok(canvas_widgets::registry::InstallOutcome::SameVersion) => {
                        self.show_toast(self.trf(
                            keys::TOAST_WIDGET_SAME_VERSION,
                            &[("{name}", manifest.name.as_str())],
                        ));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "установка виджета не удалась");
                        self.show_toast(
                            self.trf(keys::TOAST_INSTALL_FAILED, &[("{err}", &e.to_string())]),
                        );
                        self.request_redraw();
                        return;
                    }
                }
                // Нода в точке дропа (SPEC §10: «виджет ставится на канвас»);
                // при обновлении — не дублируем (П5: props/ноды сохраняются)
                if !updating {
                    self.push_undo();
                    let id = self.widgets.next_node_id(&self.scene.canvas);
                    let node = self.widgets.build_widget_node(
                        &manifest.id,
                        id,
                        [pos[0] + 40.0, pos[1] + 30.0],
                    );
                    if let Some(node) = node {
                        self.scene.canvas.nodes.push(node);
                        let index = self.scene.canvas.nodes.len() - 1;
                        let node_ref = &self.scene.canvas.nodes[index];
                        self.scene.spatial.insert(index, node_ref);
                        self.selected = Some(Selection::Node(index));
                        self.scene.mark_dirty();
                    }
                }
                self.request_redraw();
            }
            AppDialog::RemovePackage { widget_id, name } => {
                match self.widgets.remove_package(&widget_id) {
                    Ok(()) => {
                        self.show_toast(
                            self.trf(keys::TOAST_PACKAGE_REMOVED, &[("{name}", &name)]),
                        );
                        // Ноды пакета остаются (деградируют в заглушки —
                        // package_ok=false в LOD); пересборка spatial не нужна
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "удаление пакета не удалось");
                        self.show_toast(
                            self.trf(keys::TOAST_REMOVE_FAILED, &[("{err}", &e.to_string())]),
                        );
                    }
                }
                self.request_redraw();
            }
            // FR-014: подтверждение цикла — ребро создаётся как
            // контрольная связь (без потока значений); FR-025: построчный
            // исток не сохраняется — control-ребро значения не переносит
            AppDialog::EdgeCycle {
                from_node,
                from_side,
                to_node,
                to_side,
            } => {
                self.create_edge(
                    from_node,
                    from_side,
                    to_node,
                    to_side,
                    FlowKind::Control,
                    None,
                );
            }
            // FR-050 Н4: замена источника — ОДИН undo-шаг (FR-006): старые
            // рёбра (легаси-дубли — все) удаляются, новое создаётся;
            // инвариант «после успешного создания нет двух value-рёбер в
            // один toParam» восстанавливается
            AppDialog::ReplaceSource {
                from_node,
                from_side,
                from_line,
                to_node,
                to_side,
                param,
                old_edges,
                ..
            } => {
                self.push_undo();
                for id in &old_edges {
                    self.scene.canvas.remove_edge(id);
                }
                let mut edge = Edge::new(
                    self.scene.canvas.next_edge_id(),
                    from_node,
                    Some(from_side),
                    to_node,
                    Some(to_side),
                );
                edge.set_flow_kind(FlowKind::Value);
                edge.from_line = from_line;
                edge.to_param = Some(param);
                self.scene.canvas.add_edge(edge);
                self.scene.mark_dirty();
                self.scene.recompute_flow();
                self.request_redraw();
            }
        }
    }

    /// Отмена диалога (Esc/клик «Нет»): ничего не меняется.
    fn cancel_dialog(&mut self) {
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

    /// Применить настройку/действие ноды из палитры выделения
    /// (FR-009; диспетчер для `PaletteAction::Node`). Каждая мутирующая
    /// настройка — «push_undo → мутация → mark_dirty»; переименование
    /// входит в редактирование (его undo — commit сессии).
    fn apply_node_setting(&mut self, node_index: usize, setting: crate::ui::NodeSetting) {
        use crate::ui::NodeSetting;
        match setting {
            // FR-009: переименовать = вход в редактирование (двойной клик)
            NodeSetting::Rename => self.begin_editing(node_index),
            NodeSetting::Duplicate => {
                // Дублирование одной ноды (паттерн duplicate_selection)
                let Some(node) = self.scene.canvas.nodes.get(node_index).cloned() else {
                    return;
                };
                let copies = reassign_ids(&self.scene.canvas, &[node]);
                let nodes = paste_nodes(
                    &copies,
                    PastePlacement::Offset([DUPLICATE_OFFSET, DUPLICATE_OFFSET]),
                );
                self.insert_nodes(nodes, true);
            }
            // FR-011: mindmap из меню
            NodeSetting::AddChild => self.mindmap_add_child(node_index),
            NodeSetting::AddSibling => self.mindmap_add_sibling(node_index),
            NodeSetting::CollapseBranch => self.mindmap_set_collapsed(node_index, true),
            NodeSetting::ExpandBranch => self.mindmap_set_collapsed(node_index, false),
            // FR-009: файловые операции (открытие — Windows, SPEC §7.4)
            NodeSetting::OpenFile => {
                #[cfg(windows)]
                {
                    let path = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .and_then(|node| node.file.clone())
                        .map(|file| resolve_node_path(&file, &self.scene.canvas_dir()));
                    if let Some(path) = path {
                        if let Err(err) = canvas_shell::desktop::interop::open_file(&path) {
                            tracing::warn!(%err, path = %path.display(), "не удалось открыть файл");
                        }
                    }
                }
                #[cfg(not(windows))]
                let _ = node_index;
            }
            NodeSetting::OpenFolder => {
                // Папка файла через ShellExecuteEx на директорию (Win)
                #[cfg(windows)]
                {
                    let dir = self
                        .scene
                        .canvas
                        .nodes
                        .get(node_index)
                        .and_then(|node| node.file.clone())
                        .map(|file| resolve_node_path(&file, &self.scene.canvas_dir()))
                        .and_then(|path| path.parent().map(|p| p.to_path_buf()));
                    if let Some(dir) = dir {
                        if let Err(err) = canvas_shell::desktop::interop::open_file(&dir) {
                            tracing::warn!(%err, dir = %dir.display(), "не удалось открыть папку");
                        }
                    }
                }
                #[cfg(not(windows))]
                let _ = node_index;
            }
            NodeSetting::CopyPath => {
                let text = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .and_then(|node| node.file.clone().or_else(|| node.text.clone()))
                    .unwrap_or_default();
                if !text.is_empty() {
                    self.clipboard.set_text(text);
                    self.show_toast(self.tr(keys::TOAST_PATH_COPIED));
                }
            }
            NodeSetting::ClearText => {
                // FR-006: очистка текста — undo-шаг (no-op на пустой — без шага)
                let snapshot = self.scene.canvas.clone();
                if let Some(node) = self.scene.canvas.nodes.get_mut(node_index) {
                    node.text = Some(String::new());
                }
                if self.scene.canvas != snapshot {
                    self.scene.push_undo(snapshot);
                }
                self.scene.mark_dirty();
            }
            NodeSetting::Ungroup => {
                // Разгруппировать = удалить группу-ноду без каскада по детям
                // (removing_group_keeps_children); дети остаются на местах
                let is_group = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .is_some_and(|node| node.kind() == NodeKind::Group);
                if !is_group {
                    return;
                }
                self.push_undo();
                self.scene.canvas.remove_node(node_index);
                // Индексы сдвинулись — spatial/кэши перестраиваются
                // (паттерн delete_selected)
                self.scene.spatial = SpatialIndex::build(&self.scene.canvas);
                self.selected = None;
                self.selected_nodes.clear();
                self.scene.mark_dirty();
                self.show_toast(self.tr(keys::TOAST_GROUP_UNGROUPED));
            }
            NodeSetting::WidgetReload => {
                let node_id = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .map(|node| node.id.clone());
                if let Some(node_id) = node_id {
                    self.widgets.reload_widget(&node_id);
                    self.show_toast(self.tr(keys::TOAST_WIDGET_RELOADING));
                }
            }
            NodeSetting::WidgetPermissions => {
                let node_id = self
                    .scene
                    .canvas
                    .nodes
                    .get(node_index)
                    .map(|node| node.id.clone());
                let summary = node_id
                    .as_deref()
                    .and_then(|id| self.widgets.permissions_of_node(&self.scene.canvas, id))
                    .map(|permissions| {
                        let list: Vec<&str> =
                            permissions.list().iter().map(|p| p.as_str()).collect();
                        if list.is_empty() {
                            "нет особых разрешений".to_owned()
                        } else {
                            list.join(", ")
                        }
                    })
                    .unwrap_or_else(|| "пакет не установлен".to_owned());
                self.show_toast(self.trf(keys::TOAST_PERMISSIONS, &[("{summary}", &summary)]));
            }
        }
    }

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

    /// FR-050 Н2 (этап C): клик по меню выбора — пункт выполняет действие
    /// (геометрия отрисовки: сдвиг на заголовок); клик по заголовку/
    /// паддингу — отмена: ребро не создаётся (меню уже снято диспетчером
    /// НЕ было — берём сами, контракт как у контекстного меню T7).
    fn click_choice_menu(&mut self) {
        if let Some(menu) = self.choice_menu.take() {
            if let Some(i) = self.choice_menu_item_at(self.cursor) {
                let action = menu.items[i].action.clone();
                self.apply_choice_action(action);
            }
            // Клик по заголовку/паддингу — отмена («мимо пункта»)
        }
        self.request_redraw();
    }
}

impl ApplicationHandler<AppEvent> for App {
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                // Штатный выход (T17): форс-сейв (SPEC §9 — не ждать
                // debounce) + восстановление иконок (R5) — единая точка
                // с пунктом меню «Выход»
                self.shutdown(event_loop);
            }
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
                self.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.set_scale_factor(scale_factor);
                }
                // Миникарта (T13): буфер растеризован в физических px —
                // пересоберётся на ближайшем кадре (размер в сигнатуре)
                self.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(&event),
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Middle => {
                    self.middle_pressed = state == ElementState::Pressed;
                    self.sync_cursor_icon();
                }
                MouseButton::Left => self.on_left_button(state),
                MouseButton::Right => self.on_right_button(state, event_loop),
                _ => {}
            },
            WindowEvent::CursorMoved { position, .. } => self.on_cursor_moved(position),
            WindowEvent::CursorLeft { .. } => {
                // Курсор ушёл из окна: hover-порты гаснут (иначе подсветка
                // «залипает» до следующего входа)
                if self.hovered.take().is_some() {
                    self.request_redraw();
                }
            }
            WindowEvent::Focused(false) => {
                // Потеря фокуса окна — сброс залипших жестов (alt-tab во
                // время drag: Released придёт другому окну — нода/пан
                // оставались «прилипшими»)
                self.cancel_pointer_transients();
                self.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => self.on_mouse_wheel(delta),
            WindowEvent::PinchGesture { delta, .. } => self.on_pinch(delta),
            WindowEvent::RedrawRequested => {
                // FR-049 (US-5): ?template=<id> — применить на первом кадре
                // (вьюпорт известен — zoom-to-fit корректен); неизвестный
                // id — мягкий отказ (тост), канвас остаётся как есть
                if let Some(id) = self.pending_scheme.take() {
                    match canvas_core::schemes::SchemeRegistry::embedded().get(&id) {
                        Some(manifest) => {
                            let manifest = manifest.clone();
                            self.apply_scheme(&manifest);
                        }
                        None => {
                            tracing::warn!(scheme = %id, "?template: схема не найдена");
                            self.show_toast(i18n::trf(
                                self.settings.language,
                                keys::GALLERY_UNKNOWN,
                                &[("id", id.as_str())],
                            ));
                        }
                    }
                }
                // Замер интервала между кадрами для HUD (T5)
                let now = Instant::now();
                if let Some(prev) = self.last_frame {
                    self.frame_meter.push(now - prev);
                }
                self.last_frame = Some(now);
                // M8/W4 (wasm-port §3.4): async-инициализация (web) — футура
                // кладёт Renderer в слот и будит цикл request_redraw'ом;
                // забираем здесь, до отрисовки. Натив: слот пуст всегда —
                // ветка мертва, накладных расходов нет.
                let delivered = self.renderer_slot.as_ref().and_then(RendererSlot::take);
                match delivered {
                    Some(Ok(renderer)) => self.install_renderer(renderer),
                    Some(Err(err)) => {
                        // Полная цепочка anyhow в сообщение: на web консоль —
                        // единственный канал диагностики (поля %err
                        // компактный ConsoleLayer суффиксует, но цепочка
                        // источников видна только в alternate-формате)
                        tracing::error!("не удалось инициализировать рендер (async): {err:#}");
                        self.renderer_slot = None;
                    }
                    None => {}
                }
                // Полёт камеры к результату поиска (T14): семпл ease-out —
                // пока полёт активен, about_to_wait держит кадры идущими
                if let Some((flight, start)) = self.flight.take() {
                    let elapsed = start.elapsed().as_millis() as u32;
                    let (center, zoom) = flight.sample(elapsed);
                    self.camera.set_center(center);
                    self.camera.set_zoom(zoom);
                    if !flight.is_finished(elapsed) {
                        self.flight = Some((flight, start));
                    }
                }
                // FR-012: settle-анимация вставки в группу — группа и
                // раздвинутые соседи едут к целевым позициям ease_out_cubic
                if let Some(anim) = &self.settle_anim {
                    let t = (anim.start.elapsed().as_millis() as f32 / SETTLE_ANIM_MS).min(1.0);
                    let k = ease_out_cubic(t);
                    for (index, from, to) in &anim.moves {
                        if let Some(node) = self.scene.canvas.nodes.get_mut(*index) {
                            node.x = from[0] + (to[0] - from[0]) * k;
                            node.y = from[1] + (to[1] - from[1]) * k;
                        }
                        if let Some(node) = self.scene.canvas.nodes.get(*index) {
                            self.scene.spatial.update(*index, node);
                        }
                    }
                    if t >= 1.0 {
                        self.settle_anim = None;
                    }
                    self.request_redraw();
                }
                // Миникарта (T13): пересборка по dirty-условиям ДО отрисовки
                // (текстура должна быть готова к проходу кадра)
                self.update_minimap();
                let hud = self.hud_text();
                // World-оверлеи: Т9 призраки дропа добавляются в конец — mutable
                let mut overlay_instances: Vec<CardInstance> = Vec::new();
                // FR-022: donut-сектора wheel-меню (screen-space, мирится
                // рендерером в world)
                let mut overlay_sectors: Vec<SectorInstance> = Vec::new();
                let mut overlay_labels: Vec<String> = Vec::new();
                let mut overlay_label_pos: Vec<Vec2> = Vec::new();
                // Ширины подписей оверлея: призраки дропа — по ширине
                // карточки-призрака (Т9)
                let mut overlay_widths: Vec<f32> = Vec::new();
                // FR-052 (U2 PRD-0009): экран собирается в ПОЛОСЫ слоёв
                // (ui_registry::ScreenBands): порядок полос выводится из
                // реестра поверхностей (UiLayer по возрастанию — контракт
                // UiFrame::draw_bands), порядок внутри полосы сохранён
                // дословно — визуальный порядок канонических состояний
                // не меняется.
                let mut screen_bands = ui_registry::ScreenBands::default();
                {
                    let (settings_instances, settings_texts) = self.settings_overlay();
                    screen_bands.push(UiLayer::Panels, settings_instances, settings_texts);
                }
                if self.scheme_gallery.open {
                    let (gal_instances, gal_texts) = self.scheme_gallery_overlay();
                    screen_bands.push(UiLayer::Modals, gal_instances, gal_texts);
                } else if self.empty_state_visible() {
                    let (es_instances, es_texts) = self.empty_state_overlay();
                    screen_bands.push(UiLayer::Panels, es_instances, es_texts);
                }
                // Меню пустого канваса (T7): screen-space, константный размер
                {
                    let (menu_instances, menu_texts) = self.canvas_menu_overlay();
                    screen_bands.push(UiLayer::Popups, menu_instances, menu_texts);
                }
                // FR-050 Н2 (этап C): меню выбора (параметр приёмника /
                // строка-источник) — полоса Popups поверх меню канваса
                // (клики/Escape — через реестр поверхностей FR-052 U2)
                {
                    let (choice_instances, choice_texts) = self.choice_menu_overlay();
                    screen_bands.push(UiLayer::Popups, choice_instances, choice_texts);
                }
                // FR-027: меню помощи «?» и просмотрщик документации —
                // поверх канваса (просмотрщик выше меню: открытие закрывает
                // меню, но порядок безопасен в любом состоянии)
                {
                    let (help_instances, help_texts) = self.help_menu_overlay();
                    screen_bands.push(UiLayer::Popups, help_instances, help_texts);
                    let (docs_instances, docs_texts) = self.docs_overlay();
                    screen_bands.push(UiLayer::Popups, docs_instances, docs_texts);
                }
                // FR-028: онбординг-карусель — модальный оверлей первого запуска
                {
                    let (onb_instances, onb_texts) = self.onboarding_overlay();
                    screen_bands.push(UiLayer::Modals, onb_instances, onb_texts);
                }
                // Палитра выделения (FR-009/FR-010): тулбар под выделением;
                // rect'ы запоминаются для airspace виджетов
                let palette_view = self.palette_view();
                if let Some((lay, groups, open)) = &palette_view {
                    let (pal_instances, pal_texts) = self.palette_overlay(lay, groups, *open);
                    screen_bands.push(UiLayer::Widgets, pal_instances, pal_texts);
                }
                // Панель поиска (T14): квады/тексты поверх всего канваса
                {
                    let (search_instances, search_texts) = self.search_overlay();
                    screen_bands.push(UiLayer::Panels, search_instances, search_texts);
                }
                // FR-018: палитра шаблонов (Ctrl+P) и wheel-меню
                // (Shift+клик) — поверх канваса; иконки/хаб wheel — полоса
                // WorldOverlay (над секторами, под панелями)
                {
                    let (tpl_instances, tpl_texts) = self.template_panel_overlay();
                    screen_bands.push(UiLayer::Panels, tpl_instances, tpl_texts);
                    let (wheel_sectors, wheel_instances, wheel_texts) = self.wheel_overlay();
                    overlay_sectors.extend(wheel_sectors);
                    screen_bands.push(UiLayer::WorldOverlay, wheel_instances, wheel_texts);
                }
                // FR-021: popup подсказок Numi-ввода — поверх редактора
                {
                    let (hint_instances, hint_texts) = self.hints_overlay();
                    screen_bands.push(UiLayer::Popups, hint_instances, hint_texts);
                }
                // FR-017 (CP6): what-if нижний бар (пилюля/полоса/список/
                // таблица сравнения) — поверх канваса
                {
                    let (whatif_instances, whatif_texts) = self.whatif_overlay();
                    screen_bands.push(UiLayer::Panels, whatif_instances, whatif_texts);
                }
                // Тултип битой ссылки (T10, SPEC §7.5): у курсора — старый путь
                // файла; screen-space, константный размер при любом зуме.
                // Main stage — модален: тултипы живого канваса глушатся
                // (иначе тултип «просвечивает» поверх затемнения, дефект
                // скриншота — stage должен быть единственным источником
                // контента поверх затемнения)
                if self.main_stage.is_none() {
                    let mut tooltip_texts: Vec<OwnedScreenText> = Vec::new();
                    if let Some(file) = self.hovered.and_then(|index| {
                        self.scene.canvas.nodes.get(index).and_then(|node| {
                            (node.broken_link == Some(true))
                                .then(|| node.file.clone())
                                .flatten()
                        })
                    }) {
                        // Ограничиваем правым краём окна, чтобы длинный путь
                        // не вылез за экран (width — только клип-бounds)
                        let viewport = self.viewport_logical();
                        let origin_x = (self.cursor[0] + 14.0)
                            .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                        tooltip_texts.push(OwnedScreenText {
                            text: self.trf(keys::TOAST_FILE_UNAVAILABLE, &[("{file}", &file)]),
                            origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                            width: TOOLTIP_WIDTH,
                            font_size: 13.0,
                            color: Color::rgb(0xd4, 0xd4, 0xd4),
                            align: TextAlign::Left,
                        });
                    }
                    // Тултип ошибки формульной строки (FR-013, правка 4): курсор
                    // над бейджем «!» (зоны — с прошлого кадра) — сообщение об
                    // ошибке у курсора; так видно, ЧТО именно не так в расчёте
                    if let Some(hit) = self.expr_error_hit_at(self.cursor) {
                        let viewport = self.viewport_logical();
                        let origin_x = (self.cursor[0] + 14.0)
                            .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                        tooltip_texts.push(OwnedScreenText {
                            text: hit.message.clone(),
                            origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                            width: TOOLTIP_WIDTH,
                            font_size: 13.0,
                            color: Color::rgb(0xe5, 0x5c, 0x5c),
                            align: TextAlign::Left,
                        });
                    }
                    // FR-050 Р-3 (этап C): тултип unmapped-ребра «проблема +
                    // решение» (контракт Р-3 — ровно два пункта): курсор над
                    // пунктирной янтарной связью «не подставлено». Параметр с
                    // fromOutput — точный диагноз (какой выход у какой ноды);
                    // прочие (позиционный слот / параметр без адреса выхода) —
                    // общий шаблон. Янтарный тон — тот же, что у ребра.
                    if self.edge_drag.is_none() && self.choice_menu.is_none() {
                        let world = self.cursor_world();
                        if let Some(index) =
                            edge_at(&self.scene.canvas, world, self.settings.edges_avoid_nodes)
                        {
                            let edge = &self.scene.canvas.edges[index];
                            if self.scene.unmapped_edges.iter().any(|id| id == &edge.id) {
                                let text = if let (Some(param), Some(output)) =
                                    (&edge.to_param, &edge.from_output)
                                {
                                    let node_label = self
                                        .scene
                                        .canvas
                                        .node(&edge.from_node)
                                        .map(node_display_label)
                                        .unwrap_or_else(|| edge.from_node.clone());
                                    self.trf(
                                        keys::TOOLTIP_UNMAPPED_PARAM,
                                        &[
                                            ("{param}", param),
                                            ("{output}", output),
                                            ("{node}", &node_label),
                                        ],
                                    )
                                } else {
                                    self.tr(keys::TOOLTIP_UNMAPPED_SLOT).to_owned()
                                };
                                let viewport = self.viewport_logical();
                                let origin_x = (self.cursor[0] + 14.0)
                                    .min(viewport[0].max(0.0) - TOOLTIP_WIDTH.max(0.0));
                                tooltip_texts.push(OwnedScreenText {
                                    text,
                                    origin: [origin_x.max(0.0), self.cursor[1] + 18.0],
                                    width: TOOLTIP_WIDTH,
                                    font_size: 13.0,
                                    // Янтарный акцент анализа (severity warning)
                                    color: Color::rgb(0xf5, 0xa6, 0x23),
                                    align: TextAlign::Left,
                                });
                            }
                        }
                    }
                    screen_bands.push(UiLayer::Popups, Vec::new(), tooltip_texts);
                }
                // T21: модальный диалог (screen-space): панель + тексты +
                // кнопки; рендер после битой ссылки — поверх всего канваса
                if let Some(dialog) = &self.dialog {
                    let [dx, dy, dw, dh] = self.dialog_rect();
                    let mut dialog_instances: Vec<CardInstance> = Vec::new();
                    let mut dialog_texts: Vec<OwnedScreenText> = Vec::new();
                    dialog_instances.push(CardInstance {
                        pos: [dx, dy],
                        size: [dw, dh],
                        fill: canvas_core::tokens::DIALOG_FILL,
                        border: canvas_core::tokens::DIALOG_BORDER,
                        params: [10.0, 0.0, 0.0, 1.0],
                    });
                    let buttons = self.dialog_button_rects();
                    for (i, (label, _)) in dialog.buttons(self.settings.language).iter().enumerate()
                    {
                        let [bx, by, bw, bh] = buttons[i];
                        dialog_instances.push(CardInstance {
                            pos: [bx, by],
                            size: [bw, bh],
                            fill: if i == 0 {
                                canvas_core::tokens::DIALOG_BUTTON_PRIMARY
                            } else {
                                canvas_core::tokens::DIALOG_BUTTON_SECONDARY
                            },
                            border: canvas_core::tokens::DIALOG_BUTTON_BORDER,
                            params: [6.0, 0.0, 0.0, 1.0],
                        });
                        let (btn_box, btn_width) = centered_box(buttons[i], 4.0);
                        dialog_texts.push(OwnedScreenText {
                            text: (*label).to_owned(),
                            origin: [btn_box[0], buttons[i][1] + 7.0],
                            width: btn_width,
                            font_size: 14.0,
                            color: token_color(canvas_core::tokens::DIALOG_TEXT),
                            align: TextAlign::Center,
                        });
                    }
                    dialog_texts.push(OwnedScreenText {
                        text: dialog.title(&self.scene.canvas, self.settings.language),
                        origin: [dx + 20.0, dy + 16.0],
                        width: dw - 40.0,
                        font_size: 16.0,
                        color: token_color(canvas_core::tokens::DIALOG_TEXT),
                        align: TextAlign::Left,
                    });
                    dialog_texts.push(OwnedScreenText {
                        text: dialog.body(self.settings.language),
                        origin: [dx + 20.0, dy + 46.0],
                        width: dw - 40.0,
                        font_size: 13.0,
                        color: token_color(canvas_core::tokens::DIALOG_TEXT_MUTED),
                        align: TextAlign::Left,
                    });
                    screen_bands.push(UiLayer::Modals, dialog_instances, dialog_texts);
                }
                // T21: toast — строка внизу центра, живёт 3 с (T21-A).
                // Истечение проверяем ДО рендера (без borrow-конфликта)
                let toast_alive = self
                    .toast
                    .as_ref()
                    .is_some_and(|(_, at)| at.elapsed().as_secs_f32() < 3.0);
                if !toast_alive {
                    self.toast = None;
                } else if let Some((text, _)) = &self.toast {
                    let viewport = self.viewport_logical();
                    // CR-016: при активном what-if бар занимает низ окна
                    // [viewport−BAR_MARGIN−BAR_HEIGHT, viewport−BAR_MARGIN] —
                    // toast поднимаем над ним, чтобы не перекрывать чипы.
                    let ty = if self.scene.whatif_active {
                        viewport[1] - whatif_ui::BAR_MARGIN - whatif_ui::BAR_HEIGHT - 26.0
                    } else {
                        viewport[1] - 44.0
                    };
                    // CR-015: origin — левый край области (контракт ScreenText):
                    // область [40, viewport−40] по центру окна, текст в её центре.
                    screen_bands.push(
                        UiLayer::Toasts,
                        Vec::new(),
                        vec![OwnedScreenText {
                            text: text.clone(),
                            origin: [40.0, ty],
                            width: viewport[0] - 80.0,
                            font_size: 14.0,
                            color: token_color(canvas_core::tokens::TOAST_TEXT),
                            align: TextAlign::Center,
                        }],
                    );
                }
                // FR-042 (E3) + FR-044: main stage — МОДАЛЬНЫЙ проход кадра.
                // Валидация среза (инвариант 9: фоновые мутации MCP/undo
                // закрывают), затем сборка квадов/текстов stage в ОТДЕЛЬНЫЕ
                // списки: рендерер выводит их после всех z-сегментов, текст-
                // групп и миникарты — ни живой текст канваса (тела нод,
                // подписи связей, бейджи анализа), ни тултипы не «просвечивают»
                // сквозь затемнение (раньше stage шёл в мир-хвост ПОД текстом
                // финального сегмента — регрессия «каши», дефект скриншота).
                // PRD-0007 (X2): опрос фоновой сборки, чип устаревания и
                // реакция на ошибку — ДО заимствований рендера (тост мутирует
                // App, паттерн CP5)
                if let Some(state) = self.explain.as_mut() {
                    // AC-3.3/F-5: чип при любом изменении модели (канвас,
                    // MCP, файл, подмена листа, Apply — все идут через
                    // recompute_flow → ревизия)
                    state.stale = state.revision != self.scene.revision;
                    // Loading → Ready: затемнение и подсветка появляются
                    // атомарно с деревом (У5) — фокус-набор соберёт
                    // update_focus_state на этом же кадре
                    state.poll();
                }
                if self.explain.as_ref().is_some_and(|s| s.is_failed()) {
                    // Корень пропал между кликом и сборкой (AC-3.3):
                    // закрыть с сообщением
                    self.close_explain();
                    self.show_toast(self.tr(keys::EXPLAIN_GONE).to_owned());
                }
                if let Some(stage) = self.main_stage.as_mut() {
                    if !stage.valid(&self.scene.canvas) {
                        self.main_stage = None;
                    }
                }
                // Раскладка среза — единственный mut-заём stage (кадр);
                // далее только чтение — совместимо с методами self.tr/….
                let stage_viewport = self.viewport_logical();
                let mut stage_instances: Vec<CardInstance> = Vec::new();
                let mut stage_owned_texts: Vec<OwnedScreenText> = Vec::new();
                let relaid = self
                    .main_stage
                    .as_mut()
                    .map(|stage| stage.relayout(stage_viewport));
                if let (Some(stage), Some(layout)) = (self.main_stage.as_ref(), relaid) {
                    let (insts, texts) = self.stage_frame(stage_viewport, stage, layout);
                    stage_instances = insts;
                    stage_owned_texts = texts;
                }
                // PRD-0007 (X2): окно проверки — модальный проход кадра
                // (взаимоисключительно с stage, F-10); при закрытом окне —
                // hover-«?» у цифры результата (Closed → Hover)
                if self.explain.is_some() {
                    let (insts, texts) = self.explain_frame(stage_viewport);
                    stage_instances = insts;
                    stage_owned_texts = texts;
                } else if self.main_stage.is_none() {
                    self.explain_hover_pill(
                        stage_viewport,
                        &mut stage_instances,
                        &mut stage_owned_texts,
                    );
                }
                // FR-052 (U2): полосы в порядке отрисовки (слои по возрастанию)
                // + Owned-тексты → заимствованные ScreenText (заём живёт до
                // конца кадра, конфликтов с &mut self нет)
                let bands = screen_bands.finish();
                let band_screen_texts: Vec<Vec<ScreenText>> = bands
                    .iter()
                    .map(|(_, _, texts)| {
                        texts
                            .iter()
                            .map(|t| ScreenText {
                                text: &t.text,
                                origin: t.origin,
                                width: t.width,
                                font_size: t.font_size,
                                color: t.color,
                                align: t.align,
                            })
                            .collect()
                    })
                    .collect();
                let screen_band_refs: Vec<canvas_render::ScreenBand> = bands
                    .iter()
                    .zip(&band_screen_texts)
                    .map(|((layer, instances, _), texts)| canvas_render::ScreenBand {
                        layer: *layer,
                        instances,
                        texts,
                    })
                    .collect();
                // FR-042/FR-044: тексты main stage — отдельный список (не
                // screen_texts панелей): рисуются группой ПОСЛЕ модального
                // прохода квадов stage
                let stage_screen_texts: Vec<ScreenText> = stage_owned_texts
                    .iter()
                    .map(|t| ScreenText {
                        text: &t.text,
                        origin: t.origin,
                        width: t.width,
                        font_size: t.font_size,
                        color: t.color,
                        align: t.align,
                    })
                    .collect();
                // Призраки зоны дропа (T9): рамка bbox сетки + квады-призраки.
                // Кладём В КОНЕЦ оверлея: порядок инстансов = порядок рисования,
                // depth-теста нет — призраки поверх всего
                if let Some(preview) = &self.drop_preview {
                    let positions = crate::ui::drop_grid(preview.origin, preview.plan.len());
                    if let Some(frame) = canvas_render::cards::drop_zone_frame(
                        &positions,
                        [crate::ui::DROP_CARD_W, crate::ui::DROP_CARD_H],
                        crate::ui::DROP_GRID_GAP,
                    ) {
                        overlay_instances.push(frame);
                    }
                    overlay_instances.extend(canvas_render::cards::drop_ghosts(
                        &positions,
                        [crate::ui::DROP_CARD_W, crate::ui::DROP_CARD_H],
                        crate::ui::DROP_PREVIEW_MAX,
                    ));
                    // Подписи призраков (Т9): во время перетаскивания имена
                    // файлов/первая строка заметки видны до самого дропа —
                    // раньше призраки были пустыми рамками
                    let pad = crate::ui::DROP_GHOST_LABEL_PAD;
                    for (ins, pos) in preview.plan.iter().zip(&positions) {
                        overlay_labels.push(crate::ui::drop_ghost_label(&ins.kind));
                        overlay_label_pos.push([pos[0] + pad, pos[1] + 6.0]);
                        overlay_widths.push(crate::ui::DROP_CARD_W - pad * 2.0);
                    }
                }
                let overlay_texts: Vec<OverlayText> = overlay_labels
                    .iter()
                    .zip(&overlay_label_pos)
                    .zip(&overlay_widths)
                    .map(|((label, pos), width)| OverlayText {
                        text: label,
                        origin: *pos,
                        width: *width,
                    })
                    .collect();
                // Пульс подсветки ноды-результата (T14): world-квад с рамкой,
                // затухающей по pulse_alpha; за вырожденный — сброс (рамка
                // оверлейная — border.a, заливка прозрачна после фикса
                // cards.wgsl)
                if let Some((node, start)) = self.pulse {
                    let alpha = pulse_alpha(start.elapsed().as_millis() as u32);
                    if alpha <= 0.0 {
                        self.pulse = None;
                    } else if let Some(target) = self.scene.canvas.nodes.get(node) {
                        let grow = (1.0 - alpha) * 8.0;
                        overlay_instances.push(CardInstance {
                            pos: [target.x - grow, target.y - grow],
                            size: [target.width + grow * 2.0, target.height + grow * 2.0],
                            fill: [0.0; 4],
                            border: [
                                canvas_core::tokens::PULSE_RESULT[0],
                                canvas_core::tokens::PULSE_RESULT[1],
                                canvas_core::tokens::PULSE_RESULT[2],
                                alpha,
                            ],
                            params: [6.0, 0.0, 0.0, 1.0],
                        });
                    }
                }
                // Рамка выделения (CR-001): полупрозрачный world-квад с
                // акцентной рамкой (стиль зоны дропа T9), без тени
                if let Some((start, current, _)) = self.select_rect {
                    let rect = rubber_band_rect(start, current);
                    overlay_instances.push(CardInstance {
                        pos: [rect[0], rect[1]],
                        size: [rect[2], rect[3]],
                        fill: crate::ui::SELECT_RECT_FILL,
                        border: crate::ui::SELECT_RECT_BORDER,
                        params: [4.0, 0.0, 0.0, 1.0],
                    });
                }
                // FR-012: подсветка зоны втягивания — группа под drag-нодой
                if let Some(gi) = self.group_drop_target {
                    if let Some(group) = self.scene.canvas.nodes.get(gi) {
                        overlay_instances.push(CardInstance {
                            pos: [group.x - 4.0, group.y - 4.0],
                            size: [group.width + 8.0, group.height + 8.0],
                            fill: [
                                canvas_core::tokens::ACCENT[0],
                                canvas_core::tokens::ACCENT[1],
                                canvas_core::tokens::ACCENT[2],
                                canvas_core::tokens::ALPHA_10,
                            ],
                            border: [
                                canvas_core::tokens::ACCENT[0],
                                canvas_core::tokens::ACCENT[1],
                                canvas_core::tokens::ACCENT[2],
                                canvas_core::tokens::ALPHA_90,
                            ],
                            params: [8.0, 0.0, 0.0, 1.0],
                        });
                    }
                }
                // M5 (T20-F): airspace-прямоугольники оверлеев (план П7) —
                // до LOD-кадра виджетов; большие панели (поиск/настройки)
                // упрощённо гасят все live (транзиентно), точные rect'ы —
                // меню/подменю/палитра/хоткеи/миникарта
                let mut widget_airspace: Vec<[f32; 4]> = Vec::new();
                if let Some(rect) = self.menu_open_rect() {
                    widget_airspace.push(rect);
                    if let Some(menu) = self.menu.as_ref() {
                        if let Some(submenu) = &menu.submenu {
                            widget_airspace.push(submenu_rect(submenu));
                        }
                    }
                }
                // Палитра выделения: бар + открытая колонка (логические px)
                if let Some((lay, _, open)) = &palette_view {
                    widget_airspace.push(lay.bar);
                    if let Some(open) = open {
                        widget_airspace.push(lay.groups[*open].dropdown);
                    }
                }
                if self.hotkeys_open {
                    widget_airspace.push(hotkeys_panel_rect(self.viewport_logical()));
                }
                if let Some(renderer) = self.renderer.as_ref() {
                    if let Some(rect) = renderer.minimap_rect_logical() {
                        widget_airspace.push(rect);
                    }
                }
                let widget_overlay_active = self.select_rect.is_some()
                    || self.edge_drag.is_some()
                    || self.drop_preview.is_some()
                    || self.settings_open
                    || self.dialog.is_some()
                    || self.search.is_open();
                // T21: модальный диалог — airspace-зона (П7): живые виджеты
                // под ним гасятся в снапшоты, пока диалог открыт
                if self.dialog.is_some() {
                    widget_airspace.push(self.dialog_rect());
                }
                let widget_frame = self.widgets.update_frame(
                    &self.scene.canvas,
                    &self.camera,
                    self.viewport_logical(),
                    self.scale_factor(),
                    &widget_airspace,
                    widget_overlay_active,
                );
                // Owned-квады → ссылки для FrameOverlay (локально: заём
                // живёт до конца кадра, конфликтов с &mut self нет)
                let widget_quad_refs: Vec<canvas_render::WidgetQuad> = widget_frame
                    .quads
                    .iter()
                    .map(|q| canvas_render::WidgetQuad {
                        node_id: q.node_id.as_str(),
                        pos: q.pos,
                        size: q.size,
                    })
                    .collect();
                // CR-004: прозрачные виджет-ноды — контент реально виден
                // (live-HWND или снапшот-текстура); placeholder битых
                // пакетов (broken) остаётся серой карточкой. Виджет без
                // снапшота и вне live (транзиент первого кадра) — тоже
                // непрозрачен: иначе нода исчезала бы целиком.
                let live_ids: std::collections::HashSet<&str> =
                    widget_frame.live.iter().map(|s| s.as_str()).collect();
                let broken_ids: std::collections::HashSet<&str> =
                    widget_frame.broken.iter().map(|s| s.as_str()).collect();
                let widget_transparent: Vec<usize> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| node.kind() == NodeKind::Widget)
                    .filter(|(_, node)| {
                        let id = node.id.as_str();
                        !broken_ids.contains(id)
                            && (live_ids.contains(id)
                                || self
                                    .renderer
                                    .as_ref()
                                    .is_some_and(|r| r.has_widget_snapshot(id)))
                    })
                    .map(|(i, _)| i)
                    .collect();
                // CR-004 v1: заголовок виджет-ноды виден при hover/выделении
                let widget_title_reveal: Vec<usize> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, node)| {
                        node.kind() == NodeKind::Widget
                            && (self.hovered == Some(*i)
                                || self.selected == Some(Selection::Node(*i))
                                || self.selected_nodes.contains(i))
                    })
                    .map(|(i, _)| i)
                    .collect();
                // FR-011: скрытые ноды (свернутые поддеревья) + бейджи «+N»
                let hidden_nodes = self.hidden_subtree_nodes();
                let collapsed_counts: Vec<(usize, usize)> = self
                    .scene
                    .canvas
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, node)| node.collapsed == Some(true))
                    .map(|(i, _)| (i, canvas_core::subtree_ids(&self.scene.canvas, i).len()))
                    .filter(|(_, count)| *count > 0)
                    .collect();
                let overlay = FrameOverlay {
                    instances: &overlay_instances,
                    texts: &overlay_texts,
                    screen_bands: &screen_band_refs,
                    stage_instances: &stage_instances,
                    stage_texts: &stage_screen_texts,
                    screen_sectors: &overlay_sectors,
                    widget_quads: &widget_quad_refs,
                };
                // Резиновая линия (T8/CR-002): от порта/неподвижного конца к
                // курсору; исходная линия перепривязываемой связи скрыта
                let edge_draft = self.edge_drag.as_ref().and_then(|drag| {
                    let (port, side) = drag.draft_origin(&self.scene.canvas)?;
                    Some((port, side, self.cursor_world()))
                });
                let hidden_edge = match self.edge_drag.as_ref() {
                    Some(EdgeDrag::Rebind { edge_index, .. }) => Some(*edge_index),
                    _ => None,
                };
                // FR-016 (CP5): авто-включение оверлея при первом появлении
                // риска (Warn и выше) — один раз за запуск, с тостом;
                // ручной тогл (Ctrl+B/меню/панель) отключает авто до
                // перезапуска (решение «Открытые вопросы» FR-016). До
                // заимствований рендера и FocusView — тост мутирует App.
                if !self.settings.bottleneck_overlay
                    && !self.bottleneck_auto_enabled
                    && analyze::has_risk(&self.scene.analysis)
                {
                    self.settings.bottleneck_overlay = true;
                    self.bottleneck_auto_enabled = true;
                    self.show_toast(self.tr(keys::TOAST_ANALYSIS_ON));
                    // авто-включение не персистим: конфиг не трогаем
                }
                // T23 (brainstorm-focus): пересчёт анимации и окрестности
                // семени ДО сборки сцены — FocusView заимствует поля App
                self.update_focus_state();
                let focus = FocusView {
                    nodes: &self.focus_nodes,
                    edges: &self.focus_edges,
                    dim: self.focus_dim,
                    pulse: self
                        .focus_pulse
                        .as_ref()
                        .map(|(_, start)| focus_pulse(start.elapsed().as_millis() as u32))
                        .unwrap_or(0.0),
                };
                let analysis_view: Option<&AnalysisState> = if self.settings.bottleneck_overlay {
                    Some(&self.scene.analysis)
                } else {
                    None
                };
                if let Some(renderer) = self.renderer.as_mut() {
                    // FR-042 (E2): контекст агрегации кадра — индекс сцены +
                    // hover пучка; None при выключенной агрегации (F-13)
                    let bundle_ctx = self.settings.edge_aggregation.then_some(BundleContext {
                        index: &self.scene.bundles,
                        hover: self.bundle_hover,
                    });
                    let bundle_ctx = bundle_ctx.as_ref();
                    // FR-013 (правка 4): живые построчные результаты (Numi —
                    // результаты по ходу набора): считаем из текста СЕССИИ
                    // на каждый кадр (дёшево: парсинг только формульных
                    // строк; fit_note_size уже вызывает eval_lines покадрово)
                    let editing_line_results: Option<Vec<Option<ExprOutcome>>> =
                        self.editing.as_ref().and_then(|session| {
                            let index = session.node_index()?;
                            let node = self.scene.canvas.nodes.get(index)?;
                            node.kind()
                                .eq(&NodeKind::Text)
                                .then(|| expr::eval_lines(&session.text()))
                        });
                    let scene = SceneView {
                        canvas: &self.scene.canvas,
                        spatial: &self.scene.spatial,
                        selected: self.selected,
                        selected_nodes: &self.selected_nodes,
                        hovered: self.hovered,
                        edge_draft,
                        hidden_edge,
                        edges_avoid: self.settings.edges_avoid_nodes,
                        port_zone_px: self.settings.port_zone_px,
                        line_ports: self.settings.line_ports,
                        // FR-050 Н2 (этап C): подсветка якорей параметров
                        // во время value-drag (совместимость Н5)
                        param_drop: self.param_drop.as_ref().map(|(node_index, params)| {
                            ParamDropView {
                                node_index: *node_index,
                                params,
                            }
                        }),
                        // FR-050 Р-3 (этап C): unmapped-рёбра — пунктир
                        // янтарным акцентом анализа
                        unmapped_edges: &self.scene.unmapped_edges,
                        focus,
                        widget_transparent: &widget_transparent,
                        widget_title_reveal: &widget_title_reveal,
                        hidden_nodes: &hidden_nodes,
                        collapsed_counts: &collapsed_counts,
                        expr_results: &self.scene.expr_results,
                        expr_line_results: &self.scene.expr_line_results,
                        expr_editing_results: editing_line_results.as_deref(),
                        param_spills: &self.scene.param_spills,
                        whatif_nodes: &self.scene.whatif_nodes,
                        analysis: analysis_view,
                        analysis_overlay: self.settings.bottleneck_overlay,
                        // FR-042 (E2): контекст агрегации пучков кадра
                        // (F-13: выкл — None, поведение байт-в-байт прежнее)
                        bundles: bundle_ctx,
                    };
                    match renderer.render(
                        &self.camera,
                        &scene,
                        hud.as_deref(),
                        self.editing.as_mut(),
                        &overlay,
                    ) {
                        Ok(stats) => self.last_stats = stats,
                        Err(err) => {
                            tracing::error!(%err, "ошибка рендера, завершение");
                            event_loop.exit();
                        }
                    }
                    // FR-013 (правка 4): зоны ошибок кадра — для тултипа в
                    // оверлее следующего кадра (отставание в кадр незаметно)
                    self.expr_error_hits = renderer.line_error_hits().to_vec();
                }
                // Тамбнейлы видимых нод (T6): заказ после кадра, когда камера
                // уже установилась; ответы придут через AppEvent::ThumbsReady
                self.order_thumbnails();
            }
            _ => {}
        }
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::ThumbsReady => {
                // Забрать готовые тамбнейлы из канала и загрузить в атлас;
                // ошибки — в негативный кэш (не перезаказывать каждый кадр)
                let mut arrived = 0usize;
                for (node, result) in self.thumbs.drain() {
                    match result {
                        Some(thumb) => {
                            if let Some(renderer) = self.renderer.as_mut() {
                                renderer.set_thumbnail(node, &thumb);
                                arrived += 1;
                            }
                        }
                        None => {
                            self.thumbs_failed.insert(node);
                        }
                    }
                }
                if arrived > 0 {
                    self.request_redraw();
                }
            }
            AppEvent::Drag(event) => self.on_drag_event(event),
            AppEvent::FileEvents(events) => self.on_file_events(events),
            AppEvent::Search(event) => self.on_search_event(event),
            AppEvent::OpenScene {
                path,
                json,
                storage,
            } => self.on_open_scene(path, json, storage),
            #[cfg(windows)]
            AppEvent::Desktop(event) => self.on_desktop_event(event),
            #[cfg(windows)]
            AppEvent::Shell(event) => self.on_shell_event(event),
            #[cfg(windows)]
            AppEvent::McpWake => self.on_mcp_wake(),
            AppEvent::Widget(event) => self.on_widget_event(event),
            // T15-relaunch: exit-сигнал от нового запуска (single-instance
            // handoff) — штатное завершение: форс-сейв сцены, восстановление
            // иконок, exit. Мьютекс освободится смертью процесса, новый
            // инстанс продолжит старт (актуально для перезапуска на --desktop).
            AppEvent::InstanceExit => self.shutdown(event_loop),
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        self.scene.autosave_if_due();
        // M8/W6 (wasm-port §4.2): пока сцена грязная, цикл не засыпает —
        // запланированный кадр держит rAF-цепочку web-цикла живой, иначе
        // about_to_wait не вызывается после последнего события ввода и
        // debounce автосейва (2 с) никогда не срабатывает (правка → коммит
        // → тишина → сохранения нет). На нативе цена — пара лишних кадров
        // в течение 2 с после правки; поведение автосейва идентичное.
        if self.scene.dirty_since.is_some() {
            self.request_redraw();
        }
        // Debounce запроса поиска (T14): 200 мс покоя после правки — отправка.
        // Панель/анимации держат цикл красным через request_redraw ниже,
        // иначе ControlFlow::Wait уснул бы до следующего события
        if let Some((query, edited_at)) = self.search_pending.take() {
            if edited_at.elapsed() < SEARCH_DEBOUNCE {
                self.search_pending = Some((query, edited_at));
            } else {
                self.search_service.command(SearchCommand::Query {
                    query,
                    limit: SEARCH_RESULTS_LIMIT,
                });
            }
        }
        // Полёт камеры и пульс (T14) + фокус (T23): непрерывные кадры
        // до завершения анимаций; hover-ожидание палитры (FR-009):
        // hover-intent открытие / отсрочка закрытия при неподвижном курсоре;
        // ревизия FR-025: то же для flyout свёрнутой полосы шаблонов
        // (hover-intent 150 мс / grace 300 мс при неподвижном курсоре)
        if !self.template_panel.open
            && self.template_hover.is_some()
            && self.update_template_hover()
        {
            self.request_redraw();
        }
        if self.search_pending.is_some()
            || self.flight.is_some()
            || self.pulse.is_some()
            || self.focus_animating()
            || self.palette_hover.pending()
            || self
                .template_hover
                .as_ref()
                .is_some_and(|hover| hover.pending())
        {
            self.request_redraw();
        }
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("CanvasDesk");
        // Режим десктопа (T15): borderless-окно на весь виртуальный экран
        // без активации при создании (WS_EX_NOACTIVATE до первого клика —
        // TASKS T15; winit with_active(false)). Это же окно — фолбэк-режим,
        // если встройка не удастся (R14: не пересоздаём после winit-инициализации).
        // После attach winit-API окна НЕ трогаем — стили перезапишет
        // библиотека (R3-урок tao/Seelen); размеры — только SetWindowPos.
        let attrs = if self.desktop_mode {
            attrs
                .with_decorations(false)
                .with_resizable(false)
                .with_active(false)
        } else {
            attrs
        };
        // winit сам ставит свой IDropTarget (RegisterDragDrop с assert S_OK) —
        // отключаем и ставим свой в canvas-shell (план T9 §3)
        #[cfg(windows)]
        let attrs = attrs.with_drag_and_drop(false);
        // Точная геометрия десктоп-окна (физ. px) — только на Windows:
        // виртуальный экран из EnumDisplayMonitors; до attach — стартовый
        // размер по экрану (потом attach растянет SetWindowPos'ом).
        #[cfg(windows)]
        let attrs = if self.desktop_mode {
            let screen = canvas_shell::desktop::hierarchy::virtual_screen_rect().unwrap_or(
                canvas_shell::desktop::ScreenRect::from_ltrb(0, 0, 1280, 720),
            );
            attrs
                .with_position(winit::dpi::PhysicalPosition::new(screen.left, screen.top))
                .with_inner_size(winit::dpi::PhysicalSize::new(
                    screen.width().max(1) as u32,
                    screen.height().max(1) as u32,
                ))
        } else {
            attrs
        };
        let window = match event_loop.create_window(attrs) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                tracing::error!(%err, "не удалось создать окно");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window.clone());
        // Регистрация своего IDropTarget (T9) и встройка в десктоп (T15) — ДО
        // создания GPU-surface: так attach (SetParent/scrub) не конфликтует
        // с живым swapchain. Сама по себе невидимость встроенного окна
        // порядком не лечилась (проверено экспериментом): Vulkan-swapchain
        // не презентует в ребёнка Progman вне зависимости от момента
        // создания surface — лечится выбором DX12 для desktop-режима
        // (Renderer::new, prefer_dx12). HWND достаём через raw-window-handle
        // (winit 0.30 публично Win32-HWND не отдаёт); ошибка drag-drop —
        // warn и живём без него (graceful degradation).
        #[cfg(windows)]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            // HWND через raw-window-handle: winit 0.30 публично
            // Win32-HWND не отдаёт (внутренний windows-sys); окно
            // создано на этом потоке, handle доступен
            match window.window_handle() {
                Ok(handle) => match handle.as_raw() {
                    RawWindowHandle::Win32(win32) => {
                        match canvas_shell::dragdrop::install(
                            win32.hwnd.get(),
                            self.drag_sender.clone(),
                        ) {
                            Ok(watcher) => self.drag_watcher = Some(watcher),
                            Err(err) => {
                                tracing::warn!(%err, "drag-drop недоступен, приложение работает без него")
                            }
                        }
                        // Встройка в десктоп (T15): после всей winit-настройки
                        // окна (R3-урок: сначала окно настраивается библиотекой,
                        // репарентинг — последним, с верификацией стилей в
                        // attach), но ДО создания GPU-surface (см. выше).
                        if self.desktop_mode {
                            self.attach_desktop(win32.hwnd.get());
                        }
                        // M5 (T20-F): WebView2-хост виджетов — ребёнок окна
                        // канваса; события хоста идут через widget_sender
                        // (прокси из main: у ActiveEventLoop нет create_proxy,
                        // winit 0.30). User-data — единый корень приложения
                        // (~/.canvasdesk/webview2)
                        {
                            let sender = self.widget_sender.clone();
                            let user_data = canvas_shell::default_cache_dir()
                                .unwrap_or_default()
                                .join("webview2");
                            // hwnd.get() уже isize (raw-window-handle 0.6):
                            // без каста — иначе clippy needless_cast на Windows
                            self.widgets
                                .attach_host(win32.hwnd.get(), user_data, sender);
                        }
                    }
                    // На Windows бывает только Win32-handle
                    _ => tracing::warn!("неожиданный handle окна — drag-drop выключен"),
                },
                Err(err) => {
                    tracing::warn!(%err, "handle окна недоступен — drag-drop выключен")
                }
            }
        }
        // M8/W4 (wasm-port §3.4): запуск инициализации Renderer через
        // инъектированную стратегию. Натив — pollster::block_on (GPU-иниц
        // блокирующая, один раз при старте, SPEC §6.3: холодный старт < 2 с;
        // prefer_dx12 = desktop-режим — Vulkan не презентует в ребёнка
        // Progman). Web — spawn_local: результат придёт в слот с побудкой
        // кадра (браузерный главный поток блокировать нельзя — план §7).
        match self
            .renderer_launcher
            .launch(window.clone(), self.desktop_mode)
        {
            RendererLaunch::Ready(renderer) => self.install_renderer(*renderer),
            RendererLaunch::Pending(slot) => {
                tracing::info!("GPU-инициализация асинхронная (web): первый кадр по готовности");
                self.renderer_slot = Some(slot);
            }
            RendererLaunch::Failed(err) => {
                tracing::error!(%err, "не удалось инициализировать рендер");
                event_loop.exit();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

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
        ensure_result_reserve(&mut low, &line, &[0]);
        let needed = measured_result_reserve_height(&line, low.width, &[0]);
        assert!(
            low.height >= needed - 1e-3,
            "высота {} выросла минимум до измеренного резерва футера {needed}",
            low.height
        );
        let grown = low.height;
        ensure_result_reserve(&mut low, &line, &[0]);
        assert_eq!(low.height, grown, "повторный вызов — no-op (growth-only)");
        let mut tall = Node::text("n2", line.clone(), 0.0, 0.0);
        tall.width = 260.0;
        tall.height = 1000.0;
        ensure_result_reserve(&mut tall, &line, &[0]);
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
        let needed = measured_result_reserve_height(&mono_line, node.width, &formula_lines);
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
            measured_result_reserve_height(text, node.width, &formula_lines)
        };
        let tpl_node = scene.canvas.node("tpl1").expect("нода tpl1");
        assert!(
            tpl_node.height >= needed_for(&scene, tpl_node),
            "шаблонная нода выросла под резерв футера"
        );
        match scene.expr_results.get("tpl1").expect("результат tpl1") {
            ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1000 rps"),
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
        let needed = measured_result_reserve_height(&text, tpl_node.width, &formula_lines);
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
        expect_ok(&committed[0], "5246", "commit: хвостовое присваивание");
        expect_ok(&committed[1], "2558", "commit: b");
        expect_ok(&committed[2], "200", "commit: x");
        expect_ok(&committed[3], "7804", "commit: c = a + b");
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
        let cache_dir =
            std::env::temp_dir().join(format!("canvasdesk-w6-app-{}", std::process::id()));
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
