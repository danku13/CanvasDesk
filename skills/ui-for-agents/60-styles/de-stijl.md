# Style: De Stijl

> **Read when:** building UI in the de-stijl style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/spacing-layout-grids.md, 00/visual-hierarchy.md

**Personality:** orderly, essentialist, universal, austere. **Best for:** creative portfolios, art/culture sites, music apps, brand statements. **Avoid for:** dense data tools, kids products, long editorial reading.

**Core idea:** the Mondrian composition as interface — a white canvas divided by thick black orthogonal lines into asymmetric cells, a few filled with primary color; every element lives inside a cell and the white ground stays dominant.

**Boundary:** not bauhaus — De Stijl bans diagonals and curves, leans on heavier black lines, and keeps ~60–70% of the surface white where Bauhaus plays with circles and asymmetric mass.

## Signature (what makes it recognizable)
- Verticals and horizontals only — diagonals are historically banned (van Doesburg was expelled over them)
- Thick black lines at exactly two weights: structural 6–10px, detail 2–4px
- Palette locked to red, yellow, blue + black, white, gray — no fourth hue ever
- Zero curves anywhere: cards, buttons, avatars, images, icons — all rectangular
- Asymmetric cell sizes on a strict grid; white occupies ~60–70% of the surface
- Flat: no shadows, no gradients, no rounded corners
- Colored cells are landmarks — nav, primary CTA, one feature block — at most 3 per view
- Typography flush to cell edges, geometric sans, generous cell padding

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | zero | 0 everywhere, no exceptions |
| `shadow-*` | none | flat planes; hierarchy via line weights |
| `line-weights` | exactly two | 8px structural, 3px detail, solid `#0A0A0A` |
| `space-scale` | grid-derived | 8pt; cell gutters ≥ structural line weight |
| `color-bg` | white dominant | `#FFFFFF` (60–70% of any view) |
| `color-surface` | gray cell | `#E9E9E9` |
| `color-ink` | line black / text | `#0A0A0A` |
| `color-accent` | red cell (primary CTA) | `#DD0100` |
| `color-accent-2` | yellow cell (highlight/nav) | `#FACA03` |
| `color-accent-3` | blue cell (information) | `#225095` |
| `type-display` | geometric sans | Futura-class 500–700 |
| `type-body` | same family | 16px / 1.5 |
| `motion-duration` | sparse and exact | 150–200 ms, ease-out; opacity and color only |

## Rules

### R1. Orthogonal-only layout
**Rule:** Every divider, edge, image crop, and motion path is strictly vertical or horizontal; rotated elements, diagonal rules, and skewed blocks are refused.
**Why:** The right angle is the movement's ethical core — the "universal order" of neoplasticism collapses the moment diagonals enter (the split with van Doesburg was literal).
**Example:** A marquee strip runs full-width horizontally, never tilted; images crop to rectangles even when the subject is round.

### R2. Two line weights, never three
**Rule:** One structural weight (6–10px) divides zones; one detail weight (2–4px) divides items inside a zone; no third weight and no hairlines.
**Why:** Line weight IS the hierarchy in this style — a third weight reintroduces the fuzziness the flat grid exists to abolish.
**Example:** Page divided by 8px black lines; list rows inside one cell separated by 3px lines; nothing at 1px.

### R3. At most 3 colored cells, and they are landmarks
**Rule:** Reserve color for wayfinding and action — nav block, primary CTA, one featured block — capped at three colored cells per view; content cells stay white or gray.
**Why:** In a mostly-white composition a colored cell is a shout; three shouts per screen is the ceiling before it becomes noise.
**Example:** Home page: blue nav cell top, red "Book tickets" cell, one yellow feature cell — every other cell white.

### R4. Curves are banned, including icons
**Rule:** Border-radius 0 everywhere; icons are straight-line constructions; avatars and media render as rectangles; toggles are square sliders.
**Why:** The vocabulary contains only straight lines and right angles — one circle reads as a bug, two read as a different style.
**Example:** The play button is a black square with a triangle cut from straight edges, not a circle.

### R5. Whitespace is a cell, not emptiness
**Rule:** Treat white areas as designed cells with intentional proportions and position; never dump leftover space or pad cells "until it looks fine".
**Why:** In neoplasticism the white ground is an active element of the composition — accidental whitespace breaks the balance the cells create.
**Example:** A 2/3-width white cell holding one 16px line is deliberate; stuffing that cell with a paragraph changes the whole page's balance.

### R6. Buttons are colored cells with black lines
**Rule:** Primary buttons are rectangular red or blue cells with black text and a black border; hover inverts cell/text or steps to a neighboring primary; no ghost buttons, no pills.
**Why:** A control must obey the same cell grammar as the layout — pill or soft-outline buttons belong to other systems and read off-style.
**Example:** "Order print" = red cell, 2px black border, black uppercase text; hover swaps to black background/white text in 150ms.

### R7. Every element lives inside a cell
**Rule:** No floating elements, no overlaps, nothing crossing lines: modals, toasts, and badges are rectangular cells snapped to the grid.
**Why:** Overlaps reintroduce depth and hierarchy-by-layering, which the flat plane refuses; snapped cells also keep DOM order and visual order identical.
**Example:** A toast appears as a white cell with a 3px black border docked to the bottom edge — never a rounded floating card.

### R8. Type aligns flush to cell edges
**Rule:** Text starts at the cell's inner edge (inset = one space step) and aligns left; centering is allowed only inside symmetric CTA cells; lines never cross cell borders.
**Why:** Flush alignment binds typography to the grid the way paint sits on Mondrian's canvas — floating centered text dissolves the cell logic.
**Example:** Feature cell: heading and body both start 24px from the cell's left line; only the CTA cell centers its single line.

## A11y watchpoints
- Yellow `#FACA03` takes black text only (~13.5:1); white-on-yellow (~1.6:1) and yellow text on white (~1.5:1) are hard fails — lock the pair in tokens
- On red `#DD0100`, white text ≈5.1:1 passes but black ≈4.1:1 fails — verify every text-on-color pair at 4.5:1 instead of assuming black works
- Blue `#225095` on white ≈7.9:1 is safe for text; re-verify on gray `#E9E9E9` cells, and never set muted gray text on gray cells
- Thick black lines are decorative structure — interactive elements still need their own ≥3:1 state indication (focus outline offset from the line, hover cell swap)
- Asymmetric cells must keep DOM order equal to visual order; a Z-shaped visual flow with different DOM order confuses screen readers
- Radius-0 corners plus flat color give no hover affordance — pair state changes with a border or cell inversion, never color alone

## Checklist
- [ ] Zero diagonals and zero curves anywhere, icons included
- [ ] Exactly two line weights (6–10px structural, 2–4px detail)
- [ ] ≤3 colored cells per view, all serving wayfinding or action
- [ ] White covers ~60–70%; gray cells secondary
- [ ] All text-on-color pairs verified ≥4.5:1
- [ ] Buttons and controls are cells obeying the grid
- [ ] No overlaps; modals and toasts are snapped cells
- [ ] DOM order matches visual cell order
- [ ] Motion 150–200 ms, opacity/color only
- [ ] Radius 0 with visible non-color focus states

## Anti-patterns
- "Dynamic Mondrian" — tilted cells or diagonal dividers
- Circle avatars, pill buttons, rounded modals
- Five colored cells "to celebrate the palette"
- Three or more line weights, or timid 1px hairlines
- Drop shadows or gradients under cells
- Muted gray paragraphs on gray cells
- Floating badges overlapping cell borders
- Centering everything until the asymmetry dies

## Sources & inspiration
- De Stijl — the movement and principles: https://en.wikipedia.org/wiki/De_Stijl
- Piet Mondrian — compositions and palette: https://en.wikipedia.org/wiki/Piet_Mondrian
- Theo van Doesburg — the diagonal controversy: https://en.wikipedia.org/wiki/Theo_van_Doesburg
- Gerrit Rietveld — the style built in space: https://en.wikipedia.org/wiki/Gerrit_Rietveld
- Tate — De Stijl art term: https://www.tate.org.uk/art/art-terms/d/de-stijl
