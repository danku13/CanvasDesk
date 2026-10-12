# Empty, Loading & Error States

> **Read when:** building any region that fetches data, renders a list, or can fail — before wiring the happy path. | **Section:** 30-ux-patterns | **Related:** ../40-quality/perceived-performance.md, ../20-components/feedback-toasts-alerts.md, microcopy.md, onboarding-first-run.md, ../20-components/tables-and-data-lists.md

**Core idea:** a UI is judged by its worst moment, not its happy path. Every region has seven states that must be designed like first-class screens: empty, first-run, loading, partial, error, offline, permission-denied.

## Rules

### R1. Design all seven states per region
**Rule:** Every data-driven region ships with designed empty, first-run, loading, partial-content, error, offline, and permission-denied states. A state without a design is a defect.
**Why:** Users spend a large share of their time in non-happy states (first visit, weak network, expired token); unplanned states render as blank boxes or stack traces, which read as a broken product.
**Example:** Invoice list: empty (no invoices + CTA), loading (skeleton), partial (3 of 5 pages loaded, "Retry page 4"), error, offline ("Showing cached invoices from Tue"), denied ("Ask your admin for access").

### R2. Skeleton when layout is known and wait > 1s; spinner for short or unknown waits
**Rule:** If the final layout is predictable and the wait will exceed ~1 second, show a skeleton. For short or unpredictable waits, use a small spinner. Apply the response tiers: 0.1s — no indicator needed; up to 1s — spinner; 1–10s — skeleton or progress; beyond 10s — progress with ETA and a way out (see R11).
**Why:** A skeleton communicates structure ("cards are coming") and shortens perceived wait; a spinner communicates only activity and, shown for long waits, makes latency feel worse than showing nothing.
**Example:** Dashboard widgets (>1s, known grid) render gray card skeletons; a theme toggle (<0.1s) shows nothing; an avatar upload (unknown, ~1s) shows a tiny inline spinner.

### R3. Skeleton must match the final layout shape
**Rule:** Skeleton blocks mirror the size, count, and position of the real content blocks — same card heights, same row count, same image placement.
**Why:** A mismatched skeleton guarantees a layout jump on load, which discards the user's reading position and negates the skeleton's entire benefit.
**Example:** A list that will show 8 rows of 56px gets a skeleton of 8 gray rows of 56px — not one big gray rectangle.

### R4. Empty states teach and route to action
**Rule:** Every empty state has three parts: what lives here, why it is valuable, and one clear action that creates the first item. Never a dead blank region.
**Why:** First-time emptiness is the user's only chance to learn the region's purpose; a blank screen transfers zero knowledge and exits the user into nothing.
**Example:** "No invoices yet — bill clients in minutes. [Create your first invoice]". A bare region with only column headers and no rows is broken.

### R5. Errors: what happened, what's preserved, concrete retry
**Rule:** Error states state the failure in plain words, state what happened to the user's data, and offer one concrete next step (Retry, Edit card, Contact support) — never a bare code or "Oops".
**Why:** The user's two questions after a failure are "is my work lost?" and "what now?"; answering both converts panic into a single decision.
**Example:** "Couldn't save your changes — your draft is safe on this device. [Retry] [Copy error details]".

### R6. Optimistic UI only with a defined rollback
**Rule:** For quick, likely-to-succeed actions (like, rename, toggle) update the UI immediately and define the rollback for failure: revert + message. For payments, deletions, or anything irreversible, wait for confirmation.
**Why:** Optimistic updates make the interface feel instant, but an undefined rollback turns a failed like into a permanently wrong state and destroys trust in every other instant update.
**Example:** Clicking "Follow" flips the button at once; if the request fails, it flips back with "Follow failed — Retry".

### R7. Long waits show progress or ETA
**Rule:** Waits beyond 2–4 seconds show real progress (bar, count "3 of 10 files", or ETA). Never fake progress — an indeterminate bar pretending to be determinate is worse than an honest spinner.
**Why:** Above ~2s an unquantified wait feels unbounded and users start guessing whether it hung; quantified waits are rated shorter and users stay put (full playbook in ../40-quality/perceived-performance.md).
**Example:** Export dialog: "Exporting 1,240 rows… 640 (52%) — about 30s left".

### R8. Offline and denied states offer the path back
**Rule:** Offline states say what still works and offer retry/reconnect; permission-denied states explain why access is needed and offer the request path (re-auth, request access, open settings).
**Why:** Dead-end states ("You are offline." / "Access denied.") strand users; the path back keeps the session recoverable and converts a blocker into a step.
**Example:** Offline: "You're offline — showing cached notes. Edits will sync when you reconnect." Denied: "Camera access is needed to scan. [Allow camera]".

### R9. Fail regionally, not globally
**Rule:** A failure in one region (widget, tab, panel) degrades only that region; the rest of the page stays visible and interactive.
**Why:** Whole-page error screens waste every region that succeeded and block unrelated work; regional isolation bounds the damage and preserves orientation.
**Example:** A failing chart widget shows its own retry button inside its card; the KPI row beside it is unaffected.

### R10. Partial states are explicit
**Rule:** When only part of the data loaded, say exactly what is missing and why, and provide a targeted retry for the missing piece.
**Why:** Silent partial data is the most dangerous state — it looks complete and drives wrong decisions; explicit gaps preserve trust in the numbers that did load.
**Example:** "12 of 15 devices reporting — 3 offline sensors excluded. [Refresh]".

### R11. Waits over ~10s stay cancellable and recoverable
**Rule:** Any operation that can exceed 10 seconds offers a way out: Cancel, or "run in background" with a notification on completion.
**Why:** Past 10s users stop believing the wait and start planning escape; a provided exit keeps them in the product instead of them killing the tab or process.
**Example:** Import dialog at 10s+ gains "Run in background — we'll notify you"; cancel leaves partial imported rows clearly marked.

### R12. Show stale data now, refresh in the background
**Rule:** On revisit, render cached/stale content immediately with a freshness marker ("Updated 5 min ago") and refresh in the background; reserve skeletons for never-loaded regions.
**Why:** An instant, slightly-stale answer beats a blank wait for most revisits; hiding data the user already has behind a spinner withholds information for no gain.
**Example:** Opening a dashboard shows last-loaded values with "Updated 5 min ago" and a subtle refresh; only first-visit widgets show skeletons.

### R13. Keep the user's place across retries
**Rule:** After a failed load and a retry, restore the region's scroll position, expanded rows, and any draft input the user had; a retry resumes, it does not reset.
**Why:** Losing position on retry makes the user pay twice for one failure — once for the outage, once for re-navigation — and long lists make the second cost the worse one.
**Example:** A 400-row table fails on page 3; after "Retry", the user is back at their row with the expanded detail row still open.

## Checklist
- [ ] Every data region has designed empty, loading, and error states (plus offline/denied where applicable)
- [ ] Skeletons used only when layout is predictable and wait > 1s; spinner otherwise; tiers 0.1s / 1s / 10s applied
- [ ] Skeleton geometry matches final content geometry (no jump on load)
- [ ] Empty states name the content, its value, and one creating action
- [ ] Error messages give plain cause + data status + one concrete retry path
- [ ] Waits > 2–4s show real progress or ETA; no fake progress anywhere
- [ ] Optimistic updates have a defined, tested rollback
- [ ] Operations over 10s are cancellable or move to background
- [ ] One region's failure never blanks the whole page
- [ ] Partial data is labeled with what is missing and why

## Anti-patterns
- A blank region while data loads (no skeleton, no spinner)
- Skeleton of one gray blob for a card grid that loads as 12 varied cards
- "No data." as the entire empty state
- "Oops, something went wrong" with no retry and no data status
- Full-page error screen for a single failed widget
- Infinite spinner on a request that already failed
- Faked determinate progress bar that resets
- "You are offline." with no cached view and no retry

## Sources
- Nielsen Norman Group — Response times, the 3 important limits: https://www.nngroup.com/articles/response-times-3-important-limits/
- Nielsen Norman Group — Progress indicators: https://www.nngroup.com/articles/progress-indicators/
- Apple HIG — Loading: https://developer.apple.com/design/human-interface-guidelines/loading
- Ant Design — Empty component guidance: https://ant.design/components/empty
- Material Design 3 — Foundations: https://m3.material.io/foundations
- WCAG 2.2 — Status Messages (4.1.3): https://www.w3.org/TR/WCAG22/#status-messages
