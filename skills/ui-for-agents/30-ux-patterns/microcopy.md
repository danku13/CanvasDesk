# Microcopy

> **Read when:** writing or reviewing any UI string — buttons, labels, errors, toasts, tooltips, empty states. | **Section:** 30-ux-patterns | **Related:** forms-validation-ux.md, empty-loading-error-states.md, ../20-components/feedback-toasts-alerts.md, onboarding-first-run.md, ../40-quality/accessibility-wcag.md

**Core idea:** microcopy is interface, not prose. Every string is a control: it must be scannable, consistent, blame-free, and translatable — or it breaks the flow it decorates.

## Rules

### R1. Sentence case everywhere
**Rule:** Use sentence case for all UI text — buttons, titles, menu items, column headers, labels. Capitalize only the first word and proper nouns.
**Why:** Title Case adds visual noise and forces per-word capitalization decisions that fracture across locales; sentence case scans faster and is the default across modern systems (Material, Apple, GOV.UK).
**Example:** "Create new invoice", "Billing settings", "Payment method" — not "Create New Invoice".

### R2. Punctuation: periods on sentences only
**Rule:** No trailing periods on labels, buttons, titles, or list items. Use periods on full sentences — toasts, helper text, error explanations.
**Why:** Fragments with periods read as stilted; the period signals "complete thought", and a button is not one. The inconsistency is instantly visible when broken.
**Example:** Button "Save changes" (no period). Toast "Your changes were saved. Syncing will resume tonight." (periods).

### R3. Buttons start with a verb that names the outcome
**Rule:** Button labels open with a verb and state the result of clicking: "Save changes", "Create invoice", "Invite teammate". Avoid "OK", "Submit", "Yes/No" in dialogs.
**Why:** The label is the last confirmation of what will happen; a verb-outcome label removes ambiguity about consequences, and "OK" forces users to re-read the dialog to know what they agreed to.
**Example:** Delete dialog: primary "Delete invoice", secondary "Keep invoice" — not "OK / Cancel", where which answer deletes is anyone's guess.

### R4. Errors never blame, never show raw codes alone
**Rule:** Write errors neutrally about the system state, not the user's failure: "This password doesn't match our records" — never "You entered a wrong password". Codes may appear as secondary detail, never as the only content.
**Why:** Blame triggers defensiveness and abandonment; raw codes ("E-4021") give the user nothing to act on and mark the product as broken.
**Example:** "We couldn't process that card. Try another payment method or contact your bank. (Error 402)".

### R5. One concept = one word, everywhere
**Rule:** Pick a single term per concept and use it on every screen, string, and error: "workspace" or "project" — never both; "delete" or "remove" — never alternately. Maintain a product term base.
**Why:** Alternating terms forces users to constantly check whether two words mean different things; consistency is what makes search, help, and learning transferable.
**Example:** If the sidebar says "Workspaces", then settings, docs, errors, and emails all say "workspace" — never "project".

### R6. Numbers, dates, units localized
**Rule:** Format numbers, dates, times, currencies, and units per the user's locale (decimal separators, date order, 12/24h, currency position). Never hardcode a format.
**Why:** "03/04/2024" is March 4 or April 3 depending on the reader; ambiguous dates in an invoicing product cause real financial errors.
**Example:** The same timestamp renders "Apr 3, 2024" (en-US) and "03.04.2024" (de-DE) — one value, per-locale rendering.

### R7. Respect length budgets
**Rule:** Buttons 1–3 words; nav items 1–2 words; toasts ≤ 2 short sentences; tooltips ≤ 1 sentence; helper text ≤ 1 line. If it does not fit, fix the design — do not shrink the font.
**Why:** Long labels slow scanning and break layouts across screen sizes and translations (German strings run ~35% longer than English); budgets force the design to absorb the constraint.
**Example:** "Add" not "Click here to add a new item to your list"; a tooltip "Copies the link to your clipboard", not a paragraph.

### R8. Write for translation: whole strings, no concatenation
**Rule:** Store complete sentences as single strings; never build sentences by joining fragments or concatenating variables into grammar. Keep punctuation out of glued fragments.
**Why:** Word order differs per language, so concatenated fragments produce untranslatable word salad; full strings let translators reorder grammar correctly.
**Example:** Use a pluralized template "You have {count} open invoice|invoices" — never "You have " + n + " invoice" + suffix.

### R9. State the benefit, not the mechanics
**Rule:** Hint and empty-state copy describes what the user gets: "Add a card to start selling" — not the mechanical requirement "Card required" or "Field cannot be empty".
**Why:** Users act on value, not on validation rules; benefit phrasing converts a demanded input into a wanted action.
**Example:** Profile hint: "Add a photo so teammates recognize your comments" — not "Photo: required field".

### R10. Use the user's vocabulary, not the system's
**Rule:** Name things the way users think of them ("Delete", "Email", "Buy"), not internal model or engineering terms ("Remove record entity", "Purge dataset", "Execute purchase").
**Why:** Interfaces are conversations in the user's language; internal jargon transfers cognitive load the product was supposed to absorb.
**Example:** "Trash" not "Recycler binary"; "Undo changes" not "Revert transaction state".

### R11. Pair the message with the recovery action
**Rule:** Error and blocking messages include or sit next to the action that resolves them (a "Retry" button, a "Log in instead" link) — a message that only describes the problem is incomplete.
**Why:** Every extra navigation step between reading the problem and fixing it multiplies abandonment; the shortest path is the one embedded in the message.
**Example:** "Your session expired. [Log in again]" — not "Session expired." with no path forward.

### R12. Second person, active voice, plain present tense
**Rule:** Address the user as "you" and keep verbs active and present: "We couldn't save your file" / "Save your draft". Avoid passive voice and abstract agentless constructions.
**Why:** Active second-person copy is parsed faster and assigns clear agency, which matters most in errors where the user needs to know who does what next.
**Example:** "We couldn't reach the server" — not "It has been detected that a connection failure occurred".

### R13. Test copy at 2× length
**Rule:** Before shipping, verify every string survives doubling in length without clipping, wrapping into oblivion, or breaking the layout; truncate predictably where needed.
**Why:** English is compact; German, Finnish, and Russian routinely double label lengths, and untested strings clip to "…" or overflow containers in exactly the locales you will ship later.
**Example:** A button budgeted as "Save" must still fit "Änderungen speichern" at the same token size — reserve the space up front.

## Checklist
- [ ] All UI text is sentence case; no Title Case fragments
- [ ] No trailing periods on labels/buttons/titles; periods present on full sentences
- [ ] Every button opens with a verb naming the outcome; no bare "OK"/"Submit" on ambiguous dialogs
- [ ] No error message blames the user or shows only a code
- [ ] One term per concept across all screens (term base exists for the top 20 nouns)
- [ ] Dates, numbers, currencies rendered per locale
- [ ] Buttons ≤ 3 words, toasts ≤ 2 sentences, tooltips ≤ 1 sentence
- [ ] No concatenated sentence fragments; all strings complete and translatable
- [ ] Blocking messages carry their recovery action
- [ ] Strings verified at 2× length without clipping

## Anti-patterns
- "Are you sure you want to proceed? OK / Cancel"
- "You entered an invalid password!" (blame plus exclamation)
- "Error: 0x80070057" as the whole message
- Mixing "workspace"/"project" or "delete"/"remove" for one concept
- "You have 1 invoice(s)" or any "(s)" plural hack
- "Saved!" — exclamation marks on routine system feedback
- ALL CAPS or Title Case labels ("SAVE CHANGES")
- Helper text that restates the label ("Email: enter your email")

## Sources
- Microsoft Writing Style Guide — voice & tone top 10: https://learn.microsoft.com/en-us/style-guide/top-10-tips-style-voice-tone
- Material Design 3 — Foundations (content design): https://m3.material.io/foundations
- GOV.UK — Content design guidance: https://www.gov.uk/guidance/content-design
- Nielsen Norman Group — Ten usability heuristics (error recovery, match with the real world): https://www.nngroup.com/articles/ten-usability-heuristics/
- W3C Internationalization — dates, numbers, locales: https://www.w3.org/International/
