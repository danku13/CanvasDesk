# Translation Guide — RU → EN for agent-facing documentation

Owner decision 2026-10-10 (language policy, issue #24): agent-facing
documentation is translated to English and maintained in English. This guide
fixes terminology and rules so that all waves produce a consistent corpus.
Wave 1 (done): `AGENTS.md`, `CONTEXT.md`, `skills/README.md`. Wave 2
(next sessions): `docs/SPEC.md`, `docs/TASKS.md`, `docs/RECIPES.md`,
`docs/ui-kit.md`, `docs/WASM-TESTING.md`, active `index-cr-fr.md`, `skills/`
(scope files).

## Scope rules

1. **Translate:** agent-consumed rule/reference docs listed above.
2. **Do not translate:** `user-docs/` (Russian, user-facing),
   `worklog.md` and `worklog/archive/` (journal, Russian), historical ADRs
   and `docs/prd/*` unless a task requires it, git commit messages (RU
   convention stays), code comments (Russian unless the owner decides
   otherwise).
3. The English version becomes canonical and is edited in English; the
   Russian text lives in git history.
4. Translate meaning 1:1 — do not shorten, do not drop rules, do not
   re-organize sections. Section headers get stable English names (they are
   cited from code comments and other docs).

## Terminology (canonical EN; keep as-is in text)

| RU | EN | Notes |
|---|---|---|
| нода | node | canvas element |
| карточка | card | visual representation of a node |
| связь | edge | generic |
| value-связь | value edge | carries a value (`canvasdesk.flow.kind = value`) |
| контрольная связь | control edge | visual-only link |
| поток значений | value flow | DAG propagation |
| проливание | spill / spill-over | value into a parameter input, beats the default |
| Numi-лист | Numi sheet | node text as a program |
| Numi-движок | Numi engine | parser/eval (`crates/canvas-core/src/expr/`) |
| шаблон | template | computational role with `template.json` |
| шаблонная нода | template node | text node with a `canvasdesk.template` snapshot |
| порт значений | value port | named output / parameter input (`toParam`) |
| расчётная волна | modeling wave | FR-013…FR-029, CR-013 |
| приёмка | acceptance | manual (owner) or automated (gates) |
| чек-лист | checklist | |
| волн(а) | wave | also wave sections in ACCEPTANCE |
| вёрстка | layout | UI layout work |
| налезание / наложение | overlap | layout regression class |
| эталонная модель | reference model | Instagram MVP, catalog №1–№5 |
| реестр инструментов | tool registry | `TOOLS` in canvas-mcp |
| скилл | skill | package under `skills/` |
| ожидает приёмки | awaiting acceptance | status; do NOT archive such rows |

## Do not translate

- File paths, crate names, code identifiers (`canvas-core`, `TextMeasurer`,
  `SPACING_*`, `graph_apply`), shell commands, JSON keys.
- Document IDs: FR-0xx, CR-0xx, ADR-00xx, PRD-00xx, T-numbers, M-numbers,
  R-recipes (R1–R17), gate names (G1–G8), UR-0xx.
- Statuses as recorded in documents/indexes: «реализовано», «выполнено»,
  «в работе», «в анализе», «ожидает приёмки» — keep the Russian token in
  status cells (they are matched by scripts and the archive split), the
  surrounding prose may be English.
- Quotes of owner directives may keep the Russian original in parentheses
  when wording matters (e.g. the demand gate).

## Style

- En dash for ranges (0.25–0.6), em dash for asides.
- Keep markdown structure byte-compatible: same heading levels, same tables,
  same list nesting, same code fences.
- Keep every cross-reference as a repo path; verify it exists
  (`python3 scripts/doc_lint.py` — 0 errors is the gate).
- Numbers, thresholds and identifiers must match the source exactly; when in
  doubt, quote the source file rather than paraphrasing.
- After each translated wave: record a token measurement (o200k, before/after)
  in the worklog and tick the wave checklist in issue #24.
