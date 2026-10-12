# Accessibility (WCAG 2.2)

> **Read when:** building or reviewing any interactive UI — web DOM, Rust-native, or WASM/canvas. **Section:** 40-quality. **Related:** 00-fundamentals/color.md, 00-fundamentals/motion-principles.md, 20-components/forms-and-inputs.md, 30-ux-patterns/forms-validation-ux.md, 50-platforms/rust-native-gui.md, visual-qa-protocol.md

**Core idea:** organize accessibility work by POUR — Perceivable, Operable, Understandable, Robust — and enforce the concrete criteria below. They hold for every renderer: a canvas UI with no accessibility tree is inaccessible by definition, not merely incomplete.

## POUR map

| POUR | Covered by |
|---|---|
| Perceivable | R1 contrast, R2 not color-only, R3 reflow at 200% |
| Operable | R4 keyboard, R5 focus visible, R6 target size, R7 bypass, R11 reduced motion |
| Understandable | R8 labels & error suggestions, R9 consistent identification |
| Robust | R10 roles/names/states in the a11y tree, R12 layered test stack |

## Rules

### R1. Enforce contrast on every real foreground/background pair
**Rule:** Text ≥ 4.5:1 against its actual rendered background; large text (≥ 24 px regular / ~19 px bold equivalent) and meaningful non-text elements (icons, input borders, focus rings, chart series) ≥ 3:1. Measure the rendered pair, not the token sheet. (WCAG 2.2: 1.4.3 Contrast Minimum, 1.4.11 Non-text Contrast)
**Why:** Contrast is a property of the final pixels: gradients, images behind text, translucent overlays, and dark-mode remaps routinely break ratios that the token file claims are safe.
**Example:** White text over a photo passes at the top-left (4.6:1) and fails over a bright cloud (2.9:1) — add a scrim or reposition; do not eyeball it.

### R2. Never encode status in color alone
**Rule:** Every state communicated by color must also carry a second channel: text, icon, shape, or position. (1.4.1 Use of Color)
**Why:** Roughly 8% of men have impaired red-green discrimination, and UIs render in monochrome contexts (e-readers, print, dimmed panels); color-only signals are silently lost.
**Example:** Invalid fields get a red border + error message naming the field + warning icon — not a red border alone. Chart lines differ by dash pattern, not hue only.

### R3. Support 200% text scaling with reflow
**Rule:** At 200% text scaling (or 320 CSS px viewport with 400% zoom), content reflows to a single column with no horizontal scrolling and no loss of function or information. (1.4.4 Resize Text, 1.4.10 Reflow)
**Why:** Low-vision users zoom instead of switching assistive tech; fixed-width layouts clip content or force two-dimensional scrolling, which breaks tracking and hides actions.
**Example:** At 200% text size a two-column settings page stacks to one column, labels stay attached to their fields, and the save button stays reachable without sideways scrolling.

### R4. Make every function keyboard-operable with no traps
**Rule:** All functionality is reachable and usable with keyboard alone; focus can always leave any component with standard keys (Tab, arrows, Esc). (2.1.1 Keyboard, 2.1.2 No Keyboard Trap)
**Why:** Keyboard operation is the base layer under switch access and screen readers; a trap locks those users out of the rest of the interface with no recovery path.
**Example:** A rich-text editor that swallows Tab for indentation still offers Tab-out (e.g. Esc then Tab); a modal closes and returns focus on Esc.

### R5. Render a visible focus indicator ≥ 3:1
**Rule:** The focused element shows an indicator visible against its adjacent colors at ≥ 3:1, on every interactive element, always — including mouse-initiated focus in custom renderers. (2.4.7 Focus Visible, 2.4.13 Focus Appearance)
**Why:** Keyboard users navigate by focus; if the indicator is invisible or suppressed, they are navigating blind and cannot tell the focused button from plain text.
**Example:** A 2 px accent ring with 2 px offset, verified ≥ 3:1 on both light and dark surfaces per theme — not per brand palette in isolation.

### R6. Keep pointer targets ≥ 24×24 px
**Rule:** Every pointer target is at least 24×24 px, or has 24 px of spacing around it; use 44×44 px on touch-first surfaces. (2.5.8 Target Size Minimum)
**Why:** Small or tightly packed targets cause mis-hits, and mis-hits on navigation or destructive controls are expensive; 24 px is the enforced floor, 44 px the ergonomic finger recommendation.
**Example:** Dense table toolbar icon buttons are 24×24 with 8 px gaps; the touch layout uses 44×44 hit areas with the glyph smaller than the hit region.

### R7. Provide a skip mechanism for repeated navigation
**Rule:** Views with a repeated navigation block expose a skip link (or equivalent first-focus jump) as the first focusable element. (2.4.1 Bypass Blocks)
**Why:** Without a bypass, keyboard and screen-reader users re-tab through the same nav on every view — 20 items × 50 views is a dead stop, not an inconvenience.
**Example:** First Tab reveals "Skip to main content" anchored offscreen at the top-left; native apps set initial focus on the main content region instead.

### R8. Label every input and suggest concrete fixes for errors
**Rule:** Every input has a programmatic, visible label; on validation failure, each field announces what failed and how to fix it, in text. (3.3.1 Error Identification, 3.3.2 Labels or Instructions, 3.3.3 Error Suggestion)
**Why:** Unlabeled fields are guesswork with assistive tech ("edit text, blank"), and "Invalid input" without a fix forces users to reverse-engineer the rule — most give up.
**Example:** "Date must be DD/MM/YYYY — you entered 13/2024" beats "Invalid date". Placeholder text is a hint, never the label.

### R9. Identify components consistently
**Rule:** The same function uses the same name, icon, and behavior everywhere in the product. (3.2.4 Consistent Identification)
**Why:** Users and assistive tech build a vocabulary once ("trash = delete"); synonyms for the same action force re-learning and cause wrong choices, especially with screen readers reading names verbatim.
**Example:** Deletion is always "Delete" with a trash glyph — never "Remove" on one screen and "Discard" on another for the identical operation.

### R10. Expose roles, names, and states to the accessibility tree
**Rule:** Every interactive element exposes its role, accessible name, and state (checked/expanded/disabled/selected) to assistive technology: ARIA semantics in DOM apps, the AccessKit tree in Rust-native apps, platform equivalents elsewhere. (4.1.2 Name, Role, Value)
**Why:** AT users operate a model of the UI, not the pixels; if the model says "clickable div" instead of "checkbox, checked", the control is unusable no matter how correct it looks.
**Example:** A custom toggle renders `<div role="switch" aria-checked="true" aria-label="Dark mode">` — or, in egui/iced, registers the switch role and checked state via AccessKit so screen readers announce "Dark mode, switch, on".

### R11. Honor prefers-reduced-motion
**Rule:** When the user's reduced-motion setting is on, disable non-essential motion (parallax, auto-playing decoration, large transitions); keep short opacity fades only where they communicate state. (2.3.3 Animation from Interactions)
**Why:** Vestibular disorders make large motion physically nauseating, not merely annoying; the setting is an explicit user request, and honoring it is cheaper than any workaround.
**Example:** Page transitions become instant swaps or ≤ 200 ms fades; a celebration confetti animation does not run at all.

### R12. Layer the test stack: automation, keyboard, screen reader
**Rule:** Ship only after (a) an automated pass (axe or platform equivalent) is clean, (b) a manual keyboard-only walkthrough of every core flow, and (c) at least one screen-reader session on a representative flow. Automation catches roughly 30% of issues — the manual passes surface the rest and are mandatory, not optional.
**Why:** Automated tools check measurable properties (contrast, labels, ARIA validity) but cannot judge whether focus order makes sense, whether a name is meaningful, or whether a flow is operable at all.
**Example:** axe reports zero violations, yet the keyboard pass reveals a date picker that traps focus — only the manual pass catches it; log both results.

## Checklist
- [ ] Every text/background pair measured ≥ 4.5:1 (large text and UI components ≥ 3:1) in both themes
- [ ] No state is carried by color alone — icon, text, or shape accompanies every signal
- [ ] 200% text scale / 320 px reflow: single column, no horizontal scroll, nothing lost
- [ ] Every flow completable with keyboard only; Esc exits overlays; no traps
- [ ] Focus indicator visible and ≥ 3:1 on every interactive element in both themes
- [ ] All pointer targets ≥ 24×24 px (44×44 on touch surfaces)
- [ ] Repeated navigation has a working skip mechanism
- [ ] Every input labeled programmatically; errors state the fix, per field
- [ ] Roles/names/states present in the a11y tree (ARIA or AccessKit) for all custom widgets
- [ ] Reduced-motion variant verified; axe clean + keyboard pass logged + one screen-reader session done

## Anti-patterns
- `outline: none` (or cleared focus visuals) with no equal or better replacement
- Placeholder-as-label; icon-only buttons with no accessible name
- Gray-on-gray "subtle" text below 4.5:1 justified as "it's only secondary"
- ARIA sprinkled to silence linters — `aria-label` duplicating visible text, wrong roles
- A modal that traps focus with no Esc and no return of focus to the trigger
- Custom canvas widgets with no AccessKit/platform node ("a11y comes later")
- Auto-playing carousels and parallax with no reduced-motion path
- Treating a clean axe report as the entire accessibility budget
- Shipping "a11y support" that only exists in the README, untested in the rendered product

## Sources
- W3C — WCAG 2.2 quick reference (criterion numbers used above): https://www.w3.org/WAI/WCAG22/quickref/
- W3C — WAI-ARIA Authoring Practices: https://www.w3.org/WAI/ARIA/apg/
- Deque — axe-core automated rules: https://github.com/dequelabs/axe-core
- AccessKit — accessibility tree for Rust UIs: https://github.com/AccessKit/accesskit
- Nielsen Norman Group — Keyboard accessibility: https://www.nngroup.com/articles/keyboard-accessibility/
