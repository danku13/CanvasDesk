---
name: canvasdesk-whatif
description: What-if analysis on CanvasDesk via MCP — “what if” scenarios without editing the model file: whatif_set_override, whatif_set_param, the deltas of whatif_deltas, scenarios (create/activate/delete), whatif_apply/whatif_reset. Use when you need to answer a question about the model's sensitivity without changing the base. Triggers: what-if, scenario, sensitivity, «а что если», deltas, override parameter.
version: 1
---

# What-if: “what if” scenarios

This skill answers questions like “what if traffic ×2?” — value
overrides live in a scenario and do NOT change the model file until you
explicitly apply them. Requires a connected MCP server (the
`canvasdesk-mcp` skill); the model must be built (the
`canvasdesk-model-build` skill).

**The discipline rule:** a scenario is a temporary tool for answering a
question. The model's permanent assumptions are changed with
`param_set`/`node_edit` (the build skill), and the what-if is left
unapplied — the file remains byte-for-byte the same.

## Overrides

- `whatif_set_override` {node_id, line, expr} — overrides the line
  `line` of the node's text (0-based numbering; lines of the form
  `имя = значение` (name = value)): the original is replaced by the
  expr expression for the duration of the scenario. The base is not
  mutated. The mode and scenario are raised automatically (the implicit
  «Сценарий MCP» / “MCP Scenario”) if nothing has been created.
- `whatif_set_param` {node_id, param, value} — sugar for template
  nodes: it finds the “param = …” line in the node's text itself and
  builds the override from the value. No such parameter — an error.
- Cascade: overriding a starting assumption recalculates the entire
  downstream (an override beats a toParam spill, a spill beats the
  local value — Р-1).

## Scenarios

- `whatif_scenario_list` {} → {active, whatif_active, scenarios:
  [{name, overrides, stale}]} — active = the name of the active
  scenario («База» / Base = no overrides); stale — the number of
  expired overrides (the node/line was deleted or the line became
  prose; the recalculation skips them).
- `whatif_scenario_create` {name} — a named scenario (limit 3),
  active immediately; it is saved into canvasdesk.whatif inside
  .canvas — one undo step. Without name — the default name.
- `whatif_scenario_activate` {name} — switching «База» (Base) ↔ a
  scenario; runtime-only, it does not touch the file, the canvas is
  recalculated with the overrides.
- `whatif_scenario_delete` {name} — delete a scenario (an undo step).
- `whatif_reset` {} — reset the overrides of the ACTIVE scenario
  (runtime); the mode stays active.

## Deltas and checking

`whatif_deltas` {} → {active, deltas: {"node:line"|"node:value":
{node, line?, base, whatif, delta}}} — the same “was → became (+Δ)”
pairs the user sees on the canvas. The reference cascade A→B→C with the
override `a = 5 → 20`: `{A: 5→20 (+15), B: 10→40 (+30), C: 11→41 (+30)}`.

The reading tools (the `canvasdesk-model-verify` skill) show the active
state: `flow_recalc`, `lineage`, `analyze_bottlenecks` take the active
scenario's overrides into account. When answering the user's scenario
question, quote the numbers from `whatif_deltas`/`flow_recalc` — they
match the canvas (MCP visibility = UI).

## Apply and rollback

- `whatif_apply` {} — WRITE the active scenario's overrides into the
  persisted lines/params of the canvas (one undo step), the scenario
  is deleted, «База» (Base) switches to the new values. Only on an
  explicit “commit” decision.
- Rolling back a scenario without writing: `whatif_reset` {} (a reset
  of the overrides) or `whatif_scenario_activate` {name: "База"}
  (return to the base, the scenario's overrides are kept).
- Undo (the user's Ctrl+Z) rolls back apply/create/delete in full.

## A typical session

1. `whatif_scenario_create` {name: "Пик ×2"} (or simply the first
   override — the scenario will be raised on its own).
2. `whatif_set_param` {node_id, param: "dau", value: "2000000"} —
   an override of the assumption.
3. `flow_recalc` {} and/or `analyze_bottlenecks` {} — what resulted
   (check against `whatif_deltas`: base → whatif (+Δ)).
4. The answer to the user: the deltas at the points of interest, the
   bottlenecks under the new load.
5. By default — `whatif_scenario_activate` {name: "База"}, or leave the
   scenario active IF the user is looking at it right now.
   `whatif_apply` {} — only if the user asks to commit.

## Limitations and errors

- A limit of 3 scenarios per canvas (`whatif_scenario_create` beyond
  that — an error; delete the extra one with `whatif_scenario_delete`).
- An override of a non-existent line/node, line < 0, a prose line — a
  call error (isError).
- Expired overrides (stale > 0) are silently skipped by the
  recalculation — check `whatif_scenario_list` after structural edits
  to the model.
