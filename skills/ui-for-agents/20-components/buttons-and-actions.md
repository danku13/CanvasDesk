# Buttons and Actions

> **Read when:** adding or styling any button, action row, or command trigger. **Section:** 20-components. **Related:** 20/components-catalog.md, 00/visual-hierarchy.md, 00/color.md, 30-ux-patterns/microcopy.md, 40-quality/accessibility-wcag.md, 00/motion-principles.md

**Core idea:** buttons are the contract between hierarchy and action: one dominant action, everything else visibly weaker, every state designed, and semantics never faked.

## Rules

### R1. Build four style levels and use only them
**Rule:** Primary = filled accent, secondary = outline, tertiary/ghost = text-only, destructive = danger color. Every button on every screen uses one of these four levels.
**Why:** Levels are the visual grammar for importance; a fifth ad-hoc style forces users to re-derive importance per screen and multiplies the styles a design system must keep consistent.
**Example:** "Create invoice" filled, "Save draft" outline, "Cancel" ghost, "Delete" danger — four distinct treatments, no other weights.

### R2. Exactly one primary button per view
**Rule:** One filled/accent button per screen or section; if two sections each have a primary, each is primary only within its section, and the page still has one global primary.
**Why:** Competing primaries paralyze choice and dilute the call to action — users scan for the strongest signal and follow it, so two equal signals halve the odds of the right one (see 00/visual-hierarchy.md).
**Example:** In a "Create invoice" dialog: "Create" filled; "Save draft" and "Cancel" are not filled. Three filled buttons = broken.

### R3. Label with verbs, not noise words
**Rule:** Labels are verb-first and name the outcome: "Create project", "Export CSV" — never "OK", "Yes", "Submit", "Click here".
**Why:** The button label is the last confirmation the user reads before committing; "OK" forces them to re-read the dialog title to know what they agreed to, which fails exactly when it matters most.
**Example:** Destructive dialog reads "Delete 3 invoices?" with buttons "Delete" and "Keep invoices" — not "Yes / No". Copy details: 30-ux-patterns/microcopy.md.

### R4. Separate destructive actions from safe ones
**Rule:** Destructive buttons use the danger color, sit apart from safe actions (other end of the action row or a divider), and are never the visually dominant element unless the view's purpose is that destruction.
**Why:** Distance and color are the two cheapest mistake-proofing cues; adjacency makes slips likely, and a filled danger button beside safe ones invites accidental commits.
**Example:** Settings page: "Save changes" bottom-right primary; "Delete account" bottom-left danger ghost — separated by the full row width.

### R5. Ship every state, or the button is unfinished
**Rule:** Each button implements all states: default, hover, focus, active/pressed, loading, disabled — with the requirements below.

| State | Requirement |
|---|---|
| Default | Label contrast ≥ 4.5:1; boundary visible ≥ 3:1 against adjacent colors |
| Hover | Distinct change; transition within the 150–300 ms micro-interaction range |
| Focus | Visible focus indicator ≥ 3:1 against adjacent colors; keyboard-reachable |
| Active | Confirms the press (shade or 1–2 px offset); instant, no delay |
| Loading | Busy indicator; label retained; width preserved; further clicks ignored |
| Disabled | Visually distinct; reason available (see R7) |

**Why:** Users interact through these states; a missing one is a bug surfaced by the first keyboard user or the first slow network.
**Example:** A button with hover but no focus state is unusable by keyboard — a checklist failure, not a style nit.

### R6. Loading state preserves width and blocks re-submit
**Rule:** While an action runs, swap the label for a busy indicator without changing the button's width, and ignore further activation until done.
**Why:** Width changes shift the layout and move other controls mid-click (mis-clicks), and an unblocked button invites double submission — the classic duplicate-order bug.
**Example:** "Pay now" becomes "Paying…" with a spinner at identical width; second clicks do nothing; on success the result view confirms it.

### R7. Disabled must be explained, not just shown
**Rule:** A disabled button is visually distinct and its reason is available — via tooltip, adjacent helper text, or by replacing it with the helper text itself. Prefer enabling the button and validating on click with a clear message when the reason cannot be discoverable.
**Why:** A silently disabled state is unactionable: users see a dead control and cannot tell whether the page is broken, something is missing, or they lack permission — each demands a different fix.
**Example:** "Invite" disabled with tooltip "Add at least one email address" — or keep it enabled and show that message on click.

### R8. Icon-only buttons must have names and tooltips
**Rule:** Every icon-only button carries an accessible name (platform label/aria-equivalent, not the icon file name) and ideally a visible tooltip; if the meaning is not unambiguous, add text.
**Why:** Icons without names are invisible to assistive tech and unlearnable for newcomers; the tooltip is also the only way sighted users ever discover the meaning.
**Example:** A "⋯" row action exposes "Archive" only on hover — give it a real name; a trash-can icon gets `label: "Delete row"`, not "icon-trash".

### R9. Meet target sizes: 24 px minimum, 44 px touch
**Rule:** Hit areas are at least 24×24 px, and 44×44 px on touch-first surfaces; spacing between adjacent targets prevents cross-activation.
**Why:** Below these sizes, miss rates and mis-taps climb sharply for motor-impaired users and on mobile; the number is a WCAG floor, not a style opinion.
**Example:** A dense toolbar with 16 px icon buttons fails — pad hit areas to 24 px and space adjacent ones so slips don't hit the neighbor.

### R10. Buttons act; links navigate — never cross-dress them
**Rule:** If the element triggers an action it is a button; if it navigates, it is a link. Never style a link as a primary button to trigger an action, and never fake navigation with a button.
**Why:** Semantics drive keyboard behavior, assistive-tech announcements, and user expectations (middle-click, "open in new tab"); crossing them breaks all three at once.
**Example:** "Open dashboard" = link; "Log out" = button (an action, not a destination).

### R11. Asynchronous actions keep context and show progress
**Rule:** Long-running actions show a busy state where the user is — inline progress or a progress view — and never silently navigate away, close, or reset the page.
**Why:** Users need to know the click registered and the app still works; silent transitions read as "nothing happened", causing repeated clicks and abandoned flows.
**Example:** Export with 20 s runtime: button shows loading, then a toast "Export ready" with a link — the page itself never navigates away uninvited.

### R12. Keyboard activation and visible focus are non-negotiable
**Rule:** Buttons activate with Enter and Space, are reachable in reading order, and show the focus indicator of R5 whenever focused.
**Why:** Keyboard is the automation and accessibility path; Space-vs-Enter behavior and a visible ring are what make a button a button across platforms.
**Example:** Tab reaches "Create" → ring appears → Enter activates. An invisible outline (0-opacity focus) is a hard violation (see 40-quality/accessibility-wcag.md).

### R13. Group action sets in one place, in one order
**Rule:** All actions for a context sit in one cluster with a fixed order convention (primary at the end of the reading path; destructive separated per R4); the same order repeats in every dialog and page.
**Why:** Fixed position lets users aim before reading — muscle memory converts to speed; scattered or reordered action rows cause misfires in exactly the high-stakes moments.
**Example:** Every modal in the app: tertiary left, safe secondary + primary right (LTR); destructive always at the opposite end.

## Checklist
- [ ] Exactly one primary button per view/section
- [ ] All labels verb-first; no OK/Yes/Submit
- [ ] Destructive actions danger-colored and spatially separated
- [ ] Loading state preserves width and blocks re-click
- [ ] Every disabled button has a discoverable reason
- [ ] Icon-only buttons have accessible names (+ tooltips)
- [ ] Hit areas ≥ 24×24 px; 44×44 px on touch surfaces
- [ ] Links navigate, buttons act — no cross-dressing
- [ ] Async actions show busy state without losing context
- [ ] Enter/Space activate; focus ring visible ≥ 3:1

## Anti-patterns
- Two filled buttons competing in one view
- "OK" / "Yes" labels on decision dialogs
- Disabled button with no explanation anywhere
- Loading state that shrinks the button and shifts the row
- Double-submit because the button never disabled
- Icon-only buttons with no tooltip and no accessible name
- Focus ring removed "because it looks ugly"
- A link styled as the primary action that deletes something

## Sources
- Material Design 3 — Buttons: https://m3.material.io/components/buttons/overview
- Apple HIG — Buttons: https://developer.apple.com/design/human-interface-guidelines/buttons
- WAI-ARIA APG — Button pattern: https://www.w3.org/WAI/ARIA/apg/patterns/button/
- WCAG 2.2 — Target Size (Minimum): https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html
- Nielsen Norman Group — Flat UI, false affordances: https://www.nngroup.com/articles/flat-ui/
