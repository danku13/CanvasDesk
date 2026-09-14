# FR-015: Доменные единицы и queueing-theory функции

- **Статус:** выявлено
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-15): концепция «архитектор рисует ландшафт сервиса … задаёт параметры … CanvasDesk сразу показывает узкое место, риск очереди, SLA». Уточнение владельца (2026-09-15): доменные примитивы v1 — системные метрики (rps, latency p95, replicas, CPU/RAM, error rate) + теория очередей (Little's law, M/M/1, M/M/c, utilization).
- **Связанные задачи:** FR-013 (`expr` парсер/eval — принимает новые функции и единицы), FR-014 (поток значений — формулы на ландшафте используют доменные функции), FR-016 (bottleneck/queue risk индикаторы используют результаты FR-015), FR-017 (what-if сценарии с queueing-функциями), SPEC.md §5.1, §6.2 (LOD)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-15
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

FR-013 даёт базовый Numi-калькулятор: арифметика, скалярные единицы, функции
`sum/avg/max/min/percentile`. Этого достаточно для счёта «5 ms × 200 rps», но
не для расчёта «какой длины будет очередь при λ=1000 rps, μ=200 rps, c=3
серверах». FR-015 добавляет **доменные единицы** и **функции теории очередей**,
которые делают ландшафт сервиса «считающим»: архитектор пишет формулу
`mm1(arrival_rate=1000 rps, service_rate=1200 rps)` и сразу видит
`utilization = 0.83`, `queue_length = 5.0`, `wait_time = 4.2 ms`.

**Границы v1 FR-015:**

- **Единицы:** `ms, sec, min, hr, GB, MB, KB, B, rps, rpm, req/s, %, $, k, M, G`,
  канонические составные (`req/s`, `MB/s`). Conversions: `1 sec = 1000 ms`,
  `1 MB = 1024 KB`, `1 min = 60 sec`, `1 hr = 60 min`, `1 G = 1e9`.
- **Функции v1:**
  - Базовые (из FR-013): `sum, avg, max, min, percentile(p, ...)`.
  - **Queueing theory:**
    - `utilization(arrival_rate, service_rate, servers=1)` → `ρ = λ / (c · μ)`.
    - `mm1(arrival_rate, service_rate)` → `{ utilization, queue_length, wait_time, response_time }`.
    - `mmc(arrival_rate, service_rate, servers)` → то же, для M/M/c (Erlang-C).
    - `littles_law(arrival_rate, response_time)` → `L = λ · W`.
    - `erlang_c(arrival_rate, service_rate, servers)` → доля задержанных
      запросов (для SLA-расчётов).
  - **Системные метрики (композитные):**
    - `throughput(rps, replicas)` → `rps × replicas`.
    - `latency_p(p, samples...)` → percentile (алиас `percentile`).
    - `error_budget(sla)` → `1 − SLA` (например, `error_budget(99.9%) = 0.1%`).
    - `mttr(failures, total_time)` → среднее время восстановления.
- **Расширяемость:** доменные функции регистрируются в `expr::Function`
  registry; v2 позволит добавлять через манифест (как `widget.json`).

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `expr.rs` | Новый подмодуль `expr/units.rs` (таблица conversions) + `expr/queueing.rs` (функции) | FR-013, `crates/canvas-core/src/expr.rs` |
| `expr::Value` | `Unit` расширена: `Time, Rate, Bytes, Money, Percent, Count` + композитный | FR-013 |
| Рендер | Составные единицы — упрощённый вид (`1000 rps`, не `1000 req·s⁻¹`); для queueing — несколько строк (util, queue, wait) | `crates/canvas-render/src/cards.rs` |
| MCP | Не требует новых инструментов — формулы пишутся через `node_edit { expr }` (FR-005/013) | — |
| Тесты | Расширение `crates/canvas-core/tests/expr_*.rs` — golden cases для каждой доменной функции | FR-013 |

## Анализ (Root Cause)

FR-013 вводит `expr::Value { num: f64, unit: Unit }`, где `Unit` — enum. FR-015
расширяет `Unit` до полной системы размерностей и добавляет доменные функции.
Точки расширения:

- **`expr::Value` и `Unit`.** FR-013 оставил `Unit` минимальным (`Scalar`,
  `Composite(Vec<(Dimension, i8)>)`). FR-015 заполняет `Dimension`:
  `Time, Rate, Bytes, Money, Percent, Count, Servers, Custom(String)`.
- **Conversions.** Таблица в `expr/units.rs`: `sec → ms × 1000`, `MB → KB × 1024`,
  `min → sec × 60`, `G → k × 1e6` (для численных суффиксов). Без conversions
  `1 sec + 500 ms` — `Err(UnitMismatch)`; с conversions — `1.5 sec`.
- **Функции registry.** FR-013 — `Function` enum с фиксированным набором
  (`Sum, Avg, Max, Min, Percentile`). FR-015 — `Function` реестр:
  `HashMap<&'static str, fn(&[Value]) -> Result<Value, EvalError>>`. Регистрация
  при инициализации `expr` модуля; точка расширения — в `expr/queueing.rs`.
- **Queueing math.** M/M/1, M/M/c, Erlang-C — стандартные формулы; важно
  зафиксировать единицы выхода:
  - `utilization` → `Percent` (0..1, может быть >1 при overload).
  - `queue_length` → `Count` (L_q, среднее число в очереди).
  - `wait_time` → `Time` (W_q, среднее время ожидания).
  - `response_time` → `Time` (W = W_q + 1/μ).
- **Композитные единицы** для `throughput(rps, replicas)` = `rps × replicas`
  → `rps` (размерность сохраняется, число умножается). Для `littles_law`:
  `arrival_rate (rps) × response_time (sec)` → `Count` (безразмерная L).
- **Тестируемость.** Каждая доменная функция — чистая `fn(&[Value]) -> Result<Value, EvalError>`.
  Golden cases — отдельный `tests/expr_queueing.rs` с табличными тестами
  (`λ=1000, μ=1200 → util=0.833, L_q=4.17, W_q=4.17 ms`).

## Требуемые изменения (Changes)

1. **`crates/canvas-core/src/expr/units.rs`** — таблица единиц и conversions:
   ```rust
   pub enum Dimension { Time, Rate, Bytes, Money, Percent, Count, Servers, Custom(String) }
   pub struct Unit { dims: Vec<(Dimension, i8)> }
   pub fn convert(value: Value, target: Unit) -> Result<Value, EvalError>
   ```
   - Conversions: `sec ↔ ms × 1000`, `min ↔ sec × 60`, `hr ↔ min × 60`,
     `MB ↔ KB × 1024`, `GB ↔ MB × 1024`, `G ↔ M × 1e3`, `M ↔ k × 1e3`.
   - `Time + Time` — OK (после conversions); `Time + Rate` →
     `Err(UnitMismatch)`.

2. **`crates/canvas-core/src/expr/queueing.rs`** — доменные функции:
   - `pub fn utilization(args: &[Value]) -> Result<Value, EvalError>`
     - 2 аргумента: `arrival_rate: Rate`, `service_rate: Rate` →
       `Percent = λ / μ` (для M/M/1).
     - 3 аргумента: + `servers: Count` → `λ / (c · μ)`.
   - `pub fn mm1(args: &[Value]) -> Result<Value, EvalError>`
     - 2 аргумента: `arrival_rate`, `service_rate`.
     - Возвращает композитный `Value::Struct` (или форматированную строку
       для рендера): `{ utilization, queue_length, wait_time, response_time }`.
     - Математика:
       - `ρ = λ / μ`
       - `L_q = ρ² / (1 − ρ)` (при ρ < 1; ρ ≥ 1 → `Err(Overload)`)
       - `W_q = L_q / λ`
       - `W = W_q + 1/μ`
   - `pub fn mmc(args: &[Value]) -> Result<Value, EvalError>`
     - 3 аргумента: `arrival_rate, service_rate, servers`.
     - Использует Erlang-C: `P_wait = ErlangC(c, λ/μ)`,
       `L_q = P_wait · ρ / (1 − ρ)`, `W_q = L_q / λ`, `W = W_q + 1/μ`.
   - `pub fn littles_law(args: &[Value]) -> Result<Value, EvalError>`
     - 2 аргумента: `arrival_rate: Rate`, `response_time: Time` →
       `Count = λ · W` (с conversions единиц).
   - `pub fn erlang_c(args: &[Value]) -> Result<Value, EvalError>`
     - 3 аргумента: `arrival_rate, service_rate, servers` →
       `Percent` (доля запросов, ждущих в очереди).

3. **`crates/canvas-core/src/expr.rs`** — реестр функций:
   - `pub fn register_builtin(reg: &mut FunctionRegistry)` — регистрирует
     FR-013 базовые + FR-015 доменные.
   - `pub struct Value::Struct(HashMap<String, Value>)` — для результатов
     `mm1/mmc` (несколько полей); рендер показывает все поля.

4. **`crates/canvas-render/src/cards.rs`** — рендер `Value::Struct`:
   - Несколько строк ниже текста ноды:
     ```
     utilization: 0.833
     queue_length: 4.17
     wait_time: 4.17 ms
     response_time: 5.0 ms
     ```
   - Для LOD `< 0.6` — только `utilization` (главный показатель).

5. **`crates/canvas-core/tests/expr_queueing.rs`** — золотые тесты:
   - `mm1(1000 rps, 1200 rps)` → util=0.833, L_q=4.17, W_q=4.17 ms,
     W=5.0 ms (допуск ±0.01).
   - `mmc(1000 rps, 600 rps, 3)` (3 сервера, λ=1000, μ=600, c=3) →
     util=0.556, P_wait (Erlang-C) ≈ 0.21 (допуск ±0.01).
   - `mm1(1000 rps, 500 rps)` → `Err(Overload)` (ρ=2 > 1).
   - `littles_law(1000 rps, 5 ms)` → `5` (L = λW = 1000 × 0.005 = 5).
   - `utilization(1000 rps, 400 rps, 3)` → `0.833` (λ/(c·μ) = 1000/1200).

6. **Документация.** `docs/SPEC.md` §5.1 (расширение `canvasdesk.expr` с
   доменными функциями — без изменений формата, функции регистрируются в
   runtime). `docs/interface-objects/node.md` §7 (точки входа — доменные
   функции доступны в calc-ноде).

## Архитектура тестируемости (инварианты FR-015)

1. **Каждая доменная функция — чистая.** `fn(&[Value]) -> Result<Value, EvalError>`,
   без I/O, без состояния. Golden cases в `tests/expr_queueing.rs` —
   табличные тесты с допусками.
2. **Математика зафиксирована.** M/M/1, M/M/c, Erlang-C — стандартные
   формулы; ссылка на канонический источник (Kingman's formula, Erlang-C
   Wikipedia) в docstring каждой функции. Любое расхождение теста и формулы —
   баг в реализации.
3. **Единицы на выходе — явные.** `utilization` всегда `Percent`,
   `wait_time` — `Time`, `queue_length` — `Count`. Тесты проверяют не только
   число, но и `unit`. Это делает FR-016 (анализ bottleneck) надёжным:
   `util > 0.7` — корректное сравнение, без risk-факторов от единиц.
4. **Расширяемость.** Новая доменная функция = один `pub fn` в
   `expr/queueing.rs` + регистрация + тесты. Не требует правки FR-013
   парсера (функции регистрируются в реестре, парсер видит их по имени).

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §7 (точки входа — доменные функции в
  calc-ноде).
- `docs/SPEC.md` §5.1 (без изменений формата — функции runtime-registered).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-015 (золотые тесты).
- `docs/change-requests/fr-013-text-node-numi-expr.md` — базовый calc-движок.
- `docs/change-requests/fr-016-bottleneck-queue-risk.md` — анализ
  использует `utilization`, `queue_length`, `wait_time` из FR-015.
- `docs/change-requests/fr-017-what-if-scenarios.md` — сценарии с доменными
  функциями.

## Проверка (Verification)

### Юнит-тесты

- `units::convert(Value { 1, sec }, ms_unit) == Value { 1000, ms }`.
- `units::convert(Value { 1, GB }, MB_unit) == Value { 1024, MB }`.
- `units::add(Value { 1, sec }, Value { 500, ms }) == Value { 1.5, sec }`
  (auto-convert).
- `units::add(Value { 5, ms }, Value { 3, rps })` → `Err(UnitMismatch)`.
- `queueing::utilization(&[1000 rps, 1200 rps])` → `Ok(0.833 %)`.
- `queueing::utilization(&[1000 rps, 400 rps, 3 servers])` → `Ok(0.833 %)`.
- `queueing::mm1(&[1000 rps, 1200 rps])` → `Ok(Struct { utilization: 0.833,
  queue_length: 4.17, wait_time: 4.17 ms, response_time: 5.0 ms })`.
- `queueing::mmc(&[1000 rps, 600 rps, 3])` → `Ok(Struct { ... P_wait: 0.21 })`.
- `queueing::mm1(&[1000 rps, 500 rps])` → `Err(Overload { rho: 2.0 })`.
- `queueing::littles_law(&[1000 rps, 5 ms])` → `Ok(5)` (L = 1000 × 0.005).
- `expr::parse("mm1(1000 rps, 1200 rps)")` → `Ok(Expr::Call("mm1", [...]))`.
- `expr::eval(parse("mm1(1000 rps, 1200 rps)"), Env::empty())` →
  `Ok(Struct { ... })`.

### Интеграционные тесты

- `crates/canvas-app/tests/integration_expr_queueing.rs`:
  - Создание calc-ноды через MCP `node_edit { id, expr: "mm1(1000 rps, 1200 rps)" }`.
  - `flow_recalc` возвращает структуру с 4 полями; рендер показывает 4 строки
    под текстом ноды.
  - Изменение `expr` на `mmc(1000 rps, 600 rps, 3)` → пересчёт, новые значения.
  - Изменение `expr` на `mm1(1000 rps, 500 rps)` → red-строка с тултипом
    `Overload: ρ = 2.0 ≥ 1`.

### Ручная приёмка

- Пользователь пишет формулу `mm1(1000 rps, 1200 rps)` → под нодой:
  ```
  utilization: 83.3%
  queue_length: 4.17
  wait_time: 4.17 ms
  response_time: 5.0 ms
  ```
- Пользователь пишет `utilization(1000 rps, 400 rps, 3)` → `83.3%`.
- Пользователь пишет `mm1(1000 rps, 500 rps)` → красная строка
  `Overload: ρ = 2.0 ≥ 1`.
- Пользователь пишет `littles_law(1000 rps, 5 ms)` → `5`.
- На ландшафте: 5 нод-сервисов, каждый с `mm1(...)` формулой; рёбра value
  (FR-014) передают `arrival_rate` между нодами; результат пересчитывается
  live при изменении входа.
- MCP: `node_edit { id, expr: "mmc(5000 rps, 600 rps, 10)" }` → нода
  показывает структуру с 4 полями; `flow_recalc` отдаёт JSON-карту.

## Открытые вопросы дизайна

- **M/M/c Erlang-C формула.** Точная формула включает факториалы при малых
  `c`; для `c > 170` — численная нестабильность. Решение: рекуррентная
  формула Erlang-C (B формула → C формула); предельный случай `c → ∞`
  отложить. v1 — `c ≤ 1000`, выше — `Err(ServersLimitExceeded)`.
- **Составные единицы в выводе.** `mm1` возвращает `Value::Struct`, не
  скаляр. Рендер — 4 строки. Альтернатива — `Value::String` с форматом
  `"util=0.83, L_q=4.17, W_q=4.17ms"` — компактнее, но не парсится
  downstream-формулой. Решение: `Value::Struct` + accessor `$in.utilization`
  (расширение `Expr::Field` в FR-014/v2).
- **Расширенные модели очередей.** M/G/1, M/G/c, G/G/1 (Kingman) —
  отложены до v2. v1 — только Markovian (M/M/1, M/M/c).
- **Суффиксы `k`, `M`, `G`.** В Numi `1k = 1000`; в CanvasDesk сохранить.
  Но `1 MB` ≠ `1M` (байт vs миллион) — важно документировать; парсер
  различает по пробелу (`1k rps` = 1000 rps; `1 kbps` = килобит/сек).
- **Локализация.** Функции — латиница (`mm1`, `mmc`, `utilization`).
  Синонимы (`м/м/1`, `утилизация`) — v2.

## История изменений (Changelog)

- `2026-09-15` — агент: документ создан по запросу пользователя (доменные
  примитивы: системные метрики + теория очередей). Зафиксированы 4 инварианта
  тестируемости (чистые функции, зафиксированная математика, явные единицы
  выхода, расширяемость через реестр). Статус `выявлено`. Зависимости:
  FR-013 (парсер/eval), FR-014 (поток значений для chained-расчётов),
  FR-016 (анализ bottleneck), FR-017 (what-if).

## Источники истины (References)

- `crates/canvas-core/src/expr.rs` (FR-013) — `Value`, `Unit`, `Function`,
  `Env`. Точки расширения: `Dimension`, `FunctionRegistry`, `Value::Struct`.
- `crates/canvas-core/src/expr/units.rs` (новый) — таблица conversions.
- `crates/canvas-core/src/expr/queueing.rs` (новый) — доменные функции.
- `crates/canvas-render/src/cards.rs` — рендер `Value::Struct` (несколько
  строк).
- `crates/canvas-core/tests/expr_queueing.rs` (новый) — золотые тесты.
- `docs/SPEC.md` §5.1 (формат `.canvas` — без изменений), §6.2 (LOD для
  struct-вывода).
- `docs/interface-objects/node.md` §7 (точки входа — доменные функции).
- `docs/change-requests/fr-013-text-node-numi-expr.md` — базовый calc-движок.
- `docs/change-requests/fr-014-edge-value-flow.md` — поток значений
  (chained-расчёты на ландшафте).
- `docs/change-requests/fr-016-bottleneck-queue-risk.md` — анализ
  использует результаты FR-015.
- `docs/change-requests/fr-017-what-if-scenarios.md` — сценарии с доменными
  функциями.
- Внешние источники:
  - Erlang-C formula: `https://en.wikipedia.org/wiki/Erlang_(unit)#Erlang_C_formula`.
  - Little's law: `https://en.wikipedia.org/wiki/Little%27s_law`.
  - Kingman's formula (для v2 M/G/1): `https://en.wikipedia.org/wiki/Kingman%27s_formula`.
