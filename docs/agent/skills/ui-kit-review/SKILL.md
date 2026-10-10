---
name: ui-kit-review
description: Mandatory rules for any Rust UI work in CanvasDesk — what to use from the canvas-ui kit, the six forbidden hardcode patterns, the kit-extension procedure and the pre-submit review checklist, plus widget security (M5). Load before writing or reviewing UI code.
version: 1
---

# $ui-kit-review

Kit discipline for Rust UI work. Decomposes AGENTS.md ("UI kit — mandatory rule
for Rust UI work" + "Security and widget rules (M5)"); AGENTS.md stays the law.

## When to load

- Route `ui-rust` (docs/agent/routes.yaml): any change to overlay / panel /
  dialog / control / layout / theme / rendering code — before writing the first
  line and again before submitting the task; also when proposing a kit
  extension or reviewing someone else's UI diff.
- The full guide is `docs/ui-kit.md` (linked in References) — the 3-step
  "add a surface" walkthrough, layout primitives, text measurement, lints.

## Mandatory rules

1. **Kit first.** Any Rust UI work goes through the existing `canvas-ui` kit
   (`crates/canvas-ui/src/`). Hardcoding quads/colors/geometry bypassing the
   kit is forbidden — it breaks theme consistency and reuse, and makes visual
   regressions invisible until manual testing.
2. **No workarounds for gaps.** If something is missing from the kit, the agent
   MUST propose a kit extension instead of working around it with hardcode
   (procedure below).
3. **Layer direction (G7).** The kit does not depend on render/OS; render
   depends on the kit, not the other way. New components/slots/tokens live in
   `canvas-ui`, not in `canvas-app`.

## What to use from the kit

| Need | Canonical kit path |
|---|---|
| UI element colors | `KitPalette` (`ThemeColors` v2 slots: `control_fill`, `control_danger`, `text`, `palette_border`, `menu_fill`, …) — NEVER inline `[f32;4]` rgba literals |
| Padding/radii/gaps | `canvas_core::tokens` (`SPACING_*`, `RADIUS_*`) — NEVER duplicate `4.0/8.0/16.0` as magic constants |
| Control sizes | `kit::BUTTON_HEIGHT`, `kit::ICON_BUTTON_SIZE`, `kit::LIST_ROW_H`, `kit::TEXT_FIELD_HEIGHT`, `kit::CHIP_HEIGHT`, `kit::SWITCH_W/H` — NEVER re-declare `HEADER_H=30.0`, `ROW_H=26.0`, `INPUT_H=32.0` locally |
| Layout (positioning) | `kit::constrain`, `kit::stack`, `kit::pad`, `Column`/`Row`/`Child`/`MeasuredItem`/`RowPolicy` — NEVER compute `x = panel.right() - 28.0, y = panel.y + 6.0` with magic numbers |
| Text (shaping + width + wrap) | `TextMeasurer` (`width_of`, `wrap`, `ellipsis`) — NEVER estimate width as `len() * 5.5 + 14.0` (CR-015 explicitly forbids this) and never build `Buffer::new` + `set_text` manually in overlay logic |
| Buttons / chips / icons / switches / fields | `kit::button_layout/style/size`, `kit::chip_*`, `kit::icon_button`, `kit::switch`, `kit::text_field` |
| Lists and scroll | `kit::list_rows` + `kit::ScrollState` + `kit::scroll_bar` |
| Popups / modals / tooltips | `kit::dropdown_menu`, `kit::modal`, `kit::tooltip` (anchor + flip + delay) |
| Panels / cards | `kit::panel_rect/style`, `kit::card` |
| Tables | `kit::Table` (retained) or `kit::row_guides` + `kit::paint_row` |
| Draw layer (quads on screen) | `Painter::rect/panel/control/label` + `PaintItem` (NOT a manual `CardInstance { pos, size, fill, border, params, corners }`) |

## Forbidden patterns (caught in review and by lints)

1. **Inline rgba literals in UI code** — `[0.30, 0.75, 0.55, 1.0]`, `Color::rgba(77, 191, 140, 255)` in overlay/panel logic. Even a "one-off" is future color drift. Route through `KitPalette` (an existing or new slot).
2. **Magic geometry numbers** — `4.0, 6.0, 22.0, 28.0, 110.0` in `UiRect::new(...)` and `pos: [...]`. Take them from `kit::*` constants or `tokens::SPACING_*`.
3. **Duplicated local constants** — `HEADER_H`, `ROW_H`, `INPUT_H`, `FONT_*` in every `*_ui.rs`. They must either `pub use` from `kit`, or become a new `kit::CONST` (if the value is unique).
4. **Manual `Buffer::new` + `set_text` + `shape` in overlay logic** — only via `TextMeasurer` (or a `Shaper` behind a trait boundary, FR-068 W2). `set_text`+`shape_until_scroll` is allowed only inside kit/components and in `crates/canvas-render/src/text.rs` (that is the renderer itself).
5. **Text width heuristics** — `len() as f32 * factor + pad`. The CR-015 defect class: different glyphs have different widths, Cyrillic is wider than Latin, emojis consume space. Only `TextMeasurer::width_of`.
6. **Manual `CardInstance { pos, size, fill, border, params, corners }`** in overlay logic — route through `Painter::rect/panel/control` or `KitDraw` (the Painter↔`Vec<CardInstance>` adapter). `CardInstance` directly — only in `canvas-render` (its type) and in the adapters `crates/canvas-app/src/app/support.rs` (the layer boundary).

## Kit-extension procedure (if something is missing from the kit)

1. **Identify the gap** — which component/token/slot is missing, on which pattern it repeats in several places (≥ 2 files → a kit candidate).
2. **Propose the kit extension** — in the task/PR:
   - **New component** → add to `crates/canvas-ui/src/component/` (or extend an existing one) + export via `kit.rs`. The F-8 contract of PRD-0009: palette slots only, token scale only, text only via `TextMeasurer`.
   - **New `KitPalette` color slot** → the field in `KitPalette` (`crates/canvas-ui/src/component/mod.rs`) + the mapping in `ThemeColors` (`crates/canvas-render/src/theme.rs`) + presets (`crates/canvas-render/src/theme_presets.rs`) + the key in `REQUIRED_KEYS` (the semantics-parity test).
   - **New geometry token** → `crates/canvas-core/src/tokens.rs` (`SPACING_*`, `RADIUS_*`, control heights) + the mirror JSON in `design/tokens/` (the JSON↔Rust parity test).
   - **New layout pattern** (radio_card, chat_bubble, crumbs, tree_layout, anchored_stack, footer_buttons, chip_strip, two_column, backdrop, banner) → a component in `crates/canvas-ui/src/component/` + export via `kit.rs`.
3. **File an FR document** (if the extension is significant) using the [cr-template.md](../../../../docs/change-requests/cr-template.md) structure: What/Impact/Changes/Tests. Small extensions (a new palette slot) may live in the task commit without an FR.
4. **Implement the kit extension FIRST** — only then build the surface via the new kit component. Never the other way around.

## Review checklist (before submitting a UI task)

- [ ] Colors come from `KitPalette`; no inline rgba literals (grep `\[\s*0\.[0-9]+\s*,\s*0\.[0-9]+` in changed lines).
- [ ] Geometry from `kit::*` constants and `tokens::SPACING_*`/`RADIUS_*`; no new `const HEADER_H: f32 = 30.0` in `*_ui.rs`.
- [ ] Layout via `Column`/`Row`/`stack`/`constrain`/`pad`; no manual `x = panel.right() - MAGIC` formulas.
- [ ] Text via `TextMeasurer`; no `Buffer::new` in overlay logic and no `len() * factor` width heuristics.
- [ ] Quads via `Painter::rect/panel/control` or `KitDraw`; no manual `CardInstance { ... }` literals in overlay logic.
- [ ] If a new component/slot/token was added — it is in `canvas-ui`, not in `canvas-app` (per G7: the kit does not depend on render/OS).
- [ ] If something was missing — the agent explicitly stated it in the task (an FR or a commit note) instead of silently hardcoding around it.

## Exceptions (acceptable hardcoding)

- **Test fixtures** — `KitPalette::default()` with `[0.0;4]` slots for geometry checks, `Color::rgba(...)` in tests (`*_ui.rs::tests`, `admin_ui.rs::test_palette`) — no UI meaning, asserts only.
- **Layer adapters** — `crates/canvas-app/src/app/support.rs::paint_items_to_band`, `crates/canvas-app/src/app/overlays.rs::KitDraw` — the Painter↔GPU-instance boundary; `CardInstance` is built here by right (this IS the kit→renderer adapter).
- **Render** — `crates/canvas-render/src/{cards.rs,renderer.rs,text.rs}` — the GPU backend; `CardInstance` is its own type; tokens (`tokens::EDGE_*`, `tokens::ACCENT`) are already canonical.
- **Diagnostic overlays** — `crates/canvas-app/src/debug_overlay.rs` — layer colors by design "diagnostic, not theme"; documented in the file header.
- **Specialized primitives** (polar wheel in template_ui, sector SDF in `crates/canvas-render/src/sectors.rs`) — an escape hatch via `Custom(rect)` with a justification comment (the G8 grep audit).

## Widget security (M5)

- Widgets — local packages only (`%APPDATA%/canvasdesk/widgets/<id>/`), installed explicitly by the user. A remote URL as a widget — forbidden architecturally.
- Permissions from `widget.json` are checked on EVERY bridge call; without `network` all outgoing `WebResourceRequested` are blocked; navigation outside the package is forbidden; `SetVirtualHostNameToFolderMapping` + CSP `default-src 'self'`.
- Bridge — typed JSON-RPC over postMessage; all messages validated with serde schemas; an invalid message = drop + warn, never a panic.
- No network calls in the host — the product is local; the network exists only inside widgets with the `network` permission.

## References

- [docs/ui-kit.md](../../../../docs/ui-kit.md) — the kit guide.
- [docs/prd/prd-0009-ui-layering-uikit.md](../../../../docs/prd/prd-0009-ui-layering-uikit.md) — UI/kit layer architecture; surface contract — `docs/interface-objects/surface-registry.md`.
- [docs/change-requests/fr-046-design-tokens.md](../../../../docs/change-requests/fr-046-design-tokens.md) — tokens in code; the design-token PRD — `docs/prd/prd-0006-design-system-tokens.md`.
- [docs/dev-researches/ui-hardcode-audit.md](../../../../docs/dev-researches/ui-hardcode-audit.md) — the hardcode audit (kit gaps, migration order).
- FR lineage (background, read on demand): `fr-051-ui-layering-uikit.md` (U1), `fr-053-ui-layering-u3-pilots.md` (U3), `fr-055-ui-layering-u4-kit.md` (U4), `fr-057-ui-kit-painter-widget-state.md` (Painter + WidgetState).
- Token sources: `crates/canvas-core/src/tokens.rs` + `design/tokens/`.
