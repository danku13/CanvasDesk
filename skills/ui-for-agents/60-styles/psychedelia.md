# Style: Psychedelia

> **Read when:** building UI in the psychedelia style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/motion-principles.md, 00/typography.md, 00/visual-hierarchy.md

**Personality:** trippy, fluid, immersive, rebellious, expressive. **Best for:** music festivals, vinyl/culture shops, immersive art experiences, entertainment heroes, creative event pages. **Avoid for:** any productivity surface, forms-heavy products, data tools, healthcare.

**Core idea:** liquid type, vibrating complementary color pairs, and swirling gradients — the style lives entirely in expressive hero zones while the working interface underneath stays modern and calm. A 1967 poster on top of a 2025 product.

## Signature (what makes it recognizable)
- Warped, liquid display type — letterforms that melt and merge into each other
- Vibrating complementary pairs (magenta/lime, orange/cyan) on shapes and borders
- Swirl and liquid gradients as hero backdrops
- Melting blob containers instead of rectangles in expressive zones
- Mandala and kaleidoscope ornaments
- 60s poster composition: arched text baselines, tight line spacing
- A calm modern interface underneath — the poster never leaks into the forms

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | organic blobs in hero zones | irregular (e.g. `55% 45% 60% 40%`); calm chrome 8–16px |
| `shadow-*` | flat poster | none; depth comes from gradients and layering |
| `space-scale` | split personality | hero zones free-form; working UI standard 8pt |
| `color-bg` | cream canvas | `#FFF6E8` for calm chrome |
| `hero-gradient` | swirl, 2–3 stops | violet `#5B2A86` → magenta `#D3268E` → orange `#FF7A00` |
| `color-ink / text` | near-black on solid panels | `#1A1A1A` |
| `vibrating-pair-a` | shapes/borders only | magenta `#D3268E` + lime `#A8E10C` |
| `vibrating-pair-b` | shapes/borders only | orange `#FF7A00` + cyan `#00C2C7` |
| `type-display` | warped liquid, display-only | custom warped SVG/type, ≥32px |
| `type-display-fallback` | same info, legible | bold sans mirroring the display hierarchy |
| `type-body` | plain sans on solid panels | 16px / 1.5 |
| `motion-duration` | slow hero loops, fast UI | hero drifts 6–20 s; UI 150–250 ms; reduced-motion → static |
| `warp-set` | one distortion kit | liquid type + blob SVG assets from a single source |

## Rules

### R1. Psychedelia lives in hero/expressive zones only
**Rule:** Declare expressive zones (hero, section art, event promos) before styling; forms, tables, checkout, and settings render modern-calm with standard chrome.
**Why:** This is the most stimulation-heavy style in the library — the marketing-vs-product boundary discipline that governs expressive styles applies doubly here.
**Example:** Festival landing is a full swirl poster; the ticket form is white background, black text, default focus rings.

### R2. Vibrating pairs vibrate shapes, never text
**Rule:** Magenta/lime and orange/cyan pairs appear on shapes, borders, and fills — never as text-on-text or text-on-gradient pairs.
**Why:** Complementary pairs of similar luminance are unreadable as type by construction; as adjacent shapes they produce the signature optical buzz.
**Example:** A lime blob outlined in magenta sits behind the lineup; the lineup text itself is black on cream.

### R3. Warped type is display-only, with a legible twin
**Rule:** Liquid/warped lettering appears at ≥32px and always has a solid-color legible fallback in the same zone carrying the same information (image alt text or an adjacent plain heading).
**Why:** Same information twice — the audio-description principle: expression for visual flair, plain text for actual reading, scanning, and assistive tech.
**Example:** Warped "SUMMER OF SOUND" SVG with descriptive alt text, plus a plain bold sans subheading with the dates.

### R4. Loops collapse to a static poster
**Rule:** All swirl/drift/pulse animation collapses to a static composition under `prefers-reduced-motion`; autoplaying loops are slow (≥6 s cycles), non-essential, and never carry information.
**Why:** Continuous organic motion is a vestibular trigger — and the style's beauty survives perfectly as a still poster.
**Example:** Hero swirl drifts on a 12 s loop for most users; reduced-motion users get the exact poster frozen mid-frame.

### R5. Body text sits on solid panels
**Rule:** All reading text (paragraphs, lineups, prices, legal) sits on solid cream or white panels in plain 16px sans — never directly on gradients or blobs.
**Why:** Contrast on a gradient is unauditable; measuring against the worst point is a trap, so the style simply doesn't play that game.
**Example:** The event description sits on a `#FFF6E8` card floating over the swirl.

### R6. One focal swirl per view
**Rule:** Each view gets exactly one psychedelic focal composition (swirl, mandala, kaleidoscope); secondary zones stay quiet or static.
**Why:** Kaleidoscope everywhere is noise — one strong focal point is what makes everything else read as intentional.
**Example:** The hero has THE swirl; the lineup section uses at most a single static mandala corner ornament.

### R7. Contrast is measured at the worst gradient point
**Rule:** Any text near expressive zones is either on a solid panel or verified at 4.5:1 against the gradient's darkest AND lightest points — assume it fails and panel it.
**Why:** Gradients swing wildly; auditing one point of a swirl is meaningless on the most contrast-hostile surface in the library.
**Example:** "Book tickets" is ink-on-cream, never white-on-swirl.

## A11y watchpoints
- The most contrast-hostile style in the library: every text pair checked against the worst-case gradient point — or just panel everything (the default)
- Motion loops are vestibular triggers: the `prefers-reduced-motion` static fallback is mandatory, not optional; stay inside WCAG 2.3.1 flash/pulse limits
- Arched and baseline-shifted text is display-only: screen readers need plain DOM text, so warped/arched art always carries equivalent text
- Never animate essential info — prices, dates, and errors stay static and solid
- Vibrating pairs fail as text by design; verify any UI boundary they form at 3:1 against adjacent surfaces
- Focus rings must survive the noise: 2px offset solid ring, tested over gradients and blobs

## Checklist
- [ ] Psychedelic treatment confined to declared expressive zones
- [ ] Forms, tables, checkout modern-calm with default chrome
- [ ] Vibrating pairs never used for text
- [ ] Warped type ≥32px with same-info legible fallback (alt or adjacent)
- [ ] All body text on solid panels, pairs ≥4.5:1
- [ ] Reduced-motion → static poster; loops ≥6 s and non-essential
- [ ] One focal swirl composition per view
- [ ] Essential info never animated
- [ ] Focus visible over every expressive backdrop

## Anti-patterns
- Warped type on buttons, labels, or form fields
- Paragraphs set directly on swirl gradients
- Kaleidoscope in every section — six focal points equals none
- Fast pulsing loops that ignore `prefers-reduced-motion`
- Psychedelia inside checkout "for consistency"
- Magenta-on-lime text because it looks exciting (it reads as nothing)
- Dark HUD chrome and neon glow — that is `futuristic-neon`'s tech discipline, not 60s organic fluid
- 90s/2000s digital nostalgia (grids, statues, pastel PC aesthetics) — that is vaporwave, a different decade's dream

## Sources & inspiration
- Psychedelic art — the movement: https://en.wikipedia.org/wiki/Psychedelic_art
- Wes Wilson — the liquid-letter pioneer: https://en.wikipedia.org/wiki/Wes_Wilson
- Victor Moscoso — vibrating-color poster master: https://en.wikipedia.org/wiki/Victor_Moscoso
- Op art — the vibrating-pair optics this style borrows: https://en.wikipedia.org/wiki/Op_art
- Fillmore West — the poster scene this style comes from: https://en.wikipedia.org/wiki/Fillmore_West
- SFMOMA — holds psychedelic poster collections: https://www.sfmoma.org
