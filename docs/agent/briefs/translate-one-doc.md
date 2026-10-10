# Brief: Translate one agent-contour document RU→EN

Accepted brief (issue #24, waves 1–2). Reproduces an in-place translation of
a single markdown file of the agent contour, structure-preserving.

Task ID: <assigned>          Repo: /home/z/my-project/CanvasDesk
Base commit: <git sha>

## Objective

`<docs/.../FILE.md>` is fully English, meaning- and structure-preserving
(1:1); the Russian original remains recoverable from git history. The file is
canonical EN after this task — it is edited in English from now on (language
policy, owner decision 2026-10-10).

## Context (read, in order)

1. `docs/translation-guide.md` — canonical terminology, do-not-translate
   list, status-cell rule, style.
2. The source file in full.
3. `scripts/doc_lint.py` — the link-check contract the result must pass.

## Constraints

- Headings: same count, same level sequence, same numbering (cross-refs stay
  valid).
- **Code fences are NOT translated** — byte-identical to git HEAD, including
  Russian comments/strings inside code (verify with a fence-by-fence diff).
- Status cells matched by the guide's status-token rule stay in Russian, with
  an EN gloss where helpful; owner-directive quotes stay verbatim.
- Identifiers (FR-/CR-/ADR-/PRD-/T/M/R/G/AC-numbers), paths, commands, env
  vars, URLs — count-identical before/after.
- Terminology per the guide's table (node/card/edge, value edge/control edge,
  spill, layout, wave, skill…); no synonyms.
- Table shape: same rows/columns; list nesting unchanged.
- Do not commit; no branch switch; leave the working tree for the orchestrator.

## Deliverable

- `<docs/.../FILE.md>` translated in place.
- If the orchestrator asked: part files under `/home/z/my-project/scripts/…`
  for audit.

## Self-verification (run before reporting)

- Line count within ±5 % of HEAD (target 1:1); heading multiset equal;
  fence marks count equal; fenced bodies byte-identical vs `git show HEAD:<path>`.
- Cyrillic scan outside fences: every remaining Cyrillic line is a justified
  exception per the guide (list them in the report).
- `python3 scripts/doc_lint.py` → 0 errors.
- `git diff --stat -- <path>` recorded in the report.

## Report format

Append to `/home/z/my-project/worklog.md`: Task ID / Agent / Task / Work Log
/ Stage Summary + `Tokens: in≈N, out≈N, total≈N (estimate), model=<model>,
scope=<scope>`.
