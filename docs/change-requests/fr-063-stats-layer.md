# FR-063: Доменный уровень статистики — распределения, квантили, доверительные интервалы и детерминированный RNG в слое L2

- **Статус:** ✅ реализовано (S0-депсы + P1/P2/P3, 2026-09-24)
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** агент (планирование)
- **Источник:** план реализации ADR-0008 волна S (`docs/plans/adr-0008-wave-s-plan.md`), по запросу владельца 2026-09-24
- **Связанные задачи:** ADR-0008; `docs/architecture/math-computing-stack.md` §4.2/§9 (M2/S1); `docs/plans/product-roadmap.md` §4.5 (S1); FR-013 (Numi-движок), FR-015 (доменные функции queueing — образец паттерна), FR-021 (FN_HINTS parity), FR-027 (финансовые функции), FR-017 (what-if — потребитель ДИ); FR-066 (M5 Monte Carlo — потребитель RNG/распределений); `docs/DEPENDENCIES.md` §3
- **Создан:** 2026-09-24
- **Обновлён:** 2026-09-24

## Описание (What)

Добавляется новый доменный модуль `expr/stats.rs` (слой L2 расчётного стека ADR-0008): чистые функции распределений, квантилей и доверительных интервалов, скрытые за обёрткой `stats::dispatch` по образцу `queueing::dispatch` (FR-015), и детерминированный RNG (`ChaCha8Rng::seed_from_u64`) для выборок в Monte Carlo (M5, FR-066). Назначение — дать волне V (вероятностные оценки: запас мощности, риск-анализ, чувствительность глубже чем сетки ±20 % из FR-017 v1) общий вычислительный фундамент без правок движка и без нарушения инварианта zero-dep сборки `cargo build --no-default-features`. Сидированная случайность гарантирует побитовую воспроизводимость выборок между прогонами, что является контрактом для M5 и для любого ручного «попробовать ещё раз».

**Почему сейчас.** FR-017 v1 даёт only-дельты по сеткам ±20 % — этого хватает для грубой прикидки запаса мощности, но не для риск-анализа (нужны P95/P99 утилизации) и не для чувствительности к редким событиям (нужны хвосты распределений). Monte Carlo (M5, FR-066) требует сидированного ГСЧ — иначе выборки не воспроизводимы и any сравнение сценариев бессмысленно. ADR-0008 волна S уже зарезервировала фичу `stats` (CP0) и планирует deps-волной S0 подключение `statrs`/`rand`/`rand_chacha`/`rand_distr`; FR-063 — это фаза S1, которая активирует фичу и строит доменный слой. Без FR-063 волна V остаётся без фундамента и вынуждена либо тащить статистику в каждый шаблон отдельно, либо отказаться от вероятностных оценок.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `expr` | Новый домен `stats` (`stats::dispatch` + чистые функции); новый arm в `eval_call` за `#[cfg(feature = "stats")]`; новая запись `mod stats` рядом с `mod queueing` | `crates/canvas-core/src/expr.rs`, `crates/canvas-core/src/expr/stats.rs` (новый) |
| `templates` | Формулы шаблонных нод могут использовать `normal_quantile`, `normal_cdf`, `lognormal_quantile`, `exp_quantile`, `poisson_pmf`, `triangular`, `ci_mean`, `normal_sample`, `lognormal_sample` | `assets/templates/` (финансовые/UE-шаблоны FR-027 — по мере апдейта) |
| `what-if` | Сценарии FR-017 получают доверительные интервалы поверх дельт (`ci_mean(mean, sigma, n, conf)` на агрегатах сценария); P95/P99 утилизации видны прямо на канвасе | `docs/change-requests/fr-017-what-if-scenarios.md` (потребитель); `user-docs/calculations.md` |
| `MCP` | `flow_recalc` прозрачно возвращает значения, считаемые через stats-функции (без новых полей — статистика лежит в формулах нод, не в API) | `docs/SPEC.md` §MCP |
| `Cargo` | Активация фичи `stats = ["dep:statrs","dep:rand","dep:rand_chacha","dep:rand_distr"]` (deps уже прописаны волной S0 — **не добавлять deps в этом FR**) | `crates/canvas-core/Cargo.toml` `[features]` |
| `docs` | `DEPENDENCIES.md` §3 → §2 (статус «в работе» → «принят/подключён»); `user-docs/calculations.md` — раздел «Вероятностные оценки»; `SPEC.md` §расчётный стек | см. «Точки входа» |

## Анализ (Root Cause)

Слой L2 (доменные функции за диспетчером) сегодня содержит только `queueing` (`crates/canvas-core/src/expr/queueing.rs:37`, `dispatch`), вызванный из центрального диспетчера `eval_call` (`expr.rs:1699–1734`). Домена `stats` нет ни на уровне модуля, ни в реестре `eval_call`, ни в `FN_HINTS` (`expr.rs:1897+`). Конкретно отсутствует:

- **Модуль `stats`.** В `expr.rs:42` (`mod queueing;`) нет парного `mod stats;`. Файла `crates/canvas-core/src/expr/stats.rs` не существует. Сигнатуры-образца `pub(super) fn dispatch(func: &str, values: &[Value]) -> Result<Value, EvalError>` для статистики нет.
- **Arm в `eval_call`.** Матч по `func` (`expr.rs:1699–1734`) содержит встроенные `sum/avg/max/min/percentile` и доменные `utilization/mm1/mmc/littles_law/erlang_c/npv/cagr/irr/cohort_ltv` (последние уходят в `queueing::dispatch`); fallback — `Err(EvalError::UnknownFunction(_))`. Ветки для `normal_quantile`/`normal_cdf`/`lognormal_quantile`/`exp_quantile`/`poisson_pmf`/`triangular`/`ci_mean`/`normal_sample`/`lognormal_sample` нет.
- **FN_HINTS parity.** Реестра для статистических функций нет → parity-тест FR-021 (`expr.rs:1897+`) не покрывает stats-домен. Это означает, что подсказки ввода FR-021 не будут предлагать новые функции, хотя движок их распознает, — расхождение «движок знает, UI не подсказывает», которое паттерн FR-021 как раз и закрывает.
- **Helpers для разбора аргументов.** Утилиты `is_single_dim`, `scalar_arg`, `rate_arg`, `time_arg`, `percent_unit`, `time_unit`, `count_unit`, `bad_arity`, `bad_zero_service` приватны для `queueing.rs`. Stats-функциям нужен тот же набор (`scalar_arg`, `bad_arity`, размерностная проверка через `Unit::dims()/scale()`) — их приходится либо выносить в `expr/args.rs` (shared), либо дублировать локально. Сегодня файла `expr/args.rs` нет.
- **Cargo-фича.** `crates/canvas-core/Cargo.toml [features]` уже содержит пустые `stats = []` и `parallel = []` (CP0). Optional-deps `statrs/rand/rand_chacha/rand_distr` ставит волна S0 (предшествующая S1 в плане ADR-0008). FR-063 не добавляет deps, а только **активирует** фичу: `stats = ["dep:statrs","dep:rand","dep:rand_chacha","dep:rand_distr"]`.
- **RNG.** Сидированного ГСЧ в кодовой базе нет. `thread_rng()` в архдоке §5.6.2 запрещён (детерминизм). Доверительного источника `ChaCha8Rng::seed_from_u64(u64)` в проекте пока нет ни в одном модуле — FR-063 вводит его и фиксирует контракт сида для M5 (FR-066).
- **Зависимости.** Кандидаты версий: `statrs` 0.17 (MIT), `rand` 0.8 (MIT OR Apache-2.0), `rand_chacha` 0.3 (MIT OR Apache-2.0), `rand_distr` 0.4 (MIT OR Apache-2.0). Все pure-Rust, без криптографии, в allowlist архдока §7.2. Это значит, что wasm-сборка остаётся зелёной (`scripts/wasm_gate.sh --check`) — никаких `getrandom`-хуков вне web-sys не появляется.

## Требуемые изменения (Changes)

| Что → Где → Как |
|---|
| 1. **`crates/canvas-core/src/expr/stats.rs` (новый)** → чистый модуль: `pub(super) fn dispatch(func: &str, values: &[Value]) -> Result<Value, EvalError>` по образцу `queueing.rs:37` (матч по имени → arity-проверка → вызов чистой функции). Внутренние функции: `normal_quantile`, `normal_cdf`, `lognormal_quantile`, `exp_quantile`, `poisson_pmf`, `triangular` (P2); `ci_mean` + `normal_sample`/`lognormal_sample` (P3). Все сигнатуры `(&[Value]) -> Result<Value, EvalError>`. Модуль и его arm в `eval_call` обёрнуты в `#[cfg(feature = "stats")]`. |
| 2. **`crates/canvas-core/src/expr.rs`** → `mod stats;` рядом с `mod queueing;` (`expr.rs:42`) за `#[cfg(feature = "stats")]`; новый arm в `eval_call` (`expr.rs:1699–1734`) — матч по именам stats-функций → `stats::dispatch`, обёрнутый `#[cfg(feature = "stats")]`. Существующие arms и fallback `Err(EvalError::UnknownFunction)` не трогаются. |
| 3. **`crates/canvas-core/src/expr.rs` `FN_HINTS`** (`expr.rs:1897+`) → записи для всех stats-функций (имя, arity, краткое описание, тип аргументов). Паритет с `eval_call` покрывается тестом (паттерн FR-021). |
| 4. **`crates/canvas-core/src/expr/args.rs` (новый shared-модуль)** → вынос `scalar_arg`, `bad_arity`, `is_single_dim` (и при необходимости других) из `queueing.rs` в общий модуль, чтобы stats и queueing пользовались одним набором. Альтернатива (если реализатор не хочет трогать queueing.rs) — локальные приватные хелперы в `stats.rs`. Выбор фиксируется реализатором; в любом случае **`queueing.rs` public-API не меняется**, а его поведение — тем более. |
| 5. **`crates/canvas-core/Cargo.toml`** → активация фичи `stats = ["dep:statrs","dep:rand","dep:rand_chacha","dep:rand_distr"]`. **Deps не добавляются** — они прописаны волной S0 в `[workspace.dependencies]` и canvas-core. Если на момент запуска FR-063 deps ещё не прописаны, FR-063 блокируется на S0 (отметить в Ченжлог при смещении). |
| 6. **Тесты (новые)** → (a) golden распределений CDF/inverse-CDF против табличных значений ±1e-9 (`normal_cdf(1.96, 0, 1) ≈ 0.975`, `normal_quantile(0.95, μ, σ)`); (b) arity/размерностная проверка (`normal_quantile(0.95, utilization, 0.1)` — `utilization` безразмерный, σ в долях); (c) FN_HINTS parity-тест (множество имён в `eval_call` stats-arm = множество в `FN_HINTS`); (d) сид-воспроизводимость (один сид → побитово одна выборка `normal_sample`); (e) `cargo build --no-default-features` остаётся zero-dep (модуль не входит в сборку без фичи). |
| 7. **Документация** → `user-docs/calculations.md` — раздел «Вероятностные оценки» (синтаксис, единицы, детерминизм); `docs/DEPENDENCIES.md` §3 → §2 (перенос строк statrs/rand/rand_chacha/rand_distr из «в работе» в «принят/подключён»); `docs/SPEC.md` §расчётный стек — упоминание L2-stats. |

## Контракты на стыках

- **План §5.3 (eval_call arm за фичей).** Stats-arm живёт за `#[cfg(feature = "stats")]`; существующие arms (`sum/avg/max/min/percentile`, `queueing::dispatch`-делегация, fallback `Err(EvalError::UnknownFunction)`) **остаются стабильны**. Без фичи stats-функции возвращают `UnknownFunction` — обратная совместимость с любым `.canvas`, где формула вдруг содержит `normal_quantile(...)`, гарантируется graceful-деградацией (вижн-сценарии волны V активны только с включённой фичей).
- **План §5.6 (feature-флаги).** Фича `stats` ортогональна `parallel`. Активация — только в `Cargo.toml` `[features]`; никаких `cfg!`-веток в логике. Зависимости — pure-Rust, разрешены allowlist'ом архдока §7.2.
- **План §5.7 (детерминизм: сидированная случайность).** Единственный разрешённый источник случайности — `rand_chacha::ChaCha8Rng::seed_from_u64(u64)`. `thread_rng()` **запрещён** (архдок §5.6.2). Сид для выборок — детерминированная функция от контента ноды/сценария (`hash(content) ⊕ scenario_seed`); конкретная формула хеша фиксируется в P3. Два прогона с одним сидом дают побитово одинаковую выборку — это контракт для M5 Monte Carlo (FR-066) и для воспроизводимых what-if-экспериментов (FR-017).
- **План §5.8 (wasm).** Все четыре deps pure-Rust: `statrs` (чистая математика), `rand` (без `getrandom` в режиме `rand_chacha`), `rand_chacha` (чистый ChaCha8), `rand_distr` (распределения поверх `Rng`). `scripts/wasm_gate.sh --check` должен остаться зелёным — это явный гейт P2/P3.
- **Границы изменений.** НЕ трогаются: `flow.rs` (поток значений), `scene.rs` (состояние сцены), `templates.rs` (манифесты), `expr/queueing.rs` (его public-API и поведение). Изменения локализованы в `expr/stats.rs` (новый), `expr/args.rs` (новый, опционально), `expr.rs` (три строки: `mod`, arm, FN_HINTS), `Cargo.toml` (одна строка активации фичи), тесты, документация.
- **Стык с FR-017 (what-if).** Доверительные интервалы считаются поверх агрегатов сценария — то есть what-if бар FR-017 вызывает `ci_mean(mean, sigma, n, conf)` из своей формулы ноды или из шаблона, а не из отдельного API. Это значит, что FR-063 не вводит новой MCP-команды и не меняет `flow_recalc` — статистика прозрачна для propagator, как и `npv`/`cagr` из `queueing`. Сценарий FR-017 остаётся источником `mean`/`sigma`/`n`; FR-063 лишь даёт функцию для их свёртки в ДИ.
- **Стык с FR-066 (Monte Carlo, M5).** `normal_sample`/`lognormal_sample` возвращают scalar-агрегат выборки (v1, см. «Открытые вопросы» № 4); полный Monte Carlo с массивами — ответственность FR-066. Контракт, который FR-063 фиксирует для M5: один и тот же `(content, scenario_seed)` → побитово одна и та же выборка, независимо от платформы и времени прогона. Любое нарушение этого контракта в FR-066 — баг FR-063, а не FR-066.

## Фазы и проверяемые коммиты

Каждая фаза — отдельный коммит с зелёным гейтом (паттерн «одна сессия агента = один коммит», AGENTS.md). Между фазами допускается работа других агентов над параллельными фронтами волны S.

### P1 — skeleton
**Коммит:** `feat(core/stats): skeleton — stats::dispatch + mod stats + cfg gate (FR-063 P1)`
- Пустой `stats::dispatch` (возвращает `Err(EvalError::UnknownFunction(_))` для любого имени).
- `mod stats;` в `expr.rs:42` за `#[cfg(feature = "stats")]`.
- Arm в `eval_call` за `#[cfg(feature = "stats")]`, делегирующий в `stats::dispatch`.
- Активация фичи `stats = ["dep:statrs","dep:rand","dep:rand_chacha","dep:rand_distr"]` в `Cargo.toml`.
- Parity-тест FN_HINTS (пока пустое множество stats-имён = пустое множество в `FN_HINTS`).
**Гейт:** `cargo test -p canvas-core --features stats` зелёный; `cargo build --no-default-features` зелёный (zero-dep инвариант); `cargo fmt --check`; `cargo clippy -D warnings`.

### P2 — распределения и квантили
**Коммит:** `feat(core/stats): normal/lognormal/exp/poisson distributions + quantiles (FR-063 P2)`
- Реализация `normal_quantile(p, μ, σ)`, `normal_cdf(x, μ, σ)`, `lognormal_quantile`, `exp_quantile`, `poisson_pmf`, `triangular` над `statrs`/`rand_distr`.
- Размерностная проверка: `p` ∈ [0, 1] (безразмерное), `μ`/`σ` в единицах аргумента (для `normal_quantile(0.95, utilization, 0.1)` — `utilization` безразмерный, `σ = 0.1` безразмерный; результат — безразмерный). Размерность результата = размерность `μ`.
- Golden-тесты CDF/inverse-CDF против табличных значений ±1e-9 (`normal_cdf(1.96, 0, 1) ≈ 0.975002`; `normal_quantile(0.975, 0, 1) ≈ 1.96`; `poisson_pmf(k=3, λ=2) ≈ 0.180447`).
- Arity/`EvalError` (включая `BadCall{func,msg}` для `p ∉ [0,1]`, `σ ≤ 0`).
- FN_HINTS parity-тест с актуальным множеством имён.
**Гейт:** `cargo test -p canvas-core --features stats`; golden ±1e-9; `cargo fmt --check`; `cargo clippy -D warnings`; `scripts/wasm_gate.sh --check` (stats pure-Rust — зелёный).

### P3 — RNG и доверительные интервалы
**Коммит:** `feat(core/stats): deterministic ChaCha8 RNG + confidence intervals (FR-063 P3)`
- Сид `seed_from_u64(hash(content) ⊕ scenario_seed)`; конкретная формула хеша зафиксирована в коде и описана в `user-docs/calculations.md`.
- `normal_sample(μ, σ, n, seed)` / `lognormal_sample(μ, σ, n, seed)` — детерминированные выборки (для M5 Monte Carlo, FR-066).
- `ci_mean(mean, sigma, n, conf)` — доверительный интервал для среднего через `statrs` (normal-approx; t-распределение для малых `n` — по мере необходимости).
- Тест воспроизводимости: один сид → побитово одна выборка (два вызова `normal_sample` с одинаковыми аргументами возвращают равные `Vec<f64>`).
- Тест `cargo deny check` — все лицензии в allowlist'е.
**Гейт:** `cargo test -p canvas-core --features stats`; сид-воспроизводимость зелёная; `cargo deny check` зелёный; `scripts/wasm_gate.sh --check` зелёный; `cargo fmt --check`; `cargo clippy -D warnings`.

## Ограничения для агента-реализатора

1. **Чистые функции.** Все stats-функции имеют сигнатуру `(&[Value]) -> Result<Value, EvalError>` (или `fn(&mut Rng, &[Value]) -> Result<Value, EvalError>` для sampling-функций, где `Rng: rand::Rng`). Никаких `&mut Env`, никакого доступа к `Canvas`/`SceneState` — статистика живёт в слое L2, как и `queueing`.
2. **RNG.** `thread_rng()` **запрещён**. Единственный источник случайности — `ChaCha8Rng::seed_from_u64(u64)` (архдок §5.6.2). Сид детерминирован из контента и сценария — никакого «случайного по часам».
3. **Размерность.** Размерностная проверка — через `pub(crate) Unit::dims()` и `Unit::scale()` (доступны из `stats.rs`, тот же крейт `canvas-core`). Для `normal_quantile(p, μ, σ)`: `p` безразмерный (`Unit::Scalar`), `μ` и `σ` одной размерности, результат — той же. Несовпадение → `EvalError::UnitMismatch`. Для `ci_mean(mean, sigma, n, conf)`: `mean` и `sigma` одной размерности, `n` — безразмерный count, `conf` ∈ [0,1], результат — той же размерности (границы ДИ).
4. **Helpers.** `is_single_dim`/`scalar_arg`/`bad_arity`/и т.д. приватны для `queueing.rs`. Реализатор выбирает: (a) вынести общий набор в новый `expr/args.rs` (предпочтительно — меньше дублей) ИЛИ (b) завести локальные приватные хелперы в `stats.rs`. В любом случае **`queueing.rs` public-API и поведение не меняются**.
5. **Feature-gate.** `#[cfg(feature = "stats")]` на `mod stats;`, на arm в `eval_call`, на записи `FN_HINTS` (если реестр собирается статически — на соответствующих строках). Без фичи модуль не компилируется, функции не регистрируются, `cargo build --no-default-features` остаётся zero-dep.
6. **Границы.** НЕ трогать `flow.rs`, `scene.rs`, `templates.rs`, `expr/queueing.rs`. Изменения в `expr.rs` — три локальные правки (`mod`, arm, FN_HINTS), без рефакторинга существующих arms.
7. **EvalError.** Использовать существующие варианты (`BadCall{func,msg}` для ошибок аргументов, `UnitMismatch` для размерности, `DivisionByZero` где уместно). Новых вариантов не вводить. `PartialEq` есть (нужен для тестов), `Eq` нет — это нормально (f64).

## Открытые вопросы реализации

Фиксируются здесь, чтобы агент-реализатор не додумывал на ходу; окончательное решение — на этапе P1 (если не оговорено иное):

1. **Где живут helpers — `expr/args.rs` (shared) или локально в `stats.rs`.** Предпочтение — shared-модуль (меньше дублей, симметрия с будущими доменами L2). Риск: триггерит рефакторинг `queueing.rs` (вынос `use` и замена вызовов). Если реализатор хочет минимизировать поверхность изменений P1 — локальные хелперы в `stats.rs` принимаются как компромисс; второй проход (отдельный коммит) может их консолидировать.
2. **Формула сида.** `seed_from_u64(hash(content) ⊕ scenario_seed)` — контур зафиксирован; конкретный хеш (FxHash/std::DefaultHasher/`seahash`) и то, что входит в `content` (текст ноды? весь канвас? id сцены?) — решение P3. Контракт: одинаковые `content` + `scenario_seed` → одинаковая выборка, независимо от платформы и времени. `std::DefaultHasher` не стабилен по версиям Rust — НЕ подходит; нужен явный детерминированный хеш.
3. **`ci_mean` для малых `n`.** Normal-approximation достаточна для v1 (волна V работает с большими выборками из Monte Carlo M5); t-распределение — опционально, если в P3 появится `statrs::distribution::StudentsT`. Решение: normal-approx в P3, t-расширение — отдельный коммит, если потребуется.
4. **Sampling-функции в `FN_HINTS`.** `normal_sample(μ, σ, n, seed)` принимает 4 аргумента, возвращает массив — но `Value` скаляр. Решение: либо возвращать `Value` как scalar-среднее выборки (v1, волна V довольствуется агрегатом), либо вводить List-`Value` (требует расширения `Value`, выходит за рамки FR-063). Принимается **scalar-агрегат** для v1; полный Monte Carlo с массивами — FR-066 (M5).
5. **Блокировка на S0.** Если на момент запуска P1 deps `statrs`/`rand`/`rand_chacha`/`rand_distr` ещё не прописаны волной S0 в `[workspace.dependencies]` и canvas-core, FR-063 не стартует — зафиксировать в Ченжлог как «ожидает S0». P1 (skeleton без вызовов `statrs`) может быть выполнен с пустой фичей `stats = []`, но P2/P3 блокируются.

## Наглядная проверка (5 минут)

**Демо.** На эталоне ADR-0006 №1 добавить ноду с формулой `normal_quantile(0.95, utilization, 0.1)` — P95 утилизации виден на канвасе. Поменять входную нагрузку на +20 % → P95 сдвигается. Повторный прогон с тем же сидом для `normal_sample(...)` (если в формуле есть sampling) → идентичная выборка (побитово). Включить/выключить фичу `stats` — без фичи нода показывает ошибку `UnknownFunction("normal_quantile")`, с фичей — число. Это наблюдаемо без чтения кода: виден и сам P95, и обратная совместимость.

**Автотесты (гейт коммита).** Golden распределений ±1e-9; FN_HINTS parity (множество stats-имён в `eval_call` = множество в `FN_HINTS`); сид-воспроизводимость (один сид → равные выборки); `cargo deny check`; `scripts/wasm_gate.sh --check` (stats pure-Rust — должен быть зелёным); `cargo build --no-default-features` (zero-dep инвариант).

## Проверка (Verification)

Чек-лист гейтов для закрытия FR-063 (все три фазы). Тесты живут в `crates/canvas-core/tests/` (integration) и в `crates/canvas-core/src/expr/stats.rs` (`#[cfg(test)] mod tests`); имена файлов по образцу существующих (`integration_flow.rs`, `integration_units.rs`):

- [ ] `cargo test -p canvas-core --features stats` — зелёный (включая golden ±1e-9 и сид-воспроизводимость).
- [ ] `cargo build --no-default-features` — зелёный (zero-dep инвариант; stats-модуль не входит в сборку).
- [ ] `cargo build -p canvas-core --features stats` — зелёный (deps резолвятся из `[workspace.dependencies]`).
- [ ] `cargo fmt --check` — зелёный.
- [ ] `cargo clippy -- -D warnings` (с фичей `stats`) — зелёный.
- [ ] `cargo deny check` — зелёный (лицензии `statrs` 0.17 MIT; `rand` 0.8 MIT OR Apache-2.0; `rand_chacha` 0.3 MIT OR Apache-2.0; `rand_distr` 0.4 MIT OR Apache-2.0 — все в allowlist'е архдока §7.2).
- [ ] `scripts/wasm_gate.sh --check` — зелёный (все deps pure-Rust, без `getrandom`-хуков вне web-sys).
- [ ] `tests/integration_stats.rs` (новый) — golden-кейсы:
      `normal_cdf(1.96, 0, 1) ≈ 0.975002`;
      `normal_quantile(0.975, 0, 1) ≈ 1.959964`;
      `lognormal_quantile(0.5, 0, 1) = 1.0`;
      `exp_quantile(0.632, λ=1) ≈ 1.0`;
      `poisson_pmf(k=3, λ=2) ≈ 0.180447`;
      `triangular_quantile(0.5, a=0, b=1, c=0.5) = 0.5`.
- [ ] `tests/integration_stats.rs` — размерностная проверка:
      `normal_quantile(0.95, utilization_scalar, 0.1_scalar)` → безразмерный результат;
      `normal_quantile(0.95, value_with_unit_rps, sigma_rps)` → результат в `rps`;
      `normal_quantile(1.5, ...)` → `Err(EvalError::BadCall{func, msg})` (`p ∉ [0,1]`);
      `normal_quantile(0.5, μ, σ=-1)` → `Err(EvalError::BadCall{func, msg})`.
- [ ] FN_HINTS parity-тест — зелёный (множество stats-имён в `eval_call` == множество в `FN_HINTS`); паттерн FR-021.
- [ ] Сид-воспроизводимость: `normal_sample(0, 1, n=1000, seed=42)` вызывается дважды → равные выборки (байт-в-байт). Тест фиксирует, что смена платформы (linux/macos/wasm) не меняет результат.
- [ ] Обратная совместимость: `.canvas` с формулой `normal_quantile(...)` открывается без фичи `stats` — нода помечается ошибкой `UnknownFunction`, остальной поток значений считается штатно (no panic, no abort).
- [ ] Наглядная проверка (5 минут) — пройдена вручную (см. выше).

## Точки входа

- `docs/interface-objects/node.md` — формулы с stats-функциями (P95 утилизации, ДИ) в списках поддерживаемых функций; примеры.
- `user-docs/calculations.md` — раздел «Вероятностные оценки» (синтаксис, единицы, контракт детерминизма сида, что значит P95 на канвасе).
- `docs/SPEC.md` §расчётный стек — слой L2 теперь содержит `queueing` и `stats`.
- `docs/DEPENDENCIES.md` §3 → §2 — перенос `statrs`/`rand`/`rand_chacha`/`rand_distr` из «в работе» в «принят/подключён» (с указанием версии и лицензии).
- `docs/change-requests/fr-017-what-if-scenarios.md` — потребитель ДИ (ссылка на FR-063 как источник `ci_mean`).
- `docs/change-requests/fr-066-...` (M5 Monte Carlo, когда появится) — потребитель `normal_sample`/`lognormal_sample` и контракта сида.

## История изменений

- `2026-09-24` — агент: создан документ (план, статус `выявлено (план)`). Зафиксированы: структура модуля `expr/stats.rs` по образцу `queueing`, три места интеграции в `expr.rs` (`mod`, `eval_call` arm, `FN_HINTS`), активация фичи `stats` без добавления deps (их ставит S0), три фазы-коммита (P1 skeleton / P2 распределения / P3 RNG + ДИ) с гейтами, ограничения для реализатора (чистые функции, запрет `thread_rng`, размерность через `Unit::dims()/scale()`, feature-gate, границы «не трогать flow/scene/templates/queueing»), 5-минутная наглядная проверка и чек-лист гейтов.
- `2026-09-24` — агент: **реализовано** (S0 + P1 + P2 + P3, ветка `feature/fr-063-stats-layer`).
  - **S0-депсы выполнены этим FR** (Открытый вопрос № 5): на момент старта волна S0 не была выполнена — deps не были прописаны; подключены в этом же цикле (`statrs` 0.17 MIT; `rand` 0.8 / `rand_chacha` 0.3 / `rand_distr` 0.4 MIT OR Apache-2.0 — optional, `[workspace.dependencies]`, Cargo.lock чисто аддитивный). Мои rand/rand_chacha/rand_distr — `default-features = false` (без getrandom; `thread_rng()` не только запрещён, но и не компилируется).
  - **P1**: `expr/stats.rs` (skeleton), `mod stats` + arm в `eval_call` за `#[cfg(feature = "stats")]` (guard по единой точке `STATS_FUNCTIONS` — список имён не может разойтись с arm), `expr/args.rs` (shared-хелперы — вариант (a) Открытого вопроса № 1: `bad_arity`/`scalar_arg`/`is_single_dim`/конструкторы юнитов вынесены из queueing.rs дословно, поведение queueing не изменено). **Найден и закрыт существующий пробел parity FR-021**: 4 финансовые функции FR-027 отсутствовали в `FN_HINTS` — добавлены (побочный фикс, в скоупе правки FN_HINTS).
  - **P2**: `normal_quantile`/`normal_cdf`/`lognormal_quantile`/`exp_quantile`/`poisson_pmf`/`triangular_quantile` поверх statrs 0.17 (`ContinuousCDF::inverse_cdf/cdf`, `Discrete::pmf`); края p=0/1 обработаны явно (statrs паникует вне [0,1] — вход валидируется до вызова); `triangular` — алиас (расхождение имён в самом FR: «Изменения» — triangular, golden — triangular_quantile; поддержаны оба, parity-тест держит).
  - **P3**: `ChaCha8Rng::seed_from_u64` (единственный источник случайности); сид-контракт M5 зафиксирован в коде: `seed_from_parts(content, scenario_seed) = FNV-1a 64(content) ⊕ scenario_seed` (FNV-1a — явный детерминированный хеш, DefaultHasher забракован FR; векторы зафиксированы тестом; `run_idx` — добавляет FR-066); `ci_mean` — полуширина ДИ (норм. аппроксимация, Открытый вопрос № 3), границы — формулой потребителя `mean ± ci_mean(...)`; `normal_sample`/`lognormal_sample` — scalar-агрегат (среднее выборки, Открытый вопрос № 4), кап n ≤ 1e6 (защита канваса; массовые прогоны — FR-066).
  - **Тесты**: golden ±1e-9 (`expr_stats.rs`, 22), воспроизводимость `to_bits` (один сид → побитово одна выборка), FNV-векторы, размерности/юниты, BadCall/UnitMismatch, обратная совместимость без фичи (`expr_stats_compat.rs`, зеркальный cfg) — UnknownFunction без паники, встроенные/queueing функции штатны.
  - **Гейты**: `cargo test -p canvas-core --features stats` ✓ (390+22+…); `cargo test -p canvas-core` ✓ (384+2 compat); `cargo test -p canvas-app --lib` ✓ (342, hints-каталог потребителя не задет); `cargo build --no-default-features` ✓ (zero-dep); `scripts/wasm_gate.sh --check` ✓; `cargo deny check` ✓ (licenses/bans/sources/advisories); `cargo fmt --check` ✓; `cargo clippy --features stats -D warnings` ✓; THIRD-PARTY-NOTICES перегенерирован (попутно починен about.toml — предсуществующий баг: без `[private] ignore = true` генерация падала на AGPL-workspace-крейтах с d6fb7df).
  - **Отклонения/решения реализации** (все задокументированы в коде): (1) имя теста — `tests/expr_stats.rs` по текущей конвенции репо (образец `expr_queueing.rs`), а не `integration_stats.rs` (файлы `integration_*` живут в canvas-app/tests, в canvas-core/tests такой конвенции нет — ссылка FR устарела); (2) triangular_quantile + алиас triangular; (3) ci_mean возвращает полуширину (Value скалярен — возвращать «две границы» одним Value нельзя); (4) lognormal-параметры строго безразмерны (лог размерной величины не определён); (5) exp_quantile: Rate-λ → секунды, скалярный λ → скаляр.
  - **Находка для владельца (wasm+stats)**: statrs 0.17 жёстко тянет rand с default-фичами → транзитивный getrandom 0.2 попадает в граф ТОЛЬКО при включённой фиче `stats` и под wasm32-unknown-unknown не компилируется (compile_error без js-фичи). Все формальные гейты зелёные: wasm-гейт и CI wasm-check идут с default-фичами (stats не включён — rand в граф не попадает), нативная сборка --features stats зелёная, рантайм-вызовов thread_rng/OsRng нет. Решение по web-сборке с stats (getrandom js-фича через web-sys-хук, как санкционировано формулировкой FR, либо чистая математика без statrs — тогда +истинный кросс-платформенный детерминизм) — точка решения владельца, вне скоупа этого FR (прецедент no-wasm-фичи уже запланирован для qmc/FR-066).

## Источники истины

- `crates/canvas-core/src/expr.rs:42` — `mod queueing;` (место для парного `mod stats;`).
- `crates/canvas-core/src/expr.rs:337` — `Value { num: f64, unit: Unit }` (методы `scalar`/`with_unit`/`dims`/`display_parts`).
- `crates/canvas-core/src/expr.rs:486` — `Env { vars, params, inbound, qualified }` (приватные поля, `Clone+Default+PartialEq`).
- `crates/canvas-core/src/expr.rs:603` — `EvalError` (варианты `UnitMismatch/UnknownFunction/BadCall/...`, `PartialEq`, без `Eq`).
- `crates/canvas-core/src/expr.rs:1699–1734` — `eval_call` (центральный диспетчер; место для stats-arm за `#[cfg(feature = "stats")]`).
- `crates/canvas-core/src/expr.rs:1897+` — `FN_HINTS` (реестр для parity-теста FR-021).
- `crates/canvas-core/src/expr/queueing.rs:37` — `pub(super) fn dispatch(func: &str, values: &[Value]) -> Result<Value, EvalError>` (образец паттерна для stats).
- `crates/canvas-core/src/expr/queueing.rs` — приватные helpers (`is_single_dim`, `scalar_arg`, `rate_arg`, `time_arg`, `percent_unit`, `time_unit`, `count_unit`, `bad_arity`, `bad_zero_service`) — основание для решения «вынести в `expr/args.rs` ИЛИ локально в `stats.rs`».
- `crates/canvas-core/src/expr.rs` (`Unit`) — `pub(crate) dims()` / `pub(crate) scale()` (доступны из stats.rs, тот же крейт), `Unit::Scalar` const.
- `crates/canvas-core/Cargo.toml` `[features]` — пустые `stats = []`/`parallel = []` (CP0); FR-063 активирует `stats = ["dep:statrs","dep:rand","dep:rand_chacha","dep:rand_distr"]`.
- `docs/plans/adr-0008-wave-s-plan.md` — план волны S, §5.3 (eval_call arm за фичей), §5.6 (feature-флаги), §5.7 (детерминизм), §5.8 (wasm). S0 — deps-волна (предшествует S1/FR-063).
- `docs/architecture/math-computing-stack.md` §4.2 (слой L2 доменных функций), §9 (M2/S1 — stats-слой), §5.6.2 (запрет `thread_rng`), §7.2 (allowlist лицензий deps).
- `docs/plans/product-roadmap.md` §4.5 (S1 — stats-слой в дорожной карте).
- `docs/DEPENDENCIES.md` §3 — статус «в работе» для `statrs`/`rand`/`rand_chacha`/`rand_distr`; после активации → §2.
- `docs/change-requests/fr-015-domain-units-queueing.md` — образец паттерна доменных функций (FR-063 зеркалит структуру).
- `docs/change-requests/fr-021-numi-input-hints.md` — паттерн FN_HINTS parity-теста.
- `docs/change-requests/fr-017-what-if-scenarios.md` — потребитель ДИ (`ci_mean`) поверх сценариев.
- `docs/change-requests/fr-027-templates-ue-pa.md` — потребитель stats-функций в финансовых/UE-шаблонах.
- `docs/change-requests/fr-066-...` (когда появится) — M5 Monte Carlo, потребитель RNG/распределений и контракта сида.
- Аудит репо на коммите `71f483b`/`28b4dc8` (2026-09-24) — источник точных ссылок `file:line` в этом документе.
- Внешние: `statrs` 0.17 (MIT), `rand` 0.8 (MIT OR Apache-2.0), `rand_chacha` 0.3 (MIT OR Apache-2.0), `rand_distr` 0.4 (MIT OR Apache-2.0) — crates.io; `ChaCha8Rng::seed_from_u64` — `rand_chacha` API.
