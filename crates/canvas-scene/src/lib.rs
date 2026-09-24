//! canvas-scene — модельное состояние сцены и слой MCP-инструментов
//! (FR-037/ADR-0012): SceneState (канвас + spatial index + история +
//! кэши расчётов) и `mcp_dispatch` (28 инструментов) — платформенно-
//! нейтральные, собираются и исполняются под wasm (гейт FR-036/037).
//! UI-состояние взаимодействия (выделение, drag) и транспорт MCP живут
//! в canvas-app / canvas-mcp.

pub mod mcp;
pub mod measure;
pub mod scene;
/// FR-049 (PRD-0008 T2): чистый инстансер схем (ремап id, bbox→origin)
/// + oracle-тесты стартового набора.
pub mod scheme_apply;
pub mod view;
/// FR-064 P1 (ADR-0008 M3): сценарный воркер — desktop-only (на wasm32
/// `std::thread` неработоспособен, контракт плана волны S §5.8); модуль
/// не собирается под wasm, сцена там выполняет sync-пересчёт.
#[cfg(not(target_arch = "wasm32"))]
pub mod worker;

pub use mcp::{
    mcp_dispatch, mcp_flow_v2, mcp_unwrap_call, DEFAULT_FILE_CARD_H, DEFAULT_FILE_CARD_W,
};
pub use measure::{
    ensure_result_reserve, estimated_result_reserve_height, fit_template_node_height,
    formula_line_indices, install_measured_reserve, refit_to_measured_content, wrapped_body_rows,
};
pub use scene::{
    next_free_id, outputs_to_results, read_flow, seed_canvas, split_formula_lines,
    whatif_delta_str, FlowBuffer, SceneState, Viewport, AUTOSAVE_DEBOUNCE, MAX_ZOOM, MIN_ZOOM,
    UNDO_LIMIT,
};
pub use view::{SpillView, WhatIfNode};

/// FR-064 P1: воркер-типы (desktop-only) — приложение спавнит воркер в
/// main() и подключает хэндл сцене.
#[cfg(not(target_arch = "wasm32"))]
pub use worker::{FlowJob, FlowKind, FlowNotifier, FlowOutcome, FlowWorkerHandle};

#[cfg(test)]
mod tests;
