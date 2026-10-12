# Forms: Validation UX

> **Read when:** building or reviewing any input flow — sign-up, settings, checkout, editors. | **Section:** 30-ux-patterns | **Related:** ../20-components/forms-and-inputs.md, ../20-components/buttons-and-actions.md, microcopy.md, ../40-quality/accessibility-wcag.md, ../20-components/modals-and-overlays.md

**Core idea:** validation timing decides whether errors feel like help or harassment. Catch errors late enough to respect the user, early enough to prevent dead-end submits, and always make the fix one glance away.

## Rules

### R1. Validate on blur, not on every keystroke
**Rule:** Run field validation when the user leaves the field (blur) or on submit — never while the user is still composing their first entry.
**Why:** Early errors feel nagging ("invalid" while the user is mid-word); late corrections feel helpful. Blur timing produced the highest success and satisfaction rates in NN/g's inline-validation studies; per-keystroke checking interrupts input and flags values the user was about to fix.
**Example:** User types "anna@company" and tabs away → that moment may show "Email is missing a domain". While they type, the field stays silent.

### R2. Re-validate live once a field has an error
**Rule:** After a field shows its first error, validate on every input change until the error clears; then stop validating until the next blur.
**Why:** Once the user is in correction mode, instant feedback confirms the fix and rewards it; without it, users over-correct or cannot tell whether the fix "took".
**Example:** Password field shows "Use at least 8 characters"; as the 8th character lands, the error disappears immediately — no blur needed.

### R3. Never validate untouched fields
**Rule:** A field the user has not interacted with may not show an error, even when its current value is known-bad; the submit path handles those.
**Why:** Errors on untouched fields read as accusations and train users to ignore messages; they also violate the expectation that feedback follows action.
**Example:** A form pre-fills "Country: United States"; do not flag it before submit, even if another selection makes it inconsistent.

### R4. Place the error message next to the field
**Rule:** Show the error text directly below (or beside) the offending field, in the same column, visually bound to that field only.
**Why:** Errors placed far from their field force a memory loop of "which one was wrong?"; adjacency cuts recovery to a glance and satisfies WCAG 3.3.1 (error identified and tied to the input).
**Example:** An error block at the top of the form alone is broken; it may supplement, but never replace, per-field messages.

### R5. Name the fix, not the failure
**Rule:** Write validation messages as the condition to meet — "Use at least 8 characters", "Enter a date in the past" — never as a verdict ("Invalid input", "Wrong format") or a bare code.
**Why:** The user's next question is always "so what do I do?"; answering it directly removes a translation step and shortens recovery to one action.
**Example:** "Username must be 3–20 characters, letters and numbers only" beats "Error 422".

### R6. Preserve user input — always
**Rule:** On validation failure, navigation away, refresh, or session restore, all user-entered values reappear exactly as typed; the user never retypes anything they already entered.
**Why:** Retyping is pure loss and a leading cause of form abandonment; data loss on error reads as the product punishing the user for the product's mistake.
**Example:** A failed submit re-renders the form with all values and selections intact and focus on the first error — not an empty form with "an error occurred".

### R7. Long forms: error summary with anchor links
**Rule:** Forms longer than about one screen show an error summary at the top after submit: the count of problems, each a link that scrolls to and focuses its field.
**Why:** Users cannot see all fields at once; the summary gives scope ("3 problems") and a one-click path to each, instead of a hunting expedition.
**Example:** GOV.UK pattern: a boxed "There is a problem — 1) Enter your name 2) Enter a valid phone number", each line jumping to its field.

### R8. Keep submit enabled; validate on click
**Rule:** Do not disable the submit button on invalid input. Keep it enabled; on click, validate everything, show all errors, and focus the first one.
**Why:** A disabled submit with no explanation leaves the user guessing which field is guilty and is invisible to assistive tech as a *reason*; an enabled submit turns the click into a complete error report.
**Example:** A sign-up button that stays gray until the whole form is valid is broken. Rare exception: a single-field form where the cause is self-evident.

### R9. Autosave drafts for long processes
**Rule:** Any form or editor a user may occupy for minutes gets automatic draft saving with a visible "Saved" confirmation and restore-on-return.
**Why:** Long forms accumulate high sunk cost; a crash or accidental close destroying it is a trust-killer, and autosave removes the fear that blocks experimentation.
**Example:** A job-application form autosaves after each section; reopening shows "Draft restored — last saved 2 min ago".

### R10. Multi-step: visible progress and lossless back
**Rule:** Wizards of 3+ steps show a progress indicator (step n of m, or named steps), and Back never discards entered data — going back and forward restores everything.
**Why:** Progress communicates remaining cost and that an end exists; lossless back makes exploration safe, which measurably increases completion.
**Example:** A 4-step checkout shows "Step 2 of 4 — Payment"; going back to edit shipping keeps the card number intact.

### R11. Confirm destructive actions only
**Rule:** Ask for confirmation only for destructive or hard-to-reverse actions (delete, overwrite, pay, send). Everything else executes immediately; never confirm out of habit.
**Why:** Every routine confirmation trains users to click through dialogs reflexively; reserving them for real danger keeps the reflex meaningful when it matters.
**Example:** "Delete invoice?" gets a confirm; "Mark as read" does not. Prefer undo (toast with Undo) over confirm for soft-deletable items.

### R12. Distinguish blocking errors from warnings
**Rule:** Separate hard errors (submit blocked) from warnings (submit allowed) in both color and copy: a warning says "This email looks unusual — submit anyway?" and means it.
**Why:** Blocking on suspicious-but-valid input (unusual names, autocorrected addresses) locks out real users; warnings protect data quality without creating lockouts.
**Example:** A phone field warns "We didn't recognize the country code" with a "Submit anyway" path instead of rejecting.

### R13. Split validation: format early, business rules late
**Rule:** Check cheap local rules (required, length, format) at blur; check rules that need other fields or a remote lookup (username taken, balance sufficient) only at submit or field completion.
**Why:** Remote checks on every blur cause request storms and flickering verdicts on partial input; local format checks are silent, instant, and safe to run early.
**Example:** "Enter a valid email" appears at blur; "This email is already registered" appears at submit with a "Log in instead" link.

## Checklist
- [ ] No field shows an error before first interaction (blur or submit)
- [ ] Fields with an active error re-validate on input and clear the moment they pass
- [ ] Every error message names the fix and sits adjacent to its field
- [ ] No raw error codes or "Invalid input"-style verdicts anywhere
- [ ] Failed submits, back-navigation, and refreshes preserve all entered data
- [ ] Long forms show an error summary with working anchor links on submit failure
- [ ] Submit button is enabled by default; on click it validates and focuses the first error
- [ ] Long processes autosave drafts with a visible "Saved" state
- [ ] Multi-step flows show progress and Back is lossless
- [ ] Confirmations exist only for destructive/irreversible actions
- [ ] Error indication is not color alone: text ≥ 4.5:1 contrast, border/icon ≥ 3:1

## Anti-patterns
- Validating on every keystroke before the first blur ("that's not a real email!" mid-typing)
- Disabled submit button and zero explanation of what is missing
- Error text only at the top of a long form, unlinked to fields
- Clearing the password field after a failed login submit
- Confirm dialogs for harmless actions ("Are you sure you want to close preferences?")
- Red-only error indication with no icon or text
- "Error 500: something went wrong" as the only feedback for a submit
- Back in a wizard that wipes the previous step's input

## Sources
- Nielsen Norman Group — Inline validation in web forms: https://www.nngroup.com/articles/form-design-inline-validation/
- GOV.UK Design System — Error message & error summary: https://design-system.service.gov.uk/components/error-message/
- WCAG 2.2 — Error Identification / Error Suggestion (3.3.1, 3.3.3): https://www.w3.org/TR/WCAG22/#error-identification
- W3C WAI Forms Tutorial: https://www.w3.org/WAI/tutorials/forms/
- Material Design 3 — Text fields: https://m3.material.io/components/text-fields/overview
