# Style: Skeuomorphism

> **Read when:** building UI in the skeuomorphism style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/elevation-depth.md, 00/color.md, 00/iconography.md, 00/visual-hierarchy.md

**Personality:** familiar, tactile, realistic, rich. **Best for:** app icons, smart-home control surfaces, audio-gear UIs, automotive dashboards, niche products imitating physical objects, retro-flavored marketing. **Avoid for:** dense productivity tools, modern minimal brands, data-heavy apps.

**Core idea:** digital imitates physics — leather stitching, brushed metal, glossy buttons, paper pages; every control looks and behaves like the physical object it represents, so affordance comes from realism. The historical motive matters: a generation of users learned what "tappable" meant from realistic controls.

Boundary: distinct from neumorphism.md (single-hue soft extrusion, minimal palette) and claymorphism.md (abstract soft 3D) — skeuomorphism is multi-material realism.

## Signature (what makes it recognizable)
- Realistic material fills: leather, wood, brushed metal, glass, felt, paper
- Glossy button highlights: vertical gradient with a top sheen
- Inner shadows and bevels on every raised element
- Stitched or dashed borders on leather zones
- Knobs, sliders, and toggles that mimic real hardware mechanics
- One implied light source (top-center) governing all materials and shadows
- Multi-layer shadows: soft drop shadow plus a tight contact shadow

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | hardware-driven | 4–10 px on metal; 14–20 px on cards and buttons |
| `shadow-*` | multi-layer, soft | drop 0/2/6 px at 30–40% alpha + 0/1/2 px contact shadow |
| `bevel` | raised edges | 1–2 px light top edge / dark bottom edge pair |
| `gloss-highlight` | restrained sheen | white 30–40% vertical fade across the top half |
| `material-leather` | warm zones | noise + weave fill, 1 px dashed stitch border inset 4 px |
| `material-metal` | control decks | vertical brushed bands, ±4% lightness variation |
| `material-glass / paper` | overlays, panels | white 10–20% alpha fill over a darker base |
| `color-bg` | deep neutral stage | dark warm gray / walnut range |
| `color-surface` | material fills only | chosen from the fixed material library, never raw color |
| `color-text` | high-contrast ink | near-white on dark materials, near-black on light zones |
| `type` | engraved / embossed pair | engraved = darker fill + light shadow below; embossed is the inverse |
| `motion-duration` | physical mass | 250–350 ms with a slight spring on press and release |

## Rules

### R1. The metaphor must be honest
**Rule:** A control that looks like a physical object must behave like it — toggles slide, knobs rotate, pages flip. The metaphor teaches; breaking it deceives.
**Why:** Realism is this style's only affordance channel; a realistic-looking control with arbitrary behavior destroys the trust the realism built.
**Example:** Volume knob: dragging rotates it with matching tick feedback — never tap-to-jump or an undocumented swipe.

### R2. One light source, top-center, forever
**Rule:** All gradients, bevels, sheens, and shadows in the product derive from a single implied top-center light.
**Why:** Mixed light directions read as broken rendering, not as materials — the illusion is the style, and the illusion is light-consistent.
**Example:** Metal deck and leather panel on the same screen both shade darker toward their bottom edges.

### R3. Materials come from a fixed library
**Rule:** Define 5–6 material tokens (leather, metal, glass, wood, paper) and reuse them everywhere; never invent a material per screen.
**Why:** A material is a component, not a decoration; per-screen materials fragment the product into unrelated dioramas.
**Example:** The player screen and the settings screen both use `material-metal` for control decks — same texture, same sheen.

### R4. Text sits on high-contrast zones, with shadow pairs
**Rule:** Place text on the flattest, highest-contrast zone of the material; engraved or embossed lettering must pair the fill with its shadow (light-below or dark-below, consistently) and never go below 14 px.
**Why:** Texture under text is noise; unpaired engraved text blurs at small sizes and fails contrast checks at its worst point.
**Example:** Track title on brushed metal: flat polished strip, white 16 px text; engraved variant = dark fill + 1 px light shadow below.

### R5. Gloss is subtle, not candy
**Rule:** The top sheen on glossy elements is a white vertical fade at 30–40% alpha over the top half — never a bubble highlight covering most of the element.
**Why:** Strong gloss dominates the content and dates the product to 2009; subtle sheen reads as material, heavy gloss reads as decoration.
**Example:** Primary button: base color, 35% white fade on the top half, 1 px bevel — label still 4.5:1 against the darkest gloss point.

### R6. States change realism, not just color
**Rule:** Pressed = bevel inverts + shadow shrinks + sheen dims, on top of a color shift; released reverses it. States must be visible in the material physics.
**Why:** The style promises a physical surface; a pressed button that only changes hue feels like a sticker, not a button.
**Example:** Pressed toggle: 2 px dark inset, drop shadow collapses to 1 px, sheen off, track tint darkens — four redundant signals.

### R7. Motion has mass
**Rule:** Animate interactions at 250–350 ms with a slight spring on press/release and settling on drags; nothing snaps instantly.
**Why:** Objects with weight build the tactile illusion; instant snaps break physics and feel glitchy rather than responsive.
**Example:** Knob rotation follows the finger with a 300 ms ease and 8% overshoot; page turn eases 320 ms with a shadow sweep.

### R8. Budget the realism (the iOS 7 lesson)
**Rule:** Spend realism on icons, control surfaces, and hero moments; keep core flows (forms, lists, settings) in plain flat chrome — and plan the style's retirement as the audience matures.
**Why:** The movement's documented failure was scale: when every surface shouts, hierarchy dies and maintenance cost explodes (iOS's own 2013 pivot away).
**Example:** Tuner widget fully skeuomorphic; the search-and-results flow above it is quiet flat chrome sharing the same ink color.

## A11y watchpoints
- Contrast is measured against textured materials at the WORST point, not the average — verify 4.5:1 text / 3:1 UI at every texture seam and sheen edge
- Every state must survive without the gloss: color + border + label redundancy, never sheen-only
- Physical metaphors must not obscure function: label every metaphor control (knob = "Volume", switch = "Power")
- Textured backgrounds need solid or high-alpha text panels; never set body text directly on leather or wood
- Focus is a visible outline (≥ 3:1), not a bevel change; engraved text below 14 px is illegible — enforce minimum sizes

## Checklist
- [ ] One top-center light source across all materials and shadows
- [ ] Material library fixed at 5–6 tokens and reused everywhere
- [ ] Every metaphor control behaves like its physical counterpart
- [ ] Text ≥ 4.5:1 at the worst texture/sheen point, on flat zones
- [ ] Engraved/embossed text carries a consistent shadow pair
- [ ] Gloss ≤ 40% alpha sheen, top half only
- [ ] Pressed state = bevel inversion + shadow change + color
- [ ] Motion 250–350 ms with mass; drags settle, nothing snaps
- [ ] Dense flows (forms, lists) use plain chrome, not materials
- [ ] Every metaphor control has a visible text label

## Anti-patterns
- Mixed light sources (top-left on one card, top-right on its neighbor)
- Per-screen invented materials — neon plastic next to stitched leather
- Candy gloss: 60%+ sheen covering the whole element
- Pressed states that only change color
- Skeuomorphic data tables, forms, and dashboards
- Metaphor icons with no label ("mystery dial")
- Body text set directly on texture
- Faking hardware the product does not have (non-functional knobs)

## Sources & inspiration
- Skeuomorph — definition and design history: https://en.wikipedia.org/wiki/Skeuomorph
- iOS 7 — the documented pivot away from skeuomorphism: https://en.wikipedia.org/wiki/IOS_7
- NN/g — flat design's origins, including the skeuomorphic era's trade-offs: https://www.nngroup.com/articles/flat-design/
- Web Design Museum — era-authentic skeuomorphic UI examples: https://www.webdesignmuseum.org/timeline
- Apple Human Interface Guidelines — current stance on realism and materials: https://developer.apple.com/design/human-interface-guidelines/
- WCAG 2.2 — non-text contrast on textured surfaces: https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
