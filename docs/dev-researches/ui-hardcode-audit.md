# Аудит хардкода UI — пробелы kit и порядок миграции

**Дата:** 05.10.2026
**Автор:** агент-сессия (Super Z)
**Тип:** технический аудит / инвентаризация kit-пробелов
**База:** commit `5ac171e` (после фикса полосы результата)
**Связанные документы:**
[`ui-kit-adoption-audit.md`](./ui-kit-adoption-audit.md) (03.10.2026 — первичный аудит по поверхностям),
[`docs/ui-kit.md`](../ui-kit.md),
[`docs/prd/prd-0009-ui-layering-uikit.md`](../prd/prd-0009-ui-layering-uikit.md),
[`AGENTS.md`](../../AGENTS.md) §«UI-кит — обязательное правило»,
FR-046/051/053/055/057/062/068.

---

## 1. Контекст и задача

Владелец попросил провести полный анализ всех UI-поверхностей на предмет
хардкода вместо использования `canvas-ui` kit, и зафиксировать правила в
`AGENTS.md`. Этот документ — детальный инвентаризационный перечень
найденных пробелов kit (отсутствующих компонентов/слотов/токенов) с
указанием файлов, severity и рекомендованного порядка миграции. Правила
использования kit вынесены в `AGENTS.md` §«UI-кит — обязательное
правило при вёрстке на Rust».

Аудит-2026-10-05 расширяет первичный аудит [`ui-kit-adoption-audit.md`](./ui-kit-adoption-audit.md)
от 03.10.2026: фокус смещён с per-surface Kit-статуса на **отсутствующие
сущности kit** (цветовые слоты, геометрические токены, компоненты,
layout-паттерны) и конкретные места хардкода с координатами строк.

## 2. Степень adoption ui-kit по слоям кода

| Слой | Adoption | Комментарий |
|---|---|---|
| **Layout-примитивы** (`constrain`/`stack`/`pad`/`Column`/`Row`/`MeasuredItem`/`RowPolicy`) | ✅ отличное | Все поверхности используют для раскладки |
| **`TextMeasurer`** (`width_of`/`wrap`/`ellipsis`) | ✅ хорошее | За исключением `tooltip.rs::wrap_width` (расширение NBSP) и `ai_status_panel.rs:232` (регрессия CR-015) |
| **Scroll списки** (`list_rows`+`ScrollState`+`scroll_bar`) | ✅ хорошее | Большинство списков через kit |
| **Popups** (`dropdown_menu`/`modal`/`tooltip`) | ✅ хорошее | Повсеместно |
| **Core controls** (`button`/`chip`/`icon_button`/`switch`/`text_field`/`card`/`panel`/`table`) | ✅ хорошее | Базовый набор закрыт |
| **Draw-слой** (`Painter` + `PaintItem`) | ⚠️ смешанное | `onboarding_overlay`/`ai_onboarding_overlay`/`kit_gallery`/`admin_panel`/`agent_panel`/`ai_status_panel`/`graph_builder` мигрировали на Painter; `overlays.rs::settings_overlay`/`docs_overlay`/`whatif_overlay` — ещё ручной `CardInstance` |
| **Административные/kit-gallery поверхности** (`admin_ui.rs`/`kit_ui.rs`) | ✅ образцовые | Эталонные потребители kit, на них ровняются |
| **`overlays.rs` (20+ оверлеев, 6091 строка)** | ❌ худшее | Массовый ручной `CardInstance` с инлайн rgba литералами |

## 3. Отсутствующие цветовые слоты `KitPalette`

Сегодня `KitPalette` (`crates/canvas-ui/src/component/mod.rs`) содержит
`control_fill`, `control_danger`, `text`, `palette_border`, `menu_fill`,
`hover_fill` и т.д., но НЕ содержит слотов для статусов успеха/предупреждения
и некоторых хром-цветов — они захардкожены литералами в нескольких файлах.

| Отсутствующий слот | Литерал (rgba) | Где захардкожен (файл:строка) | Семантика | Приоритет |
|---|---|---|---|---|
| `control_success` | `[0.30, 0.75, 0.55, 1.0]` и `Color::rgba(77, 191, 140, 255)` (одно и то же в разных представлениях) | `app/agent_panel.rs:434,491`; `app/ai_status_panel.rs:203`; `app/overlays.rs:5668,5681` | зелёный цвет успеха (бейдж done, completed) | **HIGH** — дрейф цвета неминуем |
| `control_warning` | `[0.95, 0.65, 0.30, 1.0]` и `[242/255, 165/255, 76/255, 1.0]` (одно и то же в разных представлениях!) | `app/ai_status_panel.rs:201,370,386` | амбер warning (paused, low-quota) | **HIGH** — два представления одного цвета = drift risk |
| `scrollbar_thumb` | `[0.35, 0.38, 0.46, 0.7]` | `app/overlays.rs:3489` (docs_overlay) | цвет ползунка скроллбара | MEDIUM |
| `rule_color` | `[0.30, 0.33, 0.40, 0.8]` | `app/overlays.rs:3454` (docs rule) | горизонтальная линия-разделитель markdown | MEDIUM |
| `stage_dim` | `[0.02, 0.02, 0.04, 0.85]` | `app/overlays.rs:3354,4917` (backdrop, дублируется!) | dim-overlay под модалью | **HIGH** — уже есть `ThemeColors::stage_dim`, но `KitPalette` его не пробрасывает |
| `dropdown_highlight` | `[0.24, 0.30, 0.42, 0.6]` | `app/overlays.rs:5730` | подсветка элемента dropdown menu | MEDIUM — `hover_fill` близко, но не идентично |

**Рекомендация:** добавить слоты `control_success`/`control_warning`/
`stage_dim`/`scrollbar_thumb`/`rule_color` в `KitPalette` +
маппинг в `ThemeColors` + пресеты `theme_presets.rs` + ключи в
`REQUIRED_KEYS` (тест паритета семантики). Это устранит ~10 инлайн
rgba литералов в `overlays.rs`/`agent_panel.rs`/`ai_status_panel.rs`.

## 4. Отсутствующие геометрические токены

Сегодня `canvas_core::tokens.rs` содержит `CARD_HEADER_HEIGHT=34`,
`SPACING_S/SM/MD/LG/XL`, `RADIUS_CHIP/PANEL/PILL`, но отсутствуют
токены для высот шапок панелей, размеров шрифтов, ширин бейджей и
геометрии chat-пузырей — они переобъявляются в каждом `*_ui.rs`.

| Отсутствующий токен | Литералы по файлам | Где | Приоритет |
|---|---|---|---|
| `PANEL_HEADER_H` | `30.0, 36.0, 38.0, 40.0, 44.0, 56.0, 58.0` (7 разных значений!) | `flowmap_ui`(36), `docs_ui`(38), `scheme_gallery_ui`(40), `template_ui`(30), `calc_panel_ui`(нет, через SPACING), `autolink_ui`(58), `explain_ui`(56), `agent_panel`(44), `kit_ui`(через BUTTON_HEIGHT) | **HIGH** — нет консистентности |
| `FONT_TITLE_LG`/`FONT_BODY`/`FONT_CAPTION`/`FONT_HINT` | `18.0, 19.0, 16.0, 13.0, 12.5, 12.0, 11.5, 11.0, 10.5, 10.0` (10 разных размеров!) | `onboarding_ui`(18/13/24/19/12.5/11.5/13.5), `ai_status_panel`(10.5), `search_ui`(13/11), `hints_ui`(13), `explain_ui`(12) | **HIGH** — нет типографической шкалы |
| `BADGE_W` / `BADGE_FONT` | `110.0, 14.0, 10.5, 196.0` | `overlays.rs::settings_overlay`(110), `ai_status_panel`(10.5), `autolink_ui`(196/32) | MEDIUM |
| `CHAT_BUBBLE_H` / `CHAT_BUBBLE_PAD` | `24.0 + n_lines * 14.0`, `+14.0` per tool_call | `agent_panel.rs:434+` (формула из n строк) | MEDIUM — нужен `kit::chat_bubble` компонент |
| `DROPDOWN_W` | `170.0, 200.0, 240.0, 260.0, 300.0` (5 разных ширин dropdown) | `settings_ui`(170), `hints_ui`(300), `template_ui`(260 flyout), `search_ui`(460 — не dropdown) | MEDIUM |
| `ICON_BUTTON_SIZE_LG` (close button) | `22.0, 24.0, 26.0, 28.0, 30.0` (5 разных размеров!) | `flowmap_ui`(22), `scheme_gallery_ui`(24), `autolink_ui`(30 — documented dev), `overlays.rs::docs/settings`(26 через kit, но рядом 24 в hint caret) | MEDIUM |

**Рекомендация:** добавить токены `PANEL_HEADER_H` (с вариантами L/M/S),
`FONT_TITLE_LG`/`FONT_BODY`/`FONT_CAPTION`/`FONT_HINT` (типографическая
шкала), `BADGE_W`/`BADGE_FONT`, `DROPDOWN_W` в `tokens.rs` + зеркальный
JSON в `design/tokens/` (тест паритета JSON↔Rust ловит расхождения).

## 5. Регрессия CR-015 — эвристики ширины текста

CR-015 явно запрещает оценивать ширину текста через `len() * factor` —
это класс дефекта: разные глифы дают разную ширину, кириллица шире
латиницы, эмодзи «съедают» место. `TextMeasurer` ввели именно чтобы это
убить. Но в `ai_status_panel.rs` регрессия уже случилась:

| Файл:строка | Эвристика | Что должен использовать |
|---|---|---|
| `app/ai_status_panel.rs:232` | `provider_label.len() as f32 * 5.5 + 14.0` (chip width) | `kit::chip_layout(slot, label, state, palette)` или `MeasuredItem::Text { pad_x }` с `TextMeasurer::width_of` |
| `app/agent_panel.rs` bubble geometry | `text.len() / 48` для подсчёта строк | `TextMeasurer::wrap(text, max_w, family, size).len()` |

**Рекомендация:** HIGH-приоритет — немедленно заменить на `TextMeasurer`.
Это тип бага, который «работает» на латинице, но ломается на кириллице
и эмодзи (а CanvasDesk — русскоязычный продукт).

## 6. Отсутствующие kit-компоненты и layout-паттерны

Перечень паттернов, повторяющихся в ≥ 2 файлах, которые достойны
выделения в kit-компонент (по правилу «≥ 2 файла → кандидат в kit»).

### 6.1. Новые компоненты

| Компонент | Где повторяется (файлы) | Что инкапсулирует | Приоритет |
|---|---|---|---|
| `kit::icon_composition(rect, kind, tint) -> Vec<PaintItem>` | `palette.rs` (~30 hand-rolled quads для LinesSolid/TreeHorizontal/Radial/Pin/Sliders/…) | векторные иконки из квадов | **HIGH** — крупнейший источник ручных `CardInstance` |
| `kit::ghost_card(rect, palette, alpha)` | `suggest.rs::card_rects`, `agent_panel.rs::preview` | полупрозрачная preview-карточка | MEDIUM |
| `kit::radio_card(slot, label, desc, selected)` | `ai_onboarding` (3×), `graph_builder_ui` (3×) | радио-карточка (label + desc + selected ring) | MEDIUM |
| `kit::chat_bubble(rect, kind, text, tool_calls)` | `agent_panel.rs` | bubble сообщения чата (normal/error/success + tool calls) | MEDIUM |
| `kit::crumbs(anchor, count) -> (offset, Vec<UiRect>)` | `explain_ui` (breadcrumbs) | чипы-хлебные крошки с overflow `take_while` | LOW |
| `kit::tree_layout(nodes, edges, slot)` | `explain_ui::layout_tree` | 2D tidy-tree раскладка | LOW |
| `kit::anchored_stack(anchor, side, cards, viewport)` | `suggest.rs::card_rects`, `tooltip.rs` stack | обобщение `dropdown_menu` для многоэлементного стека с flip+clamp | MEDIUM |
| `kit::stage_close_button(slot) -> UiRect` | `explain`, `autolink`, `docs`, `settings`, `kit_gallery`, `admin`, `onboarding` (7 файлов!) | кнопка «×» в правом верхнем углу панели/модали | **HIGH** — самый повторяющийся паттерн |
| `kit::stage_close_button_lg(slot) -> UiRect` (FR-070 Agent K) | `autolink_ui::close_rect`, `explain_ui::close_rect` (HEADER_H=56–58 dialog-шапки) | LG-вариант `stage_close_button`: 30×30 (вместо 26) — visual balance с tall-header dialogs; inset `SPACING_SM`=8 (canonical). Дополнительная запись: `stage_close_button` мигрировал 2 сайта (flowmap/scheme_gallery Agent G); LG-вариант мигрировал 2 сайта (autolink/explain Agent K). | MEDIUM — 2 сайта, отклонение от canonical size (26→30) документировано |
| `kit::panel_header(slot, [buttons]) -> PanelHeaderLayout` (Agent K K3/K4 — добавлено в backlog) | `kit_ui::gallery_layout`+`gallery_hit_slots`, `admin_ui::admin_hit_slots` | раскладка ВСЕХ кнопок шапки панели вместе (title \| theme \| reset \| close) с единым inset/центрированием; устраняет «close/theme/reset выстроены в одну строку content» — `stage_close_button(panel)` отрывает close от соседних широких toggle-кнопок (THEME_SLOT_W=170, RESET_SLOT_W=96). | MEDIUM — 2 файла, требует UX-ревью форм-фактора шапки |
| `kit::tooltip_rect_anchored(anchor: UiRect, text_size, viewport, hovered_ms, delay_ms) -> Option<TooltipLayout>` (Agent K K6 — добавлено в backlog) | `app/explain.rs:1589` (explain-button tooltip), будущие button-anchored tooltips | обобщение `kit::tooltip` (point-anchor) на rect-anchor: natural-position «над-слева от правого-верхнего угла якоря» (button-anchored tooltip), flip вниз-вправо при нехватке места; устраняет hand-rolled формулу `x = anchor.right - w, y = anchor.top - h - 6` в explain.rs. | MEDIUM — 1 сайт сейчас, но паттерн повторится (node-button tooltips в будущих волнах) |
| `kit::overflow_arrow(body, direction)` | `explain`, `calc_panel`, `search` (scroll hint) | 4-стрелочный индикатор переполнения | LOW |
| `kit::backdrop(rect, palette)` | `overlays.rs` (7+ мест: docs/settings/onboarding/ai_onboarding/scheme_gallery/autolink/explain) | dim-overlay под модалью | **HIGH** |
| `kit::banner(rect, label, action)` | `autolink_ui` (rejected items) | баннер с действием | LOW |
| `kit::footer_buttons(rect, [labels]) -> Vec<(UiRect, &str)>` | `autolink_ui`, `onboarding`, `settings`, `graph_builder_ui` | 3-кнопочный правый футер | MEDIUM |
| `kit::chip_strip(slot, items, palette)` | `scheme_gallery_ui`, `template_ui`, `settings_ui` | Row из Fit-чипов с опциональным «All» | MEDIUM |
| `kit::autocomplete_popup(anchor, items)` | `hints_ui` | popup автодополнения (anchor + flip + list_rows + ellipsis) | LOW |
| `kit::search_panel(slot, query, results)` | `search_ui` | top-center панель с input + result list + scroll | LOW |
| `kit::two_column(slot, left_w, right_w)` | `calc_panel_ui`, `admin_ui`, `settings_ui` | [sidebar | content] раскладка | MEDIUM |

### 6.2. Новые immediate-layout хелперы

| Хелпер | Где повторяется | Что инкапсулирует | Приоритет |
|---|---|---|---|
| `kit::table_layout_immediate(columns, rows, slot) -> Vec<Vec<UiRect>>` | `whatif_ui::table_layout` (hand-rolled `Vec<Vec<[f32;4]>>`), `overlays.rs::whatif_overlay` cells | immediate API для таблиц (сегодня только retained `kit::Table`) | MEDIUM |

### 6.3. Расширения `TextMeasurer`

| Метод | Где повторяется | Что инкапсулирует | Приоритет |
|---|---|---|---|
| `TextMeasurer::wrap_paragraphs(text, max_w, family, size) -> Vec<String>` с уважением `\n` и NBSP | `tooltip.rs::wrap_width` (локальная реализация) | wrap, уважающий явные переносы и NBSP | LOW — `kit::wrap` близко, но не уважает NBSP |

## 7. Конкретные `CardInstance` литералы к миграции (HIGH severity)

`app/overlays.rs` — крупнейший источник ручных `CardInstance` литералов.
Перечень конкретных строк с указанием, чем заменить:

| Файл:строка | Литерал | Заменить на |
|---|---|---|
| `overlays.rs:3354,4917` | `[0.02, 0.02, 0.04, 0.85]` (backdrop) | `palette.stage_dim` через новый `kit::backdrop(rect, palette)` |
| `overlays.rs:3366,4941` | `params: [8.0, …]` (modal radius) | `kit::modal_style(palette).radius` (= `RADIUS_PANEL=10.0` — mismatch 8 vs 10!) |
| `overlays.rs:3382` | `params: [6.0, …]` (close button radius) | `kit::icon_button_style(state, palette).radius` (= `RADIUS_CHIP=6.0` — совпадает) |
| `overlays.rs:3454` | `[0.30, 0.33, 0.40, 0.8]` (rule) | новый `palette.rule_color` |
| `overlays.rs:3489` | `[0.35, 0.38, 0.46, 0.7]` (scrollbar thumb) | новый `palette.scrollbar_thumb` + `kit::scroll_bar` |
| `overlays.rs:5314` | `[0.30, 0.33, 0.40, 0.9]` (toggle track off) | `kit::switch(slot, on, state, palette).track_fill` |
| `overlays.rs:5324` | `[0.92, 0.92, 0.94, 1.0]` (knob) | `kit::switch(...).knob_fill` |
| `overlays.rs:5668,5681` | `Color::rgba(77, 191, 140, 255)` (success badge) | новый `palette.control_success` |
| `overlays.rs:5730` | `[0.24, 0.30, 0.42, 0.6]` (dropdown highlight) | `kit_palette.hover_fill` |
| `overlays.rs` (whatif_overlay, строки 312-789) | несколько `CardInstance` для chips/bar/dropdown/table cells | `KitDraw::control` + `kit::chip_style`/`kit::button_style`/`kit::Table` |
| `app/explain.rs:1589` | `CardInstance { fill: palette.menu_fill, border: palette.palette_border, params: [6.0, …] }` (tooltip) | `kit::tooltip` + `Painter::panel` (как уже сделано в `app/tooltip.rs::layout_tooltips` — несогласованная миграция) |

## 8. Рекомендованный порядок миграции (impact × cost)

Приоритезация по соотношению «визуальный эффект × стоимость реализации»:

### Tier 1 — быстрые победы (1-2 дня)

1. **Добавить цветовые слоты `control_success`/`control_warning`/`stage_dim`/`scrollbar_thumb`/`rule_color` в `KitPalette`** — устраняет ~10 инлайн rgba литералов. Изменение: `component/mod.rs` + `theme.rs` + `theme_presets.rs` + `REQUIRED_KEYS`.
2. **Заменить `ai_status_panel.rs:232` эвристику `len() * 5.5 + 14.0`** на `kit::chip_layout` — убирает регрессию CR-015. Изменение: 1 строка + импорт.
3. **Добавить `kit::stage_close_button(slot) -> UiRect`** — устраняет 7 дублирований «× в углу». Изменение: `component/button.rs` + экспорт.

### Tier 2 — средние миграции (3-5 дней)

4. **Мигрировать `overlays.rs::settings_overlay` toggle/dropdown/button rows** на `kit::switch`/`kit::text_field`/`kit::button_layout` + `KitDraw` — крупнейшая концентрация ручных `CardInstance`.
5. **Мигрировать `overlays.rs::docs_overlay` на `Painter` + `kit::scroll_bar`** — устраняет rule/scrollbar/thumb литералы.
6. **Добавить `kit::backdrop(rect, palette)`** — устраняет 7+ дублирований dim-overlay.
7. **Промоутить `PANEL_HEADER_H`/`FONT_TITLE_LG`/`FONT_BODY`/`FONT_CAPTION`/`FONT_HINT` токены** в `tokens.rs` — большая площадь, но механическое изменение.

### Tier 3 — стратегические компоненты (1-2 недели)

8. **`kit::icon_composition`** для `palette.rs` — устраняет ~30 hand-rolled quads. Значимая работа: ~30 векторных иконок переносятся в kit-описание (data-driven: glyph → quads).
9. **`kit::radio_card`/`kit::chat_bubble`/`kit::footer_buttons`/`kit::chip_strip`** — 4 компонента, каждый мигрирует 2-3 файла.
10. **`kit::anchored_stack`** — обобщение `dropdown_menu` для многоэлементных стеков (suggest cards, tooltip stacks).
11. **`kit::table_layout_immediate`** — миграция `whatif_ui::table_layout`.

### Tier 4 — низкоприоритетные (по мере необходимости)

12. `kit::crumbs`, `kit::tree_layout`, `kit::overflow_arrow`, `kit::banner`, `kit::autocomplete_popup`, `kit::search_panel`, `kit::two_column`, `TextMeasurer::wrap_paragraphs` — паттерны с малой площадью или близкие к существующим kit-функциям.

## 9. Файлы без kit-пробелов (clean)

Эти файлы прошли аудит без замечаний:

- `app/export_ui.rs` — чистый file I/O, UI не строит.
- `app/ui_layout_lint.rs` — CI lint-тесты, без UI-логики.
- `app/ui_registry.rs` — реестр поверхностей + hit-rect'ы; рисует через layout-функции, без draw-кода.
- `app/support.rs` — адаптеры Painter↔`CardInstance` (граница слоёв; `CardInstance` строится здесь по праву).
- `suggest_worker.rs` — фоновый поток, без UI.
- `canvas-render/src/renderer.rs`/`cards.rs` — GPU-бэкенд, `CardInstance` его собственный тип (не kit-потребитель).
- `kit_ui.rs`/`admin_ui.rs` — образцовые kit-потребители, на них ровняются.

## 10. Чек-лист для будущих UI-задач

При любой новой UI-задаче агент сверяется с чек-листом из `AGENTS.md`
§«UI-кит — обязательное правило» (7 пунктов: цвета, геометрия, раскладка,
текст, квады, размещение нового кода в `canvas-ui` по G7, явное заявление
о пробеле kit). Если пробел обнаружен — агент ОБЯЗАН предложить
расширение kit (новый компонент/слот/токен), а не обходить хардкодом.

## 11. История изменений

- **2026-10-05** — агент-сессия (Super Z): первичный аудит kit-пробелов
  с координатами строк и порядком миграции. Дополняет
  [`ui-kit-adoption-audit.md`](./ui-kit-adoption-audit.md) от 2026-10-03
  (фокус смещён с per-surface Kit-статуса на отсутствующие сущности kit).
  Правила использования kit вынесены в `AGENTS.md` §«UI-кит — обязательное
  правило при вёрстке на Rust».
