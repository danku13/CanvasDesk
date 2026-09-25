# FR-061. Аудит вёрстки шаблонных нод против canvas-ui (kit-выравнивание)

- **Статус:** исполнено (аудит + выравнивание, 2026-09-26).
- **Дата:** 2026-09-26
- **Задача-источник:** запрос владельца (сессия web-77d836dd, задача 4 из 4):
  «ещё раз перепроверь как сейчас строится вёрстка шаблонных нод, т.к. сейчас
  выглядит так что они не на новом canvas-ui. если это так, то пересобери их на
  новом kit».
- **Автор:** агент-аудитор шаблонных нод (параллельная волна, worklog Task ID: 3).
- **Связанные:** FR-018 (шаблонные ноды, полоса категории + квад-иконка),
  FR-061 (табличное тело ноды: D-2/D-3/D-4/D-5/D-6/D-15, этапы A–F),
  FR-069 (Σ-строки, пилюли бейджей, авто-строки), FR-068 W3 (каталог миграции
  `fr-068-w3-consumer-migration.md` §9.4 «не мигрировать»), FR-046
  (design-токены `canvas_core::tokens`), FR-057/FR-055 (Painter/kit v1),
  `docs/ui-kit.md` (инвентарь кита).

---

## 1. Постановка и короткий ответ

**Вопрос:** строятся ли шаблонные ноды мимо нового canvas-ui, и если да —
пересобрать их на kit.

**Ответ (после аудита всех элементов карточки):** шаблонная нода — это
text-нода со снимком `canvasdesk.template` (FR-018). Её карточка состоит из
двух слоёв:

1. **world-space каркас** (шелл-квад, полоса категории, квад-иконка, текст
   тела/футера, порты) — рисуется GPU-instanced пайплайном `canvas-render`
   в мировых координатах через камеру. Этот слой **НЕ переносится на
   canvas-ui осознанно** — владельческое решение зафиксировано в каталоге
   миграции FR-068 W3 §9.4: «канвас-домен (canvas-render::cards), рендерится
   квадами по нодной геометрии; компонентная модель — для экранного UI, не
   для world-space контента». Кит — screen-space (полосы `UiLayer`,
   логические пиксели окна), карточки нод — world-space (zoom/pan,
   SDF-шейдер `cards.wgsl`, instanced draw); «пересборка на kit» шелла была
   бы сменой системы координат, а не рефакторингом.
2. **геометрия табличного тела** (направляющие колонок, замер ячеек,
   штрихи лидера, пад бейджа, зебра-прогон, ellipsis) — вот она **уже на
   canvas-ui** (этапы A/E FR-061), и аудит подтвердил это по каждому
   элементу (§2).

Отдельного «шаблонного» код-пути вёрстки НЕТ: базовая карточка шаблонной
ноды байт-в-байт равна карточке обычной text-ноды — это пинит регрессионный
оракул `card_instance_template_node_equals_plain_text_node` (cards.rs);
отличия (полоса + иконка) — декоративные оверлеи поверх `card_instance`.

Единственная найденная дублирующая логика — **правило зебра-прогонов**
(D-5): inline-цикл в `text.rs`, тогда как кит экспортирует остальные
табличные решения. Перенесено в кит 1:1 (§3).

## 2. Карта реализации: элемент → код → источник геометрии/стиля

Обозначения: **kit** — геометрия/решение из `canvas-ui` (kit-функции,
`RowGuides`, `TextMeasurer`); **токены** — `canvas_core::tokens`
(design-токены FR-046, общий источник кита и рендера); **render** —
canvas-render-local (world-space/GPU-домен или тема рендера).

| Элемент карточки шаблонной ноды | Реализация (crate/файл/функция) | Источник |-kit/токены/render |
|---|---|---|---|
| Шелл: заливка/рамка/радиус/тень/выделение | `canvas-render/cards.rs::card_instance` → instanced SDF `shaders/cards.wgsl`; выделение `tokens::ACCENT`, битая ссылка `tokens::BROKEN_BORDER`, радиус `tokens::CARD_CORNER_RADIUS`, шапка `tokens::CARD_HEADER_HEIGHT` | render + токены | world-space; отдельного template-пути нет (оракул равенства с text-нодой) |
| Полоса категории (band 6 px) | `cards.rs::template_band_instance` (константа `TEMPLATE_BAND_H`), цвет — hex-снимок `canvasdesk.template.color` (`parse_hex`); рисует `renderer.rs` (~1519) | render | world-space-оверлей; kit-эквивалента нет (нет и дублирования) |
| Квад-иконка роли (16 px) | `cards.rs::template_icon_rect` + `template_icon_quads` (`TEMPLATE_ICON_SIZE/MARGIN*`), tint `theme.icon`; рисует `renderer.rs` (~1527) | render | плоские квады (решение владельца FR-018), не kit-`Icon` (шрифтовые глифы экрана) |
| Заголовок (шапка 34 px) | `text.rs` (шейп заголовка, клип `title_clip_width` с резервом `ICON_WIDTH` под иконку шаблона — CR-010, тест `template_title_clip_clears_quad_icon`); кегль/линия — `tokens::TYPE_TITLE*` | render + токены | world-space текст (glyphon) |
| Тело: проза/GFM | `text.rs::shape_body` + `markdown.rs`/`gfm.rs` (квады GFM — `BodyQuadKind`) | render | world-space текст + квады |
| Описание (D-8) | `text.rs::clamp_desc_text` — сознательно космический Buffer (кламп обязан совпадать с версткой стека; `TextMeasurer` без космических буферов — по дизайну D-8) | render | задокументированное исключение, не дубль |
| Таблица: сборка строк (D-2) | `row_grid.rs::build_rows` (род строки — ТОЛЬКО `canvas_core::expr::line_kind`, грамматика не дублируется) | render (домен) | данные ноды — world-домен |
| Таблица: замер ячеек (D-3/этап A) | `canvas_ui::row_guides::measure_row_cells` + `TextMeasurer::width_of_weighted` (вес — паритет mono-шейпинга, тест `table_cell_measure_matches_mono_shaping`) | **kit** |TextMeasurer — кит |
| Таблица: направляющие колонок (проход A/B) | `canvas_ui::row_guides::RowGuides::measure` + `with_right_edge`; зазор `tokens::TABLE_NODE_GUIDE_GAP`, лидер-минимум/пад `tokens::TABLE_LEADER_MIN/PAD` (алиасы `row_grid.rs`) | **kit + токены** | RowGuides — кит (Ф-14) |
| Таблица: лестница деградации бейджей (§3.4) | `row_grid.rs::pass_a` (`BadgeMode` Text→Icon→None) + план ellipsis (алиасы Q8 → `TextMeasurer::ellipsis_weighted`); пад пилюли `canvas_ui::kit::ROW_BADGE_PAD_H` | **kit** (замер/ellipsis/пад) + render (лестница) | лестница — доменное решение тела ноды (у kit-Row её нет) |
| Лидер-пунктир (D-5) | `canvas_ui::kit::leader_dash_rects` (обе точки вызова text.rs; Y — `tokens::TABLE_LEADER_Y_FRAC`; минимум дорожки — исторические 6 px тела, параметр функции); оракул паритета — kit-тест `leader_dashes_match_node_arithmetic` | **kit + токены** | единая геометрия с kit-Row (D-15) |
| Зебра-фон (D-5) | **было:** inline-цикл `text.rs` (порог `tokens::TABLE_ZEBRA_RUN_MIN`) → **стало:** `canvas_ui::kit::zebra_run_flags` (перенос 1:1, §3); цвет — `theme.search_row_fill` (`renderer.rs::body_quad_fill`) | **kit** (решение) + render (цвет, F-8) | перенос этого аудита |
| Ячейки значение/юнит/бейдж (D-4) | `text.rs::shape_row_cell` + право-прижатие по направляющим в `prepare_titles` (право = `value_right()/unit_right()` кита) | **kit** (координаты) + render (шейп/текст) | glyphon-буферы — world-space |
| Пилюля бейджа (FR-069) | rect — `text.rs` (`BADGE_PILL_H` 14, право на край колонки, вертикальный центр строки); капсула/цвета — `renderer.rs::body_quad_instance`/`body_quad_fill` (альфа 0.12/0.55 от слота темы) | render | арифметика цвета в ките запрещена (F-8) — здесь она и должна быть в потребителе |
| Авто-строки приёмника (Р-4/FR-069) | префикс — `text.rs::spill_row_items` (наклонное моно Р-2); хром — янтарный фон/пунктиры (`BodyQuadKind::AutoRow*`, токен `SEVERITY_DARK`), штрихи — kit `leader_dash_rects`; высота строки `AUTO_ROW_LINE_HEIGHT` 18 (render-константа, зеркало в canvas-scene::measure) | render + **kit** (штрихи) | |
| Заголовок блока/превью/Σ-строка (D-7/D-9/FR-069) | вставка строк `text.rs` (по `row_grid::block_mode`/`block_header_text_lang`), линия Σ — `BodyQuadKind::SigmaRule` | render | Y-ряд не меняется (I-1) |
| Футер «ИТОГ» (FR-069) | метка — `row_grid::result_footer_label_lang` (RU/EN), шейп `text.rs` (~3249), вертикаль `result_footer_y`; итог справа по `RESULT_*` токенам | render + токены | world-space |
| Порты строк/футера, якоря параметров (FR-025/FR-050 Н2) | `text.rs::line_ports`/`param_ports` (`result_row_y`/`entry_body_block` — та же геометрия блоков, I-1); паритет — `anatomy.rs` | render | world-space точки на краях ноды |
| Ошибки строк («!» + тултип) | `row_grid::RowBadge::Error`, hit-зона `LINE_ERROR_HIT_PAD_PX`, цвет `theme.error` | render | |
| Диагностика направляющих (D-14/Q9) | `text.rs::guide_debug_quads`, цвет `tokens::TABLE_GUIDE_DEBUG_COLOR`, только DebugOverlay | render + токены | невидима в проде |

**Вывод по карте:** дублирования kit-эквивалентов в render ровно одно —
правило зебра-прогонов (устранено, §3). Остальные render-локальные константы
(`TEMPLATE_*`, `AUTO_ROW_LINE_HEIGHT`, `BODY_PADDING`, `BADGE_PILL_H`,
минимум лидера 6 px) — world-space-геометрия карточки без kit-аналогов:
совпадения значений с токенами (`18.0` == `TYPE_HUD_LINE` и т.п.) —
коинцидентные, подменять токенами другого семейства не корректно.

## 3. Что выровнено в этом изменении

| # | Изменение | Файлы | Доказательство байт-паритета |
|---|---|---|---|
| 1 | Зебра-прогон (D-5) перенесён в кит 1:1: `canvas_ui::row_guides::zebra_run_flags(block_pos, is_chrome_row, run_min)` (решение — кит; цвет — потребитель, F-8). Реэкспорт: `kit.rs` + корень крейта | `canvas-ui/src/row_guides.rs`, `kit.rs`, `lib.rs` | юнит-тесты кита: `zebra_flags_alternate_within_min_run`, `zebra_flags_break_on_block_gap`, `zebra_flags_skip_chrome_rows`, `zebra_flags_chrome_row_can_open_run`, `zebra_flags_empty_and_deterministic` |
| 2 | `text.rs::prepare_titles`: inline-цикл зебры заменён вызовом `canvas_ui::kit::zebra_run_flags` (порог — прежний токен `TABLE_ZEBRA_RUN_MIN`) | `canvas-render/src/text.rs` | оракул `text::tests::zebra_run_matches_inline_oracle` — дословная копия прежнего цикла против кит-функции на корпусе прогонов (сплошная таблица, проза-разрыв, Total/Preview/Sigma в начале/середине/конце, длины 3/4) |

Больше миграционных кандидатов аудит не нашёл: штрихи/замер/направляющие/
ellipsis/пад бейджа уже на ките (см. §2); цветовая арифметика пилюль
(0.12/0.55) — обязанность потребителя по контракту F-8; `clamp_desc_text` —
осознанный космический путь (D-8), не дубль `TextMeasurer`.

## 4. Архитектурная граница: почему шелл не «пересобирается на kit»

Кит `canvas-ui` — **screen-space** модель: полосы окна (`UiLayer`),
логические пиксели, `Painter`-элементы, палитра слотов `KitPalette`,
компоненты возвращают геометрию в слот-координатах экрана. Карточка ноды —
**world-space**: позиция/размер из `.canvas`-файла, камера (zoom/pan)
переводит мир в физические пиксели, скругление/тень — SDF в
`shaders/cards.wgsl`, весь кадр карточек — один instanced draw. Рисовать
карточку средствами кита означало бы: (а) завести в ките вторую систему
координат (world-band) или (б) конвертировать каждый квад через камеру,
потеряв instancing и SDF-хром. Это не рефакторинг, а смена архитектуры
рендера — вне задачи и против владельческого решения FR-068 W3
(каталог §9.4, «не мигрировать: канвас-домен»).

Правильная линия среза, подтверждённая аудитом, — **единая ГЕОМЕТРИЯ,
разные исполнения**: кит владеет чистыми решениями (направляющие `RowGuides`,
замер `TextMeasurer`/`measure_row_cells`, штрихи `leader_dash_rects`, пад
бейджа `ROW_BADGE_PAD_H`, зебра-прогон `zebra_run_flags`, ellipsis), а
экранные поверхности (kit-Row/Table v2) и world-space тело ноды
(`row_grid.rs` + `text.rs`) потребляют одни и те же функции, различаясь
только исполнением (Painter vs glyphon-буферы + GPU-квады) и источником
цвета (тема рендера/слоты палитры — у своего потребителя, F-8).

## 5. Что потребовала бы полная миграция (non-goal)

1. **Camera-aware kit** — слой `UiLayer::World` с world-слотами: киту
   понадобились бы камера (zoom/pan), мир-координаты в `UiRect`, LOD-логика
   рендера — дублирование `canvas-render::camera` внутри UI-крейта.
2. **World-band renderer** — Painter-бэкенд поверх instanced SDF-пайплайна
   (`cards.wgsl`/`icons.wgsl`), т.е. кит начал бы владеть GPU-исполнением,
   что нарушает его контракт «кит не рисует» (F-8/U3, G7 — ноль
   GPU-зависимостей).
3. Перенос текста тела с glyphon/cosmic-буферов кита на экранную модель —
   потеря кэша по нодам (T5) и покадрового шейпинга мировых кеглей.

Все три пункта — смена архитектуры без продуктовой выгоды (визуальный
результат тот же); зафиксированы как non-goal до прямого решения владельца.

## 6. Гейты

- `cargo test -p canvas-ui --lib` — 201 passed / 0 failed (+5 зебра-тестов).
- `cargo test -p canvas-render --lib` — 377 passed / 0 failed
  (+1 оракул `zebra_run_matches_inline_oracle`).
- `cargo fmt` по изменённым файлам; `cargo clippy --workspace --all-targets
  -- -D warnings` — зелёные (см. worklog Task ID: 3).
- Регистрация в `docs/change-requests/index-cr-fr.md` не требуется
  (индекс покрывает только `docs/change-requests/*.md`; план-документы
  `docs/plans/*` — прецеденты `fr-068-table-v2.md`,
  `fr-068-w3-consumer-migration.md` — в индекс не входят).
