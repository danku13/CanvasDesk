## 2026-09-19 — Реализация MW2 (FR-037: мост canvas-mcp под wasm)

- **Задача (владелец):** «распланируй и реализуй MW2 с 3 сабагентами» —
  план `docs/plans/mw2-wasm-bridge.md`: мост `canvas-mcp` под wasm
  (`run_stdio` → `run_stdio_with_transport`, гварды автоспавн-тестов,
  включение моста в wasm-гейты); MW2 без MW1; ветка
  `feature/fr-037-mw2-wasm-bridge`, один коммит.
- **Сделано** (MW2-a ∥ MW2-b → верификация/коммит MW2-c):
  - **`crates/canvas-mcp/src/lib.rs`** (MW2-a): тело stdio-цикла
    (stdin → `split_frames` → `handle_input` → stdout; буфер 8192,
    EOF = штатный выход) перенесено в
    `pub fn run_stdio_with_transport<T, R>(transport: Option<T>,
    reconnect: R)` (`T: AppTransport`, `R: FnMut(&mut Option<T>)`) —
    хук `reconnect(&mut transport)` перед каждым пакетом вместо
    cfg-вызова `refresh_transport`; `run_stdio(&[String])` — тонкая
    обёртка (автоспавн FR-035 / offline ADR-0009 / хук reconnect
    FR-034), pub-сигнатура неизменна, `main.rs` не тронут; гварды
    `#[cfg(all(unix, not(target_arch = "wasm32")))]`
    (`spawn_service_command_isolates_stdio`) и
    `#[cfg(not(target_arch = "wasm32"))]`
    (`autosprawn_target_prefers_sibling_gui_for_standalone_bridge`) —
    15 тестов, имена/ассерты не менялись; мин-правка MW2-c: убран
    лишний `mut` у `transport` в обёртке (clippy `unused_mut` после
    выделения цикла, семантика прежняя).
  - **`scripts/wasm_gate.sh`** (MW2-b): `CRATES` += `-p canvas-mcp`
    (ступени 1–2); ступень 3 — явный список `-p canvas-core
    -p canvas-mcp` (не `$CRATES`: render/widgets под wasip1 не
    тестируются — wgpu-тесты требуют GPU-адаптер); шапка/echo
    синхронизированы.
  - **`.github/workflows/ci.yml`** (MW2-b): джоба `wasm-check` —
    `run` += `-p canvas-mcp`, имя шага и комментарий (FR-037/MW2)
    актуализированы; `targets`/прочие джобы не тронуты.
  - **`docs/plans/mw2-wasm-bridge.md`** (оркестратор): план MW2 —
    декомпозиция MW2-a/b/c, сигнатура с хуком reconnect (§3), риски,
    критерии приёмки.
  - **`docs/change-requests/fr-037-mcp-wasm-verification.md`**:
    строка MW2 «Выполнено 2026-09-19» + Changelog.
- **Гейты:** `cargo fmt --check` ✓; `cargo clippy -p canvas-mcp
  --all-targets -- -D warnings` ✓; `cargo test -p canvas-mcp` нативно —
  15 passed/0 failed (регресс FR-008/034/035 — ноль);
  `cargo check --target wasm32-unknown-unknown -p canvas-mcp` ✓ (R1);
  `RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-mcp`
  — 13 passed/0 failed в wasmtime (R2; 2 автоспавн-теста исключены
  гвардами); `scripts/wasm_gate.sh` — полный зелёный прогон ступеней
  1–3 (canvas-core 318 + canvas-mcp 13 тестов в wasmtime).
- **Решения:** сигнатура выделенного цикла — с хуком
  `R: FnMut(&mut Option<T>)`: reconnect (FR-034) платформенный и не
  прячется в трейт `AppTransport` (§3 плана); MW3 (headless) вызовет
  `run_stdio_with_transport(Some(session), |_| {})`. Контингенция
  wasmtime-флагов не понадобилась (wasmtime 48.0.2 принял runner из
  `.cargo/config.toml`); после ребейза на апстрим (CP6/FR-017, main
  `83d43a9`) применена вторая контингенция плана (§4 MW2-a п.4): cfg
  хелперов `spawn_service_command`/`autosprawn_target` расширен до
  `#[cfg(any(windows, all(test, not(target_arch = "wasm32"))))]` —
  dead_code-warning'и под wasip1-test устранены, `std::process` полностью
  вне wasm-сборки; итоговый полный гейт — 0 warnings.

---

## 2026-09-18 — Реализация CP5 (FR-016: индикаторы узких мест, волна B1)

- **Задача (владелец):** «Реализуй CP5» — по `docs/plans/product-roadmap.md`
  §9: FR-016 (волна B1) — bottleneck/queue-risk индикаторы по ρ и W из
  уже посчитанного потока; гейт — «эталон №1 визуально exposes узкие
  места без чтения чисел в нодах», автотесты — юниты классификации
  (пороги ρ/L) + e2e на эталоне.
- **Сделано:**
  - **`canvas-core/src/analyze.rs`** (новый, чистая функция — образец
    validate.rs): `AnalysisFlags {utilization, queue_length, wait_sec,
    severity}` / `Severity {None, Warn, Critical, Overload}` /
    `AnalysisConfig` (пороги 0.7/0.9, 100 ms/1 s, 1/10 — дефолты
    документа FR-016, вынесены в данные — инвариант 2) / `badge_text`
    (строка бейджа — канвас = MCP, инвариант 4) / `has_risk` (гейт
    авто-включения). Детекция v1 — актуализация под скалярный движок
    (документ писал про `Value::Struct` от mm1 — это v2 движка):
    `Err(Overload{rho})` → Overload (ρ из ошибки); named-выход
    `utilization` (Percent, доля, ≥1 → Overload) / Percent-значение
    ноды → ρ; Time-значение → W; named `wait_time`/`queue_length` —
    точки расширения; Count сам по себе НЕ очередь (анти-ложные
    срабатывания на innocent «10 req»); SLA — v2. Unit-хелперы
    expr.rs (`Atom::new`, `Unit::dims/scale`) подняты до pub(crate).
  - **Манифесты**: 13 queue-шаблонов (cdn, tcp-lb, lb, api-gateway,
    auth-service, http-endpoint, cache-redis, db-sql-master/-replica,
    queue-kafka, worker, graphql, grpc-service) + именованный выход
    `utilization` (зеркалит аргументы своего mm1), версии +minor —
    ρ стал данными потока через инфраструктуру FR-029.
  - **canvas-app**: `SceneState.analysis` (runtime-кэш, хвост
    `recompute_flow` — покрывает все 20+ точек мутаций); настройка
    `bottleneck_overlay` (config.toml, персистентная, дефолт выкл);
    тоглы — Ctrl+B (канвас-уровень, кириллица «и»; Bold в редакторе
    не конфликтует — маршрутизация выше), пункт меню канваса «Узкие
    места (Ctrl+B)» (6-й, ✓), строка панели настроек «Индикаторы
    узких мест»; авто-включение один раз за запуск при первом риске
    (Warn+) с тостом, ручной тогл глушит авто до перезапуска.
  - **Рендер**: рамка серьёзности на карточке (приоритет шейдера
    selected > broken > border.a; выделенная нода с риском — внешнее
    кольцо `analysis_ring_instance`, обе рамки видны — ручная
    приёмка); бейдж — моно-текст `badge_text` цветом серьёзности над
    правым верхним углом (шапка занята заголовком/иконкой —
    адаптация документирована в FR-016 changelog); LOD: бейджи ≥ 0.6
    эфф. зума (физическая читаемость, как titles_visible), рамки
    Warn/Critical ≥ 0.25, Overload — всегда; цвета/тексты — по темам
    (контраст CR-007: тёмно-красный #7A0010 из документа читаем на
    светлом, на тёмном — яркие аналоги); SceneView/TitleFrame +
    поля `analysis`/`analysis_overlay`/`analysis_badges`.
  - **MCP**: инструмент `analyze_bottlenecks` (TOOLS 26 → 27) —
    {nodes: [{id, severity, utilization?, queue_length?, wait_sec?,
    badge}], thresholds}; свежий пересчёт как flow_recalc; чтение
    (e2e проверяет канвас/undo байт-в-байт).
  - **Тесты** (+22): 16 юнит analyze (все уровни, пороги-как-данные,
    ρ из ошибки, named-выходы, детерминизм, сериализация, бейджи);
    4 юнит рендера (палитры серьёзностей по темам, LOD-пороги,
    геометрия кольца, текстовые цвета); e2e
    `analyze_bottlenecks_reference_and_growth` — мини-эталон №1
    (ADR-0006): базовая линия CDN ρ 0.417/W 34.29 ms (здоров),
    node_update_text DAU ×2 → Warn ρ 0.833 + бейдж «83% · W: 120 ms»,
    DAU ×5.35 → Overload ρ 2.23 (ветка C эталона №2) ±1 %, GW
    здоров (ρ 0.334), read-only-инварианты, синхронность
    runtime-кэша. Обновлены пины версий манифестов (lb 1.1→1.2) и
    счётчики меню (6)/инструментов (27).
  - **Доки**: FR-016 → «реализовано (v1)» + Changelog (актуализация
    детекции, бейдж над карточкой, цвета по темам); index-cr-fr;
    roadmap Changelog (4); SPEC §4 (27) + §13 analyze_bottlenecks;
    ACCEPTANCE §25 (FR-016.1–9); user-docs hotkeys (Ctrl+B) /
    interface (раздел «Индикаторы узких мест») / calculations
    (связка ρ→оверлей); interface-objects/node.md §5 (состояние
    серьёзности).
- **Гейты:** cargo fmt ✓; clippy --workspace --all-targets -D warnings ✓;
  cargo test --workspace — 1034 passed / 0 failed. Инцидент: линковка
  упала ld signal 7 — диск 100 % (target/ 9.1G, тот же диагноз, что в
  записи FR-035); cargo clean → прогон заново.
- **Открытые пункты (следующие шаги):** CP4 = R5 (рецепт агента
  user-docs/agent-recipe.md — не начат), CP6 = FR-017 v1 (what-if —
  переиспользует analyze + AnalysisConfig на override-результатах);
  v2-хвосты FR-016: SLA-сравнение, кастомные пороги в конфиге,
  паттерны рамки для цветовой слепоты, heatmap.

---

## 2026-09-18 — Реализация CP0 (волна 0) + CP1 (FR-029 порты значений)

- **Задача (владелец):** на основании product-roadmap.md реализовать CP0 и CP1.
- **Сделано:**

  **CP0 (волна 0 — гигиена):**
  - `deny.toml` (cargo-deny): allowlist §7.2 архдока + `Apache-2.0 WITH
    LLVM-exception` с обоснованием (строго пермиссивнее Apache-2.0), bans
    (wildcards deny, множественные версии — warn), sources (только crates.io),
    advisories: yanked deny, три осознанных ignore unmaintained рендер-стека
    (RUSTSEC-2024-0436 paste / RUSTSEC-2026-0206 rustybuzz /
    RUSTSEC-2026-0192 ttf-parser) с планом миграции после гейта Go;
  - `publish = false` всем 7 workspace-крейтам (закрытый B2B-продукт,
    защита от случайной публикации; требуется private-ignore в cargo-deny);
  - CI (ci.yml): джоба `licenses` (EmbarkStudios/cargo-deny-action@v2) на
    каждый пуш/PR; артефактные сборки — `cargo auditable build`
    (SBOM-in-binary); build-all.yml: джоба `notices-sbom` — регенерация
    THIRD-PARTY-NOTICES + дрифт-контроль + артефакт;
  - `about.toml` + `docs/templates/third-party-notices.hbs` + сгенерированный
    `THIRD-PARTY-NOTICES.md` (382 крейта, все лицензии пермиссивные);
  - `docs/DEPENDENCIES.md` — реестр: first-party, 16 прямых прод-зависимостей
    с лицензиями, кандидаты волны S из архдока §4.2, долг сопровождения
    (unmaintained), рецепты (notices/SBOM/добавление зависимости);
  - cargo-фичи `stats`/`parallel` в canvas-core (пустые гейты волны S,
    archdoc M1); `include_dir`/`image` централизованы в
    [workspace.dependencies] (SPEC §3);
  - триаж волны 0: FR-023 → «выполнено (v1)» (`7551e12` + CR-010/CR-012),
    FR-024 → «выполнено (v1)» (`74b4333`, расширен FR-030), CR-005 →
    бэклог §7 (точечно по мере боли); changelog-записи в документах +
    строки index-cr-fr.md.

  **CP1 (FR-029 — порты значений, волна A1):**
  - `model.rs`: `Edge.from_output`/`to_param` (сериализация `fromOutput`/
    `toParam`, мягкое чтение как `fromLine`, round-trip старых `.canvas`
    байт-в-байт — тесты); перепривязка from-конца сбрасывает `from_output`;
  - `templates.rs`: `OutputSpec { name, unit, source: Line(i)|Expr(s) }`,
    секция `outputs` манифеста (схема 1.1, опциональна — 45 builtin
    валидны), снапшот `canvasdesk.template.outputs` (ключ только при
    непустой секции), перенос outputs в custom-шаблон при «Сохранить
    как шаблон»;
  - `flow.rs`: `FlowSolutions.named` + `warnings`; именованные выходы
    шаблонных нод (Line — из построчных, Expr — в окружении ноды) и
    текстовых (переменные Numi-листа — адресация живёт при сдвиге строк);
    проливание `toParam` ПОСЛЕ локальных параметров («проливание сильнее
    дефолта»), рёбра с `toParam` вне позиционных слотов, конфликты —
    последнее по `canvas.edges` + предупреждение;
  - MCP: `edge_create` v2 (kind/fromLine/fromOutput/toParam, взаимные
    исключения, валидация имён по снапшотам шаблонов/переменным листа),
    новый `edges_list` (23 инструмента), `flow_recalc` v2 (value + outputs
    + lines + warnings), `template_list` отдаёт outputs;
  - манифесты: outputs для cdn, tcp-lb (fan-out auth/feed/media по долям
    — новые параметры), graphql (db_qps/events), db-sql-master
    (replica_load), db-sql-replica, queue-kafka (consume_rate),
    api-gateway, lb, cache-redis;
  - тесты: round-trip/lenient (model), проливание/перекрытие/совместимость
    слотов/сдвиг строк/конфликты/тихая деградация (flow), Instagram MVP
    e2e `mcp_fr029_instagram_mvp_reference` — 12 нод собираются MCP,
    10 адресованных рёбер, oracle ADR-0005 ±1% (avg 555.6 / peak 1388.9 /
    origin 555.6 / auth 83.3 / feed 333.3 / media 138.9 / db_qps 80 /
    events 333.3), правка DAU одним `node_edit` удваивает цепочку;
  - документация: SPEC §5.1 (поля рёбер + outputs снимка), user-docs/
    calculations.md («Проливание в параметры»), fr-029 changelog + статус,
    index, roadmap changelog.

  **Гейты:** `cargo fmt --check` ✓, `cargo clippy --workspace --all-targets
  -D warnings` ✓, `cargo test --workspace` — 981 passed / 0 failed ✓,
  `cargo deny check` — licenses/bans/sources/advisories ok ✓.
- **Коммит:** см. git log — CP0/CP1.
- **Интеграция с CP2 (FR-032, влит параллельным коммитом `1fb63e6`):**
  выполнен чек-лист из changelog FR-032 — `port_contract_issues`
  (E-UNIT/E-PORT-UNKNOWN/E-DOUBLE-INPUT на адресации FR-029),
  W-AMBIGUOUS-SRC с `fromOutput`, W-UNUSED-SLOT без `toParam`-рёбер,
  `mcp_edge_json` с полями портов (edges_list/edge_get), дубликат ветки
  edges_list устранён (TOOLs = 25); гейт A2 — 3 подсаженные ошибки на
  эталоне Instagram → ровно 3 issue (в составе e2e
  `mcp_fr029_instagram_mvp_reference`).
- **Открытые пункты (следующие шаги):** CP3 = FR-033 (graph_apply), CP4 =
  R5 (рецепт агента user-docs/agent-recipe.md), CP5 = FR-016, CP6 =
  FR-017 v1.

---

## 2026-09-18 — CP2: FR-032 v1 — чтение графа + graph_validate (код)

- **Задача (владелец):** реализовать CP2 продуктового роадмапа (`product-roadmap.md`
  §9) — FR-032 (R2, волна A2). Параллельно другой агент ведёт CP0/CP1 (FR-029 —
  порты значений); работа CP2 — в собственном клоне, ветка
  `feature/fr-032-graph-read-validate`.
- **Сделано (v1 — всё, что не зависит от полей FR-029):**
  - `canvas-core/src/validate.rs` (новый, чистая функция без I/O):
    `ValidationIssue {severity, code, node_id, edge_id, message}` — сериализация
    snake_case, коды — стабильный контракт; `validate(&Canvas) ->
    Vec<ValidationIssue>`; реализованы `E-CYCLE` (при цикле отчёт
    ограничивается топологией — без шума слот-предупреждений), `E-OVERLOAD`
    (из `propagate_with_lines`), `W-AMBIGUOUS-SRC` (многолинейный исток без
    `fromLine`), `W-UNUSED-SLOT` (обход дерева формул приёмника: `$N`/`$in`,
    фенсы и проза пропускаются; диагностика G5 — позиционное ребро в шаблонную
    ноду теряется); `E-UNIT`/`E-PORT-UNKNOWN`/`E-DOUBLE-INPUT` — на полях
    FR-029, точка добавления — `port_contract_issues` (контракт в док-комменте);
  - `canvas-mcp`: TOOLS 22 → 25 — `edges_list`, `edge_get`, `graph_validate`
    (схемы + описание контракта кодов); тест схем обновлён;
  - `canvas-app` `mcp_dispatch`: три ветки чтения (валидация не мутирует:
    без undo/автосейва/`recompute_flow`); каноническая схема ребра —
    `mcp_edge_json` (единственное место, куда FR-029 добавит `toParam`/
    `fromOutput`);
  - тесты: 10 юнит в `validate.rs` (чистая сцена → `issues == []`, фиксстура
    на каждый код, `$in`/`$N`-семантика слотов, шаблонная нода, детерминизм,
    контракт сериализации) + 3 e2e `mcp_dispatch` (`mcp_edges_list_and_get`,
    `mcp_graph_validate_clean_and_cycle`, `mcp_graph_validate_overload_and_unused_slot`);
  - дока: SPEC.md §4 (структура: validate.rs, 25 инструментов) + §5.1 (буллет
    MCP-чтения/валидации), ACCEPTANCE.md §21 (чек-лист, сценарии 10–12 —
    «после CP1»), interface-objects/edge.md (строка «Чтение топологии»,
    §6-буллет, источник), FR-032 → «в работе (v1)» + Changelog (включая
    чек-лист интеграции CP1 из 5 пунктов), index-cr-fr.md.
- **Гейты:** fmt/clippy(-D warnings, --all-targets)/test --workspace — зелёные
  (все 36 сюит; ядро 175 тестов, +13 новых).
- **Интеграция CP1 (для агента, вливающего FR-029):** см. Changelog FR-032 —
  заполнить `port_contract_issues`, два условия в `validate()`, поля в
  `mcp_edge_json`, сценарии ACCEPTANCE FR-032.10–12 и гейт A2 «3 подсаженные
  ошибки → ровно 3 issue». До влития CP1 три из кодов гейта A2 определить не
  на чем (контракта портов ещё не существует) — это ожидаемое состояние
  параллельной разработки, не дефект.
- **Коммит/ветка:** `feature/fr-032-graph-read-validate` (база 433e3fa),
  коммит — см. git log (feat(core,mcp,app): FR-032 v1).
---

## 2026-09-18 — FR-033 graph_apply: атомарная батч-композиция (CP3, ветка feature/fr-033-graph-apply)

- **Задача (владелец):** по `docs/plans/product-roadmap.md` реализовать CP3
  (FR-033, волна A3) — параллельно: агент_1 ведёт CP0+CP1 (волна 0 + FR-029),
  агент_2 — CP2 (FR-032). Работа в отдельной ветке для бесконфликтной сборки.
- **Сделано (код):**
  - `canvas-mcp`: инструмент `graph_apply` (TOOLS 22 → 23 на ветке) — схема
    `operations: [Op; 1..=256]`, тег op с 6 вариантами; лимиты в описании;
  - `canvas-app`: `mcp_graph_apply` (транзакция: клон → apply → commit;
    ошибка операции → `{ok:false, op_index, code, message}`, канвас байт-в-байт
    прежний; успех → ровно один undo-шаг + spatial + dirty + recompute_flow),
    `batch_apply_op` (6 операций: node_create_note/file, template_instantiate,
    edge_create с портами FR-029 и валидацией имён/циклов, param_set с правкой
    ровно одной строки и синхронизацией снапшота шаблона, node_move),
    ref-резолв (дубликат ref — E-BAD-OP), `mcp_flow_v2` (flow_recalc v2:
    value/unit/outputs/lines/error);
  - `canvas-core` (минимальный контур FR-029 — необходим гейту CP3, при
    интеграции уступает полной реализации CP1): `Edge.to_param`/`from_output`
    (мягкое чтение, round-trip, сброс при перепривязке истока), `OutputSpec`/
    `OutputSource` в манифесте и снапшоте `TemplateRef`, проливание `to_param`
    поверх локальных параметров («последнее ребро побеждает»), резолв
    `from_output` (Line/Expr) в `propagate_with_lines`, `FlowSolutions.named`;
  - `assets/templates/com.canvasdesk.cdn`: именованный выход `origin` (демо
    мини-эталона; остальные манифесты — за полной реализацией FR-029).
- **Тесты:** +14 (7 `graph_apply_*` в main.rs: oracle e2e ±1 % по эталону №1
  ADR-0006 — 208.33 rps / 34.29 ms / 20.83 rps / 3.20 ms, атомарность,
  ref-правила, param_set, лимиты, undo/redo, цикл; 7 flow-тестов проливания;
  2 round-trip порта в model.rs). Гейты зелёные: fmt, clippy -D warnings,
  test --workspace — 988 тестов, 0 провалов.
- **Документация:** SPEC §13 «MCP-инструменты канваса» (graph_apply: схема,
  лимиты, транзакция, коды ошибок) + счётчик 23 в §4; ACCEPTANCE §21
  (FR-033.1–FR-033.10); fr-033 — статус «реализовано (v1)» + Changelog;
  index-cr-fr — статус.
- **Коммит:** ветка `feature/fr-033-graph-apply` (не main — параллельные
  CP0/CP1/CP2 других агентов; порядок интеграции: CP0/CP1 → CP2 → CP3,
  контур FR-029 в этой ветке уступить ветке CP1).

---

## 2026-09-18 — Критический путь: детализация + FR-032/FR-033 (разметка FR/CR)

- **Задача (владелец):** прописать критический путь с обоснованием, разметить
  и заполнить FR/CR; каждый этап должен быть наглядным в части тестирования.
- **Сделано:**
  - product-roadmap.md: новый §9 «Критический путь: обоснование и тестовая
    наглядность» — CP0–CP7 (правило наглядности «наблюдаемо без чтения кода»,
    обоснование позиции каждого шага, наглядный тест-демо на 5 минут +
    автотесты; блок «почему порядок нельзя переставить»); таблицы §4.2/§5/§6
    обновлены ссылками на созданные документы; Changelog — запись (2);
  - создан fr-032-graph-read-validate.md (R2: edges_list/edge_get +
    graph_validate, коды E-*/W-* как стабильный контракт, чистая функция
    validate.rs, секция «Наглядная проверка (5 минут)»);
  - создан fr-033-graph-apply-batch.md (R3: graph_apply — транзакция
    «всё или ничего», один undo-шаг, ref-резолв, лимиты ≤256 операций,
    секция «Наглядная проверка (5 минут)»);
  - разметка: CR-013 — таблица R1–R5 (R2 → FR-032, R3 → FR-033, R4 → после
    гейта S4, R5 → A4, CP-позиции) + Changelog; FR-029 — связки с FR-032/033,
    «Наглядная проверка (5 минут, A1/CP1)», Changelog; FR-016 — CP5;
    FR-017 — CP6 + разграничение v1/S2; index-cr-fr.md — строки FR-032/033,
    примечание о нумерации (следующий — FR-034).
- **Коммит:** см. git log — docs(cr,plans): критический путь + FR-032/FR-033.

---

# Worklog — журнал работ агентов по репозиторию CanvasDesk

Журнал дополняется сверху вниз (новые записи — выше). Ссылка на этот файл — в
подвале `docs/change-requests/index-cr-fr.md` (там, где «отчёты в worklog.md репо»).
Формат записи: дата, задача, что сделано, артефакты/коммиты.

---

## 2026-09-18 — Пересмотр продуктового роадмапа (по ADR-0008 + math-computing-stack.md)

- **Задача (владелец):** полностью пересмотреть роадмап разработки на основании
  `docs/architecture/math-computing-stack.md` и
  `docs/adr/adr-0008-math-computing-stack.md`; анализ документации + продуктовое
  интервью (8 вопросов) + оформление решений в репо.
- **Решения владельца (интервью):** канал проверки — догфудинг + живые демо;
  «вау» — полная петля «агент собирает → человек крутит»; эталоны — все №1–№5
  (ответ «D» — трактовка «все варианты выше», №1 и №5 первыми); дедлайна
  B2B/реестра нет; ADR-0008 — принять; инфра-хвосты и WASM — бэклог; гейт —
  ≥ 5 внешних пользователей сами построили модель и вернулись второй раз.
- **Сделано:**
  - создан `docs/plans/product-roadmap.md` — волны 0/A/B/V/S, гейты, привязка
    к M0–M6 архдока и R1–R5 CR-013, нумерация FR (FR-032 = R2, FR-033 = R3),
    бэклог с триггерами, операционализация метрики гейта;
  - ADR-0008 + архдок: статус «предложено» → «принято» (решение владельца
    2026-09-18), в §9 архдока добавлено продуктовое время этапов;
  - README: раздел «Дорожная карта» переписан под волны (старый пункт
    «Композиция моделей» заменён);
  - AGENTS.md: роадмап добавлен в источники истины; SPEC §12: битая ссылка
    на несуществующий `roadmap.md` → `docs/plans/product-roadmap.md`;
  - index-cr-fr.md: статусы FR-018/019/020 синхронизированы с заголовками
    документов («выполнено (v1)»; индекс отставал от документов);
    примечание о нумерации — FR-032/FR-033 закреплены, следующий — FR-034;
  - CR-013: запись в Changelog о привязке R1–R5 к волнам;
    docs/adr/README.md: строка ADR-0008 → «принято»;
  - создан настоящий worklog.md (до этого файл не существовал, хотя индекс
    CR/FR ссылался на него).
- **Коммит:** см. `git log` — docs(adr,plans): продуктовый роадмап + ADR-0008 принято.
- **Открытые пункты (следующие шаги):** волна 0 (cargo-deny в CI + deny.toml,
  реестр зависимостей, фичи `stats`/`parallel`, триаж FR-023/FR-024/CR-005);
  затем FR-029 (критический путь волны A).

---

## 2026-09-16 — Аудит реализации всех CR/FR (main `984ca6b`)

- Аудит провёл агент; отчёты — в файлах Changelog соответствующих CR/FR
  документов `docs/change-requests/` (обновления статусов «выполнено» = код в
  main + автотесты зелёные). Ручная приёмка по `docs/ACCEPTANCE.md` §13–14 —
  отдельный процесс владельца.

## 2026-09-18 — Интеграция: merge feature/fr-033-graph-apply → main (все CP слиты)

- **Операция:** слияние CP3 (FR-033 graph_apply) в main поверх интегрированных
  CP0 (лицензионная гигиена), CP2 (FR-032) и CP1 волны A (FR-029); ветки
  fr-027 и fr-032 были уже полностью слиты ранее.
- **Разрешение конфликтов (10 файлов, ~37 гунков):** во всех зонах FR-029
  (model.rs, templates.rs, flow.rs, edgegeom.rs) приоритет полной реализации
  CP1 из main — минимальный контур ветки CP3 уступил по её же оговорке;
  тесты main.rs сохранены ОБЕ стороны (CP1 Instagram MVP + CP3 graph_apply,
  коллизий имён нет); canvas-mcp — счётчик инструментов 22 → 26, ассерты
  FR-032 и FR-033 объединены; манифест cdn — схема CP1 (плоская), выход
  `origin_rps`, дубль ключа `outputs` от автослияния устранён; дубли полей
  в литералах (templates.rs, main.rs, json_canvas_io.rs) вычищены; вызов
  `inbound_slots_with_lines` приведён к 4-арговой сигнатуре CP1.
- **Адаптация CP3:** oracle-тест graph_apply переведён на выход `origin_rps`
  (контракт манифеста CP1) — значения оракула не изменились (20.8333/41.6667
  rps, ±1 %); SPEC §13 и счётчики (26) актуализированы, ACCEPTANCE §22,
  index-cr-fr: fr-033 «реализовано (v1)».
- **Гейты:** cargo fmt — ок; clippy --workspace --all-targets -D warnings —
  ок; cargo test --workspace — 1007 passed, 0 failed.

## 2026-09-18 — Стабилизация CI: таймаут search-тестов 2с → 10с (флейк windows-latest)

- **Диагноз:** merge-коммит f305972 (CP3) уронил CI на windows-latest — все 9
  search-тестов canvas-shell упали по таймауту «событие поиска не пришло:
  Timeout» (search.rs:412, RECV=2с), при зелёных macOS/Linux и зелёных
  watcher-тестах того же бинарника (порог первого события — 5с). Тестовый
  бинарник canvas-shell между зелёным 9fc11f3 и красным f305972 идентичен
  (CP1/CP3 не трогали canvas-shell и Cargo-манифесты) — регрессии нет,
  чистый флейк: медленный раннер отдаёт событие позже 2с. CI на ветку CP3
  не гонялся (мерж пушем в main, без PR) — потому и всплыл только на main.
- **Фикс:** RECV в тестах search.rs 2с → 10с — выше порога watcher (5с) с
  запасом на Windows-раннеры (параллельные тесты, сканирование свежих
  cache.db). Таймаут теста — защита от зависания, не гейт производительности.
- **Гейты:** fmt — ок; clippy --workspace --all-targets -D warnings — ок;
  cargo test --workspace — 1007 passed / 0 failed (порог merge-коммита).
- **Открытые пункты:** флейк-политика «сначала зелёный main»: слияния CP
  в main делать через PR (ci.yml гоняет гейты на pull_request) либо
  локально прогонять полный гейт перед пушем в main.

## 2026-09-18 — Стабилизация CI (продолжение): watcher_debounce_burst + FIRST 5с→10с

- **Диагноз №2:** после фикса search-таймаутов (b6b7718) Windows стал зелёным,
  но ubuntu уронил `watcher_debounce_burst` («шквал должен схлопнуться в ≤2
  батча, пришло 3»). Жёсткое допущение неверно: trailing-дебаунсер ОБЯЗАН
  разорвать шквал на ≥2 батча, если сами 10 записей растянулись больше окна
  DEBOUNCE=300 мс (медленный IO раннера под параллельными тестами) — это
  корректное поведение, не баг. Локально 3 прогона подряд — 129/0.
- **Фиксы (watcher.rs):** 1) лимит батчей производен от длительности шквала —
  `burst_elapsed/DEBOUNCE + 2` (на быстрой машине = исходные 2; не-дебаунсинг
  ловится: 10 отдельных батчей требуют ≥8×DEBOUNCE на записи 10 байтовых
  строк); 2) FIRST 5с → 10с — выровнен с RECV search-тестов.
- **Гейты:** fmt — ок; clippy --workspace --all-targets -D warnings — ок;
  cargo test --workspace — 1007 passed / 0 failed; canvas-shell ×3 — стабильно
  129/0.

## 2026-09-18 — FR-034: перепроектирование MCP-транспорта (ADR-0009) — hermes agent подключается

- **Приказ владельца:** «перепроектировать MCP — hermes agent не может
  нормально подключиться». Уточнено: hermes — на той же машине; транспорт
  нужен спек-совместимый; формат — ADR + реализация.
- **Диагноз** (чтение кода + прогон эмуляции клиентской сессии против
  canvasdesk-mcp, скрипт mcp_client_probe.sh): 5 дефектов моста —
  1) даунгрейд версии: SUPPORTED_PROTOCOLS без 2025-06-18 → клиенту с
  актуальной версией отвечали 2024-11-05, строгие SDK рвут соединение;
  2) batch-массивы (JSON-RPC, spec 2025-03-26) → -32600 «ожидался объект»;
  3) двойная упаковка tools/call: прод-приложение отвечает JSON-RPC-конвертом
  (on_mcp_wake → build_result), мост клал конверт ЦЕЛИКОМ в content[0].text
  (юнит-тест этого не ловил — FakeTransport возвращал чистое значение);
  4) initialize без pipe → JSON-RPC ошибка -32002 и exit(2) — клиент видит
  краш сервера, сессии нет; 5) транспорт фиксировался на старте —
  приложение, поднявшееся позже, не подхватывалось. Дополнительно:
  resources/list, prompts/list, resources/templates/list, logging/setLevel,
  notifications/cancelled → -32601 (хосты зондируют безотносительно
  capabilities).
- **ADR-0009** (docs/adr/adr-0009-mcp-transport-compatibility.md, статус
  «принято»): выбран вариант C — точечное перепроектирование конвертного
  слоя canvas-mcp; отклонены A (сетевой транспорт Streamable HTTP — hermes
  локальный, SPEC §7.6 «только локально»; вернуться отдельным ADR при
  удалённых агентах, с Bearer-токеном) и B (переход на официальный MCP SDK —
  несоразмерно). FR-034 (docs/change-requests/fr-034-mcp-transport-
  compatibility.md) — реализация решения.
- **Реализация (crates/canvas-mcp/src/lib.rs, main.rs):**
  SUPPORTED_PROTOCOLS = [2025-06-18, 2025-03-26, 2024-11-05] (эхо клиентской);
  handle_input — batch-разбор (поэлементно, пустой массив → -32600,
  все-уведомления → тишина); unwrap_app_payload — разворот конверта
  приложения: result → чистый JSON в content[0].text + structuredContent
  (объект), error → isError, isError-результат приложения — насквозь;
  initialize успешен ВСЕГДА (HandleOutcome::Exit удалён — процесс живёт до
  закрытия stdio); refresh_transport — reconnect перед каждым пакетом
  (WaitNamedPipeW 500 мс, без автоспавна); толерантные заглушки read-only
  методов. build_call_result сменил сигнатуру (&str → &Value).
  Автоспавн (FR-008), pipe-сервер canvas-shell и mcp_dispatch (26
  инструментов) — без изменений.
- **Тесты canvas-mcp:** новые batch_requests, call_result_unwrapping,
  offline_handshake_and_calls (вместо pipe_unavailable_scenarios),
  read_only_stubs_and_cancelled; handshake_and_call_with_connected_pipe
  переведён на прод-конверт (envelope в FakeTransport) + structuredContent;
  initialize_protocol_negotiation — эхо 2025-06-18. Итог: 17 тестов крейта.
- **Верификация реальной сессией (probe):** initialize без приложения →
  success + эхо 2025-06-18, exit 0 (было: -32002 + exit 2); batch
  [initialize, ping] → 2 ответа; tools/call offline → isError «не запущен».
- **Доки:** SPEC.md §13 «Транспорт stdio (FR-034, ADR-0009)»; BYOK.md §3 —
  гарантии транспорта, Hermes Agent в списке клиентов; ACCEPTANCE.md §23 —
  8 пунктов чек-листа; index-cr-fr.md — строка FR-034, следующий номер
  FR-035.
- **Гейты:** fmt — ок; clippy --workspace --all-targets -D warnings — ок;
  cargo test --workspace — 1010 passed / 0 failed.

## 2026-09-18 — FR-035: чистота stdout MCP-потока (ADR-0010) — «Invalid JSON \x1b[2m…» у hermes устранён

- **Триггер:** владелец прислал лог hermes после FR-034: сервер регистрируется
  (parked → connected, инструменты видны), но вызовы падают —
  `Invalid JSON: expected value at line 1 column 1,
  input_value='\x1b[2m2026-09-18T11:16:…canvasdesk\\widgets"'`.
- **Диагноз (улика разобрана, цепочка подтверждена кодом):** `\x1b[2m` — ANSI
  escape формата tracing_subscriber; в stdout моста попадали логи
  автоспавненного GUI. 1) `connect_app` спавнил `Command::new(exe).spawn()`
  без Stdio — в Rust это НАСЛЕДОВАНИЕ stdin/stdout/stderr родителя;
  2) GUI инициализировал `tracing_subscriber::fmt()` — writer по умолчанию
  stdout, ANSI включён; 3) старт GUI (версия, реестр виджетов
  `…\canvasdesk\widgets`, wgpu) заливал цветной текст прямо в JSON-RPC-канал;
  4) hermes парсит stdout как newline-delimited JSON → каждая лог-строка =
  Invalid JSON → вызов падает → сессия парковится/возрождается (в логе два
  «revived» за 12 с), цикл. Доп. дефект: автономный `canvasdesk-mcp.exe`
  спавнил current_exe = САМ СЕБЯ (двойник крадёт stdin, рекурсивный спавн).
- **ADR-0010** (docs/adr/adr-0010-mcp-stdio-purity.md, «принято»): вариант C —
  изоляция stdio автоспавна + ориентация спавна + stderr-логи GUI
  (defense-in-depth); отклонены: фильтрация мусора в мосту (лечение симптома),
  отказ от автоспавна (регрессия FR-008), только stderr без изоляции
  (GUI может писать в stdout не через tracing).
- **Реализация canvas-mcp:** `spawn_service_command` — Stdio::null() на все
  три хэндла; `autosprawn_target` — единый `canvasdesk` спавнит сам себя
  (GUI-режим), автономный `canvasdesk-mcp` ищет соседа `canvasdesk.exe` в
  своём каталоге, нет соседа → offline (ADR-0009); `connect_app` переведён на
  хелперы. `#[cfg(any(windows, test))]` — без dead_code на Linux.
- **Реализация canvas-app:** tracing_subscriber → `.with_writer(io::stderr)`
  + `.with_ansi(io::IsTerminal::is_terminal(&stderr()))`; актуализирован
  комментарий перехвата подкоманды `mcp`.
- **Тесты (canvas-mcp 15):** `spawn_service_command_isolates_stdio` —
  поведенческий (unix): ребёнок репортит `[ -c /dev/fd/N ]` по трём fd в файл
  до любого редиректа → «ccc» (/dev/null); `autosprawn_target_…` — мост без
  соседа → None, с соседом → Some(gui), единый бинарь → сам себя, CAPS-стем.
  Нюанс: `Command::get_stdin/get_stdout/get_stderr` НЕ стабилизированы —
  первый вариант теста заменён поведенческим.
- **Probe реальной сессии** (/home/z/my-project/scripts/
  mcp_stdio_purity_probe.sh): initialize (эхо 2025-06-18) + batch + tools/call
  offline + cancelled + мусор; каждая строка stdout — валидный JSON
  (bad=0), контракт FR-034 сохранён. PROBE PASS.
- **Инцидент гейтов:** первый полный `cargo test --workspace` упал
  `ld: signal 7 (Bus error)` на линковке thumbs_smoke — диск 100%
  (target/ = 8.4G). cargo clean (−9 ГБ) → прогон заново.
- **Доки:** SPEC §13 «Чистота stdout (FR-035, ADR-0010)»; BYOK §3 — гарантия
  чистоты канала; ACCEPTANCE §24 (7 пунктов); index-cr-fr — строка FR-035,
  следующий номер FR-036; adr/README.md — добавлены ADR-0009 (пропуск
  прошлой итерации) и ADR-0010.
- **Гейты:** fmt — ок; clippy --workspace --all-targets -D warnings — ок;
  cargo test --workspace — 1012 passed / 0 failed (+2 к FR-034).

## 2026-09-18 — FR-036: wasm-сборка ядра — гейт wasm32-unknown-unknown + исполнение тестов в wasmtime (ADR-0011)

- **Триггер:** приказ владельца «распланировать сборку wasm и реализовать
  сборку для повышения твоей автономности в тестировании». Уточнение скоупа
  (AskUserQuestion): гейт + тесты в wasm-рантайме; таргеты unknown-unknown
  (продукт) + wasip1 (тесты); CI-джоба в ci.yml; коммит сразу в main.
- **Инцидент среды:** контейнер откатился к старому снапшоту (HEAD = 28b4dc8,
  состояние после ADR-0008; ~250 файлов «modified» = смена прав 100644→100755;
  Rust-тулчейн пропал). Восстановлено из origin/main: `git fetch` +
  `git reset --hard origin/main` (= 3a13c33, FR-035) + переустановка rustup
  stable 1.98.1 (rustfmt, clippy). Урок: работы сессий живут только в пуше.
- **Исследование:** план M8 (`docs/plans/wasm-port.md`, §2) уже фиксировал
  wasm-совместимость ядра экспериментом 2026-09-16; W0 (CI-гейт) — открыт.
  Эмпирика на 3a13c33: check ядра под wasm32-unknown-unknown зелёный (55.8 с).
- **Компиляция ≠ исполнение:** wasmtime 48.0.2 (преduccт-бинарь в
  ~/.local/bin) + wasm32-wasip1 → тесты canvas-core падали: паника
  `std::env::temp_dir()` (std на wasm её не реализует) и — после
  первого фикса — паника `std::process::id()` в scene_ops (frame 11
  бэктрейса wasmtime). Флэйк первого прогона маскировал счётчиком «283
  passed» — реальный полный набор 301 (spatial 6 + templates_schema 8 не
  влезли в обрезанный вывод).
- **ADR-0011** (docs/adr/adr-0011-wasm-build-gate.md, «принято»): вариант B —
  гейт (check + rlib) + исполнение тестов ядра в wasmtime; отклонены:
  только W0-check (исполнение не доказано), wasm-bindgen-фасад (W4/W12),
  вынос mcp_dispatch в lib (W2, отдельная задача).
- **Реализация:** `scripts/wasm_gate.sh` (ступени: check ядра → build rlib →
  `RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-core`;
  режим `--check` без wasmtime); `.cargo/config.toml` — runner
  `wasmtime run -S inherit-env --dir .::/ --dir /tmp::/tmp` (только при
  явном `--target wasm32-wasip1`); CI-джоба `wasm-check` (ubuntu, ступень 1 —
  приёмка W0); тестовая песочница `test_scratch_root()` в `#[cfg(test)]`
  lib.rs (натив — temp_dir, wasm — `.wasi-scratch` в CWD) + локальная копия
  в scene_ops.rs (интеграционные тесты не видят cfg(test)-хелперы) + замена
  process::id → SystemTime; `.gitignore` += `.wasi-scratch/`.
- **Результат:** 301/301 тестов canvas-core под wasip1 (wasmtime 48) и
  301/301 нативно (нулевой регресс); rlib ядра 21M; полный
  `scripts/wasm_gate.sh` — OK.
- **Доки:** ADR-0011; FR-036 (docs/change-requests/fr-036-wasm-build-gate.md);
  ACCEPTANCE §25 (7 пунктов); index-cr-fr — строка FR-036, следующий номер
  FR-037; adr/README.md; wasm-port.md — W0 «выполнено» + примечание о
  тест-раннере; AGENTS «Сборка и тесты» — wasm-гейт; SPEC §3 — строка
  wasm-таргетов.
- **Гейты:** wasm-gate — OK (3 ступени + `--check`); fmt — ок; clippy
  --workspace --all-targets -D warnings — ок; cargo test --workspace —
  1012 passed / 0 failed (без изменений к FR-035 — нулевой регресс).

---

## 2026-09-18 — FR-037/ADR-0012: план верификации MCP-реализации в WASM (без реализации)

- **Задача (владелец):** «теперь спланировать реализацию MCP для WASM,
  чтобы можно было проверять не только UI, но и реализацию MCP».
- **Формат:** только планирование — документы и индексы; код не меняется
  (Task ID сессии агента: fr-037-plan).

**Исследование (факты кода, main `b9e011c`):**
- `mcp_dispatch` (main.rs:6074, ~1550 строк, 27 инструментов) — уже чистая
  функция: зависимости только SceneState (mark_dirty/push_undo/
  recompute_flow/move_node/ensure_reserve_at), Camera (4 метода:
  position/zoom/set_center/set_zoom, main.rs:6855–6876),
  TemplateRegistry::builtin() (embedded). `node_create_file` диск не трогает.
- Мост canvas-mcp — чистый протокол за трейтом AppTransport (lib.rs:60) +
  run_stdio (lib.rs:594); платформенное только под cfg(windows). Под
  wasm-таргеты мост НЕ проверялся (гейт FR-036 = core/render/widgets).
- ~40 MCP-тестов в main.rs:12060+ (~2970 строк), включая эталонные гейты
  CP1 (mcp_fr029_instagram_mvp_reference:14183), CP3, CP5 — уже исполняются
  нативно на Linux, но не в wasm-рантайме; end-to-end сессия требует
  Windows pipe + GUI.
- SceneState смешивает модель и ввод: UI-поля selected/selected_nodes/
  dragging = 99 мест доступа; модельные поля — путь доступа self.scene.*
  не меняется при переносе типа.

**Документы (этот коммит):**
- **ADR-0012** (docs/adr/adr-0012-mcp-wasm-verification.md, «предложено»):
  вариант D — крейт canvas-scene (SceneState-модель + модуль mcp; ноль
  новых внешних зависимостей; viewport-зеркало вместо зависимости от
  canvas-render) + canvas-mcp: run_stdio_with_transport<T> + лист-крейт
  canvas-mcp-headless (HeadlessSession за AppTransport, bin для
  wasmtime/wasip1) + драйвер mcp_wasm_e2e.py и гейт mcp_wasm_gate.sh.
  Отклонены: «ничего не выносить» (нет сессии), полный W2 (объём),
  wasm-bindgen-фасад (волна 2, прецедент ADR-0011-C).
- **FR-037** (docs/change-requests/fr-037-mcp-wasm-verification.md,
  «выявлено (план)»): задачи MW1 (canvas-scene, M) → MW2 (мост под wasm,
  S) → MW3 (headless, M) → MW4 (гейт/CI/доки, M) + опции MW5 (инспектор)
  / MW6 (файловый режим); критерии приёмки (≥356 wasm-тестов, oracle ±1%
  эталона №1, нативный регресс 0); 5 открытых вопросов (Q1 имена, Q2
  viewport, Q3 CI-wasmtime, Q4 инспектор, Q5 файлы) с рекомендациями.
- Индексы: adr/README.md += ADR-0012; index-cr-fr.md += FR-037, следующий
  номер FR-038; wasm-port.md — примечание FR-037 (W2 сужается; волна 2
  MCP-мост = HeadlessSession) + §9-строка моста дополнена ссылкой.

**Границы:** продуктовое поведение не меняется; реализация MW1–MW4 не
начата — по плану, каждая задача = сессия = коммит.
