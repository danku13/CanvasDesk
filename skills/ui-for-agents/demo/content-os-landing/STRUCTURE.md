# Content OS Landing — Universal Structure

One fixed DOM skeleton, five historical style skins. This demo proves the core claim of
the library: **structure is style-agnostic; style lives in tokens + a scoped skin layer.**

- Product: **Content OS** — an ecosystem of AI agents for product marketing
  (strategy → tactics → implementation) for open-source and enterprise projects.
- Primary JTBD: *As a CPO, I want Content OS to take over all product-marketing work —
  from strategy to generated post chains and publishing — with a transparent calendar
  of distribution flows (social networks, thematic portals, newsletters).*

## DOM contract

- A single `index.html` serves all five themes. Themes may only use CSS.
- The active theme is the `data-style` attribute on `<html>`
  (`art-nouveau | de-stijl | bauhaus | constructivism | art-deco`).
- Each theme file defines **tokens** (CSS custom properties) plus **scoped skin rules**
  (every selector is prefixed with `html[data-style="…"]`).
- Decorative hooks (`.hero__visual`, `.shape--a/b/c`) are empty `aria-hidden` containers —
  every theme paints its signature geometry into the same hooks.
- No JS DOM changes on theme switch; JS only swaps the attribute and persists the choice.

## Sections (fixed order)

| # | Section | Purpose / JTBD mapping | Theme hooks |
|---|---------|------------------------|-------------|
| 1 | `header.site-header` | Brand + nav + primary CTA | border weights, brand face |
| 2 | `.hero` | JTBD statement: agents take over product marketing end-to-end | signature decor set (arch / mondrian / primitives / wedge / sunburst) |
| 3 | `.stats` | Credibility strip (agents, channels, 3 levels, 1 calendar) | numeric display face |
| 4 | `.pipeline` | 3 levels: Strategy → Tactics → Implementation | step-number geometry |
| 5 | `.agents` | Ecosystem: Strategist, Editor, Calendar, Publisher, Analyst | card skin |
| 6 | `.calendar` | **Key JTBD artifact**: flows × days grid with post chips | cell borders, chip skin |
| 7 | `.segments` | Open source vs Enterprise offering | two-panel skin |
| 8 | `.quote` | CPO voice of customer | pull-quote treatment |
| 9 | `.cta` | Final conversion band | inverted/jewel band |
| 10 | `footer.site-footer` | Credits, links to the library | hairline rules |
| +  | `.switcher` | Style switcher (radios, localStorage) | theme-neutral chrome |

## Theme layer rules

1. Tokens first: palette, radius, line weights, type, motion — from the matching preset
   in [`60-styles/`](../../60-styles/). Hex values are taken verbatim from the presets.
2. Skin rules only where tokens are not enough (decor shapes, structural accents).
3. Respect each preset's hard rules: e.g. De Stijl = no diagonals/curves and exactly two
   line weights; Constructivism = ≤3 diagonal elements per view, 0–150 ms motion;
   Art Deco = symmetric hero, gold as line only; Art Nouveau = gold is never text;
   Bauhaus = primitives only, one primary per component.
4. Motion tokens (`--speed`, `--hero-dur`) differ per preset; all motion is disabled under
   `prefers-reduced-motion`.

## Accessibility contract (inherited from the presets)

- Text contrast ≥ 4.5:1 against the actual background; accent text uses the
  preset's darkened variants (`--accent-ink`), never raw gold/yellow.
- Focus is always visible: `outline: 3px solid var(--focus)` + offset.
- Switcher = native radio group (keyboard + screen-reader friendly), targets ≥ 24px.
- The calendar uses table roles (`table/row/columnheader/rowheader/cell`).
- All decorative shapes are `aria-hidden="true"`; DOM order = reading order.

## File map

```
index.html            single DOM for all themes
css/main.css          skeleton: layout + components driven by custom properties
css/theme-*.css       5 skins (tokens + scoped rules), one per movement
js/switcher.js        data-style switch + persistence + live announcement
STRUCTURE.md          this document
```

## Adding a 6th style

1. Copy any `theme-*.css` → `theme-<name>.css`.
2. Replace token values from the new preset in `60-styles/`.
3. Re-skin the decor hooks within the preset's rules.
4. Add one `<link>` and one radio to the switcher. No HTML/JS changes otherwise.
