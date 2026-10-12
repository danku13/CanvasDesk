# Onboarding & First-Run

> **Read when:** building sign-up flows, first-run screens, setup wizards, or permission prompts. | **Section:** 30-ux-patterns | **Related:** empty-loading-error-states.md, microcopy.md, ../00-fundamentals/visual-hierarchy.md, ../20-components/buttons-and-actions.md

**Core idea:** the best onboarding is a usable product. Users come to reach value, not to complete a tour; every setup step must pay rent by moving them toward their first real success.

## Rules

### R1. Value before setup
**Rule:** Show the working product before asking for configuration: let the user see, click, and get output before demanding profiles, integrations, or billing.
**Why:** Value experienced first creates the motivation that pays for setup; asking for work before proving worth is the top abandonment point in sign-up funnels.
**Example:** A PDF tool opens on a working "Drop a file here" screen; account creation is requested only when the user saves their result.

### R2. Empty states beat modal tours
**Rule:** Teach through the product surface: empty states with sample data, pre-filled templates, or one clear first action — not a multi-screen modal slideshow about features.
**Why:** Learning by doing sticks; tours interrupt, get clicked through, and are forgotten within minutes, while an empty state teaches at the exact moment and place of need (see empty-loading-error-states.md R4).
**Example:** A project board's first-run view contains a demo card "This is a task — drag me" that the user can actually manipulate, instead of a 5-slide tour.

### R3. Tours: skippable, resumable, never blocking
**Rule:** If a tour exists at all, it must be skippable in one obvious click (hit area ≥ 24×24 px), resumable later from Help, and never hide the interface behind an unexitable modal wall.
**Why:** Forced tours are abandoned and breed resentment before the first value moment; a skip option costs nothing for willing users and saves goodwill for everyone else.
**Example:** The tour has a visible "Skip" in the corner; "Show intro again" lives in Help; the product behind step 1 is dimmed but visible, not hidden.

### R4. Hints appear contextually with dismissal memory
**Rule:** Reveal feature hints on the user's first visit to the relevant screen, anchored to the control, with dismissal remembered per user (not per session) — never show the same hint twice after dismissal.
**Why:** Contextual hints arrive when attention is on the task, so they land; session-only memory causes nagging repeats, which trains users to dismiss everything reflexively.
**Example:** First visit to Reports shows one bubble "Group by month here"; after dismissal it never reappears for that account.

### R5. Multi-step setup as a checklist with partial credit
**Rule:** Setup flows of 3+ steps use a persistent checklist: each completed step stays checked, progress is visible, and the product is usable with partial completion — no all-or-nothing gating.
**Why:** Checklists externalize remaining cost and build momentum; partial credit keeps users who stall on one step (e.g. no admin rights today) active and productive.
**Example:** "Get started: ✅ Create workspace ✅ Invite 2 teammates ⬜ Connect calendar ⬜ Import data — 2 of 4 done". The app is fully usable at 2 of 4.

### R6. Ask permissions in context, with a pre-explanation
**Rule:** Request notifications, camera, files, or contacts at the moment the user first needs that capability — preceded by an in-product explanation of what will be asked and why. Never stack permission prompts at first launch.
**Why:** OS-level prompts cannot be re-shown after denial; a pre-explanation lets the user decline the fake prompt without burning the real one, and context converts refusals into grants.
**Example:** Before the scan screen: "To scan receipts we need camera access — nothing is uploaded without your action. [Continue] [Not now]" → then the OS dialog.

### R7. Measure activation, not tour completion
**Rule:** Instrument onboarding by the first value event (created first project, sent first invoice, imported first file) and time-to-activation — not by tour slides seen or checklist opened.
**Why:** Tour completion measures compliance with your tutorial; activation measures whether onboarding worked. Optimizing the former reliably worsens the latter.
**Example:** Success metric: "60% of new accounts create a project within 24h" as the onboarding KPI; tour completion rate is a diagnostic at most.

### R8. Returning users never see first-run content
**Rule:** All first-run content — tours, hints, sample data, checklists — is keyed to durable per-user state and never resurfaces after dismissal or completion.
**Why:** Repeated onboarding content tells users the product does not listen or remember, and it actively blocks returning users from their task.
**Example:** A user who dismissed the tour on Monday and logs in on Tuesday lands directly on their workspace — no splash, no tour, no re-highlighted features.

### R9. One primary action on first-run screens
**Rule:** The first-run screen has exactly one primary call to action (the fastest path to value); everything else — import, explore demo, skip — is secondary or tertiary.
**Why:** New users have zero context to compare options; competing primaries multiply decision cost at the moment of maximum uncertainty (see ../00-fundamentals/visual-hierarchy.md R1).
**Example:** "Create your first board" is the filled button; "Import from another tool" and "Explore sample board" are quiet links below.

### R10. Sample data is clearly marked and removable
**Rule:** Demo/sample content used in first-run is visibly labeled ("Sample") and can be deleted in one action without touching real user data.
**Why:** Unmarked sample data pollutes real workspaces and destroys trust in every number shown afterward; unremovable samples force users to abandon the account and start over.
**Example:** A sample project shows a "Sample — delete anytime" badge in its header and disappears cleanly via its own menu.

### R11. Ask the use-case question once, personalize once
**Rule:** If the product is broad, ask each new user one early question ("What will you mainly use this for?") and use the answer to tailor the first screen, templates, and default settings. Never re-ask.
**Why:** A single self-declared intent substitutes for weeks of behavioral data and removes irrelevant paths from the highest-uncertainty moment; re-asking signals the answer was ignored.
**Example:** Choosing "Design agency" on signup makes the first screen show client-project templates, not warehouse inventory templates.

### R12. Onboarding state survives interruption
**Rule:** Setup progress, dismissed hints, and tour position persist across sessions and devices; a half-finished setup resumes exactly where it stopped.
**Why:** Interruption is the norm, not the exception; forcing a restart punishes the user for coming back and inflates apparent abandonment.
**Example:** A user who closed the app mid-setup returns to a banner "Finish setup — 2 of 4 done", jumping straight to step 3 on click.

### R13. Teach one concept at a time
**Rule:** Never fire multiple hints, tours, or tooltips simultaneously anywhere in the product; one guidance element owns the screen until dismissed, then the next may appear.
**Why:** Overlapping hints stack into a wall of dismissals where users cancel all guidance (including useful hints) to regain control — the inverse of the intended teaching.
**Example:** The invite hint waits until the import hint is dismissed; queued guidance appears one at a time over the first session.

## Checklist
- [ ] The product is usable and shows its value before any setup request
- [ ] First-run education happens in empty states/sample data; any tour is skippable and resumable
- [ ] No modal tour blocks the UI without an always-visible exit
- [ ] Contextual hints fire once per user with durable dismissal memory
- [ ] Setup >3 steps is a checklist with progress and partial credit
- [ ] Permission prompts are pre-explained and triggered at the moment of need
- [ ] Activation (first value event) is instrumented and used as the onboarding KPI
- [ ] No returning user ever sees first-run content again
- [ ] Sample data is labeled and one-click removable
- [ ] First-run screen has exactly one primary action

## Anti-patterns
- A 5-slide modal tour before the user can touch the product
- Mandatory "Watch intro" with a disabled or invisible skip
- The same tooltip reappearing every session
- Requesting notifications, contacts, and location on first launch
- Setup wizard that gates all functionality behind full completion
- Sample data that cannot be deleted or is indistinguishable from real data
- A 12-field signup form before any value is shown
- Celebrating "tour completion rate" while activation drops

## Sources
- Nielsen Norman Group — Ten usability heuristics (user control, recognition over recall): https://www.nngroup.com/articles/ten-usability-heuristics/
- Apple HIG — Onboarding: https://developer.apple.com/design/human-interface-guidelines/onboarding
- UserOnboard — teardown library: https://www.useronboard.com/
- Growth.Design — onboarding case studies: https://growth.design/case-studies
- Material Design 3 — Foundations: https://m3.material.io/foundations
