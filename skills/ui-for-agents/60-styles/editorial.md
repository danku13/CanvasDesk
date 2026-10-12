# Style: Editorial

> **Read when:** building UI in the editorial style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/typography.md, 00/spacing-layout-grids.md, 00/color.md

**Personality:** literate, confident, cultured, timeless. **Best for:** media, magazines, long-form content, fashion, portfolios, agencies. **Avoid for:** dense data tools, real-time dashboards, enterprise admin.

**Core idea:** the content is the interface — typography, whitespace, and captioned imagery do the work that chrome and decoration do elsewhere.

## Signature (what makes it recognizable)
- High-contrast serif display headlines paired with a clean, quiet sans body
- Strong grid used asymmetrically on purpose: wide margins, offset columns, hanging elements
- Hairline rules separating sections; boxes and cards almost never used
- Generous leading — body line-height 1.6–1.8 — at a 45–75ch measure
- Large pull-quotes and numbered sections as rhythm devices
- Black-and-white or duotone photography, always captioned, with deliberate aspect ratios
- Minimal chrome: navigation persistent but visually subordinate to content

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | sharp to small | 0–4 px |
| `shadow-*` | none | flat; overlays use hairline border + stepped surface tone |
| `space-scale` | generous, print-like | 8pt scale; wide outer margins (≥ 64 px-equivalent desktop) |
| `color-bg / surface` | paper-white / ink | #FDFCF8 / #141414 range |
| `color-accent` | one warm hue, rare | links and highlights only; 4.5:1 on paper |
| `type-family` | serif display + sans body | high-contrast serif for headlines; workhorse sans for UI text |
| `type-scale-ratio` | large | ~1.333; display sizes ≥ 32 px-equivalent |
| `leading / measure` | airy | line-height 1.6–1.8; text blocks 45–75 ch |
| `motion-duration` | restrained | 150–250 ms, ease-out; no decorative loops |
| `imagery` | content, not decoration | B&W or duotone, captioned, fixed aspect ratios |
| `border-style` | hairlines | 1px low-contrast rules; never boxed cards |

## Rules

### R1. Typography is the interface
**Rule:** Invest the identity budget in the type system — serif display scale, body sans ramp, caption style, pull-quote treatment — defined once and reused everywhere.
**Why:** With chrome removed, type is the only remaining carrier of hierarchy and personality; an under-specified type system makes every page look like a default document.
**Example:** Article page: 40px-equivalent serif headline, 16px sans body at 1.7, 14px caption with hairline above — the same trio on every story.

### R2. Reading experience first
**Rule:** Text blocks sit at 45–75ch with 1.6–1.8 line-height; never interrupt a paragraph mid-flow with popovers, sticky bars, or injected modules.
**Why:** Editorial credibility is a promise of comfortable long-form reading; interruptions and cramped measures break that promise faster than any styling choice can repair it.
**Example:** A 2,000-word feature keeps a single column, at most one mid-content break, and related links after the article ends.

### R3. Images are content, not decoration
**Rule:** Every image is captioned, has a deliberate aspect ratio, and earns its place in the narrative; no orphan stock photos, no imagery under text.
**Why:** In this style an image is a statement; uncaptioned decoration reads as filler and dilutes the literate tone.
**Example:** Photo essay: full-bleed duotone images alternating with offset text columns, each image carrying caption plus credit line.

### R4. Asymmetry on purpose
**Rule:** Layouts may break the symmetric grid for editorial effect — offset images, hanging headlines, wide margins — but every element snaps to declared alignment axes.
**Why:** Asymmetry is expressive only when the underlying grid is still felt; random offsets read as broken, not artful.
**Example:** Hero with the headline starting at column 2 of 6 and the deck hanging at column 4 — both edges traceable to the same 12-column grid.

### R5. Hairline rules over boxes
**Rule:** Separate and group content with 1px horizontal rules, generous space, and typographic weight; reserve background fills and borders for true overlays.
**Why:** Boxes fragment the page into tiles and kill the continuous flow of a print spread; hairlines structure without interrupting.
**Example:** Related-articles block introduced by a hairline plus small-caps label — not a bordered card.

### R6. Whitespace like print margins
**Rule:** Treat outer margins as fixed print margins that scroll UI never encroaches on, and keep section spacing at least 2× component spacing.
**Why:** The margin is what makes content feel published rather than poured; cramped edges are this style's most common failure.
**Example:** Long-form reader holds a wide outer margin even on large screens; content max-width enforced above all breakpoints.

### R7. Navigation quiet
**Rule:** Navigation persists but stays visually subordinate — small sans, ink-gray, hairline separators; content always outranks chrome in size and contrast.
**Why:** The reader came for the text; loud navigation competes with it and cheapens the tone.
**Example:** Slim top bar with wordmark plus four links, collapsing to a single "Menu" item while reading.

### R8. One warm accent, spent on reading paths
**Rule:** The accent hue appears only on links, active states, and rare highlights; all other hierarchy comes from type scale and ink levels.
**Why:** Editorial color works by scarcity — one recurring warm tone becomes a wayfinding signature, while several hues read as a marketing page.
**Example:** Links in warm oxblood, always underlined; every other emphasis is italic or heavier weight.

### R9. Rhythm devices used consistently
**Rule:** Pull-quotes, numbered sections, and drop caps (if used) follow one written spec — size, spacing, and frequency limits, e.g. one pull-quote per ~800 words.
**Why:** These are the style's signature flourishes; improvised use turns them into clutter and breaks the published feel.
**Example:** Feature template: numbered section headings, optional drop cap on the opening paragraph only, one pull-quote budget per section.

## A11y watchpoints
- Display serif loses legibility below headline sizes — restrict serif to display use (≥ 24 px-equivalent); body and UI text stay sans
- Light-gray "sophisticated" text on paper-white fails: keep all ink at 4.5:1, including muted caption gray
- Captions and footnotes must not shrink below readable minimums (≥ 14 px-equivalent) and still need 4.5:1
- Links must survive color removal: underline them — never rely on the accent hue alone (WCAG 1.4.1)
- Oversized display type must reflow at 200% zoom without clipping (WCAG 1.4.4); quiet nav controls keep 24×24 px targets and visible focus

## Checklist
- [ ] Serif reserved for display sizes; sans carries body and UI text
- [ ] Body text 45–75 ch at 1.6–1.8 line-height
- [ ] Every image captioned, deliberate aspect ratio
- [ ] Sections separated by hairlines and whitespace — no boxed cards
- [ ] Asymmetric layouts traceable to declared grid axes
- [ ] Navigation persistent, quiet, keyboard-reachable with visible focus
- [ ] Accent on links and highlights only; links underlined
- [ ] All motion 150–250 ms ease-out; no decorative animation
- [ ] Ink pairs ≥ 4.5:1, both themes, captions included

## Anti-patterns
- "Editorial" = centered text in a random Google serif with no grid underneath
- Grayscale photos as dark text backgrounds with contrast below the floors
- Justified text everywhere with rivers of hyphenation chaos
- Drop caps on every paragraph; three pull-quotes per screen
- Serif body text at small sizes "for charm"
- Hairlines so faint they vanish — or hairlines used as control borders
- Chrome so minimal that focus states, breadcrumbs, and navigation disappear
- Fake newsprint textures and skeuomorphic paper backgrounds

## Sources & inspiration
- Butterick's Practical Typography — leading, measure, hierarchy: https://practicaltypography.com
- Typewolf — serif/sans pairing references: https://www.typewolf.com
- Fonts In Use — editorial type and layout archive: https://fontsinuse.com
- NN/g — legibility, readability, comprehension: https://www.nngroup.com/articles/legibility-readability-comprehension/
- WCAG 2.2 — contrast and reflow floors: https://www.w3.org/TR/WCAG22/
