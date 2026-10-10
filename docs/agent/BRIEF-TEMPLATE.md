# Brief Template (for sub-agents)

A **brief** is the single reliable channel of context from the orchestrating
agent to a sub-agent (plan v2.1 stage 5; verified in practice — sessions
2026-10-10). The sub-agent does not see the orchestrator's conversation;
everything it needs must be in this file or in the repository paths it lists.

Rules: be specific to the point of pedantry — exact paths, exact line ranges,
exact acceptance checks. Accepted briefs are copied into
[briefs/](briefs/README.md) as few-shot examples (the library rule: a brief
from the library reproduces the result).

---

```markdown
# Brief: <one-line task title>

Task ID: <global order id, e.g. 2-a>          # for the shared worklog
Repo: <absolute path to the local clone>
Base commit: <git sha the sub-agent starts from>

## Objective
<what must exist when the task is done — the end state, not the steps>

## Context (read, in order)
1. <path> (lines <N>-<M>) — <what to take from it>
2. <path> — <what to take from it>

## Constraints
- <hard rules: language policy, style, do-not-touch lists, byte-exact zones>
- <what NOT to do — explicit anti-goals>

## Deliverable
- <file(s) to create/modify — exact paths>
- <what stays untouched (e.g. "do not commit", "no branch switch")>

## Self-verification (run before reporting)
- <command or check> — expected result
- <structural check vs baseline> — expected equality

## Report format (append to /home/z/my-project/worklog.md)
Task ID / Agent / Task / Work Log (concrete steps) / Stage Summary +
Tokens: in≈N, out≈N, total≈N (estimate), model=<model>, scope=<scope>
```

---

## Field notes (why each field exists)

| Field | Reason |
|---|---|
| Task ID | The shared worklog (`/home/z/my-project/worklog.md`) aggregates parallel sub-agents by ID; without it the log is unmergeable. |
| Base commit | The sub-agent verifies it sees the same content the orchestrator planned against; guards against a moved main. |
| Context with line ranges | Cuts the sub-agent's reading budget to the relevant span; line ranges are checked against the base commit. |
| Constraints with anti-goals | Failures cluster on unstated assumptions; name them (e.g. "code fences stay byte-identical", "do not translate status cells"). |
| Self-verification | The sub-agent proves correctness before reporting; the orchestrator re-runs the same checks instead of trusting prose. |
| Report format | Uniform worklog entries; the `Tokens:` line feeds per-scope cost tracing (owner rule 2026-10-08). |
