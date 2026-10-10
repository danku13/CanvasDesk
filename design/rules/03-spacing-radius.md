# 03 — Spacing, radii, heights

> Scales: `design/tokens/dimensions.json` ↔ `canvas_core::tokens`
> (SPACING_*/RADIUS_*). Consumers must take values from the scale;
> literals in surface modules are forbidden (G5/G8 audit).

## S1. Spacing scale

| Token | Value | Typical use |
|---|---|---|
| SPACING_S | 6 | chip gap in a bar, row gap, LIST_ROW_GAP |
| SPACING_SM | 8 | inner padding of comparison tables, control gap (GAP_CONTROLS) |
| SPACING_MD | 10 | bar/panel padding (BAR_PADDING), empty-button padding |
| SPACING_LG | 12 | outer margins of panels (PANEL_PAD, BAR_MARGIN), card TITLE_PADDING/BODY_PADDING |
| SPACING_XL | 24 | breathing room of large surfaces (gallery panel ↔ viewport) |

Selection rule: inner gaps — S/SM, container padding — MD/LG,
outer margins from the viewport edges — LG/XL. Half-steps (7, 9, 11) are
forbidden.

## S2. Radius scale

| Token | Value | Use |
|---|---|---|
| RADIUS_CHIP | 6 | chips, buttons, gallery empty/filter buttons |
| RADIUS_CARD | 10 | node cards (CORNER_RADIUS; FR-075: 8→10 prototype-unified — the 2026-09 edit bypassed this folder, the 2026-10-09 audit synced it) |
| RADIUS_PANEL | 10 | panels (gallery, modals, docs) |
| RADIUS_PILL | 12 | pills: category chips, switch track, badge pills |

A new radius outside the scale is forbidden; an "almost pill" (10.5) does not
exist. The one separate off-scale value is the minimap border, 8 (`minimap.rs`,
its own use case `../use-cases/minimap.md`); unifying it with RADIUS_CARD is
an owner decision via an edit of this file (I-1).

## S3. Fixed component heights (kit v1/v2)

| Component | Constant | Value |
|---|---|---|
| Button | BUTTON_HEIGHT | 30 (horizontal padding 12) |
| Icon button | ICON_BUTTON_SIZE | 26 |
| Chip | CHIP_HEIGHT | 24 (horizontal padding 8) |
| Text field | TEXT_FIELD_HEIGHT | 30 (min width 80, padding 8) |
| List row | LIST_ROW_H | 26 (gap 6) |
| Switch | SWITCH_W × SWITCH_H | 36 × 20 (knob pad 2) |
| Scrollbar | SCROLLBAR_WIDTH | 4 (knob ≥ 20) |
| Node card header | HEADER_HEIGHT | 34 (FR-023: 28 → 34 for the 16+14 % type size) |
| Empty-state button | EMPTY_BTN_H | 34 |
| Tooltip | offset (14, 18) | 500 ms delay, clip width 380 |
| Panel header S/M/L | PANEL_HEADER_H_S/M/L | 30 / 38 / 44 (tokens.dimensions.json panel_header; 2026-10-09 audit: added to the rule — before that they lived only in tokens) |

Live deviations from S/M/L (TODO W-d in code; migration — audit wave W1):
scheme_gallery 40→38, autolink 58→44. Until the migration the value in code
takes priority (I-1); after it the constants are removed. (The "explain 56→44"
line was closed by LAY-W7, 04de817: `explain_ui::HEADER_H = PANEL_HEADER_H_L`;
synced by LAY-W19.)

S3-derived heights — panel-strip containers, values outside the S3 scale
as standalone components (LAY-W7, 04de817: the derivative arithmetic is
documented in code, the values are pinned `lay_w7_heights_are_canonical_s3`;
the rows were added to the rule by LAY-W19):

| Component | Constant | Value | Rationale (S3-derived) |
|---|---|---|---|
| Agent context-chip strip | `agent_panel` CTX_H | 32 | chip-row container: CHIP_HEIGHT 24 chips centered in the 32-px strip |
| Agent input strip | `agent_panel` INPUT_H | 44 | input-row container = MIN_TOUCH_TARGET 44 (LAY8.3); the field inside is TEXT_FIELD_HEIGHT 30 |
| Agent quick-actions strip | `agent_panel` QUICK_H | 36 | chip-row container: CHIP_HEIGHT 24 pills in the 36-px strip |
| Calc fan pill (2 lines) | `calc_panel_ui` PILL_H_TWO_LINE | 34 | 2 lines × TITLE_LINE_H 17 + 2×pad (the number matches CARD_HEADER_HEIGHT, the semantics is a fan pill, not a header) |
| Calc fan pill (1 line) | `calc_panel_ui` PILL_H_ONE_LINE | 20 | 1 line × TITLE_LINE_H 17 + 2×pad (the number matches SWITCH_H, the semantics is a pill, not a switch) |

A new "S3-derived" height is introduced by editing this table in the commit
with the code (I-1 — analogous to the LAY7 "Exceptions" in `11-layouts.md`).

Rule: heights are kit constants, not call parameters. A new component
inherits the closest height from this table.

## S4. Surface gaps

| Pair | Value |
|---|---|
| Dropdown ↔ anchor | DROPDOWN_GAP 4 |
| What-if enter pill ↔ bottom | 44 (floats +26 above the bar) |
| Toast zone ↔ viewport bottom | 44 |
| Corner cluster ⚙/theme/? | SETTINGS_BUTTON 36, MARGIN 12, GAP 8 |
| Minimap ↔ edge | MARGIN 16 |
| Search/palette ↔ edge | 12 |
| Port click zone | 10 (presets 10/14/20/28/40, clamp [10,40]) |
| Edge hit tolerance | EDGE_HIT_TOLERANCE 6 (or half the stroke width + 2) |
| Card resize handle | 16 |

Off-scale micro-values frozen by the prototypes are listed in
`11-layouts.md` §LAY7 "Exceptions" (command palette: 5/5/3/2).

## S5. Canvas grid and snapping

Minor grid 20, major 100 (GridDensity::Medium); a sub-grid (minor/2)
at zoom > 1.5, the large one (major) at zoom < 0.5; snap tolerance 8 screen px.
Collision/packing are off by default.

## S6. Densities (overflow control)

> The normative source for the overflow and adaptivity policies is `11-layouts.md`
> (LAY3 policies, LAY4 flex factors, LAY8 viewports/degradation); here —
> a summary of values.

- Row `Fit` overflow is NOT masked — the G4 lint catches it on the three
  reference viewports (1280×800 / 1024×640 / 800×560).
- A narrow slot degrades by name: `RowPolicy::SqueezeTail` (the tail is squeezed
  to zero, not dropped) — a replacement for a silent `take()`.
- Wrap packing: row height = max of the children, gap on both axes; the slot
  height determines the number of visible rows.
- Node body: column gap 8 (node token table.node_guide_gap), kit
  guide_gap 6; zebra striping turns on at a run of ≥ 4 rows; the description
  clamp is 2 lines («⋯ целиком ▾» ("⋯ full ▾")).
