# Style: Vaporwave & Synthwave

> **Read when:** building UI in the vaporwave or synthwave mode. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/motion-principles.md, 00/iconography.md

**Personality:** nostalgic-ironic, dreamy, retro-futuristic, nocturnal. **Best for:** music (synthwave/chillwave/vaporwave acts), gaming, crypto-art, nightlife/events, creative experiments, streaming visuals. **Avoid for:** productivity, healthcare, enterprise, anything forms-heavy.

**Core idea:** two sibling modes sharing one DNA — 80s/90s nostalgia filtered through digital irony. Vaporwave: pastel pink/cyan, Greek busts, dead-mall and Windows-95 longing, glitch. Synthwave: dark purple nights, sunset gradients, perspective grids, neon chrome. Pick ONE mode per product; never blend 50/50.

Boundaries: futuristic-neon is immersive sci-fi product UI with HUD discipline — vapor/synth are nostalgia-irony for marketing and entertainment surfaces. Psychedelia is 60s hand-drawn fluidity; this is 80s/90s digital nostalgia. Inside the style: vapor = ironic daylight pastel, synth = nocturnal neon drama.

## Signature (what makes it recognizable)
- Vapor: pastel pink/purple/cyan backgrounds, daylight irony
- Vapor: Greek/Roman statue imagery and retro OS window frames (95/98) in surreal collages
- Vapor: VHS glitch, chromatic aberration, hijacked-corporate-logo irony
- Vapor: Japanese/Chinese text accents (decoration, never the only label)
- Synth: deep purple-navy base in the #12081F family, layered steps
- Synth: sunset gradient (magenta→orange) cut by horizontal blind lines
- Synth: perspective grid floor, neon outline shapes, chrome display type
- Shared: scanline overlays at 5–10% opacity, slow drift motion, occasional glitch snap

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `color-bg` | mode-split | vapor pastel #F5D0E8 / #C9E8F5 wash; synth #12081F → #1B0B33 steps |
| `color-surface` | mode-split | vapor solid white/pastel panels; synth #241040 cards, #33165A popovers |
| `color-text/ink` | mode-split | vapor dark ink #2A1240 (never pastel); synth near-white #F2EAFB |
| `color-accent` | one saturated pair, fixed roles | vapor pink #FF5FA8 + cyan #35D0E0; synth neon #FF3DBE + #2DE2FC |
| `gradient-sunset` | synth hero only | magenta #FF3DBE → orange #FF9E4A, 3–5 blind lines |
| `grid-floor` | synth perspective decoration | 1px neon lines at 30–50% alpha, below content |
| `overlay-scanline` | shared texture, behind content | repeating 2–3px lines at 5–10% opacity |
| `glow-*` | synth neon glow, interactive/live only | 0 0 12px @40% hover / 0 0 24px @60% focus |
| `radius-*` | era-angular or window-true | vapor 0–2px (95/98 windows); synth 0–4px angular |
| `space-scale` | poster-roomy | 8pt scale; hero zones get 2× padding |
| `type-display` | chrome or neon outline (synth), OS/system look (vapor) | 40px+; display only, never body |
| `type-body` | plain sans | vapor ink on solid pastel; synth near-white, 16px/1.5 |
| `motion-duration` | slow drift + rare snap | 300–500ms transitions; glitch <200ms, ≤1 per view |

## Rules

### R1. Pick the mode and commit
**Rule:** Choose vapor OR synth per product, record the choice in the token documentation, and hold it everywhere; at most one deliberate crossover accent per view — never a 50/50 blend.
**Why:** The palettes contradict: pastel is ironic daylight, neon-purple is nocturnal drama; mixing them reads as indecision, not nostalgia.
**Example:** An album site goes full synth — purple steps, sunset hero, grid floor — and contains zero pastel pink.

### R2. Glow marks interactive and live only
**Rule:** Same discipline as futuristic-neon: neon glow (synth) or saturated shadow (vapor) marks interactive/live states; statues, headlines and decoration never glow.
**Why:** Glow means "something happens here"; nostalgic decoration that glows floods the page with false affordances and buries the real actions.
**Example:** The "Play" button glows on hover and focus; the Greek bust behind it stays matte.

### R3. One nostalgia asset library
**Rule:** Statues, grids, OS windows, busts and palm silhouettes come from one fixed library with one rendering style — line-art OR halftone OR 3D render, never mixed.
**Why:** The collage is the style; colliding rendering styles read as a mood board instead of a designed world.
**Example:** All statues use the same marble-line style; the grid is drawn one way across hero, footer and dividers.

### R4. CJK text accents are decoration
**Rule:** Japanese/Chinese strings are aria-hidden decoration; anything meaningful exists in the user's language as real text; images carrying CJK meaning get translated alt text.
**Why:** Screen readers announce languages the user may not speak, and untranslated labels are a usability failure before an aesthetic one.
**Example:** "都市の夜" sits large behind the hero with aria-hidden; the English subtitle below carries the actual meaning.

### R5. Texture overlays stay behind content
**Rule:** Scanlines, VHS noise and chromatic aberration render at ≤10% opacity, behind text and controls, never on top; glitch occurs ≤1 per view and never mid-interaction.
**Why:** Overlays on top of text destroy legibility and sit outside the user's control; the texture should be felt, not read through.
**Example:** A full-page scanline layer at 6% under content; the headline gets a one-time 150ms RGB-split on load, then holds still.

### R6. Body text plain sans on solid surfaces
**Rule:** Body copy is a plain sans on solid panels — dark ink on pastel (vapor), near-white on the dark base (synth); pastel-on-pastel is banned; neon text only as short display strings verified 4.5:1 (3:1 at ≥24px).
**Why:** Both modes fail the same way — decoration eats the text layer; legibility is what separates a revival from pastiche.
**Example:** Tracklist in 16px ink on a solid white card; the "LIVE" badge in neon display at 4.5:1 on its chip.

### R7. Motion: slow drift + occasional snap
**Rule:** Ambient motion drifts slowly (grids scroll, statues drift on 8–20s loops); transitions run 300–500ms; glitch snaps are <200ms; nothing strobes faster than 3 flashes/second; `prefers-reduced-motion` freezes drift and disables glitch.
**Why:** The dream is hazy and slow — rapid strobing is a photosensitivity risk (WCAG 2.3.1) and instantly cheapens the reference.
**Example:** Grid floor scrolls on a 20s linear loop; under reduced motion it renders static; the one glitch never repeats.

### R8. The nostalgia chrome must actually work
**Rule:** Retro window frames and fake system dialogs wrap real functioning UI — real hit targets (≥24×24), keyboard operability, visible focus, Esc closes; decoration never blocks interaction.
**Why:** The irony only lands when the product works; a fake dialog that cannot close is just broken software wearing a costume.
**Example:** The 95-style window is a real modal: focusable title bar, working close target, focus trapped and returned.

## A11y watchpoints
- Verify every neon-on-dark pair at 4.5:1 (3:1 for ≥24px display); pastel text on pastel bg is banned — use dark ink or solid panels
- Glitch and strobe effects respect photosensitivity (≤3 flashes/s) and collapse under `prefers-reduced-motion`
- Decorative CJK, statues and scanlines are aria-hidden; meaningful imagery gets alt text
- Retro window chrome needs real targets (≥24×24), keyboard paths and focus visible against busy backgrounds
- Chromatic aberration never touches body text — red/blue splits punish low vision and color-blind users
- Re-check text contrast after overlay compositing: even 6% scanlines shift the effective background

## Checklist
- [ ] Mode chosen and documented (vapor OR synth); no 50/50 blend
- [ ] Glow only on interactive/live elements; decoration stays matte
- [ ] One rendering style across the whole nostalgia asset library
- [ ] CJK accents aria-hidden; meaning available in the user's language
- [ ] Scanline/glitch ≤10% opacity, behind content, ≤1 glitch per view
- [ ] Body text plain sans, dark-ink-on-pastel or near-white-on-dark, 4.5:1
- [ ] Neon display strings verified at size-dependent ratios
- [ ] Motion: slow drift loops, 300–500ms transitions, zero strobing
- [ ] Reduced-motion freezes drift and disables glitch
- [ ] Retro chrome is functional: targets, keyboard, focus, Esc

## Anti-patterns
- Blending both modes into pastel-neon mush
- Glowing statues and glowing headlines
- Untranslated CJK as the only label for a real action
- Full-screen glitch loops running over live text
- Pastel text on pastel background
- Five rendering styles colliding in one collage
- A fake Windows dialog with no usable close button
- Neon body paragraphs

## Sources & inspiration
- Vaporwave — Wikipedia overview of the movement and its visuals: https://en.wikipedia.org/wiki/Vaporwave
- Synthwave — Wikipedia overview of the genre and aesthetic: https://en.wikipedia.org/wiki/Synthwave
- CARI — Consumer Aesthetics Research Institute, documents both modes: https://cari.institute
- Web Design Museum — the 90s/2000s UI archaeology this style quotes: https://www.webdesignmuseum.org
- WCAG 2.2 Understanding 2.3.1 — the flash threshold for glitch effects: https://www.w3.org/WAI/WCAG22/Understanding/three-flashes-or-below-threshold.html
- Chromatic aberration — the optical artifact behind the RGB-split look: https://en.wikipedia.org/wiki/Chromatic_aberration
