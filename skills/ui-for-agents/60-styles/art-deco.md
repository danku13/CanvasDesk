# Style: Art Deco

> **Read when:** building UI in the art-deco style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/motion-principles.md

**Personality:** luxurious, symmetrical, theatrical, confident. **Best for:** luxury hotels, restaurants, fashion, cosmetics, cinema/premiere, premium drinks. **Avoid for:** kids products, casual community, utilitarian tools.

**Core idea:** Gatsby glamour — symmetric ceremonial composition, geometric ornament (sunbursts, fans, chevrons, stepped ziggurats), gold on black with one jewel tone, and tall condensed display type; the interface is a grand hotel lobby.

**Boundary:** not dark-premium (that style is minimalist tech-luxury with almost no ornament; deco is ceremonial, ornamental, and strictly symmetrical).

## Signature (what makes it recognizable)
- Symmetry everywhere — the centered vertical axis is law for hero, headers, and CTAs
- Gold `#D4AF37` + near-black `#0E0E10` + one jewel tone (emerald `#0B3D2E`, burgundy `#4A1C24`, or navy `#14203E`)
- Motifs from a fixed set: sunburst, fan, chevron, stepped ziggurat — drawn as flat geometry
- Tall condensed sans or high-contrast serif display, uppercase, wide letter-spacing (0.08–0.14em)
- Stepped (ziggurat) corner clips instead of rounded corners
- Marquee frames: double-rule borders (1px + 3px gold) around featured blocks
- Gold-on-dark or ivory-on-jewel schemes; imagery duotoned gold/black or black/ivory
- Motion slow and confident: 300–400 ms fades, nothing springy

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | zero — steps replace radii | clip-path ziggurat corners, 2–3 steps, consistent depth |
| `shadow-*` | minimal | none at rest; one soft black lift for overlays only |
| `space-scale` | ceremonial | 8pt; hero padding 2× standard; symmetric center axis |
| `color-bg` | near-black | `#0E0E10` |
| `color-surface` | charcoal step | `#1A1A1E` |
| `color-ink` | ivory text | `#F2EDE0` |
| `color-accent` | gold — lines, frames, large display | `#D4AF37` |
| `color-jewel` | one jewel field | emerald `#0B3D2E` (or burgundy/navy) |
| `type-display` | tall condensed / high-contrast serif | uppercase, tracking 0.08–0.14em, ≥24px |
| `type-body` | clean sans | 16px / 1.5, ivory on dark |
| `motion-duration` | slow confident fade | 300–400 ms, ease-out |
| `border-marquee` | double rule | 1px + 3px gold, 4px apart |
| `ornament-set` | fixed motifs | pick 3–4: sunburst, fan, chevron, step |

## Rules

### R1. Symmetry is law
**Rule:** Compose hero, section headers, CTAs, and footer on a centered vertical axis with mirrored ornament; asymmetry is permitted only inside long content columns (menus, articles).
**Why:** Symmetry is the ceremony — it signals occasion and control; a lopsided deco page reads as a theme party, and the mirrored ornament is what separates deco from every other gold-on-black style.
**Example:** Booking hero: centered wordmark, sunburst behind, symmetric double-rule frame; the room-list grid below stays symmetric too.

### R2. Gold is a line, body text is ivory
**Rule:** Render gold as borders, rules, motifs, and large display only; paragraphs, labels, and button text use ivory `#F2EDE0` on dark or ink on ivory panels.
**Why:** Gold text at small sizes turns to glitter dust and its contrast dies on lighter panels; the gold-line/ivory-text split keeps the luxury legible.
**Example:** "GRAND BALLROOM" at 48px gold and tracked; the description under it 16px ivory; the button is a gold-framed cell with ivory text.

### R3. Ornament comes from a fixed motif set
**Rule:** Pick 3–4 motifs (sunburst, fan, chevron, step) at project start and reuse them everywhere at consistent stroke weights; no laurels, ribbons, or flourishes outside the set.
**Why:** Deco ornament is geometry, not filigree — a closed set keeps every page recognizably the same hotel; motif soup reads as casino.
**Example:** Dividers are chevron rows, section markers are fans, the loader is a rotating sunburst — three motifs, no fourth.

### R4. Stepped corners replace radii
**Rule:** Terminate feature blocks and frames in 2–3-step ziggurat clips (clip-path geometry, consistent step depth); border-radius stays 0.
**Why:** The setback silhouette is deco's architectural signature — the Chrysler Building in miniature; rounded corners import the wrong decade instantly.
**Example:** The booking card's corners step twice at 8px depth; inner badges stay sharp rectangles.

### R5. Display uppercase and tracked; body plain
**Rule:** Set display type uppercase with 0.08–0.14em tracking at ≥24px; body is a clean sans at 16px/1.5 with normal tracking; condensed faces never run in paragraphs.
**Why:** Tracked uppercase is the marquee voice — at paragraph scale it becomes unreadable; the plain body is what makes the theater comfortable.
**Example:** Menu headers in tracked condensed caps; dish descriptions in plain 16px sans at 1.5 line-height.

### R6. Imagery is duotone, framed by rules
**Rule:** Duotone photos to gold/black or black/ivory and frame them in marquee double rules; no full-color photography anywhere.
**Why:** Color photos punch through the gilded palette and break the period illusion; duotone makes even modern shoots look like 1930s premieres.
**Example:** Suite photo as black/ivory duotone in a gold double-rule frame with stepped corners.

### R7. Motion is a slow confident fade
**Rule:** Run all motion 300–400 ms ease-out — fades, slight rises, one-time ornament draws; no springs, bounces, or slides beyond 400ms, and nothing loops.
**Why:** Deco moves like staff in a grand lobby: unhurried, rehearsed; springy motion is casual-consumer body language and kills the ceremony.
**Example:** Section reveal: 350ms fade + 8px rise; the marquee frame's gold rules draw once and stop.

### R8. One jewel field per view
**Rule:** The jewel tone appears as at most one large field per view (hero panel, feature band); everywhere else is black/charcoal/ivory with gold lines.
**Why:** Jewel tones are the velvet rooms of the palette — two competing jewel fields turn the lobby into a cabaret and dilute the gold.
**Example:** Emerald hero band with ivory text and gold frame; the rest of the page stays charcoal with gold rules.

## A11y watchpoints
- Gold `#D4AF37` on near-black computes ≈9:1 — fine even small; but re-verify gold on charcoal and jewel steps, and never gold text on ivory panels (≈1.8:1)
- Ornament display faces and heavy tracking get a ≥24px floor; never for body, labels, or form text
- 300–400ms motion must respect prefers-reduced-motion (jump to final state); ornament draws happen once, never loop
- Symmetric mirrored layouts: keep DOM order equal to visual reading order (left column before right) so screen readers do not zigzag
- Decorative motifs are `aria-hidden`; the double-rule frames are decoration — interactive boundaries still need their own ≥3:1 state indication
- Ivory on jewel fields verified per tone (ivory/emerald ≈10:1 passes); re-check darker burgundy/navy mixes before text lands on them

## Checklist
- [ ] Hero, headers, CTAs on one centered axis with mirrored ornament
- [ ] Gold on lines, frames, large display only; body text ivory/ink
- [ ] 3–4 motifs fixed and reused; no outside flourishes
- [ ] Stepped corners 2–3 steps; radius 0
- [ ] Display uppercase tracked ≥24px; body plain 16px
- [ ] Imagery duotone + marquee frames; zero full-color photos
- [ ] Motion 300–400 ms fades; reduced-motion honored
- [ ] One jewel field per view
- [ ] Gold text pairs verified on every surface step
- [ ] Motifs aria-hidden; focus states ≥3:1 independent of ornament

## Anti-patterns
- Asymmetric or left-aligned "modernized" deco heroes
- Gold body text and gold small labels
- Rounded corners with gold gradients (casino shortcut language)
- Seven ornament motifs mixed on one page
- Full-color photography over the gilded palette
- Springy playful micro-interactions
- Two or three jewel tones competing per view
- Looping shimmer/sparkle animation on gold elements

## Sources & inspiration
- Art Deco — the movement: https://en.wikipedia.org/wiki/Art_Deco
- Chrysler Building — the stepped-silhouette icon: https://en.wikipedia.org/wiki/Chrysler_Building
- Tate — Art Deco art term: https://www.tate.org.uk/art/art-terms/a/art-deco
- Streamline Moderne — deco's late phase: https://en.wikipedia.org/wiki/Streamline_Moderne
- Limelight — open deco display face: https://fonts.google.com/specimen/Limelight
