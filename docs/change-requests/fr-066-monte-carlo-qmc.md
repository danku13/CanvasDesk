# FR-066: Monte Carlo + QMC-движок (M5 волны S ADR-0008) — N≥10⁴ прогонов с распределёнными параметрами и P50/P90/P99-квантилями

- **Статус:** выявлено (план)
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** агент (планирование)
- **Источник:** план реализации ADR-0008 волна S (`docs/plans/adr-0008-wave-s-plan.md`), по запросу владельца 2026-09-24
- **Связанные задачи:** ADR-0008; `docs/architecture/math-computing-stack.md` §5.4 (P3 сценарный/MC), §5.6 (детерминизм), §9 (M5/S3); `docs/plans/product-roadmap.md` §4.5 (S3); FR-063 (M2 stats — распределения/RNG, **ОБЯЗАТЕЛЬНАЯ** зависимость), FR-065 (M4 parallel — параллельные прогоны, **ОБЯЗАТЕЛЬНАЯ** зависимость), FR-016 (analyze — overlay на квантилях), FR-017 (what-if — сценарии), FR-029 (порты значений), ADR-0006 (эталон №5 unit economics — приёмка); `docs/DEPENDENCIES.md` §3 (`sobol_burley`)
- **Создан:** 2026-09-24
- **Обновлён:** 2026-09-24

---

## Описание (What)

**Что добавляется.** Monte Carlo + QMC-движок — слой L4/P3 в
архитектуре `math-computing-stack.md` §5.4. Движок выполняет `N≥10⁴`
прогонов расчётного графа (`propagate_with_lines`) с распределёнными
параметрами (Normal/LogNormal/Exp/Poisson из FR-063) и сводит
результаты к квантилям **P50/P90/P99** как новым типам результатов
нод. QMC-режим (`sobol_burley`, Owen-scrambled Sobol) даёт меньшую
дисперсию при том же `N` через квазислучайную последовательность с
низким расхождением. Детерминизм — сидированный RNG
(`ChaCha8Rng::seed_from_u64`, hash(content) ⊕ scenario_seed ⊕
run_idx) + версия движка в `canvas.extra["canvasdesk"]["engine"]`.

**Для кого.** Архитектор/финдиректор, которому нужна вероятностная
оценка риска, запаса мощности или чувствительности глубже ±20 %-сеток
FR-017 v1. Пример: эталон №5 ADR-0006 (unit economics) — CAC/LTV/churn
как распределения → `runway` P10, `mrr` P50/P90; bottleneck overlay на
P90 (FR-016). MC/QMC отвечает на вопрос «с какой вероятностью runway
уйдёт в ноль за 12 месяцев» — вопрос, на который один прогон
`propagate_with_lines` ответить не может.

**Границы v1 FR-066.** `propagate_with_lines` **остаётся нетронутой** —
M5 добавляет **новую** sibling-функцию `flow::propagate_monte_carlo`.
Квантили — **collect-then-reduce** (sort node ids → `percentile`
builtin `expr.rs:1700–1711`). `analyze::analyze` кормится
**синтетическим** `FlowSolutions` (P50/P90/P99 в `outputs`/`named`) —
**без правок analyze**. Фича `qmc = ["stats", "parallel",
"dep:sobol_burley"]` подразумевает обе обязательные зависимости и **не
собирается на wasm** (`wasm32-wasip1`); core `propagate_with_lines`
остаётся wasm-чистым.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `flow` | **НОВАЯ** `propagate_monte_carlo(canvas, mc_config) -> Result<McResult, ...>`; `propagate_with_lines` не трогается (контракт §5.1) | `docs/architecture/math-computing-stack.md` §5.4; `crates/canvas-core/src/flow.rs:394` |
| `expr/mc.rs` | **Новый модуль**: `McConfig`, `Distribution`-маппинг, Owen-scrambled Sobol → inverse-CDF, collect-then-reduce квантили | `docs/architecture/math-computing-stack.md` §5.4, §5.6; новый файл |
| `analyze` | Кормится синтетическим `FlowSolutions` (P90 в `outputs`/`named`) — **без правок** `analyze.rs` | `crates/canvas-core/src/analyze.rs:242`; FR-016 |
| MCP | **Новый инструмент** `monte_carlo_run` (ToolSpec + dispatch arm); `skills/` — в том же коммите (UPDATE-PROTOCOL) | `crates/canvas-mcp/src/lib.rs:186–489`; `crates/canvas-scene/src/mcp.rs:256`; `skills/UPDATE-PROTOCOL.md` |
| Cargo | Новая фича `qmc = ["stats", "parallel", "dep:sobol_burley"]`; `sobol_burley` в `[workspace.dependencies]` (optional) | `Cargo.toml`; `docs/DEPENDENCIES.md` §3→§2 |
| `.canvas` | Engine-метаданные `canvas.extra["canvasdesk"]["engine"] = {version, seed, stats, parallel, qmc}` (raw JSON, паттерн `whatif.rs`) | `crates/canvas-core/src/model.rs:763`; `docs/SPEC.md` §5.1 |
| wasm | Фича `qmc` **не собирается** на `wasm32-wasip1`; core (`propagate_with_lines`) — wasm-чистый | `scripts/wasm_gate.sh`; FR-036 |

## Анализ (Root Cause)

Слой расчётов сегодня (`flow.rs:394`, `flow.rs:353`):

- **`propagate_with_lines` — один прогон.** Сигнатура
  `propagate_with_lines(canvas, whatif) -> Result<FlowSolutions,
  CycleError>` (`flow.rs:394`, тонкая обёртка над
  `propagate_with_lines_data` `flow.rs:405`) — **один**
  детерминированный пересчёт DAG с фиксированными значениями. Никакого
  пакетного переисполнения с распределениями нет.
- **Нет распределений как входов.** `WhatIfOverrides { line_exprs,
  node_values }` (`flow.rs:235`) и `Scenario { name, line_exprs }`
  (`whatif.rs:21`) подменяют строки/значения **конкретными**
  выражениями. Типа «`rps ~ Normal(μ=1000, σ=100)`» в схеме нет — для
  него нужен `McConfig { runs, params: HashMap<(node_id, param),
  Distribution>, seed, quantiles }` (определяется в FR).
- **Нет квантилей как результатов.** `FlowSolutions { outputs, lines,
  named, warnings }` (`flow.rs:353`) хранит **по одному** значению на
  ноду/строку/named-output. Никаких P50/P90/P99 → `analyze::analyze`
  (`analyze.rs:242`) не видит «хвоста» распределения и не может наложить
  overlay на P90.
- **Точки опоры (уже есть в репо).**
  - `FlowSolutions` — `Clone+Default`; `FlowOutputs = HashMap<String,
    Result<Value, EvalError>>`; `NamedOutputs = HashMap<(String,
    String), Value>` (`flow.rs:353`). → M5 строит **синтетический**
    `FlowSolutions` (P50/P90/P99 в `outputs`/`named`) → `analyze::analyze`
    без правок.
  - `percentile` — **уже builtin** (`eval_percentile`,
    `expr.rs:1700–1711`). → M5 переиспользует (collect-then-reduce:
    `Vec<Value>` per node → sort → percentile).
  - `analyze::analyze(canvas, solutions, config) -> AnalysisState`
    (`analyze.rs:242`) — чистая функция; читает `solutions.outputs` и
    `solutions.named` (`utilization`/`wait_time`/`queue_length`,
    `analyze.rs:46–48`); `AnalysisConfig` — пороги
    `warn_util:0.7`/`critical_util:0.9`/… (`analyze.rs:52–79`);
    `Severity { None<Warn<Critical<Overload }`. → overlay на P90 без
    правок `analyze`.
  - `Canvas.extra` — `serde_json::Map<String, Value>` (`model.rs:763`,
    `#[serde(flatten)]`); unknown fields survive round-trip. →
    `canvasdesk.engine` ложится туда как raw JSON, без typed-struct на
    `Canvas`.
  - Паттерн персистентности `whatif.rs:39–115` — raw JSON в
    `canvas.extra["canvasdesk"]["whatif"]`. → M5 зеркально кладёт
    `canvas.extra["canvasdesk"]["engine"]` через
    `engine_from_canvas`/`engine_to_canvas`.
  - `rand_chacha::ChaCha8Rng::seed_from_u64(u64)` — из FR-063.
    → сидированный RNG, `thread_rng()` **запрещён**.
  - MCP: `mcp_dispatch(scene, templates, method, params) ->
    Result<Value, String>` (`mcp.rs:256`); TOOLS — `&[ToolSpec]` в
    `canvas-mcp/src/lib.rs:186–489` (40 инструментов). → M5 добавляет
    `monte_carlo_run`.
- **Чего нет на wasm.** `propagate_with_lines` сегодня wasm-чистый
  (FR-036, `scripts/wasm_gate.sh`). M5 **не должен сломать** это: фича
  `qmc` (parallel+rand+sobol_burley) не собирается на `wasm32-wasip1`;
  core остаётся за `default`-фичами.

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| **Новый модуль `expr/mc.rs`** | `crates/canvas-core/src/expr/mc.rs` (новый) | `McConfig { runs: usize, params: HashMap<(String, String), Distribution>, seed: u64, quantiles: Vec<f64> /* [0.5,0.9,0.99] */ }`; `Distribution`-enum из FR-063; Owen-scrambled Sobol (`sobol_burley::SobolSequence`) → сэмплы `[0,1)` → inverse-CDF (`statrs`) → params; collect-then-reduce квантили (sort node ids → `percentile`) |
| **`flow::propagate_monte_carlo`** | `crates/canvas-core/src/flow.rs` (новая sibling) | `propagate_monte_carlo(canvas, mc_config: &McConfig) -> Result<McResult, CycleError>`; N прогонов `propagate_with_lines` с сэмплированными `WhatIfOverrides` (params → `node_values`); чанкование 256/задача (`parallel` из FR-065); прогресс через callback/`AppEvent`; **НЕ трогает** `propagate_with_lines` |
| **Синтетический `FlowSolutions`** | `crates/canvas-core/src/expr/mc.rs` | Из `Vec<FlowSolutions>` собрать P50/P90/P99 в `outputs`/`named` (sort node ids; `percentile` builtin `expr.rs:1700–1711`) → `analyze::analyze` без правок |
| **Engine-метаданные** | `crates/canvas-core/src/expr/mc.rs` (паттерн `whatif.rs:39–115`) | `canvas.extra["canvasdesk"]["engine"] = { "version": "M5.0", "seed": "<hash>", "stats": true, "parallel": true, "qmc": true }`; `engine_from_canvas`/`engine_to_canvas` (raw JSON, без typed-struct на `Canvas`); `Canvas.extra` (`model.rs:763`) |
| **MCP `monte_carlo_run`** | `crates/canvas-mcp/src/lib.rs:186–489` (ToolSpec) + `crates/canvas-scene/src/mcp.rs:256` (dispatch arm) | `ToolSpec { name: "monte_carlo_run", description, required: ["runs","params"], properties: {...} }`; dispatch → `propagate_monte_carlo` → `analyze::analyze` → `{ quantiles, severity, runs, seed, duration_ms }` |
| **Активация фичи `qmc`** | `Cargo.toml` (workspace + `canvas-core`) | `qmc = ["stats", "parallel", "dep:sobol_burley"]`; `sobol_burley` (MIT OR Apache-2.0) в `[workspace.dependencies]` optional; canvas-core — optional за `qmc` |
| **`skills/` (новый MCP-инструмент)** | `skills/` (по UPDATE-PROTOCOL) | В **том же коммите** что и `monte_carlo_run` ToolSpec; контракт-тест `skills_*` в canvas-mcp валит CI при рассинхроне |

## Контракты на стыках

- **§5.1 — `propagate_with_lines` СТАБИЛЬНА.** M5 добавляет **НОВУЮ**
  `propagate_monte_carlo`, **НЕ трогает** существующую. Сигнатура
  `propagate_with_lines(canvas, whatif) -> Result<FlowSolutions,
  CycleError>` (`flow.rs:394`) — неизменна. Существующие вызовы
  (`scene.rs::recompute_flow` — M3 владелец) и MCP `propagate_*`
  работают как прежде.
- **§5.5 — `FlowSolutions` стабильна.** M5 **производит** синтетический
  снимок (P50/P90/P99 в `outputs`/`named`) → `analyze::analyze` БЕЗ
  правок. Структура `FlowSolutions { outputs, lines, named, warnings }`
  (`flow.rs:353`) — без изменений; `Clone+Default` сохраняется.
- **§5.6 — `qmc` подразумевает обе.** `qmc = ["stats", "parallel",
  "dep:sobol_burley"]`. M5 нуждается и в `stats` (RNG, распределения,
  `statrs`), и в `parallel` (прогоны чанками 256/задача). Невозможно
  активировать `qmc` без `stats`+`parallel`.
- **§5.7.2 — сид = `hash(content) ⊕ scenario_seed ⊕ run_idx`.**
  Детерминизм: один и тот же `mc_config.seed` → **побитово**
  идентичный `Vec<FlowSolutions>` → идентичные квантили. `thread_rng()`
  **запрещён**; только `ChaCha8Rng::seed_from_u64`.
- **§5.7.3 — collect-then-reduce квантили.** `Vec<Value>` per node →
  sort node ids (детерминированный порядок) → `percentile` builtin
  (`expr.rs:1700–1711`). **НЕТ** `par_iter().reduce()` (race-condition
  на плавающей точке); reduce на собранном `Vec`.
- **§5.7.4 — версия движка в `extra`.**
  `canvas.extra["canvasdesk"]["engine"] = { "version": "M5.0",
  "seed": "<hash>", "stats": true, "parallel": true, "qmc": true }`.
  Raw JSON, паттерн `whatif.rs:39–115` — без typed-struct на `Canvas`.
  При рассинхроне версии → `McResult` помечается `stale=true`.
- **§5.8 — `qmc` не на wasm.** `wasm32-wasip1` сборка **без** `qmc`
  (`scripts/wasm_gate.sh --check` — зелёный). Core
  `propagate_with_lines` остаётся wasm-чистым. MCP на wasm — без
  `monte_carlo_run` (`mcp_wasm_gate.sh`).

**Принципиально:** M5 — **волна S2** (после merge FR-063+FR-065). **НЕ
трогать** `propagate_with_lines`/`scene.rs::recompute_flow` (M3 —
владелец).

## Фазы и проверяемые коммиты

### Фаза P1 — `propagate_monte_carlo` + seeded RNG + chunked runs

**Коммит:** `feat(core/mc): propagate_monte_carlo + seeded RNG + chunked runs (FR-066 P1)`

Новый модуль `expr/mc.rs`: `McConfig`, `Distribution`-маппинг
(переиспользует FR-063). `flow::propagate_monte_carlo` — N прогонов
`propagate_with_lines` с сэмплированными params (`rand_distr` +
`ChaCha8Rng::seed_from_u64(hash(content) ⊕ seed ⊕ run_idx)`); чанкование
256/задача (`parallel` из FR-065); прогресс через callback/`AppEvent`.
**НЕ трогает** `propagate_with_lines`.

**Гейт:** `cargo test -p canvas-core --features qmc` — зелёный;
seed-воспроизводимость (тот же seed → **побитово** тот же
`Vec<FlowSolutions>`); `scripts/wasm_gate.sh --check` (core без `qmc`
зелёный).

### Фаза P2 — `sobol_burley` QMC + P50/P90/P99

**Коммит:** `feat(core/mc): sobol_burley QMC + P50/P90/P99 quantile results (FR-066 P2)`

QMC-режим: `sobol_burley` Owen-scrambled → сэмплы `[0,1)` → inverse-CDF
(`statrs`) → params. Квантили collect-then-reduce (sort node ids →
`percentile` builtin `expr.rs:1700–1711`). Синтетический `FlowSolutions`
(P50/P90/P99 в `outputs`/`named`). Engine-метаданные
`canvas.extra["canvasdesk"]["engine"]` (`engine_from_canvas`/`engine_to_canvas`,
паттерн `whatif.rs:39–115`).

**Гейт:** QMC vs MC — **меньшая дисперсия** при том же `N` (тест на
эталоне №5, oracle-числа паттерн `close_1pct` `canvas-scene/src/tests.rs:1470`);
квантили воспроизводимы (тот же seed → те же P50/P90/P99); `cargo deny
check` — `sobol_burley` (MIT OR Apache-2.0) разрешена.

### Фаза P3 — MCP `monte_carlo_run` + analyze integration

**Коммит:** `feat(mcp): monte_carlo_run tool + analyze integration (FR-066 P3)`

`monte_carlo_run` ToolSpec + TOOLS (`canvas-mcp/src/lib.rs:186–489`) +
dispatch arm (`canvas-scene/src/mcp.rs:256`). `skills/` обновлены **в
том же коммите** (UPDATE-PROTOCOL; контракт-тест `skills_*`). Синтетический
снимок → `analyze::analyze` (overlay bottleneck на P90 — **без правок
analyze**). e2e: 10⁴ прогонов эталона ADR-0006 №5 **< 1 с/4 ядра**;
фиксированный seed → идентичные квантили при повторе.

**Гейт:** `cargo test --workspace --features qmc` — зелёный;
`mcp_wasm_gate.sh --check` (без `qmc`, без `monte_carlo_run`);
skills контракт-тест; `cargo clippy -D warnings`.

## Ограничения для агента-реализатора

1. **Сидированный RNG.** Только `rand_chacha::ChaCha8Rng::seed_from_u64(u64)`
   (из FR-063). `thread_rng()` **ЗАПРЕЩЁН**. Сид = `hash(content) ⊕
   scenario_seed ⊕ run_idx` (§5.7.2).
2. **Collect-then-reduce квантили.** `Vec<Value>` per node → **sort node
   ids** (детерминированный порядок) → `percentile` builtin
   (`expr.rs:1700–1711`). **НЕТ** `par_iter().reduce()` — race-condition
   на плавающей точке ломает воспроизводимость.
3. **`analyze` — без правок.** Синтетический `FlowSolutions` →
   `analyze::analyze` (`analyze.rs:242`) как есть. `AnalysisConfig`
   (`analyze.rs:52–79`) — без изменений.
4. **Версия движка в `canvas.extra["canvasdesk"]["engine"]`.** Raw JSON,
   паттерн `whatif.rs:39–115` — **без typed-struct на `Canvas`**.
   `engine_from_canvas`/`engine_to_canvas` зеркалят whatif.
5. **Прогоны чанкуются (256/задача).** Прогресс публикуется через
   callback/`AppEvent` (UI показывает `N/total`, ETA).
6. **Воспроизводимость — first-class.** Фиксированный seed →
   **идентичные** квантили (побитово). Любая недетерминированная
   операция — баг.
7. **Бюджет производительности.** 10⁴ прогонов эталона №5 ADR-0006 —
   **< 1 с / 4 ядра** (QMC или MC). Превышение — блокер для merge.
8. **`qmc` не на wasm.** Feature-gate; `scripts/wasm_gate.sh --check`
   (core без `qmc`) — зелёный. Core `propagate_with_lines` —
   wasm-чистый (без feature).
9. **НЕ трогать `propagate_with_lines`/`scene.rs::recompute_flow`.** M3 —
   владелец. M5 только **добавляет** sibling `propagate_monte_carlo`.
10. **`skills/` — в том же коммите** что и `monte_carlo_run` ToolSpec
    (AGENTS.md; UPDATE-PROTOCOL; контракт-тест `skills_*`).

## Наглядная проверка (5 минут)

**Демо.** Эталон №5 (unit economics ADR-0006) — Monte Carlo 10⁴
прогонов (CAC/LTV/churn как распределения: `cac ~ LogNormal(μ=120,
σ=15)`, `ltv ~ Normal(μ=600, σ=80)`, `churn ~ Exp(λ=0.05)`) →
P50/P90/P99 видны как результаты нод (`runway` P10, `mrr` P50/P90);
bottleneck overlay на P90 (FR-016 цвета). Повторный прогон с **тем же
seed** → идентичные квантили (визуально дельт нет, diff снимков = 0).

**Автотесты:**
- MC e2e **< 1 с / 4 ядра** (10⁴ прогонов эталона №5).
- Seed-воспроизводимость: тот же seed → **побитово** тот же
  `Vec<FlowSolutions>` и те же квантили.
- `analyze` на синтетическом снимке: `Severity` корректно считается на
  P90 (overlay bottleneck на P90).
- QMC-дисперсия **< MC-дисперсия** при том же `N` (тест на эталоне №5).
- `cargo deny check` — зелёный (sobol_burley MIT OR Apache-2.0).
- `scripts/wasm_gate.sh --check` — core без `qmc` зелёный.

## Проверка (Verification)

Чек-лист перед переходом статуса `выявлено (план)` → `в работе`:

- [ ] `cargo test --workspace --features qmc` — зелёный (все три фазы).
- [ ] `cargo clippy -D warnings` — зелёный.
- [ ] `cargo deny check` — `sobol_burley` лицензия разрешена.
- [ ] `scripts/wasm_gate.sh --check` — core **без** `qmc` зелёный.
- [ ] `mcp_wasm_gate.sh --check` — без `qmc` зелёный (без `monte_carlo_run`).
- [ ] MC e2e: 10⁴ прогонов эталона №5 ADR-0006 **< 1 с / 4 ядра**.
- [ ] Seed-воспроизводимость: тот же seed → идентичные квантили (побитово).
- [ ] `analyze` на синтетическом снимке — `Severity` на P90 корректен.
- [ ] QMC-дисперсия < MC-дисперсия при том же `N` (тест).
- [ ] Skills контракт-тест `skills_*` — зелёный.
- [ ] `propagate_with_lines` сигнатура **не изменена** (контракт §5.1).
- [ ] `scene.rs::recompute_flow` — **не тронут** (M3 владелец).
- [ ] `FlowSolutions` структура — **без изменений** (контракт §5.5).

## Точки входа

После реализации (переход `в работе` → `выполнено`) обновить:

- **`docs/interface-objects/node.md`** — квантили P50/P90/P99 как новые
  типы результатов нод (раздел «Результаты»).
- **`user-docs/calculations.md`** — раздел «Monte Carlo» (когда
  применять, как задавать распределения, как читать P50/P90/P99,
  воспроизводимость через seed).
- **`docs/SPEC.md`** — §MCP (добавить `monte_carlo_run`); §6.3 (расчётный
  движок — добавить слой L4/P3 MC/QMC).
- **`docs/DEPENDENCIES.md`** — §3 (запись `sobol_burley`) → §2 (workspace.dependencies).
- **`docs/architecture/math-computing-stack.md`** — §5.4 (P3 сценарный/MC —
  актуализировать после merge), §5.6 (детерминизм — ссылка на FR-066),
  §9 (M5/S3 — отметить выполнено).
- **`skills/`** — если новый MCP-инструмент `monte_carlo_run` — обновить
  в **том же коммите** по UPDATE-PROTOCOL.
- **`docs/change-requests/index-cr-fr.md`** — добавить FR-066 в индекс.

## История изменений

- `2026-09-24` — агент: создан документ (план, статус `выявлено`).
  Заголовок, Описание, Влияние, Анализ (код-ссылки `flow.rs:394/353/235`,
  `analyze.rs:242/46–48/52–79`, `expr.rs:1700–1711`, `whatif.rs:21/39–115`,
  `model.rs:763`, `mcp.rs:256`, `lib.rs:186–489`, `tests.rs:1470`),
  Требуемые изменения, Контракты на стыках (§§5.1, 5.5, 5.6, 5.7.2–5.7.4,
  5.8), Фазы P1–P3 с коммит-сообщениями, Ограничения для
  агента-реализатора, Наглядная проверка (5 минут), Проверка, Точки
  входа, Источники истины.

## Источники истины

- **Код (актуальный аудит репо):** `crates/canvas-core/src/flow.rs:394`
  (`propagate_with_lines`, СТАБИЛЬНА §5.1), `flow.rs:405`
  (`propagate_with_lines_data`), `flow.rs:353` (`FlowSolutions`, СТАБИЛЬНА
  §5.5), `flow.rs:235` (`WhatIfOverrides`);
  `crates/canvas-core/src/analyze.rs:242` (`analyze::analyze`, без правок),
  `analyze.rs:46–48` (named outputs), `analyze.rs:52–79` (`AnalysisConfig`);
  `crates/canvas-core/src/expr.rs:1700–1711` (`eval_percentile` builtin);
  `crates/canvas-core/src/whatif.rs:21` (`Scenario`), `whatif.rs:39–115`
  (паттерн raw-JSON персистентности — зеркало для `engine`);
  `crates/canvas-core/src/model.rs:763` (`Canvas.extra`,
  `serde_json::Map`, `#[serde(flatten)]`, unknown fields survive
  round-trip); `crates/canvas-scene/src/mcp.rs:256` (`mcp_dispatch`);
  `crates/canvas-mcp/src/lib.rs:186–489` (TOOLS `&[ToolSpec]`, 40 → 41);
  `crates/canvas-scene/src/tests.rs:1470` (паттерн `close_1pct`, oracle
  эталона №5).
- **План/архитектура:** `docs/plans/adr-0008-wave-s-plan.md` — §§5.1,
  5.5, 5.6, 5.7.2, 5.7.3, 5.7.4, 5.8 (контракты M5);
  `docs/architecture/math-computing-stack.md` — §5.4 (P3 сценарный/MC),
  §5.6 (детерминизм), §9 (M5/S3); `docs/plans/product-roadmap.md` §4.5
  (S3); ADR-0006 — эталон №5 (unit economics, приёмка 10⁴ прогонов < 1 с
  /4 ядра, фиксированный seed); `docs/DEPENDENCIES.md` §3 (`sobol_burley`).
- **Связанные FR:** FR-063 (M2 stats — **обязательная** зависимость),
  FR-065 (M4 parallel — **обязательная** зависимость), FR-016 (analyze —
  overlay на P90), FR-017 (what-if — паттерн `whatif.rs`), FR-029 (порты
  значений), FR-036 (wasm gate).
- **AGENTS.md / протоколы:** `AGENTS.md` — правило «состав MCP-инструментов
  → `skills/` в том же коммите»; `skills/UPDATE-PROTOCOL.md`;
  `scripts/wasm_gate.sh`, `scripts/mcp_wasm_gate.sh`.
- **Внешние крейты:** `rand_chacha::ChaCha8Rng::seed_from_u64`
  (сидированный RNG); `rand_distr` (Normal/LogNormal/Exp/Poisson);
  `statrs` (inverse-CDF для QMC-сэмплов); `sobol_burley` (MIT OR
  Apache-2.0, Owen-scrambled Sobol, `SobolSequence`).
