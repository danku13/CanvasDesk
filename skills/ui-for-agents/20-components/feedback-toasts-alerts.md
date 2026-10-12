# Feedback: Toasts, Banners, Alerts

> **Read when:** reporting the outcome of any action or surfacing any system condition to the user. **Section:** 20-components. **Related:** 20/components-catalog.md, 20/modals-and-overlays.md, 30-ux-patterns/empty-loading-error-states.md, 30-ux-patterns/microcopy.md, 40-quality/accessibility-wcag.md, 00/motion-principles.md

**Core idea:** feedback channels differ in lifetime and scope — pick the channel by how long the information must live and where it applies, then write the message with what happened and what to do next.

## Channel selection table

| Channel | Lifetime | Scope | Use when |
|---|---|---|---|
| Inline message | Until resolved | One field/element | Issue tied to that element (validation, local warning) |
| Toast | Auto-dismiss 4–6 s | App/page edge | Confirmation that a completed action succeeded |
| Banner | Until resolved | Page-level | Persistent condition affecting the whole page or section |
| Dialog | Until decided | Task-blocking | Blocking decisions only (see 20/modals-and-overlays.md) |

## Rules

### R1. Pick the channel by lifetime and scope, never by convenience
**Rule:** Match the message to the table above: contextual → inline; transient confirmation → toast; persistent page condition → banner; blocking decision → dialog.
**Why:** Each channel encodes a contract — a toast promises "you may ignore this", a banner promises "this stays until fixed"; breaking the contract either loses information (condition in a toast) or buries the user in permanent noise (confirmation in a banner).
**Example:** "Saved" = toast. "Payment method expired — update it" = banner. "Email already registered" = inline under the field. "Discard changes?" = dialog.

### R2. Never put critical information only in a toast
**Rule:** Anything the user must not miss (errors blocking work, security events, data loss) appears in a persistent channel — banner, inline, or dialog — with the toast, at most, as an additional ping.
**Why:** Toasts auto-dismiss in 4–6 s and stack away; a critical message there is received only by whoever was looking at the right corner at that second — everyone else is silently uninformed.
**Example:** "Your account was flagged" is a banner at the top of every affected page until resolved — the toast (if any) is redundant, not the carrier.

### R3. Prefer an undo toast over confirm-then-do for safe destructive actions
**Rule:** For reversible destructive actions, perform the action and offer "Undo" in a toast (extended duration, e.g. near the 6 s upper bound) instead of a pre-confirmation dialog.
**Why:** Undo shifts the cost of caution from every user (dialog every time) to the rare user who errs (one undo); safety stays identical while friction drops to zero — this only works when the undo is genuinely possible.
**Example:** "Archive" removes the item instantly, toast "Invoice archived — Undo" holds for the full window; the pre-delete of permanent items keeps a real confirmation dialog.

### R4. The visible result is the best success feedback
**Rule:** If the UI already shows the outcome (row appears, status chip flips to "Saved", list updates), do not also toast "Success"; toasts are for outcomes that are NOT visible where the user is looking.
**Why:** A redundant "Saved" toast is pure noise that trains users to ignore toasts — the channel's value collapses exactly when a real, non-obvious confirmation needs it.
**Example:** Editing a detail page that shows "Saved" inline needs no toast; a background export finished while the user is on another page does.

### R5. Toasts auto-dismiss in 4–6 s; errors live in persistent channels instead
**Rule:** Success/info toasts dismiss automatically after 4–6 s; anything requiring user action is never toast-only — it moves to a banner, inline message, or dialog; hover/pause may extend reading but is never required to receive the message.
**Why:** 4–6 s covers reading one short sentence without permanence; longer-lived toasts degenerate into banners with worse placement — if it must persist, it should be a banner from the start.
**Example:** "Report exported" gone in ~5 s; "Export failed: disk full" is a banner with a "Free up space" action, not a 5-second blip.

### R6. Error messages say what happened and what to do next
**Rule:** Every error states the concrete problem and the next actionable step — never blame the user, never show a bare code without a human sentence.
**Why:** "Something went wrong (E-4021)" gives the user nothing to do and forces support contact; "what + next" converts the same failure into a self-served fix, and a calm, user-respecting tone preserves trust (see 30-ux-patterns/microcopy.md).
**Example:** "Upload failed — the file is 60 MB, the limit is 25 MB. Compress it or upload the PDF version." Not: "Error 413."

### R7. Do not stack toasts — queue or aggregate
**Rule:** New toasts replace or queue behind the current one (one visible at a time, 150–300 ms transitions), and repeated same-type events aggregate ("3 files uploaded", not three toasts).
**Why:** Toast stacks obscure the app and each other, arrive out of order, and multiply dismissal noise; aggregation carries the same information in one glance-sized unit.
**Example:** Ten import errors during a batch → one toast "10 items failed — Review" linking to the list, not a ten-layer tower.

### R8. Announce async feedback to assistive tech
**Rule:** Toasts and status messages expose the platform's live-region/status semantics (the web's `aria-live="polite"` / status role concept has equivalents in every toolkit) so completions and errors are announced without focus changes; errors demanding action use the assertive equivalent.
**Why:** Async results arrive with no focus change, so visual-only feedback is invisible to screen-reader users — the announcement is the only delivery path for the outcome.
**Example:** Upload finishing announces "Upload complete" politely; a failed payment announces assertively (see 40-quality/accessibility-wcag.md).

### R9. One consistent position and style for each channel
**Rule:** Toasts appear in one fixed corner and style app-wide; banners at one fixed slot (top of the page or the affected section); inline messages always directly at their element.
**Why:** Users learn where to glance and where to ignore; per-page placement turns the feedback channel itself into a surprise element and weakens every future message.
**Example:** Toasts bottom-right everywhere, banners above the content area, inline under fields — three slots, zero variation per page.

### R10. Banners persist until resolved and always offer the path to resolution
**Rule:** A banner stays while its condition holds, states the condition + action ("Update payment method"), and its dismiss control appears only when the user can legitimately proceed without resolving it.
**Why:** A condition banner dismissed by habit or clutter is lost — the condition still holds; the dismiss button is a promise that ignoring the banner is safe, and it must be kept.
**Example:** "Trial ends in 3 days — Upgrade" keeps its × (user may continue); "Data sync broken — Reconnect" has no × until the user opts to defer explicitly.

### R11. Enter and exit animations stay in the micro-interaction range
**Rule:** Toast/banner enter and exit use quick transitions (150–300 ms) and never block interaction for longer than that; banners appearing do not push content around without an equally quick settle.
**Why:** Feedback animation is for noticing, not for waiting — beyond the range it delays reading and interaction, below it the change is missed; content push that lingers breaks the reading position it just moved (see 00/motion-principles.md).
**Example:** Toast slides in over 200 ms, waits ~5 s, slides out 150 ms; the banner above the table settles within 250 ms, no bounce.

## Checklist
- [ ] Every message mapped to channel by lifetime and scope; no critical info carried only by a toast
- [ ] Reversible destructive actions use undo toasts, not confirm dialogs
- [ ] No redundant success toast when the result is already visible
- [ ] Toast auto-dismiss 4–6 s; action-required items use persistent channels
- [ ] Errors state what happened + what to do next; no bare codes, no blame
- [ ] Toasts queue/replace/aggregate — never stack visibly
- [ ] Async outcomes announced via live-region/status semantics
- [ ] One fixed position per channel across the whole app
- [ ] Banners persist until resolved; dismiss only where ignoring is safe
- [ ] Toast/banner transitions within 150–300 ms

## Anti-patterns
- "Your account has been suspended" as a 5-second toast
- "Saved" toast on a page that already displays "Saved ✓"
- Confirm dialog before an action that has a working Undo
- Ten stacked toasts from one batch operation
- "Error 500 — OK" as the entire failure message
- Permanent banner for a one-off success
- Toast position switching per page (top here, bottom there)
- Silent (unannounced) background completion for screen-reader users

## Sources
- Nielsen Norman Group — Toasts/snackbars: https://www.nngroup.com/articles/toasts-or-snackbars/
- WAI-ARIA APG — Live regions: https://www.w3.org/WAI/ARIA/apg/practices/live-regions/
- Material Design 3 — Snackbars and banners: https://m3.material.io/components/snackbars/overview
- Apple HIG — Alerts: https://developer.apple.com/design/human-interface-guidelines/alerts
- WCAG 2.2 — Timing Adjustable: https://www.w3.org/WAI/WCAG22/Understanding/timing-adjustable.html
