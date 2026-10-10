# CanvasDesk MCP tool catalog — 43 tools

The full registry of the CanvasDesk MCP server's tools, by group. The
canonical source is the response of `tools/list` from the live server
(the `TOOLS` registry in `crates/canvas-mcp/src/lib.rs`); when the text
and the schema disagree, the schema wins. The skill package is
synchronized with this catalog (the `skills_sync` contract test in
`canvas-mcp`, see [UPDATE-PROTOCOL](../../UPDATE-PROTOCOL.md)).

Notation: `?` — an optional parameter; alternatives are separated by `|`;
types — string / number / boolean / integer / null. Calls are shown in
the canonical form `<tool>` {params}.

## Exploration and navigation (10)

| Tool | Signature | Purpose |
|---|---|---|
| `canvas_info` {} | — | Canvas summary: node and edge counts, path to the .canvas file, application version |
| `nodes_list` {text?} | text: boolean | Node list: id, type, coordinates, sizes, label, file; the text field — only when text=true |
| `node_get` {id} | id: string | One node by id with all fields (including text) |
| `nodes_search` {query} | query: string | Node search: case-insensitive substring over text/title/label/file (title — the explicit canvasdesk.title heading, FR-072) |
| `edges_list` {} | — | All edges: {id, from, to, kind, fromLine?, fromOutput?, toParam?, fromSide, toSide} — reconstructing the graph topology |
| `edge_get` {id} | id: string | One edge by id — the schema is the same as for edges_list elements |
| `template_list` {} | — | Template registry: id, name, version, category, expr, params, outputs (named outputs for fromOutput) |
| `schemes_list` {} | — | Gallery of built-in schemes: {id, name, name_en, category, version, description, nodes, edges} — RU-primary |
| `viewport_get` {} | — | Viewport center in world coordinates and the zoom |
| `viewport_set` {x, y, zoom?} | x, y: number; zoom: number | Set the viewport center and optionally the zoom |

## Nodes: creation and editing (8)

| Tool | Signature | Purpose |
|---|---|---|
| `node_create_note` {x, y, text?, title?, width?, height?} | x, y: number | Create a note node (a Numi sheet); returns the id. Default 260×120; height smaller than the content is clamped to the minimum (FR-081). title (FR-072, wave 1) — the explicit card heading; without it the header shows the «—» placeholder, the first text line does not leak into the header (legacy nodes migrate on load) |
| `node_create_file` {path, x, y, width?, height?} | path: string | Create a file node by path (the file is NOT created on disk) |
| `node_update_text` {id, text} | id, text: string | Replace the text of a note node in full |
| `node_edit` {id, text?, title?, label?, color?, expr?, x?, y?, width?, height?} | id: string | Edit ONLY the passed fields; label/color/expr/title = null — reset (title = null — the header shows the «—» placeholder, the first text line does not leak into the header, wave 1); expr — a Numi formula; width/height: height smaller than the content is clamped to the measured minimum (FR-081); returns the updated node |
| `node_move` {id, x, y} | — | Move a node to world coordinates |
| `node_resize` {id, width, height, fit?} | width/height: number > 0; fit: boolean (optional, default false) | Resize a node. Height smaller than the content is NOT applied — it is clamped to the measured minimum (FR-081); fit:true — fit the height exactly to the visible content. Returns {id, width, height, fit_applied, height_clamped, min_height} |
| `node_delete` {id} | — | Delete a node (edges — cascading; group children are NOT deleted) |
| `node_set_color` {id, color} | color: "1".."6" \| null | Node color preset, or null to reset |

## Groups (1)

| Tool | Signature | Purpose |
|---|---|---|
| `group_create` {nodes, label?, padding?} | nodes: array of id, ≥1; label: string (default «Группа»); padding: number 0..500 (default 40) | FR-012 v4 (MCP parity of the UI «Сгруппировать» / Group): wrap the nodes in a new group — the frame is bbox(nodes)+padding, children — an EXPLICIT list. Single-membership invariant: the wrapped nodes are struck out of other groups, the new group becomes a child of the innermost ancestor group, the ancestor chain auto-expands. Groups themselves can be grouped too. Response: {id, label, children, parent\|null, x, y, width, height}. One undo step |

## Layout (1)

| Tool | Signature | Purpose |
|---|---|---|
| `nodes_layout_apply` {mode?, rows?, x?, y?, colGap?, rowGap?, fit?} | mode: grid \| smart (default grid); rows: array of row-arrays of ids; colGap/rowGap: number 0..500 (defaults 80/48); fit: boolean (default true) | FR-081: node arrangement FOR THE AGENT in one call. grid — the “reading order” (a transcription of the layout from a screenshot): columns by max width, rows by max height; groups in rows are forbidden, group frames with moved children auto-expand; response {mode, moved, positions, bbox, groups_resized}. smart — semantic layout of the whole canvas (plan_scheme_layout FR-071), the bbox is preserved. Before layout, heights are clamped to the content (fit, default true). One undo step |

## Edges and flow (4)

| Tool | Signature | Purpose |
|---|---|---|
| `edge_create` {from, to, kind?, fromLine?, fromOutput?, toParam?, fromSide?, toSide?} | from, to: node ids | Create an edge; kind "value" enables the value flow (default "control" — visual); fromLine — a per-line source; fromOutput — a named output; toParam — a spill into a parameter of the receiver |
| `edge_delete` {id} | id: string | Delete an edge by id |
| `flow_set_kind` {id, kind} | kind: value \| control | The edge's flow type; a toggle to value that closes a cycle — an error; recalculation is immediate |
| `edge_ports` {id, pin} | pin: auto \| from \| to \| both | Connection sides: "auto" — release the pins, anything else — pin the current effective sides (WYSIWYG) |

## Batch composition (1)

| Tool | Signature | Purpose |
|---|---|---|
| `graph_apply` {operations} | an array of 1..256 objects with an op field | An atomic all-or-nothing batch: node_create_note, node_create_file, template_instantiate, edge_create, edge_delete, param_set, node_move, group_create (+ ref addressing inside the batch; groups count toward the limit of 128 new nodes); one undo step |

## Templates and schemes (2)

| Tool | Signature | Purpose |
|---|---|---|
| `template_instantiate` {id, x, y, params?} | id: a template id | Create a text node from a template; params — {<name>: number \| {num, unit}}; outside min/max — an error |
| `schemes_apply` {id, x?, y?} | id: a scheme id | Insert a gallery scheme into the current canvas: id remap without collisions, one undo step, response {applied, name, nodes, edges, bbox, flow} |

## Computation and verification (7)

| Tool | Signature | Purpose |
|---|---|---|
| `flow_recalc` {} | — | Value map of the flow of the ACTIVE what-if state: {node_id: {value, unit, outputs, lines, warnings?, spilled?, autoRows?}} |
| `lineage` {node_id, line?} | line: integer \| null | Lineage tree of a number (parity with the chain-check window): {root, nodes[]}, kind calc\|leaf\|cycle\|unmapped\|unlinked\|truncated, via edges |
| `explain_number` {node_id, line?} | line: integer \| null | Explains a number AS TEXT (F-9, PRD-0007): a linear expansion of the tree with addresses and values; response {render: "text", text, root, nodes, truncated} — the bridge returns text as the content text |
| `flow_cycle_check` {} | — | Checks the DAG invariant of value edges: [] — no cycles, otherwise the list of participant ids |
| `graph_validate` {} | — | Model validation: {valid, issues: [{severity, code, node_id, edge_id, message}]} — codes E-CYCLE, E-OVERLOAD, E-UNIT, E-PORT-UNKNOWN, E-DOUBLE-INPUT, W-AMBIGUOUS-SRC, W-UNUSED-SLOT |
| `analyze_bottlenecks` {} | — | Bottlenecks and queue risk of the ACTIVE state: {nodes: [{id, severity, utilization?, queue_length?, wait_sec?, badge}], thresholds} |
| `monte_carlo_run` {runs, params, mode?, seed?, quantiles?} | runs: integer 1..10⁶; params: {"node:param": {dist, mean/sd \| lambda}}; mode: qmc \| mc (default qmc); seed: u64 (default 0); quantiles: [0..1] | FR-066: an MC/QMC run of N ≥ 10⁴ with distributed parameters → quantiles P50/P90/P99 of totals/lines/named outputs + analysis (bottlenecks at the tail P90); dist: normal/lognormal {mean, sd} (the natural space), exp/poisson {lambda}; a parameter — the sheet's “param = …” line; the same seed → the same quantiles (native-only: wasm has no qmc, §5.8) |

## What-if scenarios (9)

| Tool | Signature | Purpose |
|---|---|---|
| `whatif_set_override` {node_id, line, expr} | line: integer ≥ 0 | Per-line override in the active scenario; the mode/scenario are raised automatically |
| `whatif_set_param` {node_id, param, value} | value: string | Sugar for template nodes: finds the “param = …” line itself; no such parameter — an error |
| `whatif_scenario_list` {} | — | {active, whatif_active, scenarios: [{name, overrides, stale}]} — stale = expired overrides |
| `whatif_scenario_create` {name?} | — | Create a named scenario (limit 3), active immediately; frozen into the .canvas, one undo step |
| `whatif_scenario_delete` {name} | — | Delete a scenario (a .canvas mutation, an undo step) |
| `whatif_scenario_activate` {name} | name: «База» \| name | Switch the active scenario; runtime-only, the file is not changed |
| `whatif_deltas` {} | — | Deltas of the active scenario against the base: {node:line \| node:value: {node, line?, base, whatif, delta}} |
| `whatif_apply` {} | — | Write the overrides into the canvas (one undo step), the scenario is deleted |
| `whatif_reset` {} | — | Reset the overrides of the active scenario (runtime); the mode stays active |
