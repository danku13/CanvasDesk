//! bin canvasdesk-mcp-headless (FR-037 MW3, ADR-0012): stdio-MCP-сервер
//! без Windows и GUI — реальная MCP-сессия для внешнего клиента (драйвер
//! mcp_wasm_e2e.py, инспектор MW5, агент) в Linux-контейнере и в wasmtime
//! (wasm32-wasip1). Продуктовый граф бинарников не затронут: лист-крейт.
//!
//! Прогон: `wasmtime run target/wasm32-wasip1/debug/canvasdesk-mcp-headless.wasm`
//! (или нативный бинарник) — initialize/tools_list/tools/call по stdin,
//! ответы по stdout, EOF — штатный выход.

use canvas_mcp_headless::HeadlessSession;

fn main() -> anyhow::Result<()> {
    // Reconnect-хук не нужен: сессия in-process, `is_connected` всегда
    // true (хук актуален только для Windows-pipe, FR-034).
    let reconnect = |_transport: &mut Option<HeadlessSession>| {};
    canvas_mcp::run_stdio_with_transport(Some(HeadlessSession::new()), reconnect)
}
