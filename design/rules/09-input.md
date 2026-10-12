# 09 — Behavioral adequacy of input modules

> The normative result of the 2026-10-09 audit (the “input adequacy” wave): a
> single field model, the keyboard contract, caret visibility, selection,
> states, type semantics, IME/web parity, no duplicates. The model code —
> `crates/canvas-ui/src/component/text_field.rs` (`TextFieldModel`,
> `TextFieldAction`, `text_field()`), the key mapper — the app layer
> (`canvas_app::app::input::text_field_action`). Sources: `00-principles.md`
> (П4/П5/П6), `08-states.md` (ST1/ST2), `../use-cases/text-field.md`.

## IN1. Single field model — kit `TextFieldModel`

Every text field of the app UI (search, filters, settings, chat agent,
inline edits) keeps text/caret/selection in the kit's `TextFieldModel`.
Positions are in CHARACTERS (`chars().count()`), not bytes (П6).

Forbidden:

- homegrown “string + cursor” structures (`SearchInput`, `filter: String` +
  `cursor: usize`, `EditField { text: String }`) — duplicates of caret
  heuristics that historically broke on Cyrillic;
- `String::push`/`pop()` as the input model — an “always at the end” caret
  with no click/arrow support does not qualify as adequate;
- byte indices for the caret.

## IN2. Keyboard contract (the editing minimum of a single-line field)

The field must handle ONE and the same set of keys — via the kit actions
`TextFieldAction` + `TextFieldModel::apply` (headless-testable), not via its
own match blocks:

| Key | Action |
|---|---|
| Printable characters / IME commit | `Insert` (replaces the selection) |
| Space | `Insert(" ")` — Space is a winit Named key; the Character branch does not see it |
| Backspace / Ctrl+Backspace | `Backspace` / `BackspaceWord` |
| Delete / Ctrl+Delete | `Delete` / `DeleteWord` |
| ← / → (±Shift) | `CaretLeft/Right { extend }` |
| Ctrl+← / Ctrl+→ (±Shift) | `CaretWordLeft/Right { extend }` |
| Home / End (±Shift) | `Home` / `End` |
| Ctrl+A (Latin/Cyrillic/control code) | `SelectAll` |
| Ctrl+C / Ctrl+X / Ctrl+V | `Copy` / `Cut` / `Paste` → effects to the consumer (the clipboard is on the app side: the kit is zero-dep) |

Semantics without a selection: `Copy`/`Cut` — the whole text (the convention
of single-line chat/filter fields); `BackspaceWord`/`DeleteWord` — the word
heuristic (whitespace + ASCII punctuation — separators). A Ctrl/Alt+symbol
combination not in the table is NOT inserted into the field (it goes to the
hotkey ladder).

Enter/Esc — consumer semantics (submit/cancel); they are not part of `apply`.

## IN3. The caret is always visible (scroll-following)

If the text is wider than the field: when focused, a window of text following
the caret is shown (`TextFieldLayout.scroll_x`, the Clip policy — see
`../use-cases/text-field.md` §6); ellipsis is not applied to active input.
Caret literals `format!("{}|")` in the text are FORBIDDEN — the caret is drawn
from `TextFieldLayout.caret_x` (1.5 px, the `accent` slot, the agent-panel pattern).

## IN4. Selection

Shift-arrows/Shift+Home/End extend the selection (anchor/head in characters);
the selection underlay is accent α0.25 beneath the text (the background
before the text, the bounds by measuring prefixes in the same font size).
Typing/Backspace/Delete replace the selection.

## IN5. Mouse

- A click on the field — focus + the caret at the click location
  (`caret_index_at_x`, a character position accounting for `scroll_x`).
- Double click — word selection (the v2 plan); drag-selection — v2.
- The field hit zone = the field rect from the same layout function (П5).

## IN6. Field states (per ST1/ST2)

Normal / Focused (an accent border, the caret visible) / Disabled (the text
`control_disabled_text`, input ignored, the consumer's busy status) /
Error (a border of the `control_danger` slot, drawn by the consumer —
validation). The field has no hover fill (the field is identified by focus).

## IN7. Field type semantics

| Type | Required properties | Screen examples |
|---|---|---|
| text | placeholder, scroll-following, the full keyboard contract | settings fields (URL, model) |
| password (mask) | `mask_char: Option<char>` in the layout — glyphs are replaced with the mask, the model keeps the original; caret/selection work over the mask | API keys (LlmSettings) |
| search/filter | + a live filter without Enter, reset of row selection/scroll on edit, placeholder | Ctrl+F, the palette Ctrl+P, the gallery Ctrl+T |
| chat-input | + Enter=send, busy→Disabled, quoting the scenario via the placeholder | the agent panel Ctrl+I |

`max_chars` (the model policy `TextFieldModel.max_chars`) — a hard ceiling on
`Insert` (clamps the insertion to the limit; backspace/delete are not
limited). Multiline field/number-spinner/select — outside the product
screens; they are introduced only when a consumer screen appears (the “no
speculative components” principle).

## IN8. IME/web parity

Every active text sink must:
1) be present in `App::text_input_active()` (the web shim holds the virtual
keyboard focus);
2) be present in `App::insert_committed_text` (the Ime::Commit route);
3) the sink priority is fixed: the settings field → the note editor →
search → the palette → the agent panel → inline edits. A sink missing from the
route = on web/mobile the input is silently lost (the agent-panel defect found).

## IN9. No duplicates — kit everywhere

A new text sink = `TextFieldModel` + the app mapper
`text_field_action()` + rendering via `kit::text_field()`. Copying
keyboard match blocks is forbidden; a contract divergence between fields is
a defect, not a feature. The existing duplicates migrate to the kit (the
“input adequacy” wave of 2026-10-09: SearchInput, the gallery, the palette,
settings, explain inline, the agent panel).

## IN10. Sufficiency of the kit set

The minimally sufficient composition of the kit input module: `TextFieldModel`
(text/caret/selection/word operations/max_chars) + `TextFieldAction`/
`TextFieldEffect` (the keyboard semantics) + the `text_field()` layout
(placeholder/ellipsis/scroll-following/mask/caret_x) + `caret_index_at_x`
(click) + the `TextField` component (container/focus/states). Sufficiency
was verified by the screens audit: all text inputs of the app are covered by
the IN7 types; what was missing at the audit time — implemented in this wave.

## IN11. Vertical label centering

The area of a single-line label is the LINE BOX: height = `size × 1.3`
(`SCREEN_LINE_FACTOR`), vertically centered in the control rect —
`canvas_ui::measure::text_area_vcentered(rect, size)` (x/w inherited, the
box is clamped to `rect.h`). The renderer mirrors the convention:
`ScreenText.area_h` carries the owner-rect height and the renderer
auto-centers the line box when slack is present (`vcenter_top`,
`canvas-render/text.rs`); `area_h == 0.0` or `== line_h` → the top anchor,
byte-identical to the pre-canon rendering (the bit-parity guarantee for
not-yet-migrated sites).

A top anchor is allowed ONLY for intentional multi-line stacks (list rows,
about-lines, search results, tooltips) that advance by the line height —
never as a substitute for centering a control label.

Forbidden: hand-made sub-rects and magic y-offsets (+2…+8) at label call
sites; a second line-height factor — the row/table factor is
`ROW_LINE_FRAC == SCREEN_LINE_FACTOR` (one multiplier everywhere; the
historical 1.1 was a defect class, issue #68).

## IN12. Caret geometry

The caret of EVERY text field comes from ONE formula —
`canvas_ui::measure::caret_rect(text_area, size, caret_x)`:
`h = size × 1.3 − 2·2.0` (clamped to ≥ 6.0), `w = 1.5`, vertically centered
on the text line (the same center as the label; the canonical input is the
line box from `text_area_vcentered` — a full control rect also works, but
the line box is the convention). The X position (caret index/scroll) is the
caller's job. Per-surface math (`area.h − 4`, `area.h − 8`, widths
1.0/1.5 mixed — the pre-canon state of 7 sites) is forbidden: a caret
taller than the text line is a defect, not a style.

## IN13. Forbidden magic vertical offsets

No hand-computed y-offsets at label/caret call sites: the vertical geometry
of a label is `text_area_vcentered`, of a caret — `caret_rect` (the kit
helpers live in `canvas_ui::measure`, re-exported via `canvas_ui::kit`).
The renderer auto-centering (`ScreenText.area_h`) covers sites that pass a
full control rect, but new code passes the line box explicitly. Enforced by
the V3 gate (issue #68): `bash scripts/v3_ui_lint.sh` (scanner
`scripts/v3_ui_scan.py`, CI job `v3-ui-lint`) — full-rect label calls,
vertical literal offsets at label/caret sites, and `× 1.3`/`× 1.1`
line-height literals outside `crates/canvas-ui/src/measure.rs`. The only
exception is the inline per-line marker `// v3:allow <reason>` (deliberate
top-anchored stacks, IN11); file-level allowlists do not exist.
