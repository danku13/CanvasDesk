# Cross-Platform Parity

> **Read when:** one product ships to web + native (Rust GUI) + canvas/WASM, or any feature must behave consistently across renderers. | **Section:** 50-platforms | **Related:** 10/design-tokens.md, 40/visual-qa-protocol.md, 50/web-css-dom.md, 50/rust-native-gui.md, 50/wasm-canvas-rendering.md, 00/spacing-layout-grids.md

**Core idea:** parity is a managed contract, not an accident: one token schema defines what the product is, per-renderer adapters define how it is drawn, and a parity matrix records every intentional difference so drift is detectable.

## Rules

### R1. One token schema, many renderers
**Rule:** Keep the token schema platform-neutral (names + values, no renderer syntax); write exactly one adapter per renderer (CSS custom properties, Slint globals, egui `Style` struct) and let components consume adapter output only.
**Why:** The schema is the contract: names and values living in one place means a design decision lands everywhere with one edit, while adapters absorb renderer syntax without leaking it back into component code.
**Example:** `color.action.primary`, `space.2 = 8`, `radius.md = 6` in the schema ⇒ `--color-action-primary` in CSS, `global ColorActionPrimary` in Slint, a `theme.rs` mapping into egui `Visuals`.

### R2. Separate universal intent from platform execution
**Rule:** Classify every rule as universal (contrast ≥ 4.5:1, hierarchy, spacing scale, state coverage: hover/focus/active/disabled/loading/error) or platform-execution (shortcut modifier, file dialog, scrollbar behavior, default fonts) — universal rules hold everywhere, execution follows host conventions.
**Why:** Teams drift in both directions: enforcing web specifics on native (hover-dependent UI, Cmd on Windows) and relaxing universals per platform ("native gets weaker contrast"); the split makes each category auditable on its own terms.
**Example:** "One primary action per view; focus always visible" = universal. "Primary action sits bottom-right; shortcut is Ctrl vs Cmd+S" = execution, decided per platform.

### R3. Maintain a parity matrix as a first-class artifact
**Rule:** Keep a documented table of features × platforms marking each cell "same", "adapted (how)", or "unavailable (why)" — every deviation from sameness must have a row with an owner and a reason.
**Why:** Undocumented differences are indistinguishable from bugs; the matrix turns "difference" into a decision, makes drift reviewable as a diff instead of user-reported, and onboards new contributors to what is intentional.
**Example:** `Export PNG: web=button, linux=button, macos=button` · `Save shortcut: web=Ctrl+S, linux=Ctrl+S, macos=Cmd+S` · `Notifications: web=push, linux=absent (v1), macos=absent (v1)`.

### R4. Budget for text growth and direction
**Rule:** Design layouts to survive +30% string expansion (typical for DE/RU/FI translations), build direction with logical start/end semantics, mirror directional iconography under RTL, and test layouts with the longest locale, not English.
**Why:** Localized strings change length and direction; fixed-width labels truncate, absolutely-positioned elements overlap, and unmirrored icons in mirrored layouts reverse meaning — a "back" arrow pointing forward.
**Example:** Button labels sized by content with token-based min-widths, so German "Rechnung erstellen" fits the same button as "Create invoice" without reflowing the toolbar.

### R5. Component parity is behavioral, not pixel-exact
**Rule:** A component "is the same" across renderers when it has the same states, the same keyboard story (tab order, activation keys, shortcuts), and the same error/edge behavior — even when its pixels are toolkit-native.
**Why:** Users switch platforms mid-task and carry expectations with them; identical behavior is what makes the switch invisible, while pixel-identity attempts produce uncanny, convention-breaking results on each host.
**Example:** The date field accepts typing, opens a picker, and reports parse errors identically everywhere; the popup itself uses each toolkit's native popover rendering and shadow conventions.

### R6. When parity is wrong, native convention wins — and is recorded
**Rule:** Where host conventions are strong (menu placement, confirmation patterns, system dialogs, modifier keys), follow the host even if it diverges from other platforms — and add the divergence to the parity matrix deliberately.
**Why:** Fighting platform conventions taxes every user of that platform to preserve an abstract sameness; a recorded adaptation is a decision, an unrecorded one is drift waiting for a bug report.
**Example:** macOS hosts app menus in the system menu bar; Linux/Windows use an in-window menu bar — both are intentional matrix rows, not a macOS "bug".

### R7. Numbers come from the schema, verified per renderer
**Rule:** All shared constants (contrast thresholds, the 24×24 px minimum target, 150–300 ms durations, spacing steps) are defined once in the schema and code-generated or mechanically synced into each renderer; CI fails on manual drift.
**Why:** Hand-copied constants diverge silently below feature level, which the parity matrix cannot see; generation makes the contract enforceable instead of aspirational and keeps dark-mode/theming edits single-source.
**Example:** `tokens.json` → build step emits `tokens.css`, `tokens.slint`, `theme.rs`; a CI diff check fails if any checked-in renderer file stops matching the schema.

### R8. One design review per renderer, always
**Rule:** A change is "shipped" only when its visual + a11y QA pass has run on each target renderer per 40-quality/visual-qa-protocol.md — passing on web does not certify native, and vice versa.
**Why:** Renderers differ in font metrics, focus rings, DPI handling, and a11y-tree construction; the same change produces different defects per platform, and unreviewed renderers accumulate them until parity is folklore.
**Example:** A new toast lands: web screenshot + focus test pass, then egui harness snapshot + AccessKit role assertions pass — only then is the matrix row updated to "same".

### R9. Detect capabilities, not platform names
**Rule:** Adapt on measured capabilities (pointer precision, hover availability, storage quota, GPU presence) exposed by each renderer's adapter — never on platform-name branching or user-agent sniffing.
**Why:** Platform names lie — touch-capable laptops, high-DPI phones, desktop browsers without GPUs; capability checks degrade gracefully on the anomaly instead of guessing wrong for a whole category of devices.
**Example:** Web gates hover styles behind `@media (hover: hover)`; the native adapter sets `capabilities.hover = false` for touch panels, so the same "no hover → tap reveals actions" path runs on both.

### R10. Record renderer limits next to the rule they break
**Rule:** When a renderer cannot meet a universal rule (contrast floor, target size, motion band), document the exception in the parity matrix and in the deliverable notes — silence is a violation, per AGENT-WORKFLOW.md.
**Why:** Undocumented failures resurface as "the product is inconsistent" bugs with no decision trail; an explicit record lets the team track the debt and pay it down when the toolkit catches up.
**Example:** "Slint v1.x text input cannot honor 24×24 px minimum on spinner arrows — workaround: whole-field stepper target; tracked as PM-142, revisited on upgrade."

### R11. Sync release trains — timing is part of parity
**Rule:** Ship a feature across renderers in the same release window, or stage it and record the availability gap in the parity matrix with an end date; never let "not ported yet" become an undocumented permanent difference.
**Why:** Users experience availability gaps as broken parity regardless of intent; a recorded, dated gap is a plan, while an open-ended one is decay that nobody owns.
**Example:** Bulk export lands on web in 1.4; native adapters are scheduled for 1.5 — the matrix row reads "native: pending, ETA 1.5", not "native: missing".

### R12. Share the words, not just the widgets
**Rule:** Keep microcopy and error strings in one per-locale catalog consumed by all renderers; renderer code never hardcodes strings.
**Why:** Divergent wording for the same error is the most visible parity break and the most avoidable; a shared catalog also makes the +30% expansion budget (R4) measurable in one place and ties into 30-ux-patterns/microcopy.md.
**Example:** One catalog entry `error-export-failed = "Export failed — try a smaller range"` consumed verbatim by the web DOM, the egui status label, and the canvas error overlay.

## Checklist
- [ ] Token schema contains zero renderer-specific syntax; each renderer has exactly one adapter
- [ ] Every rule/feature classified as universal or platform-execution
- [ ] Parity matrix exists, is reviewed in PRs, and every difference has a stated reason
- [ ] Longest-locale layout tested; +30% expansion causes no truncation or overlap
- [ ] RTL tested: mirrored layout and mirrored directional icons where meaning requires
- [ ] Same states, keyboard story, and error behavior per component across renderers
- [ ] Shared constants generated/synced from the schema; CI fails on drift
- [ ] Capability detection used instead of platform-name branching
- [ ] Every universal-rule exception is recorded in the matrix and deliverable notes
- [ ] QA pass (screenshots + a11y assertions) executed per renderer before "done"

## Anti-patterns
- Claiming "same" from a web screenshot alone
- Pixel-perfect replication of web UI on native desktop (uncanny, convention-breaking)
- Hand-copied constants per renderer ("the gray is #334 on web, #3345 on native")
- English-only screenshots as parity evidence
- Unmirrored directional icons in RTL locales
- Differences discovered by users because no matrix row recorded them
- Universal relaxations per platform ("contrast only matters on web")
- One adapter accumulating second-source values "just for this component"

## Sources
- Material Design 3 — foundations & adaptive design: https://m3.material.io/foundations
- Apple HIG — platform conventions: https://developer.apple.com/design/human-interface-guidelines
- GNOME HIG: https://developer.gnome.org/hig/
- W3C i18n — text direction & RTL: https://www.w3.org/International/questions/qa-html-dir
- Unicode — UAX #14 line breaking: https://www.unicode.org/reports/tr14/
- Unicode CLDR (locale data): https://cldr.unicode.org
- MDN — CSS logical properties (RTL execution): https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_logical_properties_and_values
- Microsoft globalization guidance: https://learn.microsoft.com/en-us/globalization/
- WCAG 2.2 — contrast, target size, focus visibility: https://www.w3.org/TR/WCAG22/
