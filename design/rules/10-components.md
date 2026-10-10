# 10 — Registry of kit v2 components and palette slots

> The normative result of the 2026-10-09 audit (the “rules ↔ ui-kit” wave): it
> closes the B1/B2 gaps of the `design/rules-audit-2026-10.md` audit — the
> component contracts of the FR-058/FR-068/FR-UI-* waves, the full `KitPalette` slot
> registry, the scroll model, the showcase requirement. Sources: `00-principles.md`
> (П4/П6/П7), `08-states.md` (ST1–ST3, ST6), `07-theming.md` (TH6), `docs/ui-kit.md` §7.

## K1. Component registry

Every interactive kit component obeys ST1–ST3 (the state matrix, the slots,
the click contract) and ST6 (a decoration that catches a click is a control).
Color — only the K2 slots; metrics — only the 03 scales (SPACING_*/RADIUS_*/the
S3 heights) and the T2 font sizes. A new state = a new slot (П4), not a formula.

| Component | Module (`canvas-ui/src/component/`) | Anatomy | States (ST1) | Key constants |
|---|---|---|---|---|
| Button (+IconButton) | `button.rs` | a rect + a measured label (ellipsis), the Primary/Secondary/Ghost/Danger variant | all ST1; Ghost-hover — an accent border | BUTTON_HEIGHT 30, BUTTON_PAD_H 12, BUTTON_FONT_SIZE 13, ICON_BUTTON_SIZE 26 |
| Switch | `button.rs` | a track pill + a knob, the SPACING_SM inset | Normal/Hovered/Disabled; on → control_primary | SWITCH_W×H 36×20, KNOB_PAD 2, the RADIUS_PILL radius |
| Chip | `chip.rs` | a Fit chip: measured text + CHIP_PAD_H | Normal/Selected/Hovered/Disabled | CHIP_HEIGHT 24, CHIP_FONT_SIZE 12 |
| ChipStrip | `chip.rs` | a Row of Fit chips + the optional “All” preset, a Wrap variant | inherited from Chip | the SPACING_S gap |
| TextField | `text_field.rs` | a container + text_area + caret; the model IN1–IN7 | Normal/Focused/Disabled (+Error — the consumer) | TEXT_FIELD_HEIGHT 30, PAD_H 8, MIN_W 80, FONT_SIZE 13 |
| Dropdown / Tooltip / Toast | `dropdown.rs` | a menu from an anchor (flip at the edge), a tooltip by delay, a toast TTL | items — the consumer's WidgetState | DROPDOWN_GAP 4, TOOLTIP_DELAY 500, OFFSET (14,18), TOAST_TTL 3000 |
| Panel / Card | `panel.rs` | fill/border/pad from slots, card = a panel with a header | chrome without states (a container) | panel_style().pad = SPACING_LG |
| PanelHeader | `panel_header.rs` | an S/M/L header + aligned buttons (close/theme/…) | buttons — ST1 | PANEL_HEADER_H_S/M/L 30/38/44 (see S3) |
| Modal | `modal.rs` | stage_dim dimming + a panel + focus_order | a container; content — ST1 | the RADIUS_PANEL radius |
| List + Scroll | `list.rs` | list_rows (a visibility window, partial rows) + scroll_bar | rows — the consumer's WidgetState | LIST_ROW_H 26, LIST_ROW_GAP 6, SCROLLBAR_* (K3) |
| Row (RowGuides) | `row.rs` | a row on column guides: marker/label/value/unit/badge | RowStyle per KitState | guide_gap 6 (kit) / 8 (node), zebra ≥ 4 |
| Table | `table.rs` | retained orchestration of Row: a shared measurement → guides → window → per-row style → thumb | per-row KitState | TABLE_ZEBRA_RUN_MIN 4, paint_scrollbar |
| Banner | `banner.rs` | an Info/Warning/Error strip: a border slot, text, an action button | button — ST1; text disabled_text | BANNER_TINT_ALPHA 0.10 (an alpha-tint of the kind slot) |
| ChatBubble | `chat_bubble.rs` | a Normal/Error/Success message + tool-call lines | background/border — an alpha-tint of the kind slot (0.08/0.38) | CHAT_LINE_H, pad SPACING_SM |
| RadioCard | `radio_card.rs` | a 12×12 indicator + label + desc; a selected ring | Normal/Hovered/Disabled + selected | RADIO_LABEL_LINE_H ≈ 17, tint accent α0.10 |
| Crumbs | `crumbs.rs` | a row of crumb chips, overflow take_while from the right edge | last-visible — Selected | CRUMB_H, CRUMB_FONT_SIZE 11 |
| Footer | `footer.rs` | a 3-button right footer (Cancel/OK…) | buttons — ST1 | BUTTON_WIDTH 100 (reference-default; the measured width — footer_buttons_measured) |
| TwoColumn | `two_column.rs` | [sidebar | content]: a fixed column + filling content (left/right) | containers | — |
| Icon | `icon.rs` | IconKind → Vec<PaintItem> (data, not glyphs) | tint by a slot | the 18×18 slot; geometry — the former magic numbers (v2 unification) |

Rules:

- **K1.1** A new kit component must appear in this table in the same wave as
  in code (the FR-058/FR-068 lesson: 11 components broke away from the rules —
  see audit §2-B1).
- **K1.2** Component choice: a list of rows — List; table guides — Row/Table;
  an exclusive choice — RadioCard; a status strip — Banner; an agent message —
  ChatBubble. Do not breed parallel solutions.
- **K1.3** A component does not animate itself (M3): state transitions are
  instantaneous; the M1 durations belong to the consumer.

## K2. KitPalette slot registry (19)

The only color source for the kit (П4). Filled by the `From<&ThemeColors>`
bridge (TH6). The full list with semantics:

| Slot | Semantics | Theme source |
|---|---|---|
| panel_fill | panel/modal fill | menu_fill |
| panel_border | panel border | palette_border |
| control_fill | secondary/ghost control | palette_chip_fill |
| control_border | control border | DIALOG_BUTTON_BORDER |
| control_primary | primary fill | DIALOG_BUTTON_PRIMARY |
| control_danger | destructive action | error |
| hover_fill | hover of a normal control | control_hover_fill (derived, TH4) |
| primary_hover_fill | primary hover | control_primary_hover_fill (derived) |
| selected_fill | selected row/chip | control_selected_fill (derived; = hover, I-1) |
| text | body text | body |
| text_title | headings | title |
| text_muted | muted text | DIALOG_TEXT_MUTED |
| disabled_text | disabled text | control_disabled_text (derived) |
| accent | focus border, Ghost-hover, rings | accent |
| control_success | the “success” status (FR-070) | derived theme.rs |
| control_warning | the “warning” status (FR-070) | derived theme.rs |
| stage_dim | background dimming under a modal | stage_dim |
| scrollbar_thumb | scrollbar thumb (FR-070) | derived theme.rs |
| rule_color | the markdown `---` line (FR-070) | derived theme.rs |

Rules: **K2.1** a new slot = editing KitPalette + the bridge + this table
simultaneously; **K2.2** alpha variants of slots — only via `tint()` with a
constant from C2 (recorded in the K1 table); rgb is never modified.

## K3. Scroll model

- `ScrollState { offset, content_h, viewport_h }` — the kit's only scroll
  model (`scroll_by`/`clamp`/`needs_scroll`/`max_offset`); own “offset: f32 +
  manual clamp” setups in surfaces are forbidden (the IN1 analog).
- `list_rows(area, s, row_h, gap, count)` — a pure visibility-window function;
  partial rows at the edges are included.
- `scroll_bar(area, s, p)` — the thumb: width SCROLLBAR_WIDTH 4, the minimum
  SCROLLBAR_KNOB_MIN 20, the scrollbar_thumb slot; the hit zone is the thumb
  rect (П5). A separate kit scrollbar component (track+inertia) — the audit's W3
  backlog; until then consumers draw the thumb only with this function.
- A Row-window over `ScrollState` — the sanctioned pattern for lists with a
  MIXED row stride (section headers + cards — there is no single stride,
  `list_rows` by a constant is inapplicable, LAY-W9): the state is the first visible
  row, the px offset of `ScrollState` = the sum of the row strides above the window.
  The canonical implementation — `canvas-app/src/template_ui.rs::RowScroll` (the
  template panel and flyout; promotion into the kit — upon the third consumer).
  The anti-pattern “offset: f32 + manual clamp” in px remains forbidden (LAY10).

## K4. Showcase (extension of ST5)

The UI console (“?” → “UI console”) must show a live sample of every K1
component in all ST1 states (matrix × RU/EN × themes). The section set as of
2026-10-09: buttons, icon_buttons, chips, dropdown, toast, tooltip,
text_field, switch, card, list, icons, table, panel_header — **missing**
banner, chat_bubble, crumbs, footer, radio_card, two_column (the audit's W2
backlog). A new component without a showcase section — an unclosed wave.

**Layout showcase** (normative source `design/rules/11-layouts.md`, checklist
LAY11 item 10): a showcase section is updated in the same wave as the new
layout/LAY mechanism. The layout section set as of 2026-10-10 (code —
`crates/canvas-app/src/kit_ui.rs`, constants `SECTION_*`/`SECTION_LAYOUT_*`):

| Section | 11-layouts.md mechanism |
|---|---|
| measured | LAY6 — measured text (`Row::lay_out_measured`, F-13) |
| grow | LAY4 — flex factors (`Child::flexible`, grow 2:1) |
| wrap | LAY3 — the `RowPolicy::Wrap` policy |
| grid | LAY2/LAY5 — `grid_cells` (equal explicit columns) |
| focus | LAY11 — the showcase Tab ring (FocusRing) |
| squeeze | LAY3 — the `RowPolicy::SqueezeTail` policy |
| align | LAY2 — `MainAlign::SpaceBetween` / `Column` + a spacer (LAY7.3) |
| component_row / component_panel | the K1 component layer (Row/Table/Panel) |
| layout_constrain | LAY2 — `constrain(min, max, desired)` (clamp) |
| layout_pad | LAY2 — `pad(slot, EdgeInsets)` from the S1 scale |
| layout_stack | LAY2 — `stack(slot, size, HAlign, VAlign)` (center/corner) |
| layout_gaps | LAY7 — the S1 gap scale (S/SM/MD/LG/XL with labels) |
| layout_percent | LAY5 — the `SceneDim::Percent` + `Fill` scene (width fractions) |
| layout_aspect | LAY5 — the aspect-ratio scene (16:9 tiles) |
| layout_sticky | LAY5 — the Sticky scene in a scrolling window (Hidden+offset) |
| layout_hide_below | LAY8 item 3 — `DegradationPolicy::HideBelow` (the 900×600 threshold) |

Layout showcase rules:

- **K4.1** Demo sections are built ONLY from the `canvas_ui::layout` primitives
  and the `SceneNode` scene (`lay_out_scene`); gaps/paddings — only the S1
  scales (LAY7), text — via `TextMeasurer`/`MeasuredItem` (LAY6), no silent
  clamps (G5), `Custom` — only with a justification (G8).
- **K4.2** The demo geometry and the section captions come from the single
  layout (`gallery_layout`): hit slots (when interaction appears) and drawing
  are built from the same rects (LAY1.2); the scroll shift/full-visibility
  filter — a shared showcase mechanism.
- **K4.3** Scene sections check the engine's `LayoutFeatures` mask before the
  extended policies (LAY5.2); a LAY mechanism not shown by a showcase section —
  an unclosed wave (the K1.1 analog for components).
