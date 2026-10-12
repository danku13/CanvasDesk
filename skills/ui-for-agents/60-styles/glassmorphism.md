# Style: Glassmorphism

> **Read when:** building UI in the glassmorphism style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/elevation-depth.md, 00/motion-principles.md, 00/visual-hierarchy.md

**Personality:** modern, airy, layered, futuristic-calm. **Best for:** media players, weather/environment apps, brand moments, overlays on rich imagery. **Avoid for:** data-dense tools, long-session apps, text-heavy reading surfaces.

**Core idea:** glass is a depth system, not a filter — a designed scene sits behind translucent panels, and every piece of text earns its legibility with a scrim or solid zone, never from the blur itself.

## Signature (what makes it recognizable)
- Frosted translucent panels: backdrop blur 10–30 px with white/black alpha fills
- A vivid, colorful scene — gradient or art-directed imagery — behind everything
- 1px light-alpha borders tracing exactly where the blur ends
- Layered depth: scene → panel → elevated panel as alpha steps; panels visibly float
- Rounded corners 12–20 px; floating pill controls
- Content anchored on solid or scrimmed zones inside panels
- Calm short-motion panels over a slowly drifting scene

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `color-bg` | the scene — designed, part of the system | 2–3 stop vivid gradient or art-directed photo |
| `color-surface` | translucent fill, never opaque | white 40–70% alpha (light) / black 45–65% (dark) |
| `blur` | ONE intensity token for all panels | 10–30 px backdrop blur |
| `color-border` | light-alpha edge definition | white 15–25% alpha, 1px |
| `scrim` | solid/gradient zone behind text | black 30–60% alpha |
| `color-text` | opaque ink only | near-white/near-black; 4.5:1 vs worst-case panel point |
| `color-accent` | one saturated hue, readable on panels | 4.5:1 on panel fill for text use |
| `radius-*` | generous | 12–20 px; pills = 999 px |
| `shadow-*` | soft ambient, optional | 0 8–24px black at 10–20%; depth carried by alpha, not shadow |
| `space-scale` | airy | 8pt scale, +1 step |
| `type` | neutral/humanist sans, medium weights | scale ratio ~1.25 |
| `motion-duration` | panels slide/fade; scene drifts | 200–300 ms ease-out; scene loops ≥ 60 s |

## Rules

### R1. Blur is background; content is opaque-critical
**Rule:** Text, icons, and controls sit on solid fills or scrimmed zones inside the panel; blur alone never carries legibility.
**Why:** The scene varies pixel to pixel and over time — 4.5:1 must hold against its worst point, which no blur amount can guarantee.
**Example:** Weather card: scene visible at the top, but the temperature block sits on a black 40% scrim band inside the glass.

### R2. The scene is part of the system — design it
**Rule:** Specify the scene like a token: gradient stops, hue relationship to the accent, imagery art direction; never a stock photo dropped behind divs.
**Why:** The scene defines half the contrast and all of the mood; an undesigned scene makes every panel an illegibility lottery.
**Example:** Defined scene = 3-stop gradient #1B2A6B → #7A3E9D → #E4559A at 120°, accent #FFD166 verified on all panel fills.

### R3. Panel hierarchy via alpha steps, never blur amounts
**Rule:** Distinguish panel levels by fill alpha (e.g. 40% → 55% → 70%) and border brightness; blur intensity stays identical everywhere.
**Why:** Varying blur fakes depth but reads as error, fragments the material story, and multiplies GPU layers; alpha steps keep one material, many levels.
**Example:** Base card white 45%, elevated popover white 60% — both blur 20px, border white 20%.

### R4. Borders define where the blur ends
**Rule:** Every translucent panel gets a 1px light-alpha border (white 15–25%); a borderless glass edge dissolves into the scene.
**Why:** The hairline is what makes the material read as a crisp object rather than a smudge, and it supplies the 3:1 UI boundary.
**Example:** Media player card: blur 20px, fill black 50%, border white 22% — edge crisp on any scene.

### R5. One blur token — blur soup kills perf and coherence
**Rule:** Ship exactly one blur value; if a surface cannot read, fix its fill alpha or scrim, never add another blur level.
**Why:** Each distinct blur is a separate GPU surface — multiple intensities tank low-end devices and break the single-material promise.
**Example:** Audit finds blur 14 / 18 / 24 px in the build → all collapsed to the single `blur` token at 20 px.

### R6. Fallback when backdrop-filter is unsupported
**Rule:** Feature-detect backdrop-filter (or the platform compositing equivalent); the fallback is a solid alpha surface at the high end of the range (70–85%) with the same border — never a broken transparent box.
**Why:** Without blur, low-alpha fills leak the scene into text; the raised-alpha fallback preserves both legibility and the material feel.
**Example:** In the Rust GUI build with no blur compositing, panels render white at 82% alpha — same radius, same border.

### R7. Panels animate 200–300 ms; the scene drifts slowly
**Rule:** Panels slide/fade at 200–300 ms ease-out; scene motion (if any) is a ≥ 60 s ambient loop that pauses behind reading zones and honors reduced-motion.
**Why:** Two fast layers destroy comprehension; the calm-panel/slow-scene split is the style's motion personality.
**Example:** Player expands 250 ms; gradient hue shifts over a 90 s loop, frozen while lyrics are open.

### R8. Density stays low — two to three panels per view
**Rule:** Glass surfaces host sparse content: 2–3 panel layers, generous padding, one idea per panel; if density grows, drop to opaque surfaces or switch style.
**Why:** Translucency taxes reading; stacked glass over busy scenes fatigues within minutes and violates the long-session rule in style-catalog.md.
**Example:** Weather app: one hero panel, one forecast strip, one pill control — never a glass data table.

### R9. Floating controls are pills
**Rule:** Buttons and controls render as fully-rounded pills with panel fill + border; pill geometry is reserved for interactive elements so affordance and touch targets stay obvious.
**Why:** The pill is the style's tactile signature and keeps actions findable on top of an ambiguous translucent material.
**Example:** 44 px-tall play pill: white 60% fill, white 20% border, blur inherited, icon + label inside.

## A11y watchpoints
- Text over a varied scene: verify 4.5:1 against the WORST-CASE background point under the text zone — scrim or solid fill is mandatory; blur is not contrast
- Never place body-text panels over animated scene regions; freeze or scrim the scene behind reading areas
- Windows High Contrast / forced-colors: transparency is stripped — provide an opaque forced-colors variant with strong borders
- Blur is expensive on low-end GPUs: provide a reduced-effects mode (solid surfaces, no blur) and honor prefers-reduced-transparency
- Focus indicators must survive translucency: 2px outline on a solid ring or offset so it holds 3:1 against both panel and scene

## Checklist
- [ ] Exactly one blur token (10–30 px) across the product
- [ ] All text on solid or scrimmed zones; worst-case 4.5:1 verified
- [ ] Panel hierarchy = alpha steps; blur never varies
- [ ] 1px light-alpha borders on every translucent edge
- [ ] Scene specified as tokens/direction (stops, hues, drift), not ad-hoc stock
- [ ] No-backdrop-filter fallback renders a solid alpha surface
- [ ] Reduced-effects + forced-colors variants defined and tested
- [ ] Panels 200–300 ms; scene drift ≥ 60 s and pausable
- [ ] ≤ 3 panel layers per view; no glass data tables
- [ ] Pills only for interactive controls; targets ≥ 24×24 px

## Anti-patterns
- Text floating directly on raw blurred imagery with no scrim
- Three blur intensities "for richer depth"
- Glass panel on glass panel on glass panel (panel stack)
- Undesigned stock-photo scene fighting the accent hue
- Fast scene animation behind live text
- Glass forms, tables, or dashboards for 8-hour sessions
- Opaque panels with a blur token applied but no alpha (fake glass)
- White fill + white border + white text over a bright scene (washed out)

## Sources & inspiration
- Apple HIG — materials (translucency done systemically): https://developer.apple.com/design/human-interface-guidelines/materials
- Microsoft Fluent — acrylic material guidance: https://learn.microsoft.com/en-us/windows/apps/design/style/acrylic
- Michał Malewicz — Glassmorphism in user interfaces: https://uxdesign.cc/glassmorphism-in-user-interfaces-1f39bb80806c
- MDN — backdrop-filter support and fallback: https://developer.mozilla.org/en-US/docs/Web/CSS/backdrop-filter
- MDN — prefers-reduced-transparency: https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-transparency
- WCAG 2.2 — contrast minimum (1.4.3): https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html
