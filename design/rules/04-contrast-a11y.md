# 04 — Contrast and accessibility

> Machine: `crates/canvas-render/src/contrast.rs` (WCAG 2.1).
> Trigger: CR-007 — themed text on colored cards dropped to 2.3–4.1:1.

## A1. Contrast targets

| What | Norm | Standard |
|---|---|---|
| Text on any fill | ≥ 4.5:1 | WCAG AA 1.4.3 |
| Meaning-bearing graphics (severity frames, guides, the explain sheet) | ≥ 3:1 | WCAG 1.4.11 |
| A component's own controlled fills (self-controlled) | target 7:1 | WCAG AAA |

The targets are baked into code (`contrast.rs`, `theme.rs` readable_on_card)
and are run through the contrast machine for every theme — G3. A new
theme/slot that has not passed G3 is not accepted.

## A2. Contrast machine (auto-contrast)

For user card fills (color presets 1–6, colored backgrounds) the text color
is not fixed — it is computed:

1. If `contrast_ratio(fg, bg) ≥ target` — leave as is.
2. Otherwise — shift fg toward the extreme (black/white) preserving the hue,
   binary search to the comfortable target `target + 1.55`.
3. Fallback — the best of the ink candidates (pure black/white); `pick_ink`
   guarantees ≥ 4.5:1 against any fill (worst case #777 → 4.66:1).

Rule: a component never "guesses" the ink — it calls
`pick_ink`/`ensure_contrast` instead of making the text white on a whim.

## A3. Hit zones (minimum interaction areas)

| Element | Zone |
|---|---|
| Edge port | 10 px (presets 10/14/20/28/40, grows with zoom, clamp [10,40]) |
| Edge (body) | 6 px around the axis (or half the stroke width + 2) |
| Card resize handle | 16 px corner |
| Row error badge hover pad | +10 px to the bounds |
| Menu item | 26 px height of the whole row, not just the text |

The hit zone is always ≥ the visual; hit-rects are built by the same layout
functions as the rendering (П5).

## A4. Keyboard and focus

- The focus frame is the `accent` slot (the showcase TextField pattern).
- FocusRing: Tab/Shift+Tab over the content rects of a surface; the order is
  rebuilt preserving position (`FocusRing::retain_order`).
- The Esc stack closes the topmost Block/scope surface — the ladder is derived
  from the registry automatically.
- In fields (the inline editor of a node note — the only multiline editor): Enter/Ctrl+Enter — commit, Shift+Enter — line break, Esc — cancel. Single-line kit fields live by IN2 (Enter/Esc — consumer semantics; clarified by the 2026-10-09 audit — the previous wording contradicted 09-input.md).
- Editor undo depth — 50.

## A5. Degradation and readability

- Below a surface's minimum — hide it entirely (`HideBelow`), not shrink:
  the what-if bar < 900×600; lint references 1280×800 / 1024×640 / 800×560.
- HUD/badge texts have a shadow/underlay for readability over the canvas
  (HUD shadow #101012).
- The minimap does not draw edges above 500 nodes (readability over completeness).
- The minimum readable type size of a card header is 4 physical px; below that we do not render.

## A6. Motion accessibility

Animations are highlighting/camera movement only, no high-frequency
flashing: the result pulse of 1200 ms — a smooth alpha sine, the focus
"breathing" of 1600 ms. No strobes/sharp flashes in loops.
