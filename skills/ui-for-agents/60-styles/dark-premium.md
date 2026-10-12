# Style: Dark Premium

> **Read when:** building UI in the dark-premium style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/elevation-depth.md, 00/color.md, 00/typography.md, 00/motion-principles.md

**Personality:** refined, exclusive, dramatic, serious. **Best for:** luxury, AI/hardware products, media, finance apps. **Avoid for:** kids products, healthcare forms, government services.

**Core idea:** darkness is layered, not empty — near-black surface steps, one metallic accent used like jewelry, and slow confident motion turn "dark mode" into "night boutique".

## Signature (what makes it recognizable)
- Layered near-black surfaces: bg → surface-1 → surface-2 as distinct lighter steps; never flat #000
- Desaturated gray text ramp; off-white ink, never pure white
- Exactly ONE metallic (gold/champagne) or jewel accent; budget < 5% of any view
- High-contrast serif or display face for headlines and numerals; clean sans for body
- Depth from surface lightness + 1px light-alpha hairlines (gold hairline dividers), not heavy shadows
- Subtle glow reserved for interactive and focus elements only
- Dark-adapted imagery: darkened, desaturated, or duotone so photos sit inside the scene

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `color-bg` | near-black, never #000 | #0A0A0C |
| `color-surface-1 / -2` | lighter steps; elevation = lightness | #131316 / #1C1C21 |
| `color-border` | light-alpha hairlines | white at 8–14% alpha |
| `color-text` | desaturated off-white / muted grays | #EDEDEF / #A0A0A8 (verify each pair 4.5:1) |
| `color-accent` | one metallic or jewel hue | champagne gold ~#D4B872 + lightened text-safe variant |
| `radius-*` | small–medium, crisp | 4–10 px |
| `shadow-*` | soft, low-alpha, overlays only | 0 8px 24px black at 30–40% |
| `glow` | subtle, interactive/focus only | 0 0 12px accent at 20–35% alpha |
| `space-scale` | generous | 8pt scale; +1 step versus dense presets |
| `font-display` | high-contrast serif or display grotesque | headlines, numerals, KPI figures |
| `font-body` | clean neutral sans | body, forms, tables |
| `type-scale-ratio` | moderate | ~1.25; 2–3 weights max |
| `motion-duration` | slow-smooth | 250–350 ms, ease-out, no bounce |

## Rules

### R1. Layer surfaces, never pure black
**Rule:** Build every screen from 3–4 near-black surface steps (#0A0A0C → #131316 → #1C1C21); express elevation as lightness steps per 00/elevation-depth.md, never as gray shadow piles.
**Why:** Pure #000 kills the layer cues that make depth readable and makes borders and imagery look pasted on; lightness steps read as crafted, black holes read as unfinished.
**Example:** Page bg #0A0A0C, card #131316, hovered card #1C1C21, modal #1C1C21 + border white 14%.

### R2. Accent is jewelry — under 5%
**Rule:** The metallic/jewel accent appears only on the primary action, active states, key numerals, and hairline dividers — under 5% of any view's area; everything else stays in the dark neutral family.
**Why:** Preciousness is a scarcity effect; gold everywhere reads as casino, gold sprinkled reads as couture.
**Example:** Wealth dashboard: all-neutral chart; gold only on the total figure, the "Invest" button, and a 1px divider under the header.

### R3. Serif display against clean sans body
**Rule:** Headlines and numerals use a high-contrast serif or display face; body, labels, and forms use a neutral sans — the tension between the two IS the typographic identity.
**Why:** Type contrast delivers the luxury cue that color cannot; a one-family dark UI reads as generic dark mode.
**Example:** KPI card: value "€ 2.4 M" in serif display 32/400, label "Portfolio value" in sans 13/500 muted.

### R4. Depth = surface steps + hairline borders
**Rule:** Separate layers with 1px light-alpha borders and surface steps; reserve soft shadows for true overlays (modals, popovers) and keep them low-alpha.
**Why:** Heavy shadows designed for light themes turn to gray mush on near-black; hairlines stay crisp at every DPI and in every renderer, from CSS to Rust-native to canvas.
**Example:** Card = #131316 + border white 10%; only the command palette gets a drop shadow.

### R5. Glow is an interaction signal, not decoration
**Rule:** Use the subtle accent glow only on hover, focus, and active interactive elements; nothing glows at rest.
**Why:** Glow at rest inflates into neon pastiche and competes with real signals; as an interaction cue it reads as "this responds to you" — quiet and premium.
**Example:** Primary button gains the glow token on hover and a 2px accent outline on keyboard focus; headlines never glow.

### R6. Imagery must be dark-adapted
**Rule:** Darken (brightness ~0.8–0.9), desaturate (~0.7–0.85), or duotone every photo and illustration before it ships; logos get a light-mono variant.
**Why:** Bright stock imagery is the loudest light-theme artifact — it shatters the scene and drags attention away from the accent.
**Example:** Hero photo at brightness 0.85 with a gradient scrim falling to #0A0A0C at the text edge.

### R7. Motion is slow and smooth — speed feels cheap here
**Rule:** Animate at 250–350 ms with ease-out, no bounce, no spring; fades and gentle translates only.
**Why:** Snappy 100 ms motion reads utilitarian; the deliberate pace is part of the luxury signal — but it stays under 350 ms so it never blocks task flow.
**Example:** Menu opens 300 ms ease-out fade + 8px rise; hover transitions 250 ms.

### R8. Craft the emptiness
**Rule:** Empty states, loaders, and 404s are designed compositions — display-serif statement, muted illustration or fine line art, one quiet action; never a bare void or a light-theme spinner pasted in.
**Why:** In a dark scene emptiness is highly visible; unstyled voids read as broken, crafted ones read as intentional exclusivity.
**Example:** Empty watchlist: serif "Nothing here yet", thin gold line art, "Add an asset" ghost button.

### R9. Two accent values: decorative and text-safe
**Rule:** Maintain a rich decorative accent (fills, dividers, large display) and a lightened text-safe variant that passes 4.5:1 on bg for small accent-colored text; verify both pairs independently on every surface step.
**Why:** Rich metallics usually fail 4.5:1 as small text on near-black; one unverified gold value is this style's most common defect.
**Example:** Gold #D4B872 fills the button; text-safe variant #E8D5A4 carries the "Active" label at 4.5:1+.

## A11y watchpoints
- Muted-gray-on-dark is the classic failure: verify EVERY text pair at 4.5:1 (3:1 large text) against each surface step — check pairs, not hex codes
- Gold/metallic small text on dark usually fails 4.5:1 — use the lightened text-safe variant for labels; keep the rich value for fills and large display
- Glow is not a focus indicator: keep a visible 2px+ outline at 3:1 against adjacent surfaces (WCAG 2.4.7/2.4.13)
- Disabled states must stay legible — target ~3:1; near-invisible gray-on-gray disables are a defect, not a style
- Fine serif hairlines drop below legibility at small sizes: never set body text in the display serif

## Checklist
- [ ] No #000 background, no #FFF text; surfaces built from 3–4 near-black steps
- [ ] Elevation reads as surface lightness + hairlines; shadows only on overlays
- [ ] Accent is one metallic/jewel hue, < 5% of every view
- [ ] Decorative vs text-safe accent split exists; both pairs verified
- [ ] All text pairs ≥ 4.5:1, including muted grays on every surface step
- [ ] Headlines/numerals in display serif; body in sans
- [ ] Glow appears only on hover/focus/active elements
- [ ] All imagery darkened/desaturated/duotoned
- [ ] Motion 250–350 ms ease-out, no bounce
- [ ] Empty states are designed compositions

## Anti-patterns
- Pure #000 background with pure #FFF text ("dark mode, not dark premium")
- Gold text on every label — accent inflation
- Light-theme drop shadows pasted onto dark surfaces (gray mush)
- Neon rainbow accents (that is futuristic-neon, not premium)
- Bright, saturated stock photos dropped into the scene untouched
- 100 ms snappy motion or springy bounces
- Glow on static decoration; pulsing ambient effects
- Gray-on-gray secondary text below 4.5:1 "because it's subtle"

## Sources & inspiration
- Material Design 3 — dark theme, elevation via surface color: https://m3.material.io/foundations
- Apple HIG — dark mode: https://developer.apple.com/design/human-interface-guidelines/dark-mode
- WCAG 2.2 — contrast minimum (1.4.3): https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
- WebAIM contrast checker — verify pairs, not hexes: https://webaim.org/resources/contrastchecker/
- Aesop — luxury e-commerce reference (dark-adapted imagery, restrained accent): https://www.aesop.com
- Linear — dark product UI with layered near-black surfaces: https://linear.app
