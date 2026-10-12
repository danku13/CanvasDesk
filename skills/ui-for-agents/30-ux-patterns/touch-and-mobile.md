# Touch & Mobile Patterns

> **Read when:** designing or building UI for touch-first devices — phones, tablets, kiosks, or any pointerless surface. **Section:** 30-ux-patterns. **Related:** ../20-components/navigation-patterns.md, ../20-components/modals-and-overlays.md, ../20-components/forms-and-inputs.md, ../40-quality/responsive-adaptive.md, ../40-quality/accessibility-wcag.md, ../00-fundamentals/motion-principles.md, ../40-quality/perceived-performance.md

**Core idea:** touch input is imprecise, one-handed, and attention-interrupted. Put controls where thumbs live, make every gesture an optional shortcut with a visible alternative, and never fight the platform's own edges.

This file covers interaction patterns for touch surfaces. It is stack-agnostic: CSS mechanics (viewport meta, `safe-area-inset`, `touch-action`) live in `50-platforms/web-css-dom.md`; the same rules apply to native toolkits.

## Rules

### R1. Size targets for fingers, not pointers
**Rule:** Every interactive target is ≥ 44×44 px (recommended; 24×24 px is the WCAG 2.2 minimum for pointer inputs), with ≥ 8 px spacing between adjacent targets; the touchable area is the whole visual tile, not just the glyph.
**Why:** A fingertip is ~7–10 mm wide and lands with several millimeters of error; undersized adjacent targets produce the two most common mobile errors — mistaps and nothing-taps.
**Example:** A 24 px icon inside a 44 px hit-slop container passes; a 32 px row of three icon buttons separated by 4 px fails on every mid-priced phone.

### R2. Design for the thumb zone
**Rule:** Primary and frequent actions sit in the bottom third of the screen (or bottom bar); destructive and hard-to-undo actions sit away from the natural thumb sweep — top edges and corners are the worst place for them.
**Why:** ~half of one-handed interactions are thumb-driven and the natural arc covers the bottom half best; "confirm delete" in the top-right corner is exactly where strays land.
**Example:** Mobile tab bar and main CTA at the bottom; the "Delete project" confirm button is top-right while "Cancel" sits in the thumb zone — the error cost is asymmetric on purpose.

### R3. Treat hover as a desktop-only enhancement
**Rule:** Nothing critical may exist only on hover — tooltips, dropdown menus, and action buttons have touch-first equivalents (tap-to-open, visible labels, inline actions); apply sticky-hover mitigation so a tap never leaves the element stuck in hover state.
**Why:** Touch has no persistent hover state; hover-only affordances are invisible on mobile, and naive `:hover` styles linger after a tap, making the UI look broken.
**Example:** Desktop shows the row actions on hover; mobile always shows a "⋯" overflow button — same actions, discoverable without a pointer.

### R4. Gestures are shortcuts, never the only path
**Rule:** Every gesture (swipe-to-delete, pull-to-refresh, long-press menus, drag-to-reorder) has a visible, labeled control alternative; destructive gestures offer undo rather than confirm.
**Why:** Gestures are invisible (no affordance — see ../00-fundamentals/visual-hierarchy.md), unlearnable for new users and unavailable to switch/voice input; undo-after-destructive matches how swipes are actually made — fast and accidental.
**Example:** Swipe a email row left → trash; the same row's "⋯" menu also offers Delete, and a 5-second undo toast follows either path.

### R5. Request the right keyboard and input hints
**Rule:** Text fields declare their input expectations — numeric/phone/email keyboard (`inputmode`/`type`), autocomplete intents, entry-key label — and the layout keeps the focused field and its label visible above the keyboard.
**Why:** The default alphabetic keyboard for a card number or OTP field multiplies typos and switches; a keyboard covering the active field is the most common reason mobile forms are abandoned (see ../20-components/forms-and-inputs.md).
**Example:** OTP input: `inputmode="numeric"`, `autocomplete="one-time-code"`, no autocorrect; the field scrolls to mid-screen, not under the keyboard.

### R6. Respect safe areas and system edges
**Rule:** Content and controls honor the notch, home indicator, and system-gesture zones (safe-area insets); edge-attached UI never conflicts with the OS back/home gestures.
**Why:** The system draws and gestures over those regions — a CTA under the home indicator is both unpressable and a constant accidental-exit source.
**Example:** The bottom tab bar pads itself above the home indicator inset; a horizontal photo carousel disables its own edge-swipe on the screen's outer 20 px where the back gesture lives.

### R7. Prefer bottom sheets over full-screen takeovers
**Rule:** Contextual, quick tasks (filters, details, pickers) open in a bottom sheet that keeps parent context visible; full-screen modals are reserved for complete task switches; a FAB hosts exactly one primary action per view.
**Why:** Bottom sheets preserve spatial orientation and allow comparing with the underlying content; stacked full-screen modals bury users in back-button archaeology (see ../20-components/modals-and-overlays.md).
**Example:** "Filter results" opens a half-screen sheet with the result list dimmed behind; only "Compose new report" takes a full screen.

### R8. Give every touch immediate feedback
**Rule:** Every touchable element shows a pressed/active state within ~100 ms (overlay or scale — ../00-fundamentals/motion-principles.md), plus optional haptics for confirmations; nothing waits silently on the network.
**Why:** Touch users hold the result in their hand — literally; without instant acknowledgment they re-tap (double navigation) or believe the app is frozen, which is why perceived latency dominates mobile reviews (see ../40-quality/perceived-performance.md).
**Example:** A card press dims it to 92% opacity in 80 ms before its 300 ms navigation; payment tap fires haptic + spinner, never a bare 2-second pause.

### R9. Never disable user zoom or text resizing
**Rule:** Pinch-zoom, browser font scaling, and OS text-size settings remain enabled; layout adapts to them (reflow, no clipped text), and `user-scalable=no`-style overrides are banned.
**Why:** Low-vision users rely on zoom and large text (WCAG 1.4.4); disabling zoom trades a developer's pixel-perfection against a real user's ability to read at all — and iOS ignores it anyway.
**Example:** At 200% OS text scale the settings list reflows to two-line rows instead of clipping; nothing on the page blocks pinch.

### R10. Test on slow hands and slow phones
**Rule:** Verify scroll performance and interaction latency on a mid-range device and with realistic data volumes; fixed elements must not jank during scroll, and lists of hundreds of items virtualize.
**Why:** Development machines are fast and full of cached data; the first 3 seconds of sluggish scroll on a 3-year-old phone define the product's perceived quality more than any visual polish.
**Example:** A 2,000-row customer list scrolls at 60 fps because rows are virtualized; on a 2019-class device the opening screen is interactive in under 2 seconds.

## Checklist
- [ ] All interactive targets ≥ 44×44 px (24 px absolute floor) with ≥ 8 px spacing
- [ ] Primary actions in the thumb zone; destructive actions away from stray-tap corners
- [ ] No hover-only affordances; sticky-hover mitigated
- [ ] Every gesture has a visible labeled alternative; destructive gestures undo instead of confirm
- [ ] Fields declare inputmode/autocomplete/enter-key; active field stays visible above the keyboard
- [ ] Safe-area insets honored; no controls in system-gesture zones; no edge-swipe conflicts
- [ ] Contextual tasks open in bottom sheets; FAB hosts one primary action; no modal stacking
- [ ] Pressed states fire within ~100 ms; loading never silent
- [ ] Pinch-zoom and OS text scaling work; layout reflows instead of clipping
- [ ] Scrolled on a mid-range device with real data volumes — no jank on fixed elements

## Anti-patterns
- Hover dropdown navigation that mobile users cannot open at all
- A 20 px "×" close button in the very top-right corner
- Swipe-to-delete with no undo and no menu alternative
- Numeric inputs opening the full QWERTY keyboard
- `user-scalable=no` "to keep the layout intact"
- Full-screen modal for a one-field quantity edit
- Controls jammed against the home indicator
- A carousel whose swipe competes with the system back gesture
- Infinite scroll over 5,000 unvirtualized rows "because it works on my machine"

## Sources
- Apple HIG — Touch/gestures, target sizes, iPhone device metrics: https://developer.apple.com/design/human-interface-guidelines/gestures
- Material Design 3 — Touch & gesture guidance, FAB, bottom sheets: https://m3.material.io/foundations
- WCAG 2.2 — 2.5.5 Target Size (Enhanced), 2.5.8 Target Size (Minimum), 1.4.4 Resize Text: https://www.w3.org/TR/WCAG22/
- Nielsen Norman Group — Magic mobile numbers & thumb-zone research: https://www.nngroup.com/articles/magic-numbers-mobile/
- Nielsen Norman Group — Mobile input & keyboard research: https://www.nngroup.com/topic/mobile/
