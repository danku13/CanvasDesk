# Style: Bauhaus

> **Read when:** building UI in the bauhaus style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/spacing-layout-grids.md

**Personality:** rational, constructive, playful-geometric, functional. **Best for:** architecture and interior studios, art schools, galleries, design tools, cultural institutions. **Avoid for:** banking forms, data-dense dashboards, long-form reading apps.

**Core idea:** form follows function — compositions built from three geometric primitives (circle, triangle, square) in a primary triad plus black on off-white, arranged with asymmetric-but-deliberate balance; the machine-age optimism of "art + industry" made interactive.

**Boundary:** not minimalist-swiss (swiss = gray neutrals + one accent + strict grid; bauhaus = primary colors + geometric play + asymmetry) and not de-stijl (de stijl = orthogonal only, heavier black lines, no curves — bauhaus keeps circles and play).

## Signature (what makes it recognizable)
- Decoration vocabulary is a closed set: circle, triangle, square — no fourth shape ever
- Primary triad `#DA291C` / `#F4C300` / `#0F5CA8` + black `#141414` on off-white `#F4F1EA`
- Geometric sans everywhere (Futura-class); display set uppercase; no decorative faces
- Thick black rules and borders do the structural work hairlines do in other styles
- Asymmetric balance: one large mass counterweighted by smaller shapes, never dead-center
- Radius at extremes only — 0 (machine edge) or full circle (sphere); middle radii banned
- Zero shadows, zero gradients: mechanical flatness; depth comes from overlapping flat shapes
- Motion 150–250 ms, sharp ease-out, no bounce — the machine does not jiggle

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | extremes only | 0 for cards/buttons; 9999px for badges, toggles, image dots |
| `shadow-*` | none | flat; hierarchy via black borders and overlap |
| `border-strong` | structural rule | 2–3px solid `#141414` |
| `space-scale` | disciplined 8pt | 8 / 16 / 24 / 32 / 48; modules sit on a visible grid |
| `color-bg` | warm off-white | `#F4F1EA` |
| `color-surface` | pure white | `#FFFFFF` |
| `color-ink` | near-black | `#141414` |
| `color-accent` | red = action/alert | `#DA291C` |
| `color-accent-2` | blue = links/information | `#0F5CA8` |
| `color-accent-3` | yellow = highlight shapes only | `#F4C300` |
| `type-display` | geometric sans, uppercase | Futura-class (e.g. Jost) 600–700, tracking +2–4% |
| `type-body` | same family, regular | 16px / line-height 1.5 |
| `motion-duration` | quick mechanical | 150–250 ms, ease-out, no bounce |

## Rules

### R1. The primitive set is closed
**Rule:** Every decorative or structural shape is a circle, a triangle, or a square (rectangles included); stars, swooshes, blobs, and freeform curves are refused, and icons are geometric constructions from the same three shapes.
**Why:** The three-primitive vocabulary is the style's entire identity — one imported organic shape dissolves it into generic flat design.
**Example:** Badges: circle for counts, triangle for warnings, square for tags; the icon set is drawn on one 24px grid with uniform 2px strokes.

### R2. Primaries are roles, not decoration soup
**Rule:** Assign one primary per role — red to the primary action and alerts, blue to links and information, yellow to non-text highlights — and never place two primaries in one component.
**Why:** The triad reads as a system only while each hue keeps one meaning; mixing them per-component turns constructive design into confetti.
**Example:** A form: red "Submit", blue links, one yellow circle behind the active step number — never a red-to-yellow gradient button.

### R3. Geometric sans everywhere, uppercase at display
**Rule:** Use one geometric sans family (Futura-class) for display and body; set display uppercase with slight tracking; no serif, script, or rounded faces anywhere.
**Why:** The letterform is a geometric primitive like the circle and square — a second type voice breaks the single-vocabulary promise.
**Example:** Hero "BAUHAUS ARCHIVE" in uppercase Jost 700; the paragraph below is 16px Jost 400 in sentence case.

### R4. Asymmetric but deliberately balanced
**Rule:** Compose pages off-axis: one dominant mass (large image, big type, big color field) counterweighted by two or three smaller elements; never center everything by default.
**Why:** Dynamic balance is what separates Bauhaus composition from a template — full symmetry reads Swiss, imbalance without counterweight reads broken.
**Example:** Hero: large red circle left, headline upper right, two small photo squares lower right balancing the circle's mass.

### R5. Radius extremes only
**Rule:** Corners are either sharp (0px) or fully round (circle/pill); 4–16px "softening" radii are banned.
**Why:** Middle radii are neither machine nor sphere — they are the default of unconsidered design and instantly dilute the geometric language.
**Example:** Buttons are 0px rectangles; the cart count is a full-circle badge; nothing on the page sits at 8px.

### R6. Black rules are the structure
**Rule:** Use 2–3px solid black rules and borders for section division, card edges, and underlines; hairline grays and decorative dividers are out.
**Why:** Confident black lines are the print-era skeleton of the style; timid hairlines leave the composition feeling underfed.
**Example:** Section headers carry a full-width 3px rule; cards are white with a 2px black border and no shadow.

### R7. Motion is mechanical: 150–250 ms, no bounce
**Rule:** Animate state changes 150–250 ms with ease-out and strictly linear geometry (translate, scale, color step); no springs, overshoot, or parallax.
**Why:** The machine metaphor demands deterministic movement; a jiggling hover reads as handmade in the wrong way.
**Example:** Card hover: border thickens 2→3px and the black rule slides 8px in 180ms; nothing else moves.

### R8. Never signal meaning by hue alone
**Rule:** Pair every primary-colored signal with a shape or label: alert = red + triangle icon, active step = yellow + number, link = blue + underline; success never reuses red.
**Why:** With only three strong hues, roles collide fast, and color-blind users lose red distinctions first — shape and text must carry the semantics.
**Example:** Error state: red border + triangle icon + message text; success is a filled black square marker with ink text.

## A11y watchpoints
- Yellow `#F4C300` fails on white/off-white (~1.5:1) — yellow is for filled shapes only, never text, thin strokes, or icon lines
- Red `#DA291C` on off-white computes ≈4.3:1 — fine for large display and 3:1 UI boundaries, not for body-size text; darken toward `#B32217` for small red text
- Blue `#0F5CA8` on off-white ≈6:1 — safe for links; re-verify on pure white surfaces and against yellow-adjacent fills
- Flat surfaces hide focus: use a 2px black outline with 2px offset (3:1 against white and all three primaries)
- Red and yellow converge under protanopia — the shape pairing in R8 is mandatory, not decorative

## Checklist
- [ ] Every decoration is circle, triangle, or square; zero imported shapes
- [ ] One primary per role; no component mixes two primaries
- [ ] Single geometric sans; display set uppercase
- [ ] Layouts asymmetric with deliberate counterweight
- [ ] Radii only 0 or full-circle
- [ ] Zero shadows and gradients
- [ ] Black rules 2–3px carry the structure
- [ ] Motion 150–250 ms, no bounce or spring
- [ ] Yellow never carries text or thin strokes
- [ ] No signal is color-only; focus visible everywhere

## Anti-patterns
- 8–16px rounded corners "to make it friendly"
- Gradients, soft shadows, glass blur — any soft-UI import
- All three primaries on one banner or button
- Yellow text or thin yellow strokes on white
- Fully centered symmetric hero (Swiss template logic)
- Serif or script display faces "for character"
- Bouncy spring hover animations
- Organic blobs and freeform swooshes as decoration

## Sources & inspiration
- Bauhaus — the movement: https://en.wikipedia.org/wiki/Bauhaus
- Bauhaus Dessau Foundation — original buildings and artifacts: https://www.bauhaus-dessau.de/en/
- Tate — Bauhaus art term: https://www.tate.org.uk/art/art-terms/b/bauhaus
- Futura — the canonical geometric sans: https://en.wikipedia.org/wiki/Futura_(typeface)
- Jost — open Futura-class face on Google Fonts: https://fonts.google.com/specimen/Jost
