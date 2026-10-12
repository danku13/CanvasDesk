# Style: Brutalism

> **Read when:** building UI in the brutalism style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/typography.md, 00/color.md, 00/visual-hierarchy.md, 00/spacing-layout-grids.md

**Personality:** raw, honest, loud, anti-corporate. **Best for:** portfolios, agencies, culture/music, editorial experiments, brands contrasting with polished corporate web. **Avoid for:** fintech, healthcare, enterprise tools — anywhere trust is built through polish.

**Core idea:** honesty is the aesthetic — elements look exactly like what they are: buttons are blocks, links are underlined, structure is shown, not painted over.

## Signature (what makes it recognizable)
- Visible structure: 2–4px solid black borders on interactive elements and containers
- Hard offset shadows, zero blur: 4–8px solid
- Radius 0 — nothing rounded, anywhere
- System/mono/grotesque display type at oversized scale; plain readable body
- Max two loud hues + black/white, clashing on purpose (or pure black/white)
- Default underlined links — default blue is acceptable and on-brand
- Motion snaps: 0–100 ms or none; hover = hard state flips

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `border-width` | thick, always visible | 2–4 px solid #000 |
| `shadow-*` | hard solid offset, zero blur | 4px 4px 0 #000 (active: translate by the offset) |
| `radius-*` | zero | 0 everywhere |
| `color-bg` | stark | #FFFFFF or #0A0A0A, or one loud hue field |
| `color-surface` | flat blocks and inversions | inverse pair (black block on white, white on black) |
| `color-accent` | loud, clashing on purpose | max 2 hues (e.g. #FF4400 + #0000EE) + black/white |
| `color-text` | ink on stark | black/white pairs; loud-on-loud only if ≥ 4.5:1 |
| `font-display` | mono or grotesque, oversized | 1.5–2× normal display scale; uppercase allowed |
| `font-body` | plain system sans/serif | 16 px-equivalent, 45–75ch measure |
| `type-scale-ratio` | steeper than baseline | ~1.333 — display jumps, body stays put |
| `space-scale` | conventional grid | 8pt scale; the grid is honest, not broken |
| `motion-duration` | snaps | 0–100 ms or none; no easing curves |

## Rules

### R1. Honesty is the aesthetic
**Rule:** Elements render as what they are — buttons look like pressable blocks, links underlined, headings like headings; decoration never disguises function.
**Why:** The style's entire promise is anti-painted-UI; the moment a block stops announcing its function, you have pastiche, not brutalism.
**Example:** A "Buy ticket" button: bordered block, hard shadow, uppercase label — no icon, no gradient, no rounding.

### R2. Semantic elements first, rawness in the styling
**Rule:** Build from real headings, links, buttons, lists; the raw look comes from tokens, not from div-soup pretending to be markup.
**Why:** Anti-corporate honesty includes machine honesty — semantic structure keeps the style accessible and maintainable on any platform.
**Example:** The nav is a real list of underlined links, set in 20px mono with 2px hover borders.

### R3. The grid holds — except one deliberate break per view
**Rule:** Layout sits on the conventional grid; break it exactly once per view, on purpose, at the point you want remembered.
**Why:** One broken thing is a statement; five broken things are a mess — and a mess is what brutalism's critics already expect.
**Example:** Editorial page: strict 12-column grid everywhere, except the manifesto headline rotated −2° bleeding off-grid.

### R4. Hover/active = hard flips, pressing INTO the shadow
**Rule:** Hover inverts colors (bg ↔ fg) or flips the shadow side; active translates the element by the shadow offset so it presses into its own shadow.
**Why:** Binary hard states replace easing as the style's feedback language and keep affordance loud without motion.
**Example:** Button hover: white→black bg, black→white text; mousedown: translate(4px, 4px), shadow removed.

### R5. Oversized display type as image; body stays boring
**Rule:** Display headlines may dominate the viewport (mono/grotesque, uppercase, 1.5–2× scale); body text stays plain — 16 px-equivalent, 45–75ch, normal case.
**Why:** The loudness budget is spent on display only; brutalist sites that set body copy in tiny mono uppercase are just hard to read, not radical.
**Example:** Hero: "LOUD & HONEST" at 120px; the paragraph under it: 17px system sans, black on white.

### R6. Palette budget: two loud hues + black/white
**Rule:** Max two saturated hues plus black/white; they may clash deliberately on surfaces, but every text pair must still measure ≥ 4.5:1.
**Why:** The clashing pair IS the identity; a third hue dissolves intent into chaos, and failing pairs turn the manifesto into an a11y violation.
**Example:** #FF4400 blocks + #0000EE links on white with black text — verified pairs only.

### R7. Affordances stay unambiguous despite the rawness
**Rule:** Thick borders + hard hover flips carry interactivity; never rely on hover alone — focus, disabled, and current states all get distinct treatments.
**Why:** Rawness reduces the visual vocabulary, so the remaining signals must be louder, not subtler.
**Example:** Current nav item: inverted block; hover: shadow appears; keyboard focus: 3px outline; disabled: hatched gray fill.

### R8. Display type must reflow
**Rule:** Oversized type scales fluidly (viewport-relative sizing with sensible clamps in CSS; measured minimums in native/canvas renderers) and never forces horizontal scrolling down to 320 px-equivalent width (WCAG 1.4.10).
**Why:** The style's most common real-world failure is a 120px headline that breaks mobile reflow — loud is fine, unusable is not.
**Example:** Hero headline sized clamp(48px, 12vw, 128px) — wraps to three lines at 360px, no overflow.

### R9. Defaults are allowed — and on-brand
**Rule:** Use platform defaults where they are honest: default-blue underlined links, visible system focus outlines, native form controls with bordered treatment.
**Why:** Brutalism critiques over-designed UI; unstyled honesty (blue link, real checkbox) is a feature, and defaults carry built-in accessibility.
**Example:** Legal page: default blue underlined links in 17px sans — perfectly on-style.

## A11y watchpoints
- Loud-hue-on-loud-hue text pairs fail 4.5:1 easily (red-on-green classics) — verify every pair; "clashing" is for blocks, never for text/background
- Hover-invert must not hide focus: keyboard focus keeps a dedicated ≥ 3:1 outline distinct from the inverted hover state
- Oversized type must not break reflow (WCAG 1.4.10): test at 320 px-equivalent and 400% zoom — no horizontal scrolling
- Rawness tempts skipped states: focus, disabled, and current must remain visually distinct without relying on subtle color deltas
- Reduced motion is trivially satisfied (the style barely animates) — keep it that way; no blinking/flashing decoration (2.3.1)

## Checklist
- [ ] Borders 2–4px solid; radius 0 everywhere
- [ ] Hard offset shadows, zero blur; active state presses into the shadow
- [ ] Max 2 loud hues + black/white; no gradients, no soft shadows
- [ ] Every text pair ≥ 4.5:1, including loud-on-loud experiments
- [ ] Focus outline visible, ≥ 3:1, distinct from hover inversion
- [ ] Display type reflows without horizontal scroll (1.4.10)
- [ ] Body text plain: 16 px-equivalent, 45–75ch, 1.4–1.6 line-height
- [ ] Links recognizable (underlined), native controls where honest
- [ ] Motion 0–100 ms or none; no added flashing decoration
- [ ] At most one deliberate grid break per view

## Anti-patterns
- Rounded corners + soft shadows ("friendly brutalism" = bold minimalism, not this)
- Five broken-grid moments per view — mess, not statement
- Loud hue text on loud hue background below 4.5:1
- Hover-only affordances with no focus/disabled story
- Div-soup styled to look raw but semantically empty
- Neon gradients and glow effects (that is futuristic-neon)
- Body copy in 12px uppercase mono "for the vibe"
- Brutalism on trust surfaces: bank checkout, hospital intake, enterprise admin

## Sources & inspiration
- Brutalist Websites — the canonical archive: https://www.brutalistwebsites.com
- Neubrutalism (web design) — the modernized derivative and its drift: https://en.wikipedia.org/wiki/Neubrutalism_(web_design)
- Brutalist architecture — the movement whose ethics the style borrows: https://en.wikipedia.org/wiki/Brutalist_architecture
- WCAG 2.2 — reflow (1.4.10): https://www.w3.org/WAI/WCAG22/Understanding/reflow.html
- WCAG 2.2 — contrast minimum (1.4.3): https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
- Gumroad — production neubrutalist reference: https://gumroad.com
