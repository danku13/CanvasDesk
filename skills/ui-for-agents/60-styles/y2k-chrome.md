# Style: Y2K Chrome

> **Read when:** building UI in the y2k-chrome style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/visual-hierarchy.md, 00/motion-principles.md, 10/design-tokens.md

**Personality:** techno-optimistic, shiny, futuristic-naive, playful. **Best for:** fashion drops, music/beauty brands, Gen-Z campaigns, party/event sites, streaming promo pages. **Avoid for:** banking, enterprise, government, long-session productivity.

**Core idea:** the future as imagined in the year 2000 — liquid chrome, iridescent bubbles, translucent tech-blue panels and rounded squishy hardware optimism (the iMac G3 lineage); a naive, sincere faith that technology equals fun.

Boundaries: futuristic-neon is dark HUD glow discipline; acid-graphics corrupts and distorts the chrome; vaporwave is ironic nostalgia where Y2K is sincere optimism. If the product needs trust over excitement, do not use this style.

## Signature (what makes it recognizable)
- Liquid-chrome and metallic-gradient display type and blobs
- Iridescent/holographic accents (oil-slick sheen on focal elements only)
- Translucent blue/silver glassy panels with thin light borders
- Bubble/squishy rounded shapes — the iMac G3 hardware language
- Thin white gloss highlights along top edges (one believable light source)
- Lens flares and four-point star sparkles
- Palette: silver/white base with translucent cyan-pink-lilac, or the dark "space" variant
- Pixel-era leftovers used ironically — pixel cursors, chunky bevels, bitmap badges

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | bubble-round | 16–32px cards, pills and circles; blobs from one SVG set |
| `shadow-*` | colored glow + soft drop | 0 8px 24px rgba(80,140,255,.35); 1px light inner border |
| `space-scale` | roomy | 8pt scale, generous hero and section padding |
| `color-bg` | silver-white or space dark | #E8ECF4 light variant / #0B0B2A space variant |
| `color-surface` | glassy translucent or solid tint | rgba(255,255,255,.55)+blur, fallback solid #DDE6F5 |
| `color-text/ink` | deep blue-black, never #000 | #101433 light variant; #F4F7FF on space variant |
| `color-accent` | tech blue | #2E6BFF family |
| `color-accent-2` | iridescent ramp, focal only | cyan #6FE7FF → lilac #C7A6FF → pink #FF9AD5 |
| `gloss-*` | thin white top sheen | linear-gradient(rgba(255,255,255,.5), transparent 40%) |
| `chrome-*` | metallic ramp, 4–6 stops | #F8FAFF → #B9C4D9 → #6E7A96 → #E9EEF9 |
| `type-display` | chrome artwork or bold rounded sans | 40px+; chrome rendered as SVG/image with alt text |
| `type-body` | plain neutral sans | 16px, line-height 1.5, on solid panels only |
| `motion-duration` | quick springy + shimmer | 200–400ms ease-out; shimmer on focal elements only |

## Rules

### R1. Chrome type is display artwork
**Rule:** Chrome/metallic lettering is rendered as SVG or image with alt text and used only for hero display; body text, labels and buttons stay plain, and nothing is interactive by appearance alone.
**Why:** Chrome gradients are illegible below display sizes, and text baked into images is invisible to assistive tech — the era's classic sin.
**Example:** Hero title "CYBER DROP 01" is an SVG with `alt="Cyber Drop 01"`; the "Shop now" button below is real text on a solid accent fill.

### R2. Gloss is a thin highlight, not candy
**Rule:** Gloss is one white overlay — ~50% alpha fading out by 40% height — plus at most a 1px light inner border; never stacked gradients or bevel-on-bevel.
**Why:** The era's realism came from a single believable light source; stacked highlights read as candy and wreck label contrast on buttons.
**Example:** A tech-blue button with one top sheen; its label holds 4.5:1 against the lightest band of the fill, not just the base.

### R3. Iridescence budget: 1–2 focal elements
**Rule:** Per view, at most two holographic focal elements (hero type or one blob); every other surface stays solid silver, white or blue.
**Why:** Holography is the style's most expensive resource; scattered across cards it becomes noise and dissolves the focal hierarchy.
**Example:** Product grid: iridescent blob behind the hero, solid silver cards with flat ink below.

### R4. Body text lives on solid panels
**Rule:** Body copy, forms and data sit on solid fills or scrimmed panels that pass worst-point contrast; translucent glass holds only short display text or nothing.
**Why:** Whatever sits behind a translucent panel shifts with imagery; body text needs a predictable surface to stay readable.
**Example:** Event schedule in dark ink on a solid white card; the glassy panel beside it carries only the date in display type.

### R5. Lens flare budget: ≤1 per composition
**Rule:** At most one lens flare or star sparkle per composition, never over text, always decorative (aria-hidden).
**Why:** Flares are the instant-dating risk of the style and they mask content when overlaid; one is signature, three is clip-art.
**Example:** One flare at the hero's upper corner; the product grid stays completely clean.

### R6. One bubble geometry set
**Rule:** All rounded shapes come from one geometry set — fixed radius steps plus one squish/blob family reused everywhere; no per-screen corner improvisation.
**Why:** The squishiness reads as industrial-design lineage (translucent plastic hardware) only when systematic; random radii read as sticker sheets.
**Example:** Pills at radius-full, cards at 24px, blobs from one 3-variant SVG set — identical on every page.

### R7. Motion: quick spring, disciplined shimmer
**Rule:** Transitions run 200–400ms with slight overshoot allowed on buttons; shimmer/spin loops only on the iridescent focal elements; `prefers-reduced-motion` collapses them to the static gradient.
**Why:** Bouncy motion is the hardware optimism of the era, but infinite shimmer everywhere is sensory noise that competes with content.
**Example:** Button hover at 250ms with 4% scale overshoot; the hero blob's sheen sweep freezes under reduced motion.

### R8. The space variant keeps the contrast floor
**Rule:** The dark space version uses layered near-black blues with near-white text; chrome and iridescence stay focal; silver-on-silver text is banned in both variants.
**Why:** Silver-on-silver is the style's signature contrast failure; the dark variant only survives if text stays near-white on the darkest steps.
**Example:** Space variant: #F4F7FF body on #0B0B2A, chrome logo SVG above, iridescence only on the hero object.

### R9. Pixel leftovers are accents, not systems
**Rule:** Pixel cursors, chunky bevels and bitmap badges appear as 1–2 ironic accents per view, aria-hidden, and never replace real cursors, focus rings or controls.
**Why:** They are jokes about the era, not the interface; promoted to a system they break input affordances and accessibility.
**Example:** A pixel star beside the footer badge; the actual cursor, focus outlines and controls stay fully modern.

## A11y watchpoints
- Chrome text is an image: alt text is mandatory, decorative variants get empty alt, interactive chrome carries real text or aria labels
- Text over iridescent or translucent zones has unpredictable contrast — verify worst-point 4.5:1 on the actual gradient, or use solid panels/scrims
- Gloss lightens the top of buttons: check label contrast against the lightest band, not the base fill
- Shimmer, spin and flare loops collapse under `prefers-reduced-motion`; nothing flashes more than 3×/second (WCAG 2.3.1)
- Decorative pixel cursors must never replace the real cursor or focus affordances — `pointer-events: none`, aria-hidden
- Test both palette variants: silver-on-silver (light) and gray-on-dark (space) are the classic failures

## Checklist
- [ ] Chrome lettering is SVG/image with alt text; body text is plain sans
- [ ] ≤2 iridescent focal elements per view; the rest solid
- [ ] ≤1 lens flare per composition, never over text
- [ ] Gloss = single top-40% white sheen, no stacked bevels
- [ ] Body text on solid or scrimmed panels, 4.5:1 verified
- [ ] One bubble geometry set across the product
- [ ] Motion 200–400ms; shimmer collapses under reduced-motion
- [ ] Pixel/bevel jokes ≤2 per view, aria-hidden, never replacing affordances
- [ ] Both variants (silver light / space dark) pass contrast checks

## Anti-patterns
- Chrome body text or chrome labels — unreadable by design
- Candy gloss overload: bevel on bevel, gloss on every element
- Holographic everything: gradients on every card and button
- Flares over headlines; five sparkles per section
- Text inside a translucent panel floating over busy imagery
- Random radius soup: 4px here, 40px there, a blob everywhere
- Fake UI drawn as bevel icons with no hit target (the bevel "X" close)
- Distorted, corrupted chrome — that is acid-graphics, not Y2K

## Sources & inspiration
- Y2K aesthetic — Wikipedia overview of the movement and its 2020s revival: https://en.wikipedia.org/wiki/Y2K_aesthetic
- iMac G3 — the translucent hardware optimism this style inherits: https://en.wikipedia.org/wiki/IMac_G3
- Web Design Museum — galleries of 1990s–2000s web design: https://www.webdesignmuseum.org
- CARI — Consumer Aesthetics Research Institute, documents the revival: https://cari.institute
- Lens flare — the era's most abused effect, used here under a strict budget: https://en.wikipedia.org/wiki/Lens_flare
- WCAG 2.2 Understanding 2.3.1 — flash and photosensitivity threshold: https://www.w3.org/WAI/WCAG22/Understanding/three-flashes-or-below-threshold.html
