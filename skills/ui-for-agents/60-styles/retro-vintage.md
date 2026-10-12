# Style: Retro Vintage

> **Read when:** building UI in the retro-vintage style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/motion-principles.md

**Personality:** nostalgic, warm, characterful, crafted. **Best for:** craft brands, music/vinyl, bars & restaurants, indie games, nostalgia products. **Avoid for:** fintech, medical, enterprise tools, data-dense dashboards.

**Core idea:** nostalgia comes from committing to ONE era and executing it like a print shop — limited inks, period display type, honest texture — while the working interface (body text, forms, buttons) stays quietly modern and legible.

## Signature (what makes it recognizable)
- Era-locked palette: exactly one decade chosen (70s earth tones / 80s pastel-memphis / 90s web), 4–6 inks mapped to fixed roles
- Period display type for headlines only: slab serif (70s), groovy rounded curves (80s), or pixel faces (90s)
- Texture overlays at 3–8% opacity — paper grain, halftone dots, film dust — always behind content, never on text
- Badges, stamps, and thick frames used as containers for headers, prices, and CTAs
- Hard-offset shadows (`4px 4px 0 ink`) or no shadows at all; never soft modern blurs
- Imagery period-true: duotone or halftone-processed photos, or no photography at all
- Decorative rules and ornaments appear at most once per section

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | era-dependent | 70s: 8–16px soft; 80s: 4–16px mixed; 90s: 0–2px hard |
| `shadow-*` | hard-offset or none | `4px 4px 0 ink` (80s/90s); none for 70s flat print |
| `space-scale` | standard density | 8pt scale, no extra air — print was compact |
| `color-bg` | paper or era color | cream `#F4EBD9` (70s), peach/mint `#FBE8DC` (80s), light gray `#E8E8E8` (90s) |
| `color-palette-*` | 4–6 fixed inks | ink, bg, accent-1, accent-2, alert — mapped to roles once |
| `color-accent` | era ink, medium saturation | 70s olive/burnt orange; 80s teal/magenta; 90s navy/purple |
| `type-display` | period face, display only | slab serif / groovy / pixel; used at ≥20px, never below |
| `type-body` | plain legible sans | system sans 16px, line-height 1.5, scale ~1.25 |
| `motion-duration` | minimal or none | 150–250 ms ease-out for state changes; zero decorative motion |
| `texture-overlay` | constant low voice | grain/halftone at 3–8% opacity on a layer below content |

## Rules

### R1. One era, committed
**Rule:** Pick a single decade (70s / 80s / 90s) before any styling and pull every token — palette, type, radius, texture — from that era's artifacts; never blend decades.
**Why:** Mixing decades reads as a costume party, not a style: users feel the uncertainty, and every new element restarts the "which era is this?" question.
**Example:** A vinyl store: 70s cream bg, olive/burnt-orange inks, slab-serif headers, grain texture — zero pixel fonts, zero memphis shapes.

### R2. Period type is display-only
**Rule:** Groovy, slab, or pixel faces appear only at display sizes (≥20px) on headlines, badges, and prices; body text, labels, and buttons use a plain legible sans.
**Why:** Period faces trade letterform clarity for character; at reading sizes they slow recognition and break forms — readability is not nostalgic.
**Example:** "TUESDAY VINYL NIGHT" in pixel display; the event description below it in 16px system sans.

### R3. Texture speaks quietly, from behind
**Rule:** Grain and halftone layers sit at 3–8% opacity below content; every text zone gets a solid backing surface so contrast is measured against color, not texture noise.
**Why:** Texture is the era's atmosphere — foregrounded it becomes noise, makes contrast unpredictable, and ruins the print illusion it was meant to create.
**Example:** Hero photo carries a 6% halftone overlay; the ticket form sits on a solid cream panel above it.

### R4. Palette like screen-print
**Rule:** Define 4–6 ink swatches total, assign each a fixed role (ink, bg, accent-1, accent-2, alert), and never introduce a seventh color — accents are spot colors, not gradients.
**Why:** Screen-printing's charm is limitation; a role-mapped limited palette keeps every screen coherent and makes contrast pairs auditable.
**Example:** Menu site: ink brown = text, cream = bg, mustard = highlights, brick red = prices and alerts, olive = tags. Nothing else.

### R5. Ornaments frame, they do not fill
**Rule:** Badges, stamps, thick borders, and dividers wrap or mark existing content — section headers, featured items — at most one ornament per section; they never wrap paragraphs or sit inside dense lists.
**Why:** Ornaments signal importance; scattered everywhere they compete with content and the page becomes a sticker sheet.
**Example:** A "House Special" badge on one dish card; the other thirty dishes keep the plain card.

### R6. Shadows are hard or absent
**Rule:** Use hard-offset shadows (`4px 4px 0` ink at full opacity) for the 80s/90s feel, or flat print with borders for the 70s; no soft blurred drop shadows, no glow.
**Why:** Soft modern shadows belong to the soft-UI era and instantly break the print illusion; hard offsets read as stacked print plates.
**Example:** Buttons press by shrinking the shadow from 4px to 0 with a 1px translate — a physical "print button" feel.

### R7. Imagery is period-true or absent
**Rule:** Photos are duotoned or halftoned into the palette inks and framed in period style (thick border, era radius); if the image cannot be processed into the era, use illustration or none.
**Why:** A raw modern color photo screams stock and breaks the illusion more than any missing decoration; processed imagery unifies the tone.
**Example:** Band photo as an olive/cream duotone with a 2px ink frame; never a full-color unprocessed shot.

### R8. Nostalgia wraps the product, not the workflow
**Rule:** Forms, tables, checkout, and settings keep modern structure — visible labels, single column, 8pt spacing, 24×24px targets — with era styling only on chrome (borders, headers, buttons).
**Why:** Users navigate with modern conventions; making the checkout as awkward as 1997 is a bug, not authenticity.
**Example:** Checkout uses a standard modern form; the submit button and panel borders carry the era.

### R9. Motion is a print shop's: almost none
**Rule:** State changes animate at 150–250 ms ease-out; no marquees, blinks, parallax, or looping textures — a 90s gimmick may appear only as a documented one-off on the marketing hero and must die under reduced-motion.
**Why:** These eras were static media (print, early web); decorative motion reads as anachronistic and adds fatigue to a style whose charm is stillness.
**Example:** Hovering a card shifts its hard shadow and border color; nothing else moves.

### R10. Steal from real artifacts, not from vibes
**Rule:** Build the palette and type from actual era sources — scanned posters, menus, print ads, archived websites — sampling real values instead of approximating "old-looking".
**Why:** Approximated nostalgia converges on cliché (sepia + Western font); sampled values carry the era's actual color logic and instantly look right.
**Example:** Eyedrop ink colors from a 1972 concert poster and adopt its title face — not a "vintage-styled" revival mixing five eras.

## A11y watchpoints
- Text over texture: measure 4.5:1 against the WORST point of the textured zone, not the average — or back all text with solid panels (the style default)
- Period display faces collapse below ~20px and many lack complete glyph sets: enforce a 20px display floor and verify accented characters before committing
- Era palettes love low-contrast pairs (olive on cream, pastel on pastel): verify every text pair at 4.5:1 and every UI boundary at 3:1; swap in the dark ink where needed
- A limited ink count means status colors collide with accents: never signal state by hue alone — always pair color with an icon or text
- Reduced-motion is nearly free here (the style is static), but kill any blink/marquee exceptions; keyboard focus stays a visible 2px ink outline on every badged button

## Checklist
- [ ] Exactly one era chosen; zero elements from other decades
- [ ] Period display type ≥20px on headlines/badges only; body is plain sans ≥16px
- [ ] Palette = 4–6 inks, each mapped to one role, reused everywhere
- [ ] Texture ≤8% opacity; every text zone has a solid backing
- [ ] All text pairs ≥4.5:1 against actual (not theoretical) backgrounds
- [ ] Shadows hard-offset or none; no soft blurs or glows
- [ ] Ornament count ≤1 per section; none wrap body text
- [ ] Forms and tables keep modern structure and 24×24px targets
- [ ] Motion 150–250 ms functional only; no blink/marquee (or reduced-motion-safe)

## Anti-patterns
- Decade soup: 70s palette + 80s memphis shapes + 90s pixel font on one screen
- Pixel or groovy fonts for paragraphs, form labels, or button text
- Heavy grain or halftone running under paragraphs "because print wasn't perfect"
- Sepia-on-sepia low-contrast text "for atmosphere"
- Badges and stamps on every element until the page is a sticker sheet
- Soft modern shadows and glassy gradients under a retro palette
- Auto-playing 90s gimmicks: blink, marquee, visitor counters, cursor trails
- Recreating 1997 forms and navigation literally — usability is not period-correct

## Sources & inspiration
- Web Design Museum — archived 90s web design artifacts: https://www.webdesignmuseum.org
- Memphis Group — the 80s postmodern design movement: https://en.wikipedia.org/wiki/Memphis_Group
- Screen printing — the limited-ink logic this palette mimics: https://en.wikipedia.org/wiki/Screen_printing
- Halftone — the period photo treatment signature: https://en.wikipedia.org/wiki/Halftone
- Slab serif — the 70s display workhorse: https://en.wikipedia.org/wiki/Slab_serif
- Coolors — building and locking era palettes: https://coolors.co
