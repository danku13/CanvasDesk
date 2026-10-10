# Agent Glossary (seed)

Terms of the **agent workflow layer** — planning, briefs, gates, budgets.
Domain terms of the product itself (node, card, edge, value edge, spill,
Numi sheet, template node…) live in `CONTEXT.md` — the single source;
translation terminology and the do-not-translate list —
`docs/translation-guide.md`. This file holds only what those two do not.

## Workflow terms

- **Agent contour** — the machine-consumed documentation layer maintained in
  English (owner language policy 2026-10-10, issue #24): `AGENTS.md`,
  `CONTEXT.md`, `skills/`, `docs/agent/`, and the translated machine layer of
  `docs/` (SPEC, TASKS, RECIPES, ui-kit, WASM-TESTING, active
  `index-cr-fr.md`). Contrast: **user contour** (`user-docs/`) and the
  journal (`worklog.md`) stay Russian.
- **Brief** — a self-contained task description handed to a sub-agent; the
  only reliable context channel across the orchestrator→sub-agent boundary.
  Template — `docs/agent/BRIEF-TEMPLATE.md`; accepted briefs —
  `docs/agent/briefs/` (a brief from the library reproduces the result).
- **Gate** — a command or check that must pass (or be explicitly reported as
  not-run with a reason) before a task is reported complete. Examples:
  `python3 scripts/doc_lint.py`, `cargo test --workspace`,
  `scripts/wasm_gate.sh --check`. Routes bind gates to task types in
  `docs/agent/routes.yaml`.
- **Budget** — a size limit on a documentation file, enforced by the doc_lint
  gate (CI job `docs-lint`). Purpose: context economy — an entry file that
  grew past its budget stops being read in full by agents. Current budgets
  live in `scripts/doc_lint.py` (`BUDGETS`), not in prose — do not copy the
  numbers here.
- **Wave** — a batch of related changes delivered and verified as one unit
  (e.g. translation wave 1/2/3 of the agent contour). Waves subdivide into
  per-file tasks with independent self-verification.
- **Task ID** — the global-order identifier of a unit of work (e.g. `2-a`,
  `4-b`); parallel sub-agents are distinguished by suffixes. Required in the
  shared worklog and in briefs.
- **Scope** — what a token-accounting record applies to: a specific CR/FR,
  defect, document, or wave (format: `scope=<CR-019|FR-100|DOCS-EN-w2|…>`).
- **Machine-consumed layer** — documents written to be read by agents as
  instructions/rules (as opposed to human review artifacts). The agent
  contour is its English-speaking subset.
- **Single source (of truth)** — the one file allowed to state a volatile
  fact; every other mention links to it instead of repeating the number.
  Registry of single owners — `docs/agent/MAP.md` §"Sources of truth".
- **Rotate (worklog)** — move entries older than the current period from
  `worklog.md` to `worklog/archive/`, losslessly, updating the Journal index.
  Procedure — `docs/agent/skills/worklog-rotate/SKILL.md`.
- **Contract test** — a Rust test that embeds documentation/markdown via
  `include_str!` and fails CI on drift between code and docs
  (tool registry ↔ skills package; entry-point invariants in AGENTS.md /
  CONTEXT.md).
