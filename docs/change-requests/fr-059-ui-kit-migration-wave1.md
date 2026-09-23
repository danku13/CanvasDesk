# FR-059: Миграция волна 1 — hints_ui, flowmap_ui, calc_panel_ui на кит v2 + витрина kit_gallery пополняется компонентами v2

- **Статус:** ✅ реализовано (2026-09-23)
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** агент (реализация по приказу владельца «реализуй FR-059»)
- **Источник:** гэп-анализ кита (сессия 2026-09-23): 3 модуля рисуют UI полностью hand-rolled (0 обращений к canvas_ui)
- **Связанные задачи:** PRD-0009 §8/§13; FR-057 (Painter/WidgetState), FR-058 (компоненты v2), FR-056 (scissor — G5-опора); паттерн миграции — FR-053/FR-054 (U3/U5)
- **Создан:** 2026-09-23
- **Обновлён:** 2026-09-23

## Описание
Три потребительских модуля не используют каркас вообще: `hints_ui.rs` (414 строк — popup-подсказки Numi-ввода), `flowmap_ui.rs` (355 — карта проливаний), `calc_panel_ui.rs` (614 — панель «Как считается»). Их раскладка/стили/hover — ручная математика с литеральными константами. Перенести на кит v2 (Panel/Card/список+скролл/Dropdown/WidgetState/Painter) по паттерну U3/U5 (числа дословно, 0 визуального скачка). Витрина `kit_gallery` (`app.rs:5848` `kit_gallery_overlay`) получает секции новых компонентов (компоненты × состояния × RU/EN × темы — контракт FR-055). Выявлено агентом.

## Влияние
| Объект | Что меняется | Где в документации |
|---|---|---|
| `hints` (popup подсказок) | раскладка на kit-компоненты, кламп → dropdown flip | `docs/interface-objects/` (страница подсказок, если заведена), PRD-0009 §15 G5 |
| `flowmap` (карта проливаний) | карточки/строки → Card/список | FR-050 (Н9-4), PRD-0009 G5 |
| `calc_panel` («Как считается») | панель/строки → Panel/список | FR-044 (Р-4/Р-5/Р-8), PRD-0009 G5 |
| `kit_gallery` (витрина) | + секции TextField/Switch/Card/список/Icon | `docs/ui-kit.md` §7, контракт витрины FR-055 |

## Анализ
- `crates/canvas-app/src/hints_ui.rs` — ручной popup-rect с клампом к окну (паттерн, который PRD-0009 §2 фиксирует как класс дефекта CR-015); строки-подсказки и hover — вручную. Замена: `kit::dropdown_menu` (якорь+flip), `kit::list_rows`+`ScrollState`, `widget::WidgetState` на строках.
- `crates/canvas-app/src/flowmap_ui.rs` — ручные карточки-оверлеи и строки «исток → приёмник» с кликом. Замена: `kit::card`, `kit::list_rows`, `ButtonVariant::Ghost/Selected` для строк.
- `crates/canvas-app/src/calc_panel_ui.rs` — ручные панель/секции/строки формул. Замена: `kit::panel_rect`+`panel_content`, `kit::list_rows`, Painter.
- `app.rs:5848` `kit_gallery_overlay` — витрина знает только 8 компонентов v1; компоненты v2 (FR-058) в витрине не представлены.
- Все три модуля — «чистые модели» (прокомментировано в шапках): логика без I/O, рендер/ввод — в app.rs; перенос начинается с моделей, draw-функции app.rs переходят на Painter.

## Требуемые изменения
| Модуль | Было (hand-rolled) | Стало (кит) |
|---|---|---|
| `hints_ui.rs` | popup-rect + кламп к окну; ручные строки/hover | `kit::dropdown_menu` (якорь+flip), `kit::list_rows`+`ScrollState`, `WidgetState` строк, Painter |
| `flowmap_ui.rs` | ручные карточки, строки, клик-зоны | `kit::card`, `kit::list_rows`, `kit::button_style` (Selected-строки), Painter |
| `calc_panel_ui.rs` | ручные панель/секции/строки | `kit::panel_rect`+`panel_content`, `kit::list_rows`, Painter |
| `app.rs` (vitрина) | витрина v1 | секции TextField/Switch/Card/список/Icon (состояния × RU/EN × темы) |
| `app.rs` | draw/hit/on_key-ветки перечисленных поверхностей | переход на Painter/WidgetState; **только** функции этих поверхностей + секция витрины |

Правило переноса (паттерн U5): числа дословно; замены только там, где устраняется эвристика (ручная ширина → измеренная, кламп → flip/скролл); hit-rect'ы регистрируются в реестре как сегодня (G1/G2 не трогаются).

## Права на файлы (для параллельных агентов)
- **Вправе править:** `crates/canvas-app/src/hints_ui.rs`, `flowmap_ui.rs`, `calc_panel_ui.rs`, `kit_ui.rs` (только витринные layout-хелперы, если потребуются), `app.rs` — **только** функции `hints_overlay`, flowmap-функции, calc_panel-функции, `kit_gallery_overlay` (5848+) и их ветки в `on_left_button`/`on_key`/wheel; точный перечень функций зафиксировать в worklog при старте.
- **Запрещено трогать:** `crates/canvas-ui/**` (только использование), `crates/canvas-render/**`, `settings_ui.rs`/`template_ui.rs`/`docs_ui.rs`/`whatif_ui.rs`/`scheme_gallery_ui.rs`/`onboarding_ui.rs`, `autolink_ui.rs`/`palette.rs`/`explain_ui.rs` (владелец FR-060), `docs/ui-kit.md` (только строка состава витрины, если нужно — координировать с FR-058 §7.2; по умолчанию НЕ править).

## Зависимости
| Отношение | С кем |
|---|---|
| Кодировать | после слияния FR-057 и FR-058 (импортирует их API) |
| Сливать | после FR-056 (G5-аудит опирается на рендер-клип); **до** FR-060 (общий `app.rs`) |
| Последовательно | FR-060 — строго после слияния этого FR |

## Точки входа
- `docs/ui-kit.md` — состав витрины (если менялся).
- `docs/prd/prd-0009-ui-layering-uikit.md` — §16 (история), G5-расширение.
- `worklog.md` — запись реализации + перечень затронутых функций app.rs.

## Проверка
- G4-линт-матрица (1280×800, 1024×640, 800×560 × RU/EN) — 0 налезаний интерактивных rect'ов одного слоя, 0 переполнений, 0 выходов за вьюпорт.
- G5-аудит: в перенесённых модулях 0 `take(`/break-клампов/`truncate_chars` (grep).
- 0 визуального скачка: геометрия/цвета/тексты дословно (кроме замен эвристик на измеренные ширины); headless-тесты pick/hit/клавиатуры этих поверхностей зелёные.
- Витрина: новые секции проходят G4-линт (контракт FR-055 «кит покрыт линтом»).
- Все гейты (fmt/clippy -D warnings/test/wasm/mcp-wasm) зелёные.

## История изменений
- `2026-09-23` — агент: создан документ (постановка волны 2 кита, FR-059), статус «выявлено»; состав волны 1 и права на файлы зафиксированы.
- `2026-09-23` — агент: статус `✅ реализовано`. Миграция волны 1 (паттерн U5, числа дословно — 0 визуального скачка):
  - `hints_ui.rs` — геометрия popup через `kit::dropdown_menu` («якорь + flip»: якорь-строка каретки высотой `HINT_CARET_LINE_H` 16, прежний flip «−20» = 16 + `DROPDOWN_GAP` 4 дословно; зажим ширины — `constrain`), строки — `kit::list_rows` + `ScrollState` (окно без прокрутки, лимит `HINT_LIMIT` сохранён; подсветка строки `[px+4, y, pw−8, 24]` дословно); `token_before_caret` — без `break` (`take_while`-скан, класс символов прежний — G5); тесты обновлены (+2: якорь/flip/кламп, стопка строк).
  - `flowmap_ui.rs` — панель через `kit::stack` (End/Start в слоте с полями — прежние формулы x/y дословно), строки — `kit::list_rows` + `ScrollState` (состояние — `App.flow_map_scroll`, сброс на открытие; частичные строки клипятся пересечением с окном списка); кап `VISIBLE_CAP` со строкой «… ещё N» и `break`-клампом подгонки УДАЛЕНЫ — окно списка до 12 строк (`LIST_MAX_ROWS`, прежняя геометрия панели), все строки доступны скроллом, бегунок `kit::scroll_bar` (цвет — слот рамки); hit-тесты возвращают МОДЕЛЬНЫЕ индексы (скролл-независимые); `kit::card` не применён — фикс-пад 12 несовместим с прежней геометрией (решение зафиксировано в шапке модуля; правило U5 сильнее перечня «замена»); +1 тест прокрутки.
  - `calc_panel_ui.rs` — панель через `kit::stack` (Start/End, низ слота над `PANEL_BOTTOM_GAP`), строки обеих колонок — `kit::list_rows` + `ScrollState` (`App.stage_calc_{vars,formulas}_scroll`, сброс на открытии stage; resize-паттерн docs_ui — синхронизация в раскладке, идемпотентно); срез «… ещё N» (`vars_cut`/`formulas_cut`) УДАЛЁН — кап высоты остался ограничением РАЗМЕРА панели, переполнение прокручивается (бегунок `kit::scroll_bar`); `var_rows`/`formula_rows` — `Vec<(модельный индекс, rect)>`, `StageCalcFocus` без изменений (инвариант 3/6/7);
  - `app.rs` (только функции перечисленных поверхностей + секция витрины): `hints_overlay`, `flow_map_{rows,layout,row_text,overlay}`, `click_flow_map`, `toggle_flow_map`, `flow_map_scroll_by`, `stage_frame_ctx`, `stage_frame` (секция 8b), `stage_calc_wheel_scroll`, `kit_gallery_overlay`, `click_kit_gallery`, ветки wheel (flowmap/витрина/панель «Как считается»), поля App (4 ScrollState + сбросы при открытии поверхностей); отрисовка трёх поверхностей — через `Painter` (FR-057) с конвертацией `paint_items_to_band` (сырые лог. px — конвенция полос) / `paint_items_to_stage` (world-конвенция модального прохода stage); состояния строк — `WidgetState` (Selected/Hovered); ширины текстов панели «Как считается» — ИЗМЕРЕННЫЕ (`TextMeasurer`/`ellipsis` — замена эвристики «6.3·символ», правило «ручная ширина → измеренная»).
  - Витрина `kit_gallery` — секции v2: TextField ×3 (Normal/Focused/Disabled, каретка по `caret_x`, placeholder), Switch ×4 (Off/On × Normal/Hovered/Disabled, `kit::switch`), Card (`kit::card`, хедер+тело), список+скролл (8 строк в окне 3, выделенная строка, демо-сдвиг — бегунок в треке), Icon-глифы ×4 (Search/ArrowLeft/ArrowRight/Refresh через `icon_glyph`); контент витрины ПРОКРУЧИВАЕТСЯ (`kit::ScrollState` в App, колесо над контентом, шапка фиксирована, при offset 0 — прежняя раскладка дословно; бегунок `kit::scroll_bar`); i18n RU/EN +13 ключей; `cursor_state`/`dropdown_item_state` — `#[deprecated]` (потребители мигрированы на `WidgetState`).
  - Реестр (`ui_registry.rs` — минимально, сигнатуры раскладок): hit-rect'ы FLOW_MAP из той же раскладки (`app.flow_map_layout()`, UiRect) — G1/G2 не тронуты.
  - Гейты: fmt; clippy -D warnings (canvas-ui, canvas-app all-targets); canvas-ui 120/120, canvas-app lib 322/322 (+4) + 9 интеграционных бинарей зелёные, canvas-core 375, canvas-render 319, canvas-scene 93; G4-линт-матрица (в т.ч. `lint_kit_gallery_open`) зелёная; G5-аудит трёх модулей — 0 `take(`/`break`/`truncate_chars`; wasm-gate --check зелёный (G7: 0 новых зависимостей).
- `2026-09-23` — примечание: полный прогон wasm/mcp-wasm ступеней 2–3 — за CI песочницы (диск; прецедент FR-057).

## Источники истины
- `crates/canvas-app/src/hints_ui.rs` (414 строк), `flowmap_ui.rs` (355), `calc_panel_ui.rs` (614) — шапки модулей (паттерн «чистая модель»).
- `crates/canvas-app/src/app.rs:5848` — `kit_gallery_overlay` (витрина).
- `docs/change-requests/fr-054-ui-layering-u5-migration.md` — паттерн миграции и G4/G5-гигиена.
- `docs/change-requests/fr-055-ui-layering-u4-kit.md` — контракт витрины.
