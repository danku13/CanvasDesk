# Style: Acid Graphics

> **Read when:** building UI in the acid-graphics style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/motion-principles.md, 00/iconography.md, 00/visual-hierarchy.md

**Personality:** hallucinatory, digital, rebellious, hyper-modern. **Best for:** music and rave events, streetwear drops, web3 with edge, gaming promos, counter-culture media. **Avoid for:** anything forms-heavy, banking, healthcare, government, kids.

**Core idea:** chrome corrupted — liquid chrome type, hyper-saturated gradients, and glitch/RGB-split distortion carrying rave-flyer energy; expressive zones are anti-ergonomic on purpose, and functional UI stays completely hostile-free.

## Signature (what makes it recognizable)
- Liquid chrome / metallic 3D display type, rendered as artwork — never live text
- Hyper-saturated gradient set: acid lime `#CCFF00`, magenta, cyan on stark black or white
- Glitch, RGB-split, and moiré effects as accents, never as full layers
- Barcode, tech-template, and xerox-collage motifs
- Blurred ghost-duplicate type trails
- Dense overprinted compositions where layers deliberately collide
- One plain readable sans doing all functional text — the calm in the storm

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | sharp | 0–4px; pills only as ticket/badge shapes |
| `shadow-*` | none or hard | none at rest; glow belongs to artwork, not elevation |
| `color-bg` | stark black or white | `#0A0A0A` (rave) / `#FFFFFF` (zine) |
| `color-surface` | solid panel over noise | `#111111` / `#FAFAFA`, always opaque |
| `color-ink` | opposite of bg | `#F5F5F5` on dark (≈18:1), `#0A0A0A` on light |
| `color-acid-1` | hero acid hue | lime `#CCFF00` (on black ≈18:1) |
| `color-acid-2` | secondary acid hue | magenta `#FF00E5`-class (on black ≈6.4:1) |
| `color-acid-3` | tertiary, sparse | cyan `#00F0FF` |
| `gradient-set` | defined once | 2–3 gradients as image assets, fixed stops |
| `type-display` | chrome artwork | rendered SVG/PNG/WebP per headline, with alt text |
| `type-body` | plain readable sans | 16px / 1.5 on solid panels — never stylized |
| `motion-glitch` | transition accent only | 150–300 ms, static or slow by default |

## Rules

### R1. Chrome type is artwork, not text
**Rule:** Liquid chrome, glitched, and blurred-ghost type exist only as rendered images/SVG with proper alt text — never body text, never button labels, never form text, never anything a screen reader or a 320px viewport must parse as text.
**Why:** Chromed and distorted letterforms are unreadable at small sizes, unusable as live text, and invisible to assistive tech; as labeled artwork they keep the aesthetic while the real text layer stays machine-readable.
**Example:** Hero: chrome "ACID NIGHTS" as WebP with alt text. Date, venue, and the buy button below in plain 16px sans on a black panel.

### R2. Photosensitivity is a hard gate
**Rule:** Nothing flashes faster than 3 per second (WCAG 2.3.1); glitch effects are static by default or slow (≥1s cycle), and there is no strobe, flicker, or rapid RGB-cycling anywhere — marketing "strobe" moments become static distortion artwork.
**Why:** This style flirts with seizure-inducing territory by default; the flash limit is non-negotiable, and a style that cannot pass 2.3.1 cannot ship.
**Example:** The drop teaser uses a 1.2s RGB-split cycle with crossfade — never a 100ms strobe.

### R3. Saturation budget: ≤20% acid
**Rule:** Acid hues cover at most ~20% of any view — accents, one hero moment, thin strips; the remaining ground is stark black/white with gray structure.
**Why:** Hyper-saturation works by scarcity; at 60% coverage the page is a uniform scream with no hierarchy, and the acid stops signaling anything.
**Example:** Event page: black ground, lime headline artwork, one magenta marquee strip, everything else white-on-black type.

### R4. Body text is boring on purpose
**Rule:** All functional text — body, labels, buttons, forms, prices — is a plain readable sans at 16px on solid opaque panels; no stylization, no outline text, no text over compositions.
**Why:** The expressiveness lives in artwork zones; functional text must simply work, and the contrast between hostile art and boring UI is the style itself, not a compromise.
**Example:** Ticket checkout: white panels, black sans text, standard fields; acid appears only in the header strip.

### R5. Glitch is a transition accent, not a state
**Rule:** Glitch/RGB-split runs as hover or transition accents at 150–300 ms on non-essential elements — never on reading text, never as the only indicator of state, and fully collapsed under `prefers-reduced-motion`.
**Why:** Essential information inside a distortion effect is information users can miss; glitch reads as atmosphere, while states need stable signals.
**Example:** Nav links glitch-shift on hover for 200 ms; the active link is statically underlined in lime.

### R6. Ghosts are decorative
**Rule:** Blurred duplicates, echo trails, and moiré layers are `aria-hidden`, `pointer-events: none`, and sit behind or beside content — never wrapped around live text, never interactive.
**Why:** Screen readers would announce duplicated content twice, and ghost layers over links create fake targets; decorative layers must be invisible to both.
**Example:** The headline has one offset cyan ghost behind it (`aria-hidden`, non-interactive); links never carry ghosts.

### R7. Acid is not neon, and not y2k
**Rule:** Futuristic-neon is disciplined HUD glow on layered darks, where glow marks interactive/live state; y2k-chrome is optimistic glossy chrome and bubble optimism; acid is corrupted chrome — blur, overprint, distortion, anti-design energy. Glow-marks-state → build neon; smiling chrome → build y2k; melting chrome → build acid.
**Why:** The three share a techno palette and get blended into mush; the distinction is behavioral (glow = state vs distortion = art) and tonal (optimism vs corrosion).
**Example:** Neon: outline button whose hover intensifies its glow. Acid: chrome logo artwork with a torn RGB edge; the actual button is flat lime with black text.

## A11y watchpoints
- WCAG 2.3.1 (≤3 flashes/s) is a hard gate — audit every animation frame-by-frame, including strobe artwork, GIFs, and video backgrounds
- Chromed/glitched headlines are images: alt text mandatory, and the same words must exist as real text (visible subline, title, or meta)
- Contrast happens on solid panels over dense compositions — verify every pair 4.5:1 (lime-on-black ≈18:1 is your friend; magenta-on-white ≈3.3:1 is display-only)
- All glitch/ghost/marquee motion collapses to static artwork under `prefers-reduced-motion`, losing nothing essential
- Never signal state by acid hue alone in a 3-acid system — pair with label, underline, or shape
- Ghost layers must be `pointer-events: none` so they never block targets — check stacking on mobile

## Checklist
- [ ] All chrome/glitch display type is rendered artwork with alt text
- [ ] Nothing flashes >3/s anywhere (incl. video, GIFs, lottie files)
- [ ] Acid hues ≤ ~20% of every view
- [ ] Functional text plain sans ≥16px on opaque solid panels
- [ ] Glitch only as 150–300 ms hover/transition accents, never essential
- [ ] Ghosts/echoes `aria-hidden` + `pointer-events: none`
- [ ] All text pairs ≥4.5:1 on their actual panels; state never color-only
- [ ] Reduced-motion: everything static, nothing essential lost
- [ ] 320px test: artwork scales or swaps to plain display type, zero overflow

## Anti-patterns
- Strobe intro flashes "for the rave vibe"
- Chrome or glitched fonts for body copy, buttons, or form labels
- Acid gradients covering the whole viewport
- Text set directly over overprinted collage
- Ghost trails wrapped around live links
- Essential content (dates, prices) living only inside a glitch animation
- Bubble-gloss optimism (that's y2k) wearing acid hues
- Hostile forms: distorted inputs and unreadable labels "as a statement"

## Sources & inspiration
- Glitch art — the core distortion vocabulary: https://en.wikipedia.org/wiki/Glitch_art
- Acid house — the rave-flyer lineage: https://en.wikipedia.org/wiki/Acid_house
- Chromatic aberration — the RGB-split physics: https://en.wikipedia.org/wiki/Chromatic_aberration
- Photosensitive epilepsy — why 2.3.1 exists: https://en.wikipedia.org/wiki/Photosensitive_epilepsy
- WCAG 2.3.1 Three Flashes or Below Threshold: https://www.w3.org/WAI/WCAG21/Understanding/three-flashes-or-below-threshold.html
- It's Nice That — contemporary acid/rave design coverage: https://www.itsnicethat.com
