# Style: Maximalism

> **Read when:** building UI in the maximalism style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/visual-hierarchy.md, 00/typography.md, 00/spacing-layout-grids.md

**Personality:** exuberant, abundant, eclectic, confident. **Best for:** fashion, media and entertainment, creative portfolios, event campaigns, Gen-Z brands, music artists. **Avoid for:** enterprise, banking, tools, long-form reading surfaces.

**Core idea:** more is more — but curated: layered patterns, a 6–8 color role-mapped palette, up to three display typefaces, and dense collage composition; the craft is curation, and the primary action must survive the abundance.

## Signature (what makes it recognizable)
- 2–3 layered patterns per view, always behind content
- Rich saturated palette of 6–8 colors mapped to fixed roles (never chosen ad hoc per screen)
- Up to 3 display typefaces mixed deliberately — serif, grotesque, script — each with a job
- Collage and photo clusters: overlapping, rotated, framed, taped-on
- Ornamental borders, frames, and arch shapes around featured content
- Dense heroes and footers; mid-page breathing room as a pressure valve
- Marquee text strips and oversized repeated headlines
- Sticker and badge accumulations marking drops, tags, and states

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | expressive, fixed by role | 0px frames + 16–24px pills coexist; each role keeps its radius |
| `shadow-*` | hard offsets or thick outlines | `4px 4px 0 ink` or 3px solid border; no soft blur |
| `space-scale` | standard base, compressed hero | 8pt scale; hero may halve, forms keep full spacing |
| `color-bg` | saturated or rich cream | e.g. deep violet `#1F0E3C` or `#FFF1E6` — chosen once |
| `color-surface` | always solid content panels | opaque, never translucent over patterns |
| `color-ink` | high-contrast neutral | `#111111` on light, `#FFF8F0` on dark |
| `color-accent-1..3` | 3 of the 6–8 roles | e.g. magenta `#FF3EA5`, cobalt `#2B4FF2`, gold `#FFC940` |
| `pattern-set` | fixed once, reused everywhere | 2–3 pattern assets, versioned |
| `type-display` | up to 3 faces, roles fixed | face A headlines, B stickers, C numerals |
| `type-body` | plain readable sans | 16px / 1.5 — never a display face |
| `motion-duration` | functional, fast | 150–250 ms; marquee loop is a separate token |
| `sticker-budget` | hard cap per view | ≤6 badges/stickers, counted in review |

## Rules

### R1. The curation contract
**Rule:** Define the system once — the 6–8 color palette with roles, the ≤3 display faces with jobs, the 2–3 pattern files, the shadow style — and never add an element outside it; chaos lives only inside the contract.
**Why:** Maximalism fails not from abundance but from inconsistency; a fixed system is what separates curated-maximal from every designer's favorite things stacked on one page.
**Example:** The palette doc lists 7 swatches with roles; six months later checkout uses the same magenta, the same script face, the same checker pattern file.

### R2. One focal point per view
**Rule:** Every view lands the eye somewhere first — through size (a 3–5× display moment) or isolation (a calm zone in dense surroundings) — and the surrounding density is arranged to point at it.
**Why:** Without a designated landing point the eye wanders and users abandon; density without hierarchy is noise, and noise has no conversion path.
**Example:** A tour poster hero: the artist name at 140px dominates; badges, patterns, and collage orbit it, never larger.

### R3. The primary CTA survives the noise
**Rule:** The primary action gets an isolation zone (clear margin no pattern may enter), a fill no other element shares, or both — then verify with the grayscale test: desaturate the view and find the CTA in under 3 seconds.
**Why:** In an 8-color, 3-typeface environment "looks different" is not enough; the CTA competes with everything, so its dominance must be structural and survive color-blindness and desaturation.
**Example:** "Buy tickets" sits in a solid cream margin gap with the only gold fill on the page; in grayscale it is still the brightest, most isolated rectangle.

### R4. Density gradient: maximal at the edges, calm where work happens
**Rule:** Heroes, footers, and section dividers may go fully maximal; forms, checkout, settings, and multi-step flows calm down progressively — solid panels, fewer patterns, standard spacing — dropping a notch at every step closer to payment.
**Why:** Abundance is for looking, not for typing a card number; users in task mode need the noise dialed out, and a gradient keeps the brand while restoring cognition.
**Example:** Landing page: full collage hero. Checkout: same palette and one pattern strip in the header; every field sits on plain solid panels.

### R5. Patterns behind, panels solid
**Rule:** Pattern layers run full-bleed behind content; every text and control zone gets an opaque solid panel between it and the pattern — no text directly on a pattern, ever.
**Why:** Contrast over a pattern is unpredictable at every point (it must be measured against the worst pixel, not the average); solid panels make every pair auditable.
**Example:** A checkerboard runs the page; the newsletter form sits on an opaque `#111111` panel with cream text on top.

### R6. Marquees are decoration with an off switch
**Rule:** Marquee strips, repeated-type loops, and parallax layers are decorative only (`aria-hidden`, no essential content), pause on hover/focus, and are fully stopped under `prefers-reduced-motion`.
**Why:** Moving text is unread noise for screen readers and a trap for low-vision and vestibular users; essential content in a marquee is content users can lose.
**Example:** A marquee repeats the tour name between sections; dates and the buy link live in static text beside it.

### R7. Sticker budget
**Rule:** Define a per-view badge/sticker cap (e.g. ≤6), enforce it in review, and make every sticker mark real content — a drop, a tag, a state, a price — never pure decoration.
**Why:** Sticker accumulation is the fastest maximalism failure: at 15 badges the eye trusts none of them, and decorative stickers dilute the ones carrying information.
**Example:** Product card: "NEW" tag, price burst, one collab logo — three stickers, counted, each earning its place.

### R8. Maximalism is not Memphis
**Rule:** Memphis is a specific 1980s movement with a locked vocabulary (squiggles, terrazzo, pastel palettes); maximalism is the umbrella abundance aesthetic with no era-locked shapes — pull patterns and type from your own brand world, not from 1981 stock squiggles.
**Why:** Reaching for the Memphis starter pack collapses a brand style into an 80s costume and dates the product instantly.
**Example:** A Gen-Z fashion drop uses barcode tape, chrome blobs, and blackletter — zero squiggles — and is still fully maximalist.

## A11y watchpoints
- The hardest style for contrast: verify every text pair at 4.5:1 against the solid panel it actually sits on — never against the pattern, never "on average"
- Never signal by color alone in an 8-color system: pair every state and accent with shape, label, or icon (WCAG 1.4.1)
- Marquee, parallax, and auto-playing collage motion honor `prefers-reduced-motion` fully — pause-on-hover alone is not enough
- CTA findability must survive grayscale and CVD simulation — test both before shipping
- Dense layouts squeeze targets: keep ≥24×24px hit areas; stacked collage layers must not overlap interactive targets
- Saturated fields fatigue long sessions: keep reading surfaces (articles, checkout) on solid low-saturation panels

## Checklist
- [ ] Palette, type set, pattern set, shadow style defined once and reused everywhere
- [ ] Every view has one dominant focal point
- [ ] CTA found in <3s in grayscale; has isolation zone and/or unique fill
- [ ] Density gradient holds: hero maximal → forms/checkout calm, stepwise
- [ ] No text directly on patterns; every text zone on an opaque solid panel
- [ ] All text pairs ≥4.5:1 on their actual panels
- [ ] Marquees decorative, `aria-hidden`, pausable, dead under reduced-motion
- [ ] Sticker count ≤ defined budget; every sticker marks real content
- [ ] States and accents never color-only (shape/label paired)
- [ ] Body text 16px plain sans, 45–75ch measure on reading surfaces

## Anti-patterns
- A new accent color per campaign — palette drift kills the contract
- Body paragraphs and form labels set in display faces
- Text over busy patterns with "it looks fine" contrast
- Fifteen stickers on one product card
- A marquee carrying the only instance of the price or date
- Soft blurred shadows and glass panels under a loud palette
- Checkout as dense as the hero
- Symmetric centered template in maximal colors — abundance without composition

## Sources & inspiration
- Maximalism — the movement and its lineage: https://en.wikipedia.org/wiki/Maximalism
- Memphis Group — the 80s boundary case (what this is not): https://en.wikipedia.org/wiki/Memphis_Group
- Collage — the compositional logic: https://en.wikipedia.org/wiki/Collage
- Fonts In Use — evidence for display-face pairing: https://fontsinuse.com
- It's Nice That — contemporary maximal brand work: https://www.itsnicethat.com
- Anton — a workhorse chunky display face: https://fonts.google.com/specimen/Anton
