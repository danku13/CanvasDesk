---
name: canvasdesk-model-verify
description: Verification of the numbers and the correctness of a model on the CanvasDesk canvas via MCP — flow_recalc (the value map), lineage (the lineage tree of a number), explain_number (a ready textual explanation), graph_validate (error codes), flow_cycle_check, analyze_bottlenecks (bottlenecks). Use after building or editing a model, to check against expectations/oracles, and to answer “why is this number like this”. Triggers: verify model, recalculate, flow, lineage, explain number, why this value, validation, bottleneck analysis, overload.
version: 3
---

# Model verification: numbers, lineage, validity

This skill answers three questions: what numbers came out, where they
came from, and whether the model is correct. Requires a connected
MCP server (the `canvasdesk-mcp` skill). All reading tools return the
values of the **active what-if state** — exactly those the user sees on
the canvas (MCP visibility = UI). Reading does not mutate the canvas and
does not touch the undo history.

## flow_recalc — the value map

`flow_recalc` {} recalculates the whole graph and returns, for each node:

```
{ "<node_id>": {
    "value": …, "unit": "…",          // итог ноды
    "outputs": { "<имя>": {"value": …, "unit": …} },   // именованные выходы
    "lines":   [{ "index": 0, "value": …, "unit": …}], // построчные значения
    "warnings"?: […],                  // конфликты значений
    "spilled"?:  { "<param>": { "from": …, "value": … } },  // что пролилось
    "autoRows"?: [{ "slot": …, "edge": …, "path": "Объект.Поле", "value": … }] }}
```

- `outputs` — named outputs of templates AND variables of text nodes'
  Numi sheets (consumed by edges' fromOutput).
- `lines` — per-line values; check their count against the intended
  number of lines (the multiline check).
- `spilled` — the origin of spilled values: from which node into which
  parameter.
- `autoRows` — derived lines of receivers of value edges without
  toParam: the «Объект.Поле» (“Object.Field”) path and the slot value
  (`unmapped` — not substituted).
- **Check the numbers against the model's expectations here, not “by eye
  on the canvas”.** For reference scenarios the tolerance is ±1 % (see
  the `canvasdesk-model-build` skill, the Instagram MVP example).
- The `flow` field in the `graph_apply`/`schemes_apply` response — the
  same format: after a build/insertion a second call is not needed.
- A cycle of value edges — a call error (run `flow_cycle_check` first).

## lineage — the lineage of a single number

`lineage` {node_id, line} — the lineage tree of a number (the same data
as the user's chain-check window): `line` = null/absent — the node's
total, otherwise the index of the Numi sheet line.

The response `{root, nodes: [...]}`: DFS order (parent before child, a
diamond is unfolded); each node — kind: `calc` (computed) |
`leaf` (a constant) | `cycle` | `unmapped` | `unlinked` | `truncated`
(a budget of 4096 nodes), value+unit or error, formula, title, label
(terminals: the name of a variable/input). `children[].via` — the edge
(`edge_id`, `from_node`, `to_node`, `from_line`/`from_output`/
`to_param`) for highlighting the chain.

When to use: the user asks “where does this number come from?” or a
number in `flow_recalc` does not match the expectation — walk down the
tree to the leaves and find the diverging assumption. Prose as the
root, `line` < 0, a non-existent node — isError.

## explain_number — explaining a number in text

`explain_number` {node_id, line?} — a linear expansion of the SAME tree
as `lineage`, ready to be shown to the user / put into a report: each
line is a node with the address `[node_id:строка]`, the value, the
formula and the arrival channel («через выход X» = “via output X”,
«через строку N» = “via line N”, «пролито в параметр P» = “spilled into
parameter P”, «локальная переменная листа» = “a local variable of the
sheet”); the heading — the root with its value; the last line — the
stats «Всего узлов: N (листьев: M)» (“Total nodes: N (leaves: M)”).
Terminals in text: «цикл» (“cycle”), «значение не подставлено» (“value
not substituted”), «не связано: имя» (“not linked: name”), «усечено»
(“truncated”) (a budget of 4096).

The response `{render: "text", text, root, nodes, truncated}`: the
bridge returns `text` as the content text (not a JSON echo) — it can be
quoted verbatim; `structuredContent` duplicates the object for
structured clients. With an active what-if the text starts with the
«Режим what-if» (“What-if mode”) preamble — clarify whether we are
showing the base or a scenario (switching — the `canvasdesk-whatif`
skill).

When to use: the user asks to “explain this number” or “why is it like
this” — one call instead of manually expanding the `lineage` JSON;
for programmatic processing of the tree use `lineage`.

## flow_cycle_check — the DAG invariant

`flow_cycle_check` {} → `[]` (no cycles) or the list of ids of cycle
participants. With a cycle the values are not computed — check BEFORE
comparing numbers.

## graph_validate — model validity

`graph_validate` {} → `{valid, issues: [{severity, code, node_id,
edge_id, message, fix}]}`. **The readiness criterion: `valid: true`**
(no issues with severity "error"). `fix` (FR-077) — a repair recipe for
each code: concrete steps with supported tools (edge_delete, node_edit,
etc.), execute it and re-run the validation. The codes are a stable
contract:

| Code | Severity | Meaning | What to do (briefly; the full version — in `fix`) |
|---|---|---|---|
| `E-CYCLE` | error | a cycle of value edges | break the cycle: value edges form a DAG |
| `E-OVERLOAD` | error | utilization ρ ≥ 1 — the queue grows without bound | raise the node's capacity or reduce the load |
| `E-UNIT` | error | the source's unit is incompatible with the receiver's parameter | compare the units via `template_list` |
| `E-PORT-UNKNOWN` | error | an unknown fromOutput/toParam | port names — from the template manifest |
| `E-DOUBLE-INPUT` | error | two value edges into one toParam | introduce the aggregate as a separate node |
| `W-AMBIGUOUS-SRC` | warning | a multiline source without fromLine/fromOutput | address the source explicitly |
| `W-UNUSED-SLOT` | warning | the $N input is not read by the receiver's formula | the formula does not reference $in/$N |

## analyze_bottlenecks — bottlenecks and queues

`analyze_bottlenecks` {} → `{nodes: [{id, severity, utilization?,
queue_length?, wait_sec?, badge}], thresholds}` — the same flags as the
Ctrl+B overlay at the user's, for the ACTIVE state. severity:
`none` | `warn` | `critical` | `overload`; utilization — ρ (a share
of 0..1, > 1 under overload); wait_sec — the average wait W in base
seconds; badge — the canvas badge string. Default thresholds: ρ 0.7/0.9,
W 100 ms/1 s, queue 1/10. A pure function — the canvas is not mutated.

## monte_carlo_run — quantiles under uncertainty

`monte_carlo_run` {runs, params} — N ≥ 10⁴ runs of the model with
distributed parameters (Monte Carlo / QMC) and the quantiles P50/P90/P99
of the results. It answers questions unavailable to a single run: “with
what probability does the runway go to zero”, “what is the LTV in the
pessimistic scenario” — deeper than the ±20 % what-if grids.

```
monte_carlo_run {
  "runs": 10000,
  "params": {
    "<node_id>:<param>": {"dist": "normal",  "mean": 3000, "sd": 300},
    "<node_id>:<param>": {"dist": "lognormal", "mean": 120, "sd": 15},
    "<node_id>:<param>": {"dist": "exp", "lambda": 0.05},
    "<node_id>:<param>": {"dist": "poisson", "lambda": 10}
  },
  "mode": "qmc",          // дефолт: Sobol (меньшая дисперсия) | "mc"
  "seed": 0,              // дефолт 0: тот же seed → те же квантили
  "quantiles": [0.5, 0.9, 0.99]   // дефолт; P10 → [0.1, ...]
}
```

- A parameter — the “param = …” line of the node's Numi sheet (as whatif_set_param);
  the unit is imparted by the consuming formula («rho = load × 1 %»).
- The response: `{runs, failed_runs, mode, seed, stale, duration_ms, quantiles,
  outputs{node:{P50:{value,unit},…}}, lines{"node:line":{…}},
  named{"node:выход":{…}}, analysis{quantile, nodes, thresholds},
  severity}`.
- `analysis` — bottlenecks (the analyze_bottlenecks format) at the
  TAIL quantile P90: severity escalates on the tail — “sub-critical at
  the median, critical at P90”.
- First-class reproducibility: the same seed — bit-for-bit the same
  quantiles; record the seed in reports. Limits: runs ≤ 10⁶; qmc ≤ 65536;
  poisson λ ≤ 1000. Mutates `canvasdesk.engine` in .canvas (an undo step).
- Available in the native build (the wasm target — without qmc, FR-066 §5.8).

## Verification order after a build/edit

1. `flow_cycle_check` {} — the topology is computable.
2. `flow_recalc` {} (or the `flow` field of the batch response) — the
   check against expectations/oracles ±1 %.
3. `graph_validate` {} — `valid: true`.
4. `analyze_bottlenecks` {} — no overload/critical that you do not know
   about.
5. For a disputed number — `lineage` {node_id, line} — down to the
   source leaf; to answer the user — `explain_number` {node_id, line}
   as ready text.
6. Parameters with uncertainty — `monte_carlo_run` {runs, params}:
   the quantiles P50/P90/P99 and the severity at P90; the seed goes
   into the report for reproducibility.

## Discipline of answers to the user

Report the numbers from `flow_recalc`/`lineage`/`whatif_deltas` (the
active state), not from «базы» (the base) — otherwise the agent and the
user will diverge in numbers. If the user is looking at «База» (the base
scenario) after experiments — first `whatif_scenario_activate` {name: "База"}
(the `canvasdesk-whatif` skill), then a recalculation.
