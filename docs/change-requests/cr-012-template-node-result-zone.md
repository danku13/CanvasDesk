# CR-012: Зона результата внизу шаблонной ноды — обрезка тела и налезание футера на переносах

- **Статус:** выполнено
- **Тип:** CR (исправление дефекта вёрстки карточки ноды)
- **Приоритет:** важно
- **Владелец:** агент (по скриншоту и сообщению пользователя)
- **Источник:** сообщение пользователя (сессия 2026-09-16): «так и не починена
  зона для результата рассчёта внизу ноды для шаблонов» + скриншот (шаблонная
  нода GraphQL: последняя строка тела обрезана клипом, футер «0.01² ms / sec»
  визуально лежит поверх тела у самого низа карточки)
- **Связанные задачи:** FR-023 (вёрстка карточки шаблонной ноды), FR-013
  (Numi-формулы, построчные результаты), CR-010 (авто-высота с учётом переносов
  — предшественник; исправление оценки рядов), FR-018 (шаблонные ноды)
- **Создан:** 2026-09-16
- **Обновлён:** 2026-09-16 (создан, анализ, реализация; правка 2 — измерение
  реальным шейпингом вместо оценки)

---

## Описание (What)

У шаблонной ноды (напр. GraphQL) строки параметров моноширинным шрифтом
переносятся на больше визуальных рядов, чем планировала оценка высоты → высота
карточки занижена → последняя строка тела обрезана клипом, а футер результата
(«0.01² ms / sec») лежит поверх тела у самого низа карточки. Дефект живёт и у
новых нод (недооценка стартовой высоты), и у нод, получивших футер другими
путями (загрузка `.canvas`, MCP-правки текста/формулы), — резерв под футер
они не получали вовсе.

## Влияние (Impact)

| Объект | Что сломано | Где в документации |
|---|---|---|
| `node` (шаблонная) | Высота занижена на ряд(а): тело обрезано клипом, футер результата налезает на тело | `docs/interface-objects/node.md` §3, §11 (CR-010) |
| `node` (обычная, с итогом формулы) | Ноды из `.canvas`/MCP без резерва под футер — тот же дефект при переносах | `user-docs/calculations.md` |

## Анализ (Root Cause)

- `crates/canvas-app/src/main.rs` (`wrapped_body_rows`) — оценка рядов переносов
  считала КАЖДУЮ строку по средней ширине глифа пропорционального Noto Sans
  (`AVG_CHAR_W = 7.0` px). Строки-присваивания Numi-листа рендерятся
  моноширинным Noto Sans Mono (mono-флаг при `source_line`,
  `crates/canvas-render/src/text.rs:530-532`), чей аванс ≈8.6 px при размере
  14 px → строка «имя = значение» из 32–40 символов: оценка давала 1 ряд,
  рендер переносил на 2. Высота занижалась на `BODY_LINE_HEIGHT` за каждый
  недооценённый перенос.
- Рост высоты (резерв `RESULT_LINE_HEIGHT + 2` под футер) выполнялся только при
  инстанциации (`fit_template_node_height`, вызовы в `instantiate_template_at` и
  MCP `template_instantiate`). Ноды, получившие футер при загрузке `.canvas`
  (`SceneState::new`) или MCP-мутациях (`node_update_text`, `node_edit` с expr),
  резерв не получали — правило показа футера рендера
  (`text.rs:1264-1276`) выполнялся без сопутствующего роста высоты.

## Требуемые изменения (Changes)

1. **`crates/canvas-app/src/main.rs`** — `wrapped_body_rows`: строки Numi-листа
   (вердикт `expr::line_kind` — существующий API движка: `Assignment` /
   `Expression`) считать по моноширинной метрике `MONO_AVG_CHAR_W = 0.614 ·
   BODY_FONT_SIZE` (≈8.6 px @14); проза — по-прежнему `AVG_CHAR_W = 7.0`.
   Оценка по-прежнему только растёт высоту (завышение безопасно).
2. **Lazy-refit** — `ensure_result_reserve(node)` (growth-only, та же формула,
   что и `fit_template_node_height`: `HEADER_HEIGHT + BODY_TOP_GAP + ряды ·
   BODY_LINE_HEIGHT + BODY_PADDING + RESULT_LINE_HEIGHT + 2`):
   - `SceneState::apply_result_reserve` в конце `recompute_flow` (обе ветки,
     включая фолбэк при цикле value-рёбер) — для каждой ноды, которой рендер
     покажет футер по правилу `text.rs:1264-1276` (шаблонные — всегда; обычные —
     только без построчных Numi-результатов и с итогом/ошибкой формулы);
     spatial index обновляется только при реальном росте. Growth-only → нет
     осцилляций при частых пересчётах; покрывает загрузку `.canvas` и все
     MCP-мутации, идущие через `recompute_flow`.
   - ветка `node_edit` с текстом (без expr — `recompute_flow` там не вызывается):
     точечный `ensure_reserve_at(index)` после обновления текста.
3. **Тесты** (`main.rs`): `wrapped_body_rows_mono_assignment_uses_mono_metric`
   (32-символьное присваивание при ширине тела 240 → 2 ряда против 1 по старой
   метрике; проза той же длины — 1 ряд), `ensure_result_reserve_grows_only`
   (рост заниженной, идемпотентность, достаточная не трогается),
   усиленный `fit_template_height_covers_wrapped_lines` (mono-строка, где старая
   оценка занижала на ряд), `recompute_grows_result_reserve_for_footer_nodes`
   (SceneState-уровень: шаблонная нода выросла при загрузке и после
   MCP `node_update_text`; прозаическая нода не тронута).

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` — §3 (визуальная структура карточки:
  футер результата), §11 (таблица CR/FR).
- `user-docs/calculations.md` — строка итога под текстом ноды.

## Проверка (Verification)

- Шаблонная нода с длинным значением параметра (перенос mono-строки на 2 ряда):
  карточка целиком покрывает тело + резерв футера, последняя строка не обрезана,
  футер не налезает на тело (скриншотный сценарий пользователя).
- Загрузка `.canvas` со шаблонной нодой заниженной высоты → высота выросла
  после `SceneState::new` (первичный `recompute_flow`).
- MCP `node_update_text` с более длинным текстом шаблонной ноды → рост высоты
  тем же вызовом.
- Обычная прозаическая нода и нода с построчными Numi-результатами — высота
  не меняется (футера нет).
- Гейты: `cargo fmt --check`, `clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` — зелёные.

## История изменений (Changelog)

- `2026-09-16` — агент (правка 2): высота под резерв футера считается
  **реальным шейпингом**, а не оценкой среднего аванса — оценка принципиально
  хрупка (любое расхождение метрик шрифта возвращает дефект), измерение
  устраняет класс ошибок. `text.rs`: общий ленивый FontSystem из тех же 4
  встроенных Noto-шрифтов (`MEASURE_FS`, метрики идентичны рендеру) и
  `pub fn measure_body_height(text, body_width, formula_lines)` — зеркало
  `shape_body` при zoom=1.0; общая часть стека блоков вынесена в
  `with_body_stack` (колбэки на блок/линию) — измерение и рендер делят один
  код стека и не могут разъехаться. `main.rs`: двухуровневый growth-only
  refit — дешёвая оценка (`estimated_result_reserve_height`, прежняя
  `wrapped_body_rows`-метрика) ворота; при заниженности — точная высота
  `measured_result_reserve_height`, рост ровно до измеренного needed, **без
  фантомного ряда**. `formula_lines` берутся из тех же источников, что у
  рендера: `SceneState::ensure_reserve_at` — из `expr_line_results`,
  `fit_template_node_height` — из `expr::eval_lines`, `fit_note_size` —
  из живых построчных исходов текста сессии (у шаблонной ноды — все
  строки-формулы листа); `fit_note_size` рост по измеренной высоте тела
  вместо `content_size_px` буфера редактора (тот шейпил текст без
  GFM/mono-разбивки и занижал переносы Numi-строк). Инвариант: высота ноды
  с футером — ровно по измеренному стеку тела (включая зазоры абзацев и
  12 px линий `---`), повторные загрузки не растят дальше (нет осцилляций).
  Тесты: `measure_body_height_*` (text.rs: точный Numi-лист, mono-перенос
  на 2 ряда, sans/mono-метрики, линия `---`, анти-дрейф против `shape_body`),
  `recompute_result_reserve_matches_measured_height` (ровно измеренная
  высота, идемпотентность повторной загрузки), актуализированы
  `ensure_result_reserve_grows_only`, `fit_template_height_covers_wrapped_lines`,
  `recompute_grows_result_reserve_for_footer_nodes`; оценочные
  `wrapped_body_rows_*` сохранены (оценка остаётся воротами).
- `2026-09-16` — агент: реализовано, статус `выполнено`. `wrapped_body_rows` —
  mono-метрика через `expr::line_kind` (`MONO_AVG_CHAR_W = 0.614 ·
  BODY_FONT_SIZE`); `ensure_result_reserve`/`needed_result_reserve_height`
  (growth-only), `fit_template_node_height` — делегирует общей формуле;
  `SceneState::node_shows_result_footer`/`ensure_reserve_at`/
  `apply_result_reserve` — refit в конце `recompute_flow` (включая ветку цикла
  value-рёбер) и в MCP `node_edit` при тексте без expr. Тесты:
  `wrapped_body_rows_mono_assignment_uses_mono_metric`,
  `ensure_result_reserve_grows_only`, усиленный
  `fit_template_height_covers_wrapped_lines` (mono-строка),
  `recompute_grows_result_reserve_for_footer_nodes`. Гейты зелёные
  (fmt, clippy `--all-targets -D warnings`, `cargo test -p canvas-app
  -p canvas-render`).

## Источники истины (References)

- `crates/canvas-app/src/main.rs` — `wrapped_body_rows`, `MONO_AVG_CHAR_W`,
  `estimated_result_reserve_height` / `measured_result_reserve_height` /
  `formula_line_indices`, `ensure_result_reserve` (двухуровневый),
  `fit_template_node_height`, `fit_note_size` (рост по измерению),
  `SceneState::{node_shows_result_footer, ensure_reserve_at,
  apply_result_reserve}`, `recompute_flow`.
- `crates/canvas-render/src/text.rs` — `measure_body_height` (измерение,
  правка 2), `with_body_stack` (общий стек рендера и измерения),
  mono-флаг `source_line` (530-532), правило показа футера (1264-1276),
  позиционирование футера (1929-1954), клип тела под футер (1776-1787),
  `BODY_FONT_SIZE` (87).
- `crates/canvas-core/src/expr.rs` — `line_kind`/`NumiLineKind` (1693+).
- `docs/change-requests/cr-010-template-node-title-layout.md` — предшественник
  (пропорциональная оценка переносов из CR-010 недооценивала mono-строки).
