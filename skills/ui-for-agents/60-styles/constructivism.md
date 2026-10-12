# Style: Constructivism

> **Read when:** building UI in the constructivism style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/motion-principles.md

**Personality:** revolutionary, urgent, bold, agitprop. **Best for:** events/festivals, cultural campaigns, music, sports, edgy brand landings, poster-like heroes. **Avoid for:** healthcare, banking, long-form reading.

**Core idea:** the propaganda poster as interface — red and black diagonal wedges, photomontage cut-outs, condensed uppercase type at dynamic angles; energy comes from diagonals, mass, and contrast, never from decoration.

**Boundary:** not brutalism (brutalism = raw unstyled honesty on an intact grid; constructivism = curated poster art with a strict palette and composed diagonals) and not de-stijl (diagonals vs orthogonal).

## Signature (what makes it recognizable)
- Diagonal compositions: 30–45° wedges, beams, and bars cutting the layout
- Palette: poster red `#D42B1E` + ink black `#16130F` + cream paper `#F2E8D5` + one deep neutral `#333944`
- Condensed sans/slab display in uppercase, stepped (each line a staircase step) or rotated
- Photomontage: duotone red/black cut-out figures with hard-edged crops
- Pure geometry where there is no photo: wedges, beams, circles (Lissitzky's red wedge)
- No ornament, no gradients, no rounded corners — heavy flat blocks on paper
- Type is mass: huge condensed headlines carry the composition; body text stays quiet

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | zero | 0 everywhere |
| `shadow-*` | none | flat blocks; depth by overlap only |
| `wedge-angle` | diagonal energy | 30–45°; 2–3 wedges per view |
| `space-scale` | poster-tight display, reading-safe body | 8pt; body measure 45–75ch |
| `color-bg` | cream paper, never pure white | `#F2E8D5` (surfaces step to `#E9DEC8`) |
| `color-ink` | ink black | `#16130F` |
| `color-accent` | poster red — action AND danger | `#D42B1E` |
| `color-block` | one deep neutral for secondary masses | `#333944` |
| `type-display` | condensed sans/slab, uppercase | Oswald/Anton-class 700–900, ≥32px |
| `type-body` | plain legible sans | 16px / 1.5, horizontal only |
| `motion-duration` | sharp snap | 0–150 ms, ease-out or instant |
| `texture` | paper grain | ≤5% opacity, behind content only |
| `imagery` | duotone cut-out | red/black photomontage or pure geometry |

## Rules

### R1. Diagonal energy budget
**Rule:** Allow 2–3 diagonal elements per view (wedges, tilted headline, angled photo crop); body text, forms, and navigation stay strictly horizontal.
**Why:** Diagonals are the style's engine — but a screen where everything slants has no stable reading path and exhausts the eye; the budget keeps poster energy in the hero, not the checkout.
**Example:** Event page: one red 40° wedge behind the hero title and one angled photo beam; the schedule below sits on an untouched horizontal grid.

### R2. Red means action AND danger — disambiguate by shape and label
**Rule:** Because the palette merges accent and alert into one red, every red control or warning carries an explicit shape and label (button text, triangle icon, "SOLD OUT" tag); hue is never the signal.
**Why:** With red-only semantics, users cannot tell "buy ticket" from "error" — and color-blind users never could; shape and text carry the meaning.
**Example:** Red CTA reads "BUY TICKETS" as a rectangular cell; the red alert shows a black triangle icon plus message; they never share an unlabeled red block.

### R3. Uppercase display, staircase composition
**Rule:** Set headlines in condensed uppercase broken into 2–4 stepped lines (each line indented one step) or rotated at most ~15°; paragraphs never take display treatment.
**Why:** The staircase line break is the movement's signature cadence — it gives static text the diagonal momentum the posters drew by hand.
**Example:** Hero: "JAZZ / MARATHON / 48 HOURS" stepping right at 64px condensed; the description below in 16px sentence case.

### R4. Imagery is photomontage or geometry — no stock photos
**Rule:** Photos are duotoned to red/black (or black/cream), hard-cropped, optionally halftoned; anything that cannot be processed into the palette is replaced by pure geometric shapes.
**Why:** A full-color modern stock photo breaks the 1920s print illusion instantly; processed imagery makes even new photography read as a period artifact.
**Example:** Performer portraits as red/black duotone cut-outs with one wedge behind; empty states built from a wedge + circle composition.

### R5. Cream paper, never pure white
**Rule:** Backgrounds use cream `#F2E8D5` (or darker paper steps); pure `#FFFFFF` is banned as a surface.
**Why:** The style lives on newsprint and poster paper — pure white reads as clinical SaaS and kills the ink-on-paper contrast the palette depends on.
**Example:** Cards are `#E9DEC8` on the `#F2E8D5` ground with black rules; the darkest section inverts to ink black with cream text.

### R6. Motion snaps: 0–150 ms
**Rule:** Resolve interactions in 0–150 ms with sharp ease-out or instantly — color flips, wedge reveals, hard cuts; no floats, no fades over 150 ms, no springs.
**Why:** The poster is a stamp, not a cloud; long soft transitions contradict the urgency that is the style's entire message.
**Example:** CTA hover: background flips cream→red instantly, text inverts in 100ms; the wedge reveal is a 120ms clip-path cut.

### R7. Hierarchy by mass, not by weight-range
**Rule:** Build hierarchy from extreme size contrast — display at 3–5× body size, one dominant red or black mass per view — not from many intermediate weights and grays.
**Why:** The poster vocabulary has two voices, shout and plain; a gradient of middle sizes reads as corporate layout wearing a costume.
**Example:** Lineup page: 72px condensed headliners, 16px plain support text — nothing styled at 24px "medium emphasis".

### R8. DOM order = reading order, even when the layout slants
**Rule:** Keep visually rotated or stepped elements in logical sequence in the DOM; never let the diagonal composition reorder content for assistive tech.
**Why:** The diagonal look comes from transforms and positioning — if the DOM zigzags too, screen readers read poster chaos instead of the message.
**Example:** The stepped hero reads "JAZZ → MARATHON → 48 HOURS" in source order; the tilt is a transform, not a markup order.

## A11y watchpoints
- Rotated text is banned below 24px — readability degrades fast off-axis, and WCAG 1.4.8's visual-presentation requirements assume upright text; rotation stays a display-size device
- Red `#D42B1E` on ink black computes ≈3.6:1 — red-on-black is for large display or 3:1 fills only; body text on dark sections is cream (~14.9:1)
- Red on cream is ≈4.2:1 — red fails 4.5:1 for body text; red is for fills and large display, ink carries the reading
- Motion-trigger check: 0–150ms snaps and wedge reveals must respect prefers-reduced-motion (render the final state); no flashing sequences (2.3.1)
- Diagonal layouts must not break DOM reading order (R8); overlapped wedges must not cover controls or steal pointer events — keep targets ≥24×24px clear

## Checklist
- [ ] ≤3 diagonal elements per view; body and forms horizontal
- [ ] Every red usage labeled or shaped — action vs danger distinguishable
- [ ] Display uppercase, stepped or rotated, ≥24px only
- [ ] Imagery duotone or geometric; zero unprocessed stock photos
- [ ] Cream paper ground; no pure white surfaces
- [ ] Motion 0–150 ms, reduced-motion safe, no flashing
- [ ] Size contrast extreme (3–5×); no mid-gray hierarchy
- [ ] DOM order matches reading order under rotation
- [ ] All text pairs ≥4.5:1 against actual paper/ink
- [ ] Targets ≥24×24px even inside the poster composition

## Anti-patterns
- Everything tilted 10–45° "for energy"
- Red alert blocks indistinguishable from red CTAs
- Long paragraphs in condensed uppercase
- Full-color stock photos dropped onto cream paper
- Pure white backgrounds and soft shadows
- Gentle 400ms fades on a propaganda poster
- Pastel accents "to soften it"
- Diagonal DOM order breaking screen-reader flow

## Sources & inspiration
- Constructivism — the movement: https://en.wikipedia.org/wiki/Constructivism_(art)
- Suprematism — Lissitzky's geometric abstraction: https://en.wikipedia.org/wiki/Suprematism
- Alexander Rodchenko — graphic design and photomontage: https://en.wikipedia.org/wiki/Alexander_Rodchenko
- El Lissitzky — the red wedge compositions: https://en.wikipedia.org/wiki/El_Lissitzky
- Tate — Constructivism art term: https://www.tate.org.uk/art/art-terms/c/constructivism
- Oswald — open condensed sans: https://fonts.google.com/specimen/Oswald
