# Style: Pop Art

> **Read when:** building UI in the pop-art style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/visual-hierarchy.md, 00/iconography.md

**Personality:** mass-culture, ironic, bold, graphic, loud. **Best for:** music, fashion drops, entertainment, snacks/FMCG, youth campaigns, gallery and comic shops. **Avoid for:** enterprise, finance, legal, any dense data product.

**Core idea:** comic book meets silkscreen — halftone dots, thick black outlines, speech bubbles, primary colors with hot accents, repetition as pattern; commercial imagery celebrated flat, like a printed page that never apologizes.

## Signature (what makes it recognizable)
- Halftone / Ben-Day dot textures inside imagery zones
- Thick black comic outlines (4–6px) with one thin (2px) secondary weight
- Speech bubbles and caption boxes doubling as UI containers
- Palette: paper-white or yellow grounds + primaries red/blue/yellow + hot pink/magenta accent
- Duotone silkscreen imagery — photos flattened into 1–2 plate colors
- Bold comic display type (all-caps) over a plain legible body sans
- Warhol-style 2×2 repetition grids as a featured motif
- Hard-offset shadows (`4px 4px 0 ink`) or none — never soft blur

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | panels sharp, bubbles pill | 0–4px on cards/panels; full pill for speech bubbles |
| `border-*` | two weights only | thick 4–6px + thin 2px, solid `#111111` |
| `shadow-*` | hard offset or none | `4px 4px 0 #111111`; never soft blur |
| `space-scale` | print-compact | 8pt scale; page margins generous, components tight |
| `color-bg` | paper white / yellow feature zones | `#FAF7EE` base, `#FFD400` feature zones |
| `color-ink / text` | black | `#111111` |
| `color-accent` | primaries with fixed roles | red `#E63946`, blue `#2C5FDB`, yellow `#FFD400` |
| `color-accent-2` | hot accent | magenta `#E93CAC` |
| `type-display` | bold comic display | Bangers-like, uppercase, 20–64px |
| `type-body` | plain legible sans | 16px / 1.5, scale ~1.25 |
| `halftone-dot` | imagery texture | 4–8px dots at 10–20% coverage, imagery zones only |
| `motion-duration` | pop snaps | 100–200 ms; scale 0.95→1 on press |
| `bubble-set` | reserved containers | speech-bubble + caption-box SVG kit |

## Rules

### R1. Halftone is texture for imagery, never for text
**Rule:** Ben-Day dot and halftone fills belong inside illustrated or photographic zones only; text always sits on a solid panel.
**Why:** Dots behind text shred letterform edges and make contrast unauditable; inside imagery they are the era's print signature.
**Example:** Product illustration filled with 6px red-on-cream dots; the price below sits on solid white.

### R2. One outline system: two weights
**Rule:** Exactly one thick weight (4–6px) for panels, bubbles, and heroes, and one thin weight (2px) for inner detail; never introduce a third.
**Why:** Comic art stays legible because line hierarchy is strict; three weights drift into clip-art chaos.
**Example:** Card border 5px, icon strokes 2px, bubble tail 5px — the same two values on every screen.

### R3. Speech bubbles are microcopy moments
**Rule:** Bubbles and caption boxes appear only for tips, quotes, and empty states — at most two per view — never as form fields, dialogs, or navigation.
**Why:** Bubbles read as "someone is speaking"; putting functional UI inside them makes interfaces look like jokes.
**Example:** Empty cart shows a bubble: "Nothing here yet — fix that!"; the checkout form itself stays plain.

### R4. Repetition is a motif, used once
**Rule:** The Warhol 2×2 echo (a featured image repeated four times, possibly recolored) may appear once per page as a hero or section motif — never as the default grid.
**Why:** Repetition is the style's punchline; overused it becomes wallpaper and flattens hierarchy.
**Example:** The landing hero shows the sneaker in a 2×2 with one magenta variant; product lists stay ordinary grids.

### R5. Flat silkscreen color only
**Rule:** No gradients, no soft shadows, no glass — fills are flat plate colors; depth comes from hard-offset black shadows or nothing at all.
**Why:** Pop art celebrates cheap flat printing; one soft gradient breaks the silkscreen illusion completely.
**Example:** Buttons are flat red with a 4px black offset; pressed state removes the offset and translates 4px down-right.

### R6. Comic display ≥20px, body stays plain
**Rule:** The comic display face works only at 20px and above (headlines, prices, badges); body copy, labels, and forms use a plain legible sans at 16px.
**Why:** Comic faces trade letterform clarity for character — below 20px they collapse, and forms need fast recognition, not costume.
**Example:** "50% OFF!" in display caps at 40px; the terms line under it in 16px system sans.

### R7. Motion pops, it does not float
**Rule:** Interactions snap: press scales 0.95→1 and returns at 100–200 ms; hovers swap fills or offsets; no fades longer than 200 ms, no loops.
**Why:** Comic panels don't drift — the punchy snap IS the medium's motion language.
**Example:** Clicking "Add to cart" pops the button scale and flashes the outline once — done in 150 ms.

## A11y watchpoints
- Yellow on white is the classic failure (~1.4:1): yellow is a plate/fill color, never text; every text pair is ink on a light solid
- Comic display faces need a 20px floor plus glyph-coverage checks — verify accented characters before committing
- Halftone behind text is banned; any text over an imagery zone is measured against its worst point or moved to a solid panel
- Outlines must not be the only state indicator — pair outline changes with fill shifts; focus is a 2px offset ring, never just a thicker border
- Plate colors carry no meaning alone — pair every color-coded state with an icon or text label
- Hard-offset shadows are decoration; focus visibility never relies on them

## Checklist
- [ ] Halftone appears only in imagery zones
- [ ] Exactly two outline weights (4–6px and 2px) site-wide
- [ ] ≤2 speech bubbles per view; none contain forms or nav
- [ ] 2×2 repetition motif max once per page
- [ ] Zero gradients, soft shadows, or glass effects
- [ ] Display face ≥20px; body 16px plain sans
- [ ] All text pairs ≥4.5:1 on solid panels
- [ ] Press feedback = 100–200 ms pop scale
- [ ] States change outline + fill together, never outline alone

## Anti-patterns
- Yellow or magenta text on white "because pop"
- Halftone dots running under paragraphs
- Speech bubbles as dialogs, error tooltips, or form containers
- A third outline weight "just for the footer"
- Gradient shading and soft shadows — modern gloss kills the silkscreen flatness
- Comic display face for body text, labels, or buttons
- Warhol grids on every section until repetition means nothing
- Liquid psychedelic swirls or memphis squiggles leaking in — wrong movements (psychedelia is the 60s fluid counterpart, memphis the 80s one)

## Sources & inspiration
- Pop art — the movement: https://en.wikipedia.org/wiki/Pop_art
- Ben-Day dots — the print signature: https://en.wikipedia.org/wiki/Ben-Day_dots
- Roy Lichtenstein — comic-panel canon: https://en.wikipedia.org/wiki/Roy_Lichtenstein
- Andy Warhol — repetition and silkscreen: https://en.wikipedia.org/wiki/Andy_Warhol
- Bangers — comic display face on Google Fonts: https://fonts.google.com/specimen/Bangers
- MoMA — pop art collection: https://www.moma.org
