# 01 — Colors

> Primitive layer: `design/tokens/colors.json` ↔ `canvas_core::tokens`.
> Semantics: `crates/canvas-render/src/theme.rs` (ThemeColors, 36+ slots).
> Format: sRGB, f32 arrays 0..1 RGBA or u8 RGB bytes.

## C1. The single accent

`accent = [0.396, 0.612, 0.969, 1.0]` (#65A0F7) — the single source of the whole
accent family. Everything "selected/active/being-stretched" is marked with the accent
color: the card selection border, the focus border, the draft edge, widget
chrome on hover, drop zones, text selection, groups.

| Derivative | Value | Where |
|---|---|---|
| Selection/focus border | accent α1.0 | cards.rs SELECTION_BORDER/FOCUS_EDGE |
| Draft (rubber-band) edge | accent α0.70 | EDGE_DRAFT |
| Widget chrome on hover | accent α0.10 | WIDGET_CHROME_HOVER_FILL |
| Drop fill/border/zone | accent α0.10/0.70/0.50 | DROP_GHOST_* |
| Text selection | accent α0.35 | selection_fill |
| Groups (fill/border) | dark: α0.08/0.40 · light: α0.10/0.50 | group_fill/group_border |

Rule: do not introduce a "second blue". A shade is needed — a derivative of the accent
through an alpha step fixed in the table above.

## C2. Alpha steps

The allowed set of transparencies (opacity_steps): **0.08, 0.10, 0.22, 0.30,
0.35, 0.38, 0.40, 0.50, 0.55, 0.60, 0.65, 0.70, 0.85, 0.90, 0.92, 0.95, 0.97**.
A new alpha = an edit of the steps + code, not a local `a: 0.37`. The steps are
a record of the set actually in use, not a free palette
(the 2026-10-09 audit: 0.38 was entered as actually used — chat-bubble
borders, source agent_panel.rs).

## C3. Semantic states (not themes)

State colors are the same for the dark/light theme unless explicitly stated otherwise:

| Semantics | Token | Value | Usage |
|---|---|---|---|
| Error | `error` | #E55C5C | a result line with an error, the danger button |
| Broken link | `state.broken` | #737373 | the border of a card with a broken port |
| `==текст==` highlight | `state.highlight` | [0.85,0.75,0.30,0.30] | the fill of highlighted text |
| What-if line | `whatif_fill` | [0.30,0.55,0.95,0.22] | the background of compared lines |
| What-if chip | `whatif_chip` / `chip_dim` | [0.30,0.55,0.95,1] / #242A33 | the active/muted chip |
| Delta badge | `whatif_badge` | #DFA63E | scenario deltas |
| HUD | `hud` | #659CF8 (+ shadow #101012) | the F3 overlay |
| Result pulse | `pulse_result` | #FFD959 (alpha animated) | the border of a card with a fresh result |
| Explain sheet | `explain_leaf` | #9FD6FF dark / #1C6EA8 light | the calculation tree |
| Success | `control_success` | derived (theme.rs, FR-070) | done/completed badges, chat-bubble Success |
| Warning | `control_warning` | derived (theme.rs, FR-070) | paused, low-quota, Banner::Warning |

## C4. Edges

| Type | Color | Meaning |
|---|---|---|
| `edge.default` | [0.52,0.58,0.66,1] | a neutral edge |
| `edge.flow` | [0.13,0.66,0.55,1] (teal) | a value edge (value spill) |
| `edge.draft` | accent α0.70 | the rubber-band line while dragging |
| Amber (severity[0]) | dark [0.961,0.651,0.137] | an unmapped edge, a warning |

Edge recolor priority: focus (accent) > broken/amber > flow (teal) >
default; non-focused edges darken to the floor α0.35 (see 06-motion, focus_fade).

## C5. Dialogs and toasts

Dialog panel: fill [0.09,0.11,0.15,0.97], border [0.23,0.51,0.96,1] (a blue
related to the accent), primary button [0.16,0.32,0.60,1], secondary
[0.20,0.23,0.29,1], button borders [0.35,0.40,0.50,1], text #E8ECF4,
secondary text #B6BECE. Toast: text #F0E6C2 (warm paper) on a dark
static background. These colors are slots, not literals; the light theme takes them from
ThemeColors.

## C6. Wheel menu

Dim disk over the world [0,0,0,0.35]; category sector [0.17,0.18,0.22,0.92];
template sector [0.20,0.22,0.27,0.92]; hover/active hub [0.18,0.29,0.48,0.95];
border [0.22,0.24,0.30,0.90]. Transparencies 0.90–0.95 are mandatory: the canvas
is visible under the menu.

## C7. Control state slots

hover/selected/disabled — ThemeColors slots (not on-the-spot computations):

| Slot | Dark | Light |
|---|---|---|
| control_hover_fill | [0.183,0.183,0.229,0.97] | [1,1,1,0.97] |
| control_selected_fill | = hover (indistinguishable today; differentiation — v2) | = hover |
| control_primary_hover_fill | [0.248,0.456,0.84,1] (both themes) | ← |
| control_disabled_text | #8A909C (both themes) | ← |

Rule: the hover fill of a row/button is taken as a slot; the formula
`c·1.3 + 0.04` remains in history — writing it in code is forbidden.

## C8. Forbidden practices

- A color literal in a component/surface builder (G1) — a defect
  (test fixtures `#[cfg(test)]` are not builders, excluded from G1).
- Color arithmetic in `canvas-ui` (lightening/darkening on the spot,
  rgb substitution/multiplications).
- Exception — the alpha-tint of a slot: `canvas_ui::paint::tint(slot, α)` replaces
  ONLY the alpha channel of the slot with a constant from C2 (banner α0.10, chat-bubble
  α0.08/0.38, radio-card α0.10). rgb is not touched — no new colors are
  produced (the 2026-10-09 audit: the pattern was legitimized; before that it was
  a gray zone of П4).
- A new alpha outside C2; a new "accent" color outside the accent.
- Color reaches the shader in no way other than through a ThemeColors/KitPalette instance.
