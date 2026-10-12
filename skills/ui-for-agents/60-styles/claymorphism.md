# Style: Claymorphism

> **Read when:** building UI in the claymorphism style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/elevation-depth.md, 00/color.md, 00/motion-principles.md, 00/typography.md

**Personality:** soft, cute, toy-like, friendly-3D. **Best for:** kids apps, crypto wallets wanting friendliness, playful SaaS marketing, stickers and avatars, fun dashboards, gamified learning. **Avoid for:** dense tools, luxury, serious B2B, legal and medical products.

**Core idea:** inflated clay — surfaces puff up from the background via a double inner shadow (light top, dark bottom), sitting on pastel colored backgrounds with big radii and 3D clay props; it is neumorphism that escaped its monochrome trap by embracing color.

Boundary: vs neumorphism.md (monochrome extrusion on one hue, a11y-hostile) — claymorphism is colorful puffy on colored backgrounds, friendlier and more legible; vs skeuomorphism.md — abstract cute 3D, not realistic materials.

## Signature (what makes it recognizable)
- Puffy inflated surfaces: the inner-shadow pair creates candy volume
- Radius 16–40 px everywhere; no sharp corners
- Pastel candy palette on light COLORED backgrounds (lavender, mint, peach — not white/gray)
- 3D clay illustration objects: characters, blobs, shapes
- Soft same-hue outer shadow for float
- Chunky rounded controls with deep press feedback
- Rounded friendly type

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | puffy | 16–40 px, uniform |
| `puff-inset-light` | top inner | white ~60% alpha, offset 0/−6 px, blur 12 px |
| `puff-inset-dark` | bottom inner | same-hue darkened ~40%, offset 0/+6 px, blur 12 px |
| `shadow-float` | outer soft | same-hue 15–25% alpha, 0/8/24 px |
| `color-bg` | pastel colored | lavender/mint/peach family, L ~92–96% |
| `color-surface` | lighter pastel of the bg hue | L ~97–99%, same or adjacent hue |
| `color-ink` | dark ink, mandatory | deep plum/charcoal of the bg hue, ≥ 4.5:1 on surface |
| `color-accent` | saturated candy | 4.5:1 for text use; CTA fills verified 3:1 vs surface |
| `type` | rounded friendly sans | scale ~1.25; body 400+ |
| `press-scale` | clay squish | 0.97–0.98 on press |
| `motion-duration` | bouncy | 250–400 ms with overshoot on success only |
| `props` | one clay asset set | same material and lighting render across the product |

## Rules

### R1. The puff recipe is fixed
**Rule:** Every puffy surface uses the same triple — inner shadow light (top, white ~60%) + inner shadow dark (bottom, same-hue darkened ~40%) + outer soft shadow (same-hue, low alpha) — documented as tokens and applied consistently.
**Why:** The 3D illusion comes from consistent virtual lighting; one element with inverted shadows breaks the clay metaphor for the whole screen.
**Example:** Card, button, and avatar badge all carry identical `puff-inset-*` values; only blur scales with size.

### R2. Colored background is required
**Rule:** Backgrounds are pastel colored, never white or gray; surface and bg must differ in hue or lightness enough for the puff to read.
**Why:** Puffiness needs hue/lightness contrast to inflate against — the neumorphism one-hue-on-one-hue failure is exactly what this style fixed.
**Example:** Mint bg #DFF5EA with near-white mint surface; the same pair on #FFFFFF collapses into flat white cards.

### R3. Pressed clay squishes
**Rule:** Pressed state = inner shadows deepen + scale 0.97–0.98 + border darkens; release springs back with slight overshoot.
**Why:** Clay deforms when pushed — the deformation is the affordance; color-only states do not read as physical feedback.
**Example:** CTA on press: scale 0.97, dark inset doubles, 1 px darker border appears — three signals in 250 ms.

### R4. Dark ink on solid light surfaces, always
**Rule:** Text is dark ink on solid light surfaces; puffy pastel fills never carry low-contrast or white text; text never sits on 3D props.
**Why:** Pastel-on-pastel is the style's classic contrast failure, and the puff recipe already eats local contrast — ink must carry the floor.
**Example:** Balance figure: deep plum 22/600 on the near-white mint card, 4.5:1 verified; no white text on peach buttons.

### R5. One clay asset set
**Rule:** All 3D props (characters, blobs, shapes) come from a single set rendered with the same material, lighting, and camera; never mix libraries.
**Why:** Clay props are the style's hero objects; mismatched lighting or materials instantly reads as clip-art collage.
**Example:** Every empty state draws from the `props` set — same matte clay, same top-left key light, same soft ambient occlusion.

### R6. Bouncy motion, budgeted
**Rule:** 250–400 ms with overshoot reserved for success and reward moments (level-up, added item, confetti); navigation and state changes use gentler ease-out; reduced-motion collapses everything to fades.
**Why:** Bounce is charm but spends attention; everywhere-bounce is exhausting and the overshoot stops meaning "yay".
**Example:** Add-to-wallet: card puffs in at 350 ms with 8% overshoot; the tab switch beside it is a 200 ms fade.

### R7. Radius consistency
**Rule:** 16–40 px radii everywhere — cards, buttons, inputs, images; the style has no sharp corners, and radius within a view varies by at most one step.
**Why:** The toy look is geometric softness; one sharp corner breaks the illusion the way a chipped toy does.
**Example:** 24 px cards, 20 px buttons, 16 px inputs — all rounded, nothing square, no leftover 4 px defaults.

### R8. Scope clay to the shell; keep dense zones flat
**Rule:** Apply the puff to cards, controls, and moments; data-dense surfaces (tables, forms, settings lists) render flat with a border — clay stays a frame, not a grid.
**Why:** Inflated surfaces on every row multiply shadows into noise and destroy usable density; even friendly products have spreadsheet moments.
**Example:** Wallet home: puffy balance card and chunky actions; the transaction list below is flat rows with 1 px dividers in the same ink.

## A11y watchpoints
- Pastel-on-pastel is the classic fail: dark ink mandatory for all text; verify 4.5:1 against the lightest surface point
- Pressed state must be visible beyond the shadow change — scale + border + color, never a puff-swap alone (3:1 UI floor)
- Bounce and overshoot respect `prefers-reduced-motion` — collapse to 0–150 ms fades
- 3D props are decorative by default → `aria-hidden`; their meaning must exist in adjacent text
- Focus is a distinct outline (≥ 3:1) outside the puff shadows; shadows never double as focus indicators
- Chunky radii invite tiny pill targets — enforce ≥ 24×24 px (44×44 touch)

## Checklist
- [ ] Puff triple (light inset / dark inset / float) tokenized and identical everywhere
- [ ] Backgrounds pastel colored; surface–bg hue/lightness gap verified
- [ ] Press = scale 0.97–0.98 + deeper inset + border
- [ ] All text dark ink, 4.5:1 on solid light surfaces
- [ ] One clay prop set with the same material and lighting
- [ ] Overshoot only on success moments; reduced-motion fallback works
- [ ] Radius 16–40 px everywhere; no sharp corners
- [ ] Dense zones (tables, forms) flat with borders
- [ ] Focus outline distinct from puff shadows
- [ ] Targets ≥ 24×24 px (44×44 touch) despite chunky styling

## Anti-patterns
- Pastel or white text on pastel surfaces
- White/gray backgrounds — the style's premise collapses
- Mixing clay prop libraries or lighting renders
- Any sharp corner or leftover 4 px radius
- Shadow-only pressed states
- Overshoot on every transition
- Puffy table rows and form grids
- Looping busy 3D scene animations

## Sources & inspiration
- Flat design — the lineage skeuomorphism → flat → soft 3D: https://en.wikipedia.org/wiki/Flat_design
- neumorphism.io — soft-shadow math the puff recipe extends: https://neumorphism.io
- Dribbble — claymorphism origin shots and derivatives: https://dribbble.com/tags/claymorphism
- WCAG 2.2 — non-text contrast on pastel surfaces: https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
- MDN box-shadow — implementing the double inner shadow: https://developer.mozilla.org/en-US/docs/Web/CSS/box-shadow
- WebAIM contrast checker — verify every pastel pair: https://webaim.org/resources/contrastchecker/
