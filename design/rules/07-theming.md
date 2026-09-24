# 07 — Темизация

> Код: `canvas-render/src/theme.rs` (ThemeColors), `canvas-core/src/theme_presets.rs`
> (пресеты), `design/tokens/themes/*.json` (данные тем).

## TH1. Темы — данные, не код

Тема = JSON-файл `design/tokens/themes/<id>.json` + одна строка в реестре
`PRESETS`. Enum-вариантов `Theme::Nord` не существует (отклонено в PRD-0006:
is_dark() и таблицы форкались бы). JSON строго валидируется: недостающий или
лишний ключ → ошибка загрузки → фолбэк на классическую тему (I-47.1).

## TH2. Реестр пресетов (7)

| id | label | Тон | background | accent |
|---|---|---|---|---|
| nord | Nord | dark | #2E3440 | #81A1C1 |
| dracula | Dracula | dark | #282A36 | #BD93F9 |
| catppuccin-mocha | Mocha | dark | #1E1E2E | #CBA6F7 |
| tokyo-night | Tokyo Night | dark | #1A1B26 | #7AA2F7 |
| gruvbox-dark | Gruvbox | dark | #282828 | #B8BB26 |
| catppuccin-latte | Latte | light | #EFF1F5 | #8839EF |
| solarized-light | Solarized | light | #FDF6E3 | #268BD2 |

Классические темы Dark/Light (не пресеты) — конструкторы `ThemeColors::dark()/
light()` из примитивов токенов. Выбор в настройках: `theme: Dark|Light` +
`theme_preset: String` (пусто = классика; неизвестный id мягко деградирует).

## TH3. Обязательные слоты (36)

Каждый JSON несёт все слоты ThemeColors в `#RRGGBB[AA]`:
background, grid_minor/major, card_fill, edge_edit_fill, edge_label_fill,
menu_fill, search_input_fill, search_row_fill, palette_row/chip/tile/selected/
hover_fill, palette_border, title, icon, body, edge_label, link, quote,
code_text, gfm_code_fill, gfm_quote_fill, gfm_muted_fill, group_fill/
group_border, guide_align, guide_grid, accent, selection_fill, highlight,
whatif_fill, whatif_badge, error, hud, stage_dim + `"dark": bool`
(информационно). Полный список — `REQUIRED_KEYS` в theme_presets.rs.

Паритет label (I-47.2) и контраст-машина по каждому пресету (I-47.3, G3) —
обязательны.

## TH4. Derived-слоты (в JSON не хранятся)

Вычисляются при сборке палитры (`canvas-render/src/theme_presets.rs`):

| Слот | Правило |
|---|---|
| explain_leaf | по светимости фона: Y > 0.5 → light-вариант, иначе dark |
| control_hover_fill / control_selected_fill | из menu_fill: `c·1.3 + 0.04` (blue +0.06) |
| control_primary_hover_fill | из dialog.button_primary_fill (hover-формула) |
| control_disabled_text | из примитивов (#8A909C) |
| formula_fn / formula_op | = link / quote |

Правило: если слот выводим — он выводится, а не копируется в JSON
(единственная точка вывода — theme_presets.rs).

## TH5. Определение dark/light

`is_dark()` = сумма байтов background < 384. Используется для derived-слотов
и выбора вариантов severity-таблиц.

## TH6. Мост в кит

`From<&ThemeColors> for KitPalette`: panel_fill = menu_fill,
panel_border = palette_border, control_fill = palette_chip_fill,
control_border = DIALOG_BUTTON_BORDER, control_primary = DIALOG_BUTTON_PRIMARY,
control_danger = error, text = body, text_title = title,
text_muted = DIALOG_TEXT_MUTED, accent = accent. Кит не знает тем — только
слоты.

## TH7. Правки тем

- Поменять цвет пресета → правка JSON (значение обязано пройти G3).
- Добавить пресет → JSON + строка PRESETS; количество слотов = 36, иначе
  фолбэк.
- Поменять классическую dark/light → правка конструкторов из примитивов
  (не JSON).
