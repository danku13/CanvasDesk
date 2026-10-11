# 08 — Control states

> Code: `crates/canvas-ui/src/widget.rs` (WidgetState),
> `crates/canvas-ui/src/component/mod.rs` (KitState, state_layer_alpha,
> resolve_state), `crates/canvas-ui/src/component/button.rs` (button_style),
> slots — `01-colors.md §C7`.

## ST1. State matrix

`KitState::Normal | Hovered | Focused | Pressed | Dragged | Disabled | Selected | Error`

(8 values; `component/mod.rs` — Wave T §5.1.1 added Focused/Dragged/Error.)

Priority when evaluating (`widget.rs::WidgetState::kit_state`, the upper one
swallows the lower ones):

```
Disabled > Error > Pressed > Hovered > Focused > Dragged > Selected > Normal
```

Rules:
- `Pressed` = the button is held down and the cursor is over it; the cursor
  leaves → return to Hovered (the press is cancelled).
- `Focused` = keyboard focus only (`:focus-visible` semantics): a mouse
  click does not activate it (`WidgetState::set_focused` carries the
  keyboard-origin flag; only keyboard-origin draws the focus-ring).
  Visual — the focus-ring in the `accent` slot + the 0.10 state-layer.
- `Dragged` = a drag operation is in progress (state-layer 16%).
- `Error` = a validation failure; the kit does not paint it — the consumer
  renders it from the `control_danger` slot (the field example — IN6).
- `Selected` — a persistent selection state (the selected gallery row, a
  filter chip), orthogonal to hover.
- `Disabled` — never highlighted by hover; pressing a disabled control does
  not “charge” a click (press on disabled does not arm the release event).

Two reading models, both normative:
- the ladder above — the single-value resolver `kit_state()` (the priority
  model; 40+ consumers per the `widget.rs` header);
- additive — `active_states()` returns up to 4 stacked states
  `[Error, Pressed, Hovered, Focused]`; Selected/Dragged participate in the
  ladder only. The additive set feeds the M3 state-layer (`state_layer_alpha`
  + `resolve_state`: several active states → the max alpha, not a sum).

## ST2. State slots

The fill color is chosen by mapping state → palette slot (no arithmetic) —
this is what `button_style` differentiates:

| State | Slot (normal controls) | Slot (primary) |
|---|---|---|
| Normal | control_fill | control_primary |
| Hovered | control_hover_fill | control_primary_hover_fill |
| Selected | control_selected_fill | — |
| Pressed | = Hovered (the hover slots; the 2026-10-09 audit was aligned to the actual `button_style` — the earlier “= Normal” text contradicted the code) | = primary_hover_fill |
| Disabled | control_fill + text → control_disabled_text | ← |
| Focused | = Normal — the fill does not change | = Normal |
| Dragged | = Normal — the fill does not change | = Normal |
| Error | = Normal in the kit; the visual — the `control_danger` slot on the consumer | ← |

The Wave T §5.1.1 states have no fill slots of their own: the mechanism is
the M3 state-layer — a translucent overlay over the base fill
(`state_layer_alpha`: Hovered 0.08 / Focused 0.10 / Pressed 0.10 /
Dragged 0.16, the rest 0.0; the overlay color = content or accent). With
several active states the alphas do not sum: `resolve_state` takes the max.

A separate `control_pressed_fill` slot (full pressed differentiation) —
a managed v2 change: editing this table + KitPalette + the tokens.

Invariant I-1: today selected_fill = hover_fill (a gallery row does not
distinguish selection/hover) — the semantics are already separated by slots,
the visual differentiation is a managed v2 change via editing this folder and
the tokens.

## ST3. Click contract

A click fires once: press inside + release inside (`clicked =
released_inside`). Press outside → release inside does NOT fire. Press inside →
release outside — does not fire. This is the contract of all interactive rects
of the kit.

## ST4. States of non-control entities

| Entity | States |
|---|---|
| Node card | normal / selected (accent border) / broken (#737373 border) / in a group (accent α0.08 fill) — border priority: selected > broken > group |
| Edge | default / flow (teal) / draft (accent α0.70) / focus (accent + breath) / dimmed (α floor 0.35) / amber (unmapped) |
| Port | idle (dot 10) / hover (grows up to 26 max) / active draft |
| Surface | open / closed / hidden by HideBelow |

## ST5. Hover timings

| Behavior | Value |
|---|---|
| Tooltip delay | 500 ms |
| Palette flyout opening | 150 ms (hover) |
| Flyout closing | 300 ms (after the cursor leaves) |
| Double click | a 500 ms window & ≤5 px offset |

Showcase: the entire ST1–ST4 matrix is shown in the UI admin panel (FR-070,
the “?” menu → “UI console”, the “Components” and “Canvas” sections) — a live
sample of the palette slots, including container fill levels and live slot
editing. The full per-component section list — `10-components.md` §K4 (the
2026-10-09 audit: 6 components lack sections — the W2 backlog).

## ST6. What counts as a control

A control = an interactive rect with a state from ST1. Non-controls (text,
icons, separators) have no hover states and are not picked as interactive
(`HitRect::decoration`). If a decoration must catch a click — it is a control,
and it must have all the states of the matrix.
