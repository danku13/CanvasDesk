# Theming & Dark Mode

> **Read when:** adding a theme, brand variant, or dark mode — or before shipping one. | **Section:** 10-design-system | **Related:** design-tokens.md, ../00-fundamentals/elevation-depth.md, ../00-fundamentals/color.md, ../40-quality/visual-qa-protocol.md

**Core idea:** a theme is a remapping of semantic tokens — zero component changes; that is the point of the token tiers. Dark mode is not inversion: lightness is rebuilt, saturation is re-tuned, and every contrast pair is re-verified.

## Rules

### R1. A theme is a semantic-token remap, nothing else
**Rule:** Implement every theme as a new set of values for the semantic tier; component code, structure, and markup must not change between themes.
**Why:** If a theme requires component edits, theming does not scale past two themes and every new component re-opens the question; the token tiers exist precisely to make a theme a reviewable table of values.
**Example:** `dark = { bg-surface: grey-900, text-primary: grey-50, … }` — a values-only diff; zero component files touched.

### R2. Dark mode is not inversion
**Rule:** Build a dark theme by re-mapping lightness on the ramps — dark surface ramp, light text ramp, re-checked accent — not by swapping white↔black.
**Why:** Naive inversion produces pure-white text on pure-black, which vibrates and halates on emissive displays, and it inverts muted-emphasis relationships so hierarchy reads backwards.
**Example:** Light `#ffffff`/`#111111` becomes dark `#121212`/`#e0e0e0` — never `#000000`/`#ffffff`; muted text stays near 70% lightness, not 30%.

### R3. Desaturate slightly; give the accent a dark-mode variant
**Rule:** In dark themes reduce the saturation of large surfaces and status colors, and provide a lighter brand-accent variant when the light-mode accent cannot hold contrast on dark surfaces.
**Why:** Simultaneous contrast makes saturated color read louder and vibrate on dark; accents that pass 4.5:1 on white often fall far below it on `grey-900`.
**Example:** Brand blue that hits 4.6:1 on white ships as a lightened variant for text/links in dark; large decorative surfaces drop one saturation step.

### R4. Elevation in dark = lighter surface per level, not stronger shadows
**Rule:** Express elevation in dark themes as progressively lighter surfaces per level (surface → surface-1 → surface-2), not by intensifying shadows.
**Why:** Dark shadows on dark backgrounds are invisible, so shadow-based depth cues vanish exactly when layering matters most; surface lightness restores the cue and matches the intuition of a light source above (see ../00-fundamentals/elevation-depth.md).
**Example:** Base `#121212`, card `#1e1e1e`, dialog `#252525`; shadow tokens may remain but must not be the only elevation channel.

### R5. Re-verify every contrast pair per theme
**Rule:** For each theme, re-check all text pairs at ≥ 4.5:1 and large-text/UI-component pairs at ≥ 3:1 — a value that passed in light mode says nothing about dark.
**Why:** Contrast is a property of the pair, not of a color; a theme remap changes one end of every pair, so every check must re-run per theme.
**Example:** CI computes ratios for `text-primary/bg-surface`, `text-muted/bg-surface`, `focus-ring/bg-surface` in both themes; any pair below its floor fails the build.

### R6. Give non-token assets a dark treatment
**Rule:** Provide dark handling for images, illustrations, logos, shadows, and colored overlays — tokens do not flow through bitmaps or baked styles.
**Why:** A white-background illustration on a dark card glows like a lightbox; logos tuned for white lose contrast; scrim opacities computed for white backdrops under- or over-dim on dark.
**Example:** Logo shipped as `logo-light`/`logo-dark`; illustrations have dark-background variants; image overlays use a heavier scrim in dark than in light.

### R7. Respect system preference, allow override, persist the choice
**Rule:** Default to the system color-scheme preference; provide an explicit light/dark/system control; persist the user's manual choice.
**Why:** The system default matches expectation at first run and costs nothing; without a persistent override, users who need a specific mode (low vision, migraine, OLED) re-decide at every launch — and drop the app instead.
**Example:** First launch follows the OS setting; the user picks Dark; the choice is stored and survives restart; System stays selectable.

### R8. Never hardcode white/black — including fallbacks
**Rule:** No literal `white`, `black`, `#fff`, or platform equivalents in component code — not even as default/fallback values.
**Why:** Hardcoded neutrals are the number-one dark-mode defect and survive because they look fine in light mode; a fallback is what renders whenever a token is missing, so every forgotten token becomes a white flash.
**Example:** `background: var(--bg-surface, #fff)` is a defect: the fallback paints white in dark mode. Fail loudly or fall back to a semantic neutral.

### R9. Re-tune borders, focus rings, and shadows for dark
**Rule:** Adjust border/divider colors (usually lighter in dark), verify focus indicators hold ≥ 3:1 against every dark surface they appear on, and confirm shadows still communicate anything.
**Why:** Every background changed, so boundaries tuned at 1px for light either vanish or double in weight; focus rings tuned for white routinely drop below the UI floor on dark surfaces.
**Example:** `border: grey-200` → `border: grey-700`; the focus ring color is re-measured against base, raised, and overlay surfaces.

### R10. Test every screen in both themes before ship
**Rule:** Before shipping any theme-affecting change, render and review every screen in both themes side by side; no screen ships verified in one theme only.
**Why:** Theme defects concentrate where nobody looks — toasts, empty states, badges, focus rings, charts — and each unreviewed screen is an unverified contract (see ../40-quality/visual-qa-protocol.md).
**Example:** The QA pass is a screenshot matrix (screens × light/dark); a new settings page reviewed only in light is a ship-blocker.

### R11. One token vocabulary across themes
**Rule:** Keep token names identical across themes — only values differ; never add `dark-button-bg`-style tokens or per-theme component code.
**Why:** Names-per-theme double the token set and force `if (dark)` branches into components — the exact failure the semantic tier exists to prevent.
**Example:** Components read `bg-surface`; the theme decides whether that is `#ffffff` or `#121212`; no conditional styling exists in component code.

### R12. Apply the theme before first paint
**Rule:** Resolve and apply the theme during startup, before content renders; theme switches apply immediately (at most a ≤ 150 ms fade).
**Why:** A light flash on dark-mode load reads as broken and is worse than no dark mode; long cross-fades make every switch feel sluggish.
**Example:** Theme value read from storage/system before the first frame; switching applies instantly — no white flash, no 500ms transition.

### R13. Status and data-viz colors get dark pairs too
**Rule:** Success/warning/error/info colors and chart series palettes need verified dark-mode pairs, not the light values reused.
**Why:** Status hues that pass on white often fail on dark surfaces, and full-saturation chart palettes vibrate against dark backgrounds — both are also exactly the pairs QA forgets to re-check.
**Example:** Error text on dark uses a lightened red pair; chart series are re-picked for dark with re-verified 3:1 against the plot surface.

## Checklist
- [ ] Theme diff is values-only; zero component or markup changes between themes
- [ ] Dark surfaces from a rebuilt ramp; no pure #000000/#ffffff pairs
- [ ] Accent holds ≥ 4.5:1 (text) / 3:1 (UI) on dark surfaces; dark variant provided if not
- [ ] Elevation expressed as surface-lightness steps in dark
- [ ] All contrast pairs re-verified in every theme (4.5:1 / 3:1)
- [ ] Images, logos, illustrations, and overlays have dark variants or scrims
- [ ] System preference respected + manual override + persisted choice
- [ ] Zero literal white/black values in component code
- [ ] Borders and focus rings re-tuned and re-measured for dark
- [ ] Every screen screenshot-reviewed in both themes

## Anti-patterns
- `filter: invert(1)` dark mode
- Pure white text on pure black
- The same shadow values carrying elevation in both themes
- `var(--x, #fff)` fallbacks scattered through components
- A dark toggle that resets on every reload
- The light accent-on-dark text pair carried over unchanged
- Shipping after reviewing screenshots in light theme only
- `theme-dark-button-bg` token names and `if (dark)` branches in components

## Sources
- Material Design 3 — Color system and roles: https://m3.material.io/styles/color/system/overview
- Material Design (M2) — Dark theme guidance: https://material.io/design/color/dark-theme.html
- Apple HIG — Dark mode: https://developer.apple.com/design/human-interface-guidelines/dark-mode
- GitHub Primer — Color modes: https://primer.style/foundations/color
- WCAG 2.2 — 1.4.3 Contrast (Minimum), 1.4.11 Non-text Contrast: https://www.w3.org/TR/WCAG22/
- Nielsen Norman Group — Dark mode adoption and expectations: https://www.nngroup.com/articles/dark-mode/
- WebAIM — Contrast checker: https://webaim.org/resources/contrastchecker/
