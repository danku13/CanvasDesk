# 11 — Layouts and adaptivity

> The single normative source for UI layouts (closing the 2026-10 audit
> gap: this folder had no layout rules — the fragments lived in П7/П10
> and S6). Sources: `crates/canvas-ui/src/layout.rs` + `layout/{flex,scene}.rs` (FR-053/FR-062/FR-068/FR-074),
> ADR-0013/0014/0015, `docs/ui-kit.md` §4/§6, CR-014 (the web shell), FR-097 (touch
> targets), FR-099 (safe-area). The primitives' semantics — modern web
> standards: CSS Flexbox (§9.7), CSS Grid (§11.5–11.8), container-based
> layout, `clamp()`, aspect-ratio, sticky, `@media`/`pointer: coarse`, safe-area. The rule sees
> the code, not rewriting it (I-1): the values already equal the current
> constants.

## LAY1. Slot — the only source of geometry

Immediate-mode layout from the parent's slot: every frame the consumer
passes the slot (the parent rect) and the list of children — the
primitives return the computed rects. Nothing is stored or drawn between
frames (D2, ADR-0013).

- **LAY1.1** The geometry of any child is derived from the parent slot,
  not directly from the viewport. The viewport enters the layout only via
  two paths: as the root slot of a surface and as the degradation
  breakpoint (LAY8). "Layout from the window" deep inside the tree is
  forbidden — it is insensitive to surface movement and breaks the G4
  lint.
- **LAY1.2** Input = what is visible (П5): hit-rects are built from **the
  same** layout results. A second, "manual" computation of the geometry
  for hits is forbidden — a draw/hit divergence is not caught by the lint
  and will always drift sooner or later.
- **LAY1.3** Coordinate cascade: World px — the canvas (scaled by zoom),
  screen/ui px — the chrome (panels, HUD; П11). The `canvas_ui::layout` primitives operate
  in ui px; mixing scales inside a single layout tree is forbidden.

## LAY2. Primitives and their web equivalents

The primitive is chosen from the table. A new layout must be expressed
with these primitives; "manual" arithmetic `x += w + gap` outside `layout/` is forbidden
(the exception is `Custom`, LAY5).

| Primitive (`canvas_ui::layout`) | Web equivalent | Use |
|---|---|---|
| `Row { gap, main, cross, policy }` | `display:flex; flex-direction:row` | toolbars, control rows, chip strips |
| `Column { gap, main, cross }` | `display:flex; flex-direction:column` | panels (header→body→footer), forms |
| `Child::fixed(w, h)` | a fixed flex item (`flex: 0 0 auto`) | buttons, fields, constant blocks |
| `Child::flexible(w, h, grow)` | `flex-grow` with `flex-basis` = the base size | free-space fillers |
| `Child::spacer(len)` / `MeasuredItem::Spacer` | a fixed gap block | named skeleton spacers |
| `MainAlign::{Start, SpaceBetween, End}` | `justify-content` | press a row to the right edge, distribute |
| `CrossAlign::{Start, Center, End}` | `align-items` | vertical centering of controls of different heights |
| `RowPolicy::Wrap` | `flex-wrap: wrap` | category chips, tags, filters |
| `grid_cells(slot, cols, rows, row_h, gap)` | `grid-template-columns` (equal explicit) | showcases, card galleries |
| `stack(slot, size, HAlign, VAlign)` | absolute center / pinning a fixed block | centered modals, empty cards |
| `constrain(min, max, desired)` | `clamp()` (min takes priority over max) | modal and panel sizes |
| `pad(slot, EdgeInsets)` | `padding` | container inner padding |
| `MeasuredItem::Text { pad_x, min_w, max_w, h }` | `width: max-content` + explicit min/max | text-driven auto sizing (chips, buttons) |

Selection rule: two axes — `Row`/`Column`; a regular grid — `grid_cells`;
overlap/centering — `stack`; size constraining — `constrain`; inner padding — `pad`. The
combination covers all regular layouts of the kit; `SceneNode` (LAY5) — when
percent/aspect/sticky is needed.

## LAY3. Main-axis overflow policies

Overflow is never silently masked (the CR-015 class). Three named `RowPolicy`
policies:

| Policy | Web analog | Semantics |
|---|---|---|
| `Fit` (default) | `overflow: visible` | children fill the slot; the spill beyond the slot is visible and caught by the G4 lint |
| `SqueezeTail` | a named degradation (NOT `flex-shrink` — the C3 divergence is documented) | each child gets `min(desired, остаток)`; the tail is squeezed to zero width and is not picked up (half-open hit) |
| `Wrap` | `flex-wrap: wrap` | does not fit into the row — wrap; the row height = max of the row's children heights; rows below the slot are not masked (caught by G4) |

- **LAY3.1** `Fit` — the default, and always when overflow is a content
  defect rather than expected behavior. A silent clip of a narrow slot
  (`break`/`take` clamps) is forbidden by the G5 lint.
- **LAY3.2** `SqueezeTail` — only for left-anchored bars where the right tail
  (chips, hints) is semantically secondary: the what-if bar, status rows.
  The policy takes priority over the flex factors: the `grow` of children is
  ignored under it.
- **LAY3.3** `Wrap` — for sets of homogeneous chips/tags. The number of
  visible rows is set by the HEIGHT of the slot, not by a policy
  parameter; rows spilling past the bottom edge are a defect signal (G4),
  not masked.
- **LAY3.4** Text truncation — only explicit: `max_w` in `MeasuredItem::Text` + ellipsis/clip
  by the consumer. A silent clamp of the width to the slot remainder is
  forbidden (G5, П6).

## LAY4. Flex factors (free-space distribution)

`Child.grow` — the share of the slot's free space (after the fixed children, the
flex children base sizes and the gaps) distributed proportionally to the
factors — verbatim CSS flexbox §9.7 "Resolving Flexible Lengths" with `flex-basis`
= the base size; `FlexLayoutEngine` additionally snaps the main sizes to an integral px
grid per the taffy `round_layout` (tolerance ≤ 0.5 ui px, documented in tests).

- **LAY4.1** Each main axis has at most one grow filler child (the analog
  of a `flex: 1` spacer). Several grow children with different factors — only
  when the shares are meaningful (panels 2:1), not "so that something
  stretches somehow".
- **LAY4.2** Priorities (pinned by tests): `SqueezeTail` > `grow` distribution; at
  Σgrow > 0 the free space is consumed by the grow children — `SpaceBetween`/`End`
  degrade to the base gap/alignment (documented, not a defect).
- **LAY4.3** `grow = 0` — byte-for-byte the former fixed behavior (the
  "addition-only" invariant FR-062). Existing layouts do not change a
  single rect when flex factors are added.

## LAY5. Grid and the extended scene (SceneNode)

Two levels of expressiveness. Escalation — as needed, without rewriting
consumers (the FR-068 contract §Contract-1: the primitive signatures are
stable).

**Level 1 — primitives (the default):** `Row`/`Column`/`grid_cells`/`stack`/`constrain`/`pad`. `grid_cells` —
equal explicit columns, row-major (the guide widths are set by the call).

**Level 2 — the `SceneNode` scene (FR-074, capabilities beyond the primitives):**

| Capability | Web analog | When |
|---|---|---|
| `SceneDim::Percent` | `%` of the parent's content-box | width/height fractions |
| `SceneDim::Fill` | `flex: 1` / `align-self: stretch` | flexible scene tracks |
| `aspect` | `aspect-ratio` | previews, minimap, icon slots |
| `ScenePosition::Absolute` | `position: absolute` (from the parent's border-box) | overlays inside a container |
| `ScenePosition::Fixed` | `position: fixed` (from the root slot) | tooltips, dropdowns at the edge |
| `ScenePosition::Sticky { top, left }` | `position: sticky` (each axis independent) | table headers during scrolling |
| `SceneOverflow::Hidden` | `overflow: hidden` (does not change rects; the clip is `PaintItem::ClipRect`, FR-056) | list windows, scroll containers |
| `SceneNode::offset` | scroll content-shift | scrolled containers |
| Grid tracks `Auto/Length/Percent/Fill/MinMax{min,max}` + `span` | CSS Grid §11.5–11.8 (`minmax()`, `fr`, auto computation from the max-content of span-1 cells) | irregular grids, masonry-lite |

- **LAY5.1** Escalation rule: if the primitives cover the need, the scene
  is not used. The scene is used when
  percent/aspect/sticky/auto-tracks/minmax/scroll-offset is needed.
  Exotics outside both models (the polar coordinates of the wheel menu —
  see "Exceptions", drop grids) — `Custom(rect)` **with a justifying comment**
  (caught by the G8 grep audit).
- **LAY5.2** Before the extended policies the consumer checks the engine's
  `LayoutFeatures` mask (`FLEX_GROW/FLEX_SHRINK/FLEX_BASIS/FLEX_WRAP/
  GRID_2D/AUTO_SIZE/OVERFLOW_CLIP/PERCENT/ASPECT_RATIO/STICKY`) — the code must not silently assume a capability the
  engine does not have.
- **LAY5.3** `SqueezeTail` — NOT `flex-shrink`: it is a named degradation (LAY3.2). The CSS
  flex-shrink semantics are not provided by the engine; needing it is a
  signal to rethink the layout (a fixed tail → Wrap).

## LAY6. Measured text in layout

Widths for layout — only via `TextMeasurer` (П6): real cosmic-text shaping, the same
(family, weight) pair as the renderer.

- **LAY6.1** Auto-sizing of a text child — `MeasuredItem::Text`: the width from
  measurement, `pad_x` — the total horizontal padding (bit-for-bit ≡ the
  wiring of `Fixed { w: width_of + pad_x }` — a chip/button without manual measurement), `min_w` — the
  width floor (a short caption is raised to the minimum), `max_w` — the only
  clamp (G5), `h: Some(constant)` — an explicit height set by a design constant (chip
  `CHIP_HEIGHT`), `None` — measured (the font size · line factor of the string).
- **LAY6.2** Manual per-character widths (`chars.len() × constant`) and eyeballed fixed widths
  for a specific string are forbidden — they break on the first
  localization (RU/EN is run through the G4 lint).
- **LAY6.3** Measurement happens BEFORE the backend in all engines (in the
  `lay_out_measured` signature) — the result does not depend on the backend choice.

## LAY7. Gaps and paddings — only from the scales

- **LAY7.1** `Row.gap`/`Column.gap` and `EdgeInsets` — values from the S1 scale only (`SPACING_S 6 / SM 8 / MD 10 / LG 12 / XL 24`);
  literal gaps in calls are forbidden (the G5/G8 audit). Half-steps (7, 9,
  11) do not exist. The neutral zero gap `gap: 0.0` — not a member of the S1
  scale; it is allowed only as the documented "rhythm without gap"
  exception (see "Exceptions").
- **LAY7.2** Selection rule: the inner gaps of controls — S/SM; the
  padding of containers — MD/LG; the air from the viewport edges — LG/XL
  (the "surface ↔ edge" pairs — the S4 table).
- **LAY7.3** The vertical gap in a `Column` — `MeasuredItem::Fixed
  { w: 0.0, h: <зазор> }`, not `Spacer`: a spacer in a
  column takes no space on the main axis (pinned by the test `measured_column_matches_manual_fixed_oracle`).

**Exceptions (documented deviations from LAY7.1):** the command palette
(`crates/canvas-app/src/palette.rs`) — `PAL_GAP 5 / PAL_DROP_PAD 5 /
PAL_DROP_GAP 3 / PAL_CAPTION_PAD 2` — the values are frozen by the F-7.9 CSS prototype (list
density; micro-values below the scale). Each constant in the code is
annotated with a comment referencing this item. New off-scale values
without adding them to this list are forbidden.

## LAY8. Adaptivity: viewports, breakpoints, degradation

The degradation order when a slot/window narrows — strictly named:

1. **Fluid** — grow children and `Fill`/percent tracks shrink first (the free
   space runs out);
2. **Named policy** — `SqueezeTail`/`Wrap` on the main axis; ellipsis on text (LAY6.1);
3. **Surface degradation** — `DegradationPolicy::HideBelow { min_w, min_h }` (П10): a window smaller than the minimum
   → the surface is hidden WHOLE, not squeezed into unreadable mush (the
   what-if bar < 900×600);
4. **No intermediate steps** — "half-hidden", translucent and
   "squeezed-to-unreadability" states are forbidden.

- **LAY8.1** Canonical gate viewports: **1280×800 / 1024×640 / 800×560**
  (the G4 lint: 0 intersections of interactive rects of different surfaces
  of the same band, 0 exits beyond the viewport, 0 text overflows; ×
  RU/EN). The web audit of the shell — additionally 7 viewports 375–1920
  (CR-014). The breakpoint pair **1280 / 768** — not gate viewports but
  intra-surface adaptation of the settings modal (LAY-W5): `SETTINGS_BP_COMPACT`/`SETTINGS_BP_MOBILE` (`crates/canvas-app/src/app/ui_registry.rs`,
  the single declaration next to the surface declaration — LAY8.2; `settings_ui`
  consumes them via the re-exported `MODAL_BP_*`): ≥ 1280 — a two-column layout,
  768…1279 — a single column with a horizontal tab bar, < 768 — a
  fullscreen sheet. The boundaries are pinned by tests; the 1280 threshold
  coincides with the canonical gate viewport 1280×800.
- **LAY8.2** The DOM shell: `@media (max-width: 1199px)` — the toolbar to the bottom-left corner;
  `@media (pointer: coarse)` — touch targets. The GPU-UI breakpoint equivalent — `HideBelow` in `SurfaceRegistry`
  (LAY8, item 3); the breakpoint must be registered where the surface is
  drawn, not in the drawing body.
- **LAY8.3** Touch targets: on a coarse pointer the hit-rect is stretched
  to **44 logical px** on both axes (`MIN_TOUCH_TARGET`, FR-097), centered, clamped into
  the container; the visuals do not change, the desktop stays bit-for-bit
  the same. A new interactive zone < 44 px must go through the hit-only
  expansion.
- **LAY8.4** Safe-area: the DOM shell — `viewport-fit=cover` + `max(8px, env(safe-area-inset-*))` on pinned elements
  (FR-099). GPU panels keep margins from the edges ≥ 8–16 px (S4) so that
  a notch/gesture bar does not cover the content; passing env() into the
  GPU-UI — only via a new AppEvent channel (an owner decision), not via
  CSS parsing at runtime.

## LAY9. Layout engines and parity

- **LAY9.1** The single contract — `trait LayoutBackend`: `lay_out_row/column/
  measured/grid` + the `features()` mask. The consumer
  writes `Row { .. }.lay_out(slot, &children)` and does not know who computes the rects; the explicit choice
  — `lay_out_with(backend, …)` (pilot surfaces, tests).

  Status **"parity oracle"** (LAY-W18): the `pilot_backend()` calls in non-pilot
  consumers — `kit_ui.rs` (the showcase ruler, W3b), `crates/canvas-app/src/app/agent_panel.rs` (`AgentPanelLayout::build`, W4), `admin_ui.rs` (the
  demo-section bodies, W13) — are not pilot escalation but a deliberate
  bitwise-parity oracle with the former manual arithmetic: NativeBackend
  computes in pure f32 without the integer rounding of `finish_line`
  (FlexLayoutEngine — rounding with a tolerance ≤ 0.5 ui px, LAY9.3), and
  the golden tests of these surfaces pin the geometry with a 0.005 px
  tolerance — so a 0.5-px drift would break the goldens without changing
  the layout decision. This is acceptable as a temporary state: each call
  is annotated with the anchor comment `// LAY9.1: паритет-оракул …` (a grep anchor for audits).
  **Removal plan:** moving a consumer to `default_backend()` goes in a single commit
  together with regenerating its golden baselines (tolerance ≤ 0.5 ui px,
  a deliberate diff) and deleting the same-named comments; a silent
  backend switch without regenerating the baselines is forbidden.
- **LAY9.2** `default_backend()` → `FlexLayoutEngine` (an own engine, the zero-dep invariant G7,
  ADR-0015); `pilot_backend()` → `NativeBackend` (a 1:1 port of the primitives, the oracle of the
  old semantics). taffy was cut out (W4); returning an external engine —
  only via an ADR.
- **LAY9.3** Documented divergences of the semantics (not defects): `SqueezeTail` ≠
  `flex-shrink` (C3); `MainAlign::End` under overflow — unsafe pushing past the start edge (no
  fallback); rounding on freeze ≤ 0.5 ui px. A new backend must reproduce
  the `SqueezeTail` policy verbatim (§Contract-4 of FR-068) and pass the shared
  golden baselines.
- **LAY9.4** Web-parity oracles — the HTML5 demo-goldens (`crates/canvas-ui/tests/html5_demos.rs`, 15
  baselines: sticky-header, grid-12-col, masonry-lite, aspect-ratio,
  modal-fixed-clip, … + 5 CanvasDesk-specific ones). A new CSS pattern in
  the kit: first a baseline in html5_demos (a dump of rects), then usage
  in a consumer. Changing a baseline — a deliberate PR with a diff
  (regeneration via `CANVAS_UI_UPDATE_HTML5=1`).

## LAY10. Guidelines: typical skeletons

Proven patterns on top of the primitives (they match the 15 golden
baselines):

- **Panel** — `Column`: `PanelHeader` (a fixed S3 height) → the body (grow 1 or a `list_rows`
  window + `SceneOverflow::Hidden`) → the footer/actions (fixed). Gap — SPACING_SM/MD,
  container padding — SPACING_LG (S1).
- **Toolbar/action row** — `Row { gap: SPACING_SM, main: End }`: content on the left, actions on the right;
  pressing to the right edge — `MainAlign::End`, not "a spacer + SpaceBetween".
- **Form/settings** — a `Column` of fixed rows: a label (measured) + a control;
  the cross-alignment of controls of different heights — `CrossAlign::Center`; section
  separators — `Spacer(SPACING_MD)`.
- **Modal** — `stack(center)` + `constrain(min, max)`: fixed bounds from the scales, centering from the
  viewport slot; dimming — `stage_dim` (the L4 band).
- **Showcase/gallery** — `grid_cells` with guide widths from `RowGuides`/measurement; the
  overflow of "extra" cards — a new grid row, not clipping.
- **Chip row** — `Row { policy: Wrap, gap: SPACING_S }`; chip selection — ChipStrip (K1), not hand-made
  chips.

**Anti-patterns (forbidden):**

- manual arithmetic `x += w + gap` outside `layout/` without a `Custom` justification;
- silent clamps/clips (`take(`, `break` clamps, per-character widths) — G5;
- layout from the viewport deep inside the tree (LAY1.1);
- an own scroll model (`offset + ручной кламп`) bypassing `ScrollState` (K3);
- gaps/heights outside the S1/S3 scales;
- flex factors under `SqueezeTail` (ignored by the policy — LAY3.2);
- a second geometry computation for hits (LAY1.2);
- fit overflow "hidden" under a clip (`SceneOverflow::Hidden`) instead of fixing the policy:
  the clip is for list windows, not for masking a defect.

## LAY11. New-layout checklist

Before pushing a new surface/panel:

1. Does the slot come from the parent (LAY1.1), the viewport — root only?
2. Is every container expressed with a LAY2 primitive / the LAY5 scene /
   `Custom` with a justification?
3. Are gaps and paddings from the S1/S4 scales, heights from S3?
4. Is all text measured (`MeasuredItem`/`TextMeasurer`), the clamps explicit?
5. Is overflow a named policy (Fit/SqueezeTail/Wrap), not masked?
6. Is the G4 lint green on 3 viewports × RU/EN?
7. Is there a `HideBelow` minimum if the panel is unreadable below it?
8. Are the hit-rects from the same layout rects (LAY1.2)?
9. On a coarse pointer, are the targets ≥ 44 logical px (LAY8.3)?
10. Is the UI-console showcase section updated (K4), the component added
    to K1?

## Exceptions (documented deviations)

The list is EXHAUSTIVE: below are all documented deviations of surfaces
from the letter of the LAY1–LAY11 rules, accepted by the owner based on
the 2026-10 layouts audit (`design/layouts-audit-2026-10.md`). The I-1 principle — the rule sees the
code, not rewriting it: an exception records an existing deliberate
decision, it does not sanction future improvisation. Anchors — names of
functions/constants/structs + a file; line numbers are not recorded (the
files grow), verification — with grep against the current HEAD. A new
exception is introduced ONLY by editing this section (a table row + an
explanatory item, in the commit together with the deviation code); a
silent deviation is a rule violation, not a "minor deviation". The
command-palette exception (`palette.rs`, `PAL_*` — frozen by the F-7.9 CSS prototype,
the LAY-W2 wave) is recorded inline in LAY7.1 and is not moved into the
table; the LAY7.1 inline block and this section mutually exclude
duplication — a new off-scale value is registered in one place.

| Exception | Files/anchors | Deviates from | Rationale | Status |
|---|---|---|---|---|
| Explain — tidy tree | `explain_ui.rs`: `layout_tree`, `fit_scale`; render — `crates/canvas-app/src/app/explain.rs` | LAY2, LAY10 | the 2D tidy layout of the explanation tree — algorithmic layout of a single object (the lineage tree), not a regular flex/grid case | Accepted (FR-083; the `explain_ui.rs` header) |
| Docs — GFM blocks/tables | `docs_ui.rs`: `layout_page` (`gfm::parse_blocks_opts(_, true)`), `layout_table` | LAY2, LAY10 | the markdown content rendering of the docs viewer (FR-027), not a UI skeleton: "cells of one row from one y" — outside the `list_rows`/`Table` contract | Accepted (FR-027; the `docs_ui.rs` header) |
| Wheel menu — polar geometry | `template_ui.rs`: `wheel_geometry`, `sector_point`, `WheelGeometry::hit`, `WHEEL_*`; render — `crates/canvas-render/src/sectors.rs` (SDF, `sectors.wgsl`) | LAY2, LAY5.1 | the radial arrangement of templates/categories is not expressible with either the primitives or the scene; the shape and the hit are one polar math (`sectors::angle_gap` — the WGSL mirror) | Accepted (FR-022; AGENTS.md §"Exceptions") |
| `gap: 0.0` — "rhythm without gap" | `settings_ui.rs` `modal_layout_with` (4 places), `scheme_gallery_ui.rs`/`search_ui.rs` `layout_with`, `template_ui.rs` `panel_layout`, `kit_ui.rs` `gallery_layout`, `crates/canvas-app/src/app/stage.rs` `paint_calc_panel_rows` — all `gap: 0.0` / `TableOpts.row_gap: 0.0` | LAY7.1 | 0 is outside the S1 scale; a deliberate zero gap: the vertical rhythm is carried by row heights and spacers, not by `gap` | Accepted (2026-10 audit, §3.7) |
| `row_gap: 2.0` — the demo Table | `admin_ui.rs`: `components_body`, `fill_body` (`TableOpts.row_gap: 2.0`) | LAY7.1 | a parameter of the UI-console showcase demo sections (a demonstration of `TableOpts.row_gap`), isolated by the demonstration purpose (K4) | Accepted (2026-10 audit, §3.3) |
| Hairline micro-values 2–4 px (below the S1 minimum) | `search_ui.rs` `TITLE_SUB_GAP 2`; `docs_ui.rs` `HELP_SUBMENU_GAP 2`; `hints_ui.rs` `HINT_ROW_INSET_H 4`; `settings_ui.rs` `MODAL_TITLE_CONTENT_GAP 4`; `template_ui.rs` `STRIP_PAD_H 4`; `suggest.rs` `VP_MARGIN 4`; `kit_ui.rs` `BUTTONS_SECTION_GAP_EXTRA 4`, `DROPDOWN_ANCHOR_GAP 4` | LAY7.1 | density micro-insets and historical additions (the highlight inset, the gap inside a result row, the menu column joint, the chip-strip padding, the kit-viewport clamp, the showcase ruler): 2→6/4→6 triples the micro-indent and moves the pinned geometry (kit — the golden `gallery_layout_ruler_golden_column_skeleton`); the class of the frozen palette `PAL_*` (2–5, F-7.9) | Accepted (LAY-W16, review §3.2 P2-2; `BUTTONS_SECTION_GAP_EXTRA` — W3b, a golden pin) |
| Surface paddings/margins of 14–16 px (between the S1 steps 12 and 24) | `settings_ui.rs` `MODAL_THEME_GAP 14`; `explain_ui.rs` `BODY_PAD 16`, `LAYOUT_PAD 14`; `docs_ui.rs` `DOCS_PADDING 16`; `flowmap_ui.rs` `PANEL_MARGIN 16`; `crates/canvas-app/src/app/graph_builder_ui.rs` `PAD 16`; `app.rs` `DIALOG_BTN_GAP 16` | LAY7.1 | rounding up → 24 doubles the air of the established surfaces, down → 12 squeezes without a readability gain; `flowmap PANEL_MARGIN 16` — the S4 "surface ↔ edge" family (cf. the minimap MARGIN 16 in 03-spacing-radius.md); `LAYOUT_PAD 14` — the constant of the v4 tidy-tree prototype (see the "Explain — tidy tree" row) | Accepted (LAY-W16; `DOCS_PADDING`/`PANEL_MARGIN` predate the 2026-10 audit — REV-B) |
| Margins/gaps of 20–40 px and derived sums (beyond the S1 reach) | `explain_ui.rs` `WIN_MARGIN 20`; `auto_width.rs` `HORIZONTAL_PADDING 20` (the sum of the renderer's 2×`BODY_PADDING`); `app.rs` `DIALOG_PAD_X 20`, `DIALOG_VIEWPORT_MARGIN 40` (FR-060 "verbatim"); `calc_panel_ui.rs` `PANEL_BOTTOM_GAP 34`; `onboarding_ui.rs` `AI_ONB_PAD_X 30` / `AI_ONB_PAD_TOP 26` / `AI_ONB_PAD_BOTTOM 22` (the F-8 prototype); `search_ui.rs` `ROW_TEXT_X_DOCS 30` (alignment to the "?" badge zone) | LAY7.1 | the S1 scale ends at XL 24 — 26–40 are unreachable without an absurd shift; 20/22 sit between 12 and 24, a migration in either direction changes the pinned geometry of the established windows (the FR-060 dialog, the calc panel, the F-8 card); `HORIZONTAL_PADDING` — a renderer derivative (2×10), a migration would desync the node text from the rendering | Accepted (LAY-W16, review §3.2 P2-2) |
| Element metrics, not gaps (chip padding 2.5; text line steps 14/30) | `crates/canvas-app/src/app/ai_status_panel.rs` `FEAT_PAD_Y 2.5` (the feature-chip height: `FEAT_FONT 10.5` + 2×2.5); `crates/canvas-app/src/app/overlays.rs` `AI_ONB_PRIV_TITLE_STEP 14` / `AI_ONB_PRIV_BODY_STEP 30` (the cursor steps of the privacy-block lines of the AI onboarding, font sizes 11.5/11; the block height — `AI_ONB_PRIV_H`) | LAY7.1 | the values set the size/step of an element (the chip height, the leading of the drawn lines), not a gap between blocks — the gap-scale rule is inapplicable by the nature of the quantity; `FEAT_PAD_Y` is frozen by the F-7.9 prototype (the LAY-W2 decision `3f3f8dd`, lived only as an inline comment) | Accepted (LAY-W16; `FEAT_PAD_Y` — the W2 decision) |
| Autolink — 18 px row buttons without the coarse expansion (LAY8.3) | `crates/canvas-app/src/app/autolink_ui.rs`: `rows_layout` (the «Принять»/«Отклонить» ("Accept"/"Reject") buttons `BTN_W` × `ROW_H − 8` = 18 in a `ROW_H` 26 row, S3); render and hit — one source (`crates/canvas-app/src/app/input.rs` `on_autolink_click` calls the same `rows_layout`) | LAY8.3 | the autolink review dialog — dense rows with three targets in a row (a % chip + 2 buttons of 82 px with gaps of 8/10): a centered expansion to 44 px makes the neighbors overlap each other (the spirit of LAY1.2 "input = what is visible"), a clamp into the 26 row physically cannot give 44 vertically — the expansion requires a touch revision of the row geometry, not a spot wrapper `touch_hit_xywh`; W8 recorded the dialog skip (outside the audit's toolbar list) | Accepted (LAY-W8 — a documented skip; registration — LAY-W21, the touch revision — a separate task) |

- **Explain, tidy tree.** The layout of the explanation tree (column =
  level, row = leaf order, beziers between the ports — FR-083) — an
  algorithm of one object, not a surface skeleton: the kit's "list of
  homogeneous rows + scroll" is inapplicable here, which is recorded in
  the module header. Boundary: the window chrome (the header, the
  breadcrumbs, the footer) stays on the kit/primitives (FR-060); only a
  new algorithmic layout of the same class goes through the same path.
- **Docs, GFM.** `layout_page`/`layout_table` render the embedded content (`user-docs/`): the geometry —
  a pure function of the content width and the measurement (LAY6 honored),
  the indents — on the `tokens::SPACING_*` scale; the viewer's UI wrapper (the doc, the
  "?" menu, scroll) — kit. Boundary: the content rendering is extended
  freely, the UI skeleton on top of it — no.
- **Wheel menu.** Polar coordinates — a canonicalized escape hatch
  (LAY5.1, G8): the geometry is computed by the pure function `wheel_geometry` for BOTH
  the rendering AND the hit test (LAY1.2 — one source), drawn by a
  specialized SDF pass — the card pipeline does not draw arcs. New
  "exotics" — only the same way: a pure function + a justifying comment.
- **`gap: 0.0`.** Allowed ONLY as a deliberate zero gap — "rhythm without gap",
  when the composition step is carried by the row heights/spacers (stacks
  of rows, tables without excess air; in `stage.rs` — the `sync_scroll` invariant). 0 is
  not part of S1 and cannot be read as "the gap was simply forgotten"; in
  non-obvious places — a comment in the code.
- **`row_gap: 2.0`.** The value lives only in the demo sections of the UI-console
  Table: the showcase demonstrates the `TableOpts.row_gap` parameter, so the number is
  part of the demo contract, not of the scale. Porting it into a
  production surface is forbidden; the production table rhythm — `row_gap: 0.0` (see
  above) or an S1 gap.
- **Hairline 2–4 px.** Micro-values below the S1 minimum (6): density
  insets (the row highlight, the column joint, the chip-strip padding, the
  kit-viewport clamp) and the historical additions of the showcase ruler.
  Boundary: a new hairline gap is not introduced into a production layout
  — either an S1 token or an edit of this section; the existing values are
  not changed by migration (tripling the micro-indent, pins).
- **Paddings 14–16 px.** Values between the S1 steps 12 and 24 — a "hole"
  of the scale for container paddings: migrating 14/16 → 24/12 changes the
  density of the established surfaces without a gain. New container
  paddings — MD/LG (LAY7.2), not 14/16.
- **Margins 20–40 px.** Outer window margins and clamps to the viewport:
  S1 ends at XL 24, the values 26–40 are unreachable, 20/22 — between 12
  and 24. New values outside {6, 8, 10, 12, 24} are forbidden without
  editing this section; derived sums (2× the renderer padding) are
  computed from the source and do not migrate.
- **Element metrics.** The chip padding and the text line steps are the
  size/step of an element, not a gap between blocks: the gap-scale rule is
  inapplicable to them by nature (adjacent text metrics like `TITLE_LINE_H 17` — outside
  LAY7). A new gap between blocks dressed up as a "metric" — a
  circumvention of this section, not an exception.
- **Autolink, 18 px row buttons.** The «Принять»/«Отклонить» ("Accept"/"Reject") buttons of the
  suggestion rows (the height `ROW_H − 8` = 18 in a 26 row) — interactive zones <
  44 px WITHOUT the hit-only expansion (LAY8.3): the dialog was skipped by
  the touch-44 wave (LAY-W8; the reason in worklog W8 — «диалог, не
  тулбар; много мелких кнопок в строках» — a dialog, not a toolbar; many
  small buttons in rows). Boundary: a spot expansion is not possible now
  without mutual overlaps of the neighboring targets in the 26-px row —
  the touch solution requires a revision of the row composition (a
  separate task, outside the S-scope of LAY-W21); until then the dialog is
  desktop-first, and a new SMALL zone in it — via an edit of this section.

### Skeleton rhythms `gap: 0.0` (LAY2/LAY10) — the skeletons of waves W3b/W4/W13

A separate subsection (not rows of the main table above): it registers the
`gap: 0.0` rhythms of the root Column skeletons that appeared with the waves
W3b/W4/W13 already AFTER the 2026-10 audit. The section rule "a new
exception — by editing the section in the commit with the code" was
violated back then (the code went into main without an edit); this edit
closes the debt. The subsection is kept separate from the main table so
that parallel edits of the table do not conflict.

| Exception | Files/anchors | Deviates from | Rationale | Status |
|---|---|---|---|---|
| The agent-panel skeleton — a `Column { gap: 0.0 }` of 5 bands + a `Row { gap: 0.0 }` (the "✕" button) | `crates/canvas-app/src/app/agent_panel.rs`: `AgentPanelLayout::build` | LAY7.1 | the panel bands adjoin (the former W4 rhythm): the vertical step is carried by the band heights (S3), the air is given by centering the content inside the bands (`pad`/`CrossAlign::Center`); the golden tests (canonical/narrow/wide/fractional) pin the geometry | Accepted (W4 `78e8c3f`; registration — LAY-W18) |
| The kit showcase ruler — a `Column { gap: 0.0 }` of measured blocks | `kit_ui.rs`: `gallery_layout` (the section ruler) | LAY7.1 | the ruler step is carried by the block heights and the tail gaps of the sections (`SECTION_GAP` tails), the Column gap is zero; the golden `gallery_layout_ruler_golden_column_skeleton` (65 tags, a scan over all offsets) | Accepted (W3b `4d58afa`; registration — LAY-W18) |
| The admin demo-section bodies — a `Column { gap: 0.0 }` of Fixed blocks | `admin_ui.rs`: `body_ruler` | LAY7.1 | the same "ruler" class: the block height = the former cursor step `y +=` (the byte-for-byte W13 migration), the gap — the block tail, not a `Spacer` (in a Column a spacer takes no space) | Accepted (W13; registration — LAY-W18) |

- **Class.** All three — the "rhythm without gap" of a skeleton ruler (the
  same rationale as the `gap: 0.0` row of the main table: the composition step is
  carried by the row heights/spacers, 0 is not read as "the gap was simply
  forgotten"); the difference is the level: here `gap: 0.0` sits on the ROOT
  Column skeleton of a surface, pinned by the golden tests of the
  mechanism. The `gallery_layout` anchor is also mentioned in the "rhythm without gap"
  row of the main table (the audit era) — the subsection does not replace
  or cancel it, but records the skeleton status of the W3b ruler. A new
  skeleton ruler with `gap: 0.0` — by editing THIS subsection in the commit with
  the code. Related — the "parity oracle" status of the backends of these
  skeletons: §LAY9.1.
