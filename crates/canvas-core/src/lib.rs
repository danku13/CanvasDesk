//! canvas-core — модель данных канваса и JSON Canvas I/O.
//! Не зависит от ОС и GPU (SPEC §4): вся платформенная логика — за трейтами.

/// FR-016: анализ узких мест и риска очередей — чистая функция над
/// результатами propagator-а (волна B1/CP5).
pub mod analyze;
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
mod settings;
mod spatial;
/// FR-018: реестр шаблонных архитектурных нод.
pub mod templates;
/// M8/W1 (wasm-port §6): монотонное время, безопасное под wasm32 —
/// alias над `web_time::Instant`; нативный код подмены не замечает.
pub mod time;
/// FR-032: валидация модели — чистая функция ядра (коды E-*/W-*).
pub mod validate;
/// FR-017 (CP6): what-if сценарии — именованные построчные подмены,
/// персистентность в `canvasdesk.whatif`.
pub mod whatif;

pub use analyze::{
    analyze, badge_text, has_risk, AnalysisConfig, AnalysisFlags, AnalysisState,
    Severity as AnalysisSeverity,
};
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
    propagate, propagate_with_lines, substitute_spilled_lines, topo_sort, value_path, CycleError,
    FlowKind, FlowOutputs, FlowSolutions, LineOutputs, ParamSpill, WhatIfOverrides,
};
pub use focus::{focus_set, FocusSeed, FocusSet};
pub use fs_events::{
    apply_file_events, normalize_path, path_matches, relative_if_inside, resolve_node_path,
    watched_dirs, FileEvent, NodeChange,
};
pub use layout::{
    plan_related_layout, LayoutMode, LayoutPlan, LEVEL_GAP, RADIAL_RING_STEP, SIBLING_GAP,
};
pub use model::{
    enclosing_group_indices, group_add_children, group_children, group_expand_to_children,
    group_materialize_children, group_remove_child, parent_index, plan_push_out, subtree_ids,
    Canvas, CanvasdeskExt, Edge, EdgeLineStyle, EdgeThickness, Node, NodeKind, PreviewState, Side,
};
pub use providers::{PreviewProvider, ShellIntegration, Thumbnail, ThumbnailProvider};
pub use settings::{
    clamp_onboarding_defers, clamp_port_zone, next_port_zone, Corner, GridDensity, GridStyle,
    Settings, Theme, ONBOARDING_MAX_DEFERS, PORT_ZONE_MAX, PORT_ZONE_MIN, PORT_ZONE_PRESETS,
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
