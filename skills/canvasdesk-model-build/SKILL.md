---
name: canvasdesk-model-build
description: Building an executable mathematical model on the CanvasDesk canvas via MCP — nodes, value edges with port addressing (fromOutput/fromLine/toParam), the atomic graph_apply batch, scheme insertion, node layout (grid/smart), node grouping. Use when you need to create or modify a model, connect nodes, set parameters, arrange nodes (including from a scheme screenshot), or group nodes. Triggers: build model, create nodes, edges, value flow, graph_apply, param_set, template_instantiate, schemes_apply, nodes_layout_apply, group_create.
version: 4
---

# Building a model on CanvasDesk

This recipe assembles an executable model: numbers are typed in by hand only in
the starting assumptions; everything else is computed and spilled along value
edges. Requires a connected CanvasDesk MCP server (see the `canvasdesk-mcp` skill).

**The main rule:** if a number can be computed from another node — it
must not be typed in by hand. It must spill along a value edge. Editing
one starting assumption then recalculates the whole model.

## Step 1. Exploration: templates and schemes

- `template_list` {} — the library of computational roles (CDN, load
  balancer, DB, queue…). For each: params (default, min/max, unit) and
  outputs (named outputs). Write down the port names — they will be
  needed for edges.
- `schemes_list` {} — the gallery of ready-made schemes. If the task
  resembles «бюджет» (budget), «unit-экономика» (unit economics) or
  «ёмкость сервиса» (service capacity) — it is faster to insert the ready
  scheme with `schemes_apply` {id, x?, y?} and edit it: node ids are remapped
  without collisions, the insertion = one undo step, the response carries the
  bbox (for `viewport_set` {x, y}) and a flow with ready values.
- `canvas_info` {} + `nodes_list` {text: true} + `edges_list` {} — what
  is already on the canvas, so as not to duplicate or conflict.

## Step 2. Nodes

- **Starting assumptions** — a text node via `node_create_note` {x, y,
  text}: lines of the form `имя = значение единица` (name = value unit)
  become variables, lines with formulas
  (`avg_rps = dau × sess × req / 86400`) — computed ones. This is the
  only place where numbers are typed in by hand. The node's variables
  become its named outputs (for fromOutput).
- **Card heading** (FR-072) — `node_create_note` {…, title} /
  `node_edit` {id, title}: the explicit heading lives separately from
  the text and does not change when the body is edited. Without a title
  the header shows the «—» placeholder: the first text line does not
  leak into the header (wave 1); legacy nodes migrate on load (the
  first prose line → title).
- **Computational roles** — `template_instantiate` {id, x, y, params}:
  the template id from `template_list`; params — {<name>: number} or
  {<name>: {num, unit}}; a value outside min/max — an error.
- **File node** — `node_create_file` {path, x, y}: a card linking to a
  file (the file itself is not created on disk).
- Edits: `node_update_text` {id, text} — replace the text in full;
  `node_edit` {id, …} — point edits of ONLY the passed fields
  (text, title, label, color, expr, x, y, width, height; null resets
  label/color/expr/title; expr — a Numi formula, rendered under the
  node's text).

**Multiline text:** a line break in JSON is a real `\n`
(`"text": "dau = 1000000\nsess = 4"`); the two-character variant is
normalized tolerantly. Check: `flow_recalc` returns per node as many
`lines` as the number of intended lines.

## Step 3. Value edges with port addressing

```
`edge_create` { from, to, kind: "value",
                 fromOutput: "<имя выхода>" | fromLine: <номер строки>,
                 toParam: "<имя параметра приёмника>" }
```

- `kind` is required for the value flow: the default `control` is a
  visual edge, the value is NOT carried.
- Source (a direct `edge_create` {…}): `fromOutput` — a named output of
  a template OR a variable of a text node's Numi sheet (the names —
  from `template_list` / outputs in `flow_recalc`); `fromLine` — the
  line number of a text node (0-based, survives line shifts). The
  fields are mutually exclusive; a multiline source without addressing
  → `W-AMBIGUOUS-SRC`.
- Receiver: `toParam` — a template parameter; the spill overrides the
  local value (only with kind "value"). A value edge without toParam
  creates an auto line «Объект.Поле» (“Object.Field”) at the receiver.
- Name validation against template snapshots: an unknown port →
  `E-PORT-UNKNOWN`; an incompatible unit → `E-UNIT`; a second edge into
  the same toParam → `E-DOUBLE-INPUT`; a value cycle → `E-CYCLE`.
- **Replacing the source of an occupied toParam:** the pair
  `edge_delete` {id} + `edge_create` {…} — a second edge into an
  occupied parameter fails with E-DOUBLE-INPUT, so deleting the old one
  is mandatory.
- `flow_set_kind` {id, kind} — toggle the type of an existing edge;
  `edge_ports` {id, pin} — pin/release the sides (pin: auto |
  from | to | both).

## Step 4. Fast path: the atomic graph_apply batch

Do the whole build (nodes + parameters + edges) in ONE call to
`graph_apply` {operations}:

```
{"operations": [
  {"op": "node_create_note", "ref": "traffic", "x": 0, "y": 0, "text": "…"},
  {"op": "template_instantiate", "ref": "cdn", "template": "com.canvasdesk.cdn", "params": {"cache_hit": 0.6}, "x": 400, "y": 0},
  {"op": "edge_create", "fromRef": "traffic", "toRef": "cdn", "kind": "value", "fromLine": 6, "toParam": "rps"}
]}
```

Operations (the op field):

| op | Fields | Note |
|---|---|---|
| `node_create_note` | ref?, x, y, text?, title?, width?, height? | a Numi sheet |
| `node_create_file` | ref?, x, y, path | the file is not created |
| `template_instantiate` | ref?, template, params?, x, y | outside min/max — an error |
| `edge_create` | fromRef\|from, toRef\|to, kind?, fromLine?, fromOutput?, toParam?, fromSide?, toSide? | ports as in step 3, BUT `fromOutput` in the batch is validated only against template outputs: address a text source with `fromLine` (a Numi sheet variable in the batch — `E-PORT-UNKNOWN`) |
| `edge_delete` | id\|ref | edge deletion (for replacing a source) |
| `param_set` | ref\|id, param, value, unit? | edits exactly one “param = value unit” line; no such parameter — an error (append is NOT performed) |
| `node_move` | ref\|id, x, y | moving |

- **ref** addresses nodes created earlier IN THE SAME batch
  (forward-ref → `E-NOT-FOUND`; a duplicate → `E-BAD-OP`).
- **Atomicity:** an error in any operation → `{ok: false, op_index,
  code, message}`, the canvas remains byte-for-byte the same. Success →
  one undo step for the whole batch, a full recalculation, autosave.
- **The success response already contains `flow`** in the
  `flow_recalc` format — a second recalculation call is not needed.
  Check the numbers here (the `canvasdesk-model-verify` skill).
- **Limits:** ≤ 256 operations, ≤ 128 new nodes per call.
- Operation error codes: `E-BAD-OP`, `E-NOT-FOUND`, `E-PORT-UNKNOWN`,
  `E-CYCLE`, `E-PARAM-UNKNOWN`, `E-RANGE`.

## Step 5. Layout and visual fine-tuning

- The preferred path is `nodes_layout_apply` (FR-081): layout in ONE call
  instead of N `node_move` calls:
  - `mode:"grid"` — an explicit “reading order” `rows`: an array of
    rows, each row — an array of ids left to right, rows top to bottom.
    Transcribe the layout from a scheme screenshot (or any layout of
    your own): columns are aligned by maximum width, rows by height;
    gaps `colGap`/`rowGap` (defaults 80/48), the `x`,`y` point — the
    top-left of the block (default — the current bbox of the listed
    nodes). Groups in `rows` are forbidden — lay out their children,
    the frames auto-expand. Response:
    {mode, moved, positions, bbox, groups_resized}.
    `nodes_layout_apply` {"mode":"grid","rows":[["u1","u2"],["calc1"]]} —
    two inputs in the first row, the computation below them.
  - `mode:"smart"` — semantic layout of the WHOLE canvas (FR-071):
    clusters by groups/edges, layers left to right, barycenter,
    grid alignment without “snapping”; the canvas bbox is preserved.
  - `fit` (default true) — before layout, heights are clamped to the
    content: squashed nodes do not break the alignment.
- `node_move` {id, x, y} — a point shift of a single node; `node_resize`
  {id, width, height, fit?} — size to fit the text.
  The FR-081 invariant: height smaller than the content is NOT applied
  — it is clamped to the measured minimum (the node_resize response
  carries height_clamped/min_height); `fit: true` — fit the height
  exactly to the visible content. Text nodes: width ≈ 500,
  height 140–200 (a heuristic; the minimum guarantees readability).
- `node_set_color` {id, color} — presets "1".."6" or null.
- `node_delete` {id} — delete a node (edges cascade; group children
  remain).
- `viewport_set` {x, y, zoom?} — show the result to the user
  (coordinates — from the insertion bbox or the created nodes of the
  batch response).

## Step 6. Grouping nodes

- `group_create` {nodes, label?, padding?} — wrap the listed nodes in a
  new group: the frame from the common bbox + padding (default 40,
  0..500 allowed), children — an explicit list of ids. Response:
  {id, label, children, parent, x, y, width, height}. One undo step.
- Hierarchy semantics (FR-012 v4): if the wrapped nodes are children of
  existing groups, they are struck out of them (the single-membership
  invariant), the new group becomes a child of the innermost ancestor
  group, and the whole ancestor chain auto-expands to accommodate the
  new subgroup. Groups themselves can be grouped too — list their ids
  in nodes.
- The batch `graph_apply` has the same operation with ref addressing —
  the frame is assembled in one undo step with the nodes:
  `{"op":"group_create","ref":"pack","nodes":["a","b"],"label":"Пакет"}`.
  A group counts as a new node toward the limit of 128.

## Reference example: Instagram MVP (ADR-0005)

The complete ready batch (12 nodes, 10 value edges, one call) —
[examples/instagram-mvp.json](examples/instagram-mvp.json). Expected
values after insertion (tolerance ±1 %): avg_rps ≈ 555.6,
peak_rps ≈ 1388.9, origin_rps ≈ 555.6, out_auth ≈ 83.3, out_feed ≈ 333.3,
out_media ≈ 138.9, db_qps ≈ 80, replica_load ≈ 40, consume_rate ≈ 333.3.

Reproducibility check: `param_set`/`node_edit` to `dau = 2000000` →
one `flow_recalc` → the whole chain doubles without editing any edges
or formulas.

## Build-readiness checklist

1. All computable numbers spill along value edges (no hardcoding).
2. The `graph_apply`/`flow_recalc` response matched the model's
   expectations.
3. `graph_validate` → `valid: true` (the `canvasdesk-model-verify`
   skill).
4. Any starting assumption is changed with a single call; downstream is
   recalculated without editing the graph.
5. The model is shown to the user (`viewport_set` to the bbox).
