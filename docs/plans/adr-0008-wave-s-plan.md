# План реализации ADR-0008 — волна S: параллельная разработка M2–M5

- **Статус:** предложено (план; готов к исполнению после гейта Go продуктового
  роадмапа — `docs/plans/product-roadmap.md` §4.4/§4.5)
- **Дата:** 2026-09-24
- **Автор:** агент (планирование по запросу владельца: «спланировать разработку
  решения на базе ADR-0008 — расписать CR/FR, разбить для параллельной работы
  до 4 агентов, спланировать контракты на стыках, проверяемые коммиты, анализ
  готовых библиотек»)
- **Связанные:** `docs/adr/adr-0008-math-computing-stack.md` (решение);
  `docs/architecture/math-computing-stack.md` (Архдок — слои L0–L5, этапы M0–M6,
  §5 параллелизм, §5.6 детерминизм, §7 лицензии, §9 дорожная карта);
  `docs/plans/product-roadmap.md` (волны 0/A/B/V/S, гейт Go);
  `docs/change-requests/cr-013-agent-architecture-composition.md` (R1–R5);
  FR-063/064/065/066 (создаются этим планом); `docs/DEPENDENCIES.md` §3
  (реестр кандидатов); `AGENTS.md` (правила)
- **Что НЕ делает:** не запускает реализацию до гейта Go (решение 8 роадмапа);
  не выводит M6 (линейная алгебра — «по потребности», отдельный FR с доменом);
  не вводит криптозависимости (архдок §7.5 — только отдельным ADR)

---

## 1. Контекст и границы

ADR-0008 принят (вариант B: pure-Rust реестр, этапность, allowlist). Этапы
**M0–M1 выполнены** в волне 0 продуктового роадмапа (CP0, 2026-09-18):
`deny.toml` + `cargo-deny` в CI, `THIRD-PARTY-NOTICES` (cargo-about),
SBOM-рецепт (cargo-auditable), реестр зависимостей `docs/DEPENDENCIES.md`,
cargo-фичи `stats`/`parallel` в `canvas-core` (пустые гейты). Криптозависимостей
нет, дерево 100 % пермиссивное, `Cargo.lock` закоммичен (`--locked`).

**Оставшаяся работа ADR-0008** — этапы M2–M5 волны S (после гейта Go):

| Этап | Архдок §9 | Слой | Суть | FR (этот план) |
|---|---|---|---|---|
| M2 | S1 | L2 | `expr/stats.rs`: распределения, квантили, ДИ, детерминированный сид | **FR-063** |
| M3 | S2 | L4 (P1) | worker-тред + double buffer; FR-017 v2 (freeze/сравнение сценариев) | **FR-064** |
| M4 | S3 | L4 (P2) | поярусный параллелизм DAG (`topo_levels` → `std::thread::scope` → `rayon`) | **FR-065** |
| M5 | S3 | L4 (P3) | Monte Carlo + QMC (`sobol_burley`), квантили P50/P90/P99 как результаты | **FR-066** |
| M6 | бэклог | L3 | линейная алгебра (`faer`/`nalgebra`) — «по потребности», вне этого плана | отдельный FR |

Критический путь ADR-0007/0008 (волна A — FR-029/032/033, R5; волна B —
FR-016/017 v1) **уже реализован** и сходится автотестами на эталонах
ADR-0005/0006. Волна S углубляет тяжёлые режимы, которых до гейта нет
(эталоны ≤ 45 нод, аналитические формулы, полный пересчёт уже < 10 мс).

**Границы плана:** расчётное ядро `canvas-core` (язык формул, доменные функции,
DAG-поток) и его обвязка (`canvas-scene` — ревал, `canvas-mcp` — инструменты).
Вне границ: рендер, shell, UI-волны, MCP-транспорт (адресация портов —
ADR-0003/FR-029, уже сделано).

---

## 2. Анализ готовых библиотек и рецептов (требование владельца)

Принцип ADR-0008: **собственное ядро L0 сохраняется; внешние библиотеки —
только за чистыми функциями реестра L1** (`dispatch(func, &[Value])`). Всё
дерево кандидатов пермиссивно, pure-Rust, без криптографии, без C-зависимостей
(архдок §4.2; лицензии проверены). Ниже — какие рецепты быстро реализуемы или
адаптируемы, по этапам.

### 2.1. M2 — статистический слой (FR-063)

| Крейт | Версия | Лицензия | Что берём | Рецепт адаптации |
|---|---|---|---|---|
| `statrs` | 0.17 | MIT | `Distribution::Normal/LogNormal/Exp/Poisson/Beta/Gamma/Triangular`, `.cdf()/.pdf()/.inverse_cdf()` | Обернуть в `expr/stats.rs::dispatch` → `normal_quantile(p,μ,σ)`, `normal_cdf(x,μ,σ)`, `lognormal_mean(...)` и т.д. Аргументы — `Value` (с размерностью), ошибки — `EvalError::BadCall`. Крейт pure-Rust, транзитивно чистый. |
| `rand` | 0.8 | MIT OR Apache-2.0 | `SeedableRng` + `Rng`-трейт | Только как база для `rand_chacha`/`rand_distr`; прямой вызов `thread_rng()` **запрещён** (детерминизм §5.6.2). |
| `rand_chacha` | 0.3 | MIT OR Apache-2.0 | `ChaCha8Rng::seed_from_u64(u64)` | Детерминированный RNG: сид = `hash(canvas.content) ⊕ scenario_seed ⊕ run_index`. Воспроизводимость прогона = один сид → одна выборка. ChaCha8 (не 12/20) — детерминизм + скорость. |
| `rand_distr` | 0.4 | MIT OR Apache-2.0 | `Distribution::Normal::new?`, `.sample(&mut rng)` | Сэмплирование для M5 Monte Carlo; в M2 — только если появляется сэмпл-функция. Пара к `statrs` (CDF/PDF — `statrs`, сэмплы — `rand_distr`). |

**Быстрый рецепт:** `statrs` даёт CDF/inverse-CDF «из коробки» — для
`normal_quantile(p, μ, σ)` это одна строка `Normal::new(μ,σ)?.inverse_cdf(p)`.
Единственная обвязка — arity/размерность/`EvalError`. Доменные функции
`cohort_ltv` (FR-027) уже используют интерполяцию retention-кривой —
`levenberg-marquardt` (MIT, L2→L3 через `nalgebra`) даст подгонку кривой по
точкам когорты, но это **отложено** до первого домена с оптимизацией
(архдок §9 M6-стиль «по потребности»); в M2 не тянем.

### 2.2. M3 — worker + double buffer (FR-064)

**Новых внешних зависимостей нет** (архдок §5.2: «P1 не добавляет зависимостей»).
Рецепт целиком из стандартной библиотеки и паттернов, уже доказанных в репо:

- `std::thread::spawn` + `std::sync::mpsc` — worker получает
  `(Arc<Canvas>, WhatIfOverrides)`, возвращает `Result<FlowSolutions, CycleError>`.
- Двойная буферизация результатов: UI-тред атомарно подменяет ссылку на снимок.
  Кандидат на `arc_swap::ArcSwap<FlowSolutions>` (Apache-2.0 OR MIT) — НО
  архдок §5.2 явно говорит «без новых зависимостей»; поэтому v1 —
  `Arc<RwLock<FlowSolutions>>` из `std` (писатель — воркер, читатель — рендер).
  `arc_swap` — опция v2, если lock-free чтение станет узким местом
  (бенчмарк решает).
- Wake-up UI-треда — **существующий паттерн** `EventLoopProxy<AppEvent>`
  (уже используется `McpPipeServer::spawn`, `ThumbService::spawn`,
  `WatchService::spawn`, `SearchService::spawn` — `canvas-app/src/main.rs`).
  Новый `AppEvent::FlowReady { solutions: Arc<FlowSolutions>, kind: FlowKind }`.
- **Фолбэк** (правило AGENTS «фолбэк + warn»): при отказе/таймауте воркера —
  синхронный `propagate_with_lines` на главном треде (архдок §5.2 последний
  абзац). Контракт live-инварианта сохраняется: правка → результат в пределах
  1–2 кадров.

**Рецепт FR-017 v2 (freeze/сравнение):** `WhatIfOverrides` уже умеет построчные
подмены (`flow.rs:235`). Freeze сценария = сохранить `FlowSolutions` активного
сценария как `Arc<FlowSolutions>`-снимок; сравнение = diff `outputs`/`lines`
между снимками. Сценарные сетки (FR-017 v1, CP6) уже работают синхронно;
M3 переносит пакет прогонов на воркер, когда их число перестаёт влезать в кадр.

### 2.3. M4 — поярусный параллелизм (FR-065)

| Крейт | Версия | Лицензия | Что берём | Рецепт адаптации |
|---|---|---|---|---|
| `std::thread::scope` | std | — | v1: `scope` на ярус топосортировки | Ярусы — готовая карта независимости (`flow::topo_sort` уже peel-ит Kahn по уровням, но flatten-ит). Рецепт: добавить `topo_levels() -> Vec<Vec<usize>>`, в `propagate_with_lines_data` заменить `for index in order` на `for level in &levels { scope(|| for index in level { eval node }) }`. Барьер между ярусами — по построению (зависимости). |
| `rayon` | 1.12 | MIT OR Apache-2.0 | v2: `par_iter` по узлам яруса | **Уже в дереве** (транзитивно через `cosmic-text`) — прямое включение **не добавляет новых лицензий** (архдок §5.3, §4.2). Переключение `scope`→`rayon` — по профилированию (когда спавн потоков на типичных ярусах в десятки узлов станет заметен). |

**Быстрый рецепт:** чистота функций (`(&Expr, &Env) -> Result<Value, EvalError>`
без I/O и глобального состояния — инварианты 1/2 FR-013) делает параллелизм
безопасным по построению: любые два узла одного яруса независимы. `Env`
`Clone` — каждый поток клонирует свой `Env` (уже поддержано). Единственный
контракт — **детерминизм §5.6.3**: параллельные агрегаты собираются в
фиксированном порядке (collect-then-reduce), `rayon::par_iter().collect::<Vec<_>>()`
затем sort+reduce — безопасно; `par_iter().reduce()` — **запрещён**
(недетерминированный порядок).

### 2.4. M5 — Monte Carlo + QMC (FR-066)

| Крейт | Версия | Лицензия | Что берём | Рецепт адаптации |
|---|---|---|---|---|
| `statrs` + `rand_distr` | (из M2) | — | распределения для входных параметров | Параметр ноды → `Distribution` (Normal/LogNormal/Exp/...); N прогонов графа с сэмплами. |
| `rand_chacha` | (из M2) | — | сидированный RNG | Один сид → одна выборка (воспроизводимость). Сид = `hash(model) ⊕ seed ⊕ run_idx`. |
| `sobol_burley` | 0.1 | MIT OR Apache-2.0 | Owen-scrambled Sobol — QMC-последовательности | Для чувствительности: при том же N прогонов QMC даёт меньшую дисперсию оценки квантилей, чем псевдослучайный Monte Carlo. `SobolSequence::next()` → сэмплы в [0,1) → inverse-CDF (`statrs`) → значения параметров. |

**Быстрый рецепт:** Monte Carlo — «embarrassingly parallel» (архдок §5.4): M
независимых исполнений графа с разными наборами параметров. Прогоны чанкуются
(напр. по 256 на задачу) — UI остаётся отзывчивым, результаты публикуются
прогрессом. Квантили P50/P90/P99 — collect-then-reduce: `Vec<FlowSolutions>`
→ для каждого `(node_id, output)` сортировка значений → `percentile` (уже
есть в `expr.rs` builtin). Результат — **синтетический `FlowSolutions`**,
который кормит `analyze::analyze` **без правок** (FR-016 — bottleneck overlay
на квантилях). Контракт детерминизма §5.6: версия движка в
`canvas.extra["canvasdesk"]["engine"]` (паттерн `whatif.rs`).

### 2.5. Отвергнутые рецепты (архдок §4.3 — не тянуть)

Внешние embed-движки (Lua/Python/PyO3/WASM-скрипты), C/C++ BLAS/LAPACK/nlopt,
GPU-compute, `polars` — всё отвергнуто ADR-0008. Изменение — только новым ADR.
Этот план не revisits эти решения.

---

## 3. Декомпозиция CR/FR

| FR | Этап | Заголовок | Статус | Зависимости |
|---|---|---|---|---|
| **FR-063** | M2/S1 | Статистический слой `expr/stats.rs` (распределения, квантили, ДИ, детерминированный сид) | выявлено (план) | M0/M1 (сделано) |
| **FR-064** | M3/S2 | Сценарный воркер: вынос пересчёта с главного треда (P1, double buffer) + FR-017 v2 (freeze/сравнение) | выявлено (план) | M0/M1; FR-017 v1 (сделано) |
| **FR-065** | M4/S3 | Поярусный параллелизм DAG (`topo_levels` → `thread::scope` → `rayon`), детерминизм | выявлено (план) | M0/M1; контракт с FR-064 (сигнатура `propagate_with_lines` стабильна) |
| **FR-066** | M5/S3 | Monte Carlo + QMC-движок (`sobol_burley`), квантили P50/P90/P99 как результаты, MCP | выявлено (план) | **FR-063** (распределения/RNG), **FR-065** (параллельные прогоны) |

Документы создаются по шаблону `docs/change-requests/cr-template.md`, на
русском (язык проекта), статус `выявлено (план)`. После гейта Go — перевод в
`в работе` по мере взятия.

---

## 4. Стратегия параллелизации (до 4 агентов)

Технические зависимости (архдок §9): M2 не зависит от M3/M4; M3 не зависит от
M2/M4 (трогает `scene.rs`, а не `flow.rs`-внутренности); M4 не зависит от M2/M3
(трогает `flow.rs`-внутренности, сигнатура стабильна); **M5 зависит от M2
(распределения/RNG) и M4 (параллельные прогоны)** — поэтому M5 во второй волне.

```
Волна S0 (сериал, 1 агент — Foundation) ────────────────────────────────────────┐
  S0: workspace.deps + features skeleton + DEPENDENCIES.md §3→§2 + notices        │
      (устраняет конфликт Cargo.toml между M2/M4/M5; один хозяин файла)            │
└────────────────────────────────────────────────────────────────────────────────┘
        │ (merge --no-ff, зелёный cargo deny check)
        ▼
Волна S1 (ПАРАЛЛЕЛЬНО, 4 агента) ──────────────────────────────────────────────┐
  Агент A — FR-063 (M2 stats)        Агент B — FR-064 (M3 worker)               │
  Агент C — FR-065 (M4 parallel)     Агент D — Test&Doc scaffold                 │
  (каждый на своих файлах; контракты §5 заморожены)                             │
└───────────────────────────────────────────────────────────────────────────────┘
        │ (4 merge --no-ff, каждый = зелёные гейты §7)
        ▼
Волна S2 (1 агент, после S1) ──────────────────────────────────────────────────┐
  Агент A (или новый) — FR-066 (M5 Monte Carlo + QMC)                            │
  (опирается на stats из FR-063 + parallel из FR-065)                            │
└───────────────────────────────────────────────────────────────────────────────┘
```

**Почему S0 сериален.** `crates/canvas-core/Cargo.toml` и корневой `Cargo.toml`
`[workspace.dependencies]` — общие для M2 (stats) и M4 (rayon) и M5 (sobol).
Параллельное редактирование одного файла = конфликты слияния. S0 один раз
прописывает все optional-deps за фичами `stats`/`parallel`/`qmc` и обновляет
реестр — после этого M2/M4/M5 только «включают» уже прописанные deps (правка
одной строки `[features]`, бесконфликтно). Это и есть «заранее спланированные
контракты на стыках», которые требует владелец.

**Почему 4 агента в S1, а не 3.** Четвёртый (Агент D — Test&Doc scaffold) не
трогает код ядра и работает на изолированных файлах (golden-test фикстуры,
`SPEC.md`, `user-docs/`, `skills/`), распараллеливая «обвязку качества»,
которая иначе была бы узким местом после слияния A/B/C. Это укладывается в
лимит «до 4 агентов».

---

## 5. Контракты на стыках (замороженные швы)

Контракт считается замороженным = изменение только новым ADR. Каждый агент S1
кодирует против этих швов; нарушение = конфликт слияния на ревью.

### 5.1. Сигнатура `flow::propagate_with_lines` — НЕИЗМЕННА всеми
```rust
pub fn propagate_with_lines(
    canvas: &Canvas,
    whatif: &WhatIfOverrides,
) -> Result<FlowSolutions, CycleError>          // flow.rs:394
```
- **M3** вызывает её из воркера — не меняет.
- **M4** меняет ТОЛЬКО внутреннюю реализацию (`topo_levels` + `par_iter`) —
  сигнатура стабильна.
- **M5** добавляет НОВУЮ sibling `flow::propagate_monte_carlo(...)` — не трогает
  `propagate_with_lines`.

### 5.2. `flow::topo_sort` — M4 добавляет НОВУЮ `topo_levels`, старую не трогает
```rust
pub fn topo_sort(canvas: &Canvas) -> Result<Vec<usize>, CycleError>              // flow.rs:78 — СТАБИЛЬНА
pub fn topo_levels(canvas: &Canvas) -> Result<Vec<Vec<usize>>, CycleError>       // M4 — НОВАЯ
```
M3/M5 не видят изменения. `propagate_with_lines_data` внутри переключается на
`topo_levels` + flatten/parallel — но это внутренность M4.

### 5.3. `expr::eval_call` dispatch — M2 добавляет arm за фичей, существующие не трогает
```rust
// expr.rs:1730 — существующие arms СТАБИЛЬНЫ
#[cfg(feature = "stats")]
"normal_quantile" | "normal_cdf" | "lognormal_quantile" | "poisson_pmf" | …
    => stats::dispatch(func, &values),
```
`stats::dispatch` зеркалит `queueing::dispatch` (`pub(super) fn dispatch(func,
&[Value]) -> Result<Value, EvalError>`). M5 добавляет свой arm для MC-функций
за фичей `qmc` — после M2 (волна S2), конфликта нет.

### 5.4. `SceneState::recompute_flow` — M3 единственный редактор
Контракт: метод по-прежнему производит `flow_baseline` + `flow_active` +
`expr_results` + `analysis` + `expr_line_results` + `param_spills` +
`auto_rows` + `unmapped_edges` + `bundles` (выводка O(N), остаётся на UI-треде).
Только тяжёлый вызов `propagate_with_lines` уезжает на воркер; результаты —
через `Arc<RwLock<FlowSolutions>>` (v1) с sync-фолбэком. M4/M5 `scene.rs`
**не трогают**.

### 5.5. `FlowSolutions` — стабильна; M5 производит синтетический снимок
```rust
pub struct FlowSolutions { pub outputs, pub lines, pub named, pub warnings }   // flow.rs:353
```
M5 прогоняет N раз `propagate_with_lines`, собирает `Vec<FlowSolutions>`,
считает квантили collect-then-reduce, строит **синтетический `FlowSolutions`**
(P50/P90/P99 в `outputs`/`named`) → кормит `analyze::analyze` **без правок**
(FR-016 overlay на квантилях).

### 5.6. Feature-флаги — S0 прописывает скелет, S1/S2 включают
```toml
# crates/canvas-core/Cargo.toml [features] — после S0
default = []
stats    = ["dep:statrs", "dep:rand", "dep:rand_chacha", "dep:rand_distr"]
parallel = ["dep:rayon"]                       # НЕ подразумевает stats (независимо)
qmc      = ["stats", "parallel", "dep:sobol_burley"]   # M5: подразумевает оба
```
S0 добавляет все optional-deps в `[workspace.dependencies]` и `canvas-core` как
`optional = true` (за фичами), но **фичи остаются пустыми gates**, пока
конкретный агент не начнёт наполнять. `cargo build --no-default-features` =
zero new deps (инвариант чистого дерева для B2B-сборки).

### 5.7. Детерминизм (архдок §5.6) — контракт уровня продукта, для всех
1. Фиксированный порядок обхода рёбер (`canvas.edges` — инвариант слотов `$1..$N`).
2. Сидированная случайность (M5): `seed = hash(canvas.content) ⊕ scenario_seed ⊕ run_idx`.
3. **Никаких неблокирующих float-редукций** (M4/M5): collect-then-reduce в
   фиксированном порядке; `par_iter().reduce()` запрещён.
4. Версия движка в `canvas.extra["canvasdesk"]["engine"]` (M5; паттерн
   `whatif.rs` — raw JSON, без typed-struct на `Canvas`).
5. Числа эталонов ADR-0005/0006 — golden-тесты на ОБОИХ путях (однопоточный +
   параллельный) в CI (Агент D scaffold).

### 5.8. WASM-совместимость — `parallel`/воркер desktop-only
`std::thread` и `rayon` **не работают** на `wasm32-wasip1` (скрипты
`wasm_gate.sh`/`mcp_wasm_gate.sh` с `RUST_TEST_THREADS=1`). Контракт:
- M3 воркер — `#[cfg(not(target_arch = "wasm32"))]` + sync-фолбэк на wasm.
- M4 `parallel` — `#[cfg(not(target_arch = "wasm32"))]` + flatten-на-wasm.
- M5 `qmc` — не собирается на wasm (feature-gate); core `propagate_with_lines`
  остаётся wasm-чистым.
- Гейты `scripts/wasm_gate.sh --check` и `scripts/mcp_wasm_gate.sh --check`
  **обязаны быть зелёными** на каждом коммите S1/S2.

---

## 6. Ограничения для агентов (по агентам)

Общие (из `AGENTS.md`): TDD (тесты primero), `thiserror`/`anyhow`, никаких
`unwrap`/`expect` в production-путях, `unsafe` только в `canvas-shell`/
`canvas-widgets` с SAFETY, комментарии/доки на русском, conventional commits
(задача = сессия = коммит), новые deps — с обоснованием. **Обязательный вопрос
по завершении FR:** нужна ли доработка онбординга (`onboarding_ui.rs`) и
пользовательской доки (`user-docs/`).

### Агент A — FR-063 (M2, stats)
- **Файлы (владение):** `crates/canvas-core/src/expr/stats.rs` (новый),
  `crates/canvas-core/src/expr.rs` (`mod stats;` + dispatch arm + `FN_HINTS`),
  `crates/canvas-core/tests/stats_smoke.rs` (новый). `Cargo.toml` — только
  активация `stats` feature (deps уже прописаны S0).
- **Не трогать:** `flow.rs`, `scene.rs`, `canvas-mcp`, `templates.rs`,
  `expr/queueing.rs` (только читать как образец паттерна).
- **Ограничения:** все функции — чистые `(&[Value]) -> Result<Value, EvalError>`;
  размерность через `pub(crate) Unit::dims()/scale()`; `thread_rng()` запрещён;
  сид только `ChaCha8Rng::seed_from_u64`. `FN_HINTS` parity-тест с `eval_call`
  (как FR-021) — обязателен.

### Агент B — FR-064 (M3, worker)
- **Файлы (владение):** `crates/canvas-scene/src/scene.rs` (recompute_flow),
  `crates/canvas-app/src/main.rs` (`AppEvent::FlowReady` + spawn воркера),
  `crates/canvas-app/src/app.rs` (read-path рендера),
  `crates/canvas-core/src/whatif.rs` (`ScenarioGrid` для FR-017 v2, если нужно),
  `crates/canvas-scene/tests/worker_smoke.rs` (новый).
- **Не трогать:** `flow.rs` (только вызывает `propagate_with_lines`),
  `expr.rs`, `expr/stats.rs`, `canvas-mcp` (инструменты whatif_* уже есть).
- **Ограничения:** воркер `cfg(not(target_arch="wasm32"))`; sync-фолбэк на wasm
  + `warn`; `Arc<RwLock<FlowSolutions>>` v1 (без `arc_swap` —archdoc §5.2
  «без новых зависимостей»); wake через `EventLoopProxy<AppEvent>` (существующий
  паттерн); live-инвариант (правка → результат ≤ 1–2 кадра); деградация =
  sync-пересчёт + warn.

### Агент C — FR-065 (M4, parallel)
- **Файлы (владение):** `crates/canvas-core/src/flow.rs` (`topo_levels` +
  parallel-цикл в `propagate_with_lines_data`), `crates/canvas-core/tests/
  parallel_determinism.rs` (новый). `Cargo.toml` — только активация `parallel`.
- **Не трогать:** `scene.rs`, `expr.rs`, `canvas-mcp`, `main.rs`. Сигнатуру
  `propagate_with_lines`/`topo_sort` — НЕ менять (контракт §5.1/§5.2).
- **Ограничения:** v1 `std::thread::scope`, v2 `rayon` (по профилированию);
  `cfg(not(target_arch="wasm32"))` + flatten-на-wasm; **collect-then-reduce**
  (никакого `par_iter().reduce()`); golden-тесты эталонов ADR-0005/0006
  побитово идентичны однопоточному пути (контракт §5.7); ускорение ≥ 2× на
  тяжёлом графе при ≥ 4 ядрах (критерий архдока §9 M4).

### Агент D — Test & Doc scaffold (параллелен A/B/C)
- **Файлы (владение):** `crates/canvas-core/tests/determinism_harness.rs`
  (новый — generic golden-тест «однопоточный vs параллельный путь»),
  `docs/SPEC.md` (§MCP/§6.3 — бюджеты, §расчётный стек), `user-docs/
  calculations.md` (раздел «Вероятностные оценки» — заглушка под M2/M5),
  `skills/` (если меняется состав MCP-инструментов — M5 only, волна S2).
- **Не трогать:** код ядра (`expr.rs`, `flow.rs`, `scene.rs`), `Cargo.toml`.
- **Ограничения:** scaffold-тесты пишутся с `#[ignore]` до готовности фич
  (A/B/C включают по мере merge); `user-docs` — только относительные `*.html`
  ссылки (линк-чек в тестах `docs_ui` валит CI); RU/EN i18n таблицы (FR-040).

### Агент (S2) — FR-066 (M5, Monte Carlo)
- **Файлы (владение):** `crates/canvas-core/src/expr/mc.rs` (новый),
  `crates/canvas-core/src/flow.rs` (`propagate_monte_carlo` — НОВАЯ, не трогает
  `propagate_with_lines`), `crates/canvas-scene/src/mcp.rs` (инструмент
  `monte_carlo_run`), `canvas-mcp/src/lib.rs` (ToolSpec + TOOLS),
  `crates/canvas-core/src/analyze.rs` (только чтение — кормить квантилями),
  `canvas-core/Cargo.toml` (активация `qmc`), `crates/canvas-core/tests/mc_smoke.rs`.
- **Зависимости:** FR-063 (stats) И FR-065 (parallel) — оба слиты.
- **Ограничения:** сидированный RNG (§5.7.2); collect-then-reduce квантили;
  синтетический `FlowSolutions` → `analyze::analyze` без правок; версия движка
  в `canvas.extra["canvasdesk"]["engine"]`; прогоны чанкуются (256/задача),
  прогресс публикуется; воспроизводимость — фиксированный seed → идентичные
  квантили; 10⁴ прогонов на эталоне №5 < 1 с на 4 ядрах (критерий архдока §9 M5).

---

## 7. Проверяемые коммиты (требование владельца)

**Правило:** каждый коммит = зелёный гейт. Промежуточных «сломанных» состояний
нет — feature-flags + TDD гарантируют, что `default`-сборка всегда зелёная, а
новая функциональность включается фичей и покрывается тестами в том же коммите.

Базовые гейты (все коммиты): `cargo fmt --check`, `cargo clippy --workspace
-D warnings`, `cargo test --workspace`, `scripts/wasm_gate.sh --check`,
`scripts/mcp_wasm_gate.sh --check`, `cargo deny check`.

### S0 — Foundation (1 коммит)
- `feat(core): add stats/parallel/qmc optional deps behind feature gates (ADR-0008 S0)`
- Проверка: `cargo build --no-default-features` (zero new deps);
  `cargo build --features stats`; `cargo build --features parallel`;
  `cargo build --features qmc`; `cargo deny check` зелёный; `THIRD-PARTY-NOTICES`
  перегенерирован; `docs/DEPENDENCIES.md` §3→§2 (промоутнутые кандидаты).

### S1 — параллельная волна (4 независимые коммита, каждый merge --no-ff)

**FR-063 (M2):** разбит на фазы-коммиты (каждый зелёный):
1. `feat(core/stats): skeleton — stats::dispatch + mod stats + cfg gate (FR-063 P1)`
   — пустой `dispatch` (возвращает `UnknownFunction`), `mod stats` в expr.rs,
   parity-тест `FN_HINTS`. Гейт: `cargo test -p canvas-core --features stats`.
2. `feat(core/stats): normal/lognormal/exp/poisson distributions + quantiles (FR-063 P2)`
   — обёртки над `statrs`, golden-тесты распределений (CDF/inverse-CDF против
   табличных значений).
3. `feat(core/stats): deterministic ChaCha8 RNG + confidence intervals (FR-063 P3)`
   — `rand_chacha` сидирование, ДИ, тест воспроизводимости (один сид → одна выборка).

**FR-064 (M3):** ✅ выполнено (2026-09-24, ветка `feature/fr-064-scenario-worker`,
обе фазы зелёными гейтами: fmt/clippy/test --workspace/wasm_gate/mcp_wasm_gate/deny)
1. ✅ `feat(scene): FlowWorker spawn + AppEvent::FlowReady, sync fallback (FR-064 P1)`
   — воркер + double buffer, `cfg(not(wasm32))`, sync-фолбэк, smoke-тест
   «правка → результат через воркер».
2. ✅ `feat(scene): FR-017 v2 — scenario freeze + comparison table (FR-064 P2)`
   — freeze-снимки, diff `outputs`/`lines`, таблица сравнения;
   e2e в духе эталона №2 (смена `rps` → дельты downstream). `ScenarioGrid`
   не потребовался (лимит 3 сценариев FR-017 покрывает пакет; матрица — при
   появлении потребности в >3 прогонах, FR-017 v3/FR-066).

**FR-065 (M4):**
1. `feat(core/flow): topo_levels() — tiered topo sort (FR-065 P1)`
   — НОВАЯ функция, `topo_sort` стабилен; flatten-эквивалентность
   (`topo_levels().flatten() == topo_sort()`) — тест-инвариант.
2. `feat(core/flow): std::thread::scope per-level parallel eval (FR-065 P2)`
   — `cfg(not(wasm32))`, flatten-на-wasm; golden-тесты эталонов **побитово
   идентичны** однопоточному пути (контракт §5.7).
3. `feat(core/flow): rayon par_iter switch (FR-065 P3)` — по профилированию;
   ускорение ≥ 2× на тяжёлом графе ≥ 4 ядра (бенчмарк-гейт).

**Агент D (Test&Doc):**
1. `test(core): determinism harness — single vs parallel path golden (FR-065 support)`
   — generic golden-тест с `#[ignore]` до готовности M4.
2. `docs: SPEC §6.3 budgets + calculations.md probabilistic stub (ADR-0008 wave S)`

### S2 — Monte Carlo (после S1 merge)
1. `feat(core/mc): propagate_monte_carlo + seeded RNG + chunked runs (FR-066 P1)`
2. `feat(core/mc): sobol_burley QMC + P50/P90/P99 quantile results (FR-066 P2)`
3. `feat(mcp): monte_carlo_run tool + analyze integration (FR-066 P3)` —
   синтетический `FlowSolutions` → `analyze`; e2e: 10⁴ прогонов эталона №5 <
   1 с/4 ядра, фиксированный seed → идентичные квантили.

---

## 8. Граф зависимостей и последовательность

```
M0/M1 (сделано, CP0) ──┬──> S0 (Foundation) ──┬──> FR-063 (M2) ──┐
                       │                       ├──> FR-064 (M3) ──┤
                       │                       ├──> FR-065 (M4) ──┼──> FR-066 (M5, S2)
                       │                       └──> Test&Doc (D) ──┘
                       │
FR-029/032/033 (A, сделано) ──> FR-016/017 v1 (B, сделано) ──> гейт Go ──> волна S
```

**Сцепление (почему порядок нельзя нарушить):**
1. **S0 перед S1** — Cargo.toml-конфликт (один хозяин файла).
2. **FR-066 после FR-063 + FR-065** — M5 использует распределения/RNG из M2 и
   параллельные прогоны из M4. Без них — sequential Monte Carlo (медленно) и
   собственные обёртки `statrs` (дубликат M2).
3. **FR-063/064/065 параллельны** — контракты §5.1–§5.5 разделяют файлы и
   сигнатуры; единственный общий файл (`Cargo.toml`) — за S0.
4. **Волна S после гейта Go** — единственный потребитель M2–M5 (тяжёлые
   сценарные режимы) отсутствует до подтверждения спроса (эталоны ≤ 45 нод,
   аналитика, полный пересчёт < 10 мс — архдок §2.3, §5.3).

---

## 9. Риски и смягчения

| # | Риск | Смягчение |
|---|---|---|
| R-S1 | `propagate_with_lines_data` (flow.rs:416 `for index in order`) — M4 меняет цикл, M3 вызывает функцию. Конфликт логики (не файлов). | Контракт §5.1: сигнатура стабильна; M4 меняет ТОЛЬКО внутренности, M3 НЕ трогает flow.rs. Ревью проверяет, что M3-воркер вызывает `propagate_with_lines` (не внутреннюю `*_data`). |
| R-S2 | `Env` `Clone` но не `Sync`; M4 `par_iter` клонирует `Env` на поток — budget risk при 1000 нод × N потоков. | `Env` уже `Clone` (expr.rs:486); clone O(переменные+params+inbound) — дёшево относительно eval. M4 профилирует; если тяжело — `Arc<Env>` (Env immutable в пределах eval). |
| R-S3 | Недетерминизм float-агрегатов в M4/M5. | Контракт §5.7.3: collect-then-reduce в фиксированном порядке (sort node ids перед редукцией); golden-тесты на обоих путях (Агент D). `par_iter().reduce()` запрещён. |
| R-S4 | `rand`/`statrs` мажорные обновления меняют выборку → невоспроизводимость между версиями движка. | Версия движка в `canvas.extra["canvasdesk"]["engine"]` (§5.7.4); версии фиксированы в `[workspace.dependencies]` + `Cargo.lock` (`--locked`); R3 архдока §10. |
| R-S5 | WASM-гейт падает: `std::thread`/`rayon` не работают на wasip1. | Контракт §5.8: `cfg(not(target_arch="wasm32"))` + sync-фолбэк; гейты `wasm_gate.sh --check`/`mcp_wasm_gate.sh --check` на каждом коммите. |
| R-S6 | `cargo-deny` валят CI при транзитивном copyleft от нового кандидата. | S0 проверяет `cargo deny check` ДО merge; `Cargo.lock` фиксирует версии; R2 архдока §10. |
| R-S7 | M3 воркер падает/зависает → UI завис. | Sync-фолбэк + `warn` (AGENTS правило); таймаут воркера → sync-пересчёт; live-инвариант сохранён. |
| R-S8 | M5 Monte Carlo 10⁴ прогонов × 1000 нод = память/время. | Чанкование (256 прогонов/задача), прогресс-публикация; квантили collect-then-reduce (не хранить все N `FlowSolutions` — только per-node `Vec<Value>`); бюджет архдока §9 M5 (< 1 с/4 ядра на эталоне №5). |

---

## 10. Наглядная проверка (правило роадмапа §9)

Каждый FR содержит секцию «Наглядная проверка (5 минут)» — демо для владельца,
наблюдаемое без чтения кода. Автотесты в CI дублируют те же сценарии.

| FR | Демо (5 минут) | Автотест (CI-гейт) |
|---|---|---|
| FR-063 | На эталоне №1: `normal_quantile(0.95, utilization, 0.1)` в ноде → P95 утилизации виден на канвасе; смена сида → та же выборка (воспроизводимость) | golden-тесты распределений ±1e-9; FN_HINTS parity; сид-воспроизводимость |
| FR-064 | Эталон №2: 20 прогонов сценарной сетки на воркере → UI не тормозит; freeze 2 сценариев → таблица сравнения; убийство воркера → sync-фолбэк + warn | worker smoke; freeze/diff e2e; fallback-тест |
| FR-065 | Тяжёлый граф (1000 нод): пересчёт на 4 ядрах ≥ 2× быстрее однопоточного; числа эталонов ADR-0006 **идентичны** обоим путям | `topo_levels().flatten() == topo_sort()`; determinism golden; бенчмарк ≥ 2× |
| FR-066 | Эталон №5: Monte Carlo 10⁴ прогонов → P50/P90/P99 видны как результаты нод; bottleneck overlay на P90; фиксированный seed → идентичные квантили при повторе | MC e2e < 1 с/4 ядра; seed-воспроизводимость; analyze на синтетическом снимке |

---

## 11. Источники истины

- `docs/adr/adr-0008-math-computing-stack.md` — решение (вариант B, M0–M6).
- `docs/architecture/math-computing-stack.md` — Архдок: §2 текущее состояние,
  §3 слои L0–L5, §4 реестр кандидатов, §5 параллелизм (P1/P2/P3), §5.6
  детерминизм, §7 лицензии, §8 реестр РФ, §9 дорожная карта M0–M6, §10 риски.
- `docs/plans/product-roadmap.md` — волны 0/A/B/V/S, гейт Go (§4.4),
  привязка M0–M6 (§5), критический путь CP0–CP7 (§9).
- `docs/change-requests/cr-013-agent-architecture-composition.md` — R1–R5,
  G1–G8 (волна A выполнена).
- `docs/change-requests/fr-063-stats-layer.md`, `fr-064-scenario-worker.md`,
  `fr-065-tiered-parallelism.md`, `fr-066-monte-carlo-qmc.md` — FR этого плана.
- `docs/DEPENDENCIES.md` — реестр зависимостей (§3 кандидаты → §2 после S0).
- `crates/canvas-core/Cargo.toml` — feature-флаги `stats`/`parallel` (CP0).
- `deny.toml`, `about.toml`, `THIRD-PARTY-NOTICES.md` — лицензионный контроль.
- `AGENTS.md` — правила агента (TDD, ошибки, unsafe, коммиты, онбординг/дока).

## История изменений

- `2026-09-24` — агент: создан план по запросу владельца («спланировать
  разработку на базе ADR-0008 — расписать CR/FR, параллельно до 4 агентов,
  контракты на стыках, проверяемые коммиты, анализ библиотек»). Декомпозиция
  M2–M5 → FR-063..066; волна S0 (Foundation) + S1 (4 агента параллельно:
  M2/M3/M4 + Test&Doc) + S2 (M5); замороженные контракты §5; проверяемые
  коммиты §7; анализ библиотек §2; риски R-S1..R-S8. M6 (LA) — бэклог «по
  потребности». Статус «предложено» — исполнение после гейта Go.
