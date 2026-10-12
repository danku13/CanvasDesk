# Component Catalog — Selection

> **Read when:** choosing which component expresses a given intent — before building, customizing, or inventing anything. **Section:** 20-components. **Related:** 20/buttons-and-actions.md, 20/forms-and-inputs.md, 20/navigation-patterns.md, 20/modals-and-overlays.md, 20/tables-and-data-lists.md, 20/feedback-toasts-alerts.md, 10/component-api-design.md

**Core idea:** every component answers exactly one interaction question. Selection is matching intent to component, not picking what looks nice; two components that answer the same question must not coexist.

## Decision table: intent → component

| Intent | Use | Do not use | Details in |
|---|---|---|---|
| Move to another destination | Link | Button | 20/buttons-and-actions.md |
| Perform an action | Button | Link styled as button | 20/buttons-and-actions.md |
| Switch sibling views of one context | Tabs | Top navigation | 20/navigation-patterns.md |
| Move between different destinations | Navigation | Tabs | 20/navigation-patterns.md |
| Switch a local view mode (list/grid) | Segmented control | Tabs | 20/navigation-patterns.md |
| Block the flow until a decision is made | Dialog | Popover | 20/modals-and-overlays.md |
| Auxiliary info/actions, dismissible | Popover | Dialog | 20/modals-and-overlays.md |
| Progressive disclosure in reading flow | Inline reveal | Dialog | 20/modals-and-overlays.md |
| Compare rows on shared attributes | Table | Card grid | 20/tables-and-data-lists.md |
| Scan a stream of similar items | List | Table | 20/tables-and-data-lists.md |
| Browse rich, heterogeneous objects | Cards | Table | 20/tables-and-data-lists.md |
| Independent on/off options, submitted | Checkbox | Switch | 20/forms-and-inputs.md |
| Pick one of ≤5 visible options | Radio group | Select | 20/forms-and-inputs.md |
| Instant on/off effect, commits now | Switch | Checkbox | 20/forms-and-inputs.md |
| Pick one of many (6+) options | Select | Radio group | 20/forms-and-inputs.md |
| Transient feedback on a completed action | Toast | Banner | 20/feedback-toasts-alerts.md |
| Persistent page-level condition | Banner | Toast | 20/feedback-toasts-alerts.md |
| Issue tied to one field or element | Inline message | Toast | 20/feedback-toasts-alerts.md |

## Rules

### R1. Decide navigate vs act before styling
**Rule:** A link navigates to a destination; a button performs an action. Choose by semantics first, then style — never style a link as a button to make an action "look clickable enough".
**Why:** Users build a mental model from behavior, not pixels; an element that looks like the primary action but navigates away (or vice versa) breaks the model exactly where the cost of being wrong is highest.
**Example:** "Open invoice #1042" = link. "Delete invoice #1042" = button. Swapping their styles is still wrong.

### R2. Tabs for siblings, navigation for destinations, segmented control for local modes
**Rule:** Tabs switch between alternate views of the same context; navigation moves between destinations; a segmented control switches the mode of one local view.
**Why:** The three controls look similar but set different expectations — tabs promise "same context, another view" (state persists), navigation promises "a different place". Misuse makes users lose orientation or expect persistence that does not exist.
**Example:** "Overview / Activity / Settings" for one project = tabs. "Projects / Team / Billing" = navigation. "List | Board | Calendar" over one task set = segmented control.

### R3. Dialog blocks, popover assists, inline reveal discloses
**Rule:** Use a dialog only when the flow cannot continue without a decision; use a popover for auxiliary content that should dismiss cheaply; use an inline reveal to disclose content in place, in reading order.
**Why:** A modal freezes context and costs orientation; that cost is only repaid by a blocking decision. Paying it for auxiliary content is a pure tax on every user.
**Example:** "Discard unsaved changes?" = dialog. Date picker from a field = popover. "Advanced shipping options" expanding under the address form = inline reveal.

### R4. Table compares, list scans, cards browse
**Rule:** Use a table when users compare values across items along shared columns; a list when users scan for one item among many similar ones; cards when items are rich objects without a structure worth aligning.
**Why:** Column alignment is the table's entire value — if no comparison happens the alignment is wasted cost, and if items are heterogeneous the forced alignment produces mostly empty cells.
**Example:** Server list with CPU/RAM/status columns = table. Inbox = list. Product catalog with photos and variable attributes = cards.

### R5. Checkbox independent, radio one-of-few, switch instant, select one-of-many
**Rule:** Checkboxes for independent options committed with a submit; radios for exactly one of up to ~5 visible options; switches for settings that take effect immediately; selects for one choice among many.
**Why:** The control encodes the commitment model: checkbox = "batch, then submit", switch = "now", radio = "all options visible for comparison", select = "comparison not worth the space".
**Example:** "Send copy to my email" in a form with a submit button = checkbox. "Dark mode" in settings = switch. "Region" with 12 options = select.

### R6. Toast transient, banner persistent, inline contextual
**Rule:** A toast confirms a completed action and disappears (auto-dismiss 4–6 s); a banner states a page-level condition until it is resolved; an inline message attaches to the field or element it concerns.
**Why:** The channel encodes lifetime and scope. A persistent condition in a toast disappears and is lost; a transient confirmation in a banner becomes permanent noise the user must dismiss forever.
**Example:** "Report exported" = toast. "Your trial ends in 3 days" = banner. "This email is already registered" under the email field = inline message.

### R7. Compose primitives before inventing components
**Rule:** Before creating a new component, compose it from existing primitives (button + popover, input + inline message, list + checkbox).
**Why:** Every new component adds a look, a behavior, keyboard logic, and documentation cost; composition keeps the primitive count low so consistency scales with fewer rules to learn and enforce.
**Example:** "Searchable select" = input + popover + list with keyboard support — not a bespoke widget with its own styles.

### R8. Check existing variants before adding one
**Rule:** Before adding a new variant (size, tone, layout) to any component, verify no existing variant already covers the intent.
**Why:** Redundant variants breed inconsistency: two variants covering 80% of the same cases drift apart in style and behavior, and users pay the ambiguity cost on every encounter.
**Example:** A "danger outline" button variant is redundant if the destructive style exists and placement already separates it from safe actions.

### R9. When two components fit, choose the lighter one
**Rule:** If both a blocking and a non-blocking component satisfy the intent, choose the non-blocking one: inline over popover, popover over drawer, drawer over dialog.
**Why:** Blocking components cost orientation and add dismissal work for every user, including the majority for whom the decision was not actually blocking.
**Example:** "Rate limit reached" is an inline message next to the action, not a dialog the whole team must dismiss.

### R10. One intent, one component, everywhere
**Rule:** Within one product, each interaction intent maps to exactly one component; the same intent must not use different components on different screens.
**Why:** Consistency is learned by repetition; two controls for one intent force per-screen re-learning and signal that the system lacks a decision layer — see 10-design-system/component-api-design.md for enforcing this at the API level.
**Example:** If "select one of many" is a select on the orders page, it is a select on the users page too — not a radio list there.

## Checklist
- [ ] Every interactive element classified first as navigate vs act
- [ ] Tabs used only for sibling views of one context; never for site navigation
- [ ] Dialogs only where the flow cannot continue without a decision
- [ ] Table vs list vs cards chosen by compare/scan/browse intent
- [ ] Checkbox/radio/switch/select chosen by commitment model (submit / one-of-few / now / one-of-many)
- [ ] Toast/banner/inline chosen by lifetime and scope, not convenience
- [ ] No new component invented that existing primitives compose into
- [ ] Each intent maps to exactly one component across the product

## Anti-patterns
- Link styled as a primary button that triggers a destructive action
- Tabs used for top-level site navigation sections
- Dialogs for anything a popover or inline reveal covers (pickers, help, previews)
- Card grid for tabular data users must compare cell by cell
- Switch inside a form that only takes effect on submit
- Radio group with 12 options instead of a select
- Toast carrying a persistent condition ("trial expired")
- A bespoke second component duplicating an existing one

## Sources
- Material Design 3 — Components: https://m3.material.io/components
- Apple HIG — Components: https://developer.apple.com/design/human-interface-guidelines/components
- Atlassian Design System — Components: https://atlassian.design/components/
- IBM Carbon — Components: https://carbondesignsystem.com/components/overview/
- Nielsen Norman Group — Tabs, used right: https://www.nngroup.com/articles/tabs-used-right/
