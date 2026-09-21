//! canvas-scene — модельное состояние сцены и слой MCP-инструментов
//! (FR-037/ADR-0012): SceneState (канвас + spatial index + история +
//! кэши расчётов) и `mcp_dispatch` (27 инструментов) — платформенно-
//! нейтральные, собираются и исполняются под wasm (гейт FR-036/037).
//! UI-состояние взаимодействия (выделение, drag) и транспорт MCP живут
//! в canvas-app / canvas-mcp.

pub mod mcp;
pub mod measure;
/// FR-049 (PRD-0008 T2): чистый инстансер схем (ремап id, bbox→origin)
/// + oracle-тесты стартового набора.
pub mod scheme_apply;
pub mod scene;
pub mod view;

pub use mcp::{
    mcp_dispatch, mcp_flow_v2, mcp_unwrap_call, DEFAULT_FILE_CARD_H, DEFAULT_FILE_CARD_W,
};
pub use measure::{
    ensure_result_reserve, estimated_result_reserve_height, fit_template_node_height,
    formula_line_indices, install_measured_reserve, wrapped_body_rows,
};
pub use scene::{
    next_free_id, outputs_to_results, seed_canvas, split_formula_lines, whatif_delta_str,
    SceneState, Viewport, AUTOSAVE_DEBOUNCE, MAX_ZOOM, MIN_ZOOM, UNDO_LIMIT,
};
pub use view::{SpillView, WhatIfNode};

#[cfg(test)]
mod tests;
