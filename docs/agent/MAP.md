# CanvasDesk — Documentation Map for Agents

The navigation layer for AI agents (plan v2.1 stage 5, issue #33). AGENTS.md
is the entry point and the law; this map tells you **what to read for which
task** without loading the whole repository. Routing (task type → skill /
brief → gates) — [routes.yaml](routes.yaml).

## Reading discipline

1. Read `AGENTS.md` first — always. Then this map, then the task's route in
   `routes.yaml`.
2. Read indexes, not archives: active entries only
   (`docs/change-requests/index-cr-fr.md`); do not open
   `index-cr-fr-archive.md` or `worklog/archive/` without a task that
   explicitly needs history.
3. Verify against documents, not memory — APIs and OS behavior change (SPEC
   §7.4: Progman/WorkerW in Win11 24H2+ differs from the classic one).
4. One fact — one owner. Volatile numbers (tool counter, file sizes) are read
   from their single source at the moment of need, never quoted from memory
   or from a second copy.

## Task → documents

| Task type | Read (in order) | Skill to load | Mandatory gates |
|---|---|---|---|
| Any task (start here) | `AGENTS.md`, this map, `docs/agent/routes.yaml` | — | — |
| Documentation edit | `docs/agent/skills/docs-links/SKILL.md` | `$docs-links` | `python3 scripts/doc_lint.py` = 0 errors |
| Doc translation RU→EN | `docs/translation-guide.md`, `docs/agent/briefs/translate-one-doc.md` | `$docs-links` | doc_lint + structure 1:1 self-check |
| MCP tool change | `skills/UPDATE-PROTOCOL.md`, `skills/canvasdesk-mcp/SKILL.md` | MCP package skills | `cargo test -p canvas-mcp skills`, wasm gates |
| Model building via MCP | `skills/canvasdesk-model-build/SKILL.md` | MCP package skills | ADR-0005 oracle ±1 % |
| Model verification | `skills/canvasdesk-model-verify/SKILL.md` | MCP package skills | `flow_recalc` / `graph_validate` green |
| What-if scenario work | `skills/canvasdesk-whatif/SKILL.md` | MCP package skills | scenario apply/reset contract |
| Rust UI work | `docs/agent/skills/ui-kit-review/SKILL.md`, `docs/ui-kit.md` | `$ui-kit-review` | kit checklist + WASM self-check |
| Win32 / shell integration | `docs/agent/skills/shell-win32/SKILL.md`, `docs/RECIPES.md` | `$shell-win32` | recipes verified at runtime |
| WASM verification of UI | `docs/agent/skills/wasm-test/SKILL.md`, `docs/WASM-TESTING.md` | `$wasm-test` | `scripts/wasm_gate.sh --check` (+ L2 if env allows) |
| Issue / planning workflow | `docs/agent/skills/issue-workflow/SKILL.md` | `$issue-workflow` | issue first, close after verification |
| Worklog rotation / token accounting | `docs/agent/skills/worklog-rotate/SKILL.md` | `$worklog-rotate` | lossless move, Journal index updated |
| ADR before a decision | `docs/adr/README.md`, `docs/adr/adr-template.md` | — | ADR written BEFORE implementation |
| FR/CR implementation | `docs/change-requests/cr-template.md` + the FR/CR document | `$issue-workflow` | status + Changelog + ACCEPTANCE updated |

## Sources of truth (single owners)

| Fact | Single source |
|---|---|
| MCP tool set + counter | `skills/README.md` (contract test `skills_*` in canvas-mcp) |
| Tool signatures | `skills/canvasdesk-mcp/references/tools.md` |
| Domain terms (node, edge, spill, Numi…) | `CONTEXT.md` |
| Translation terminology + do-not-translate | `docs/translation-guide.md` |
| Agent-workflow terms (brief, gate, budget…) | `docs/agent/glossary.md` |
| Task statuses (FR/CR) | the FR/CR document itself + `index-cr-fr.md` |
| Design tokens | `crates/canvas-core/src/tokens.rs` + `design/tokens/` (parity test) |
| Cross-platform decision table | `docs/plans/M7-crossplatform.md` §3.2 |

## Doc areas — weight class

**Light, read freely** (entry points and indexes):
`AGENTS.md`, `CONTEXT.md`, `docs/agent/*`, `docs/adr/README.md`,
`docs/change-requests/index-cr-fr.md`, `skills/README.md`.

**Heavy, read on demand only** (a task that names them):
`docs/SPEC.md` (read before any task per AGENTS.md — budget ~16k tokens),
`docs/TASKS.md`, `docs/RECIPES.md`, `docs/ui-kit.md`,
`docs/WASM-TESTING.md`, `docs/ACCEPTANCE.md`, individual FR/CR documents,
`docs/adr/adr-*`. Never load an archive
(`index-cr-fr-archive.md`, `docs/ACCEPTANCE-archive.md`, `worklog/archive/`)
without an explicit history task.

**Not translated to English by policy** (owner decision, #24): `user-docs/`
(user-facing, Russian), `worklog.md` (journal, Russian), historical ADRs and
`docs/prd/*` (human review artifacts). Agents read them as-is; terminology —
via `docs/translation-guide.md`.

## When documentation itself is the deliverable

- New FR/CR document → `docs/change-requests/cr-template.md` structure:
  What / Impact / Changes / Tests; register in `index-cr-fr.md`.
- New ADR → `docs/adr/adr-template.md`; write BEFORE implementing;
  `Статус: принято` only on the owner's direct request (ADR-0001, rule 2).
- New agent-facing doc → place under `docs/agent/`, add a row to this map
  and, if it defines a recurring task type, a route in `routes.yaml`.
- Any doc edit → run `python3 scripts/doc_lint.py` before committing (CI job
  `docs-lint` fails on broken links, dead backtick paths and budget
  violations).
