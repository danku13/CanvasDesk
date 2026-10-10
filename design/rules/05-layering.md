# 05 — Layers and input interception

> Code: `crates/canvas-ui/src/layer.rs`, `capture.rs`, `registry.rs`,
> `hit.rs`, `keyboard.rs`. Guide: `docs/ui-kit.md`.

## L1. Nine fixed drawing bands

| Layer | What lives there | Examples |
|---|---|---|
| L0 World | the canvas | cards, edges, grid |
| L1 WorldOverlay | world overlays | wheel-menu sectors, snapping guides, what-if pulse |
| L2 Widgets | inline world elements | text fields in nodes, ports |
| L3 Panels | docked panels | what-if bar, palette dock, search, settings, help |
| L4 Popups | popups | tooltips, dropdowns, context menus, flyout |
| L5 Modals | modals | onboarding, gallery, dialogs, main stage |
| L6 Drag | dragging | ghost of the dragged card/template |
| L7 Toasts | toasts | notifications |
| L8 Debug | debugging | F9 DebugOverlay |

Draw order = `UiLayer::DRAW_ORDER`; within a band — the frame assembly order.
A new layer is never added "on the spot" — only by editing the enum + DRAW_ORDER.

## L2. Capture policies

| Policy | Clicks on the surface | Clicks past it (backdrop) | Examples |
|---|---|---|---|
| `Block` | intercepts everything | the surface contract (close/swallow) | settings, docs, gallery, onboarding, stage, dialogs |
| `Capture` | by its own hit-rects | falls through below | what-if bar, palette dock, hotkey hint, minimap |
| `PassThrough` | does not intercept | falls through below | the world |
| `Passive` | no hit-rects at all | falls through below | toasts |

`intercept_outside()` — the backdrop behavior (Block=true, the rest=false).

## L3. Surface registry — the single point

`SurfaceDecl { id, layer, capture, keyboard_scope, degradation }`.
A duplicate id — panic at startup. Pick order: the top layer first; within
a layer — reverse registration order. Manual z-lists are forbidden.

## L4. Keyboard

- `KeyboardRouter`: a stack of keyboard scopes, the top scope swallows its
  keys; the canvas default scope — `KeyboardScopeId::CANVAS`.
- Esc stack = reverse registration order of Block/scope surfaces
  (`esc_stack`) — the topmost modality closes first.
- FocusRing — the Tab ring of the content rects of one surface.

## L5. Scissor policy

One scissor-rect per layer band; a clip never expands the visible
area. A surface without `clip` in `SurfaceFrame` is not assembled (clip
is mandatory).

## L6. Verification rules

- Pick matrix: a click under Block/Capture never reaches the canvas
  (hit.rs unit tests).
- G4 lint: 0 intersections of interactive rects of **different** surfaces of
  one layer; 0 overflows beyond the viewport (3 viewports × RU/EN).
- Intersections of interactive rects of **one** surface are allowed
  (e.g. a chip over a bar), but the order within a surface must be
  determined by the order of the hit-rects.
