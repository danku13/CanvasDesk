# Elevation & Depth

> **Read when:** using shadows, borders, z-index, or any layering cue. **Section:** 00-fundamentals. **Related:** 00/color.md, 00/motion-principles.md, 10/theming-dark-mode.md, 20/modals-and-overlays.md

**Core idea:** depth is communication — higher means closer, more transient, more interactive. Elevation only works when the scale is small, ordered, and consistently mapped to the same layering meanings.

These rules apply to real shadows, border-based definition, dark-mode surface steps, and flat renderers that simulate depth with tone alone.

## Rules

### R1. Elevation communicates layering and interactivity
**Rule:** Assign elevation levels to roles, not to components: content = 0, raised content (cards, hover) = 1, floating controls (dropdown, popover, tooltip) = 2, scrims + dialogs = 3, transient toasts = top.
**Why:** Users read elevation as "what can this do to me" — a consistent role→level map lets them predict what floats above what without trial and error; per-component ad-hoc shadows turn depth into meaningless noise.
**Example:** A card sits at level 1 everywhere; its hover lift is the only change. A dropdown always beats a card; a toast always beats a dialog.

### R2. One consistent light source
**Rule:** All shadows in the product imply a single light direction; shadows and (if used) highlights never contradict each other.
**Why:** Mixed light directions read as physically impossible and subtly broken; consistency is what makes even stylized shadows parse as depth instead of decoration.
**Example:** Every drop shadow offsets downward (light from above); an element appearing with an upward shadow reads as a bug, not a style.

### R3. Shadows = large blur + low opacity, never hard edges
**Rule:** Build shadows from generous blur radii with low alpha (a small tight layer plus a large diffuse layer, both well under ~20% opacity); never ship hard-edged or heavy shadows.
**Why:** Real shadows diffuse with distance from the light source; hard edges and dense black look dirty and compete with content, while soft large blurs read as clean height.
**Example:** A resting card: a 1–2px tight shadow plus a 16–24px diffuse shadow, each around 8% black — not a tight 40% black ring glued to the edge.

### R4. Manage stacking with a fixed z-index scale
**Rule:** Define one ordered ladder (e.g. base 0 / dropdown 100 / sticky 200 / overlay 300 / modal 400 / toast 500) with gaps for insertion; components consume declared levels, never raw numbers.
**Why:** Ad-hoc z-index (999, 9999, 99999) is unmaintainable — new layers keep outbidding old ones until nothing stacks predictably; a fixed ladder makes stacking a lookup, not a fight.
**Example:** A popover that must beat a sticky header takes the overlay band (300) — never `99999`. New layer types slot into gaps without renumbering.

### R5. In dark themes, raise surfaces with lighter color, not stronger shadows
**Rule:** Express elevation in dark UIs primarily by lightening the surface color per level, plus a hairline border; shadows barely read on dark backgrounds.
**Why:** Shadows need luminance contrast to be visible; on near-black they are nearly invisible, so surface lightness becomes the depth channel and roles must support that remap. See `10-design-system/theming-dark-mode.md`.
**Example:** Dark levels: base surface ~#121212, level 1 ~#1E1E1E, level 2 ~#252525, each with a subtle lighter border for definition.

### R6. Pair border + subtle shadow for definition
**Rule:** On low-contrast backgrounds, define surfaces with a 1px border (≥ 3:1 against the adjacent background) plus a soft shadow — never a heavy shadow alone.
**Why:** WCAG 1.4.11 requires component boundaries to reach 3:1; shadows alone fail that measurably, and borders keep working where shadows are clipped, disabled, or flattened by themes.
**Example:** A white card on an #F5F5F5 page: hairline border + soft shadow — the border alone already satisfies non-text contrast; the shadow adds the lift.

### R7. Motion may lift elevation as an affordance
**Rule:** Interactive elements may raise one elevation level on hover/press and return on release; the lift is small, runs 150–300ms, and is never applied to non-interactive content.
**Why:** A slight lift is a strong "this is pressable" cue because it is physically meaningful (object coming toward the hand); if static content lifts too, the affordance channel is spent. See `00-fundamentals/motion-principles.md`.
**Example:** A resting card at level 1 lifts to level 2 on hover over 200ms ease-out; a static banner never lifts because it is not clickable.

### R8. Avoid deep nesting of shadows
**Rule:** At most 1–2 elevation levels above the content plane in any view; flatten deeper nesting with spacing and grouping instead.
**Why:** Stacked shadows multiply darkness and visual complexity (card-in-card-in-card), muddying which container matters; flat layouts with clear spacing preserve the hierarchy intent. See `00-fundamentals/visual-hierarchy.md` R7.
**Example:** A section containing a card is fine; that card containing another shadowed card gets flattened — the inner group separates by padding alone.

### R9. Dim what floats over
**Rule:** When a modal or sheet overlays content, dim or blur the layers beneath with a scrim so the floating layer reads as clearly above and the background reads as inactive.
**Why:** Without a scrim, the overlay competes with busy content at equal contrast and users lose the "this is now the task" signal; the scrim also blocks accidental interaction below.
**Example:** A confirm dialog over a data table sits on a ~50% scrim; the table dims and stops responding until the dialog closes.

### R10. Animate elevation changes
**Rule:** When an element changes elevation (lift on hover, dialog appearing, sheet dismissing), animate the transition over 150–300ms instead of cutting between levels.
**Why:** A cut between levels reads as flicker and breaks the physical metaphor of an object moving toward or away from the user; animated height is what makes the depth language believable.
**Example:** A menu opens by lifting from level 1 to 2 with shadow growth over 200ms ease-out, and sinks back on close. See `00-fundamentals/motion-principles.md`.

### R11. Use the platform's native layering where it exists
**Rule:** Prefer the toolkit's built-in overlay/popover/modal layering (native window layers, top-layer equivalents) over hand-rolled shadow stacks; paint custom shadows only in renderers without native layering.
**Why:** Native layers handle stacking, input routing, and assistive-tech exposure correctly for free; hand-rolled duplicates tend to break input order and trap focus incorrectly.
**Example:** A menu built on the toolkit's popup API keeps keyboard and screen-reader behavior; a self-painted floating panel needs all of that rebuilt and tested.

### R12. Flat by default
**Rule:** Reserve elevation for elements that genuinely float or invite interaction; most content lives at level 0 with grouping done by spacing and tone, not shadows.
**Why:** Elevation is a scarce signal like accent color — spread across every container, it stops marking anything and adds rendering cost on low-end targets (WASM/canvas included).
**Example:** A dashboard where only the command palette, dropdowns, and toasts are elevated; all panels are flat, separated by borders and spacing.

### R13. Never clip focus indicators with elevation boundaries
**Rule:** Overflow clipping, layer edges, and stacking must never cut off focus rings; leave room (≥ 2px) around every focusable element and let rings paint above sibling content.
**Why:** Clipped focus indicators destroy keyboard usability and fail WCAG 2.2 focus-appearance requirements; a ring that disappears under a shadowed card reads as lost focus.
**Example:** A scrollable list uses inner padding so the focused row's ring is not clipped; a dropdown's shadow never paints over the trigger button's ring.

## Checklist
- [ ] Every shadow and z-index maps to a declared elevation level
- [ ] Z-index ladder documented with gaps (0/100/200/300/400/500); no raw ad-hoc values
- [ ] All shadows share one light direction; none hard-edged or high-opacity
- [ ] Dark theme expresses levels via surface lightness + hairlines, not stronger shadows
- [ ] Surfaces on low-contrast backgrounds have a border ≥ 3:1 against the adjacent background
- [ ] Hover/press lifts are ≤ 1 level, 150–300ms, on interactive elements only
- [ ] Overlays sit on a scrim; background content dims and deactivates
- [ ] No view nests more than 2 shadow levels above content
- [ ] Elevation transitions are animated, not cut
- [ ] Focus rings never clipped by scroll containers, layer edges, or sibling shadows

## Anti-patterns
- `z-index: 9999` to win a stacking fight
- Three different shadow directions on one screen
- Heavy, tight, dark shadows glued to element edges
- Dark mode reusing light-theme shadow values unchanged
- Elevated look on non-interactive static content
- Card-in-card-in-card with cumulative shadows and no scrim between levels
- Modals floating over busy content with no dimming
- Elevation as default decoration on every panel

## Sources
- Material Design 3 — Elevation: https://m3.material.io/styles/elevation/overview
- Material Design 2 — Elevation & dark-theme surface levels: https://m2.material.io/design/environment/elevation.html
- Apple HIG — Materials: https://developer.apple.com/design/human-interface-guidelines/materials
- WCAG 2.2 — 1.4.11 Non-text Contrast: https://www.w3.org/TR/WCAG22/
- Nielsen Norman Group — Flat design & depth cues: https://www.nngroup.com/articles/flat-design/
