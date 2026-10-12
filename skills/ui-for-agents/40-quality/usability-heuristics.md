# Usability Heuristics

> **Read when:** reviewing or auditing any UI (REVIEW-UI), or choosing between interaction designs. **Section:** 40-quality. **Related:** 30-ux-patterns/forms-validation-ux.md, 30-ux-patterns/microcopy.md, 20-components/navigation-patterns.md, 30-ux-patterns/onboarding-first-run.md, visual-qa-protocol.md

**Core idea:** Nielsen's 10 heuristics restated as concrete, UI-checkable statements, plus two interaction laws (Fitts, Hick) that turn "it feels crowded" into geometry. Use them as a review instrument: each finding gets a severity from the scale below, so audits are triageable, not vibes.

## Rules

### R1. Keep system status visible
**Rule:** The interface continuously answers: what is happening, what just happened, where am I — via progress indicators, current-location markers, and pending-state feedback.
**Why:** Users plan their next action from perceived state; invisible state forces guessing, and guessing produces duplicate submissions and abandoned flows.
**Example:** A wizard shows "Step 2 of 4" with a filled stepper; the active nav item is marked; a submitted form flips to "Sending…" within 0.1 s.

### R2. Match the system to the real world
**Rule:** Use the user's vocabulary in labels, errors, and navigation — never internal jargon (table names, state machines, developer concepts).
**Why:** Users map interface words onto concepts they already hold; jargon forces translation, and translation errors become wrong choices ("what is a 'record purge'?").
**Example:** "Delete file", not "Purge entity"; checkout says "Delivery address", not "Shipping object".

### R3. Offer an obvious way out
**Rule:** Undo, Cancel, Esc, and Back are reachable from every state; destructive or hard-to-reverse actions support undo after the fact.
**Why:** An exit converts fear into exploration — users who can back out try things; trapped states produce support tickets and data-avoidance behavior.
**Example:** Esc closes the modal and returns focus to its trigger; a deleted row gets a 5-second "Undo" toast instead of a confirm dialog.

### R4. Be consistent internally and follow platform standards
**Rule:** Same function = same look, same word, same place, everywhere; platform conventions (copy shortcut, Esc semantics, back gesture) are followed, not reinvented.
**Why:** Users import knowledge from the platform and from the product's own screens; every inconsistency taxes learning and makes correct behavior look suspicious.
**Example:** "Delete" is always the red trash action with the same confirmation pattern; a custom shortcut never overrides copy.

### R5. Prevent errors instead of formatting nice messages
**Rule:** Constrain input (types, ranges, masks, disabled invalid options), confirm only truly destructive actions, and make the dangerous path the harder one.
**Why:** The best error message is the error that never happened — prevention removes the failure while messages only soften it; over-confirming trains reflex clicking.
**Example:** A date picker leaves out-of-range months unselectable; "Delete workspace" requires typing the workspace name, "Rename" asks nothing.

### R6. Recognition over recall
**Rule:** Make options, objects, and allowed values visible; the user must not memorize codes, commands, or facts from a previous screen to act.
**Why:** Recognition draws on perception (cheap, reliable), recall on memory (slow, error-prone); every hidden-but-required fact multiplies mistakes and re-lookups.
**Example:** The payment form re-displays the selected plan while billing; recently used templates are listed rather than requiring a name from memory.

### R7. Serve novices and experts with the same UI
**Rule:** Provide sensible defaults and clear guided paths for first-time use, plus accelerators (shortcuts, bulk actions, recents) that stay invisible to beginners.
**Why:** Novices need guidance, experts need speed — separated tools fork the product, while layered accelerators serve both in one interface.
**Example:** A command palette and shortcuts exist alongside the menu-driven path; forms arrive pre-filled so most users just press "Next".

### R8. Minimalism: every element competes for attention
**Rule:** Remove or demote anything that does not serve the current task — irrelevant information is not neutral, it is competition.
**Why:** Attention is the scarce resource; each extra element lowers the salience of the ones that matter (see 00-fundamentals/visual-hierarchy.md), so clutter is a measurable usability cost, not a taste issue.
**Example:** The settings page shows five common options; eleven rare ones live under "Advanced" — not a flat page of 16 equal rows.

### R9. Errors: what happened, why, how to fix
**Rule:** Error messages state the problem in user terms, the cause, and a concrete next step — never blame the user. (Deep dive: 30-ux-patterns/forms-validation-ux.md)
**Why:** Users read errors to act, not to be informed; what+why+fix converts a dead end into a path, and blame triggers defensiveness and abandonment.
**Example:** "Password needs at least 12 characters — yours has 8" beats "Invalid credentials", and both beat "Error 0x8007".

### R10. Help is task-oriented, searchable, and next to the feature
**Rule:** Documentation answers tasks ("how do I export?"), is searchable, and is reachable from the feature it explains; no feature exists only in the manual.
**Why:** Help used at the moment of confusion must be found in seconds; help that requires leaving the flow is used once and then never again.
**Example:** A "?" next to the API key field opens "How to get an API key", not the documentation homepage.

### R11. Fitts's law: size and distance are interaction design
**Rule:** Make primary targets large and near the point of use; keep destructive targets small and far from their victims; group related controls within the same movement.
**Why:** Pointing time grows with distance and shrinks with target size — geometry decides error rates before styling matters; adjacent same-sized buttons cause mis-hits precisely when stakes are high.
**Example:** "Send" is a large button at the flow's end position; "Discard draft" is a low-emphasis text link separated by space, never glued to "Send".

### R12. Hick's law: every choice costs decision time
**Rule:** Reduce visible choices to what the step needs; disclose the rest progressively (advanced sections, stepwise flows, smart defaults).
**Why:** Decision time grows roughly logarithmically with option count — and non-linearly when options are similar; defaults and staged disclosure remove most of that cost.
**Example:** The export dialog asks format first; codec/bitrate/quality appear only behind "Advanced" — not 12 controls up front.

## Severity scale for review findings

Rate every finding so results can be triaged:

| Severity | Name | Meaning | Action |
|---|---|---|---|
| 0 | Cosmetic | Polish only; no effect on task success | Backlog |
| 1 | Minor | Slows or mildly confuses; users recover quickly | Fix when touching that area |
| 2 | Moderate | Recurring friction or a required workaround | Fix before next release |
| 3 | Major | Blocks some users or common paths; frequent errors | Blocks release |
| 4 | Catastrophe | Blocks task completion, data loss, or excludes users (a11y) | Blocks ship; fix immediately |

A violation without a user-impact story is a 0. Two independent minor findings on the same flow usually compound to a 2.

## Checklist
- [ ] Status and location visible at all times (progress, step, active nav, pending states)
- [ ] Vocabulary audit done: no internal jargon in user-facing strings
- [ ] Every state has an exit: Esc / Cancel / Back / Undo reachable
- [ ] Same function = same name, look, and position; platform conventions respected
- [ ] High-risk inputs constrained at the source; only truly destructive actions confirm
- [ ] No flow requires memorized codes or facts from a previous screen
- [ ] Accelerators exist for frequent users without burdening novices
- [ ] Error messages carry what + why + fix, with no blame
- [ ] Help reachable from the feature and task-oriented
- [ ] Findings rated on the 0–4 severity scale with triage actions

## Anti-patterns
- Confirm dialogs on every destructive action (training reflex clicks) while undo is missing
- "Oops! Something went wrong" as the entire error message
- A settings page of 40 flat options because grouping "takes time"
- Invented glyphs and shortcuts overriding platform conventions
- Hidden features users must memorize ("type /export to export")
- Primary and destructive buttons glued together with equal size and weight
- Help as a PDF link on the login page and nowhere else
- An audit that lists violations without severities — not triageable; redo it

## Sources
- Nielsen Norman Group — 10 Usability Heuristics: https://www.nngroup.com/articles/ten-usability-heuristics/
- Laws of UX — Fitts's law: https://lawsofux.com/fittss-law/
- Laws of UX — Hick's law: https://lawsofux.com/hicks-law/
- Nielsen Norman Group — Error-message guidelines: https://www.nngroup.com/articles/error-message-guidelines/
- Apple HIG — Controls and platform conventions: https://developer.apple.com/design/human-interface-guidelines
