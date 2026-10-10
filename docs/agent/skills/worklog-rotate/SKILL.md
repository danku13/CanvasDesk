---
name: worklog-rotate
description: Protocol for the repository journal worklog.md — current-period scope, Journal index, lossless rotation to worklog/archive/, and per-task token accounting with the exact Tokens line format. Load when appending or rotating worklog entries or when recording tokens for a finished task.
version: 1
---

# $worklog-rotate

How the repository journal [worklog.md](../../../../worklog.md) is structured,
when and how it rotates to `worklog/archive/`, and how token costs are
recorded per task (owner rules 2026-10-08 and 2026-10-10; rotation rule —
issue #23).

## When to load

- Finishing ANY task (CR/FR implementation, defect fix, document, research):
  the entry must end with a Tokens line.
- Appending a worklog entry; rotating the journal; reading task history.

## Current journal structure

- `worklog.md` holds ONLY the current period (~last 7 days) plus the
  Journal index at the top.
- Entry skeleton (appended at the end, chronological): `---` separator, then
  Task ID / Agent / Task / Work Log / Stage Summary / Tokens.
- Journal index: one line per entry (current AND archived), pipe-separated:
  date | Task ID | scope (short description) | location. The location column
  is `текущий` ("current") or the archive file name.
- Language and ID style: the in-repo journal is Russian (not translated per
  the language policy); documentation-wave entries use scope-style Task IDs
  such as `DOCS-EN-w1`, `DOCS-EN-w2`.
- Distinct fact: the outer multi-agent workspace log (worklog.md at the
  workspace root, OUTSIDE this repository) uses the same entry skeleton with
  plain agent-task IDs (e.g. 4-a). This skill governs the in-repo journal;
  do not mix the two logs up.

## Rotation (rule 2a of token accounting)

- Trigger: the start of a month, or `worklog.md` exceeding ~120 KB.
- Target: `worklog/archive/`, one file per period named
  `worklog-<year-mm-dd>_<mm-dd>.md`. Existing example:
  [worklog-2026-10-02_10-08.md](../../../../worklog/archive/worklog-2026-10-02_10-08.md).

Procedure (lossless move):

1. Count entries in the current worklog and index rows in the Journal index
   (index rows must equal current + archived entries).
2. Move whole entries older than the period into the archive file; entry
   text is NOT edited.
3. Verify counts after the move — before/after entry counts must reconcile
   (gate). Precedent (issue #23, 2026-10-10): 433→119 KB, 46 entries moved,
   Journal index 64 rows = 18 current + 46 archived.
4. Update the location column of the moved rows in the Journal index
   (`текущий` → the archive file name).

Reading discipline: an agent reading the journal reads the index and only
the entries it needs; the archive is not read by default.

## Token accounting (must keep)

1. Every completed task ends with recording the tokens spent (owner rule
   2026-10-08). Data feeds end-to-end development cost tracing: task →
   estimate → actual → calibration of future estimates.
2. Tokens are counted PER TASK (scope — a specific CR/FR/defect/document),
   not per session. Sub-agents report the same way for their tasks.
3. The record goes into `worklog.md`, in the corresponding task's entry
   (the session system journal goes there too, for historical data).
4. Line format (exact):
   `Tokens: in≈<N>, out≈<N>, total≈<N>, model=<models>, scope=<CR-019|FR-100|...>`
5. No direct API counter access — provide a JUSTIFIED ESTIMATE marked
   `estimate`, based on the volume of text read/written (guideline: ~4 chars
   ≈ 1 token for mixed RU/EN/code) plus session-context and tool-call
   overhead.
6. Estimates accumulate in the worklog as historical data used to calibrate
   effort and cost; the per-scope summary is made by the owner or a
   dedicated aggregation task.

## Gates

- Rotation reconciles: entry counts verified before/after; entry text moved
  1:1; Journal index rows = current + archived, locations updated.
- Every completed task's entry ends with a Tokens line in the exact format
  above (marked `estimate` when estimated).

## References

- [AGENTS.md](../../../../AGENTS.md) — section "Per-task token accounting
  (development cost tracing)", rules 1–5 including rule 2a (rotation).
- `worklog/archive/` — one file per period; not read by default.
