# Style: Dark Academia

> **Read when:** building UI in the dark-academia style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/typography.md, 00/color.md, 00/iconography.md, 00/motion-principles.md

**Personality:** scholarly, romantic, nostalgic, serious. **Best for:** education platforms, books and publishing, libraries, museums, course products, journaling apps. **Avoid for:** kids products, fintech, gaming, casual social apps.

**Core idea:** the old university rendered as an interface — parchment and oxblood, old-style serif everywhere, engraved imagery, candlelight warmth in dark mode — romantic scholarship delivered with mandatory modern reading comfort.

## Signature (what makes it recognizable)
- Parchment `#F3EDDF` (light) or library-dark `#1C1917` (dark) grounds — never pure white, never neutral-gray dark
- Palette of deep browns, oxblood `#6D2E2E`, forest green, with gold-leaf line accents
- Old-style serif (Garamond/Caslon class) for display AND body — the serif is the brand
- Engraved and woodcut illustration motifs; vintage map and star-chart ornaments
- Ruled-paper textures, margin notes, drop caps, and footnote styling as editorial furniture
- Wax seals, crests, emblems, and badges framing real content
- Dark mode tinted like a candlelit library: warm darks, amber-gold accents
- Imagery engraved/duotone or warm-toned photography — no clinical modern color

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | sharp, engraved-plate | 0–4px; arches only as framed imagery shapes |
| `shadow-*` | deep, warm, rare | `0 8px 24px rgba(42,28,14,0.18)` on overlays only |
| `space-scale` | comfortable, bookish | 8pt scale; margins like a book page (section gaps 64–96px) |
| `color-bg` | parchment / library dark | `#F3EDDF` light; `#1C1917` dark |
| `color-surface` | aged paper / warm dark | `#EFE6D2` light; `#26211C` dark |
| `color-ink` | sepia black | `#2A2118` (≈13:1 on parchment) |
| `color-accent` | oxblood | `#6D2E2E` (≈8.6:1 on parchment; brighten in dark mode) |
| `color-gold` | line accent only | antique gold `#A8842C` (≈3:1 on parchment — rules, never text) |
| `type-display` | old-style serif | Garamond/Caslon class, 600–700 weight for titles |
| `type-body` | same serif, size floor | ≥17px / 1.6; UI controls may switch to a clean sans |
| `motion-duration` | quiet fades | 200–300 ms ease-out; no bounce |
| `ornament-set` | one engraving set | fixed SVG set, single stroke weight |

## Rules

### R1. Serif body earns its floor
**Rule:** Serif body text is set at ≥17px with a verified x-height and line-height ≥1.6; buttons, forms, tables, and labels may switch to a clean sans or the serif's UI cut — legibility outranks romance.
**Why:** Old-style serifs at 14–16px on screens turn mushy — small x-heights and fine strokes collapse — and reading fatigue kills the scholarship fantasy faster than any aesthetic win preserves it.
**Example:** Article body: EB Garamond 17.5px/1.65. Search input and table headers: 14px sans. Both on the same parchment.

### R2. One engraving set, one stroke weight
**Rule:** All ornaments — crests, dividers, botanical cuts, star charts — come from a single fixed SVG set with a consistent stroke weight; never mix found-asset engravings of different densities.
**Why:** Engravings from different sources carry different line weights and hatching styles; mixed sets read as clip-art collage instead of one plate-maker's work.
**Example:** Dividers and chapter markers all cut from one 1.5px-stroke set; the heavier 3px crest appears once, on the certificate page.

### R3. Dark mode is a candlelit library, not a gray void
**Rule:** Dark surfaces are warm-tinted browns (`#1C1917`, `#26211C`), text is warm cream (`#EDE4D3`), and accents shift amber/gold — never reuse the light palette on a neutral-gray dark.
**Why:** Neutral darks make it generic dark mode; the warm tint is what sells candlelight — and warm darks shift perceived contrast, so every pair must be re-verified.
**Example:** Dark reader: parchment becomes `#1C1917`, ink becomes cream `#EDE4D3`, gold rules stay, oxblood brightens to `#C96B5B` (≈4.8:1).

### R4. Gold is a line, not a fill
**Rule:** Gold appears as hairline rules, drop-cap strokes, emblem linework, and focus borders — never as text fill or button background; if gold marks text, the text is large display and the specific gold passes 4.5:1.
**Why:** Antique gold mid-tones sit near 3:1 on parchment — acceptable linework, failing text; gold fills tempt low-contrast buttons that fail everything.
**Example:** Section rule and drop cap in `#A8842C` at 2px; the "Enroll" button is oxblood with parchment text, gold only as its 1px focus outline.

### R5. Imagery is engraved, duotone, or candlelit
**Rule:** Photos are warm-toned or processed into brown/oxblood duotones; illustrations are engraved/woodcut style; no clinical white-background stock, no saturated modern color photography.
**Why:** A raw modern photo breaks the period illusion more than any missing ornament; processed imagery unifies the light of the whole page.
**Example:** Faculty portraits in warm sepia duotone with 1px ink frames; a star-chart engraving as the astronomy course header.

### R6. Emblems frame real content
**Rule:** Wax seals, badges, and crests mark actual things — a course, a faculty, an achievement, a completed lesson — at most one emblem per card or section; never sprinkle emblems as decoration across every surface.
**Why:** Emblems are the style's status markers; when everything is sealed, nothing is — and screen-reader users get decorative noise instead of signal.
**Example:** Completed courses get a wax-seal badge with `aria-label="Completed"`; the syllabus list stays plain.

### R7. Period furniture stays out of the reading flow
**Rule:** Ruled-paper backgrounds, margin notes, and footnote styling frame and annotate real content (asides, definitions, citations); body measure stays 45–70ch on plain parchment with zero texture behind text.
**Why:** The furniture is charming until it sits under paragraphs; texture behind text makes contrast unauditable and ruled lines fight line spacing.
**Example:** A definition sits in the margin in italic serif; the main paragraph column is bare `#F3EDDF`.

### R8. Dark academia is not editorial
**Rule:** Editorial is the modern magazine — serif display + sans body, asymmetric grid, contemporary photography; dark academia is full period romance — serif body, centered and traditional layouts allowed, engraved imagery. If the body is sans and the grid modern-asymmetric, you built editorial with brown paint.
**Why:** The two share type DNA but promise different worlds; hybrid drift produces neither editorial's crispness nor the academy's immersion.
**Example:** Editorial: 12-col asymmetric grid, Didone display, sans body. Dark academia: centered chapter opener, Garamond body, engraved plate.

## A11y watchpoints
- Serif legibility at small sizes is the core risk: enforce the ≥17px body floor, check x-height, and swap UI controls to sans where the serif fails
- Sepia pairs fail quietly: verify every ink/parchment pair at 4.5:1 — faded-ink grays and mid-browns are the usual offenders
- Ornamental display faces get a size floor (≥24–28px); decorative serifs collapse fast below it
- Warm darks shift perceived contrast: re-verify all pairs in the dark theme, not just re-map tokens
- Gold and oxblood are mid-tones: never signal state by color alone — pair with labels or icons
- Engraving-heavy pages: keep decorative SVGs `aria-hidden` and out of the reading order

## Checklist
- [ ] Body serif ≥17px, line-height ≥1.6, x-height verified (or UI sans swapped in)
- [ ] All ornaments from one set, one stroke weight
- [ ] Dark mode warm-tinted with amber/gold accents; never neutral gray
- [ ] Gold used as linework only; any gold text passes 4.5:1
- [ ] Imagery engraved/duotone or warm-toned; zero clinical stock
- [ ] Emblems mark real content, ≤1 per card/section, labeled
- [ ] Body measure 45–70ch on texture-free parchment
- [ ] Every text pair ≥4.5:1 in BOTH themes (re-verified, not assumed)
- [ ] Motion 200–300 ms fades; reduced-motion honored

## Anti-patterns
- 14px Garamond body text "because that's the point"
- Neutral-gray dark mode with the light palette reused
- Gold button fills and gold headline text at failing contrast
- A different engraving style per section — clip-art collage
- Wax seals and crests sprinkled on every card as decoration
- Ruled-paper texture running under body paragraphs
- Clinical white-background stock photography
- Gothic blackletter for body text — display-only, if at all

## Sources & inspiration
- Dark academia — the aesthetic movement: https://en.wikipedia.org/wiki/Dark_academia
- EB Garamond — the canonical open old-style serif: https://fonts.google.com/specimen/EB+Garamond
- Collegiate Gothic — the architecture of the fantasy: https://en.wikipedia.org/wiki/Collegiate_Gothic
- Illuminated manuscript — ornament and initial logic: https://en.wikipedia.org/wiki/Illuminated_manuscript
- Garamond — the historical face: https://en.wikipedia.org/wiki/Garamond
- Fonts In Use — serif body setting in the wild: https://fontsinuse.com
