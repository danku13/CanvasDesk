# Style: Frutiger Aero

> **Read when:** building UI in the frutiger-aero style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/typography.md, 00/color.md, 00/elevation-depth.md, 00/motion-principles.md

**Personality:** fresh, optimistic, glossy-natural, humanist-tech. **Best for:** eco/environment apps, weather, wellness/health, consumer software marketing, nostalgia products, kids-friendly brands. **Avoid for:** dark-luxury brands, brutalist-leaning tech, underground culture.

**Core idea:** humanist tech-utopia — glossy aqua buttons, sunshine and water, nature photography (sky, grass, bubbles, fish), a blue-green palette and a humanist sans (the Frutiger/Segoe lineage); the era sincerely believed technology would be clean, green and friendly.

Boundaries: glassmorphism is the modern neutral descendant — blur-first, palette-free; frutiger-aero is the specific 2000s glossy-nature nostalgia with the aqua palette and the fixed gloss recipe. y2k-chrome is plastic hardware optimism; frutiger-aero is nature-gloss software optimism. Colored neon belongs to futuristic-neon, never here.

## Signature (what makes it recognizable)
- Aqua/glossy buttons: vertical gradient + top white sheen + inner glow, as one recipe
- Nature photography backgrounds — sky, water, leaves, bubbles, fish
- Blue-green-cyan-white palette, high-key and airy
- Soft white outer glows around panels and photos
- Rounded translucent glass panels floating over the scene
- Humanist sans everywhere (Frutiger/Segoe/Myriad lineage), sentence case
- Subtle aurora/gradient sweeps; glass reflections and water droplets as accents

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | soft-rounded | 8–16px; circles for bubbles and avatars |
| `shadow-*/glow-*` | soft white glow + gentle drop | 0 0 16px rgba(255,255,255,.5); 0 6px 16px rgba(30,90,120,.18) |
| `space-scale` | airy, high-key | 8pt scale, generous padding around glass panels |
| `color-bg` | sky/water photo over light gradient | gradient #EAF6FA → #CFEAF2 under the photo |
| `color-surface` | translucent glass or solid tint | rgba(255,255,255,.65)+blur, fallback solid #F4FBFD |
| `color-text/ink` | deep ocean slate, never #000 | #0E3A4A |
| `color-accent` | aqua blue | #1E9AD6 family |
| `color-accent-2` | leaf green (nature/success role) | #58B946 |
| `color-cta` | one warm accent, CTA only | #F5A623 family |
| `gloss-*` | fixed gloss recipe | fill #59C1E8→#1E7FB8 + white top-40% sheen + 1px rgba(255,255,255,.8) inner border |
| `type-display` | humanist sans, medium weight | 32–48px, sentence case |
| `type-body` | humanist sans | 16px, line-height 1.5, on solid/scrimmed zones |
| `motion-duration` | smooth, nothing glitches | 250–350ms ease-out; ambient loops ≥4s |

## Rules

### R1. The gloss recipe is fixed
**Rule:** Every glossy surface uses the identical recipe — vertical gradient fill + white top-40% sheen overlay + 1px light inner border — shipped as one token set; never hand-tune per component.
**Why:** The era's gloss read as manufactured realism because it was systematic; ad-hoc gloss looks melted, and mismatched recipes break the clean-factory promise.
**Example:** The aqua button and the nav bar both use `gloss-aqua`; only the gradient hue token changes between them.

### R2. Nature sits behind, text sits on solid
**Rule:** Scene imagery (sky, water, grass) is a background layer; text lives on solid fills or scrimmed glass panels that pass worst-point contrast — the discipline glassmorphism inherited from this style.
**Why:** Sky and water luminance shifts unpredictably across the image; text placed directly on it fails in ways you cannot test once and keep.
**Example:** Weather figures on a solid white panel over the sky photo; the scene stays visible around the panel's soft edges.

### R3. The palette anchors to blue-green
**Rule:** Backgrounds and accents stay in the blue-green-cyan family; exactly one warm accent is allowed and reserved for the primary CTA.
**Why:** The blue-green anchor IS the nature-tech optimism; warm hues flood in and the style collapses into a generic Web 2.0 rainbow.
**Example:** Aqua links and buttons, green success states, one orange "Get started" — nothing else warm on the page.

### R4. Humanist sans everywhere
**Rule:** All text uses a humanist sans (Frutiger/Segoe/Myriad lineage) — display type scales it, never replaces it; geometric-cold, techy or mono faces are banned from prose.
**Why:** The style is literally named after a typeface philosophy — human warmth inside technology; a cold grotesque breaks the promise before anything else does.
**Example:** Headings and body in a Segoe-like face; numerals use the same face's tabular figures, no data-terminal mono.

### R5. Glow is soft white, never neon
**Rule:** Outer glows are white soft light (12–24px, 40–60% alpha) around panels and photos; colored or neon glow is outside this style's vocabulary.
**Why:** White glow reads as sunshine and clarity — the style's core metaphor; a colored glow instantly re-skins it as sci-fi night.
**Example:** Photo cards carry a soft white halo; nothing magenta or cyan ever glows on this UI.

### R6. Nature imagery is art-directed or replaced
**Rule:** Photos are compressed, art-directed and hue-graded to one blue-green grade; every scene has a solid gradient fallback and a documented alt-text decision (meaningful vs decorative).
**Why:** Mismatched stock photography shatters the single natural world, and heavy images betray the era's light-fast-web spirit.
**Example:** All backgrounds graded to the same cyan-blue; if the photo fails to load, the `color-bg` gradient shows and the layout survives.

### R7. Motion breathes, never glitches
**Rule:** Transitions run 250–350ms ease-out; ambient loops (rising bubbles, breathing glows) are slow (≥4s) and few (≤2 per view); no glitch, snap or strobe.
**Why:** The feel is frictionless clean software — calm water; anything jittery belongs to vaporwave and instantly breaks the serenity.
**Example:** Bubbles rise over 6s behind a scrim; buttons fade in 300ms; nothing on the page ever jumps.

### R8. Gloss is never the only state signal
**Rule:** Hover/active/focus change fill, border or inset shadow beyond the sheen; keyboard focus always shows a visible ring at ≥3:1 against its surface.
**Why:** A sheen shift is subtle light movement — invisible on dim displays and to low-vision users; state must survive without the gloss.
**Example:** Hover darkens the gradient one step and deepens the border; focus adds the 2px ring on top.

## A11y watchpoints
- White text on light sky panels fails — panels must darken or text uses the deep ocean slate; verify 4.5:1 against the brightest zone
- Text over imagery: check worst-point contrast across the whole photo/gradient area, never the average
- Glossy buttons need a state change beyond gloss (fill/border shift); focus ring ≥3:1 and never glow-only
- Ambient loops (bubbles, breathing glows) freeze under `prefers-reduced-motion` to the static gradient
- Meaningful photos need alt text; decorative scenes get empty alt plus the solid fallback
- High-key light theme is the default — audit for stray white-on-light pairs after every token change

## Checklist
- [ ] One gloss recipe token applied to every glossy surface
- [ ] Text on solid or scrimmed zones; worst-point 4.5:1 verified
- [ ] Palette anchored blue-green; ≤1 warm CTA accent
- [ ] Humanist sans for all text; no cold or geometric faces
- [ ] All glows white; zero neon
- [ ] Photos graded consistently, with gradient fallback and alt-text decisions
- [ ] Motion 250–350ms; ambient loops ≥4s, ≤2 per view, reduced-motion freezes them
- [ ] States visible without gloss; focus ring on every interactive element
- [ ] No white-on-light text pairs anywhere in the theme

## Anti-patterns
- Neon glows, dark sci-fi moods, HUD chrome — a different style entirely
- Gloss on everything, including text and icons ("melted plastic")
- Text dropped directly onto sky/water photos with no scrim
- Rainbow Web 2.0 gradients breaking the blue-green anchor
- Cold geometric or terminal-mono type for body copy
- Ungraded stock-photo collage with clashing color temperatures
- Glitch, scanline or VHS effects — vaporwave's vocabulary
- Skeuomorphic texture cosplay: wood, leather, stitching — Aero glosses, it does not upholster

## Sources & inspiration
- Frutiger Aero — Wikipedia article on the aesthetic and its revival: https://en.wikipedia.org/wiki/Frutiger_Aero
- Frutiger (typeface) — the humanist sans the style is named after: https://en.wikipedia.org/wiki/Frutiger_(typeface)
- Windows Aero — the era's flagship glossy design language: https://en.wikipedia.org/wiki/Windows_Aero
- Web 2.0 — the software era this style dressed: https://en.wikipedia.org/wiki/Web_2.0
- CARI — Consumer Aesthetics Research Institute, archivist of the revival: https://cari.institute
- Web Design Museum — galleries of 2000s consumer software and web UI: https://www.webdesignmuseum.org
