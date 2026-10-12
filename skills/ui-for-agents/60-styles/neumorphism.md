# Style: Neumorphism

> **Read when:** building UI in the neumorphism style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/elevation-depth.md, 00/color.md, 00/iconography.md

**Personality:** tactile, soft, gadget-like, calm. **Best for:** niche control surfaces (audio tools, smart-home widgets), aesthetic experiments. **Avoid for:** production apps with critical actions, dense data, accessibility-sensitive products — honestly: this is the most a11y-hostile preset in the library.

**Core idea:** one hue, two soft shadows, and a fixed depth vocabulary fake physical extrusion — and because the style erases inherent contrast, state legibility must be engineered back in as mandatory compensation.

## Signature (what makes it recognizable)
- bg = surface: one single hue everywhere (the #E0E5EC family and cousins)
- Dual soft shadows — light top-left, dark bottom-right — simulating extrusion from the background
- Pressed/inset state (shadows flip inward) for active controls
- Large radius 16–24 px+, monochrome palette + ONE tint
- Icon + label on every control; soft icon-only buttons are invisible
- A shipped flat high-contrast mode as the explicit fallback

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `color-bg` / `color-surface` | one hue, identical | #E0E5EC (dark variant: #23272E) |
| `shadow-light` | light source top-left | white 50–80% alpha, offset −6px −6px, blur 12px |
| `shadow-dark` | darker same-hue | same hue with 30–50% lightness cut, offset +6px +6px, blur 12px |
| `shadow-inset` | pressed variant | both shadows inverted + inset, 1–2px offsets |
| `color-border` | mandatory edge compensation | darker same-hue 1px at ~30% alpha (or reinforced dark shadow) |
| `color-text` | dark same-hue ink | #3A4150, ≥ 4.5:1 on the surface |
| `color-accent` | ONE tint for active/danger | saturated same-family hue; 4.5:1 for text use |
| `focus-ring` | separate visible outline | 2px ink/accent outline, offset 2px, ≥ 3:1 |
| `radius-*` | large, puffy | 16–24 px+ |
| `space-scale` | roomy | 8pt scale, +1 step |
| `type` | neutral sans, medium weights | scale ratio ~1.25 |
| `motion-duration` | gentle press feedback | 150–250 ms, ease-out |

## Rules

### R1. One hue, one light source
**Rule:** Background and surfaces share exactly one hue; the simulated light comes from top-left on every element, everywhere, forever.
**Why:** The extrusion illusion is fragile — a second hue or a second light direction instantly shatters it into "buggy gradients".
**Example:** Every knob, card, and slider on #E0E5EC with white top-left / #BEC3CF bottom-right shadows; no exceptions for "special" panels.

### R2. Depth vocabulary is fixed and documented
**Rule:** Raised = interactive, flat = container, inset = pressed/active. Publish this mapping, then never mix: a raised element is clickable, an inset one is engaged, a flat one is neither.
**Why:** With color nearly absent, depth state is the only affordance channel — an ambiguous vocabulary makes the whole surface unlearnable.
**Example:** Volume knob raised at rest, inset while dragging; the enclosing panel stays flat through both.

### R3. State legibility outranks softness
**Rule:** Pressed/active/disabled must be unmistakable via inset + a tint/color shift, never via shadow subtlety alone; the state must survive a grayscale screenshot.
**Why:** The style's shadows sit near the contrast floor by construction; a state visible only as a 5% shadow swap is invisible to low-vision and color-blind users.
**Example:** Active toggle: inset shadows + track tinted accent + knob border darkens — three redundant signals.

### R4. Compensate edges with borders or reinforced shadows
**Rule:** Every interactive element gets a 1px darker same-hue border or a strengthened dark shadow so its boundary passes the 3:1 UI-component floor.
**Why:** Same-hue extrusion erases element boundaries; the border is the mandatory compensation, not an optional garnish.
**Example:** Soft button: raised shadows + 1px #C5CAD3 border — edge readable at arm's length.

### R5. Focus is an outline, not a shadow
**Rule:** Keyboard focus renders the dedicated `focus-ring` outline (≥ 3:1); shadows and insets never double as the focus indicator.
**Why:** Soft shadows cannot meet visible-focus requirements and vanish on inset elements; WCAG 2.4.7/2.4.13 need an unambiguous ring.
**Example:** Focused slider: 2px ink outline offset 2px outside the track; shadows unchanged.

### R6. Icon + label on every control
**Rule:** No icon-only neumorphic controls; pair the glyph with a text label inside or beside the soft surface.
**Why:** Low-contrast icons on a low-contrast surface are invisible to scanning, screen-magnifier users, and everyone in sunlight.
**Example:** "Mute" control: speaker glyph + the word, both on the raised pill.

### R7. One tint, tightly budgeted
**Rule:** Use a single accent tint only for active/on states, warnings, and the destructive action — under 10% of any view; all decoration stays in the base hue family.
**Why:** Multi-hue neumorphism breaks the monochrome illusion AND burns the only strong signal channel the style has.
**Example:** Audio deck: everything gray-blue except the recording dot (red) and engaged EQ toggles (accent tint).

### R8. Disabled = flattened and relabeled
**Rule:** Disabled controls lose their shadows (flat), dim to a still-legible ~3:1, and keep their label; never just opacity 0.4 on an already-soft surface.
**Why:** Fading a low-contrast style produces invisible controls; flattening communicates "inert" while remaining readable.
**Example:** Unavailable preset button: flat surface, muted but legible label, helper text explains why.

### R9. Scope it: control surfaces, not core flows
**Rule:** Apply neumorphism to supplementary widgets (mixer, thermostat, lighting); primary navigation, critical forms, and destructive actions need extra-strong affordances — or a different treatment entirely.
**Why:** The style's ambiguity is tolerable for playful adjustments and unacceptable where mis-presses are costly (see style-catalog.md avoid-column).
**Example:** Smart-home app: neumorphic dimmer dial on the widget screen; the alarm "Disable" button uses a flat, bordered, red-text treatment instead.

### R10. Ship a flat high-contrast mode
**Rule:** Build and expose a toggle that strips shadows, adds 1px borders, and enforces the text/UI contrast floors — the honest fallback for accessibility-sensitive contexts and forced-colors environments.
**Why:** The compensation rules get you to the floor; the flat mode is how the style survives real users and audits.
**Example:** Settings → "High-contrast controls": knobs gain dark borders, insets become filled states, all pairs re-verified.

## A11y watchpoints
- Blunt truth: the style is constructed at the edge of the contrast floors — the border + tint-shift compensations (R3/R4) are MANDATORY; verify 4.5:1 text and 3:1 UI boundaries, not vibes
- Focus must be a separate visible outline; shadow-based "depth = focus" fails 2.4.7 and disappears on inset elements
- Never deploy for primary navigation or destructive actions without extra-strong affordances (border + color + label)
- Test with color-vision-deficiency simulation and grayscale: pressed/active states must stay distinguishable without hue
- Soft puffy controls invite small targets: enforce ≥ 24×24 px (44×44 touch) despite the aesthetic's shrink bias

## Checklist
- [ ] bg and every surface share exactly one hue
- [ ] Single top-left light source across all elements
- [ ] raised / flat / inset vocabulary documented and never mixed
- [ ] Every interactive element has a border or reinforced edge at 3:1
- [ ] Pressed/active = inset + tint shift, recognizable in grayscale
- [ ] Focus = dedicated outline token, ≥ 3:1, separate from shadows
- [ ] All controls carry icon + label; zero icon-only soft buttons
- [ ] Text pairs ≥ 4.5:1 on the single surface color
- [ ] Flat high-contrast mode implemented and reachable
- [ ] Motion 150–250 ms; press feedback immediate

## Anti-patterns
- Icon-only soft buttons (the style's signature failure)
- Multi-hue or gradient neumorphism — the illusion collapses
- Disabled as opacity 0.4 on an already-low-contrast surface
- Pressed state conveyed by shadow swap alone
- Neumorphic data tables, forms, or dashboards
- Focus ring removed "because the shadows already show depth"
- Two light sources (top-left on one card, top-right on its neighbor)
- Shipped as the default style for a production app with critical flows

## Sources & inspiration
- Michał Malewicz — Neumorphism in user interfaces (critical analysis of the trend): https://uxdesign.cc/neumorphism-in-user-interfaces-b47cee3b3bc
- Dribbble — neumorphism origin shots (Alexander Plyuto and derivatives): https://dribbble.com/tags/neumorphism
- NN/g — flat UI and discoverability costs: https://www.nngroup.com/articles/flat-design/
- WCAG 2.2 — non-text contrast (1.4.11): https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
- neumorphism.io — soft-UI shadow generator (offset/blur starting points): https://neumorphism.io
- WebAIM — contrast checker for pair verification: https://webaim.org/resources/contrastchecker/
