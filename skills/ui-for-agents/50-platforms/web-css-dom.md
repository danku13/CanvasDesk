# Web — CSS & DOM

> **Read when:** building or styling any browser-rendered UI with CSS (plain, utility-first, or CSS-in-JS). | **Section:** 50-platforms | **Related:** 10/design-tokens.md, 00/spacing-layout-grids.md, 00/motion-principles.md, 20/forms-and-inputs.md, 40/accessibility-wcag.md, 40/responsive-adaptive.md

**Core idea:** the browser already implements half the quality bar — theming via the cascade, mirroring via logical properties, a11y via semantics, user-preference respect via media queries. Use the platform before rebuilding it in JS.

## Rules

### R1. Ship design tokens as CSS custom properties
**Rule:** Define the token schema once as custom properties on `:root`; re-map values per theme in exactly one place (a `[data-theme]` block or a `prefers-color-scheme` query) — never scatter hex/px literals in component CSS.
**Why:** Custom properties cascade and inherit, so a root re-map propagates to every component without touching component code; theming becomes a data change instead of a refactor, and dark mode (see 10-design-system/theming-dark-mode.md) needs no JS.
**Example:**
`:root { --surface: #fff; --ink: #111; }` + `[data-theme="dark"] { --surface: #111; --ink: #eee; }` — components read `var(--surface)` and never learn a theme switch happened.

### R2. Use logical properties, not left/right/top/bottom
**Rule:** Write `margin-inline`, `padding-block`, `inset-inline-start`, `border-inline-end` instead of physical properties for anything directional.
**Why:** Logical properties reference text flow, not the screen, so they auto-mirror under `dir="rtl"`; physical properties hard-code a direction and resurface as an RTL bug in every mirrored locale.
**Example:** `.chip { margin-inline-start: var(--space-2); }` flips sides automatically in Arabic; `margin-left: 8px` does not.

### R3. Replace JS with modern selectors where possible
**Rule:** Prefer `:has()`, container queries, `:focus-visible`, and `:user-invalid` over JavaScript that toggles classes for the same conditions.
**Why:** These run in the browser's style engine: no hydration cost, no listener bookkeeping, no drift between JS state and painted state — and container queries make components respond to their own box, not the viewport.
**Example:** `.card:has(img) { padding-block: 0; }`, `.field:user-invalid { border-color: var(--danger); }`, `.panel:focus-within { outline: 2px solid var(--focus); }` — zero script.

### R4. Respect user preferences via media queries
**Rule:** Gate all animation behind `prefers-reduced-motion` (durations stay in the 150–300 ms band when motion does play) and provide a dark palette under `prefers-color-scheme` unless the app ships an explicit theme switch.
**Why:** These queries report the OS-level user setting, so honoring them is free and complete; ignoring them means fighting the user's own configuration and breaks for vestibular-sensitive users.
**Example:** `@media (prefers-reduced-motion: reduce) { * { animation-duration: 0.01ms; transition-duration: 0.01ms; } }`

### R5. Keep specificity low; order the cascade with layers
**Rule:** Cap selectors at two levels (class + one state), ban ID selectors and `!important` from component CSS, and structure styles in `@layer` tiers (base, components, utilities) so precedence is decided by declared order.
**Why:** High specificity is compounding debt — every later override must be stronger until everything is `!important`; layers add a global, predictable precedence axis that overrides cannot accidentally break.
**Example:** `@layer components { .btn.is-active { … } }` is beaten by `@layer utilities { .bg-surface { … } }` purely by layer order — no specificity war.

### R6. Native form semantics first
**Rule:** Use real `<label for>`, `fieldset`/`legend`, `type`, `required`, `pattern`, and `autocomplete` attributes before reaching for ARIA or custom-drawn controls.
**Why:** The browser derives the accessibility tree, autofill, mobile keyboards, and validation from semantics for free; a `div` with a click handler and `aria-label` reimplements all of it, worse, and placeholder-only inputs fail labeling rules outright.
**Example:** `<label for="email">Email</label><input id="email" type="email" autocomplete="email" required>` — name, focus, autofill, and keyboard work with no script.

### R7. Control stacking with a z-index token scale; know your stacking contexts
**Rule:** Take z-index values only from a token scale (e.g. `--z-dropdown: 100`, `--z-sticky: 200`, `--z-overlay: 300`, `--z-modal: 400`, `--z-toast: 500`) and remember that `transform`, `filter`, `opacity < 1`, and `will-change` create stacking contexts that trap descendants.
**Why:** A stacking context isolates its children — a modal's `--z-modal` cannot escape a parent with a transform, which is what most "z-index doesn't work" bugs actually are; a scale keeps layers legible across the codebase.
**Example:** A modal inside `.card { transform: translateY(0) }` is stuck under later siblings: remove the transform or portal the modal to `<body>`.

### R8. Media queries: width ranges + preference queries, never devices
**Rule:** Write width queries as ranges (`(width >= 480px)`, `(480px <= width < 960px)`) around content-derived breakpoints and combine them with preference queries; never target device names or specific models.
**Why:** Device-based queries rot as hardware churns and split users into wrong buckets (new phones are wider than old tablets); content ranges survive new screens and document where each layout change happens.
**Example:** `@media (width >= 960px) and (prefers-color-scheme: dark) { … }` — no `@media ipad` ever exists.

### R9. Reserve media space with aspect-ratio + object-fit
**Rule:** Give every image and video a declared box: `width: 100%`, an `aspect-ratio` (or `width`/`height` attributes), and `object-fit: cover` or `contain`.
**Why:** Layout shift happens because the browser cannot size the box before bytes arrive; a declared ratio lets it reserve exact space, protecting CLS, scroll position, and perceived performance (see 40-quality/perceived-performance.md).
**Example:** `img { width: 100%; aspect-ratio: 16 / 9; object-fit: cover; }` — nothing jumps when the network is slow.

### R10. Units by job: px for detail, rem for scale, fluid units for layout
**Rule:** Use `px` for borders, shadows, and hairlines; `rem` for type sizes and the spacing scale; `%` / `fr` / `minmax()` for layout boxes; never `px` for font sizes.
**Why:** `rem` type and spacing scale with the user's root font-size preference (an accessibility setting), `px` borders stay crisp at any zoom, and layout units adapt to the container instead of fighting it.
**Example:** `border: 1px solid var(--border); padding: var(--space-3); font-size: 1rem; grid-template-columns: 240px minmax(0, 1fr);`

### R11. Contained scroll areas; never hijack page scroll
**Rule:** Long lists and tables scroll inside their own container (`overflow: auto` + a max-height) while the page scrolls normally; never intercept wheel/touch events to drive custom scroll, snap, or parallax.
**Why:** Hijacking breaks platform behaviors users depend on — browser gestures, momentum, find-in-page, URL anchors, scroll restoration — and re-implementing them is always worse; contained areas keep everything intact.
**Example:** A 20-row table pane scrolls independently above a sticky form footer; the page itself never scrolls horizontally.

### R12. The platform floor survives CSS abstraction
**Rule:** Interactive elements get ≥ 24×24 px hit areas (44×44 recommended for touch) and text keeps ≥ 4.5:1 contrast (≥ 3:1 for large text and UI components) regardless of theme or component framework.
**Why:** CSS makes violating the floors easy — tight line-heights, low-opacity grays, padding-less ghost buttons — and the cascade will happily render an inaccessible control that looks fine in a screenshot.
**Example:** A 20 px-tall ghost button fails the floor: raise padding to reach 24 px height instead of shrinking the label further.

### R13. Gate new CSS behind `@supports` with a usable fallback
**Rule:** Before relying on a bleeding-edge property or selector, wrap it in `@supports` and define the fallback the page degrades to; the fallback must remain usable, not just non-broken.
**Why:** Browsers roll features asynchronously — feature detection keeps the modern path available where it exists and honest where it does not, instead of a silent broken layout in the lagging browser.
**Example:** `@supports (container-type: inline-size) { .card { container-type: inline-size; } }` — older browsers keep the viewport-query layout, which still passes the checklist.

## Checklist
- [ ] All colors, sizes, durations read from custom properties; no hex/px literals in component CSS
- [ ] Directional spacing uses logical properties only (`grep margin-left` → 0 hits)
- [ ] `prefers-reduced-motion` handled; `prefers-color-scheme` or an explicit theme switch present
- [ ] No ID selectors, no `!important` in component layers; layers ordered base → components → utilities
- [ ] Every form control has a real `<label for>`; `autocomplete` set for identity/payment fields
- [ ] z-index values come from the token scale; no positioned element inside transformed parents
- [ ] Every image/video declares aspect-ratio or width/height attributes
- [ ] Font sizes in rem; borders/shadows in px; layout in %/fr/minmax
- [ ] Every interactive element ≥ 24×24 px; body text contrast ≥ 4.5:1 in both themes
- [ ] No scroll hijacking; long data scrolls in contained `overflow: auto` areas

## Anti-patterns
- `!important` chains and `#id .a .b` selectors to win the cascade
- JS toggling classes that `:has()` / `:user-invalid` / container queries would handle
- `margin-left` / `padding-right` in components that must mirror for RTL
- Fixed pixel heights on text containers (clips at the user's font scale)
- `overflow: hidden` on `body` plus a custom wheel-jacked scroller
- `z-index: 9999` as a bug fix for a stacking-context trap
- Removing focus outlines without a `:focus-visible` replacement
- Pixel font sizes (`font-size: 14px`) that ignore the user's root scale

## Sources
- MDN — CSS logical properties: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_logical_properties_and_values
- MDN — :has(), container queries, :focus-visible: https://developer.mozilla.org/en-US/docs/Web/CSS/:has , https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_container_queries , https://developer.mozilla.org/en-US/docs/Web/CSS/:focus-visible
- MDN — autocomplete values: https://developer.mozilla.org/en-US/docs/Web/HTML/Attributes/autocomplete
- web.dev — prefers-reduced-motion: https://web.dev/articles/prefers-reduced-motion
- web.dev — avoid large layout shifts (aspect-ratio): https://web.dev/articles/optimize-cls
- MDN — prefers-color-scheme: https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-color-scheme
- WCAG 2.2 — contrast & target size: https://www.w3.org/TR/WCAG22/
