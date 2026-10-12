# Style: Memphis Design

> **Read when:** building UI in the memphis style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/motion-principles.md, 00/visual-hierarchy.md

**Personality:** irreverent, loud, postmodern, artistic-playful. **Best for:** creative agencies, music, youth brands, art events, streetwear, festival sites. **Avoid for:** banking, healthcare, enterprise, long-session tools.

**Core idea:** deliberate "bad taste" as a system — clashing brights on pastel, squiggle-and-confetti geometry, terrazzo speckle, tilted elements; the chaos is curated through a fixed palette and pattern set, so it reads as an art movement, not as an accident.

## Signature (what makes it recognizable)
- Squiggles, zigzags, confetti dots, and terrazzo speckle patches
- Clashing palette: pastel backgrounds (mint, pink, baby-blue) with brights (red, yellow, cyan, magenta) and black outlines
- Black-outlined flat shapes — every element traced like a drawing
- Tilted and rotated cards (≤15°) that refuse the grid politely
- Bold geometric sans or chunky display type over a plain body sans
- Pattern zones pushed to edges and corners, framing calm content
- Terrazzo speckle as section dividers and background patches

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | mixed postmodern | 0–8px on cards; squiggles/blobs are freeform vectors |
| `border-*` | uniform black outline | 2–3px solid `#111111` on shapes and cards |
| `shadow-*` | flat or hard | none at rest; optional `3px 3px 0 #111111` for featured cards |
| `space-scale` | standard, generous in calm zones | 8pt scale |
| `color-bg` | one pastel family | mint `#CDEBD4`, pink `#F7C9D4`, or baby-blue `#C9DCF2` |
| `color-surface` | white-warm | `#FDFBF4` |
| `color-ink / text` | near-black | `#111111` |
| `color-accent` | bright set, fixed roles | red `#E8384F`, yellow `#FFD23F`, cyan `#35C4E8`, magenta `#E754B8` |
| `type-display` | chunky geometric sans | Archivo Black-like, 32px+ |
| `type-body` | plain legible sans | 16px / 1.5, scale ~1.25 |
| `motion-duration` | bouncy allowed | 200–400 ms with slight overshoot; reduced-motion → fades |
| `pattern-set` | one SVG library | squiggle, zigzag, confetti, terrazzo — single stroke weight |
| `tilt-max` | bounded rotation | 15°, max 2 tilted elements per view |

## Rules

### R1. The clash is curated
**Rule:** Lock a fixed 5-color palette (1–2 pastels + 2–3 brights + black) before designing; the page may look chaotic but it never uses a sixth color.
**Why:** Memphis reads as intentional art because the color set is finite; unlimited brights collapse it into birthday-card noise.
**Example:** Mint bg + black ink + red/yellow/cyan brights everywhere — magenta was tempting but stays unused all project long.

### R2. Pattern budget
**Rule:** Patterns (squiggles, zigzags, confetti, terrazzo) live at edges, corners, and divider bands; they never run behind body text or form fields.
**Why:** Patterns are the style's noise floor — under text they destroy legibility and break the "calm center in a loud frame" composition.
**Example:** A confetti band across the top 80px, squiggles tucked in the corner; the article text sits on a solid pastel panel.

### R3. Tilt budget
**Rule:** Maximum 2 tilted elements per view, each ≤15°; everything else stays axis-aligned.
**Why:** Tilt is spice, not diet — beyond two rotations the layout reads as broken rendering instead of deliberate play.
**Example:** One featured card rotated -8° with a hard black shadow; its neighbor +5°; all other cards square.

### R4. One shape and pattern asset set
**Rule:** Every squiggle, zigzag, confetti dot, and terrazzo speckle comes from one SVG library with a single stroke weight; scale assets, never redraw them.
**Why:** Consistent line weight is what makes the chaos look systematic; mixed weights read as clip-art collage.
**Example:** The same 3px-stroke squiggle appears at 40px and 160px — never a hand-drawn variant.

### R5. Text is ink-on-solid
**Rule:** All text renders as the single dark ink on a solid pastel or white surface — never on brights, patterns, or gradients.
**Why:** Clashing hues fail contrast as text pairs by definition; ink-on-solid is the a11y backbone that lets everything else stay loud.
**Example:** Yellow is a shape fill, never a headline color; the headline itself is black on mint.

### R6. Bounce is a language — with an off switch
**Rule:** Motion may overshoot slightly (200–400 ms springy ease) for hovers and entrances; under `prefers-reduced-motion` every bounce collapses to a plain fade.
**Why:** The postmodern wiggle is part of the voice, but repeated bouncing is a vestibular hazard and a fatigue tax.
**Example:** Card entrance springs 1.02→1.0 at 320 ms; reduced-motion users see the same card fade in at 150 ms.

### R7. Short-visit style
**Rule:** Reserve memphis for expressive, short-session surfaces (landers, event pages, campaign heroes); forms, checkout, tables, and dashboards render in a calm neutral chrome.
**Why:** Loudness taxes accumulate over long sessions (see style-catalog R3); product warmth belongs to `playful-friendly`, era-print discipline to `retro-vintage`'s 80s mode.
**Example:** Festival landing is full memphis; the ticket checkout is white background, black text, memphis only in the header chrome.

## A11y watchpoints
- In a multi-hue system only ink-on-solid text pairs are allowed; verify every ink/pastel pair at 4.5:1 (black on all three pastels passes comfortably)
- Patterns behind text are banned — a low-vision failure, not just a preference
- Vestibular safety: bounce/overshoot must respect `prefers-reduced-motion`; no autoplaying swaying loops
- Never signal state by hue alone in a multi-hue system — pair color with icon or text (a red badge also reads "Error")
- Focus must stay visible against decorative black outlines: use a 2px offset ring at 3:1, not just a thicker black border

## Checklist
- [ ] Palette locked to exactly 5 colors; zero ad-hoc hues
- [ ] Patterns only at edges/corners/dividers; none behind text
- [ ] ≤2 tilted elements per view, each ≤15°
- [ ] All shapes/patterns from one asset set, one stroke weight
- [ ] All text ink-on-solid, pairs verified at 4.5:1
- [ ] Bouncy motion 200–400 ms; reduced-motion collapses to fades
- [ ] Focus ring visible and distinct from decorative outlines
- [ ] Forms/tables/checkout in calm neutral chrome
- [ ] One loud "wow" zone per view; the rest supports it

## Anti-patterns
- A sixth color "just for this banner" — the curated clash dies with palette creep
- Squiggle and terrazzo patterns running under paragraphs
- Tilted form fields, tilted buttons, tilted everything
- Text on patterns or bright-on-bright pairs (magenta on red) — unreadable by construction
- Autoplaying bounce that ignores `prefers-reduced-motion`
- Ten squiggle stroke weights from ten different asset packs
- Memphis inside banking or healthcare "to seem fun" — wrong genre contract
- Confusing this with `retro-vintage`'s 80s mode: that file locks era-print discipline (badges, halftone, hard shadows); this is the movement's clash itself

## Sources & inspiration
- Memphis Group — the 1981 Milan collective: https://en.wikipedia.org/wiki/Memphis_Group
- Ettore Sottsass — founder and palette brain: https://en.wikipedia.org/wiki/Ettore_Sottsass
- Terrazzo — the speckle signature: https://en.wikipedia.org/wiki/Terrazzo
- Postmodern design — the broader movement: https://en.wikipedia.org/wiki/Postmodern_design
- Archivo Black — chunky display sans on Google Fonts: https://fonts.google.com/specimen/Archivo+Black
- V&A — holds Memphis pieces in its collections: https://www.vam.ac.uk
