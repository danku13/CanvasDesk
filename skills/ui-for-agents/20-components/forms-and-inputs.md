# Forms and Inputs

> **Read when:** building any form — sign-up, settings, filters, checkout. **Section:** 20-components. **Related:** 20/components-catalog.md, 20/buttons-and-actions.md, 30-ux-patterns/forms-validation-ux.md, 30-ux-patterns/microcopy.md, 30-ux-patterns/touch-and-mobile.md, 40-quality/accessibility-wcag.md, 00/spacing-layout-grids.md

**Core idea:** a form is a conversation with one question per row; every field must be self-explanatory, keyboard-complete, and stable while it is being answered.

## Rules

### R1. Labels are always visible, above the field
**Rule:** Every field has a persistent visible label placed above it (or left of it in dense desktop forms) — never only inside the field.
**Why:** The label is the question the field answers; when it lives inside the field it disappears on first input, so the user must remember it while answering — a working-memory tax on exactly the task they are doing.
**Example:** "Email" sits above the input; after typing "anna@example.com" the question is still on screen.

### R2. Placeholder is not a label
**Rule:** Placeholders may show a format hint ("DD-MM-YYYY") or an example, never the field's identity; nothing required is stored only in the placeholder.
**Why:** Placeholders vanish on input, fail contrast requirements when mimicking values, and break autofill review — users cannot check what a field was while scanning the filled form.
**Example:** Label "Phone", placeholder "+47 900 00 000" — correct. Label-less field with only "Phone" inside — broken (see 30-ux-patterns/forms-validation-ux.md for validation interplay).

### R3. Mark optional fields, not required ones
**Rule:** Mark the minority: fields that are optional get an "(optional)" tag; unmarked fields are required by default.
**Why:** In a mostly-required form, marking required repeats noise on every label while optional fields — the real decision points — go unnoticed; the inverse marking scans cleaner and shorter.
**Example:** Sign-up: "Email", "Password", "Company (optional)", "Phone (optional)" — two tags, no asterisks to decode.

### R4. Use the correct input type and completion hints
**Rule:** Each field declares its content type on the platform: keyboard layout, autofill category, and format expectations — the web's `type`/`inputmode`/`autocomplete` attributes have equivalents in native toolkits; use them.

| Content | Field configuration |
|---|---|
| Email | email keyboard; autofill `email` |
| Phone | telephone/numeric keyboard; autofill `tel` |
| Amount, quantity | numeric keyboard; explicit decimal handling |
| Postal code | numeric keyboard; autofill `postal-code` |
| Address | autofill `street-address` (single logical field preferred) |
| Date | date control + typable alternative (see R11) |
| Search | search semantics; explicit commit (Enter/button) |

**Why:** The right type summons the right keyboard and enables autofill — removing typing and typos is the single biggest form-speed win; wrong types silently cost every mobile user.
**Example:** A numeric-only "ZIP" field with the full text keyboard and no autofill is a defect, not a nit.

### R5. Reserve one slot for helper and error text
**Rule:** Helper text sits above the error position; the space for that line is reserved so that a message appearing never shifts the layout; only one message is visible per field at a time.
**Why:** Layout shift while typing moves the very field the user is aiming at and pushes buttons away from muscle memory; competing helper/error lines in one slot overwrite each other and lose content.
**Example:** Field: label, input, then a fixed-height line: helper "Use 8+ characters" before submit, error "Password too short" after — same slot, no jump.

### R6. Single-column layout by default
**Rule:** One column of fields; two columns only for genuinely paired short fields (first/last name, from/to date) that are filled as a unit.
**Why:** Single columns produce one clear scan path, fewer skipped fields, and survive narrow viewports without reflow redesign; multi-column zigzag scanning measurably slows completion.
**Example:** Address form: street, city, ZIP+city pair at most — not a 2×6 grid of mixed widths.

### R7. Group related fields and name the group
**Rule:** Related fields form a group with a visible group name (fieldset/legend concept or an equivalent heading); groups are separated by spacing stronger than intra-group spacing.
**Why:** The group name answers "why are you asking this?" before the first field and gives screen-reader users the context once instead of per field.
**Example:** Group "Shipping address" over street/city/ZIP, then "Delivery options" — two named blocks, not eight naked fields.

### R8. Up to ~5 options: radios; more: select
**Rule:** One-of-n with 5 or fewer options renders as visible radios; 6 or more uses a select; never a select for 2–3 options.
**Why:** Visible options cost zero interaction and allow comparison; beyond ~5 the stack outgrows the screen and the select's compactness wins — the crossover is where scan cost exceeds open cost.
**Example:** "Plan: Free / Pro / Team" = radios. "Country" (200 options) = select. "Yes/No" as a select = wrong.

### R9. Switch commits now; checkbox waits for submit
**Rule:** A switch applies its effect immediately (settings surfaces); a checkbox records a choice applied on submit (forms). Never place a switch inside a form whose values only apply on save.
**Why:** The switch shape promises instant effect — if nothing happens, trust in every switch erodes; the checkbox shape promises "part of what I'm submitting" — instant effect there is surprising and error-prone.
**Example:** Settings page "Dark mode" = switch, effective at once. "Include coupon (checkbox)" inside checkout = applied with the order.

### R10. Width signals expected content length
**Rule:** Field width matches the expected content: short codes get narrow fields (ZIP ≈ 6 characters), free text gets full width; nothing is a uniform default width.
**Why:** Width is a pre-attentive hint of format — a 40-character-wide ZIP field reads "long answer expected" and invites mistakes; matched widths also make omissions visible when scanning.
**Example:** "ZIP" narrow, "Search" wide, "Phone" medium — three widths, each readable as a format hint.

### R11. File and date inputs need typable alternatives
**Rule:** Anything opened via a picker (file, date, color) also accepts typed entry (path, ISO date, hex) and direct keyboard operation of the picker.
**Why:** Pickers are pointer-first and slow for known values; a typable path is also the only viable route for keyboard-only users and automation — and often the fastest one for power users.
**Example:** Date field accepts "2025-03-14" typed; file input accepts a pasted path; both are also reachable by keyboard alone.

### R12. Every field works with keyboard alone
**Rule:** Tab reaches every field in visual order; labels are programmatically associated; groups are announced; autocomplete and corrections do not trap focus.
**Why:** Keyboard completeness is the automation and accessibility baseline — a form that fails it is unusable for a whole class of users and untestable by tooling.
**Example:** Full sign-up completable with Tab/arrows/typing only, helper text announced with its field (see 40-quality/accessibility-wcag.md).

## Checklist
- [ ] Every field has a visible persistent label above it
- [ ] Placeholders are hints/examples only, never labels
- [ ] Optional fields marked "(optional)"; no asterisk forests
- [ ] Input types + autofill/inputmode set per content type
- [ ] Helper/error slot reserved — no layout shift on error
- [ ] Single column; related fields grouped and group named
- [ ] Radios ≤ 5 options; select 6+; no Yes/No selects
- [ ] Switches commit instantly; checkboxes wait for submit
- [ ] Field widths signal content length; file/date inputs accept typed values
- [ ] Full form completable by keyboard alone

## Anti-patterns
- Placeholder-only labels that vanish on input
- Asterisk on every required field, optional ones unmarked
- Date picker with no typable date entry
- Switch inside a submit-only form ("apply on Save" surprise)
- Yes/No rendered as a select dropdown
- Uniform-width fields for ZIP and biography alike
- Error text that appears and pushes the submit button down
- Two-column zigzag form on a narrow viewport

## Sources
- Nielsen Norman Group — Placeholders in form fields are harmful: https://www.nngroup.com/articles/form-design-placeholders/
- Nielsen Norman Group — Error-message guidelines for forms: https://www.nngroup.com/articles/errors-forms-design-guidelines/
- WCAG 2.2 — Identify Input Purpose: https://www.w3.org/WAI/WCAG22/Understanding/identify-input-purpose.html
- Material Design 3 — Text fields: https://m3.material.io/components/text-fields/overview
- Apple HIG — Text fields: https://developer.apple.com/design/human-interface-guidelines/text-fields
