# FR-064: Сценарный воркер — вынос пересчёта `propagate_with_lines` с UI-треда (double buffer P1) + FR-017 v2 (freeze/сравнение сценариев)

- **Статус:** выявлено (план)
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** агент (планирование)
- **Источник:** план реализации ADR-0008 волна S (`docs/plans/adr-0008-wave-s-plan.md`), по запросу владельца 2026-09-24
- **Связанные задачи:** ADR-0008; `docs/architecture/math-computing-stack.md` §5.2 (P1 worker + double buffer), §5.6 детерминизм, §9 (M3/S2); `docs/plans/product-roadmap.md` §4.5 (S2); FR-014 (propagator), FR-017 (what-if v1 — база; v2 freeze/сравнение — этот FR), FR-029 (порты значений), FR-065 (M4 параллелизм — контракт на сигнатуру `propagate_with_lines`), `AGENTS.md` (фолбэк + warn); `docs/DEPENDENCIES.md`
- **Создан:** 2026-09-24
- **Обновлён:** 2026-09-24
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Тяжёлый пересчёт потока `flow::propagate_with_lines` (baseline + active
what-if) выносится с главного (UI) треда на отдельный воркер-тред с двойной
буферизацией результатов (архдок §5.2, вариант P1). На UI-треде остаются
только лёгкие операции O(N) — `expr_results`, `flow_changed_nodes` diff,
`analyze::analyze`, сборка `bundles` — которые по-прежнему вызываются в
`SceneState::recompute_flow` после получения снимка `FlowSolutions` от
воркера. Цель: сценарные пакеты из 10–20 и более прогонов (или рост графа к
тяжёлым доменам M5) перестают влезать в кадр на UI-треде и больше не
вызывают «проглатывание» кадров/лагов при reval после каждой правки.

Вторая часть FR — **FR-017 v2 (freeze/сравнение)**: заморозка активного
сценария в `Arc<FlowSolutions>`-снимок и сравнение снимков двух и более
сценариев в таблице «переменная | База | С1 | С2» с дельтами по `outputs` и
`lines`. MCP-инструменты `whatif_*` (FR-017 v1, CP6) остаются без изменений
— новые MCP не добавляются.

**Live-инвариант сохранён** (правка → результат в пределах 1–2 кадров); при
отказе/таймауте/панике воркера — синхронный пересчёт на UI-треде + `warn`
(правило `AGENTS.md` «фолбэк + warn»).

## Влияние (Impact)

| Объект | Что меняется | Где в документе |
|---|---|---|
| `SceneState` | Воркер-тред + double buffer `Arc<RwLock<FlowSolutions>>`; `recompute_flow` остаётся единственным редактором, но тяжёлый вызов `propagate_with_lines` уезжает на воркер | `docs/architecture/math-computing-stack.md` §5.2; план §5.4 |
| `AppEvent` | Новый вариант `AppEvent::FlowReady { solutions: Arc<FlowSolutions>, kind: FlowKind }` для wake-up UI-треда | план §2.2, §5.8 |
| `canvas-app/src/main.rs` | Spawn воркера по образцу `McpPipeServer::spawn`/`ThumbService::spawn`/`WatchService::spawn`/`SearchService::spawn` | план §2.2 |
| `canvas-app/src/app.rs` | Read-path рендера читает `Arc<FlowSolutions>` (double buffer) вместо owned `flow_baseline`/`flow_active` | план §5.4 |
| `whatif` (`crates/canvas-core/src/whatif.rs`) | `ScenarioGrid` (если нужна матрица override-наборов для P2) + freeze-снимки `Arc<FlowSolutions>` + diff `outputs`/`lines` | FR-017 v2 (этот FR) |
| MCP (`canvas-mcp`/`canvas-scene/src/mcp.rs`) | **Без изменений** — 9 инструментов `whatif_*` уже есть (FR-017 v1, CP6); M3 не добавляет новые | `canvas-mcp/src/lib.rs:425–481` |
| WASM (`wasm32-wasip1`) | Воркер desktop-only: `cfg(not(target_arch = "wasm32"))` + sync-фолбэк + `warn` на wasm; гейты `wasm_gate.sh`/`mcp_wasm_gate.sh` обязаны быть зелёными | план §5.8 |

## Анализ (Root Cause)

Сегодня весь пересчёт выполняется **синхронно на UI-треде** — воркера,
`Arc`/`ArcSwap` и двойной буферизации не существует:

- `SceneState::recompute_flow(&mut self)` — `crates/canvas-scene/src/scene.rs:389`
  (мигрировал из `main.rs:~607`). Метод вызывается из ~20 точек в
  `canvas-app/src/app.rs` и после каждой мутирующей MCP-команды
  (`crates/canvas-scene/src/mcp.rs`).
- Внутри `recompute_flow` (scene.rs:389–640): (1) `revision = revision.wrapping_add(1)`;
    (2) снимок `prev_results`/`prev_solutions` для change-detection;
    (3) `flow::propagate_with_lines(&canvas, &WhatIfOverrides::default())` → baseline;
        при `CycleError` — фолбэк к per-node `recompute_all_expr`, `flow_cycle`, early return;
    (4) `active_whatif_overrides()` → `(whatif, stale)`;
    (5) если overrides непусты — `propagate_with_lines(&canvas, &whatif)` → active,
        иначе `active = baseline.clone()`;
    (6) `flow_baseline = baseline; flow_active = active;`
    (7) `expr_results`; (8) `flow_changed_nodes` diff;
    (9) `analysis = analyze::analyze(&canvas, solutions, &AnalysisConfig::default())`;
    (10) `expr_line_results`, `param_spills`, `auto_rows`, `unmapped_edges`,
         `whatif_nodes`, `bundles` (`EdgeBundleIndex::build`).
- `SceneState.canvas: Canvas` — владеет напрямую (`scene.rs:207`, НЕ `Arc`);
  `flow_baseline`/`flow_active` — owned `FlowSolutions` (НЕ `Arc`). Рендер
  читает `scene.expr_results`/`scene.flow_active` через `&App` на UI-треде.
- **Существующий паттерн воркеров** (образец для M3): `McpPipeServer::spawn`,
  `ThumbService::spawn`, `WatchService::spawn`, `SearchService::spawn` —
  `canvas-app/src/main.rs`; все используют `EventLoopProxy<AppEvent>` для
  wake-up UI-треда. M3 добавляет `AppEvent::FlowReady { solutions: Arc<FlowSolutions>, kind: FlowKind }`.
- `flow::propagate_with_lines(canvas: &Canvas, whatif: &WhatIfOverrides) -> Result<FlowSolutions, CycleError>`
  — `crates/canvas-core/src/flow.rs:394` (тонкая обёртка над
  `propagate_with_lines_data` — `flow.rs:405`). Сигнатура стабильна.
- `WhatIfOverrides { line_exprs: HashMap<(String,usize), String>, node_values: HashMap<String, Value> }`
  — `flow.rs:235`. `Scenario { name, line_exprs }` —
  `crates/canvas-core/src/whatif.rs:21`. Персистентность
  `canvas.extra["canvasdesk"]["whatif"]` — `whatif.rs:39` (raw JSON, паттерн
  для M5 engine-метаданных).
- `FlowSolutions` — `flow.rs:353`, `Clone + Default` (безопасен для `Arc` swap).
- 9 MCP-инструментов `whatif_*` (FR-017 v1, CP6): `whatif_set_override`,
  `whatif_set_param`, `whatif_scenario_list`, `whatif_scenario_create`,
  `whatif_scenario_delete`, `whatif_scenario_activate`, `whatif_deltas`,
  `whatif_apply`, `whatif_reset` — `canvas-mcp/src/lib.rs:425–481`,
  диспетчер `canvas-scene/src/mcp.rs:924–1113`. Лимит 3 сценария.
- WASM: `std::thread` НЕ работает на `wasm32-wasip1` (`scripts/wasm_gate.sh:48`,
  `mcp_wasm_gate.sh:39`, `RUST_TEST_THREADS=1`).

**Чего нет:** воркер-треда, `Arc<Canvas>` (или эквивалента), `Arc<RwLock<FlowSolutions>>`,
`AppEvent::FlowReady`, freeze-снимков сценариев, таблицы сравнения сценариев.

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Воркер `FlowWorker` | `crates/canvas-scene/src/scene.rs` (или новый модуль `worker.rs` в `canvas-scene`) | `std::thread::spawn` + `std::sync::mpsc`; воркер получает `(Arc<Canvas>, WhatIfOverrides)`, возвращает `Result<FlowSolutions, CycleError>`; `cfg(not(target_arch = "wasm32"))`. Spawn по образцу `McpPipeServer::spawn` (`canvas-app/src/main.rs`) |
| Double buffer | `crates/canvas-scene/src/scene.rs` | `Arc<RwLock<FlowSolutions>>` v1 (без `arc_swap` — архдок §5.2 «без новых зависимостей»); писатель — воркер, читатель — рендер. `arc_swap` — опция v2 по бенчмарку |
| `AppEvent::FlowReady` | `crates/canvas-app/src/main.rs` | Новый вариант `AppEvent::FlowReady { solutions: Arc<FlowSolutions>, kind: FlowKind }`; wake-up через `EventLoopProxy<AppEvent>` (существующий паттерн) |
| Spawn воркера | `crates/canvas-app/src/main.rs` | По образцу `McpPipeServer::spawn`/`ThumbService::spawn`/`WatchService::spawn`/`SearchService::spawn` |
| Read-path рендера | `crates/canvas-app/src/app.rs` | Чтение `Arc<FlowSolutions>` через `RwLock::read()` вместо owned `flow_baseline`/`flow_active`; fallback на sync-значения, если воркер ещё не отдал снимок |
| `recompute_flow` | `crates/canvas-scene/src/scene.rs:389` | Остаётся единственным редактором (план §5.4); тяжёлый вызов `propagate_with_lines` отправляется воркеру; выводка O(N) (`expr_results`/`analysis`/`bundles`) — на UI-треде |
| Sync-фолбэк | `crates/canvas-scene/src/scene.rs` | При отказе/таймауте/панике воркера и на wasm — синхронный `propagate_with_lines` на UI-треде + `warn` (правило `AGENTS.md`) |
| `ScenarioGrid` (если нужно) | `crates/canvas-core/src/whatif.rs` | Матрица override-наборов для P2 — только если требуется пакетный прогон > 3 сценариев |
| Freeze-снимки | `crates/canvas-core/src/whatif.rs` | `Arc<FlowSolutions>`-снимок активного сценария; персистентность по паттерну `canvas.extra["canvasdesk"]["whatif"]` (`whatif.rs:39`) |
| Diff/сравнение | `crates/canvas-core/src/whatif.rs` | Diff `outputs`/`lines` между freeze-снимками; таблица «переменная \| База \| С1 \| С2»; split-view канваса — v3 |
| `worker_smoke.rs` | `crates/canvas-scene/tests/worker_smoke.rs` (новый) | Smoke-тест «правка → результат через воркер ≤ 2 кадра»; fallback-тест (воркер паникует → sync-результат идентичный); freeze/diff e2e |

## Контракты на стыках

Контракты заморожены планом §5; нарушение = конфликт слияния на ревью.

- **§5.1 — сигнатура `flow::propagate_with_lines` НЕИЗМЕННА.** M3 только
  вызывает её из воркера — не меняет сигнатуру и не трогает внутренности
  (`flow.rs:394`, обёртка над `propagate_with_lines_data` — `flow.rs:405`).
  M4 меняет только внутреннюю реализацию (`topo_levels` + `par_iter`); M5
  добавляет новую sibling `propagate_monte_carlo`. M3 не видит их изменений.
- **§5.4 — `SceneState::recompute_flow` — M3 единственный редактор.** Метод
  по-прежнему производит `flow_baseline` + `flow_active` + `expr_results` +
  `analysis` + `expr_line_results` + `param_spills` + `auto_rows` +
  `unmapped_edges` + `bundles` (выводка O(N), остаётся на UI-треде). На
  воркер уходит ТОЛЬКО `propagate_with_lines`. M4/M5 `scene.rs` не трогают.
- **§5.8 — WASM-совместимость.** Воркер `cfg(not(target_arch = "wasm32"))`;
  на `wasm32-wasip1` — sync-фолбэк (синхронный `propagate_with_lines` на
  UI-треде) + `warn`. Гейты `scripts/wasm_gate.sh --check` и
  `scripts/mcp_wasm_gate.sh --check` обязаны быть зелёными на каждом
  коммите. `std::thread` и `rayon` на wasip1 неработоспособны
  (`wasm_gate.sh:48`, `mcp_wasm_gate.sh:39`, `RUST_TEST_THREADS=1`).

**Что НЕ трогать:** `crates/canvas-core/src/flow.rs` (только вызывает
`propagate_with_lines`), `crates/canvas-core/src/expr.rs`,
`crates/canvas-core/src/expr/stats.rs`, `crates/canvas-mcp` (инструменты
`whatif_*` уже есть — `canvas-mcp/src/lib.rs:425–481`).

## Фазы и проверяемые коммиты

Каждый коммит = зелёный гейт. Базовые гейты (все коммиты): `cargo fmt --check`,
`cargo clippy --workspace -D warnings`, `cargo test --workspace`,
`scripts/wasm_gate.sh --check`, `scripts/mcp_wasm_gate.sh --check`,
`cargo deny check`.

### P1 — воркер + double buffer

- **Коммит:** `feat(scene): FlowWorker spawn + AppEvent::FlowReady, sync fallback (FR-064 P1)`
- Воркер `std::thread::spawn` + `std::sync::mpsc`; получает
  `(Arc<Canvas>, WhatIfOverrides)`, возвращает `Result<FlowSolutions, CycleError>`.
- Double buffer `Arc<RwLock<FlowSolutions>>` v1 (без `arc_swap` — архдок §5.2
  «без новых зависимостей»; `arc_swap` — опция v2 по бенчмарку).
- `cfg(not(target_arch = "wasm32"))` + sync-фолбэк на wasm + `warn`.
- Wake через `EventLoopProxy<AppEvent>` (`AppEvent::FlowReady { solutions, kind }`).
- **Гейты:** smoke-тест «правка → результат через воркер ≤ 2 кадра»;
  `cargo test --workspace`; `scripts/wasm_gate.sh --check` (sync-фолбэк на
  wasm); `scripts/mcp_wasm_gate.sh --check`; `cargo clippy -D warnings`.

### P2 — FR-017 v2 freeze/сравнение

- **Коммит:** `feat(scene): FR-017 v2 — scenario freeze + comparison table (FR-064 P2)`
- Freeze сценария = `Arc<FlowSolutions>`-снимок активного; персистентность
  по паттерну `canvas.extra["canvasdesk"]["whatif"]` (`whatif.rs:39`).
- Сравнение = diff `outputs`/`lines` между снимками; таблица
  «переменная | База | С1 | С2» с дельтами.
- Split-view канваса — v3 (вне этого FR).
- `ScenarioGrid` в `whatif.rs` — только если нужна матрица override-наборов.
- **Гейты:** e2e на эталоне ADR-0006 №2 (смена `rps` → дельты downstream);
  freeze 2 сценариев → таблица сравнения с дельтами; убийство воркера →
  sync-фолбэк + `warn` (fallback-тест).

## Ограничения для агента-реализатора

- Воркер `cfg(not(target_arch = "wasm32"))`; sync-фолбэк на wasm + `warn`
  (правило `AGENTS.md` «фолбэк + warn»).
- Double buffer v1 — `Arc<RwLock<FlowSolutions>>` из `std`; **БЕЗ `arc_swap`**
  (архдок §5.2 «без новых зависимостей»; `arc_swap` — опция v2, по бенчмарку).
- Wake-up UI-треда — **существующий паттерн** `EventLoopProxy<AppEvent>`
  (образец: `McpPipeServer::spawn`, `ThumbService::spawn`,
  `WatchService::spawn`, `SearchService::spawn` — `canvas-app/src/main.rs`).
  Новый `AppEvent::FlowReady { solutions: Arc<FlowSolutions>, kind: FlowKind }`.
- **Live-инвариант сохранён**: правка → результат в пределах 1–2 кадров.
- Деградация = sync-пересчёт + `warn` (не молчаливое падение; не «зависший
  UI»).
- Выводка O(N) (`expr_results`, `analysis`, `bundles`, `flow_changed_nodes`
  diff, `expr_line_results`, `param_spills`, `auto_rows`, `unmapped_edges`,
  `whatif_nodes`) **остаётся на UI-треде** — на воркер уходит ТОЛЬКО
  `propagate_with_lines` (план §5.4).
- Сигнатуру `propagate_with_lines`/`topo_sort` — НЕ менять (контракт §5.1; M4
  меняет только внутренности).
- MCP-инструменты `whatif_*` (FR-017 v1, CP6) — **не трогать**, новые не
  добавлять (`canvas-mcp/src/lib.rs:425–481`).
- `flow.rs`, `expr.rs`, `expr/stats.rs`, `canvas-mcp` — **только читать**.
- Файлы владения: `crates/canvas-scene/src/scene.rs` (`recompute_flow`),
  `crates/canvas-app/src/main.rs` (`AppEvent::FlowReady` + spawn воркера),
  `crates/canvas-app/src/app.rs` (read-path рендера),
  `crates/canvas-core/src/whatif.rs` (`ScenarioGrid` для P2, если нужно),
  `crates/canvas-scene/tests/worker_smoke.rs` (новый).
- Комментарии/доки на русском (правило `AGENTS.md`); conventional commits
  (задача = сессия = коммит); никаких `unwrap`/`expect` в production-путях.

## Наглядная проверка (5 минут)

**Демо (для владельца, без чтения кода):**

1. Эталон ADR-0006 №2, 20 прогонов сценарной сетки на воркере → UI не
   тормозит (60 fps сохранён).
2. Freeze 2 сценариев → таблица сравнения с дельтами по `outputs`/`lines`
   («переменная | База | С1 | С2»).
3. Принудительно убить воркер → пересчёт падает на sync-фолбэк + toast-warn,
   канвас жив, числа идентичны.

**Автотесты (CI-гейты):**

- worker smoke: «правка → результат через воркер ≤ 2 кадра».
- freeze/diff e2e: эталон №2, смена `rps` → дельты downstream, freeze 2
  сценариев → таблица.
- fallback-тест: воркер паникует → sync-результат идентичный.
- `scripts/wasm_gate.sh --check` (sync-фолбэк на wasm).
- `scripts/mcp_wasm_gate.sh --check`.

## Проверка (Verification)

Чек-лист (статус → `выполнено` только при всех зелёных):

- [ ] `cargo test --workspace` — зелёный (включая `worker_smoke.rs`).
- [ ] `cargo clippy --workspace -D warnings` — зелёный.
- [ ] `scripts/wasm_gate.sh --check` — зелёный (sync-фолбэк на wasm).
- [ ] `scripts/mcp_wasm_gate.sh --check` — зелёный (sync-фолбэк на wasm).
- [ ] `cargo deny check` — зелёный (без новых зависимостей в P1).
- [ ] worker smoke: правка → результат через воркер ≤ 2 кадра.
- [ ] freeze/diff e2e: эталон №2, freeze 2 сценариев → таблица с дельтами.
- [ ] fallback-тест: воркер паникует → sync-результат побитово идентичный.
- [ ] Live-инвариант: правка → результат в пределах 1–2 кадров.
- [ ] UI не тормозит на 20 прогонах сценарной сетки (60 fps).
- [ ] Документы точек входа обновлены (см. ниже).

## Точки входа

- `docs/SPEC.md` §6.3 — бюджеты пересчёта (worker против sync; live-инвариант).
- `user-docs/calculations.md` — раздел what-if (сценарии, freeze, сравнение).
- `docs/interface-objects/node.md` — what-if (override, freeze, таблица
  сравнения).
- `docs/architecture/math-computing-stack.md` §5.2, §5.6 — после реализации
  обновить статус P1 с «план» на «реализовано».
- `docs/plans/product-roadmap.md` §4.5 (S2) — после merge перевести FR-064 в
  «в работе» → «выполнено».
- `docs/plans/adr-0008-wave-s-plan.md` §7 — отметка о зелёных коммитах P1/P2.
- `AGENTS.md` — пример фолбэк + warn (sync-пересчёт при отказе воркера).

## История изменений

- `2026-09-24` — агент: создан документ (план, статус `выявлено`). Декомпозиция
  M3/S2 на P1 (воркер + double buffer) и P2 (FR-017 v2 freeze/сравнение);
  контракты §5.1/§5.4/§5.8; проверяемые коммиты; наглядная проверка;
  код-ссылки (`scene.rs:389`, `flow.rs:394`, `whatif.rs:21`,
  `canvas-mcp/src/lib.rs:425–481`, `wasm_gate.sh:48`). Исполнение — после
  гейта Go продуктового роадмапа §4.4/§4.5.

## Источники истины

- `crates/canvas-scene/src/scene.rs:389` — `SceneState::recompute_flow`
  (мигрировал из `main.rs:~607`).
- `crates/canvas-scene/src/scene.rs:207` — `SceneState.canvas: Canvas` (owned, не `Arc`).
- `crates/canvas-core/src/flow.rs:394` — `propagate_with_lines` (сигнатура стабильна).
- `crates/canvas-core/src/flow.rs:405` — `propagate_with_lines_data` (внутренняя, не трогать).
- `crates/canvas-core/src/flow.rs:235` — `WhatIfOverrides`.
- `crates/canvas-core/src/flow.rs:353` — `FlowSolutions` (`Clone + Default`).
- `crates/canvas-core/src/whatif.rs:21` — `Scenario { name, line_exprs }`.
- `crates/canvas-core/src/whatif.rs:39` — персистентность `canvas.extra["canvasdesk"]["whatif"]`.
- `crates/canvas-mcp/src/lib.rs:425–481` — 9 инструментов `whatif_*` (FR-017 v1, CP6).
- `crates/canvas-scene/src/mcp.rs:924–1113` — диспетчер `whatif_*` MCP.
- `crates/canvas-app/src/main.rs` — `McpPipeServer::spawn`, `ThumbService::spawn`,
  `WatchService::spawn`, `SearchService::spawn` (образец воркеров +
  `EventLoopProxy<AppEvent>`).
- `crates/canvas-app/src/app.rs` — ~20 точек вызова `recompute_flow`; read-path рендера.
- `scripts/wasm_gate.sh:48`, `scripts/mcp_wasm_gate.sh:39` — `wasm32-wasip1` ограничения
  (`std::thread`, `RUST_TEST_THREADS=1`).
- `docs/adr/adr-0008-math-computing-stack.md` — решение (вариант B, этапы M0–M6).
- `docs/architecture/math-computing-stack.md` §5.2 (P1 worker + double buffer),
  §5.6 (детерминизм), §9 (дорожная карта M0–M6).
- `docs/plans/product-roadmap.md` §4.5 (S2), §4.4 (гейт Go), §9 (CP0–CP7).
- `docs/plans/adr-0008-wave-s-plan.md` §2.2 (рецепт M3), §5.1/§5.4/§5.8
  (контракты), §7 (проверяемые коммиты FR-064 P1/P2).
- `docs/change-requests/fr-017-what-if-scenarios.md` — FR-017 v1 (база; v2 freeze/сравнение — этот FR).
- `AGENTS.md` — правило «фолбэк + warn», conventional commits, TDD, комментарии на русском.
- `docs/DEPENDENCIES.md` — реестр зависимостей (`arc_swap` — кандидат v2 по бенчмарку).
