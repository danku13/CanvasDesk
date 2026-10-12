# Modals and Overlays

> **Read when:** adding any dialog, popover, drawer, or scrim-based layer. **Section:** 20-components. **Related:** 20/components-catalog.md, 20/feedback-toasts-alerts.md, 40-quality/accessibility-wcag.md, 00/motion-principles.md, 30-ux-patterns/empty-loading-error-states.md

**Core idea:** a modal is an interruption tax — it freezes the user's context and must be repaid with a blocking decision; every other need belongs to a lighter overlay.

## Overlay selection table

| Overlay | Blocks flow? | Dismissal | Use when |
|---|---|---|---|
| Inline reveal | No | Toggles in place | Content belongs in the reading flow (advanced options) |
| Popover | No | Esc, outside click, toggle | Auxiliary info, pickers, small action menus |
| Drawer / sheet | Partially | Explicit close, Esc, scrim | Focused multi-step flow; secondary panel over a wide view |
| Dialog (modal) | Yes | Completing the decision (Esc as cancel) | Blocking confirmations, focused creation, critical alerts |

## Rules

### R1. Modal only when the flow cannot continue without a decision
**Rule:** Open a modal for three cases only: blocking confirmation (irreversible or costly action), focused creation (a task with no need for the page behind), and critical alerts. Everything else uses a lighter overlay.
**Why:** A modal stops the user's task, dims their orientation anchors, and adds two dismissals (open + close) — a cost that only pays off when continuing without the decision is impossible or dangerous.
**Example:** "Delete workspace?" = modal. "Preview of attachment" = popover or drawer. "Advanced filters" = inline reveal.

### R2. Default to non-modal; make blocking the exception
**Rule:** Before opening any modal, check the lighter options first: inline reveal → popover → drawer → dialog, and pick the first that works.
**Why:** Non-modal layers let users keep their context, compare, and multitask; the blocking dialog is the most expensive tool in the set and its habitual use trains users to dismiss without reading.
**Example:** "Session about to expire" uses a banner with a "Stay signed in" action — not a modal the whole team must individually dismiss.

### R3. Focus enters on open, is trapped while open, returns to the trigger on close
**Rule:** On open, focus moves into the overlay (dialog: its first interactive element or the title); Tab cycles only within it; on close, focus returns to the element that opened it.
**Why:** Keyboard and assistive-tech users are lost on the dimmed page behind otherwise — invisible focus means silent interaction with elements that are not part of the task; returning focus restores the starting point.
**Example:** Open "Delete invoice?" → focus lands on "Cancel" (the safe action) → Tab cycles Cancel/Delete/Close only → close returns focus to the row's delete button (see 40-quality/accessibility-wcag.md).

### R4. Esc always closes; scrim click is an explicit product decision
**Rule:** Esc cancels the overlay in every case; whether clicking the scrim closes a modal must be decided once per product and applied consistently (dialogs holding entered data often intentionally ignore scrim clicks).
**Why:** Esc is the universal "get me out" key — removing it traps users in the overlay; the scrim click is a data-loss decision ("a stray click discards my form?") that must be deliberate, not accidental-by-framework.
**Example:** Settings modal with a half-filled form: Esc = cancel with confirm; scrim click = ignored (documented). Read-only preview: both close.

### R5. One modal at a time — stacking means the design is wrong
**Rule:** Never stack a modal on a modal (or confirmation-on-confirmation); if a flow seems to need it, redesign the flow into steps, a drawer, or a page.
**Why:** Each stack level multiplies the dismissal sequence and the focus bookkeeping, and most implementations lose focus-return or scroll position at level 2 — the stack smell indicates the flow was never designed, only extended.
**Example:** "Delete invoice?" needed inside "Edit invoice" modal → close edit first, or make deletion a separate route — not a modal over a modal.

### R6. Destructive confirmations state the consequence, not "Are you sure?"
**Rule:** Confirmation copy names what will happen and what it costs: object count, irreversibility, side effects; buttons repeat the outcome verb ("Delete 3 invoices"), never generic Yes/OK.
**Why:** Users dismiss confirmations on autopilot; only a concrete consequence ("This cannot be undone") is new information at decision time and gives the pause a generic prompt cannot buy.
**Example:** "Delete 3 invoices? This cannot be undone. Drafts will be kept." — buttons "Delete invoices" (danger) / "Keep invoices". Copy rules: 30-ux-patterns/microcopy.md.

### R7. Size follows content; overflow scrolls inside
**Rule:** Modal size fits its content with standard padding; long content scrolls inside the modal body while the header (and action bar) stays fixed.
**Why:** A giant modal loses the "focused task" frame and hides the actions off-screen; a fixed header keeps the decision title visible while the user reviews the long body.
**Example:** 40-item list inside a confirmation: body scrolls, "Cancel / Confirm" always visible at the bottom.

### R8. Never open a modal from a toast
**Rule:** Toasts and other auto-dismissing elements never trigger dialogs; if a toast needs follow-up, it links or navigates to a surface where the interaction happens.
**Why:** The toast disappears underneath the user (auto-dismiss 4–6 s), so the dialog can close itself or orphan mid-interaction — a stack whose bottom vanishes is unstateable.
**Example:** "Import failed — view details" toast links to the import history page; the details never open as a modal on top of a vanishing toast.

### R9. Popovers stay in the viewport and never cover their trigger's critical content
**Rule:** Popovers flip, shift, or resize to remain inside the viewport and must not obscure the field/button they serve or the content the user is comparing against.
**Why:** A clipped popover loses its action buttons; one that covers its trigger or the value being edited forces the user to close, remember, and retry — an interaction loop that reads as broken.
**Example:** Date picker opens above the field near the screen bottom and flips; its "Today" button stays visible; the selected date under the field is never covered.

### R10. Multi-step focused flows use a drawer or dedicated view
**Rule:** Any focused flow longer than a single decision (wizards, multi-field creation, reviews) uses a drawer/sheet or a separate page — not a modal that grows or chains.
**Why:** Steps need orientation (progress, back, revisited fields) that a modal's fixed frame cannot offer; drawers keep the underlying context visible so users can consult data mid-flow.
**Example:** "New teammate" (invite, role, permissions) = right-side drawer with steps, product list visible behind for reference.

### R11. Announce overlays to assistive tech and label them
**Rule:** Overlays expose the platform dialog/role semantics, are labelled by their title, hide background content from reading order while open, and their open/close is announced.
**Why:** Without role and labelling, a screen reader continues reading the dimmed page and the user never learns a decision is pending — the modal is invisible exactly when the stakes are highest.
**Example:** Dialog titled "Delete invoice?" announces "Delete invoice?, dialog" on open; background is inert until closed.

## Checklist
- [ ] Every modal justified by a blocking decision, focused creation, or critical alert
- [ ] Lighter alternative (inline/popover/drawer) considered and rejected explicitly
- [ ] Focus moves in on open, is trapped, returns to trigger on close
- [ ] Esc always closes; scrim-click behavior documented and consistent
- [ ] No stacked modals anywhere in the product
- [ ] Destructive confirmations state consequence + count, verbs on buttons
- [ ] Long content scrolls internally; header/actions stay fixed
- [ ] No toast ever opens a modal
- [ ] Popovers reposition in-viewport and never cover trigger content
- [ ] Overlay roles/titles announced; background inert while open

## Anti-patterns
- Modal for previews, help text, or anything a popover covers
- "Are you sure?" with OK/Cancel as the entire message
- Confirmation modal stacked on a creation modal
- Scrim click discarding a half-filled form without warning
- Popover clipped by the viewport, its buttons unreachable
- Modal taller than the viewport with no internal scroll
- Toast spawning a dialog, then vanishing beneath it
- Focus left on the page behind the open dialog

## Sources
- WAI-ARIA APG — Dialog (Modal) pattern: https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/
- Nielsen Norman Group — Modal & non-modal dialogs: https://www.nngroup.com/articles/modal-nonmodal-dialog/
- Nielsen Norman Group — Confirmation dialogs: https://www.nngroup.com/articles/confirmation-dialog/
- Material Design 3 — Dialogs: https://m3.material.io/components/dialogs/overview
- Apple HIG — Alerts and action sheets: https://developer.apple.com/design/human-interface-guidelines/alerts
