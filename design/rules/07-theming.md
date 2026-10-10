# 07 — Theming

> Code: `crates/canvas-render/src/theme.rs` (ThemeColors), `crates/canvas-core/src/theme_presets.rs`
> (presets), `design/tokens/themes/*.json` (theme data).

## TH1. Themes are data, not code

A theme = a JSON file `design/tokens/themes/<id>.json` + one line in the
`PRESETS` registry. There are no `Theme::Nord` enum variants (rejected in
PRD-0006: is_dark() and the tables would fork). The JSON is strictly
validated: a missing or extra key → load error → fallback to the classic
theme (I-47.1).

## TH2. Preset registry (7)

| id | label | Tone | background | accent |
|---|---|---|---|---|
| nord | Nord | dark | #2E3440 | #81A1C1 |
| dracula | Dracula | dark | #282A36 | #BD93F9 |
| catppuccin-mocha | Mocha | dark | #1E1E2E | #CBA6F7 |
| tokyo-night | Tokyo Night | dark | #1A1B26 | #7AA2F7 |
| gruvbox-dark | Gruvbox | dark | #282828 | #B8BB26 |
| catppuccin-latte | Latte | light | #EFF1F5 | #8839EF |
| solarized-light | Solarized | light | #FDF6E3 | #268BD2 |

The classic Dark/Light themes (not presets) are the `ThemeColors::dark()/
light()` constructors built from token primitives. Selection in settings:
`theme: Dark|Light` + `theme_preset: String` (empty = classic; an unknown
id degrades gracefully).

## TH3. Required slots (37)

Every JSON carries all ThemeColors slots in `#RRGGBB[AA]` (audit 2026-10-09:
the actual REQUIRED_KEYS size = 37; the earlier “36” — a stale counter):
background, grid_minor/major, card_fill, edge_edit_fill, edge_label_fill,
menu_fill, search_input_fill, search_row_fill, palette_row/chip/tile/selected/
hover_fill, palette_border, title, icon, body, edge_label, link, quote,
code_text, gfm_code_fill, gfm_quote_fill, gfm_muted_fill, group_fill/
group_border, guide_align, guide_grid, accent, selection_fill, highlight,
whatif_fill, whatif_badge, error, hud, stage_dim + `"dark": bool`
(informational). The full list — `REQUIRED_KEYS` in theme_presets.rs.

Label parity (I-47.2) and the contrast machine for every preset (I-47.3, G3)
are mandatory.

## TH4. Derived slots (not stored in JSON)

Computed when the palette is assembled (`crates/canvas-render/src/theme_presets.rs`):

| Slot | Rule |
|---|---|
| explain_leaf | by background luminance: Y > 0.5 → the light variant, otherwise dark |
| control_hover_fill / control_selected_fill | from menu_fill: `c·1.3 + 0.04` (blue +0.06) |
| control_primary_hover_fill | from dialog.button_primary_fill (the hover formula) |
| control_disabled_text | from the primitives (#8A909C) |
| formula_fn / formula_op | = link / quote |
| control_success / control_warning | the FR-070 status semantics (theme.rs) |
| scrollbar_thumb / rule_color | the FR-070 slots (overlays: the scrollbar thumb, `---`) |

Rule: if a slot is derivable, it is derived, not copied into JSON
(the single derivation point — theme_presets.rs).

## TH5. dark/light detection

`is_dark()` = the sum of the background bytes < 384. Used for the derived
slots and for choosing the severity-table variants.

## TH6. Bridge into the kit

`From<&ThemeColors> for KitPalette`: panel_fill = menu_fill,
panel_border = palette_border, control_fill = palette_chip_fill,
control_border = DIALOG_BUTTON_BORDER, control_primary = DIALOG_BUTTON_PRIMARY,
control_danger = error, text = body, text_title = title,
text_muted = DIALOG_TEXT_MUTED, accent = accent — plus the FR-070 slots
(control_success/control_warning/stage_dim/scrollbar_thumb/rule_color).
The full registry of 19 slots with semantics — `10-components.md` §K2.
The kit knows nothing about themes — only slots.

## TH7. Theme edits

- Change a preset color → edit the JSON (the value must pass G3).
- Add a preset → JSON + a PRESETS line; the slot count = 37, otherwise
  fallback.
- Change the classic dark/light → edit the constructors from primitives
  (not JSON).
