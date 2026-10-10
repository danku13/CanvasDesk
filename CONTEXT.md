# CanvasDesk Glossary

The project's shared language. Terms only — decisions and implementation
details live in `docs/adr/` and the code. The product is a visual
mathematical modeling system (ADR-0007): computation-core terms are primary.

## Canvas and objects

- **Node** — a canvas element. Types: `file` (a file card), `text` (a note;
  may be a Numi node or a template node), `group` (a container), `widget`
  (a JS/HTML widget, M5). The `link` type from JSON Canvas spec 1.0 is
  preserved on round-trip as unknown and is not used by the app.
- **Card** — the visual representation of a node on the canvas.
- **Node title** — the string in the card header (FR-072): file name / the
  explicit `canvasdesk.title` / the template snapshot name / the first line
  of the note (legacy fallback) / the group caption. The explicit title is a
  separate field, independent of body edits; `canvasdesk.title = ""` means
  set-but-empty (placeholder, no first-line leak).
- **Edge** — a line between nodes; may carry a label, line style, width and
  color. Either a **control edge** (a visual link) or a **value edge**
  (a value flow, `canvasdesk.flow.kind = value`).
- **Group** — the command that creates a group from the bbox of the current
  selection.
- **GFM block** — a block-level markdown element inside a note body: header,
  list, checkbox, quote, code block.

## Computation core (modeling)

- **Numi sheet** — a node's text as a program: assignment lines
  (`rps = 1000 rps`) and expressions evaluated line by line; variables flow
  top-down; prose and code fences are ignored (FR-013).
- **Numi engine** — the parser/eval of the computation core
  (`crates/canvas-core/src/expr/`): numbers with suffixes, dimensioned units
  of measure, functions (`sum`/`avg`/`max`/`min`/`percentile`, domain ones —
  see below).
- **Value flow** — the propagation of values along value edges: the source's
  value is available to the receiver as `$in` / `$1..$N` (positional) or as
  `$parameter` (spilled into a template); the graph is a DAG, cycles are
  blocked; live re-evaluation on every edit (FR-014).
- **Value edge** — an edge carrying a value; rendered turquoise with a live
  value label.
- **Template** — the computational role of a node with a `template.json`
  manifest: `params` (typed parameters) + `expr` (a formula with `$param`);
  62 built-in templates in three categories (infrastructure, unit economics
  `ue-*`, product analytics `pa-*`) + custom (FR-018/019/020/027; the audit
  extension 2026-09-25: 45 → 62).
- **Template node** — a text node with a `canvasdesk.template` snapshot
  (id, version, expr, params, icon, color); the text is a Numi sheet of
  parameter values.
- **Value port** — an entry/exit point of a computation node: a named output
  (the manifest's `outputs` / a Numi sheet line) or a parameter input
  (`toParam`); addressing by name is canonical, the line index (`fromLine`)
  is legacy (ADR-0003, FR-029).
- **Spill (value spill)** — passing a value along a value edge into the
  receiver's parameter input, overriding the local value ("a spill beats the
  default") without editing the formula.
- **Domain functions** — queueing (`mm1`, `mmc`, `utilization`,
  `littles_law`, `erlang_c` — FR-015) and finance (`npv`, `cagr`, `irr`,
  `cohort_ltv` — FR-027); overload detection ρ ≥ 1 → `Overload`.
- **Reference model** — a verifiable scenario that the agent assembles via
  MCP by spilling values: Instagram MVP (ADR-0005), catalog №1–№5 (ADR-0006).

## Integration

- **MCP tool** — a function that an external AI agent (Claude and other MCP
  clients) calls over the MCP protocol to operate the canvas: reading/editing
  nodes, groups (group_create), edges, value flow, templates, the atomic
  `graph_apply` batch, validation with fix recipes, what-if, bottleneck
  analysis, Monte Carlo (42 tools on native: 41 + the native-only
  `monte_carlo_run`; on wasm — 41, T24 + FR-033 + FR-016/FR-017/FR-066/
  FR-077 + FR-012 v4 MCP parity).
- **The `canvasdesk` channel** — a Windows named pipe between the canvas-mcp
  process and a running canvas-app (on Linux/macOS — a UDS, the M7 plan).
