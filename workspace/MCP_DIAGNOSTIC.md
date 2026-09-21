# CanvasDesk MCP Transport — Diagnostic Log

## Skill invocation
User invoked `canvasdesk-mcp` (C:\Users\danku\AppData\Local\hermes\skills\canvasdesk-mcp). Skill references concrete tools: `mcp__canvasdesk__canvas_info`, `node_create_note`, `node_get`, `nodes_search`, `nodes_list`, `edge_create`, `node_move`, `node_set_color`, `flow_set_kind`, `flow_recalc`.

## What's missing (root cause)
The agent framework does NOT expose `mcp__canvasdesk__*` in the model-facing tool list. Only standard tools available: `execute_code`, `terminal`, `browser_exec`, `read_file`, `patch`, etc. There is no bridge from these to the named pipe `\\.\pipe\canvasdesk`.

## Named pipe state
- Spec (crates/canvas-mcp/src/lib.rs): `PIPE_NAME = r"\\.\pipe\canvasdesk"` (Windows-only).
- MSYS bash: `ls "\\.\pipe\canvasdesk"` fails; Windows named pipes are invisible in POSIX path namespace.
- App (`canvas-app`) is NOT running: `ps aux | grep -i canvas` → empty.

## Attempted calls and errors (verbatim)
```
tool_call({"name":"mcp__canvasdesk__canvas_info"}) → "mcp__canvasdesk__canvas_info" is not a deferrable tool
Direct function_call("mcp__canvasdesk__canvas_info") → Tool 'function_call' does not exist
execute_code(python import 'mcp') → module found (hermes venv), but no named-pipe bridge script exists
```

## Source files for coding agent
- MCP server library: `crates/canvas-mcp/src/lib.rs` (JSON-RPC framing, `run_stdio`, `AppTransport` trait)
- MCP binary entrypoint: `crates/canvas-mcp/src/main.rs` (`canvas_mcp::run_stdio`)
- SDK (client-facing): `sdk/canvasdesk.ts`, `sdk/canvasdesk.js`
- Template assets (for architecture model): `assets/templates/com.canvasdesk.*`
- Skill reference doc: `references/corporate-architecture-pattern.md`

## What needs to be done
1. Build/run `canvas-app` (or start its binary) so the named pipe exists.
2. Either register `mcp__canvasdesk__*` functions in the agent framework's direct-tool registry, OR provide a Python/JS script in `sdk/` that opens the named pipe, sends JSON-RPC (newline-delimited, protocol 2024-11-05 or 2025-06-18 per ADR-0009), and returns results to the agent.
3. Confirm `canvas_info` responds with `{"results": ...}` before any node operations.

## Request context (why this matters)
User wants abstract Instagram backend architecture with value-edge (FR-014) auto-flow (`expr` on nodes, `value` edges, `flow_recalc`). Without working MCP bridge, no nodes, edges, or flow propagation can be created/verified.
