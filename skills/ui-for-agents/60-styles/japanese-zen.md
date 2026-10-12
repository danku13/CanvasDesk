# Style: Japanese Zen

> **Read when:** building UI in the japanese-zen style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/spacing-layout-grids.md, 00/color.md, 00/typography.md, 00/motion-principles.md

**Personality:** serene, spare, contemplative, precise. **Best for:** tea and coffee brands, meditation and wellness apps, architecture studios, premium minimal brands, ryokan and boutique hotels. **Avoid for:** dense dashboards, kids products, promo-heavy e-commerce.

**Core idea:** Ma (negative space) is the main material — the interface is composed from emptiness first and elements second, on warm paper with sumi ink and a single natural hue; the beauty of incompleteness (wabi-sabi) and radical simplicity (kanso) replace symmetry, boxes, and decoration.

## Signature (what makes it recognizable)
- Whitespace at roughly 2× the swiss baseline — section gaps 96px+ on desktop, one focal element per view
- Asymmetric, one-sided compositions: content weighted left or right, the other side left deliberately empty
- Palette: washi paper `#F7F5F0` + sumi ink `#2B2B28` + ONE natural hue (indigo `#3F5573`, moss green, or terracotta)
- Hairline rules (1px ink) are the only visible structure — no boxes, no cards, no shadows
- Muted, sparse photography: seasonal, desaturated, small against large emptiness
- Vertical text or isolated CJK characters used as art accents (decorative only)
- Subtle natural texture — washi grain at 2–5% opacity, never under text
- Slow, quiet motion: 300–500 ms fades, nothing springy

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | near-sharp | 0–4px; 8px only on imagery |
| `shadow-*` | effectively none | none at rest; overlays use hairline + 2% ink veil |
| `space-scale` | doubled air | 8pt base; section gaps 96–160px desktop |
| `color-bg` | warm paper | washi `#F7F5F0` (dark variant: warm charcoal `#1A1917`) |
| `color-surface` | same paper, hairline-bounded | `#F7F5F0` + 1px rule — never fill-contrast panels |
| `color-ink` | sumi | `#2B2B28` (≈13:1 on washi) |
| `color-ink-muted` | faded ink, floor-verified | `#5C5A52` (≈6.3:1 on washi) — verify, most grays fail |
| `color-accent` | one natural hue, whole product | indigo `#3F5573` (≈7:1 on washi), moss, or terracotta |
| `type-display` | mincho serif class | light weights at large sizes (44–72px / 300) |
| `type-body` | gothic sans or mincho | 16px / 400, line-height 1.7–1.8 |
| `motion-duration` | slow quiet fades | 300–500 ms, ease-out; no springs, no bounce |
| `hairline` | decorative vs structural split | decorative: 15–20% ink; structural: ≥70% ink or `#6E6C64` (≥3:1) |

## Rules

### R1. Emptiness is the material — remove, never add
**Rule:** Whitespace is set at 2× the swiss baseline (section gaps ≥96px desktop); when a layout feels wrong, remove an element instead of adding space or decoration.
**Why:** Ma works by subtraction — every retained element is amplified by the emptiness around it, and any addition competes with the one thing that should matter.
**Example:** A tea product page: one bowl photo off-left, one line of type, one link — the remaining 70% is bare washi.

### R2. One natural accent for the whole product
**Rule:** Exactly one natural hue (indigo, moss green, or terracotta) is allowed across the entire product, reserved for the primary action and rare living touches; everything else is ink on paper.
**Why:** The calm comes from a near-monochrome world; a second hue reads as marketing noise and breaks the seasonal, meditative register.
**Example:** A meditation app where indigo appears only on "Begin session" and the active timer ring.

### R3. Asymmetric balance, never mirrored symmetry
**Rule:** Compose one-sided: the focal element sits off-center (rule of thirds or harder), text and imagery weight one side, and the opposite side stays deliberately empty.
**Why:** Symmetry is static and western-formal; asymmetry creates the tension that makes emptiness feel intentional rather than unfinished.
**Example:** Hero: heading block flush left at one-third width; a single ikebana photo lower right; nothing on the diagonal's empty half.

### R4. Hairlines are the only structure
**Rule:** Separate and group with 1px ink rules (full- or partial-width); use a box or card only when interaction demands it, and never stack border + fill + shadow.
**Why:** Boxes add visual mass that fights the emptiness; a hairline draws attention to the boundary, not the container — and it survives themes without shadow mud.
**Example:** A pricing section with three columns divided by two vertical hairlines — no cards, no fills, no shadows.

### R5. CJK and vertical text are decoration, not content
**Rule:** Vertical writing and isolated kanji are art accents: `aria-hidden`, non-essential, never carrying meaning; real content stays in the product language, horizontal and readable — Japanese typography for content only if the product itself is Japanese-language.
**Why:** Characters that screen readers announce are noise; meaning users cannot read is a failure — the accent must be safe to ignore entirely.
**Example:** A hero's vertical 静 (stillness) column marked `aria-hidden="true"`, while the English headline beside it is real text.

### R6. Imagery: muted, sparse, seasonal
**Rule:** Photos are desaturated or lightly warm-toned, small against large emptiness, and rotate with the season; at most 1–2 images per view — no dense grids, no saturation.
**Why:** Loud photography is the loudest possible intrusion in a near-monochrome world; scarcity keeps each image an event.
**Example:** A ryokan site shows one misty courtyard photo per page at 60% saturation, surrounded by paper.

### R7. Motion is a slow breath
**Rule:** All motion is quiet fades and gentle shifts at 300–500 ms ease-out — no springs, no overshoot, no parallax, no loops — and reveals collapse to opacity-only or instant under `prefers-reduced-motion`.
**Why:** The style promises stillness; springy or showy motion is an aesthetic rupture, while slow fades remain calm enough to survive reduced-motion as simple crossfades.
**Example:** A section reveals once with a 400 ms fade + 8px rise on scroll-into-view; never again.

### R8. Zen is not Swiss
**Rule:** Do not drift into minimalist-swiss defaults: swiss is systematic grid discipline, neutral grays, and flush-left density; zen is asymmetric emptiness with warm paper, one natural hue, and organic texture. If the layout is grid-tight and gray, it is the wrong style.
**Why:** The two share restraint but differ in everything else; converging on swiss erases the warmth and imperfection that make zen legible as zen.
**Example:** Swiss: 12-column grid on `#FAFAFA`. Zen: one-third composition on washi `#F7F5F0`, one indigo accent, air as the grid.

## A11y watchpoints
- The muted palette is the trap: faded-ink grays fail 4.5:1 fast on washi — verify every pair on the actual paper color (sumi `#2B2B28` ≈ 13:1 is safe; `#8A8880`-class grays ≈ 3.3:1 are not)
- Structural hairlines must hold 3:1 against adjacent surfaces when they are the only boundary: 20% ink rules are decorative only — structural rules need ≥70% ink
- 300–500 ms motion is gentle but still honors `prefers-reduced-motion`: collapse reveals to instant or pure opacity
- Generous whitespace must not shrink targets: icon buttons and text links keep ≥24×24px hit areas even when visually tiny
- Text over seasonal photography needs a solid washi panel behind it — never direct overlay on the image
- Decorative CJK accents must be `aria-hidden` and absent from reading order — confirm with a screen-reader pass

## Checklist
- [ ] Section gaps ≥96px desktop; an element was removed rather than space added
- [ ] Exactly one natural accent hue in the whole product
- [ ] Focal points off-center; no mirrored symmetric heroes
- [ ] Structure is hairlines; boxes only where interaction demands, never stacked effects
- [ ] All CJK/vertical text decorative, `aria-hidden`, non-essential
- [ ] ≤2 muted images per view; zero saturated photography
- [ ] All motion 300–500 ms fades; nothing springy; reduced-motion honored
- [ ] Every text pair ≥4.5:1 on washi; structural hairlines ≥3:1
- [ ] Targets ≥24×24px despite the airy spacing

## Anti-patterns
- Symmetric centered hero with a centered CTA — template minimalism, not zen
- Faded gray body text "for atmosphere" below 4.5:1
- Cards with borders + fills + shadows on paper
- A second accent color, or a new hue per section
- Saturated lifestyle photography packed into grids
- Springy micro-interactions or scroll-jacked parallax on a still composition
- Random kanji as branding that screen readers announce as gibberish
- Promo banners and urgency badges — zen cannot survive marketing noise

## Sources & inspiration
- Ma (negative space) — the core concept: https://en.wikipedia.org/wiki/Ma_(negative_space)
- Wabi-sabi — beauty of the incomplete: https://en.wikipedia.org/wiki/Wabi-sabi
- Japanese aesthetics — kanso, yūgen, and the canon: https://en.wikipedia.org/wiki/Japanese_aesthetics
- Ink wash painting (sumi-e) — the brush reference: https://en.wikipedia.org/wiki/Ink_wash_painting
- Japanese garden — emptiness and borrowed scenery in practice: https://en.wikipedia.org/wiki/Japanese_garden
- Noto Serif JP — mincho-class web type: https://fonts.google.com/noto/specimen/Noto+Serif+JP
