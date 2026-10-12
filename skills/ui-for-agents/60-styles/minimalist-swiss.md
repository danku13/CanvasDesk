# Style: Minimalist Swiss

> **Read when:** building UI in the minimalist-swiss style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/visual-hierarchy.md, 00/spacing-layout-grids.md, 00/color.md

**Personality:** precise, calm, systematic, confident. **Best for:** SaaS, dev tools, documentation, fintech, productivity. **Avoid for:** kids products, entertainment, emotional lifestyle brands.

This is the library's baseline style — the neutral against which every other preset is defined.

## Signature (what makes it recognizable)
- Flat surfaces with hairline borders; shadows near-zero
- Left-aligned grid discipline; generous whitespace
- Neutral grays carry the UI; exactly one accent hue (budget ≤ 10% of any view)
- Neutral sans (Inter-like), clear modular scale, sentence case
- Functional motion only: 150–250 ms, ease-out, no bounce
- Photography rare; when present — clean, unsaturated, grid-aligned

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | sharp to small | 2–6 px |
| `shadow-*` | minimal | none, or one 0/2/8px layer at 6–10% alpha |
| `space-scale` | airy | 8pt scale; +1 step versus dense presets |
| `color-bg / surface` | neutral near-white / near-black | #FAFAFA / #111 range |
| `color-accent` | one medium-saturation hue | works at 4.5:1 on bg in both themes |
| `type-scale-ratio` | moderate | ~1.25; 2 weights max |
| `motion-duration` | fast functional | 150–250 ms, ease-out |

## Rules

### R1. Neutrals carry the UI, accent marks the moments
**Rule:** Gray/ink neutrals build 90%+ of the surface; the accent appears only on the primary action, active states, and critical signals.
**Why:** The accent works by scarcity; when everything is highlighted, the primary path is lost and the style collapses into generic colorfulness.
**Example:** Invoice list: all-neutral table, accent only on "New invoice" and the overdue badge.

### R2. Hairline borders before shadows
**Rule:** Separate and structure with 1px borders in a light-contrast neutral; reserve shadows for true overlays (dropdowns, modals).
**Why:** Borders are crisp and theme-stable; shadows on flat design drift toward mud in dark mode and blur the systematic feel.
**Example:** Cards defined by border + slightly offset surface tone, no drop shadow at rest.

### R3. Whitespace is the luxury signal
**Rule:** When in doubt, add space — section padding at least 2× component padding, content max-widths enforced (text 65–75ch).
**Why:** In the absence of decoration, density is the only remaining failure mode; air reads as confidence and order.
**Example:** A settings page with 48px section gaps and no dividers still scans perfectly.

### R4. Typography does the talking
**Rule:** Build hierarchy strictly from the type scale, weight, and ink levels — no decorative fonts, no color-coded headings.
**Why:** The style's identity lives in systematic type; adding decorative elements breaks the "machine precision" promise.
**Example:** Page title 24/600, section 18/600, body 16/400 muted — identical structure on every page.

### R5. Motion is felt, not seen
**Rule:** Animate state changes at 150–250 ms with ease-out; no bounce, no parallax, no decorative loops.
**Why:** Motion must confirm causality and disappear; anything slower or showier turns the calm system into a performance.
**Example:** Dropdown opens 180ms ease-out with 4px fade-shift; that is the entire motion vocabulary.

### R6. States change by tokens, not by redesign
**Rule:** Hover/focus/active/disabled are small token shifts (background step, border accent, opacity) — same geometry.
**Why:** Geometry stability is what makes the interface feel machine-precise; jumping shapes on hover read as amateur.
**Example:** Button hover = background shifts one ramp step + border darkens; radius and height unchanged.

### R7. Alignment axes are sacred
**Rule:** Every section shares the same vertical axis for labels, content, and actions; columns align across the whole product.
**Why:** Cross-screen alignment is the signature of systematic design — users feel the grid even when they cannot name it.
**Example:** Form labels, table left edges, and card padding all start on the same x-position.

### R8. Restraint is the statement
**Rule:** Any element that does not inform is removed — icons without function, badges without data, gradient without meaning.
**Why:** The style's credibility comes from every pixel having a job; one gratuitous decoration licenses visual chaos everywhere.
**Example:** Toolbar keeps 4 icons that earn their place instead of 8 decorative ones.

## A11y watchpoints
- Muted-gray-on-white is the style's classic failure: verify every muted text pair at 4.5:1 (both themes)
- Focus rings must stay visible on flat surfaces — 2px accent outline, 3:1 against adjacent colors
- Hairline borders at 3:1 minimum when they are the only control boundary
- Whitespace-heavy layouts: do not drop below 24px targets while "keeping it airy"

## Checklist
- [ ] Accent covers ≤ ~10% of every view
- [ ] Resting UI uses borders, shadows only for overlays
- [ ] Section spacing ≥ 2× component spacing; content max-widths hold
- [ ] Hierarchy built from type scale alone; no decorative fonts
- [ ] All motion 150–250 ms ease-out, no bounce
- [ ] States are token shifts with stable geometry
- [ ] Alignment axes consistent across screens
- [ ] Every element passes the "what is its job" question

## Anti-patterns
- Gradient buttons and glowing effects ("minimalism with extra steps")
- Three accent colors "for variety"
- Shadows on every card at rest
- Bounce animations on a calm system
- Dense gray-on-gray text below contrast floors
- Decorative icons and badges with no data

## Sources & inspiration
- International Typographic Style (Swiss) — grid & type foundations: https://en.wikipedia.org/wiki/International_Typographic_Style
- Linear — product reference for the style: https://linear.app
- Vercel design language: https://vercel.com/design
- Refactoring UI — restraint and hierarchy: https://www.refactoringui.com
- Every Layout — systematic layout primitives: https://every-layout.dev
