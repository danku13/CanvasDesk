---
name: issue-workflow
description: GitHub issues + Projects workflow for CanvasDesk improvements — tasks first, then code; issue granularity, sub-issues and board discipline, verified closing with green gates, and the planning tool scripts/github_tasks.py. Load before starting any improvement and before closing an issue.
version: 1
---

# $issue-workflow

Work-planning directive (owner, 2026-10-10), distilled from
[AGENTS.md](../../../../AGENTS.md) ("Work planning: GitHub issues + Projects"
and "Mandatory question for every FR/CR"). The order for any improvement:
**tasks first — then code; an issue is closed only after verification.**

## When to load

- Before the first line of code for any improvement (feature, wave, increment).
- When creating sub-issues, moving items on the board, or closing an issue/sub-issue.

## Procedure

1. Planning = creating tasks. Start with a high-level GitHub issue BEFORE
   the first line of code. Work without an issue does not start.
2. Size the issue: a high-level issue is the atomic unit of improvement (a
   unit of value, not a batch of edits). Do NOT create an issue for every
   intermediate step — cosmetics and subtasks live inside the task body.
3. Split only for parts of the implementation: waves, stages, splits across
   parallel sub-agents (tracked by Task ID) are filed as sub-issues of the
   parent task; progress is aggregated by the Sub-issues progress field on
   the board.
4. Put the high-level issue and its sub-issues on the board (see Board).
5. Implement one wave at a time.
6. Close as a verified fact (see Closing) and post the summary comment.
7. For FR/CR work, finish with the mandatory onboarding/user-docs closing
   question (see the section below and the $docs-links skill).

## Board

- Tasks (high-level and sub-issues) land on Projects #1 (CanvasDesk):
  Todo → In Progress → Done.
- In Progress — one wave at a time.

## Closing = verified fact

An issue/sub-issue is closed only when ALL of the following hold:

- The gates are green, per the task context:
  - `cargo test --workspace`
  - clippy with `-D warnings`
  - fmt
  - WASM gates: `scripts/wasm_gate.sh --check` / `scripts/mcp_wasm_gate.sh`
- The acceptance criteria from the issue body are met.
- UI changes passed the WASM check per the "WASM UI self-check" section of
  AGENTS.md.

On closing — a summary comment: what was done, key commits, how it was
verified.

## Tool: scripts/github_tasks.py

- Commands: `issue create/list/close`, `sub add/list/remove`,
  `project add/status/items`, and `plan` — which creates a high-level issue
  + sub-issues + board entries atomically in one step.
- All commands are idempotent (a re-run does not duplicate).
- Tokens (environment, or flags `--token` / `--project-token`; see the
  script header): `GITHUB_TOKEN` — issues (classic PAT scope `repo`, or
  fine-grained "Issues: Read and write"); `GITHUB_PROJECT_TOKEN` — the
  board (GraphQL; classic scope `project`, or fine-grained "Projects: Read
  and write"); if unset, `GITHUB_TOKEN` is used.
- Defaults: repository danku13/CanvasDesk, board Projects #1 (overrides:
  `--owner` / `--repo` / `--project-num`, env `GH_PROJECT_NUM`).
- Plan command example:

```
GITHUB_TOKEN=ghp_... python scripts/github_tasks.py plan --spec plan.json --dry-run
```

## FR/CR closing question (mandatory, cross-reference $docs-links)

When finishing any FR or CR implementation, explicitly ask the owner whether
onboarding and user documentation need updating. The question goes into the
task's final report; the owner's silence does not cancel the need. Checks:
onboarding tour steps (`crates/canvas-app/src/onboarding_ui.rs`, FR-028) and
the `user-docs/` pages (FR-031 viewer) — the full checklist lives in the
$docs-links skill (`docs/agent/skills/docs-links/SKILL.md`). Docs/onboarding
changes may be a separate commit at the owner's discretion; the question
does not imply automatic edits without confirmation. FR/CR completion also
requires updating the document's status (`реализовано`), its Changelog, and
`docs/ACCEPTANCE.md`.

## References

- [AGENTS.md](../../../../AGENTS.md) — sections "Work planning: GitHub issues +
  Projects" (owner directive 2026-10-10) and "Mandatory question for every
  FR/CR: onboarding and documentation".
- [scripts/github_tasks.py](../../../../scripts/github_tasks.py) — the tool;
  the header documents commands, tokens and defaults.
- [MAP.md](../../MAP.md) — routing rows "Issue / planning workflow" and
  "FR/CR implementation".
