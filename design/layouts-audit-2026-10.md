# Аудит раскладок LAY1–LAY11 (2026-10-09)

> Аудит существующих UI-поверхностей CanvasDesk на соответствие нормативу
> `design/rules/11-layouts.md` (правила LAY1–LAY11, введены волной
> LAYOUT-RULES, коммит d772799). Метод: grep-аудит потребителей GPU UI
> (`crates/canvas-app`) по зафиксированным паттернам каждого правила +
> чек-лист LAY11 по ключевым поверхностям; DOM-оболочка `crates/canvas-web`
> проверена выборочно только по LAY8.2/LAY8.4. Cargo не запускался; код и
> правила не менялись — только находки и бэклог.
> Стиль и формат — образец `design/rules-audit-2026-10.md`.

## 0. Резюме

| Направление | Находок | Критичных (P1) |
|---|---|---|
| Нарушения норматива кодом | 3 поверхности с ❌ | 2 (брейкпоинты вне SurfaceRegistry, полушаги зазоров) |
| Системные замечания (⚠️) | 11 поверхностей | — |
| Практически чисты (только P3-косметика) | 5 поверхностей | — |

Здоровые зоны: **LAY1.1/LAY1.2 держатся везде** — «вёрстки от окна в глубине
дерева» и второго расчёта геометрии для хитов не найдено (каждая поверхность
— одна чистая `layout()`-функция + hit из тех же rect'ов, тесты
`hit_slots_match_full_layout`/`bar_layout_elements_and_hits`). **LAY3/LAY4/LAY9
без нарушений**: молчаливых клампов раскладки нет (все `take(` — Option::take,
окна списков и явные политики), flex-факторы живут только в санкционированной
витрине, самодеятельных бэкендов нет, `lay_out_with` — только pilot-секции
kit-витрины (LAY9.1). **Сцена (LAY5) потребителями не используется** — и не
маскируется (`SceneOverflow::Hidden` — 0 вхождений).

Основной перекос — **LAY7 (зазоры вне шкалы S1)** и **LAY2/LAY10 (ручные
скелеты поверх ручного курсора `y +=` в витринно-демо-поверхностях и панелях
агента)**, а также **LAY8 (деградация зарегистрирована только у what-if;
тач-44 применено в одной поверхности)**.

## 1. Методика: правило → паттерны → совпадения

Все grep-прогоны по `crates/canvas-app/src` (31 файл UI-поверхностей), web —
`crates/canvas-web` (index.html, touch_platform.rs). Числа — сырые
совпадения; классификация сделана вручную (тесты/`#[cfg(test)]`, world-space
артефакты, Option::take и т.п. отнесены к своей категории).

| Правило | Grep-паттерны | Совпадений (сырых) | Вердикт |
|---|---|---|---|
| LAY1 слот/вьюпорт | `x +=`, `y +=`, `.x +`, `.y +`, `+ gap`, `gap +`; `viewport[0] <`, `w < \d{3}` | 555 (32 файла; после отсечения snap/world-арта/тестов ≈ 370 в UI-коде) | ✅ LAY1.1/LAY1.2: нарушений нет; арифметика — вопрос LAY2 |
| LAY2 примитивы | `Row {`, `Column {`, `grid_cells`, `stack(`, `constrain(`, `pad(`, `lay_out*`, `MeasuredItem`; `Custom(` | 260 в 26 файлах; `Custom(` — 0 в потребителях | ⚠️/❌: 6 поверхностей верстают руками без Custom-обоснования |
| LAY3 переполнение | `RowPolicy::`; `.take(`, `.min(` (контекст раскладки) | RowPolicy — 9 (4 код: whatif SqueezeTail, kit Wrap+SqueezeTail); take/min — 185 | ✅: молчаливых клампов раскладки нет; исключение — `row_gap: 2.0` в демо |
| LAY4 flex-факторы | `grow` | 37 сырых; flex-`grow` — 2 ряда витрины (kit_ui 893–901) | ✅: LAY4.1/4.2 не нарушены |
| LAY5 сцена | `SceneNode`, `SceneDim`, `ScenePosition`, `SceneOverflow`, `LayoutFeatures` | 0 у потребителей | ✅/—: сцена не нужна и не маскирует |
| LAY6 измеренный текст | `MeasuredItem::Text`, `TextMeasurer`, `chars().count()`, `.len() as f32`, `* 8.0/7.0` | lay_out_measured — 25 (7 файлов); MeasuredItem::Text — 25 узлов; ручных ширин — 4 живых | ⚠️: 4 ручных ширины (агент, ghost-карточки, админ-хинты) |
| LAY7 шкалы зазоров | `SPACING_`; `gap: \d`; `const *(GAP\|PAD\|MARGIN\|SPACING)* = \d`; `y += \d`; `EdgeInsets` | SPACING_* — 191 (21 файл); литеральные зазоры-константы — 76 (вне S1 ≈ 30, полушаги 7/9/11 — 13); inline-литералы в арифметике — ≈ 45; `gap: 0.0` — 7 | ❌: единственное системное нарушение (детали §4.7) |
| LAY8 адаптивность | `HideBelow`, `DegradationPolicy`, `MIN_TOUCH_TARGET`, `44.0`; web: `@media`, `pointer: coarse`, `safe-area`, `viewport-fit`, `1199` | HideBelow — 1 (what-if 900×600); Always — 1 явная (settings); брейкпоинты в теле отрисовки — 2; MIN_TOUCH_TARGET применён — 1 раз; web: 8/8 паттернов найдены | ⚠️/❌: реестр покрывает 1 поверхность из ~26; тач — 1 из 19 |
| LAY9 движки | `lay_out(`, `lay_out_with(`, `default_backend`, `pilot_backend`, `LayoutBackend` | `lay_out(` — 0 у потребителей; `lay_out_with`/`pilot_backend` — 13 (kit_ui); `default_backend` — 2 (тест) | ✅: паритет движков соблюдён, самодеятельных бэкендов нет |
| LAY10 каркасы | (сводка LAY1+LAY2+K3: `ScrollState`, `scroll_top`) | ScrollState — 8 поверхностей; своя скролл-модель — 1 (template `scroll_top: usize`) | ⚠️: см. §4.10 |

## 2. Сводная матрица: поверхности × LAY1–LAY11

✅ соблюдено · ⚠️ замечание · ❌ нарушение · — неприменимо/не используется.
Полные имена файлов: `crates/canvas-app/src/*`.

| Поверхность | LAY1 | LAY2 | LAY3 | LAY4 | LAY5 | LAY6 | LAY7 | LAY8 | LAY9 | LAY10 | LAY11 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| what-if бар (whatif_ui) | ✅ | ✅ | ✅ | ✅ | — | ✅ | ⚠️ | ✅ | ✅ | ✅ | ✅ |
| Агент-панель (app/agent_panel) | ✅ | ❌ | ⚠️ | — | — | ⚠️ | ⚠️ | ❌ | ✅ | ❌ | ❌ |
| AI-статус-панель (app/ai_status_panel) | ✅ | ❌ | ✅ | — | — | ✅ | ❌ | ❌ | ✅ | ⚠️ | ❌ |
| Настройки (settings_ui) | ✅ | ✅ | ✅ | — | — | ✅ | ⚠️ | ✅ | ✅ | ⚠️ | ⚠️ |
| Схема-галерея (scheme_gallery_ui) | ✅ | ✅ | ✅ | — | — | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ⚠️ |
| Kit-витрина (kit_ui) | ✅ | ⚠️ | ✅ | ✅ | — | ✅ | ⚠️ | ✅ | ✅ | ⚠️ | ✅ |
| Админка/UI-консоль (admin_ui) | ✅ | ✅ W13 | ⚠️ | — | — | ✅ | ⚠️ | ✅ | ✅ | ✅ W13 | ⚠️ |
| Поиск (search_ui) | ✅ | ⚠️ | ✅ | — | — | ✅ | ⚠️ | ⚠️ | ✅ | ⚠️ | ⚠️ |
| Explain (explain_ui) | ✅ | ⚠️ | ✅ | — | — | ✅ | ⚠️ | ⚠️ | ✅ | ⚠️ | ⚠️ |
| Template-панель/wheel (template_ui) | ✅ | ⚠️ | ✅ | — | — | ✅ | ⚠️ | ⚠️ | ✅ | ⚠️ | ⚠️ |
| Flowmap (flowmap_ui) | ✅ | ✅ | ✅ | — | — | ✅ | ⚠️ | ✅ | ✅ | ✅ | ✅ |
| Calc-панель (calc_panel_ui) | ✅ | ✅ | ✅ | — | — | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ✅ |
| Autolink (autolink_ui) | ✅ | ✅ | ✅ | — | — | ✅ | ⚠️ | ✅ | ✅ | ⚠️ | ⚠️ |
| Docs (docs_ui) | ✅ | ⚠️ | ✅ | — | — | ✅ | ✅ | ⚠️ | ✅ | ⚠️ | ⚠️ |
| Hints (hints_ui) | ✅ | ✅ | ✅ | — | — | ✅ | ⚠️ | ⚠️ | ✅ | ✅ | ✅ |
| Палитра выделения (palette) | ✅ | ✅ | ✅ | — | — | ✅ | ❌ | ⚠️ | ✅ | ⚠️ | ⚠️ |
| Оверлеи меню/тостов (app/overlays) | ✅ | ⚠️ | ✅ | — | — | ✅ | ⚠️ | ⚠️ | ✅ | ⚠️ | ⚠️ |
| Онбординг (onboarding_ui) | ✅ | ⚠️ | ✅ | — | — | ✅ | ⚠️ | ⚠️ | ✅ | ⚠️ | ⚠️ |
| Graph builder (app/graph_builder_ui) | ✅ | ⚠️ | ✅ | — | — | ⚠️ | ⚠️ | ⚠️ | ✅ | ⚠️ | ❌ |

## 3. Детали нарушений по правилам

### 3.1 LAY1 — слот и геометрия (✅)

- **LAY1.1** — вьюпорт как корневой слот соблюдён: what-if (`whatif_ui.rs:236`,
  `viewport[0] - BAR_MARGIN * 2.0` на корне), галерея/настройки/autolink
  (`stack` от вьюпорта), меню (`kit::dropdown_menu` — кламп слотом вьюпорта).
  «Вёрстки от окна» в глубине дерева не найдено.
- **LAY1.2 (П5)** — hit = draw подтверждён на ключевых поверхностях: what-if
  (`overlays.rs:490` — hit `scenario_closes` из тех же `BarLayout`), агент
  (`agent_panel.rs:202–205` — «единая геометрия… урок CR-033»), AI-статус
  (`ai_status_panel.rs:47–48` — «клик-зоны считаются ОДНОЙ функцией замера»),
  админка (тест `admin_ui.rs:2880 hit_slots_match_full_layout`).
- Сырые 555 арифметических совпадений — это проблема **LAY2** (кто верстает
  руками), не LAY1: слоты везде спускаются от корня.

### 3.2 LAY2 — примитивы (⚠️/❌)

Примитивы живут в 26 файлах (260 совпадений); `Custom(` у потребителей — 0
(только определение `layout.rs:628`), т.е. «экзотика без обоснования» не
встречается. Но шесть поверхностей верстают каркас вручную, без примитивов и
без Custom-обоснования (анти-паттерн LAY10 «ручная арифметика x += w + gap вне
layout/»):

- **admin_ui.rs (128 совпадений) — P2.** Тела демо-секций — ручной курсор:
  ```rust
  // admin_ui.rs:473,536,588
  y += MATRIX_HEADER_H + 6.0;
  y += kit::BUTTON_HEIGHT + 8.0;
  cx += cl.rect.w + kit::GAP_CONTROLS;
  ```
  Секции buttons/chips/fields/dropdown/toast/list/rows (строки 465–930,
  1297–1680, 2150–2460) — сплошное `y +=` с литеральными шагами 4/6/8/10/12/16.
- **kit_ui.rs (62) — P2.** Тот же ручной скелет витрины:
  ```rust
  // kit_ui.rs:435,490,535
  y += 30.0 + SECTION_GAP;
  y += kit::BUTTON_HEIGHT + 8.0;
  y += kit::CHIP_HEIGHT + SECTION_GAP;
  ```
  При этом сами демо-ряды внутри секций — образцовые (Row/lay_out_measured,
  pilot_backend). Ручной остаётся «вертикальная линейка секций».
- **app/agent_panel.rs:222–254 — P2.** Каркас панели — ручная арифметика:
  ```rust
  let close = UiRect::new(px + pw - PAD - ICON_BTN, py + (HEAD_H - ICON_BTN) * 0.5, ...);
  let row_y = py + ph - QUICK_H - INPUT_H;
  let quick_w = (pw - PAD * 2.0 - GAP * 2.0) / 3.0;
  ```
  Выражается `Column` (шапка/контекст/лог-grow/input/quick) без потери
  геометрии (единая `AgentPanelLayout` сохраняется как результат).
- **app/ai_status_panel.rs:137–260 — P2.** Ручная эмуляция CSS-flex
  прототипа (`.ais-head`) — семантика Row{gap, End}, но записана руками;
  замер единый (LAY6 ✅), примитива нет.
- app/overlays.rs, onboarding_ui.rs, graph_builder_ui.rs, app.rs (диалоги) —
  точечные ручные стопки (P3: меню/тосты уже на `kit::list_rows`, хвосты —
  локальные стеки).

### 3.3 LAY3 — политики переполнения (✅)

- `RowPolicy::SqueezeTail` — whatif_ui.rs:359 (канонический кейс LAY3.2) и
  kit_ui.rs:1054 (витринное демо). `Wrap` — kit_ui.rs:947 + `chip_strip_wrap`
  в template_ui.rs:998 (чипы категорий, max_rows=2 — LAY3.3 с явным капом).
  Остальное — `Fit`-по-умолчанию с честным переливом (галерея,
  scheme_gallery_ui.rs:351,696 — комментарии «переполнение НЕ маскируется»).
- Молчаливых клампов раскладки нет: все `.take(` — Option::take, окна
  видимости (`kit::list_rows`), явные «… ещё N»-политики (overlays.rs:85 —
  ellipsis по замеру) или sanction-срезы контента (DROP_GHOST_LABEL_MAX).
- Единственное отклонение: `row_gap: 2.0` (admin_ui.rs:869,1524;
  kit_ui.rs:792,2261; stage.rs:255) — демо-параметр вне S1 (P3).

### 3.4 LAY4 — flex-факторы (✅)

`grow` используется только в витринном демо F-14 (kit_ui.rs:893–901: fixed +
grow×2 + grow×1 с осмысленными долями 2:1, рисовалка overlays.rs:1631).
`grow` внутри SqueezeTail-рядов нет; сочетаний «несколько grow без долей» нет.

### 3.5 LAY5 — сцена SceneNode (✅/—)

`SceneNode`/`SceneDim::Percent`/`Fill`/`aspect`/`ScenePosition::`/`SceneOverflow::`
в canvas-app — 0 вхождений. Правило эскалации LAY5.1 соблюдено «сверху вниз»:
примитивы покрывают все потребности, процент/aspect/sticky пока никому не
нужны. `LayoutFeatures` не проверяется нигде — тоже корректно (нет
потребителей расширенных политик). Маскирования fit-переполнения под Hidden
нет (клипы — только surface-scissor полос, L5).

### 3.6 LAY6 — измеренный текст (⚠️)

- Эталон: whatif_ui.rs:291–351 — все элементы бара `MeasuredItem::Text` c
  `pad_x`/`min_w`/`h: Some(CHIP_HEIGHT)`; search_ui (FR-088) — высота строки
  по замеру; kit-витрина — `lay_out_measured` с Text-чипами.
- Ручные ширины (LAY6.2):
  ```rust
  // app/agent_panel.rs:548
  let bubble_h = 28.0_f32.max(user_lines.len() as f32 * 16.0 + 12.0);
  // app/agent_panel.rs:1052,1119 и app/graph_builder_ui.rs:633
  (center[0] - titles.len() as f32 * 155.0, center[1] - 60.0)
  // admin_ui.rs:270,3422
  let hint_h = 24.0 + hint_lines.len() as f32 * 18.0 + ZONE_GAP;
  ```
  `user_lines`/`hint_lines` — измеренные переносы (умножение на line-h —
  легально), но line-h 16/18 — магические константы; `* 155.0` — ширина
  ghost-карточки «на глаз» (world-space превью — P3), без замера.
- Посимвольных множителей `* 8.0/7.0` в живой раскладке нет (мок
  `chars().count() * 10.0` — только тест tooltip.rs:326).

### 3.7 LAY7 — зазоры и поля из шкал (❌ — единственное системное нарушение)

`SPACING_*` используется в 21 файле (191 вхождение), но параллельно живёт слой
литеральных зазоров. **Полный перечень литеральных зазоров** (именованные
константы поверхностей; значение против S1 {6, 8, 10, 12, 24}):

**Вне шкалы — P2:**

| file:line | Константа | Значение |
|---|---|---|
| app/ai_status_panel.rs:71–72 | PAD_TOP/PAD_BOTTOM | **9.0 (полушаг)** |
| app/ai_status_panel.rs:73 | PAD_X | **11.0 (полушаг)** |
| app/ai_status_panel.rs:78–79 | GAP_HEAD_FEATS/GAP_FEATS_COST | **7.0 (полушаг)** |
| app/ai_status_panel.rs:80–81 | GAP_COST_BAR/GAP_BAR_PAUSED | 5.0 |
| app/ai_status_panel.rs:102–105 | FEAT_PAD_X 9.0 / FEAT_PAD_Y 2.5 / FEAT_GAP 5.0 | **9.0 (полушаг)** |
| app/ai_status_panel.rs:115 | PROV_CHIP_PAD_X | **7.0 (полушаг)** |
| palette.rs:70,76,78 | PAL_GAP / PAL_DROP_PAD / PAL_DROP_GAP | 5.0 / 5.0 / 3.0 |
| palette.rs:781 | PAL_CAPTION_PAD | 2.0 |
| settings_ui.rs:92 | MODAL_THEME_GAP | 14.0 |
| settings_ui.rs:2078 | `MeasuredItem::Fixed { w: 0.0, h: 4.0 }` | 4.0 (в коде помечено «вне spacing-scale») |
| search_ui.rs:56 | TITLE_SUB_GAP | 2.0 |
| search_ui.rs:60 | ROW_TEXT_X_DOCS | 30.0 |
| hints_ui.rs:43 | HINT_ROW_INSET_H | 4.0 |
| docs_ui.rs:247 | HELP_SUBMENU_GAP | 2.0 |
| explain_ui.rs:67,288,301 | WIN_MARGIN / BODY_PAD / LAYOUT_PAD | 20.0 / 16.0 / 14.0 |
| calc_panel_ui.rs:222 | PANEL_BOTTOM_GAP | 34.0 |
| auto_width.rs:67 | HORIZONTAL_PADDING | 20.0 |
| app.rs:248,253,259 | DIALOG_PAD_X / DIALOG_BTN_GAP / DIALOG_VIEWPORT_MARGIN | 20.0 / 16.0 / 40.0 |
| app/graph_builder_ui.rs:41 | PAD | 16.0 |
| onboarding_ui.rs:685–687 | AI_ONB_PAD_X / PAD_TOP / PAD_BOTTOM | 30.0 / 26.0 / 22.0 |
| template_ui.rs:534 | STRIP_PAD_H | 4.0 |

**Значение на шкале, но литерал-константа вместо токена — P3** (дублируют
S1 и дрейфуют независимо): admin_ui.rs:32 ZONE_GAP 12, :36 SIDEBAR_ITEM_GAP 6,
:1222 FILL_CELL_GAP 8; kit_ui.rs:53 SECTION_GAP 12; template_ui.rs:95–99,532,538–544
PANEL_MARGIN 12 / PANEL_PADDING 10 / STRIP_PAD_V 6 / FLYOUT_* 6–8; onboarding_ui.rs:197,211
ONBOARDING_PAD 24 / DOT_GAP 10; lib.rs:232,246,256,550,1220 SETTINGS_MARGIN 12 /
SETTINGS_GAP 8 / PANEL_PADDING 10 / HOTKEYS_PADDING 10 / DROP_GHOST_LABEL_PAD 12;
suggest.rs:510 SUGGEST_CARD_GAP 12; app/tooltip.rs:67–76 TOOLTIP_PAD 8/6/STACK_GAP 8;
app/ai_status_panel.rs:113 HEAD_GAP 6; app/graph_builder_ui.rs:47 MODE_GAP 6;
hints_ui.rs:41 HINT_MARGIN 6; flowmap_ui.rs:58 LIST_PAD_X 8; admin/kit
VIEWPORT_MARGIN 24.

**Inline-литералы в арифметике раскладки — P2/P3** (выборка):
admin_ui.rs:473,538,588,603,644–761,901,930 (`y += … + 6.0/8.0/12.0/16.0`,
`y += 3.0 * (row_h + 2.0) + 12.0`), :268 `(demo.w - 2.0 * 10.0)`;
kit_ui.rs:1119 `gap: 6.0` (Row-литерал), :1128/1150 `3.0 * GALLERY_COLUMN_CELL_H
+ 3.0 * 6.0 + 12.0`; scheme_gallery_ui.rs:302 `w: inner_w - 32.0`;
app/explain.rs:464 `- 2.0 * 10.0`; settings_ui.rs:2078 `h: 4.0`.
Нейтральные `gap: 0.0` (7 мест: settings/template/gallery/search/stage) —
осознанный «ритм без зазора», задокументирован комментариями (P3-наблюдение:
0 не входит в S1, стоит упомянуть в LAY7 как допустимый нейтральный).

### 3.8 LAY8 — адаптивность и деградация (❌/⚠️)

- **P1. Брейкпоинты в теле отрисовки вместо SurfaceRegistry (LAY8.2):**
  ```rust
  // app/agent_panel.rs:285 и 944
  if viewport[0] < AGENT_PANEL_MIN_VIEWPORT_W || viewport[1] <= 0.0 { return ...; }
  // app/ai_status_panel.rs:276 и 589
  if viewport[0] < AI_STATUS_MIN_VIEWPORT_W || viewport[1] <= 0.0 { return ...; }
  ```
  Обе панели (600 px и 900 px) не зарегистрированы в `SurfaceRegistry`
  (рисуются напрямую в handler.rs:300–326 полосой Panels) — G4-линт их состояний
  не видит, правило «брейкпоинт обязан быть зарегистрирован там же, где
  рисуется поверхность» нарушено.
- **HideBelow** — только what-if (ui_registry.rs:188–191, 900×600 — канон П10).
  Настройки — осознанный `Always` с брейкпоинтами вёрстки (ui_registry.rs:212–221);
  но сами брейкпоинты настроек MODAL_BP_COMPACT 1280 / MODAL_BP_MOBILE 768
  (settings_ui.rs:65–70) — собственные значения в теле модуля, не в реестре
  и не в нормативе (P2: правило канонических вьюпортов 1280×800/1024×640/800×560
  не запрещает дополнительные брейкпоинты, но они нигде не зафиксированы).
- **Тач-44 (LAY8.3)**: механизм полный (`touch_targets.rs`: MIN_TOUCH_TARGET 44,
  expand/intersect/hit_zone с клампом в контейнер), применён ровно один раз —
  onboarding_ui.rs:630–633. Чипы 24–26 px, кнопки ⏸/⚙ 24 px, quick-пилюли
  и тулбары расширений не имеют (P2).
- **DOM-оболочка (LAY8.2/8.4) — ✅**: `index.html:10` `viewport-fit=cover`;
  `:1034–1045` `@media (max-width: 1199px)` тулбар в левый нижний угол +
  `max(8px, env(safe-area-inset-bottom/left))`; `:1118`, `:1183`, `:1188`,
  `:1251`, `:1532` — комплект coarse/1199-медиазапросов; hit-зоны ≥44 через
  `::after` (`:1084–1089`); Rust-сторона `touch_platform.rs:87–110` —
  matchMedia pointer:coarse + change-листенер. Расхождений с нормативом нет.

### 3.9 LAY9 — движки и паритет (✅)

Потребители пишут `Row/Column {..}.lay_out_measured(...)` (25 вызовов в 7
файлах) и не знают движка; явный `lay_out_with(pilot_backend(), …)` — только
kit_ui.rs:859–1123 (пилотные секции витрины, санкционировано LAY9.1);
`default_backend` — в тесте kit_ui.rs:2445–2463. Самодельных реализаций
`LayoutBackend` нет. Расхождения семантик (SqueezeTail ≠ flex-shrink, unsafe
End) в потребителях не эксплуатируются.

### 3.10 LAY10 — каркасы и анти-паттерны (⚠️)

Соблюдены: «панель» (kit::modal + panel_header — autolink, admin, kit),
«тулбар» (MainAlign::End в витрине), «витрина/галерея» (grid_cells — kit_ui,
scheme_gallery), «строка чипов» (chip_strip/chip_strip_wrap — gallery,
template), «модалка» (stack+constrain). Скролл — `ScrollState` кита в 8
поверхностях (flowmap, autolink, settings, kit, calc, docs, hints, palette).

Нарушения/девиации:
- ручные скелеты админ/витрины и агента (см. LAY2) — анти-паттерн «x += w +
  gap вне layout/» в масштабе сотен строк (P2);
- **своя скролл-модель**: template_ui.rs:180,331,874 — `scroll_top: usize` с
  ручным клампом `(cur + delta).clamp(0, max_scroll)` — row-оконная модель
  поверх `row_step`-сумм (ScrollState px-ориентирован; девиация осознанная,
  но нигде не зафиксирована — P3);
- зазоры/высоты вне шкал (LAY7 — выше); высоты вне S3: whatif CHIP_HEIGHT 26
  (S3 чип 24), search INPUT_HEIGHT 36 (S3 поле 30), autolink ROW_H 32 /
  GROUP_H 34 (S3 строка 26), explain CHIP_H 28, flowmap HEADER_H 36
  (S/M/L 30/38/44), calc PILL_H 20/34, ROW_H 22 — плюс известные W1-девиации
  шапок (галерея 40, explain 56, autolink 58) из rules-audit-2026-10.md.

## 4. Чек-лист LAY11 по ключевым поверхностям

Формат: ✅/⚠️/❌ по 10 пунктам LAY11 (`11-layouts.md §LAY11`). Пункт 9
(тач ≥44) для десктоп-модалей — «⚠️» означает «механизм есть, поверхность его
не применяет»; пункт 10 (витрина K4) — «—» для поверхностей, не являющихся
компонентами кита.

| Поверхность | 1 слот | 2 примитивы | 3 шкалы | 4 текст | 5 переполнение | 6 G4 | 7 HideBelow | 8 hit=draw | 9 тач-44 | 10 K4 |
|---|---|---|---|---|---|---|---|---|---|---|
| what-if бар | ✅ | ✅ | ⚠️ чип 26 | ✅ Text | ✅ SqueezeTail | ✅ lint_whatif | ✅ 900×600 | ✅ | ⚠️ | — |
| Агент-панель | ✅ | ❌ ручной каркас | ⚠️ 44/44, line-h 16 | ⚠️ | ⚠️ лог стопкой | ❌ нет состояния | ❌ в теле, 600 | ✅ | ❌ | — |
| AI-статус-панель | ✅ | ❌ ручной flex | ❌ 5/7/9/11 | ✅ | ✅ ellipsis | ❌ нет состояния | ❌ в теле, 900 | ✅ | ⚠️ 24px-кнопки | — |
| Настройки | ✅ | ✅ | ⚠️ 14/4 | ✅ | ✅ | ✅ lint_settings | ✅ Always+бп | ✅ | ⚠️ | — |
| Схема-галерея | ✅ | ✅ | ⚠️ 40/34/62 | ✅ | ✅ Fit честный | ✅ lint_gallery | ⚠️ кламп 320×240 | ✅ | ⚠️ | — |
| Kit-витрина | ✅ | ⚠️ скелет y+= | ⚠️ gap 6.0 литерал | ✅ | ✅ демо Squeeze/Wrap | ✅ lint_kit_gallery | ✅ Block | ✅ | ⚠️ | ✅ |
| Админка | ✅ | ✅ Column-скелет (W13) | ⚠️ 16/18/20-шаги | ✅ | ⚠️ row_gap 2.0 | ✅ lint_admin | ✅ Block | ✅ тест | ⚠️ | — |
| Поиск | ✅ | ⚠️ смесь | ⚠️ поле 36, gap 2 | ✅ FR-088 | ✅ MAX_VISIBLE_ROWS | ✅ lint_search | ⚠️ кламп | ✅ | ⚠️ | — |
| Explain | ✅ | ⚠️ tidy-дерево | ⚠️ 56/28/20 | ✅ | ✅ take_while крошки | ✅ lint_explain | ⚠️ min 320×240 | ✅ | ⚠️ | — |
| Template-панель | ✅ | ⚠️ rows вручную | ⚠️ 4/26/52 | ✅ | ✅ Wrap+«приклейка» | ✅ lint_template×2 | ⚠️ | ✅ | ⚠️ | — |

Примечания к пункту 6: G4-состояния отсутствуют также для flowmap, calc,
hints, graph_builder, палитры (частично — lint_palette_selected есть) —
слепые зоны линта перечислены в бэклоге LAY-W11.

## 5. Бэклог

- **LAY-W1 (P1, M).** Агент-панель и AI-статус-панель: регистрация в
  `SurfaceRegistry` с `HideBelow { 600, 0 }` / `{ 900, 0 }` (проверить
  min_h), перенос решения о показе из тел `agent_panel_overlay`/
  `ai_status_panel` в реестр; канонические G4-состояния
  `lint_agent_panel_open`/`lint_ai_status_open` в ui_layout_lint.rs.
- **LAY-W2 (P1, M).** Полушаги зазоров ai_status_panel (5/7/9/11) и
  off-scale 5/3/2 палитры: решение владельца по I-1 — либо миграция на S1
  (визуальный сдвиг 1–2 px), либо фиксация значений в LAY7/S4 как
  задокументированные отклонения «CSS-прототип F-7.9».
- **LAY-W3 (P2, L).** Админка + kit-витрина: перевод вертикальных линеек
  демо-секций с ручного курсора `y +=` на Column-скелет из примитивов
  (или новый санкционированный каркас «линейка витрины» в LAY10 с запретом
  вне этих двух файлов).
  ✅ реализовано 2026-10-10 повторно (LAY-W13): прежний коммит W3a e319cc1
  УТЕРЯН (не существует в истории ни одной ветки; миграция в main не
  попадала, admin_ui сохранял 32 вхождения `y +=` — вскрыто ревью
  [`layouts-w1-w12-review.md`](layouts-w1-w12-review.md) §3.1 P1-1).
  Миграция admin_ui выполнена заново (LAY-W13, ветка `lay/w13-admin-skeleton`):
  32 y+= → 0, тела — Column/Row-скелет примитивов через `pilot_backend()`,
  дрейф 0 — golden `admin_bodies_golden_column_skeleton` (135 тегов,
  допуск 0.005 px) бит-в-бит; row_gap 2.0 — задокументированное демо-
  исключение, не тронуто.
  W3b (kit_ui) — `wave/lay-w3b-kit` (ребейз на W1–W12, слияние 4d58afa),
  y+= 0, golden `gallery_layout_ruler_golden_column_skeleton` (65 тегов).
  Кандидат в LAY-W10: `pilot_backend()` в не-пилотных потребителях.
- **LAY-W4 (P2, M).** agent_panel: `AgentPanelLayout::build` → Column/Row
  (шапка/контекст/лог-grow/input/quick), результат — та же структура;
  quick-пилюли через Row с равными долями.
  ✅ реализовано 2026-10-10 (`wave/lay-w4-agent-panel`, ребейз на W1–W12,
  слияние a6e2acd): build() — Column из 5 полос (лог-grow — нативная
  flex-доля), quick — Row с `Child::flexible(…, 1.0)` ×3; структура
  `AgentPanelLayout` и все потребители без изменений; дрейф 0 (проб на
  6 панелях бит-в-бит); 4 golden-теста (канон/узкая/широкая/дробная панели);
  backend — `pilot_backend()` (кандидат в LAY-W10).
- **LAY-W5 (P2, S).** settings: брейкпоинты 1280/768 вынести в декларацию
  поверхности (константы реестра/правил) — LAY8.2 «брейкпоинт зарегистрирован
  там же, где рисуется поверхность».
- **LAY-W6 (P2, M).** Механическая волна: литеральные зазоры-константы →
  псевдонимы токенов (паттерн search_ui.rs:24–38 `use SPACING_MD as ...`)
  по перечню §3.7-P3; off-scale значения — отдельно по решению LAY-W2.
- **LAY-W7 (P2, M).** Высоты вне S3 (what-if 26, search 36, autolink 32/34,
  explain 28/56, flowmap 36, calc 20/34, agent 44/44/32/36): канонизация на
  S3 или фиксация в S3 как отдельные строки (частично пересекается с W1
  аудита rules-audit-2026-10.md — шапки 40/56/58).
- **LAY-W8 (P2, M).** Тач-44 покрытие: расширение hit-зон через
  `touch_targets::hit_zone` для чипов what-if (26 px), quick-пилюль агента,
  кнопок ⏸/⚙ AI-статуса (24 px), пунктов палитры/тулбаров на coarse.
- **LAY-W9 (P3, M).** template_ui: обёртка row-скролла над `ScrollState`
  (px-оффсет = сумма row_step) либо фиксация row-window паттерна в K3.
- **LAY-W10 (P3, S).** Раздел «Исключения» в 11-layouts.md: documented
  девиации (explain tidy-дерево, docs GFM-блоки/таблицы, wheel-меню,
  `gap: 0.0`-ритм, row_gap 2.0 демо) — со ссылками из кода.
- **LAY-W11 (P3, S).** G4-линт: канонические состояния для graph_builder,
  flowmap, calc-панели, hints — закрыть слепые зоны линта.
- **LAY-W12 (P3, S).** CI-гейт LAY7: grep-линт `gap: <литерал не из {0, SPACING_*}>`
  и `y += \d+\.\d` в canvas-app (допуск — список из §3.7-P3 до волны W6).

## 6. Итоговая статистика

- Поверхностей рассмотрено: **19** (матрица §2).
- Практически чисты (только P3-косметика): **5** — flowmap, calc, hints,
  autolink, docs.
- С замечаниями (⚠️ без ❌): **11** — what-if, настройки, схема-галерея,
  kit-витрина, поиск, explain, template, палитра*, оверлеи, онбординг,
  graph-builder. (*палитра — ❌ по LAY7, но изолированные константы; отнесена
  к нарушениям в §3.7.)
- С нарушениями (❌): **3** — агент-панель, AI-статус-панель (LAY2+LAY7+LAY8),
  админка (LAY2/LAY10); палитра — точечное ❌ LAY7 (итого 3+1).
- Сырых grep-совпадений по методике: ≈ **1 300** (LAY1 555 · LAY2 260 ·
  LAY3 185+9 · LAY4 37 · LAY6 25+4 · LAY7 191+121 · LAY8 13 · LAY9 15);
  после классификации — **≈ 60 находок**: 3 P1, 9 P2-кластеров, ~15 P3.
- DOM-оболочка (LAY8.2/8.4): **8/8** нормативных паттернов присутствуют —
  нарушений нет.
