# 00 — UI System Principles

> The foundation on which all the other rules stand. Architecture sources:
> `docs/prd/prd-0006-design-system-tokens.md` (tokens), `docs/prd/prd-0009-ui-layering-uikit.md`
> (layers/kit), `docs/ui-kit.md` (screen-frame guide). Architectural decisions
> are not changed by editing this folder — only via an ADR.

## П1. Three token layers

Color and metric travel a path of three layers; skipping layers is forbidden:

1. **Layer 1 — primitives**: `design/tokens/*.json` (colors, dimensions, motion,
   themes/*). Machine-readable, in the W3C spirit (`$type`/`$value`/`$desc`). They store
   only values and their provenance ($desc — the coordinates of the source in code).
2. **Layer 2 — mirror + semantics**: `canvas_core::tokens` — Rust constants,
   pixel-identical to the JSON (the parity test I-5 must fail on any divergence);
   on top of the primitives — the semantic slots `ThemeColors` (render) and
   `KitPalette` (ui).
3. **Layer 3 — consumers**: kit components, app surfaces, CSS variables of
   web widgets. Color reaches a shader only from a `ThemeColors`/`KitPalette`
   instance filled from layer 2.

Corollary: **the runtime never parses JSON** (wasm purity); the JSON is needed
by the parity tests and by humans.

## П2. Invariant I-1 — zero visual jump

Token values = the current constants of the repository. Moving a component
onto a token does not change a single pixel. Any deliberate change of a value (for
example, differentiating selected from hover) is a separate owner decision through
an edit of this folder + the code, never "in passing".

## П3. Invariant I-5 — JSON ↔ Rust parity

`design/tokens/*.json` and `canvas_core::tokens` must match. The parity test in
`tokens.rs` fails CI on any divergence. You edit the JSON — the mirror is edited
in sync, and vice versa.

## П4. Slot-only kit (no color in components)

There are no color constants and no color arithmetic in `canvas-ui`. Kit components
(button, chip, field, switch, toast, modal…) take color only from the `KitPalette`
slots (full registry — `10-components.md` §K2) and choose a slot by state
(`button_style` — a state → slot mapping, without on-the-spot `c*1.3`
computations). A new state = a new slot, not a new formula.
The only exception — the alpha-tint of a slot (`paint::tint`): swapping
the alpha channel with a constant from C2 without editing rgb (see C8); rgb arithmetic
remains forbidden.

## П5. Input = what is visible

The geometry is single: a surface's hit-rects are built from the **same layout functions**,
as the drawing. The pick order and the draw order are derived from `SurfaceRegistry`
(layers → capture policies → registration order). Manual z-lists, duplicate
geometry for hits, "invisible clickable zones" are forbidden.

## П6. Measured text

Widths for layout — only via `TextMeasurer` (real cosmic-text shaping,
the same metrics as the render). Per-character heuristics
(`chars.len() × width`), silent truncations (`take()`, `break` clamps)
are forbidden (the CR-015 class). Truncation — only by a deliberate policy
Wrap/Ellipsis/Clip; the field caret is counted in characters, not in bytes.
Measurement and render use one (family, weight) pair — the test
`ui_measure_weight_matches_render_attrs`.

## П7. Layout primitives, not ad-hoc quads

Layout — `Row`/`Column`/`stack`/`constrain`/`pad`/`grid_cells` from
`canvas_ui::layout`; children — `Child::fixed/spacer/flexible(grow)`. Overflow
is not masked silently: a `Fit` overflow is caught by lint G4, narrow slots degrade
in a named way (`SqueezeTail`), exotica (polar coordinates of the wheel menu) — via
`Custom(rect)` with a justifying comment. Taffy was rejected (ADR-0013:
108.5 KB of wasm flexbox / 376 KB of grid versus the zero cost of the primitives).

## П8. A surface in 3 steps

A new surface: 1) a declaration in `SurfaceRegistry` (id, layer, capture,
keyboard-scope, degradation `HideBelow{min_w,min_h}`); 2) hit-rects from the same
layout functions; 3) a click/keyboard dispatcher + arm in the Esc handler.
Handler bodies are moved into `click_<surface>` methods.

## П9. Geometry lints

- **G1** — grep audit of color literals outside tokens/themes (a defect).
- **G3** — the contrast machine runs every theme (text ≥ 4.5:1, graphics ≥ 3:1).
- **G4** — canonical scenes × 3 viewports (1280×800 / 1024×640 / 800×560) ×
  RU/EN: 0 overlaps of interactive rects of different surfaces of one layer,
  0 exits beyond the viewport, 0 text overflows.
- **G5** — grep audit of text heuristics (`take(`, truncations, per-character widths).
- **G8** — the surface registry: the declared surfaces are registered.
- Peak matrix: a click under a `Block`/`Capture` surface never reaches the
  canvas.
- There are no golden-image pixel tests — only geometric lints
  (`testing::snap` — golden geometry, rounding to whole ui-px).

## П10. Degradation instead of ugliness

A window below a surface's minimum → the surface is **hidden entirely**
(`HideBelow`), not squeezed into unreadable mush. Reference viewports of the
lint: 1280×800, 1024×640, 800×560; the what-if bar is hidden below 900×600.

## П11. Two coordinate scales

World px — the canvas (cards, edges, captions; scaled by zoom).
Screen px — the screen chrome (panels, HUD, badges; not scaled by zoom).
The rule is written into the tokens: `typography.*` — world px, except `hud_*`/`badge_*`.
