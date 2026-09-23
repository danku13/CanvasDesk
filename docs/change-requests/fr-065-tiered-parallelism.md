# FR-065: Поярусный параллелизм пересчёта DAG (topo_levels + thread::scope + rayon)

- **Статус:** выявлено (план)
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (планирование); решения — владелец проекта
- **Источник:** план реализации ADR-0008 волна S (`docs/plans/adr-0008-wave-s-plan.md`), по запросу владельца 2026-09-24: спланировать разработку M2–M5 и расписать CR/FR для параллельной работы до 4 агентов. FR-065 покрывает этап **M4** (S3) — поярусный параллелизм пересчёта DAG (P2 в классификации архдока).
- **Связанные задачи:** ADR-0008 (`docs/adr/adr-0008-math-computing-stack.md`); `docs/architecture/math-computing-stack.md` §5.1 (чистота функций → безопасный параллелизм), §5.3 (P2 — поярусный параллелизм), §5.6 (детерминизм), §9 (дорожная карта M4/S3); `docs/plans/product-roadmap.md` §4.5 (волна S, S3); FR-013 (Numi-движок — чистые функции, инварианты 1/2 `(&Expr, &Env) → Result<Value, EvalError>` без I/O и глобального состояния), FR-014 (propagator/DAG, `topo_sort`), FR-064 (M3 воркер — **контракт на стабильность сигнатуры `propagate_with_lines`**), FR-066 (M5 Monte Carlo — потребитель параллельных прогонов); `docs/DEPENDENCIES.md` §3 (реестр кандидатов: `rayon` уже транзитивно в дереве через `cosmic-text`), §2 (прямые прод-зависимости — миграция после активации).
- **Создан:** 2026-09-24
- **Обновлён:** 2026-09-24
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

**Что добавляется.** Поярусный параллелизм пересчёта DAG в `canvas-core/src/flow.rs`:
ярусы топосортировки — готовая карта независимости (любые два узла одного яруса не
связаны value-рёбрами по построению, Kahn-фронтир), значит могут вычисляться
параллельно без блокировок. Реализация в две ступени:

- **v1 — `std::thread::scope` на ярус.** Цикл `for index in order` (`flow.rs:416`)
  в `propagate_with_lines_data` заменяется на обход по ярусам:
  `scope(|s| for index in level { s.spawn(|| eval node) })`, барьер между ярусами —
  по построению. Каждый поток клонирует свой `Env` (`expr.rs:486`, `Clone`).
- **v2 — `rayon` `par_iter` по узлам яруса.** По профилированию: когда спавн
  `scope`-потоков на ярусах в десятки узлов становится заметен, `scope` заменяется
  на `level.par_iter().map(...).collect::<Vec<_>>()`. `rayon` уже в `Cargo.lock`
  транзитивно (через `cosmic-text`, `Cargo.lock:2069`); прямое включение **не добавляет
  новых лицензий** (MIT OR Apache-2.0; архдок §5.3, §4.2).

**Зачем.** Выигрыш на тяжёлых режимах — сценарные пакеты (FR-017 v2: сетки 20+
прогонов) и Monte Carlo (FR-066, 10⁴×1000 нод). На одиночном reval выигрыш
отсутствует: эталоны ADR-0005/0006 ≤45 нод уже <10 мс (SPEC §6.3). M4 включается
только за фичей `parallel` и только на desktop (`cfg(not(target_arch="wasm32"))`);
на wasm остаётся однопоточный flatten-путь.

**Границы v1 FR-065:**

- Новая `topo_levels(canvas) -> Result<Vec<Vec<usize>>, CycleError>`; `topo_sort`
  **стабильна** (контракт плана §5.2).
- Параллельный цикл — за `#[cfg(all(feature="parallel", not(target_arch="wasm32")))]`;
  else — текущий flatten-путь.
- Фича `parallel = ["dep:rayon"]` — **НЕ подразумевает** `stats` (план §5.6). S0
  добавляет `rayon` в `[workspace.dependencies]` + `canvas-core` как optional-dep.
- Сигнатуры `propagate_with_lines`/`propagate_with_lines_data`/`topo_sort` — **НЕ
  меняются** (контракт §5.1/§5.2). M3-воркер FR-064 и M5-Monte Carlo FR-066 кодируют
  против стабильного API.
- НЕ трогать: `scene.rs`, `expr.rs`, `canvas-mcp/main.rs`. Вся работа — внутри
  `flow.rs` + активация фичи в `Cargo.toml`.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `flow` (`canvas-core/src/flow.rs`) | НОВАЯ `pub fn topo_levels() -> Result<Vec<Vec<usize>>, CycleError>`; refactor цикла `for index in order` (`flow.rs:416`) в `propagate_with_lines_data` (`flow.rs:405`) на поярусный обход за `#[cfg(all(feature="parallel", not(target_arch="wasm32")))]` + flatten-else | `docs/architecture/math-computing-stack.md` §5.3 (P2 поярусный), §5.6 (детерминизм) |
| `Cargo` (`canvas-core/Cargo.toml`) | Активация фичи `parallel = ["dep:rayon"]` (CP0-скелет `parallel = []` уже на месте — `Cargo.toml:34`); S0 добавляет `rayon` в `[workspace.dependencies]` + `canvas-core` как optional-dep | `docs/DEPENDENCIES.md` §3→§2 (миграция `rayon` из кандидатов в прямые прод-зависимости) |
| `wasm` (target `wasm32-wasip1`) | Фича `parallel` — desktop-only; на wasm цикл остаётся однопоточным (flatten-фолбэк), `rayon`/`std::thread` не инстанцируются | `docs/architecture/math-computing-stack.md` §5.8; `scripts/wasm_gate.sh`, `scripts/mcp_wasm_gate.sh` |
| Тесты | determinism golden на ОБОИХ путях (однопоточный + параллельный): числа эталонов ADR-0005/0006 побитово идентичны; flatten-эквивалентность `topo_levels().flatten() == topo_sort()`; бенчмарк ≥2× на тяжёлом графе ≥4 ядра | `crates/canvas-core/tests/parallel_determinism.rs` (новый); `crates/canvas-scene/src/tests.rs:1480,1759,2152` (golden) |
| Документация | `DEPENDENCIES.md` §3 → §2 (rayon — прямая зависимость после активации); архдок §5.3/§5.6 — реализация P2; SPEC §6.3 (бюджеты — комментарий о параллельном пути) | `docs/DEPENDENCIES.md`, `docs/SPEC.md` §6.3, `docs/architecture/math-computing-stack.md` §5.3/§5.6 |

## Анализ (Root Cause)

Слой расчётов после FR-013/014/017/025/029/045/050: Numi-лист заметки →
построчные результаты (`eval_lines_in`, `expr.rs:1258`) → propagator по DAG
(`propagate_with_lines_data`, `flow.rs:405`) → `FlowSolutions` (`flow.rs:353`). Все
функции вычисления чистые (архдок §5.1, инварианты 1/2 FR-013): `eval(expr: &Expr,
env: &Env) -> Result<Value, EvalError>` (`expr.rs:1384`) — без I/O и мутации графа.
Это и есть предпосылка M4: узлы одного яруса независимы по построению.

Чего нет сегодня (аудит репо 2026-09-24):

- **`topo_sort` возвращает ПЛОСКИЙ `Vec<usize>`.** `pub fn topo_sort(canvas: &Canvas)
  -> Result<Vec<usize>, CycleError>` (`flow.rs:78`) — Kahn с очередью по возрастанию
  индексов (детерминизм, `flow.rs:109–122`). Внутри уже peel-ит по уровням (фронтир
  Кана), но сразу flatten-ит в `order: Vec<usize>` (`flow.rs:111–113`). **Ярусы
  наружу не отдаются** — M4 извлекает их в новую `topo_levels()`.
- **Цикл пересчёта однопоточный.** `propagate_with_lines_data` (`flow.rs:405`) —
  реальный entry (тонкие обёртки `propagate`/`propagate_with_lines` — `flow.rs:394`).
  Цикл `for index in order` (`flow.rs:416`) последовательно обходит ноды в топо-порядке.
  Это место M4 параллелит: `for level in &levels { for index in level { … } }` →
  `scope`/`par_iter`.
- **`Env` — `Clone`, но не `Sync`.** `Env { vars, params, inbound, qualified }`
  (`expr.rs:486`), `#[derive(Debug, Clone, Default, PartialEq)]` (`expr.rs:485`).
  clone O(переменные + params + inbound) — дёшево относительно `eval` (риск R-S2
  плана: если тяжело — `Arc<Env>` v2, `Env` immutable в пределах eval).
- **Фича `parallel` — пустой гейт.** `canvas-core/Cargo.toml:34` — `parallel = []`
  (CP0/архдок M1). S0 добавляет optional-dep `rayon`; FR-065 только активирует
  `parallel = ["dep:rayon"]` (НЕ подразумевает `stats` — фичи независимы).
- **`rayon` уже в дереве.** `Cargo.lock:2069` (`name = "rayon"`) — транзитивно через
  `cosmic-text` (`Cargo.lock:541`); продуктовый код его не использует. Прямое
  включение **НЕ добавляет новых лицензий** (MIT OR Apache-2.0; архдок §5.3, §4.2;
  `docs/DEPENDENCIES.md` §3 — `rayon` уже в реестре кандидатов с пометкой «уже
  транзитивно в дереве через `cosmic-text`»).
- **Цикл-детект — итеративный Тарьян.** `cycle_participants` (`flow.rs:132`), без
  рекурсии — защита stack overflow. `CycleError { nodes: Vec<String> }` (`flow.rs:65`),
  `FlowKind { Control, Value }` (`flow.rs:36`). M4 не трогает — детект циклов
  выполняется до запуска параллельного пересчёта.
- **Golden-тесты эталонов — побитово стабильны.** В `crates/canvas-scene/src/tests.rs`:
  `mcp_fr029_instagram_mvp_reference` (`tests.rs:1480`, oracle `close_1pct`
  `tests.rs:1470`); `graph_apply_assembles_mini_reference_with_oracle` (`tests.rs:1759`,
  ADR-0006 №1: peak_rps≈208.33, CDN W≈34.29ms ρ0.417); `analyze_bottlenecks_reference_and_growth`
  (`tests.rs:2152`, DAU×2→Warn ρ0.833, DAU×5.35→Overload ρ2.23). M4 должен держать
  эти числа **побитово идентичными** на параллельном пути (контракт плана §5.7).

## Требуемые изменения (Changes)

### Что → где → как

| Что | Где | Как |
|---|---|---|
| НОВАЯ `topo_levels()` | `flow.rs` (рядом с `topo_sort:78`) | `pub fn topo_levels(canvas) -> Result<Vec<Vec<usize>>, CycleError>` — Kahn с drain-фронтиром в `sub-vec` на каждой итерации `while let Some(i) = queue.pop_front()`; push в `levels: Vec<Vec<usize>>`. Детерминизм — тот же (очередь по возрастанию индексов). Цикл-детект — переиспользовать `cycle_participants` (`flow.rs:132`). `topo_sort` стабилен; опционально — внутренне реализовать через `topo_levels().flatten()` (без изменения сигнатуры). |
| Инвариант-тест flatten-эквивалентности | `canvas-core/tests/parallel_determinism.rs` (новый) | `topo_levels(canvas)?.into_iter().flatten().collect::<Vec<_>>() == topo_sort(canvas)?` — на эталонах ADR-0005/0006 + случайных графах (10–1000 нод). |
| Refactor цикла `propagate_with_lines_data` | `flow.rs:416` (`for index in order`) | За `#[cfg(all(feature="parallel", not(target_arch="wasm32")))]`: `for level in &topo_levels(canvas)? { std::thread::scope(\|s\| for index in level { s.spawn(\|\| eval node) }) }` (P2). Else-ветка — текущий flatten-путь. `Env` clone на поток (`expr.rs:486`). Барьер между ярусами — по построению. **collect-then-reduce** — никаких `par_iter().reduce()`. |
| P3 rayon switch | `flow.rs` (внутри того же cfg-блока) | По профилированию P2: `level.par_iter().map(\|&i\| …).collect::<Vec<_>>()`, sort node ids перед редукцией (контракт §5.7.3). `par_iter().reduce()` — **запрещён**. |
| Активация фичи `parallel` | `canvas-core/Cargo.toml:34` | S0 (prep-коммит) добавляет `rayon` в `[workspace.dependencies]` + `canvas-core` как `optional = true`; FR-065 меняет `parallel = []` → `parallel = ["dep:rayon"]`. `stats` независим (план §5.6). |
| Determinism golden на обоих путях | `tests/parallel_determinism.rs`, `canvas-scene/src/tests.rs` | Те же `close_1pct`-оракулы (`tests.rs:1470`) и числа ADR-0006 — на default и `--features parallel` сборках. |
| Документация | `docs/DEPENDENCIES.md` (§3→§2), архдок §5.3/§5.6, `docs/SPEC.md` §6.3 | После merge P3; `rayon` — прямая прод-зависимость (раньше — кандидат). |

## Контракты на стыках

Контракты из плана `adr-0008-wave-s-plan.md` §5 — замороженные швы между агентами
S1–S3. Нарушение = конфликт слияния на ревью.

- **§5.1 — сигнатура `propagate_with_lines` СТАБИЛЬНА.** M4 меняет ТОЛЬКО внутренности
  `propagate_with_lines_data` (`flow.rs:405`); сигнатура `propagate_with_lines` (`flow.rs:394`)
  — без изменений. M3-воркер (FR-064) кодирует против неё; M5 (FR-066) добавляет sibling
  `propagate_monte_carlo`, не трогая `propagate_with_lines`.
- **§5.2 — `topo_sort` стабильна, `topo_levels` НОВАЯ.** `topo_sort` (`flow.rs:78`) без
  изменений; `topo_levels` — НОВАЯ. M3/M5 не видят изменения. Тест-инвариант:
  `topo_levels(canvas)?.into_iter().flatten().collect::<Vec<_>>() == topo_sort(canvas)?`.
- **§5.6 — фича `parallel` НЕ подразумевает `stats`.** Фичи независимы; `cargo build
  --no-default-features` = zero new deps (B2B-инвариант чистого дерева). S0 прописывает
  скелет; FR-065 активирует `parallel = ["dep:rayon"]`.
- **§5.7.3 — collect-then-reduce.** Параллельные агрегаты собираются в фиксированном
  порядке (sort node ids перед редукцией); `par_iter().collect::<Vec<_>>()` → sort →
  reduce — безопасно; **`par_iter().reduce()` — ЗАПРЕЩЁН** (недетерминированный порядок
  float-операций, архдок §5.6).
- **§5.8 — `cfg(not(target_arch="wasm32"))` + flatten-фолбэк.** `std::thread`/`rayon` не
  работают на `wasm32-wasip1` (`RUST_TEST_THREADS=1`). M4 `parallel` — `#[cfg(not(
  target_arch="wasm32"))]` + flatten-на-wasm; гейты `scripts/wasm_gate.sh --check` и
  `scripts/mcp_wasm_gate.sh --check` обязаны быть зелёными на каждом коммите S3.

**Подчеркнуть (границы владений):** НЕ трогать `scene.rs`, `expr.rs`,
`canvas-mcp/main.rs`. Сигнатуры `propagate_with_lines`/`propagate_with_lines_data`/
`topo_sort` — НЕ менять (контракт §5.1/§5.2). Вся работа — внутри `flow.rs` +
активация фичи в `canvas-core/Cargo.toml` + новый тест `parallel_determinism.rs`.

## Фазы и проверяемые коммиты

Три фазы — отдельные коммиты (паттерн «одна сессия агента = один коммит», AGENTS.md);
каждая поставляема и приёмлема независимо. Между фазами — профилирование.

### P1 — `topo_levels`

- **Коммит:** `feat(core/flow): topo_levels() — tiered topo sort (FR-065 P1)`
- **Что:** НОВАЯ `pub fn topo_levels(canvas) -> Result<Vec<Vec<usize>>, CycleError>`;
  `topo_sort` стабилен. Инвариант-тест flatten-эквивалентности на эталонах
  ADR-0005/0006 + случайных графах.
- **Гейт:** `cargo test -p canvas-core`; `cargo build --no-default-features`;
  `cargo clippy -D warnings`.

### P2 — `std::thread::scope` per-level

- **Коммит:** `feat(core/flow): std::thread::scope per-level parallel eval (FR-065 P2)`
- **Что:** Refactor цикла `for index in order` (`flow.rs:416`) →
  `for level in topo_levels() { std::thread::scope(|s| for index in level {
  s.spawn(|| eval node) }) }` за `#[cfg(all(feature="parallel",
  not(target_arch="wasm32")))]`; else — flatten. `Env` clone на поток.
  **collect-then-reduce** (никакого `par_iter().reduce()`).
- **Гейт:** determinism golden (числа эталонов ADR-0005/0006 **побитово идентичны**
  однопоточному пути, контракт §5.7); `cargo test --workspace --features parallel`;
  `scripts/wasm_gate.sh --check`; `scripts/mcp_wasm_gate.sh --check`.

### P3 — `rayon` switch

- **Коммит:** `feat(core/flow): rayon par_iter switch (FR-065 P3)`
- **Что:** По профилированию P2 (когда спавн `scope`-потоков на ярусах в десятки
  узлов заметен): `level.par_iter().map(...).collect::<Vec<_>>()` с sort+reduce в
  фиксированном порядке. Бенчмарк-гейт: ≥2× на тяжёлом графе (1000 нод) при ≥4
  ядрах; determinism golden остаётся зелёным. `cargo deny check` (`rayon` уже в
  allowlist, архдок §7).
- **Гейт:** `cargo test --workspace --features parallel`; бенчмарк ≥2× (критерий
  архдока §9 M4); `cargo deny check`; `cargo clippy -D warnings`.

## Ограничения для агента-реализатора

- **v1 `std::thread::scope`, v2 `rayon` по профилированию.** Не стартовать с `rayon`
  сразу — оверхед на спавн измеряется, и только если он доминирует, переключение на
  `par_iter`. Преждевременный `rayon` на мелких ярусах (5–10 узлов) проигрывает
  однопоточному пути.
- **`cfg(not(target_arch="wasm32"))` + flatten-на-wasm.** Параллельный путь — только
  desktop; гейты `wasm_gate.sh --check` и `mcp_wasm_gate.sh --check` обязательны на
  каждом коммите.
- **collect-then-reduce.** Никакого `par_iter().reduce()` (контракт §5.7.3 —
  недетерминированный порядок float-агрегатов). Безопасно: `par_iter().collect::<Vec<_>>()`
  → sort node ids → reduce в фиксированном порядке.
- **Golden-эталоны ADR-0005/0006 — побитово идентичны** однопоточному пути
  (`close_1pct` `tests.rs:1470`; ADR-0006 №1: peak_rps≈208.33, CDN W≈34.29ms ρ0.417).
  Дрейф = баг.
- **`Env` Clone — каждый поток клонирует свой** (`expr.rs:486`). clone O(переменные
  + params + inbound) — дёшево относительно `eval`. Если профилирование покажет
  дорогой clone на 1000-нод графах — v2: `Arc<Env>` (Env immutable в пределах eval;
  риск R-S2 плана).
- **Ускорение ≥2× на тяжёлом графе ≥4 ядра** — критерий архдока §9 M4. Бенчмарк:
  синтетический граф 1000 нод (или эталон ×20). Меньше 2× — отказ от переключения
  на `rayon` (P3 откладывается), `scope` остаётся.
- **`cargo build --no-default-features` — zero-dep инвариант.** Без `parallel`
  `rayon` НЕ подключается; `cargo deny check` зелёный; B2B-сборка не тащит
  параллелизм-зависимости.
- **Не трогать:** `scene.rs`, `expr.rs`, `canvas-mcp/main.rs`. Сигнатуры
  `propagate_with_lines`/`propagate_with_lines_data`/`topo_sort` — НЕ менять.

## Наглядная проверка (5 минут)

**Демо.** Тяжёлый граф (1000 нод — синтетический или эталон ADR-0006 №5 ×20):
пересчёт на 4 ядрах ≥2× быстрее однопоточного (видно по latency reval в debug-overlay
или `cargo bench`). Числа эталонов ADR-0006 — **идентичны** обоим путям (визуально:
дельт нет, бейджи результатов не дрейфуют между `--features parallel` и default-сборкой).

**Автотесты (зелёные):**

- `topo_levels().flatten() == topo_sort()` — flatten-эквивалентность (P1-инвариант).
- Determinism golden: `mcp_fr029_instagram_mvp_reference` (instagram MVP, ±1e-9 через
  `close_1pct`); `graph_apply_assembles_mini_reference_with_oracle` (mini-reference
  №1 побитово: peak_rps≈208.33, CDN W≈34.29ms ρ0.417); `analyze_bottlenecks_reference_and_growth`
  (ρ-гейты: DAU×2→Warn ρ0.833, DAU×5.35→Overload ρ2.23) — на ОБОИХ путях.
- Бенчмарк ≥2× на 1000 нод / 4 ядра (P3-гейт).
- `scripts/wasm_gate.sh --check` — flatten-фолбэк компилируется на `wasm32-wasip1`.

## Проверка (Verification)

Чек-лист приёмки (каждый коммит S3 — все зелёные):

- [ ] `cargo test --workspace --features parallel` — determinism golden + flatten-инвариант + параллельный путь не падает.
- [ ] `cargo test -p canvas-core` (без фич) — flatten-путь стабилен, zero-dep инвариант.
- [ ] `cargo build --no-default-features` — `rayon` НЕ подключается (B2B-инвариант).
- [ ] `cargo clippy -D warnings`; `cargo deny check` (`rayon` в allowlist, новых лицензий нет).
- [ ] `scripts/wasm_gate.sh --check`; `scripts/mcp_wasm_gate.sh --check` — flatten-фолбэк на `wasm32-wasip1`.
- [ ] Determinism golden на обоих путях: числа эталонов ADR-0005/0006 побитово идентичны (drift = баг).
- [ ] Flatten-эквивалентность: `topo_levels(canvas)?.into_iter().flatten().collect::<Vec<_>>() == topo_sort(canvas)?`.
- [ ] Бенчмарк ≥2× на 1000 нод / 4 ядра (P3-гейт; для P2 — замер без обязательства).

## Точки входа (Entry Points)

- `docs/SPEC.md` §6.3 — бюджеты пересчёта (<10 мс на 1000 нод однопоточно; параллельный
  путь — выигрыш на тяжёлых режимах, не одиночном reval).
- `docs/architecture/math-computing-stack.md` §5.3 (P2 поярусный), §5.6 (детерминизм),
  §5.8 (wasm), §9 (M4/S3 — критерий ≥2× на 4 ядрах).
- `docs/DEPENDENCIES.md` §3 → §2 (после merge P3 — миграция `rayon` из кандидатов в
  прямые прод-зависимости).
- `docs/adr/adr-0008-math-computing-stack.md` — решение (этап M4).
- `docs/plans/adr-0008-wave-s-plan.md` §5.1/§5.2/§5.6/§5.7.3/§5.8 — контракты на стыках
  между M3/M4/M5.
- FR-013 (чистые функции, инварианты 1/2), FR-014 (`topo_sort`/propagator), FR-064
  (M3 воркер — контракт `propagate_with_lines`), FR-066 (M5 Monte Carlo — потребитель
  параллельных прогонов).

## История изменений (Changelog)

- `2026-09-24` — агент: создан документ (план, статус `выявлено`). Зафиксированы 3
  фазы (P1 `topo_levels` → P2 `std::thread::scope` per-level → P3 `rayon` `par_iter`),
  контракты на стыках (§5.1/§5.2/§5.6/§5.7.3/§5.8), ограничения для агента-реализатора
  (collect-then-reduce, `cfg(not(wasm32))` + flatten-фолбэк, golden-побитовоидентичность,
  ≥2× на 1000 нод / 4 ядра, zero-dep инвариант). Код-ссылки: `topo_sort` `flow.rs:78`,
  `propagate_with_lines_data` `flow.rs:405`, цикл `for index in order` `flow.rs:416`,
  `Env` `expr.rs:486`, golden-тесты `tests.rs:1480,1759,2152`. Решения за владельцем —
  до гейта Go волны S.

## Источники истины (References)

- `crates/canvas-core/src/flow.rs:78,109–122,132,65,36,353` — `topo_sort` (ПЛОСКИЙ
  `Vec<usize>`, Kahn с `VecDeque`-фронтиром по возрастанию индексов — детерминизм;
  точка extraction `topo_levels`); `cycle_participants` (итеративный Тарьян);
  `CycleError { nodes }`; `FlowKind { Control, Value }`; `FlowSolutions { outputs,
  lines, named, warnings }` (результат пересчёта; не меняется). `topo_sort` стабилен
  по контракту §5.2.
- `crates/canvas-core/src/flow.rs:394,405,416` — `propagate_with_lines` (тонкая
  обёртка; СТАБИЛЬНА), `propagate_with_lines_data` (реальный entry; СТАБИЛЬНА),
  `for index in order` (МЕСТО M4 — refactor на поярусный обход).
- `crates/canvas-core/src/expr.rs:486,1384` — `Env { vars, params, inbound, qualified }`
  (`#[derive(Debug, Clone, Default, PartialEq)]`); `eval(expr, env) ->
  Result<Value, EvalError>` (чистая функция — инварианты 1/2 FR-013; предпосылка M4).
- `crates/canvas-core/Cargo.toml:34` — `parallel = []` (CP0-гейт; S0 добавляет optional
  `rayon`, FR-065 активирует `parallel = ["dep:rayon"]`).
- `Cargo.lock:2069,541` — `rayon` (транзитивно через `cosmic-text`; MIT OR Apache-2.0;
  прямое включение НЕ добавляет новых лицензий).
- `crates/canvas-scene/src/tests.rs:1470,1480,1759,2152` — `close_1pct` oracle,
  `mcp_fr029_instagram_mvp_reference` (ADR-0005),
  `graph_apply_assembles_mini_reference_with_oracle` (ADR-0006 №1: peak_rps≈208.33,
  CDN W≈34.29ms ρ0.417), `analyze_bottlenecks_reference_and_growth` (ADR-0006 №2:
  DAU×2→Warn ρ0.833, DAU×5.35→Overload ρ2.23).
- `docs/adr/adr-0008-math-computing-stack.md` — решение (этап M4).
- `docs/architecture/math-computing-stack.md` §5.1 (чистота → параллелизм), §5.3 (P2
  поярусный), §5.6 (детерминизм), §5.8 (wasm), §9 (M4 — критерий ≥2×).
- `docs/plans/adr-0008-wave-s-plan.md` §5.1/§5.2/§5.6/§5.7.3/§5.8 — контракты на стыках;
  §6 — фазы FR-065.
- `docs/plans/product-roadmap.md` §4.5 — волна S, S3 (после гейта Go).
- `docs/DEPENDENCIES.md` §3 (кандидат `rayon` — «уже транзитивно в дереве через
  `cosmic-text`»), §2 (миграция после активации); `docs/SPEC.md` §6.3 (бюджеты).
- `scripts/wasm_gate.sh`, `scripts/mcp_wasm_gate.sh` — гейты wasm-совместимости
  (`RUST_TEST_THREADS=1` на `wasm32-wasip1`).
