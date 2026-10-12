# Style: Art Nouveau

> **Read when:** building UI in the art-nouveau style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/iconography.md

**Personality:** ornamental, organic, romantic, elegant. **Best for:** beauty/cosmetics, florists, boutique hotels, tea/cafes, jewelry, wedding, artisan food. **Avoid for:** dev tools, dashboards, dense data.

**Core idea:** whiplash organic line and botanical ornament frame the content like an illuminated manuscript — a modern, fully legible core wrapped in hand-drawn nature.

**Boundary:** not handcrafted-organic (that style is modern craft/eco with flat warmth and minimal ornament; art-nouveau is period ornamental — arches, vines, whiplash lines, stained-glass tints).

## Signature (what makes it recognizable)
- Whiplash/serpentine curves — the drawn line itself is the primary ornament
- Floral and vine ornament as FRAMES: arches, borders, corner vines — never wallpaper behind text
- The arch container (rounded-top window) as the signature geometry for cards, images, CTAs
- Stained-glass color patches: muted jewel tints inside line-drawn panels
- Palette: cream/champagne `#F7F1E3`, deep olive-sepia ink `#4A4132`, antique gold `#B08D3E`, mauve `#9C7183` / sage `#8A9A7B` accents
- Ornamental display face (high-contrast, decorated) at display sizes; humanist sans or old-style serif body
- Mucha-style line-work illustration: hairline figures and botanicals at one consistent stroke

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | arch signature, minimal elsewhere | arch containers: top corners fully round; other elements 2px |
| `shadow-*` | none | depth via gold rules and line-work |
| `space-scale` | generous | 8pt; section gaps ≥48px; measure 45–75ch |
| `color-bg` | cream/champagne | `#F7F1E3` |
| `color-surface` | pale linen | `#FDFAF2` |
| `color-ink` | deep olive/sepia | `#4A4132` |
| `color-accent` | antique gold — lines only | `#B08D3E` |
| `color-accent-2/3` | mauve / sage washes | `#9C7183` / `#8A9A7B` (panels in light tints with ink text) |
| `type-display` | ornamental period face | ≥24px, display only |
| `type-body` | humanist sans or old-style serif | 16px / 1.6 |
| `motion-duration` | gentle drift | 200–300 ms, ease-out; no springs |
| `ornament-stroke` | fixed line-work weight | 1.5px SVG stroke, one set |
| `texture` | paper grain | ≤4% opacity, never over text |

## Rules

### R1. Ornament frames, never fills
**Rule:** Vines, whiplash borders, and corner ornament wrap around or above content zones; no ornament runs behind paragraphs, forms, or tables.
**Why:** Behind text, ornament destroys legibility and makes contrast unpredictable; as a frame it signals importance — as wallpaper it is noise.
**Example:** The tea-menu heading carries a corner vine and a gold rule; the menu list below sits on bare cream.

### R2. The arch is the one container geometry
**Rule:** Reuse a single arch system (fully rounded top, flat base, fixed proportions) for feature cards, images, and the primary CTA; secondary containers stay rectangular with 2px radius.
**Why:** The arch is this style's recognizable silhouette — one system keeps it architectural; mixed container shapes dissolve it into generic rounded corners.
**Example:** Product cards are arches with gold line borders; the checkout form fields stay plain 2px rectangles.

### R3. One focal ornament per section
**Rule:** Budget one hero ornament (arch frame, illustration, decorative initial) per section; all other structure is typography, spacing, and thin gold rules.
**Why:** Art Nouveau fails not by lacking ornament but by drowning in it — density everywhere erases the focal moments that make ornament read as luxury.
**Example:** Collection page: one Mucha-style figure illustration at top; the rest of the page earns a vine corner at most.

### R4. Gold is a line, never a text fill
**Rule:** Gold appears as borders, rules, initials, and ornament strokes; text is ink `#4A4132` (or ivory on dark panels) — gold text is refused even for prices.
**Why:** Gold on cream computes ≈2.8:1 — it cannot carry text contrast, and as a fill it turns shimmer into mud; as a line accent it reads as gilding.
**Example:** Price set in 20px ink serif with a 1.5px gold underline flourish; the "SALE" tag is ink text on a light mauve wash with a gold border.

### R5. Display face ≥24px; body stays modern
**Rule:** The ornamental display face appears only at ≥24px on headings, initials, and the wordmark; body, labels, forms, and buttons use the humanist sans or old-style serif at 16px.
**Why:** Ornament faces trade glyph clarity for flourish — below 24px they disintegrate, and inside forms they break usability; the legible core keeps the romance shoppable.
**Example:** "Maison Fleuri" in a decorated face at 40px; "Free delivery over €50" in 16px humanist sans.

### R6. Botanical line-work from one fixed set
**Rule:** Draw or commission one SVG set of botanicals (vine, stem, leaf, bloom) at a single stroke weight (1.5px) and reuse it everywhere; no mixing of sourced clip-art styles.
**Why:** The whiplash line has a specific hand — two illustration styles read as a craft fair, one reads as a house style; a fixed set also keeps ornament auditable.
**Example:** Corner vines, divider sprigs, and the logo flourish all share the same 1.5px stroke and curve-radius vocabulary.

### R7. Imagery is arch-framed and palette-bound
**Rule:** Photography sits in arch containers with gold line borders, graded warm and slightly desaturated toward the palette; no cold full-color full-bleed imagery.
**Why:** Framing plus grading folds modern photos into the illuminated-manuscript world; raw imagery pokes a hole in the period atmosphere.
**Example:** Product photo in an arch with a 1.5px gold border and warm grade; the hero is line-work illustration, not a photo.

### R8. The working core stays undecorated
**Rule:** Forms, filters, checkout, tables, and error states use plain modern components (2px radius, ink text, gold focus outline); ornament never touches them.
**Why:** The style promises a legible modern core inside the ornament — decoration in task flows converts elegance into friction, the classic Art Nouveau theme-park failure.
**Example:** Address form: white field on cream, ink labels, 2px gold focus ring; no vines, no arch inputs.

## A11y watchpoints
- Gold `#B08D3E` on cream ≈2.8:1 — below even the 3:1 UI floor: reserve it for non-essential ornament; interactive boundaries and focus rings use deepened gold `#8C6D2F` (≈4.3:1) or ink
- Mauve/sage accents fail as text or small fills on cream (~3.3:1) — use as strokes or washes; tinted panels take ink text on light washes only
- Ornamental display faces: enforce the 24px floor, never for body or labels; verify accented glyphs exist before committing to the face
- Measure contrast against ornament-adjacent zones — vines or grain near text can locally cut contrast; keep text zones on solid panels
- All decorative SVGs get `aria-hidden="true"` and `focusable="false"`; ornament must never be focusable or announced
- Arch containers: keep interactive content out of the clipped top curve; targets ≥24×24px and never cut by the arch geometry

## Checklist
- [ ] Zero ornament behind text; frames only
- [ ] One arch geometry system reused; no shape soup
- [ ] One focal ornament per section
- [ ] Gold on lines and rules only; text in ink
- [ ] Display face ≥24px; body 16px modern and legible
- [ ] One botanical SVG set, one stroke weight
- [ ] Imagery arch-framed and palette-graded
- [ ] Forms, checkout, tables plain and undecorated
- [ ] Decorative SVGs aria-hidden
- [ ] All text pairs ≥4.5:1 on actual backgrounds

## Anti-patterns
- Vine wallpaper running behind paragraphs
- Gold text for prices "because luxury"
- Ornamental face at 16px on buttons and labels
- Five illustration styles mixed from five stock sets
- Arched form inputs that clip typed text
- Ornament inside forms, tables, and error states
- Dark moody backgrounds — the style is cream and daylight
- A symmetric grid of identical arches with no focal point

## Sources & inspiration
- Art Nouveau — the movement: https://en.wikipedia.org/wiki/Art_Nouveau
- Jugendstil — the German branch: https://en.wikipedia.org/wiki/Jugendstil
- Alphonse Mucha — the poster canon: https://en.wikipedia.org/wiki/Alphonse_Mucha
- Whiplash (decorative art) — the signature line: https://en.wikipedia.org/wiki/Whiplash_(decorative_art)
- Victor Horta — architectural whiplash: https://en.wikipedia.org/wiki/Victor_Horta
