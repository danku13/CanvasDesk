---
name: canvasdesk-mcp
description: CanvasDesk MCP — connection to the visual mathematical modeling system and canvas exploration. Use when you need to read canvas content (nodes, edges, templates, schemes) via MCP, set up the connection, or pick the right CanvasDesk skill. Triggers: CanvasDesk, canvas, MCP, math modeling canvas, canvas nodes, read canvas, schemes gallery, template registry.
version: 1
---

# CanvasDesk MCP: connection and exploration

CanvasDesk is a visual mathematical modeling system: on an infinite
zoomable canvas, executable models are assembled from computational
nodes (Numi sheets and templates), and values spill along value edges
(a DAG engine with units of measurement). Agent access goes through an
MCP server (43 tools). Key property: **the agent sees exactly what the
user sees** — all value-reading tools return the numbers of the active
what-if state, i.e. precisely those displayed on the canvas.

## Connection (MCP host setup)

The server is the stdio bridge `canvasdesk mcp` (or the standalone
`canvasdesk-mcp`):

```json
{ "mcpServers": { "canvasdesk": { "command": "canvasdesk", "args": ["mcp"] } } }
```

Bridge behavior that matters to the agent:

- The handshake (`initialize`) always succeeds; protocol versions —
  2025-06-18 / 2025-03-26 / 2024-11-05 (the client version is echoed).
- If the GUI application is not running: `tools/call` returns the isError
  «CanvasDesk не запущен» (“CanvasDesk is not running”). The bridge stays
  alive and reconnects on its own (500 ms) before every packet — an
  application started later is picked up without restarting the MCP
  session. Just start CanvasDesk and repeat the call.
- Framing is newline-delimited JSON-RPC 2.0; the tools/call response is
  `content[0].text` with plain JSON + `structuredContent` (the same
  object). Call timeout — 30 s.
- `initialize`/`tools/list`/`tools/call` are JSON-RPC methods; the tools
  below are values of the name parameter in tools/call.
- The canonical catalog of parameter schemas is the live `tools/list`:
  when the skill text and the schema disagree, the schema wins.

## CanvasDesk invariants (a violation = a broken model)

1. **Values flow only along value edges.** A number that can be computed
   from another node must not be typed in by hand (hardcoded) — it must
   spill along an edge with `kind: "value"` (the default `control` is a
   visual edge, the value is NOT carried). Then editing one starting
   assumption recalculates the whole model.
2. **MCP visibility = UI.** `flow_recalc`, `lineage`,
   `analyze_bottlenecks` and the `flow` field of `graph_apply`/
   `schemes_apply` return the active what-if state. Look at the numbers
   from these tools, not “at the base”.
3. **Value priority cascade:** the local parameter value →
   the toParam spill along an edge → the what-if override (the last
   one wins).
4. **Undo discipline:** the `graph_apply` batch, scheme insertion,
   scenario apply — each is ONE undo step. Reading (`flow_recalc`,
   `lineage`, validation) does not touch the undo history.

## Canvas exploration (first calls in a session)

- `canvas_info` {} — what is open: node/edge counts, path to the file.
- `nodes_list` {text: true} — all nodes with texts; `nodes_search` {query}
  — find a node by substring; `node_get` {id} — a single node in full.
- `edges_list` {} — topology: each edge carries from/to/kind and port
  addressing (fromLine/fromOutput/toParam); `edge_get` {id} — one edge.
- `template_list` {} — the template library: parameters (default, min/max,
  unit) and named outputs (consumed by edges' fromOutput). Write down
  the parameter and output names BEFORE assembling the model.
- `schemes_list` {} — the gallery of ready-made schemes (budget, unit
  economics, service capacity): if the task resembles a ready scheme,
  it is faster to insert it and edit than to build from scratch.
- `viewport_get` {} / `viewport_set` {x, y, zoom?} — where the user is
  looking; after inserting a large structure, center the viewport
  on the bbox from the response.

## Skill map: task → skill

| Task | Skill |
|---|---|
| Build/modify a model (nodes, value edges, batch) | `canvasdesk-model-build` |
| Verify numbers, lineage, validity, bottlenecks | `canvasdesk-model-verify` |
| Answer a “what if” question without editing the file | `canvasdesk-whatif` |
| Full catalog of 43 tools | [references/tools.md](references/tools.md) |

## Pitfalls

1. **Multiline text:** a line break in JSON is a real `\n`
   (one control character). The two-character escape `\\n` is normalized
   tolerantly, but the canon is a real line break. Check:
   `flow_recalc` returned as many `lines` as the number of lines you
   intended.
2. **A forgotten `kind: "value"`** when creating an edge — the most
   common mistake: the edge exists, the value does not flow.
3. **Two edges into one toParam** are forbidden (`E-DOUBLE-INPUT`):
   fan-out of load is done with a divider template, not a second input.
4. **Units are checked** (`E-UNIT`): rps will not spill into ms — compare
   the unit of the output and of the parameter via `template_list`
   before creating the edge.
5. **Value edges form a DAG** (`E-CYCLE`): model the “consumer →
   producer” feedback as a break (a queue), not as an edge.
6. **A text source in the batch:** `fromOutput` of a Numi sheet variable
   works with a direct `edge_create`, but in `graph_apply` the source
   is validated against the template snapshot (`E-PORT-UNKNOWN`) — for a
   text source node in the batch use `fromLine` (the number of the
   needed line).

## Next steps

- Model building: the skill `canvasdesk-model-build` (a 7-step recipe,
  the Instagram MVP reference example with oracles).
- Verification: the skill `canvasdesk-model-verify` (the structure of the
  flow response, validation codes, the ±1 % oracle discipline).
- Scenarios: the skill `canvasdesk-whatif` (deltas, apply/reset).
- Signature catalog: [references/tools.md](references/tools.md).
