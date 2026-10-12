# Color

> **Read when:** choosing, structuring, or applying any color in a UI — palettes, themes, states, data encodings. **Section:** 00-fundamentals. **Related:** 00/visual-hierarchy.md, 00/typography.md, 10/design-tokens.md, 10/theming-dark-mode.md, 40/accessibility-wcag.md

**Core idea:** color is a semantic system, not decoration. Roles carry meaning, neutrals do the work, accents mark rare events — and contrast floors are non-negotiable.

This file defines how color is structured and applied anywhere in a UI. Token naming lives in `10-design-system/design-tokens.md`; dark-mode implementation lives in `10-design-system/theming-dark-mode.md`.

## Rules

### R1. Define semantic roles before picking any hex value
**Rule:** Start from named roles — background, surface, primary, secondary, danger, success, warning, info — and only then fill each role with a value. Never hardcode a hex value into a component.
**Why:** Components that reference roles can be re-mapped (dark theme, brand change, high-contrast mode) in one place; scattered hex values leak, diverge, and make every audit a hunt.
**Example:** A delete button asks for the danger role, not `#d32f2f`; swapping themes changes the value, never the component.

### R2. Build palettes as ramps, assign roles from steps
**Rule:** Generate every hue as a ramp of steps (50, 100, 200 … 900) and assign each role a step (e.g. primary = 600, primary-container = 100). Never hand-pick isolated tints per use case.
**Why:** Ramps guarantee tonal relationships — hover one step darker, container a light step of the same hue — and make dark-mode remapping a step shift instead of a redesign.
**Example:** Pressed = primary step 800, hover = 700, container background = 100, text on container = 900: all derived, auditable, consistent.

### R3. One accent, one job
**Rule:** Maintain a single accent (primary) color per product and reserve it for the primary action and key interactive signals; status colors are functional, not competing accents.
**Why:** The accent works by contrast against a mostly neutral field; every additional saturated hue raises the cost of finding "what do I press" until the accent marks nothing.
**Example:** An indigo-accent tool: "New project" button, active nav item, focus rings. Tags, banners, and charts use neutrals or status roles — never a second saturated brand hue.

### R4. Keep the functional palette small
**Rule:** Neutrals + one accent + danger/success/warning/info is the full functional palette for most products; any extra color must justify a semantic job.
**Why:** Each color beyond the core set is another code the user must learn and another pairing that must pass contrast in every theme; small palettes stay auditable.
**Example:** A kanban board with 12 label colors is content coloring (user data), fine — but the app chrome itself stays within the core set.

### R5. Neutrals carry the UI; accents are rare events
**Rule:** Backgrounds, surfaces, dividers, and most text come from the neutral ramp; accent and status colors appear only at decision points and state changes.
**Why:** Attention is a budget — a field of accent-colored chrome trains users to ignore the accent channel, and truly important signals lose their salience.
**Example:** In a settings form only the "Save" button, focus rings, and the "unsaved changes" badge are non-neutral; everything else is ink on surface.

### R6. Treat 60-30-10 as a heuristic, not a law
**Rule:** Aim for roughly 60% dominant neutral, 30% secondary surface/ink, 10% accent — then adjust to the view's function instead of forcing the ratio.
**Why:** The ratio is a memory aid for "neutrals dominate, accent is rare"; the invariant is the order of dominance, not the percentages — a data-dense dashboard legitimately runs 80-15-5.
**Example:** Forcing exactly 10% accent across 40 chart tiles paints the screen; keep the accent on the primary CTA and selection states only.

### R7. Contrast floors are hard, not aspirational
**Rule:** Body text ≥ 4.5:1 against its background; large text (≥ 24px, or ≥ 18.66px bold) and essential UI components/boundaries ≥ 3:1. Verify every text/background pair in every state and theme.
**Why:** These are WCAG 2.2 success criteria 1.4.3 and 1.4.11 — measurable minimums, not taste; low-contrast "subtle" gray-on-gray fails for low-vision users and on cheap displays in sunlight.
**Example:** Hint text at 3.8:1 fails — raise it to 4.5:1 or pair it with a visible label; a focused input whose border sits at 2.2:1 against the card fails 1.4.11.

### R8. Contrast is a property of pairs — check the rendered pair
**Rule:** Verify contrast between the actual rendered text/component and whatever ends up behind it — overlays, images, scrims, nested surfaces, hover states included — not between palette swatches in isolation.
**Why:** Contrast is computed per pair; a token that passes on the default background fails on an image, a scrim, or a stacked surface, and state changes silently create untested pairs.
**Example:** White text passes on the header image's dark overlay but fails over its bright sky region — add a scrim gradient or move the text.

### R9. Never encode meaning in hue alone
**Rule:** Every color-coded state (error, success, required, chart series) also carries a non-color signal: icon, text label, shape, or pattern.
**Why:** ~8% of men and ~0.5% of women have color-vision deficiency (red/green most common); hue-only encoding also dies in dark mode, grayscale screenshots, and glanceable distance.
**Example:** Form errors: red border + error icon + text message. Chart lines: distinct hues + distinct dash patterns or direct labels.

### R10. Status colors must differ in lightness too
**Rule:** Give danger, warning, success, and info distinct lightness levels, not just distinct hues, and keep them distinguishable from the accent.
**Why:** Hue distinctions vanish for color-blind users and in monochrome; lightness survives both, so equal-lightness status palettes collapse into "some colored thing happened".
**Example:** Error dark and saturated, warning mid-lightness, success mid-dark, info light — a grayscale screenshot still ranks them; each also keeps its icon.

### R11. Saturation and brightness set perceived weight
**Rule:** Vivid, saturated, bright colors read as louder and closer; desaturated and dark colors recede. Use saturation deliberately to rank importance within a hue.
**Why:** The visual system attends to saturation and contrast before content; an equal-saturation-everywhere palette has no channel left to mark what matters.
**Example:** Selected tag: primary at step 600; unselected tag: same hue at step 100 with step-700 text — identical structure, instant ranking.

### R12. Derive interaction states from the role, not from new colors
**Rule:** Hover, pressed, focus, selected, and disabled variants are systematic derivations of the role's ramp step (±1–2 steps, opacity, or a fixed overlay), defined once per role.
**Why:** Ad-hoc state colors drift across components and break the "consistent look = consistent meaning" contract; derivations keep every component's state language identical.
**Example:** Any interactive element: hover = +1 step, pressed = +2, disabled = ~38% opacity, focus ring = accent at ≥ 3:1 against adjacent colors.

### R13. Dark mode re-maps roles by lightness, never by inversion
**Rule:** Build dark themes by shifting roles to lighter surface steps and desaturated/darker accent tints that still meet contrast floors — not by inverting white↔black or reusing light-theme accent values.
**Why:** Inverted palettes cause glare and halation, and saturated accents vibrate on dark backgrounds; dark UIs also express elevation with lighter surfaces, so roles must support that remap.
**Example:** Light: background 50, surface 0, primary 600. Dark: background 900, surface 800 (raised = 700), primary 300 for text-level use. See `10-design-system/theming-dark-mode.md`.

## Checklist
- [ ] Every color in the UI resolves to a named role; zero raw hex values in components
- [ ] Palette is built as ramps (50–900); states derived from steps, not invented per component
- [ ] Exactly one accent color; it covers a small fraction of any view (≤ ~10%)
- [ ] Functional palette limited to neutrals + accent + the four status colors
- [ ] All body text pairs ≥ 4.5:1; large text and UI boundaries ≥ 3:1 — measured in both themes
- [ ] No state or data series encoded by hue alone (icon/label/pattern always present)
- [ ] Status colors differ in lightness, not only hue, and are distinct from the accent
- [ ] Dark theme shifts lightness of roles and desaturates accents; nothing is inverted
- [ ] UI verified with a color-vision-deficiency simulator or a grayscale pass

## Anti-patterns
- Hardcoded hex values sprinkled through component code
- A second and third saturated brand color "for variety"
- Light-gray-on-white "subtle" text below 4.5:1
- Red text alone as the error indicator; red/green-only chart series
- Dark mode as a filter invert, or the light-theme accent reused on near-black
- Every module shipping its own theme color ("rainbow accents")
- Pure black text on pure white with no tonal room left for states and surfaces

## Sources
- Material Design 3 — Color system & roles: https://m3.material.io/styles/color/system
- Apple HIG — Color: https://developer.apple.com/design/human-interface-guidelines/color
- WCAG 2.2 — 1.4.3 Contrast (Minimum) & 1.4.11 Non-text Contrast: https://www.w3.org/TR/WCAG22/
- Nielsen Norman Group — Using color to enhance design: https://www.nngroup.com/articles/color-enhance-design/
- WebAIM — Contrast checker & color accessibility: https://webaim.org/resources/contrastchecker/
- Colour Blind Awareness — Prevalence of color-vision deficiency: https://www.colourblindawareness.org/
