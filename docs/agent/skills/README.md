# Workflow Skills (docs/agent/skills/)

Skills that decompose [AGENTS.md](../../../AGENTS.md) into on-demand
modules (plan v2.1 stage 6, issue #33). Loading rule: the root file keeps the
law; the skill holds the detailed procedure. Load a skill when your task type
matches (routing — [../routes.yaml](../routes.yaml), map —
[../MAP.md](../MAP.md)).

**Not the MCP skills package:** the agent-facing package derived from the
MCP tool registry lives in `skills/` (repo root) and is contract-tested
against the `TOOLS` registry. The workflow skills here describe repository
process (docs, worklog, issues, UI kit, Win32, WASM) — they are plain
markdown modules, not part of that package.

| Skill | Covers (decomposed from AGENTS.md) |
|---|---|
| [$docs-links](docs-links/SKILL.md) | Documentation single sources of truth, link-naming rule, doc_lint gate, the mandatory FR/CR onboarding/user-docs closing question |
| [$worklog-rotate](worklog-rotate/SKILL.md) | Worklog rotation (period, Journal index, lossless archive), per-task token accounting and the Tokens line format |
| [$issue-workflow](issue-workflow/SKILL.md) | GitHub issues + Projects discipline: tasks first, granularity, sub-issues, board, verified closing, scripts/github_tasks.py |
| [$ui-kit-review](ui-kit-review/SKILL.md) | Mandatory canvas-ui kit rules for Rust UI work, forbidden hardcode patterns, kit-extension procedure, review checklist, M5 widget security |
| [$shell-win32](shell-win32/SKILL.md) | Win32/shell integration rules: runtime detection + fallback, RECIPES constraints, clean-room porting |
| [$wasm-test](wasm-test/SKILL.md) | WASM UI self-check order (L0 → native → L2), gate scripts, skip protocol, bundle/inspector tooling |

Versioning: `version` in the frontmatter is bumped on a semantic change of a
skill. When AGENTS.md changes a rule that a skill distills, update both in
the same commit — the entry-point contract test fails CI if the pointer
markers disappear from AGENTS.md.
