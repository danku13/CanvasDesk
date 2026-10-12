# Style: Punk Zine

> **Read when:** building UI in the punk-zine style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/typography.md, 00/color.md, 00/visual-hierarchy.md, 00/spacing-layout-grids.md

**Personality:** raw, confrontational, do-it-yourself, anti-establishment, honest. **Best for:** music scenes, independent media, zines/blogs, skate/streetwear, activism campaigns. **Avoid for:** finance, healthcare, enterprise, government.

**Core idea:** the photocopier is the aesthetic — xerox grain, torn edges, tape strips, ransom-note lettering, marker scribbles, deliberate misalignment; the honesty of cheap reproduction, assembled with intent.

## Signature (what makes it recognizable)
- Photocopy noise/grain at 8–15% — louder than any other style, and that is the point
- Ransom-note headline collage: mixed cut-out letterforms
- Torn paper edges and tape strips as recurring fixed assets
- Marker annotations: arrows, circles, scribbled underlines
- Palette: xerox gray-white `#EDEDE8` + black + ONE riot color (safety red/orange)
- Misaligned stacked layout with small rotations (≤4°)
- Typewriter/mono body or plain sans — the reading layer never performs

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | cut paper | 0–2px; torn edges are assets, not radii |
| `shadow-*` | none or misprint | none; optional `2px 2px 0 #0D0D0D` "double-pass" offset for stamps |
| `space-scale` | tight print | 8pt scale, compact but never below 12px component gaps |
| `color-bg` | xerox gray-white | `#EDEDE8` |
| `color-surface` | paper panel | `#F6F6F1` |
| `color-ink / text` | photocopy black | `#0D0D0D` |
| `color-accent` | ONE riot color | safety red `#E03131` (alt: safety orange `#FF6B00`) |
| `type-display` | ransom-note collage | composed SVG/image artwork with full alt text; typewriter mono headers 20px+ |
| `type-body` | mono or plain sans | 16px / 1.5 |
| `texture-overlay` | xerox grain | 8–15% opacity, imagery zones only |
| `rotation-max` | bounded tilt | 4°, max 2–3 rotated elements per view |
| `asset-set` | one collage kit | torn edges, tape strips, arrows, circles — a single library |
| `motion-duration` | nearly none | 0–100 ms snaps or none |

## Rules

### R1. Ransom-note type is display-only artwork
**Rule:** Cut-out letter collages exist only as headlines, composed as SVG/image assets with full alt text — never for paragraphs, buttons, links, or navigation.
**Why:** Mixed letterforms destroy reading speed; as images with alt text they stay expressive for the eye and accessible to everyone else.
**Example:** "ISSUE #7 OUT NOW" as a collage image with alt "Issue 7 out now"; the buy button beneath it is plain mono text.

### R2. Misalignment is bounded
**Rule:** Rotations stay ≤4°, at most 2–3 rotated elements per view, and the DOM reading order stays intact regardless of visual scatter.
**Why:** The mess is choreographed — beyond those limits it reads as broken CSS, and scattered DOM order breaks screen readers and keyboard flow.
**Example:** Two stacked flyers rotated -2° and +3°; the underlying HTML lists their content in logical order.

### R3. Xerox texture behind imagery, not body text
**Rule:** Grain sits at 8–15% over imagery and empty zones; every text zone gets a solid paper panel with contrast measured against the solid color.
**Why:** Grain is the style's voice, but under text it shreds letterforms and makes 4.5:1 unauditable — panels keep the noise where it belongs.
**Example:** The photo collage carries 12% grain; the article text sits on a solid `#F6F6F1` panel.

### R4. One riot color is the marker pen
**Rule:** Exactly one saturated accent (safety red or safety orange) works like the marker in the margin — highlights, arrows, urgent CTAs — never a second brand color, never more than one.
**Why:** The xerox world is black-and-white; the single riot color is what the eye came for, and adding more turns protest into party.
**Example:** A red marker circle around the gig date, a red arrow to the CTA, a red highlight on "SOLD OUT".

### R5. Body stays readable
**Rule:** Body copy is mono or plain sans at 16px minimum, line-height 1.5, on solid panels; typewriter flavor never drops below readable sizes.
**Why:** The aesthetic is cheap reproduction, not cheap legibility — zines were dense but readable, and the web needs 4.5:1-verified pairs.
**Example:** Article body in 16px mono on `#F6F6F1`; pull quotes in 20px typewriter.

### R6. Collage assets come from one library
**Rule:** Torn edges, tape strips, arrows, and circles come from one fixed asset set with a consistent treatment; scale and rotate them, never redraw ad hoc.
**Why:** One kit keeps the collage coherent across pages; random assets read as clip-art and break the "same photocopier" illusion.
**Example:** The same tape strip asset tops every photo; torn edges always bite from the left, per the kit.

### R7. The photocopy does not animate
**Rule:** Motion is none or ≤100 ms snaps (instant hover states, hard cuts); no fades, no drifts, no parallax.
**Why:** The medium is static paper — animation is a different technology and instantly breaks the illusion the whole style is built on.
**Example:** Hovering a flyer flips its border to black instantly; nothing else moves.

## A11y watchpoints
- Rotated body text is banned — rotation is for display collage only; tilted paragraphs fail low-vision and screen-magnifier users
- Texture noise demands solid text panels; never measure text over grain
- Verify photocopy black `#0D0D0D` on xerox `#EDEDE8` (≈16:1, passes) — the real risk is gray text drift: audit every muted pair at 4.5:1
- Ransom-note images need full alt text carrying the exact headline; never leave `alt=""` on meaningful collage art
- The riot color never signals state alone — pair with text or icon; check riot-color boundaries at 3:1
- Focus: a 2px offset black ring visible on every misaligned or stickered control

## Checklist
- [ ] Ransom-note headlines are SVG/images with full alt text; no interactive ransom text
- [ ] Rotations ≤4°, ≤3 per view, none on body text
- [ ] DOM reading order intact despite visual collage
- [ ] Grain 8–15% behind imagery only; text zones on solid panels
- [ ] Exactly one riot color, used for marker moments
- [ ] Body ≥16px mono/sans, pairs ≥4.5:1 on xerox paper
- [ ] All collage assets from the single kit
- [ ] Motion none or ≤100 ms snaps
- [ ] Targets ≥24×24 despite the sticker-scatter layout

## Anti-patterns
- Ransom-note buttons, links, or navigation — unreadable and unclickable
- Rotated paragraphs "for the vibe"
- Grain over everything, including text zones
- Two or three riot colors — protest becomes party
- A clean corporate grid with one punk font on top (punk-washing)
- Animating the photocopier: jitter loops, drifting tape, parallax flyers
- Confusing with `brutalism`: brutalism keeps the grid intact and anti-decorative; punk-zine is collage chaos with torn edges
- Confusing with acid graphics: xerox analog mess vs digital chrome-rave gradients

## Sources & inspiration
- Punk zine — the DIY press tradition: https://en.wikipedia.org/wiki/Punk_zine
- Xerox art — copier-as-aesthetic canon: https://en.wikipedia.org/wiki/Xerox_art
- Punk subculture — the do-it-yourself ethic: https://en.wikipedia.org/wiki/Punk_subculture
- Ransom note — the cut-out letter tradition: https://en.wikipedia.org/wiki/Ransom_note
- Sniffin' Glue — the definitive early punk fanzine: https://en.wikipedia.org/wiki/Sniffin%27_Glue
