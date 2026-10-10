---
name: docs-links
description: Single sources of truth for repository facts, link-naming rules, and the doc_lint gate — plus the mandatory onboarding/user-docs closing question for FR/CR work. Load before reading or editing any repository document and before finishing any FR/CR.
version: 1
---

# $docs-links

Documentation map and rules distilled from [AGENTS.md](../../../../AGENTS.md)
(sections "Documentation — single sources of truth" and "Mandatory question
for every FR/CR") and the lint contract of
[scripts/doc_lint.py](../../../../scripts/doc_lint.py).

## When to load

- Before any task: to pick the right source document instead of trusting memory.
- Before editing any `*.md`: link-naming rules and the doc_lint gate apply.
- When finishing an FR/CR implementation: the mandatory closing question below.

## Single sources of truth (one fact — one owner)

| Area | Role |
|---|---|
| `docs/SPEC.md` | The specification: stack, data model, rendering, shell integration, widgets, risks, milestone readiness criteria. Read before any task. (Wave-2 translation; canonical regardless of language.) |
| `docs/TASKS.md` | Infrastructure wave plan (T0–T22, M1–M5). Order within a milestone is strict. |
| `docs/adr/` | Architecture decision records. Index `docs/adr/README.md`; template `docs/adr/adr-template.md`. An ADR is written BEFORE implementing the decision; an "accepted" status is set only on the owner's direct request (ADR-0001, rule 2). |
| `docs/change-requests/` | FR/CR documents per `docs/change-requests/cr-template.md`. Index `docs/change-requests/index-cr-fr.md` holds ACTIVE entries only (in progress / analysis / awaiting acceptance / draft); completed ones live in `index-cr-fr-archive.md` — do not read the archive without a task. On completion: update the status in the document, the document's Changelog, and `docs/ACCEPTANCE.md` (standing methodology §1–12; dated waves in `docs/ACCEPTANCE-archive.md`). |
| `skills/` | The MCP skill package for external AI agents — a published derivative of the tool registry, NOT these workflow skills. The tool counter has a single source: `skills/README.md` (consult it, not memory). Any change to MCP tool set/semantics (`TOOLS` in `crates/canvas-mcp`, `crates/canvas-scene/src/mcp.rs`) must update `skills/` in the same commit — the `skills_*` contract tests fail CI on drift. Protocol: `skills/UPDATE-PROTOCOL.md`. |
| `docs/plans/product-roadmap.md` | The product roadmap (accepted 2026-09-18 with ADR-0008): waves 0/A/B/V/S, the demand gate "≥ 5 external users built a model themselves and returned a second time", mapping of M0–M6 and CR-013 R1–R5 to waves. Check the order of FR/CR modeling work and new dependencies against it. |
| `docs/RECIPES.md` | Desktop-embedding recipes R1–R17 (+ source-file list). Mandatory before T15–T18 and any shell-integration task. |
| `docs/DEMO.md` | Recipe for a quick demo stand on Linux (Xvfb + lavapipe + `scripts/xdemo.py`). Read before recording any demo material. |
| `docs/WASM-TESTING.md` | WASM UI testing recipe (levels L0–L3, scenarios, oracles). Read before the first WASM UI check. |
| `docs/translation-guide.md` | RU→EN rulebook for translation waves: scope rules, canonical terminology, do-not-translate list (language policy 2026-10-10, issue #24). |

## Link-naming rules (must keep)

1. Within the repository, links to md sources are relative `*.md` only.
2. `*.html` links are allowed only for real on-disk artifacts (e.g.
   `docs/prototypes/*.html`) and inside `user-docs/` (FR-031 viewer; link
   check in the `docs_ui` tests). Linking an md SOURCE through `.html` is
   forbidden.
3. Links to built Pages pages use absolute web paths.
4. Verify against these documents, not your own memory — APIs change and
   undocumented Windows behavior differs across OS versions (SPEC §7.4: the
   Progman/WorkerW/DefView hierarchy in Win11 24H2+ differs from the classic
   one).

## What doc_lint enforces

1. `*.md` link targets must exist, resolved relative to the linking file.
2. `*.html` links from `user-docs/` must have the `*.md` source next to them
   (FR-031 mechanism).
3. `*.html` links outside `user-docs/` must exist on disk.
4. `http(s)`, `mailto:`, anchors and absolute web paths are skipped.
5. Relative directory links in `.github/PULL_REQUEST_TEMPLATE.md` resolve
   from the repository root (GitHub template behavior).

## Gates

- After EVERY documentation edit: `python3 scripts/doc_lint.py` — 0 errors
  required. CI job `docs-lint` runs the same command (rule introduced
  2026-10-10, issue #18).

## Mandatory closing question for every FR/CR

When finishing ANY FR or CR implementation, explicitly ask the owner whether
onboarding and user documentation need updating. The question goes into the
task's final report; the owner's silence does not cancel the need.

1. Onboarding (`crates/canvas-app/src/onboarding_ui.rs`, FR-028): does the
   new functionality change tour steps? Add/reword steps (8±2, "one step =
   one thought"); update the step unit tests and
   `docs/interface-objects/onboarding.md`.
2. User documentation (`user-docs/`, 7 pages; built into the binary by the
   FR-031 viewer via `include_str!`, source
   `crates/canvas-app/src/docs_ui.rs`): are any formulations stale (behavior,
   hotkeys, object names)? Update the affected pages plus the tables in
   `user-docs/README.md` and `user-docs/index.md`. Page-to-page links use
   relative `*.html` only (the `docs_ui` link check fails CI on broken
   ones). Keep hotkeys in sync with the `HOTKEYS` list
   (`crates/canvas-app/src/lib.rs`, F1 overlay) and `user-docs/hotkeys.md`.

Docs/onboarding changes may be a separate commit at the owner's discretion;
the question does not imply automatic edits without confirmation.

## References

- [MAP.md](../../MAP.md) — task type → documents → skill → gates routing table.
- `docs/translation-guide.md` — translation terminology and scope rules.
- `skills/UPDATE-PROTOCOL.md` — how the MCP skills package is kept in sync.
