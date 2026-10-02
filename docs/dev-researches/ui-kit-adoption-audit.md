# Аудит внедрения ui-kit и адаптива: какие поверхности и компоненты остаются вне кита и на хардкоде

**Дата:** 03.10.2026
**Автор:** агент-сессия (Super Z)
**Тип:** технический аудит / инвентаризация
**База:** commit `e3eb854`
**Связанные документы:** `docs/ui-kit.md` (гайд PRD-0009), `docs/interface-objects/surface-registry.md`, `docs/change-requests/fr-059/060/062/068`, `docs/dev-researches/node-layout-adaptivity-analysis.md`, CR-014

---

## 1. Постановка вопроса

Владелец попросил детально проанализировать, **какие поверхности и компоненты UI до сих пор остаются не на ui-kit и на хардкоде, без адаптива, указанного в концептуальном документе по UI**. Аудит фиксирует фактическое состояние кода против эталона `docs/ui-kit.md` (PRD-0009 + волны FR-059/060/062/068) и сортирует находки по категориям с приоритетами. Код не менялся — только наблюдения.

## 2. Эталон: что считаем «на ките» и «адаптивом»

Источник требований — `docs/ui-kit.md`:

- **Геометрия** — только layout-примитивы `canvas-ui` (`Row`/`Column`/`stack`/`constrain`/`pad`/`grid_cells`/`lay_out_measured`/`list_rows`+`ScrollState`/`dropdown_menu`/`kit::modal`); escape-hatch `Custom` — только с обоснованием.
- **Цвета** — только слоты `KitPalette`/`ThemeColors` (источник — `canvas-render/theme.rs`, примитивы — `canvas-core::tokens.rs` + `design/tokens/*.json`); литералы RGBA/hex вне theme = дефект.
- **Отступы/радиусы** — только токены `SPACING_S/SM/MD/LG/XL`, `RADIUS_CHIP/PANEL/PILL`.
- **Текст** — только `TextMeasurer` + `ellipsis`/`wrap`; символьные эвристики («chars × коэффициент», `chars.truncate`) запрещены (класс CR-015).
- **Адаптив** — референс-окна G4 1280×800 / 1024×640 / **800×560**; деградация `DegradationPolicy::HideBelow`; клампы панелей/меню во вьюпорт (flip у dropdown'ов); политики `SqueezeTail`/`Wrap` вместо молчаливых `take`/`break`; скролл через `ScrollState`.
- **Модель поверхностей** — всё интерактивное объявлено в `ui_registry.rs` (hit-rect'ы из тех же функций, что рисование; ввод через HitStack; G4-линт видит поверхность).

## 3. Сводная карта поверхностей (26 идентификаторов реестра + вне-реестровые)

| Поверхность | Слой | Kit-статус | Хардкод-цвета* | Адаптив-риск |
|---|---|---|---|---|
| `whatif` | Panels | **на ките** (пилот measured, SqueezeTail, HideBelow 900×600) | 3 в draw (overlays) | список/таблица не клампятся по высоте |
| `hints` (popup) | Popups | **на ките** (модель) — **вне реестра** | 2 в draw | фикс-колонки без ellipsis |
| `flow_map` | Panels | **на ките** | 2 в draw | низкий |
| `gallery` | Modals | **на ките ~75%** | 0 (радиусы литералами) | нет колес-скролла; чипы фикс. 108px без Wrap |
| `calc_panel` (в stage) | Modals | **на ките ~65%** (list_rows/stack/Table v2) | 0 | **мин-ширина 600 > stage 560 на 800×560** |
| `autolink` | Modals | **на ките** (kit::modal+list_rows+ScrollState) | 0 | низкий (parity-тесты 7 вьюпортов) |
| `dialog` | Modals | геометрия на ките, draw сырой | 0 (токены) | низкий (эталон зоны) |
| `empty` | Panels | геометрия на ките, draw сырой | 0 | `.take(2)` — строки описания теряются |
| `search` | Panels | вёрстка на ките, но **модуль в canvas-render** | 3 в draw | сильный (клампы, узкое окно −40px) |
| `stage` | Modals | частично (Table v2 + Painter; пилюли/подписи — hand) | 1 (0x14161c) | ручные формулы; truncate_chars ×10 |
| `explain` | Modals | частично (kit::modal; дерево — documented hand) | 0 в модели; **13+ в draw** | сильный сам по себе; **нет scissor** у прохода |
| `menu` (ПКМ) | Popups | частично (rows=list_rows; панель сырая) | 2 в draw | **нет клампа/флипа к вьюпорту**; на 800×560 хвост пунктов клипуется, скролла нет |
| `help_menu` | Popups | hand-rolled (draw) | 2 в draw | низкий (кламп+флип есть) |
| `choice_menu` | Popups | hand-rolled (draw) | 1 в draw | низкий (кламп есть); нет ellipsis |
| `hotkeys` | Panels | частично (rows=list_rows, ширина от замера) | 0 | **hit-rect реестра без измеренной ширины** |
| `corner_buttons` | Panels | hand-rolled | 0 | низкий (кластер 168px); иконки — ручные квады |
| `palette` | Widgets | частично ~55% (dropdown_menu/list_rows; бар и иконки — hand) | **6 в draw** (тёмная тема зашита) | бар не режется по ширине вьюпорта |
| `template_panel`/`template_strip` | Panels | **частично ~35%** (док на примитивах; strip/flyout/окно строк — hand) | 0 | MAX_VISIBLE_ROWS=12 константой; пагинация break'ами |
| `wheel` | WorldOverlay | hand-rolled (polar Custom, обосновано) | 0 | `WHEEL_TPL_TEXT_CHARS=15` — эвристика |
| `settings` | Modals | частично (примитивы v2 формально; dropdown/switch — свои) | 0 | **нет скролла контента: на 800×560 таб «Профиль» (520px) переливается**; BP_COMPACT/MOBILE — мёртвые |
| `docs` | Popups | **hand-rolled** (свой ScrollState-дубль, свои меню/таблицы) | 0 | viewer ок; сниппет — 44 символа срезом |
| `onboarding` | Modals | **hand-rolled (0 kit-API)** — заморожен владельцем | **6 в draw** | кламп без полей; перенос — эвристика 0.62 |
| `editor` | Widgets | by design вне app-геометрии (canvas-render) | 2 в hints-draw | низкий |
| `kit_gallery` | Modals | на ките (витрина) | 0 | эталон |
| `admin_panel` | Modals | на ките (FR-070) | 2 | низкий |
| `minimap` | Panels | **вне кита (осознанно)** — свой GPU-проход | 0 (все из tokens) | скрытие <252×172; размеры 0% токенизированы |
| HUD (F3) | — вне реестра | вне кита (осознанно) | 1 (#14161c) | низкий |
| `suggest` (карточки) | — **вне реестра** | вне кита (ручная геометрия) | 0 | **карточки налагаются у нижнего края** |
| tooltips | — вне реестра (passive) | частично (TextMeasurer; kit-Tooltip/wrap нет) | 7 в handler.rs | перенос по числу слов, не по ширине |
| toast (каркас) | Passive | hand-rolled | 0 | нет подложки, avoid-бара вручную |
| debug overlay | Debug | вне кита (диагностика, задокументировано) | 12 (допустимо) | низкий |

\* «в draw» — литералы в draw-стороне (`app/overlays.rs`, `app/explain.rs`, `app/handler.rs`, `app/stage.rs`), тогда как модельные файлы поверхностей чисты.

## 4. Категория A: полностью hand-rolled (вне кита)

### A1. Onboarding (`onboarding_ui.rs`, 526 строк) — 0 kit-API
Соответствует документации («заморожен владельцем», ui-kit.md §9). Рисование — `CardInstance`/`OwnedScreenText` мимо Painter (`overlays.rs:3286-3430`); hover — `hover_fill` (ад-хок «ярление» ×1.3+0.04, `support.rs:1152-1159`) вместо `WidgetState`/`button_style`. В draw-стороне 6 литеральных цветов (`overlays.rs:3305,3341,3394,3399,3401,3403-3412`). Перенос тела — символьная эвристика `CHAR_W_FACTOR 0.62` + ручной цикл (`onboarding_ui.rs:31-48,222-254`), хотя kit-`wrap` доступен. Кламп карточки к окну есть (тест до 240×180), но без полей и контент при клампе высоты не ужимается — строки могут уйти под футер.

### A2. Docs-вьюер (`docs_ui.rs`, 1299 строк)
Из кита только `TextMeasurer`. Вся геометрия — ручная: меню «?» (`:255-315`), viewer (`:368-388`), `PageBuilder` на явных y (`:624-685`), таблицы с ручным сжатием колонок (`:790-870`). Свой `ScrollState` (`:877-906`) — параллельная реализация kit-овского. Сниппет поиска — срез по числу символов (`SEARCH_SNIPPET_MAX=44`, `:117-126`) — CR-015-класс. Адаптив неплох: ширина `min(480, viewport)`, клампы/флип меню, пропорциональное сжатие таблиц — но всё самописное.

### A3. Help-menu / choice-menu / corner buttons (draw-сторона)
Панели, hover и иконки — сырые квады (`overlays.rs:2840-2880`, `3039-3101`, `4044-4166`); состояния — ручной `point_in_rect` вместо `WidgetState`; иконки ⚙/темы собраны из квадов вручную (`:4060-4112`) — кит-`IconButton`/`icon_glyph` не задействован. Ширины меню — константы под конкретные RU-подписи («ширина под 28 символов × 7px» — комментарий `lib.rs:154-158`).

### A4. Explain-draw (`app/explain.rs:520-1382`) — документированный остаток FR-060
`explain_frame` рисует прямыми `screen_rect_quad`/`OwnedScreenText` (16 мест), состояния — ручные `point_in_rect`; лоадер и безье — вручную. `autolink_frame` рядом — уже на Painter+WidgetState (образец).

### A5. World-слой (осознанно вне кита, по докам)
`wheel` (polar-геометрия, escape-hatch `Custom` с обоснованием), `minimap` (собственный GPU-проход + wgsl, вне ScreenBand), HUD (кадр `frame.hud`, вневополосный текст), `cards.rs` (карточки нод/рёбра/порты, LOD). Это задокументированные world-декорации — не дефект, но в инвентаре хардкода учтены.

## 5. Категория B: частичная миграция (детальные находки)

### B1. `menu` (ПКМ) — главный адаптив-риск зоны
Строки на `list_rows`+`WidgetState` (FR-060), но панель/подменю — сырые `CardInstance` (`overlays.rs:2895-2966`). **Меню не клампится и не флипается к вьюпорту**: `origin: self.cursor` (`input.rs:3307-3310`), `menu_rect_for` — чистая формула без вьюпорта (`lib.rs:1146-1153`); подменю всегда вправо `menu.x + 240 + 2.0` (`lib.rs:1377-1379`). На 800×560 меню из 13 пунктов (≈350px) при ПКМ в нижней трети клипуется — хвост недостижим (скролла нет; ScrollState в коде — мёртвая декорация с `offset: 0.0`, `overlays.rs:2919-2923`). G4-линт это не видит: ставит меню в фикс. origin `[400,300]` и без выделения (`ui_layout_lint.rs:289-296`). Контраст: `choice_menu` и `help_menu` умеют клампить/флипать.

### B2. `settings` — номинальная миграция + нет скролла
W3.2 формально исполнен, но ВСЕ дети — `MeasuredItem::Fixed` (`settings_ui.rs:1204-1361`), замерщик создаётся и не читается (комментарий `:1160`) — текст в геометрии не участвует. Собственный dropdown (`dropdown_layout`, `:1419-1454`) вместо kit `dropdown_menu` (flip у кита встроен); свой pill-тумблер (`:1090-1121`) вместо kit `switch`; свой `DropdownState` с клавиатурой вместо FocusRing/WidgetState. **Скролла правой панели нет**: на 800×560 контент-зона ≈278px, а таб «Профиль» = 10 строк × 52 = 520px — перелив за низ. `MODAL_BP_COMPACT=1280`/`BP_MOBILE=768` — объявлены и не реализованы («поведение ниже точки в v1 не реализуется», `:46-51`).

### B3. `template_panel`/`template_strip` — ~35%
Развёрнутый док — на примитивах (pad/Column/Row-Wrap/measured-чипы/stack, `template_ui.rs:793-971`) — образец. Hand-rolled: `dock_strip_layout` (`:539-586`), `flyout_layout` с ручными клампами вместо kit-dropdown (`:610-652`), окно строк циклом с двумя `break`-клампами и собственным scroll-счётчиком (`:922-1031`). `MAX_VISIBLE_ROWS=12` — кэп, не зависящий от высоты окна. Эвристика подписи сектора колеса: `WHEEL_TPL_TEXT_CHARS=15` + `split_two_lines` по `chars().count()` (`:1068,1228-1256`).

### B4. `palette` — draw-сторона ломает светлую тему
Модель на kit (dropdown_menu+flip, list_rows, ScrollState), но **draw-сторона несёт 6 литеральных цветов** (`overlays.rs:3446` accent, `:3459,:3521` рамки, `:3473` hover, `:3475` заливка кнопки — тёмная тема зашита, `:3535` selected) — светлую тему палитра не получает. Бар раскладывается ручным `bx += w + PAL_GAP` (`palette.rs:845-890`); 60+ ручных квадов иконок (`:1131-1329`). Эвристика ширины кнопок групп: `chars × 5.5 + 2.0` (`:777-779`) — управляет шириной всего бара; бар не режется по правому краю вьюпорта. Подписи строк в колонке 176px — без ellipsis.

### B5. `stage` — Table v2 есть, подписи на эвристиках
Панель «Как считается» — 2× kit `Table` + Painter (`stage.rs:81-257`) — эталон. Но каркас-проход, пилюли, подписи, мини-карточки — ручные формулы (`:692-760`, `:1629-1656`); **10× `truncate_chars`** (посимвольное усечение, `support.rs:32-39`) и символьные оценки ширины 7.2/6.3 px/символ (`:640,:795,:1149,:1321`). Один сырой hex: `0x14161c` (`:1091`).

### B6. `hotkeys` — расхождение pick ↔ draw
Ширина панели дотягивается по самому длинному описанию через TextMeasurer (`overlays.rs:4177-4190`), но hit-rect реестра считает панель без измеренного расширения (`ui_registry.rs:868-876`) — правая часть видимой панели не пикается (нарушение «ввод = тому, что видно»).

### B7. `search` — вёрстка на ките, модуль вне canvas-app
Живёт в `canvas-render` (исключение из модели), в реестре, hit из тех же функций. Вёрстка — constrain/stack/pad/Column::lay_out_measured + ellipsis/wrap (`search_ui.rs:419-543`) — G5-чисто. Draw-путь (`overlays.rs:645-769`) — 3 литерала-дубля токенов (рамка `[0.22,0.24,0.30,0.9]` = WHEEL_BORDER; selected `[0.18,0.29,0.48,0.95]`; hover `[0.24,0.30,0.42,0.6]`). Локальная шкала констант (460/36/13/11/12/8/10/6) мимо SPACING_*/RADIUS_*.

### B8. `tooltip` — перенос по числу слов
Ширина бокса измеряется честно (`tooltip.rs:143-163`), но перенос — `wrap_words(text, 10)` по счёту слов (`:99-118`), а не `TextMeasurer::wrap` по ширине: 10 длинных слов шире окна → бокос клампится и текст молча клипается. 7 литералов цвета в `handler.rs:421-586` (error = значение токена ERROR, но литералом; 0xd4d4d4; янтарь 0xf5a623; голубой 0x9cc3e6). Локальные константы дублируют кит-`TOOLTIP_OFFSET`.

### B9. `suggest` — вне реестра и наложение карточек
Карточки AI-подсказок рисуются в band Popups мимо реестра/HitStack/G4 (`handler.rs:387`), hit-тесты — ручные циклы в `app.rs:5778-5790`. **Дефект клампа**: переполняющие нижний край карточки получают одинаковый `y = vh−4−H` (`suggest.rs:560-568`) — стопка полностью налагается. Кегли/радиусы — литералы в draw.

## 6. Хардкод цветов — консолидация

| Место | Кол-во | Суть |
|---|---|---|
| `app/explain.rs` | **13+** | 11× белый «текст на accent» (`:375…:1346`), рамка тултипа `[0.22,0.24,0.30,0.9]` (`:1518`), тинт `[1,1,1,1]` (`:1117`) |
| `overlays.rs` (palette draw) | 6 | тёмная тема зашита (`:3446-3535`) — ломает светлую |
| `overlays.rs` (onboarding draw) | 6 | фон-затемнение, CTA, вторичная, рамки, точки |
| `overlays.rs` (повторяющийся hover) | 6 | `[0.24,0.30,0.42,0.9]` и `[0.18,0.29,0.48,0.95]` — меню/подменю/choice/help/hints/flowmap = значения `palette_hover_fill`/`WHEEL_HOVER`, но литералами (кандидат на слоты `control_hover_fill`/`control_selected_fill`) |
| `handler.rs` (tooltips) | 7 | 4 уникальных, один = значение токена ERROR |
| `overlays.rs` (whatif/search/flowmap draw) | 5 | accent 0x4ca6ff; рамки `[0.35,0.40,0.50]`; border `[0.22,0.24,0.30,0.9]` = WHEEL_BORDER |
| `cards.rs` | 15 | 12 пресетов JSON Canvas (dark+light), фолбэк чипа #31b8a6, ROW_PORT_FILL/RING |
| `stage.rs` | 1 | `0x14161c` (`:1091`) |
| `text.rs` | 1 | `chip_text_color` #14161c — «тёмный на любой теме» (FR-075, осознанно) |
| `debug_overlay.rs` | 12 | диагностические, задокументированы как исключение |
| `canvas-web/index.html` | ~25 | две локальные DOM-палитры (тулбар+пикер+тур), не связаны с ThemeColors |

## 7. Хардкод размеров и дыры в токенах

- Радиусы `6.0`/`8.0`/`10.0` повторяются десятками мест литералами вместо `RADIUS_CHIP/PANEL/PILL` (значения совпадают — «ноль скачка», но токены не подключены).
- Крупные именованные шкалы мимо токенов: search (460/36/…), minimap (220×140+16 — 0% токенизации при 100% токенизации цветов), HUD_PADDING=12, калькулят (VARS_COL_W=360), меню (240/26/6), palette (PAL_* ×11), template (PANEL_WIDTH=340 и ещё ~25), explain_ui (~35 констант), settings (~30), autolink (~22).
- **Рассинхрон зеркала токенов**: 5 dimension-токенов из `design/tokens/dimensions.json` (edge.arrow_angle_deg/arrow_dots/dash_period/dash_duty/dot_spacing) живут только локальными константами `cards.rs:876-886` без паритет-теста; 3 motion-токена — в `canvas-render/animate.rs:18-26` мимо tokens.rs.
- **Рассинхрон дубликата**: `canvas-widgets::layout::HEADER_H = 28.0` с комментарием «совпадает с cards::HEADER_HEIGHT», фактический `CARD_HEADER_HEIGHT = 34.0` (FR-023 поднял, дубль не обновили) — контент/хит-тест WebView-виджета смещены на 6px.
- Семантический слой `ThemeColors` (~45 слотов) существует только в Rust — в `design/tokens/colors.json` его нет (JSON покрывает примитивы).
- Межстрочный множитель экранного текста `1.3` захардкожен (`text.rs:418`); LOD-пороги (0.25/0.6/1.5) вне токенов.

## 8. Дефекты адаптива — приоритизированный список

1. **Меню ПКМ без клампа/флипа** (`input.rs:3307`, `lib.rs:1146,1377`) + мёртвый скролл + слепая зона G4-линта — на 800×560 пункты недостижимы.
2. **Settings: нет скролла контента** — на 800×560 таб «Профиль» переливается за низ модали; BP_COMPACT/BP_MOBILE — мёртвые константы.
3. **Calc-panel: мин-ширина 600px при stage 560px на 800×560** (`calc_panel_ui.rs:318` против `bundles.rs:237`) — панель шире среза; G4 не ловит (у stage в реестре только rect среза).
4. **Gallery: нет колес-скролла** (только стрелки; в `on_mouse_wheel` ветки gallery нет) — каталог из 10+ схем на 800×560 прокручивается клавиатурой; ряд чипов фиксированный (108×N+56) — новая категория = переполнение (Fit не маскирует, Wrap нет).
5. **Suggest: наложение карточек у нижнего края** (`suggest.rs:560-568`).
6. **Hotkeys: pick уже видимой панели** (реестр без измеренной ширины, `ui_registry.rs:868` vs `overlays.rs:4187`).
7. **Whatif: высота списка подмен/таблицы не клампится по вертикали** (`whatif_ui.rs:392,460`) — на 900×600+ с десятками подмен уезжает за верх; чип «База» измеряется по вшитой RU-строке, а рисуется переводом (EN «Base» — контракт FR-053 нарушен, `whatif_ui.rs:187` vs `overlays.rs:345`).
8. **Palette: бар не режется по ширине вьюпорта**; подписи колонки 176px без ellipsis; draw ломает светлую тему.
9. **Explain: у модального прохода нет scissor** (комментарий `app/explain.rs:1302-1305`) — против духа FR-056; каретка по эвристике `chars×12×0.62` (`:1426`).
10. **Tooltip: перенос по числу слов** — неразрывные токены (пути) молча клипаются.
11. **Template strip: MAX_VISIBLE_ROWS=12 константой** — на высоких окнах список не растёт; flyout — ручные клампы.
12. **Onboarding: кламп без полей, контент не ужимается** (заморожен, но зафиксировать).
13. **Единственная HideBelow-деградация в реестре — whatif**; у остальных поверхностей деградация не объявлена вовсе.
14. **Web-shell: тач-цели ниже 44px** — тур-кнопки 36px, кнопки ролей ≈40px, тулбар ≈42px (после CR-014).
15. **Empty-state: `.take(2)`** — строки описания после второй теряются молча (`overlays.rs:1796-1799`).
16. **canvas-widgets: HEADER_H 28 vs 34** — контент/хром/хит виджета расходятся на 6px.

## 9. Остатки класса CR-015 (символьные эвристики)

| Место | Эвристика | Замена |
|---|---|---|
| `app/explain.rs:1426` | каретка: `chars × 12 × 0.62` | `TextMeasurer::width_of` |
| `palette.rs:777-779` | кнопки групп: `chars × 5.5 + 2.0` | `MeasuredItem::Text` |
| `template_ui.rs:1068,1228` | `WHEEL_TPL_TEXT_CHARS=15`, `split_two_lines` по chars | `TextMeasurer::wrap` |
| `support.rs:32-39` | `truncate_chars` — единый источник, 10 вызовов в stage | `ellipsis` |
| `stage.rs:640,795,1149,1321` | ширины пилюль: `chars × 7.2/6.3` | TextMeasurer |
| `onboarding_ui.rs:31-48` | `CHAR_W_FACTOR 0.62` + ручной перенос | kit `wrap` (после разморозки) |
| `app/tooltip.rs:99-118` | `wrap_words(text, 10)` по числу слов | kit `wrap` по ширине |
| `docs_ui.rs:117-126` | сниппет: срез 44 символа | measured-срез (спорно: контентный поиск) |
| `overlays.rs:1796-1799` | `.take(2)` строк empty-state | `TextMeasurer::wrap` + индикатор |
| `auto_width.rs` | 420 из «10 слов × 6 chars × 7px», замерщик в сигнатуре не читается | либо убрать замерщик из сигнатуры, либо измерять |

## 10. Рекомендуемый порядок закрытия (волны)

**W-a (дефекты адаптива, малая кровь):** кламп/флип меню ПКМ и подменю + краевой кейс в G4; скролл настроек (list_rows+ScrollState); кламп высоты whatif-списка/таблицы; фикс наложения suggest; колесо галереи; синхронизация hotkeys hit-rect с измеренной шириной; HEADER_H в canvas-widgets → токен.

**W-b (слоты цветов):** 6 литералов palette-draw → `control_*` слоты (чинит светлую тему); общий hover/selected литералы (6 мест) → слоты `control_hover_fill`/`control_selected_fill`; тултипы handler.rs → токены; 11× белый в explain → слот `text_on_accent`; 0x14161c → токен.

**W-c (догоняющая миграция кита):** explain_frame → Painter+WidgetState (последний остаток FR-060); settings dropdown/switch → kit; docs-меню/таблицы → примитивы + kit ScrollState; template strip/flyout → dropdown_menu/list_rows; corner buttons → IconButton; tooltip → kit-Tooltip + wrap.

**W-d (токены):** подключить SPACING_*/RADIUS_* вместо литералов-дублей; достроить зеркало tokens.rs (5+3 токенов без паритета); HEADER_H и LOD-пороги → dimensions.json; решить, входит ли семантический слой ThemeColors в colors.json.

**W-e (решения владельца):** BP_COMPACT/BP_MOBILE (реализовать или удалить); тач-цели web ≥44px; разморозка onboarding; пагинация template-строк → ScrollState.

## 11. Источники

- Модель/геометрия: `crates/canvas-app/src/{menu-зона: lib.rs, overlays.rs, input.rs, handler.rs}`; поверхностные модули `*_ui.rs`; `app/{ui_registry,ui_layout_lint,tooltip,explain,stage,support}.rs`
- Рендер-UI: `canvas-render/src/{search_ui,minimap_pass,text,cards,theme,tokens-зеркало}.rs`; `canvas-core/src/tokens.rs`
- Web: `canvas-web/index.html`, `src/toolbar.rs`; world-виджеты: `canvas-widgets/src/layout.rs`
- Доки-эталон: `docs/ui-kit.md`, `docs/interface-objects/surface-registry.md`, FR-059/060/062/068, CR-014, CR-015
