//! canvas-core — модель данных канваса и JSON Canvas I/O.
//! Не зависит от ОС и GPU (SPEC §4): вся платформенная логика — за трейтами.

/// FR-016: анализ узких мест и риска очередей — чистая функция над
/// результатами propagator-а (волна B1/CP5).
pub mod analyze;
/// FR-044 (расширение контракта stage FR-042): геометрия main stage —
/// лейн-раскладка подписей веера и коридор между колонками портов.
pub mod bundles;
/// FR-045 Р-2: CSV-снапшот источника данных ноды «Входные данные».
pub mod csv;
/// FR-045 Р-5 (приоритет владельца): квалифицированная адресация
/// «Объект.Поле» — единая точка сборки отображаемых путей (переиспользует
/// FR-044 для пилюль веера и панели «Как считается»).
pub mod dataref;
/// M8/W3 (wasm-port §3.1/§6): платформенно-нейтральные данные drag-drop
/// (T9): производители — shell (IDropTarget) и canvas-web (DOM, W6);
/// потребитель — приложение.
pub mod dragdrop;
mod edgegeom;
mod error;
/// FR-013: Numi-base формулы text-нод (`canvasdesk.expr`).
pub mod expr;
/// FR-014: поток значений по рёбрам — DAG-движок и live-ревал.
pub mod flow;
mod focus;
mod fs_events;
mod io;
mod layout;
/// Нормализация MCP-текстов (literal `\n` от ИИ-агентов → реальные переводы).
pub mod mcp_text;
mod model;
mod providers;
/// M8/W3 (wasm-port §3.1/§6): протокол поискового индекса + трейт
/// [`search::SearchBackend`] + [`search::MemSearch`] (web/тесты) — переехал
/// из canvas-shell, чтобы нативная (FTS5) и web-реализации жили по разные
/// стороны одного нейтрального контракта.
pub mod search;
mod settings;
mod spatial;
/// FR-018: реестр шаблонных архитектурных нод.
pub mod templates;
/// M8/W1 (wasm-port §6): монотонное время, безопасное под wasm32 —
/// alias над `web_time::Instant`; нативный код подмены не замечает.
pub mod time;
/// PRD-0006 F-1 / FR-046: design-токены — слой примитивов дизайн-системы
/// (зеркало `design/tokens/*.json` с тестом паритета); потребители —
/// `ThemeColors`, компонентные палитры, метрики карточек/текста.
pub mod tokens;
/// FR-032: валидация модели — чистая функция ядра (коды E-*/W-*).
pub mod validate;
/// FR-017 (CP6): what-if сценарии — именованные построчные подмены,
/// персистентность в `canvasdesk.whatif`.
pub mod whatif;

pub use analyze::{
    analyze, badge_text, has_risk, AnalysisConfig, AnalysisFlags, AnalysisState,
    Severity as AnalysisSeverity,
};
/// FR-042 (E1): пучки рёбер и геометрия main stage — индекс сцены (LOD-0
/// агрегация), толщина по весу, rect/веер/раскладка stage, hit-test веера.
pub use bundles::{
    bundle_thickness, main_stage_rect, stage_edge_at, stage_edge_fan, stage_edge_points,
    stage_fan_spacing, stage_layout, EdgeBundle, EdgeBundleIndex, StageLayout,
    MAIN_STAGE_MAX_FRACTION,
};
/// M8/W3 (wasm-port §3.1/§6): платформенно-нейтральные данные drag-drop
/// (T9) — shell производит (IDropTarget), canvas-web будет производить
/// те же события из DOM-листенеров (W6), приложение — единый потребитель.
pub use dragdrop::{DragData, DragEvent};
pub use edgegeom::{
    best_sides, bezier_between, curve_point, curve_tangent, distance_point_to_polyline,
    distance_to_edge, draft_curve, edge_at, edge_curve, edge_endpoint, edge_midpoint,
    edge_polyline, effective_sides, line_port_at, nearest_side, port_at, port_point, retarget_edge,
    route_polyline, side_normal, tessellate, CubicBezier, EdgeEnd, LinePort, AVOID_MARGIN,
    EDGE_HIT_TOLERANCE, MAX_DETOURS, PORT_HIT_PX, TESSELLATION_SEGMENTS,
};
pub use error::CoreError;
pub use expr::{
    Env, EvalError, Expr, ExprLineResults, ExprOutcome, ExprResults, ParseError, Value,
};
pub use flow::{
    creates_value_cycle, inbound_slots, inbound_slots_with_lines, outputs_display, param_spills,
    propagate, propagate_with_lines, propagate_with_lines_data, substitute_spilled_lines,
    topo_sort, unmapped_inputs, unmapped_inputs_with_data, value_path, CycleError, DataSnapshots,
    FlowKind, FlowOutputs, FlowSolutions, LineOutputs, ParamSpill, UnmappedInput, WhatIfOverrides,
};
pub use focus::{focus_set, FocusSeed, FocusSet};
pub use fs_events::{
    apply_file_events, normalize_path, path_matches, relative_if_inside, resolve_node_path,
    watched_dirs, FileEvent, NodeChange,
};
pub use io::{CanvasStorage, FsCanvasStorage, MemStorage};
pub use layout::{
    plan_related_layout, LayoutMode, LayoutPlan, LEVEL_GAP, RADIAL_RING_STEP, SIBLING_GAP,
};
pub use model::{
    enclosing_group_indices, group_add_children, group_children, group_expand_to_children,
    group_materialize_children, group_remove_child, parent_index, plan_push_out, subtree_ids,
    Canvas, CanvasdeskExt, Edge, EdgeLineStyle, EdgeThickness, Node, NodeKind, PreviewState, Side,
};
pub use providers::{
    ClipboardBackend, MemWidgetState, NoopClipboard, NoopThumbs, NoopWatch, PreviewProvider,
    Priority, ShellIntegration, ThumbBackend, Thumbnail, ThumbnailProvider, WatchBackend,
    WidgetStateBackend,
};
pub use search::{
    IndexEntry, MemSearch, SearchBackend, SearchCommand, SearchEvent, SearchHit, SearchResponder,
};
pub use settings::{
    clamp_onboarding_defers, clamp_port_zone, clamp_snap_coarse_zoom, clamp_snap_sub_zoom,
    clamp_snap_tolerance, next_port_zone, next_snap_coarse_zoom, next_snap_sub_zoom,
    next_snap_tolerance, validated_grid_zoom_thresholds, Corner, GridDensity, GridStyle, Language,
    Settings, SnapAnchor, Theme, COLLISION_GAP, ONBOARDING_MAX_DEFERS, PORT_ZONE_MAX,
    PORT_ZONE_MIN, PORT_ZONE_PRESETS, SNAP_COARSE_ZOOM_DEFAULT, SNAP_COARSE_ZOOM_MAX,
    SNAP_COARSE_ZOOM_MIN, SNAP_COARSE_ZOOM_PRESETS, SNAP_SUB_ZOOM_DEFAULT, SNAP_SUB_ZOOM_MAX,
    SNAP_SUB_ZOOM_MIN, SNAP_SUB_ZOOM_PRESETS, SNAP_TOLERANCE_MAX, SNAP_TOLERANCE_MIN,
    SNAP_TOLERANCE_PRESETS,
};
pub use spatial::{SpatialIndex, WorldRect};
pub use validate::{has_errors, validate, IssueCode, Severity, ValidationIssue};
pub use whatif::{
    active_line_exprs, scenarios_from_canvas, scenarios_to_canvas, validate_scenario, Scenario,
    StaleOverride,
};

// --- FR-036: тестовая песочница ------------------------------------------
/// Корень временных каталогов для тестов крейта. Нативно — системный temp
/// (поведение тестов не меняется); под wasm32 — относительный каталог
/// внутри предоткрытого CWD (runner wasmtime маппит корень пакета на `/`),
/// потому что `std::env::temp_dir` на wasm-таргетах паникует: std для
/// wasm32-unknown-unknown/wasip1 её не реализует.
///
/// Единственное исключение из правила «без cfg(target_arch) в core»
/// (wasm-port.md §3.1): хелпер живёт в `#[cfg(test)]`-коде и на продуктовые
/// сборки не попадает. Решение зафиксировано в ADR-0011.
#[cfg(test)]
pub(crate) fn test_scratch_root() -> std::path::PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::temp_dir()
    }
    #[cfg(target_arch = "wasm32")]
    {
        let dir = std::path::PathBuf::from(".wasi-scratch");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }
}
