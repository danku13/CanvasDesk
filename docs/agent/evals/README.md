# Agent Eval Set (skeleton)

Plan v2.1 stage 8 (issue #33 — preparation; execution needs live agent runs).
Ten tasks derived from real defects and recurring work of this repository
(2026-10 audit + closed issues #17–#26). Purpose: a per-model quality
baseline (target stack: GLM-5.3, GLM-5.3 flash, GLM-4.7; Zcode and Kimi Code
harnesses) and a regression net for the documentation system itself.

## Protocol

- **Paired crossover** (language A/B, where applicable): each task runs in RU
  and EN variants, order counterbalanced, 2 repeats per cell.
- **Baseline per model**: record pass/fail, gates green, human-judged
  compliance (1–5), tokens in/out, wall time.
- **Environment**: clean clone at a pinned commit; no network beyond GitHub
  API where the task requires it; no prior session memory.
- Scoring is oracle-first: a task passes when its GATE passes, not when the
  output "looks right". Tasks marked `judged` additionally need a reviewer.

## Tasks

| # | Task type | Prompt seed (abridged) | Oracle / gate | Source of the defect class |
|---|---|---|---|---|
| E01 | docs-edit | "Docs reference `*.html` sources; fix per the link-naming rule" | `python3 scripts/doc_lint.py` = 0 errors; diff only touches stated files | #18 (108 broken links) |
| E02 | docs-edit | "The tool counter in CONTEXT.md disagrees with the registry; fix per the single-source rule" | contract-test rule 5 passes; counter quoted from `skills/README.md` only | drift found 2026-10-10 (42/41 vs 43) |
| E03 | worklog-rotate | "worklog.md exceeds its budget; rotate per the skill" | lossless move (entry counts equal), Journal index updated, doc_lint 0 errors, budget green | #23 |
| E04 | docs-translation | "Translate `docs/<file>.md` RU→EN per the translation-guide" | structure 1:1 (headings/fences/tables/IDs), fences byte-identical, doc_lint 0 errors | #24 waves 1–2 |
| E05 | mcp-change | "Add MCP tool `foo_bar` to the registry and sync the package" | `cargo test -p canvas-mcp skills` green (catalog, coverage, counter, call positions); UPDATE-PROTOCOL steps followed | skills_sync contract |
| E06 | ui-rust | "Add a settings row with a custom color; make it pass review" | review checklist: no inline rgba, no magic numbers, KitPalette slot added first; clippy `-D warnings` | hardcode audit, G7/G8 |
| E07 | shell-win32 | "Port recipe R4 (WorkerW detection) to a new panel" | runtime detection + fallback present; no SPI_SETDESKWALLPAPER on raised desktop; comment links the recipe | RECIPES R4/R9 |
| E08 | model-build | "Build reference №1 via a single `graph_apply`" | ADR-0006 oracle values ±1 %; one undo step; `graph_validate` 0 errors | ADR-0005/0006, CP3 |
| E09 | wasm-verify | "Verify a UI change end-to-end on wasm in this environment" | L0 gate run; if L2 impossible — report states it explicitly + manual instructions (skip protocol) | owner directive 2026-09-25 |
| E10 | issue-workflow | "Plan and execute a small improvement end-to-end" | issue exists before code; board status transitions; closing comment with verification facts; Tokens line present | owner directive 2026-10-10 |

## Notes

- E02/E04 double as language A/B probes (RU prompt vs EN prompt on the same
  task) — they feed the M1/M11 metrics from the audit report (tiers 1–4).
- E05–E08 require a build environment (cargo) and, for E08, a running
  CanvasDesk with MCP — they are full-environment tasks; run them in the
  owner's harness, not in docs-only sandboxes.
- Results land in `worklog.md` (session entry) and in the owner's calibration
  sheet; after 2 baseline runs per model, extend the set rather than reusing
  solved tasks (contamination rule: an eval task that a model has seen is
  retired).
