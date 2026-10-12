# Style: Mid-Century Modern

> **Read when:** building UI in the mid-century-modern style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/spacing-layout-grids.md, 00/motion-principles.md

**Personality:** optimistic, warm, retro-futuristic, domestic, confident. **Best for:** lifestyle brands, furniture/interior, food & packaging, marketing sites with character, boutique hotels. **Avoid for:** enterprise tools, dense dashboards, fintech, data-heavy products.

**Core idea:** atomic-age optimism — organic irregular shapes (amoebas, boomerangs, starbursts, dots), a silkscreen 4-color palette on cream, and geometric-humanist type with occasional retro script: the Eames-era confidence that the future is friendly.

## Signature (what makes it recognizable)
- Fixed shape vocabulary: amoeba blobs, boomerangs, starbursts, atomic orbits, confetti dots — the same set everywhere
- Silkscreen palette on cream `#F6EFDD`: mustard `#E0A526`, teal `#2A7F7F`, burnt orange `#C75B39`, brown-black ink `#2B2320`
- Thin 2px ink outlines define shapes, cards, and illustration alike
- Flat illustration leads; photography is secondary, framed, or omitted
- Googie asymmetry: playful off-axis layouts, shapes floating past the grid edge
- Subtle paper texture (≤6%) warming the background
- Geometric sans display (Futura-class) with retro script as a rare accent

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | soft, some organic | 8–24px; blobs use irregular radii (e.g. `60% 40% 55% 45%`) |
| `shadow-*` | flat print | none; optional `2px 2px 0 ink` hard offset for raised moments |
| `space-scale` | domestic, airy | 8pt scale, +1 step of air in hero sections |
| `color-bg` | cream paper | `#F6EFDD` |
| `color-surface` | lighter warm | `#FBF7EC` |
| `color-ink / text` | brown-black | `#2B2320` |
| `color-accent` | mustard (shapes) | `#E0A526` |
| `color-accent-2 / -3` | teal / burnt orange | `#2A7F7F` / `#C75B39` |
| `type-display` | geometric sans, script accent | Jost-like 32–56px; script face only ≥24px |
| `type-body` | humanist sans | 16px / 1.5, scale ~1.25 |
| `motion-duration` | light, friendly | 200–300 ms, ease-out; no bounce |
| `texture-overlay` | paper grain | ≤6% opacity, behind chrome only |
| `shape-set` | fixed SVG kit | amoeba, boomerang, starburst, orbit, dot — one library |

## Rules

### R1. One shape vocabulary, reused everywhere
**Rule:** Draw decoration only from the fixed shape set (amoeba, boomerang, starburst, orbit, dot) with one stroke weight and fill treatment; never invent a new shape mid-project.
**Why:** The style is recognizable because the same shapes recur at every scale — ad-hoc shapes dissolve it into generic "playful".
**Example:** Hero orbit rings, section-divider boomerang, and the CTA starburst all come from the same SVG kit at different sizes.

### R2. Four-color print discipline
**Rule:** Treat the palette like silkscreen plates: cream is paper, ink is text and line, and mustard/teal/burnt-orange each own a fixed role (energy, calm, appetite); no fourth hue gets added.
**Why:** Mid-century print shops had exactly the inks on the shelf; role-mapped limitation is what keeps the warmth from turning into a candy shop.
**Example:** Food site: mustard = highlights and shapes, teal = links and tags, burnt orange = prices and "hot" badges, ink = all text.

### R3. Illustration leads, photography follows
**Rule:** Prefer flat thin-line illustration in one house style; when photos appear, crop or mask them into shapes from the shape set — never introduce a third visual language.
**Why:** The era's optimism was drawn, not photographed; competing imagery styles break the print illusion faster than any missing ornament.
**Example:** Menu items as flat line art; the single lobby photo sits inside an amoeba-shaped mask.

### R4. Script is display-only, ≥24px
**Rule:** The retro script face appears only on headlines, badges, and one-word accents at 24px or larger; body copy, labels, and buttons use the geometric sans.
**Why:** Script trades letterform clarity for charm; at reading sizes it slows recognition and collapses on forms and navigation.
**Example:** "Welcome" in script above a 32px sans headline; the button below reads "Book a room" in plain sans.

### R5. Texture behind chrome, never behind text
**Rule:** Paper grain stays ≤6% opacity on background layers; every text block sits on a solid cream or surface panel with contrast measured against the solid color.
**Why:** Texture under text makes contrast unpredictable and ruins the print-flat look it exists to create.
**Example:** A 5% grain on the page background; the newsletter form on a solid `#FBF7EC` card.

### R6. Asymmetric balance, not centered symmetry
**Rule:** Compose layouts off-axis — hero content offset with a starburst balancing it, cards staggered — while the underlying 8pt grid stays intact.
**Why:** Googie energy comes from dynamic imbalance; centered symmetry flattens the style into a template.
**Example:** Hero text starts at column 2 of 6, an amoeba blob bleeds off the right edge, a starburst marks the CTA.

### R7. Motion is light and quick
**Rule:** Animate state changes at 200–300 ms ease-out — fades, small slides, a one-time starburst pop on hover; no bounce, no looping animation.
**Why:** The era's media were print and broadcast idents: motion confirms causality, it does not perform.
**Example:** Card hover lifts 4px with a 240 ms fade; the featured starburst scales 1.0→1.06 exactly once.

### R8. Era lock: this is 1945–1969
**Rule:** Pull every artifact from the mid-century era; anything from the 70s, 80s, or 90s (memphis squiggles, pixel fonts, neon gradients, VHS glitches) belongs to `retro-vintage.md` or `memphis.md`, not here.
**Why:** Movements blur fast for non-specialists; the era boundary is what keeps each preset distinct and credible.
**Example:** Boomerang and starburst are correct; a squiggle, a pixel font, or a marquee is the wrong decade.

## A11y watchpoints
- Mustard `#E0A526` on cream fails contrast for text (~1.9:1) — mustard is a shape/fill color only; every text pair uses ink `#2B2320`
- Teal and burnt orange pass 3:1 (large text, UI boundaries) but fail 4.5:1 on cream — darken for body-size text (e.g. teal → `#1F6161`)
- Thin 2px outlines are structural boundaries: keep them ≥3:1 against adjacent fills; ink outlines pass everywhere
- Script faces never carry meaning alone — pair with DOM text; never use script for links, errors, or required labels
- Texture behind text is banned; any text over a textured zone is measured against its worst point or moved to a solid panel

## Checklist
- [ ] All decoration drawn from the fixed shape set; no ad-hoc shapes
- [ ] Palette = cream + ink + 3 print hues, each mapped to one role
- [ ] No retro hue used as body-size text on cream (4.5:1 verified with ink)
- [ ] Script ≥24px on headlines/badges only
- [ ] Texture ≤6% and never behind body text
- [ ] Layouts asymmetric on an intact 8pt grid
- [ ] Illustration consistent: one line weight, flat fills
- [ ] Motion 200–300 ms ease-out; no bounce, no loops
- [ ] Zero 70s/80s/90s artifacts (no squiggles, pixel fonts, neon)

## Anti-patterns
- Mustard or teal paragraph text on cream "because the palette allows it"
- Memphis squiggles and clashing brights — warm organic optimism is this style; the loud postmodern clash is another
- A starburst on every card until the page is a fireworks show
- Soft modern shadows, glass blur, or gradient mesh under a flat print style
- Symmetrical centered hero "to be safe" — kills the Googie energy
- Script for buttons, labels, or body text
- Photography colliding with illustration in competing styles
- Inverting to near-black for dark mode — the cream warmth dies; choose `dark-premium` instead

## Sources & inspiration
- Mid-century modern — the movement overview: https://en.wikipedia.org/wiki/Mid-century_modern
- Googie architecture — the commercial 50s/60s exuberance: https://en.wikipedia.org/wiki/Googie_architecture
- Atomic Age design — starbursts and orbit motifs: https://en.wikipedia.org/wiki/Atomic_Age
- Charles and Ray Eames — the era's canon: https://en.wikipedia.org/wiki/Charles_and_Ray_Eames
- Jost — a Futura-class geometric sans on Google Fonts: https://fonts.google.com/specimen/Jost
- Eames Office — archives and authentic artifacts: https://www.eamesoffice.com
