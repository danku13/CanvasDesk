# UI kit — screen frame guide (PRD-0009)

> How to add a surface in 3 steps, how to lay out with primitives, how to measure
> text, and which lints keep the geometry in order. The architecture source is
> `docs/prd/prd-0009-ui-layering-uikit.md`; the surface contract is
> `docs/interface-objects/surface-registry.md`.

## 1. What this is

The CanvasDesk screen is not ad-hoc lists of quads, but a **model of surfaces**:

- `canvas-ui` — pure screen geometry (no GPU/OS): the `UiLayer` layers,
  the `SurfaceRegistry` registry, the `CapturePolicy` capture policies, the `UiFrame`
  frame, `HitStack`, `KeyboardRouter`, layout primitives, `TextMeasurer`.
- `canvas-app::app::ui_registry` — the declaration of the app's surfaces
  (what is open, in which layer, who catches the clicks/keyboard) + hit-rects from the
  same layout functions that draw.
- `canvas-render` — executes the frame in layer bands (`ScreenBand`), the order =
  `UiLayer::DRAW_ORDER`; within a band — the assembly order.

One source of geometry → **input = what is visible**; adding a surface
does not touch the input chains.

## 2. A surface in 3 steps

1. **Declaration** — `crates/canvas-app/src/app/ui_registry.rs`:
   the `id` constant, an entry in `build_registry` (a layer, a capture policy,
   a keyboard-scope if needed, the `HideBelow` degradation).
2. **Hit-rects** — an arm in `fill_hit_rects`: rects from the same layout functions
   that the drawing uses (`HitRect::interactive` / `HitRect::decoration`).
3. **Input** — click: an arm in `dispatch_surface_click` (move the body out into a method
   `click_<surface>`); keyboard: if needed an arm in `owner_of` +
   `route_owner_key`; Esc: an arm in `dispatch_esc`.

Drawing — a dedicated overlay function in `crates/canvas-app/src/app.rs`, the quads/texts go into the band
of their layer (`ScreenBands::push`). The pick and draw order is derived from the registry —
manual z-lists are forbidden.

## 3. Layers and capture policies

The layers bottom-up: `World → WorldOverlay → Widgets → Panels → Popups → Modals
→ Drag → Toasts → Debug`.

| Policy | Clicks on the surface | Clicks past it (backdrop) | Examples |
|---|---|---|---|
| `Block` | intercepts everything | the surface contract (close/swallow) | settings, docs, the gallery, onboarding, stage, a dialog |
| `Capture` | by hit-rects | falls through below | the what-if bar, the palette dock, hotkeys, the minimap |
| `PassThrough` | does not intercept | falls through below | the world |
| `Passive` | no hit-rects | falls through below | toasts |

Popups (menus, flyout) — `Popups`, modals — `Modals`; a panel/bar — `Panels`.
The Esc ladder is derived from the registration order (`esc_stack`): register
the surfaces from the lower ones to the upper ones.

## 4. Layout primitives (`canvas_ui::layout`)

Immediate functions over the slot of the parent — they return the rects of the children:

- `Row { gap, main, cross, policy }` / `Column { gap, main, cross }` —
  linear layout; the children are `Child::fixed(w, h)`, the spacers are `Child::spacer`.
- `RowPolicy::Fit` — the overflow is NOT masked (the lint catches it);
  `RowPolicy::SqueezeTail` — a named degradation of a narrow slot (the tail
  is squeezed to zero width, is not picked) — the replacement for the silent `take`/`break`.
- `stack(slot, size, HAlign, VAlign)` — a fixed block in a slot
  (centering the modals, pinning the footer).
- `constrain(min, max, desired)` — a clamp of the size (modals, panels).
- `pad(slot, EdgeInsets)` — inner paddings.
- `Custom(rect)` — an escape hatch for the exotica (polar wheel, drop grid): only with
  a justifying comment; it falls into the G8 grep audit.

FR-062 (layout v2): the extensions of the same primitives — additions only:

- `Child.grow` / `Child::flexible(w, h, grow)` — the flex factor of the main axis
  (the free space of the slot is distributed proportionally to grow);
  `MainAlign::End` — pin to the end of the axis. Priorities: `SqueezeTail`
  beats grow; at Σgrow > 0 `SpaceBetween`/`End` degrade (grow
  consumes the free space); `grow = 0` — byte-for-byte the previous behavior.
- `MeasuredItem` + `Row::lay_out_measured(slot, items, m, fs, family, size)`
  — the size from the content: the width/height of the text from the TextMeasurer inside
  the layout (heuristics are impossible — the measurer is in the signature); the clamp of the width —
  only an explicit `max_w` (there is no silent cut — G5); ellipsis — the decision
  of the consumer. W3.3 (the pad semantics of F-13): `Text { pad_x, h }` —
  `pad_x` is the total horizontal pad added to the measured width
  (bit-for-bit ≡ the wiring `Fixed { w: width_of + pad_x }` — a chip/button =
  a single `Text` without manual measuring: whatif/template); `h: Some(constant)` —
  an explicit height as a design constant (the chip `CHIP_HEIGHT 26` ≠ the measured
  `13·1.3 = 16.9`), `None` — the measured one; bit-for-bit oracles in layout.rs.
- `RowPolicy::Wrap` — greedy packing into rows (the height of a row = the max of the children,
  the gap — on both axes, the cross — within a row); the number of visible rows is set by
  the HEIGHT of the slot, the overflow past the bottom edge is not masked (the lint G4).
- `grid_cells(slot, cols, rows, row_h, gap)` — a 2D grid of explicit columns
  (row-major); the spans — [`SceneNode::span`], the auto-computed tracks and
  minmax — FR-074 (below, the scene + the FlexLayoutEngine).
- The content focus: `kit::focus_order(rects, ring)` +
  `FocusRing::retain_order` (the rebuilding of the order with the position preserved)
  — the Tab navigation of a surface without manual indices; the frame — the `accent`
  slot (the TextField pattern of the gallery).

The gaps/radii — from the tokens of `canvas_core::tokens` (`SPACING_S/SM/MD/LG/XL`,
`RADIUS_CHIP/PANEL/PILL`) — the values are synchronized with
`design/tokens/dimensions.json`.
## 5. Measured text (`canvas_ui::measure`)

The widths for the layout — ONLY via `TextMeasurer` (the real cosmic-text shaping,
the same metrics as the screen texts of the render):

```rust
let mut measurer = canvas_ui::measure::TextMeasurer::new();
let mut fs = canvas_render::text::measure_font_system();
let w = measurer.width_of(&mut fs, label, canvas_render::text::SANS_FAMILY, 13.0);
let cut = measurer.ellipsis(&mut fs, label, FAMILY, 13.0, max_width);
let lines = measurer.wrap(&mut fs, paragraph, FAMILY, 13.0, max_w); // W3.3
```

- the measurer is created per relayout/frame (cheap; the cache within the frame);
- `FontSystem` — the ownership is the render's (`measure_font_system`), as an argument;
- the “characters × coefficient” heuristics and `chars.truncate` are forbidden (the class
  of the defect CR-015; the truncation — only `ellipsis` by the actual width);
- `wrap(fs, text, family, size, max_w)` (W3.3) — a greedy wrap by words
  (`split_whitespace`; a word wider than `max_w` — a separate line, without breaking
  across glyphs; an empty text → one empty line; the weight MEDIUM) — the replacement
  for the manual wrap loops of the consumers (the precedent `admin_ui::wrap_text`;
  the span-aware wrap of the docs viewer is a separate content engine, not a
  competitor).

**The Shaper trait (FR-068 W2, ADR-0015)**: cosmic-text is isolated behind
the `Shaper` trait boundary (`shape` + `font_system`) — the single point
of contact of the crate with the shaper (W4: the replacement of the shaper is a separate ADR; the code
of the consumers does not change). `CosmicShaper` — the default impl (the same pipeline
of shaping as the screen texts of the render — the metrics match, the CR-015 class
is protected by construction); `MockShaper` — a deterministic mock behind the feature
`mock-shaper` (only tests, not production: `TextMeasurer::new()` is always
the real shaping, the mock — only via `TextMeasurer::with_shaper`).
`TextMeasurer` internally goes through `dyn Shaper` — the public API for the consumers
did NOT change (`fs` — still an argument of the methods, the owner of the font pool —
`canvas-render::text`, PRD-0009 §14/Q6). W2: `TextMeasurer` is no longer
`Clone` (inside — `Box<dyn Shaper>`; there were no clone points in the repo).

## 6. Lints (CI)

- **G4** (`app::ui_layout_lint`, run by `cargo test --workspace`):
  a full registry frame over the canonical states × 3 windows (1280×800,
  1024×640, 800×560) × RU/EN — 0 intersections of the interactive rects of DIFFERENT
  surfaces of one band, 0 exits beyond the viewport; a Block modal covers
  the screen (the backdrop contract).
- **G5** — a grep audit of the migrated modules: 0 `take(`-slices, 0
  `break`-clamps of the layout, 0 character-based width heuristics.
- **G8** — 6 surfaces on the registry+primitives (search, settings, docs,
  the template palette, what-if, the gallery).
- The benign overlaps (a popup over a panel with “the upper opaque one hides the lower one”)
  — not a lint state: the canonical states do not combine the features; the real
  collisions (the hotkeys × the palette band, the settings modal × the band)
  were found and eliminated at U5 — see the PRD-0009 history.
- **G4+ at the crate level (FR-068 W0, `canvas-ui/tests/g4_lint.rs`, run by `cargo test -p canvas-ui`)**:
  5 canonical scenes (main canvas + whatif bar, palette dropdown, explain modal,
  search overlay, settings panel) × 3 windows × RU/EN = 30 runs: every visible
  element intersects its parent (the root — the viewport), the elements L4+ (Popups and
  above) intersect the viewport, 0 intersections of the interactive rects of one band
  (`overlaps_within_layer`), the hit-rects are inside the viewport. The widths are real
  (TextMeasurer), so the language affects the geometry. The number of runs is locked
  by the test `lint_covers_30_scenarios` (30).
- **The Painter.items snapshot tests (FR-068 W0, `canvas-ui/tests/snapshot.rs`)**:
  60 goldens — 10 kit components (Panel/Button/IconButton/Dropdown/Chip/Toast/
  Tooltip/Modal/TextField/Switch) × Normal/Hovered/Disabled × RU/EN;
  `Painter.items()` → a normalized dump (the rounding to a whole ui px,
  the sorting by `(x,y,w,h,type)`, the colors outside the dump — theme slots). The goldens —
  `canvas-ui/tests/snapshot/*.txt`; the comparison is exact string equality; a change —
  a deliberate PR with a diff (§9 Contract-9 of FR-068). Regeneration:
  `CANVAS_UI_UPDATE_SNAPSHOTS=1 cargo test -p canvas-ui --test snapshot`.
- **The HTML5 demo-goldens (FR-068 W1+W2, `canvas-ui/tests/html5_demos.rs`)**:
  the canonical list of 15 reference web layouts (ADR-0015 §Decision item 4)
  — `SceneNode` scenes, a dump of the rects of all the nodes in DFS pre-order. 10 W1
  (the mdn/css-tricks top-10: sticky-header, sidebar-overflow,
  navbar-space-between, grid-12-col, masonry-lite, aspect-ratio,
  modal-fixed-clip, dropdown-flip, virtualization, complex-form) + 5 W2
  CanvasDesk-specific ones: `11_cd_palette_grid_multiline`,
  `12_cd_whatif_bar_squeeze_tail` (historically — a double golden C3,
  W4: a single golden),
  `13_cd_kit_gallery_tab_focus`, `14_cd_search_overlay_viewport_clip`,
  `15_cd_fr061_tabular_body_grid`. The dual-backend run (W2): default Goldens —
  `canvas-ui/tests/html5_demos/*.txt`; regeneration
  `CANVAS_UI_UPDATE_HTML5=1 cargo test -p canvas-ui --test html5_demos`.
  A change of a golden — a deliberate PR with a diff.
- **The layout backends (FR-068 W1→W4, ADR-0014→ADR-0015)**: `trait
  LayoutBackend` + `NativeBackend` (1:1 the primitives of §4) + `FlexLayoutEngine`
  (our own engine, § below). **W4: taffy/TaffyBackend are cut out — the engine is one:**
  `default_backend()` → `FlexLayoutEngine`, `pilot_backend()` →
  `NativeBackend` (the API of the pilots is preserved). The consumer can choose
  a backend explicitly: `Row::lay_out_with(backend, slot, &items)` /
  `grid_cells_with(...)` / `lay_out_measured_with(...)`. The extended
  CSS capabilities (percent/fill/aspect-ratio/position absolute|fixed|
  sticky{top,left}|overflow/scroll-offset/grid tracks Auto|MinMax — FR-074)
  — the `SceneNode` scene + `FlexLayoutEngine::lay_out_scene`. The historical
  divergences from taffy (C3 `SqueezeTail` ≠ `flex_shrink`, unsafe `End`,
  round on freeze) — recorded in the Contracts of FR-068; the taffy parity oracles
  (backend_parity/flex_vs_taffy_parity, 10/10 bit-for-bit on the Auto/
  Minmax tracks of FR-074) were removed together with taffy.
- **Our own layout engine — the FlexLayoutEngine (FR-068 W2, ADR-0015)**:
  an in-house layout backend WITHOUT external dependencies (0 deps —
  the G7 zero-dep invariant; `layout/flex.rs`, the marker feature `flex-engine`
  in default). Contract: CSS flexbox (grow/shrink/basis/wrap — resolve
  flexible lengths §9.7 with freeze rounding) + `round-layout`
  snapping to an integer px grid (the positions — parent-relative round,
  the sizes — the round of the edges by the cumulative coordinates; the algorithm — a verbatim
  mirror of `taffy::compute::round_layout`, cut out in W4). The parity
  with taffy on the compatible policies was locked by the gate
  `tests/flex_vs_taffy_parity` (1000 trees, ≥ 80% bit-for-bit) — removed
  together with taffy in W4.
  `SqueezeTail` — VERBATIM (§Contract-4 of FR-068 “Flex decides”;
  the C3 divergence from the taffy `flex_shrink` — a historical note).
  The extended scene (`SceneNode`:
  percent/aspect/absolute/fixed/sticky/scroll/grid) —
  `FlexLayoutEngine::lay_out_scene` — the only oracle (W4: the scenes
  of the demo-goldens 16/16). The choice of the engine — `default_backend()` →
  `FlexLayoutEngine` always (W4 completed 2026-09-25). The grid tracks —
  FR-074: `Auto` (the content of the span-1 cells + §11.8 stretch) and
  `minmax(min, max)` (`TrackMin`/`TrackMax`, `max: Fill` — a fr with a floor;
  §11.5–11.8 in the taffy order, the parity 10/10 bit-for-bit BEFORE the taffy cut).
  The perf gate — `crates/canvas-ui/tests/perf_flex.rs` (below).
- **Perf-taffy** — removed in W4 together with taffy (the historical median
  264.8 μs — in the W1 worklog).
- **Perf baseline (FR-068 W0, `canvas-ui/tests/perf_baseline.rs`, `#[ignore]`)**:
  the reflow of a synthetic graph of 1000 nodes (Fit/flex/Wrap/SqueezeTail/grid_cells)
  — the median of 200 iterations, the gate < 1 ms (§Contract-8 of FR-068), a regression > 20%
  against `tests/perf_baseline.txt` — fail. The baseline is machine-dependent (a reference dev machine);
  the update — `CANVAS_UI_UPDATE_PERF=1 cargo test -p canvas-ui --test perf_baseline -- --ignored`.
- **Perf-flex (FR-068 W2, `canvas-ui/tests/perf_flex.rs`, `#[ignore]`)**:
  the same synthetic graph of 1000 nodes (Fit/Wrap/SqueezeTail/grid —
  SqueezeTail deliberately: Flex implements the policy verbatim, §Contract-4),
  but all the calls — through the EXPLICIT `FlexLayoutEngine`
  (`Row::lay_out_with`/`Column::lay_out_with`/`grid_cells_with`) —
  the median of 200 iterations, the gate < 1 ms (§W2: “the reflow of 1000 nodes < 1 ms on
  FlexLayoutEngine”), a regression > 20% against `tests/perf_flex_baseline.txt`
  — fail. ⚠️ The W2 stub may measure the Native delegation — the final
  baseline is regenerated by the lead at the integration of the wave. The update —
  `CANVAS_UI_UPDATE_PERF_FLEX=1 cargo test -p canvas-ui --test perf_flex -- --ignored`.
- **The scissor policy of the render (FR-056, F-5)**: each `ScreenBand` band
  carries a `UiRect` clip (log. px from the `SurfaceFrame.clip` of the registry frame);
  the render executes it with the scissor bucket of the band (one `set_scissor_rect`
  per band — R-1) and the `TextBounds` clip of the texts. Invariant: the scissor
  never expands the visible (the ceil/floor conversion + a clamp to the viewport),
  an empty clip — the band is not drawn. The migrating modules (FR-059/060)
  remove the manual clamps — the content is clipped by the system (G5).
## 7. Kit widgets (FR-055 U4, F-8)

`canvas_ui::kit` — a model of widgets over the primitives; the kit does NOT draw and does not intercept
the input (the layer/modality — only from the registry). The contracts:

- **the colors — slots only**: [`KitPalette`] (a slice of `ThemeColors` v2 + the state slots;
  the mapping — `canvas-render::theme`) or the explicit slots
  (`panel_style_of`/`control_style_of` — the migration of the surface canon,
  I-1 zero jump). Not a single color constant in the kit;
- **the paddings/radii** — only the spacing/radius-scale tokens;
- **the text** — only `TextMeasurer` + `ellipsis`; the per-character truncation —
  forbidden (the CR-015 class);
- the components: `Panel`/`Modal` (constrain+stack, dimming), `Button`
  (Primary/Secondary/Ghost/Danger × Normal/Hover/Pressed/Disabled/Selected),
  `IconButton`, `Chip`, `Dropdown` (an anchor + flip + a clamp into the viewport),
  `Toast` (bottom-center, TTL, an avoid bar), `Tooltip` (an anchor + flip + a delay
  of 500 ms); the animations — `canvas_ui::anim` (`BoolAnim`, dt-determinism).

A surface in 3 steps (§2) + the style from the kit: a widget returns a rect and
[`ControlStyle`]/[`PanelStyle`] — the consumer puts the quad/text into the band
of its layer. A live sample — the `kit_gallery` gallery (the menu “?” → “About the interface”):
the components × the states × RU/EN × the themes; the theme button in the header —
a real kit control (the switch of the theme changes the slots — the widgets
are redrawn by the same functions).

### 7.1 Painter and WidgetState (FR-057, wave 2)

The draw layer and the state machine of the widget moved from the consumer (`crates/canvas-app/src/kit_ui.rs`)
into the crate `canvas-ui` — the FR-058/059/060 migrations code against them, and not
copy the adapter:

- **`canvas_ui::paint`** — the `Painter` collects `PaintItem::Rect`/`PaintItem::Text`
  as DATA (the G7 invariant: without wgpu/winit, 0 external dependencies;
  the conversion into `CardInstance`/`OwnedText` is performed by the consumer crate).
  The order of the items = the draw order; `take_items()` hands out the accumulated and clears.
  `KitDraw` in `canvas-app` — a thin wrapper over the Painter (the methods/behavior 1:1,
  the equivalence of the quads/texts — the test `kitdraw_delegation_matches_direct_path`);
- **`canvas_ui::widget`** — `WidgetState`: the transitions of the pointer/selection/focus
  → `KitState` by a deterministic matrix of priorities
  **Disabled > Pressed > Hovered > Selected > Normal** + the click edge
  `clicked()` (“the press was inside, the release inside”; a press on disabled and a press
  outside the widget do not give a click). `cursor_state`/`dropdown_item_state` in
  `crates/canvas-app/src/kit_ui.rs` — deprecated delegates to `WidgetState` (the consumers migrate
  in FR-059/060);
- **`canvas_ui::keyboard::FocusRing`** — the Tab order of the focus-rects of a scope
  (`next`/`prev` around the ring, `current`, `clear`): the `KeyboardRouter` maintains
  the scopes of the SURFACES, the `FocusRing` — the focus of the content inside a surface
  (the frame via the `accent` slot — the decision of the consumer). The existing signatures
  of `crates/canvas-ui/src/keyboard.rs` were not changed (additions only).

```rust
let mut p = Painter::new();
p.control(btn_rect, &button_style(variant, state, &palette));
p.label(btn_rect, &label, style.text, 13.0, PaintAlign::Center);
for item in p.take_items() { /* конвертация в инстансы рендера */ }

let mut w = WidgetState::default();
w.set_pointer(hovered, pressed_now);   // каждый кадр
w.set_selected(is_on); w.set_focused(ring.current() == Some(&rect));
let style = button_style(variant, w.kit_state(), &palette); // Disabled>Pressed>Hovered>Selected>Normal
if w.clicked(released_inside) { /* действие один раз на press→release */ }
```

### DebugOverlay (F-10, G6)

The toggle — **F9** (native) / `?ui=debug` (web). From the frame of the registry it shows:
the outlines of the hit-rects with the caption “L3·Panels / settings / element” (the color by
the layer), the name of the surface/element under the cursor, the highlighting of the intersections
of the interactive rects of one layer (`overlaps_within_layer` — the mechanics
of the G4 lint at runtime). The overlay does not participate in the pick (there is no surface in the registry —
the diagnostics do not change the input); it is drawn by the band `UiLayer::Debug` (L8).
The model is pure — headless tests.

### 7.2 The components v2 (FR-058)

`canvas_ui::kit` (wave 2) — pure models/functions in the v1 style: geometry +
style + a model of the state; the drawing — via the Painter (FR-057), they do not
intercept the input, they do not own events. The components — only an **addition** to v1
(the existing signatures/constants are not changed).

| Component | Function | Contract |
|---|---|---|
| `TextField` | `text_field(slot, min, max, model, placeholder, focused, state, p, m, fs, family, size)` | The model `TextFieldModel { text, caret, sel }` + the layout `TextFieldLayout { rect, text_area, caret_x, text_shown }`. `caret_x = -1.0` — the caret is not drawn (not in focus). |
| List + scroll | `list_rows(area, s, row_h, gap, count) -> Vec<(usize, UiRect)>` + `scroll_bar(area, s, p) -> Option<UiRect>` | `ScrollState { offset, content_h, viewport_h }` — `scroll_by`/`clamp`/`needs_scroll`/`max_offset`. `list_rows` — a pure function (without mutations); the partial rows at the edges are included. |
| `Switch` | `switch(slot, on, state, p)` | `SwitchLayout { track, knob, track_style, knob_fill }`. `on` — the position of the knob (to the right) and the fill slot of the track (`control_primary` on / `control_fill` off); the radius `RADIUS_PILL`. |
| `Card` | `card(slot, min, max, header_h, p)` | `CardLayout { rect, header, body }`. The header and the body — inside the pad of the panel (`panel_style(p).pad` = `SPACING_LG`). |
| `Icon` | `icon_glyph(i) -> &'static str` + `icon_button(slot, icon, align)` | `enum Icon { Close, Gear, Question, Search, Plus, ArrowLeft, ArrowRight, Refresh }`. The glyphs — with the existing font (NotoSansDisplay-Medium): 0 new dependencies (G7). `icon_button` delegates to `icon_button_rect` (a square `ICON_BUTTON_SIZE`). |
| `Row` (FR-061 D-15, stage E) | `row_guides(m, fs, family, size, rows, right_edge, gap)` + `row_layout(..., slot, guides, parts, opts)` + `paint_row(p, lay, parts, style, size)` + `row_style(state, p)` + `leader_dash_rects(x0, x1, y, scale, min)` | A table row on the column guides ([`RowGuides`]): `RowParts` — declarative data (the marker `RowMarker::None/Dot/Glyph`, label, value, unit, badge); the right-pinning of the value/unit D-4, the leader D-5 (the dashes — a shared geometry with the body of the node), the badge — a pill. `RowOpts { leader, gap }` — the FR-044 panel without a leader. `RowStyle` — only slots (plain data: the consumer overrides the fields with the semantics of the surface — the marker/error/muting). The consumers: the gallery (the Row section), the FR-044 panel “How it is calculated”; the body of the node — `leader_dash_rects`. |

**The caret invariant** (fixed in the contract of FR-058): the positions of `caret`/`sel`
in `TextFieldModel` — in **CHARACTERS** (`chars().count()`), not bytes.
The insertion/deletion/movement are correct on unicode (the emoji are 4-byte, the Cyrillic
is 2-byte). The IME/UTF-16 conversion — on the input side of the consumer (it is tested
by `text_field_unicode_emoji_and_cyrillic_positions`).

**Non-goals** (moved into the assignment when a consumer appears): `Slider`
(a speculative component without a screen with a slider). The `kit_gallery` gallery
is updated in FR-059 (the owner of wave 1 of the migration).
### 7.3 The gallery: the composition after FR-059 (wave 1 of the migration)

The `kit_gallery` gallery shows the v1 sections (Buttons ×4 variants ×4 states,
IconButtons, Chips, Dropdown, Toast, Tooltip) + the sections of the v2 components:
**TextField** ×3 (Normal / Focused — the caret by `caret_x` / Disabled with a
placeholder), **Switch** ×4 (Off/On × Normal/Hovered/Disabled), **Card**
(the header + the body), **list+scroll** (8 rows, 3 in the window, a selected row,
a demo shift — the thumb in the track), **Icon glyphs** ×4 (Search/ArrowLeft/
ArrowRight/Refresh). The content of the gallery is above the maximum panel — the column
of the sections scrolls (`ScrollState`, the wheel over the content, the header is fixed;
at the offset 0 — the previous layout verbatim). The surfaces of wave 1
(`hints_ui`, `flowmap_ui`, `calc_panel_ui`) are migrated to
`dropdown_menu`/`stack`/`list_rows`+`ScrollState`/`scroll_bar`, the states
of the rows — `WidgetState`, the drawing — the Painter (§7.1).

FR-062 adds the sections of the layout v2: a **measured row** (3 chips — the widths from
the TextMeasurer inside the layout), a **flex row** (fixed + grow ×2 + grow ×1),
a **wrap row** (8 chips, a greedy packing into the rows of the slot), a **grid 4×2**
(grid_cells) and **focus slots** ×4 — the Tab/Shift+Tab drives the FocusRing (the frame —
the accent; the ring lives in the content coordinates `focus_targets`, the rebuilding —
`retain_order`). The tail of the gallery at the bottom scroll — the FR-062 sections.

FR-061 (stage E, D-15) adds the section **Row** — a demo table of 4 rows on
the shared guides (a parameter ×2 / the formula “ƒ” with a badge “← source” / Σ),
the states Normal/Zebra/Selected; the drawing — `paint_row` (the same functions
of the kit as the panel “How it is calculated” and the body of the node).

## 8. The UI admin panel (FR-070)

The surface `admin_panel` (Modals/Block, the entry — the menu “?” → “UI console”):
a sidebar of the sections + a demo zone (the Storybook pattern). The sections:

- **Components** — an extended matrix of the states: buttons 4 variants × 6
  states (ST1 + Focused, an accent frame), the icon buttons/chips × ST1, the fields
  Normal/Focused/Error/Disabled (Error — the primitive `ERROR` from design/tokens),
  the switches.
- **Content** — 9 containers × empty/medium/full: a list, a card,
  a field, chips, a dropdown, a table kit-Row (the guides + Σ), toasts, the template
  palette, the wheel (the slots `WHEEL_*`, simplified quads).
- **Canvas** — the entities ST4: a card of a node ×4 states, the edges ×4, the ports ×3.
- **Tokens** — a catalog: 14 slots of `KitPalette` (a swatch + id + hex; a click —
  the next candidate, a live application to the whole panel until “Reset”),
  the sizes/typography/motion — read-only.

The model — `crates/canvas-app/src/admin_ui.rs` (the pure layouts + the draw adapters
over the Painter/KitDraw), the integration — the pattern of the FR-055 gallery (the registry, Esc/
backdrop/wheel, the G4 lint state `admin_panel`). The live overriding —
`App::admin_palette_override` (the reset by the button “Reset”/by a change of the theme; the saving
into the config — v2). The documentation — `user-docs/admin.html`, the step 9 of the onboarding.

## 9. The status of the kit

- Done (U1–U3, U5): the skeleton of the layers/registry, the primitives, the TextMeasurer, the lints,
  the migration of 6 surfaces, the KeyboardRouter on the whole ladder `on_key`.
- Done (U4, FR-055): the kit widgets F-8 (`canvas_ui::kit` + `anim`),
  the DebugOverlay F-10 (G6 closed), the gallery `kit_gallery`, the chrome of the pilots
  (the what-if container/pill, the gallery “✕”) — via the kit styles (the explicit slots).
  The scissor buckets (F-5) and the measurement of the wasm gain — at the next web build
  (the G7 remainder); the TextInput of the kit — v2/beyond the PoC.
- The decision on the layout engine taffy — **ADR-0013** (2026-09-23): we do not plug it in
  (the measurement: flexbox-only ≈ 108.5 KB of wasm = the whole G7 remainder of the wave; the grid ≈ 376 KB);
  the kit is reinforced with its own primitives — the assignment **FR-062**
  (the measured children, the flex factors, Wrap, grid_cells, the focus bundle,
  the geometric snapshots); the escalation triggers T1–T4 — in ADR-0013.
- Done (FR-062, 2026-09-23): the layout v2 — the measured children (F-13),
  the flex factors + `MainAlign::End` (F-14), `RowPolicy::Wrap` (F-15),
  `grid_cells` (F-16), the focus bundle FocusRing × WidgetState (F-17),
  the geometric snapshots (F-18); the gallery — 5 sections of the layout v2 +
  the Tab navigation; taffy is not plugged in (ADR-0013, the triggers T1–T4).
- Done (FR-060, 2026-09-23): the migration wave 2 completes the transfer
  of the custom UI — autolink (`kit::modal`/`list_rows`+`ScrollState`/`stack`),
  palette (`kit::dropdown_menu` — the bar and the columns; the anchor row 6 = 10−4,
  the anchor zone of the bar ±1; the flip of the bar at the bottom edge), explain (`kit::modal`
  with an inset slot; take_while/match instead of the break clamps); app.rs:
  the confirmation dialog — a measured geometry (kit::modal + button_size —
  a fix of the class of the defects “fixed geometries”, the ellipsis instead of
  the silent clip), the menu items/hotkeys — `list_rows` + `WidgetState`;
  autolink_frame/palette_overlay — Painter + WidgetState (the panels with a shadow —
  quads: params.w is outside PaintItem); the G5 audit of the three modules is clean;
  the G4 lint: +3 states (autolink/explain/palette via `test_viewport`);
  the final measurement of the wave: +1 229 B ≈ 1.2 KB (≤ 100 KB). Deliberately
  left hand-rolled (the world decorations/out of the scope): wheel/minimap/HUD;
  explain_frame — the drawing on screen_rect_quad (an equivalent of the
  Painter conversion, without the shadows) — the remainder of the wave.
- Done (W-e, 2026-10-03): the onboarding — **unfrozen by the decision of the owner 03.10.2026**
  and migrated to the kit (the status «заморожен владельцем» is removed from
  the list of the hand-rolled remainders above). The geometry — `kit::modal` over the slot
  of the viewport with the margins `SPACING_LG` (the clamp of the card with the margins — the audit §8 item 12;
  at the clamp of the height the body is squeezed by the scroll `kit::ScrollState`/`list_rows`,
  the footer with the CTA is always visible); the wrap of the body — `TextMeasurer::wrap` (the replacement
  of the heuristic `CHAR_W_FACTOR 0.62` — the onboarding row in §9 of CR-015);
  the drawing — Painter + `WidgetState`/`button_style` (the replacement of the ad-hoc
  `hover_fill` ×1.3); the colors — only the slots of the theme (`stage_dim`, the slots
  of `button_style`, `panel_border`/`link` — the former 6 draw literals);
  the hit-rects of the card/buttons — from the same functions as the drawing.

## 10. The component layer (FR-068 W3, ADR-0015)

Done (W3, 2026-09-25): `canvas-ui` received a model of components —
`crate::component` (`component/mod.rs`): the trait `Component`
(`type Props`; `props()`; `layout(backend, slot) -> Vec<UiRect>`;
`paint(painter, rects)`; `hit_test(rects, point) -> Option<ComponentHit>`
— the default: the first rect by `UiRect::contains`). The implementation was moved from
`crates/canvas-ui/src/kit.rs` (now a thin facade-reexport — the public API of the kit is 1:1,
the consumers are not rewritten, §Contract-1 of PRD-0009 V-5):

| Module | Component | State | Notes |
|---|---|---|---|
| `component/button.rs` | `Button` | `WidgetState` | paint — `button_style(kit_state())`; the disabled is wired in `new` |
| `component/panel.rs` | `Panel` | — (non-interactive) | layout → `[panel, content]` |
| `component/dropdown.rs` | `Dropdown` | `WidgetState` | layout → `dropdown_menu().menu`; flip/viewport_clamp inside |
| `component/modal.rs` | `Modal` | — | layout → `[dim, panel]` (the contract of the indices, a test); paint — the panel only; hit_test: panel→1/dim→0 |
| `component/text_field.rs` | `TextField` | `WidgetState` + `TextFieldModel` | paint — the container (the text/caret — the consumer: the determinism of the measurement) |
| `component/list.rs` | `List` | `ScrollState` | layout → `list_rows`; paint — the scrollbar |
| `component/row.rs` | `Row` | `WidgetState` | the FR-061 row; the layout parity with `row_layout` (a test); the paint via `paint_row` |
| `component/table.rs` | `Table` | `ScrollState` + `Vec<TableRow>` | FR-068 Table v2: a table on the SHARED guides (the pass A max-per-column + the pass B from the right edge — the column of the values is stable when scrolling); the kit re-export (`Table`/`TableRow`/`TableRowStyle`/`TableOpts`/`TableProps` over the kit-Row v1 — Row/List are not changed); the implicit columns, the degradation of the empty right columns, the clip semantics of the visibility window (the truncation by the intersection); two layers of the measurement (`*_with` — an external measurer, the component one — its own; the parameter of the guides `viewport_right` — the right edge of the viewport in the coordinates of the slots); granularly `paint_rows_with`/`paint_scrollbar`; the bit-for-bit oracles T1–T6 (≡ the kit `row_guides`/the manual loop). The design — `docs/plans/fr-068-table-v2.md` |

- **Retained-state** — NOT introduced (the profile W2: the reflow of 1000 nodes ~0.2 ms <
  the threshold 1 ms — KISS, the decision is fixed in FR-068). `Row` keeps
  `TextMeasurer`/`FontSystem` retained (once per component).
- **The migration of the consumers** — staged (the catalog
  `docs/plans/fr-068-w3-consumer-migration.md`): the pilot W3.1 — the what-if bar
  (`crates/canvas-app/src/whatif_ui.rs`) is translated to the measured-API (`MeasuredItem` ×
  `Row::lay_out_measured`, bit-for-bit); W3.2 (2026-09-25) — settings_ui,
  scheme_gallery_ui, template_ui, search_ui → the measured-family
  (including the NEW `Column::lay_out_measured/_with` — the vertical
  symmetric analog of F-13); `Child::fixed` in canvas-app/canvas-render
  42 → 1 (the kit_ui demo F-14 — the decision W3.3 is with the owner).
- **Table v2** (the FR-068 W3 continuation, 2026-09-26) — the component of the tables
  on the shared guides (`component/table.rs`, the kit re-export; the design +
  the bit-for-bit oracles — `docs/plans/fr-068-table-v2.md`). The migration
  of the consumers M1–M6 of the design is executed (the panel “How it is calculated” stage —
  2×Table, the demo tables of the gallery/admin panel; the place of the overlays — the audit: not
  a candidate — the paint half of the demo table of the gallery is already on the guides
  `kit_ui::gallery_layout`, see §8 of the design).
- **The known issues (the W4 revision)**: the 2 taffy tests of whatif_ui, noted
  in W3 as the pre-existing red, went behind the feature `taffy` and were removed together
  with it (W4) — the family of the taffy roundings no longer exists;
  the native oracles (`bar_layout_no_overlap_and_covers_labels` etc.) are green.
