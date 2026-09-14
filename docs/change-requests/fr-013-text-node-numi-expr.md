# FR-013: Text-нода с `canvasdesk.expr` — Numi-калькулятор

- **Статус:** выявлено
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-15): «ноды с numi-style вычислениями … архитектор рисует ландшафт сервиса, задаёт параметры человеческим языком в стиле Numi, а CanvasDesk сразу показывает узкое место, риск очереди, SLA и эффект сценария что будет, если…». Уточнение владельца (2026-09-15): вычислительная нода — расширение существующей text-ноды; язык v1 — Numi-base; поток значений вынесен в FR-014; тестируемость — обязательный инвариант каждого этапа.
- **Связанные задачи:** FR-014 (поток значений по рёбрам + DAG), FR-015 (доменные единицы и queueing-theory функции), FR-016 (bottleneck + queue risk индикаторы), FR-017 (what-if режим), FR-005 (`node_edit` MCP), FR-006 (undo), FR-009 (пункты меню ноды), SPEC.md §5.1 (расширения `.canvas`), §6.2 (LOD), §7.6 (образец расширения виджета)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-15
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Архитектор/техлид/системный аналитик рисует на канвасе ландшафт сервиса: ноды =
компоненты (gateway, queue, replica, DB), рёбра = поток запросов. В каждой ноде он
пишет параметры «человеческим языком» в стиле Numi — например:

```
rps = 1000
latency = 50 ms
replicas = 3
cpu_per_request = 0.5 ms × 1000 rps / 3 replicas = 166.7 ms-equivalent
```

CanvasDesk парсит формулу, считает результат и показывает его отдельной строкой
ниже текста ноды. В v1 (FR-013) вычисления **локальные** — формула в ноде видит
только свои переменные; поток значений между нодами приходит в FR-014. Цель FR-013 —
дать базовый «движок формул» и точку сериализации `canvasdesk.expr` в `.canvas`,
которую FR-014..017 будут расширять.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `text`-нода | Новое расширение `canvasdesk.expr: { expr: string, format?: string }`; текст ноды остаётся пользовательским описанием | `docs/interface-objects/node.md` §2, §5, §7 |
| Рендер | Под текстом ноды — строка с результатом (LOD ≥ 0.6); для `< 0.6` — индикатор `=…` в углу | `docs/SPEC.md` §6.2, `crates/canvas-render/src/cards.rs` |
| Ввод / редактор | При двойном клике по text-ноде с `canvasdesk.expr` — выбор режима: текст / формула (или смешанный редактор) | `docs/SPEC.md` §8, `crates/canvas-app/src/main.rs:1058` (`begin_editing`) |
| MCP | `node_edit` (FR-005) получает поле `expr` (string/null) | `crates/canvas-mcp/src/lib.rs`, `crates/canvas-app/src/main.rs:2872` |
| Undo | Один undo-шаг на commit формулы (паттерн FR-006) | `crates/canvas-app/src/main.rs:311-330` |
| Формат `.canvas` | Новое поле `canvasdesk.expr` внутри text-ноды; для Obsidian — неизвестное поле, round-trip без потерь | `docs/SPEC.md` §5.1, `crates/canvas-core/tests/json_canvas_io.rs` |

## Анализ (Root Cause)

В текущей реализации вычислительной ноды нет. Точки встраивания:

- **`Node.canvasdesk`** (`crates/canvas-core/src/model.rs:150`) — `Option<CanvasdeskExt>`,
  где `CanvasdeskExt` (`model.rs:113-119`) — структура только для виджет-ноды
  (`widget_id`, `props`). Использовать её же для calc-ноды — ломает семантику.
  Решение: хранить `expr` в `Node.extra` (round-trip через `extra: Map<String, Value>`,
  `model.rs:152-153`) под ключом `canvasdesk: { expr: "..." }` — Obsidian и иные
  сторонние редакторы сохранят неизвестное поле без потерь. В runtime — accessor
  `Node::expr() -> Option<&str>`, читающий `extra["canvasdesk"]["expr"]`. Образец
  работы с `extra` — `json_canvas_io.rs` (тесты round-trip).
- **Парсера/eval формул нет.** Новый модуль `crates/canvas-core/src/expr.rs`
  (или `expr/mod.rs` с подмодулями) — Numi-base грамматика:
  - литералы: числа (`5`, `3.14`, `1k`, `2M`), единицы (`ms`, `sec`, `min`,
    `MB`, `GB`, `req/s`, `rps`, `$`, `%`),
  - операторы: `+ - * /`,
  - переменные: `name = expr`,
  - функции v1: `sum, avg, max, min, percentile(p, …)` (доменные функции
    `mm1, mmc, littles_law, utilization` — FR-015),
  - результат: `Value { num: f64, unit: Unit }` (Unit — композитный
    `{base: Vec<Dimension>, exp: i8}` для `ms·req/s` и т.п.).
- **Рендер результата.** `crates/canvas-render/src/cards.rs` — для `NodeKind::Text`
  рендерит текст; нового блока `result` нет. Добавить инстанс «строка результата»
  ниже текста (или в шапке) — по образцу подсветки `brokenLink` (`cards.rs`).
- **Редактирование.** `begin_editing` (`main.rs:1058`) открывает инлайн-редактор
  (glyphon + cosmic-text) для текста ноды. Для calc-ноды — отдельный input для
  формулы (или смешанный редактор с поддержкой блока `= ...`).
- **MCP.** `node_edit` (FR-005, `mcp_dispatch` `main.rs:2872+`) — добавить ветку
  `expr` (string/null), как уже сделано для `label`/`color`.
- **Undo.** `push_undo`/`undo` (`main.rs:311-330`) — снимок `Canvas` целиком;
  commit формулы = один шаг, как у `node_set_color`. Live-пересчёт downstream
  (FR-014) — НЕ undo-able, только исходная правка.

## Требуемые изменения (Changes)

1. **`canvas-core/src/model.rs`** — accessor для `expr` без новой struct:
   - `impl Node { pub fn expr(&self) -> Option<&str> }` — чтение из
     `self.extra.get("canvasdesk")?.get("expr")?.as_str()`.
   - `pub fn set_expr(&mut self, expr: Option<String>)` — мутация `extra` с
     удалением ключа при `None` (round-trip чистый: пустого поля нет).
   - Runtime-only поле результата — НЕ в `.canvas`, в `SceneState`:
     `expr_results: HashMap<String /*node_id*/, Value>` (`main.rs:193`, SceneState).

2. **`canvas-core/src/expr.rs`** — новый модуль (чистый Rust, без I/O):
   - `pub struct Value { num: OrderedFloat<f64>, unit: Unit }`
   - `pub enum Unit { Scalar, Composite(Vec<(Dimension, i8)>) }` (`Dimension`:
     `Time, Rate, Count, Bytes, Money, Percent, Custom(String)`).
   - `pub fn parse(input: &str) -> Result<Expr, ParseError>`
   - `pub fn eval(expr: &Expr, env: &Env) -> Result<Value, EvalError>` (`Env`:
     карта переменных; для FR-013 — только локальные, для FR-014 — `$in`/`$1`/`$2`).
   - Парсер: `pest` (грамматика в `expr.pest`) или `chumsky` — решение на этапе
     реализации; критерий — <200 строк грамматики, нулевые внешние side-эффекты.

3. **`canvas-app/src/main.rs`** — `begin_editing` (`1058`) + `mcp_dispatch`
   (`2872+`):
   - При commit формулы → `expr::parse` + `expr::eval` → сохранить `Value` в
     `SceneState.expr_results[node_id]`; `mark_dirty()` (`277`) для автосейва
     `canvasdesk.expr`.
   - Один undo-шаг: `push_undo` перед изменением, как у `node_set_color`
     (паттерн FR-006).
   - MCP `node_edit`: поле `expr` (STR/null); null — сброс, валидация: при ошибке
     парсинга — `isError: true` с диагностикой, нода не меняется.

4. **`canvas-render/src/cards.rs`** — для `NodeKind::Text` с непустым `expr`:
   - Новый инстанс «строка результата» под текстом (или в футере ноды) —
     формат `{num} {unit}` (`1000 ms·req/s`).
   - LOD `≥ 0.6` — полный текст; `< 0.6` — индикатор `=` в углу (образец:
     иконка `brokenLink`).
   - Ошибка парсинга — строка с красным акцентом + тултип.

5. **`canvas-mcp/src/lib.rs`** — расширить `ToolSpec node_edit` (FR-005) полем
   `expr` (STR/null). Описание: «Numi-style формула; результат рендерится под
   текстом ноды. null — сброс calc-режима».

6. **`canvas-core/src/expr/units.rs`** (опционально как подмодуль) — таблица
   единиц и conversions: `1 sec = 1000 ms`, `1 GB = 1024 MB`, `1 min = 60 sec`.
   Без этого — единицы «нескладываемые», но v1 можно с минимальным набором.

## Архитектура тестируемости (инварианты FR-013)

Чтобы каждое последующее FR (014–017) можно было тестировать независимо,
FR-013 фиксирует **4 инварианта**:

1. **Чистый парсер.** `expr::parse` — `(input: &str) -> Result<Expr, ParseError>`,
   без I/O, без глобального состояния. Тестируется unit-тестами на литералы,
   операторы, единицы, переменные, функции.
2. **Чистый eval.** `expr::eval(expr, env)` — `(Expr, &Env) -> Result<Value, EvalError>`,
   без side-эффектов. Тестируется свойствами: `eval(parse("5 ms + 10 ms")) == 15 ms`,
   `eval(parse("1k rps")) == 1000 rps`.
3. **Сериализация.** `Node::expr()` / `Node::set_expr()` — round-trip через
   `extra`. Тест `json_canvas_io.rs` дополняется: создание text-ноды с
   `canvasdesk.expr` → сохранить → прочитать → поле сохранено.
4. **Runtime-результат не в `.canvas`.** `expr_results` в `SceneState` —
   вычисляется из `expr` при загрузке/правке, не сериализуется (источник
   истины — формула, не результат). Это позволяет FR-014 (поток значений)
   и FR-017 (what-if) не плодить дублирующие поля в `.canvas`.

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §2 (типы — note про text+expr), §5 (состояние
  «calc»), §7 (точки входа для фичи), §8 (чек-лист расширения).
- `docs/SPEC.md` §5.1 (расширение `canvasdesk.expr` для text-ноды), §6.2 (LOD
  для строки результата), §8 (ввод — двойной клик в режиме calc).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-013 (см. ниже «Проверка»).
- `docs/change-requests/fr-014-edge-value-flow.md` — продолжение (поток значений
  по рёбрам использует `expr` из этого FR).
- `docs/change-requests/fr-005-mcp-node-edit.md` — расширение инструмента `node_edit`.
- `docs/change-requests/fr-006-undo-stack.md` — паттерн undo-шага.

## Проверка (Verification)

### Юнит-тесты (обязательные, зелёные до перевода FR-013 в `выполнено`)

- `expr::parse("5") == Ok(Expr::Num(5.0, Unit::Scalar))`
- `expr::parse("5 ms × 200 req/s") == Ok(...)` (композитный unit `ms·req/s`).
- `expr::eval(parse("1 sec + 500 ms"), Env::empty()) == Ok(Value { 1.5 sec })`.
- `expr::eval(parse("avg(10 ms, 20 ms, 30 ms)"), …) == Ok(20 ms)`.
- `expr::parse("= invalid @#$")` → `Err(ParseError { pos, msg })`.
- `expr::parse("5 ms + 3 rps")` → `Err(EvalError::UnitMismatch)` (единицы не
  складываются).
- `Node::set_expr(Some("5 ms"))` → `Node::expr() == Some("5 ms")`; сериализация
  в `.canvas` и чтение обратно — поле сохранено; `Node::set_expr(None)` →
  поле отсутствует в JSON (round-trip чистый).

### Интеграционные тесты

- `crates/canvas-core/tests/json_canvas_io.rs` — fixture `text_with_expr.canvas`:
  text-нода с `canvasdesk.expr = "5 ms × 200 req/s"`; round-trip через
  `serde_json::to_string` → `from_str` — поле сохранено, Obsidian-формат валиден
  (extra сохраняется, `model.rs:152-153`).
- `crates/canvas-app/tests/integration_expr.rs` — создание text-ноды с `expr`
  через MCP `node_edit { id, expr: "5 ms" }` → `expr_results` обновлён,
  `mark_dirty` сработал, undo восстанавливает пустой `expr`.

### Ручная приёмка

- ПКМ по text-ноде (меню FR-009) → пункт «Режим вычислений» → появляется инпут
  формулы (или mixed-редактор с поддержкой блока `= ...`).
- Пишем `5 ms × 200 req/s` → под текстом ноды — строка `1000 ms·req/s`.
- Пишем `avg(10 ms, 20 ms, 30 ms)` → `20 ms`.
- Пишем синтаксически некорректное (`5 ms +`) → красная строка с тултипом
  ошибки, `expr_results` не меняется.
- Закрытие/открытие канваса — формула и результат сохранены (через
  `.canvas` + пересчёт при загрузке).
- MCP: вызов `node_edit { id, expr: "1k rps" }` из внешнего AI-клиента →
  результат `1000 rps` виден на канвасе.
- Undo (`Ctrl+Z`) после правки формулы — формула возвращается к предыдущему
  значению; redo — восстанавливает.

## Открытые вопросы дизайна

- **Смешанный редактор vs отдельный input.** Первый — единый редактор, где
  блок после `=` трактуется как формула; второй — отдельное поле под
  текстом. Решение на этапе прототипа; критерий — не ломает существующий
  редактор текста (`begin_editing`, `main.rs:1058`).
- **Парсер `pest` vs `chumsky`.** `pest` — декларативная грамматика в `.pest`,
  проще тестировать; `chumsky` — больше контроля над ошибками. Решение на
  этапе реализации; критерий — coverage грамматики unit-тестами ≥ 90%.
- **Композитные единицы (`ms·req/s`).** В Numi — конкатенация размерностей.
  v1 — поддержать, но выводить в упрощённом виде (`1000 ms·req/s`, не
  `1000 ms·req·s⁻¹`).
- **Кириллические единицы.** Numi — латиница; в CanvasDesk — сохранить
  латиницу (`ms`, `rps`, `req/s`), но позволить синонимы (`мс`, `запр/с`)
  — open question, отложить до v2.

## История изменений (Changelog)

- `2026-09-15` — агент: документ создан по запросу пользователя с уточнениями
  (расширение text-ноды, Numi-base язык, поток значений вынесен в FR-014).
  Зафиксированы 4 инварианта тестируемости (чистый парсер, чистый eval,
  round-trip сериализация, runtime-результат не в `.canvas`). Статус `выявлено`.

## Источники истины (References)

- `crates/canvas-core/src/model.rs:113-119` — `CanvasdeskExt` (образец расширения).
- `crates/canvas-core/src/model.rs:121-187` — `Node`, `Node::text()`, `kind()`,
  `extra: Map<String, Value>` (round-trip).
- `crates/canvas-core/src/model.rs:152-153` — `extra` flatten — точка хранения
  `canvasdesk.expr`.
- `crates/canvas-app/src/main.rs:193-228` — `SceneState` (добавить
  `expr_results: HashMap<String, Value>`).
- `crates/canvas-app/src/main.rs:277-282` — `mark_dirty` / `autosave_if_due`
  (точка триггера пересчёта).
- `crates/canvas-app/src/main.rs:311-330` — `push_undo` / `undo` (паттерн).
- `crates/canvas-app/src/main.rs:1058` — `begin_editing` (точка UI-ввода).
- `crates/canvas-app/src/main.rs:2872+` — `mcp_dispatch` (расширение
  `node_edit`).
- `crates/canvas-render/src/cards.rs` — рендер text-ноды (добавить блок result).
- `crates/canvas-mcp/src/lib.rs` — `TOOLS` (расширить `node_edit`).
- `crates/canvas-core/tests/json_canvas_io.rs` — round-trip (добавить
  fixture `text_with_expr.canvas`).
- `docs/SPEC.md` §5.1 (расширения `.canvas`), §6.2 (LOD), §7.6 (образец
  расширения виджет-ноды — `canvasdesk: { widgetId, props }`), §8 (ввод).
- `docs/change-requests/cr-template.md` — шаблон.
- `docs/change-requests/fr-005-mcp-node-edit.md` — паттерн MCP-расширения.
- `docs/change-requests/fr-006-undo-stack.md` — undo-паттерн.
- `docs/change-requests/fr-011-mindmap-object.md` — образец FR с расширением
  `.canvas` (`canvasdesk.collapsed`).
