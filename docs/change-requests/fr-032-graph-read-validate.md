# FR-032: Чтение графа и валидация модели — edges_list/edge_get + graph_validate

- **Статус:** в работе (v1 — без кодов контракта портов FR-029; см. Changelog)
- **Тип:** FR
- **Приоритет:** критично (шаг R2 волны A продуктового роадмапа — `docs/plans/product-roadmap.md` §4.2; агент не может проверить сборку, не видя рёбер)
- **Владелец:** агент (оформление по решению владельца — продуктовое интервью 2026-09-18)
- **Источник:** CR-013 (G4 — агент не видит существующие связи; G7 — нет валидации совместимости при связывании); продуктовый роадмап (волна A, этап A2)
- **Связанные задачи:** CR-013; FR-029 (порты значений — R1; `toParam`/`fromOutput`/`outputs` валидируются этим FR); FR-014 (поток значений — `topo_sort`/`CycleError` переиспользуются); FR-015 (перегрузки `Overload`); `docs/plans/product-roadmap.md` §9 (CP2)
- **Создан:** 2026-09-18
- **Обновлён:** 2026-09-18

## Описание

Агент, собирающий архитектуру через MCP, не может прочитать существующие связи
графа (`edges_list` отсутствует — CR-013 G4) и не получает диагностики
несовместимости при связывании: ошибочная единица, два ребра в один параметр,
адресация несуществующего порта обнаруживаются молчаливым нулём или `Overload`
в чужом месте (CR-013 G7, «Нарушение контракта» в §Проверка). Требуется:

1. **Чтение** — MCP-инструменты `edges_list` (все рёбра с полной схемой,
   включая адресацию портов FR-029) и `edge_get {id}` (одно ребро).
2. **Валидация** — MCP-инструмент `graph_validate`: чистая функция ядра
   возвращает структурированный отчёт проблем с кодами, нодой/ребром и
   человекочитаемым сообщением; severity `error|warning`.

## Влияние

| Объект | Что меняется | Где в документации |
|---|---|---|
| `mcp` | +3 инструмента (`edges_list`, `edge_get`, `graph_validate`); TOOLS 22 → 25 | `docs/SPEC.md` §MCP |
| `canvas-core` | новый модуль `validate.rs` (чистая функция, без I/O — в духе `flow.rs`) | SPEC §3, FR-014 |
| `edge` | читаемая схема ребра — каноническая форма для агента | `docs/interface-objects/edge.md`, FR-029 |
| агентный рецепт | `graph_validate` — финальный шаг рецепта (R5, A4) | `docs/plans/product-roadmap.md` §4.2 |

## Анализ (что отсутствует)

- **MCP TOOLS** (`crates/canvas-mcp/src/lib.rs:175-332`, 22 инструмента): `nodes_list`/`nodes_search`/`node_get` читают ноды; инструмента чтения рёбер нет вообще — агент восстанавливает топологию вслепую (G4).
- **Dispatch** (`crates/canvas-app/src/main.rs:6282-6384`): ветки `edge_create`/`edge_delete`/`edge_ports` работают с рёбрами, но чтения нет; `flow_cycle_check` — единственная «валидация» (только циклы).
- **Циклы** (`crates/canvas-core/src/flow.rs:76,130`): `topo_sort`/`CycleError`/`cycle_participants` — переиспользуются как категория E-CYCLE.
- **Единицы** (`crates/canvas-core/src/expr.rs`, размерная система `Dimension`/`Unit`): конверсия есть внутри eval, но сравнение ожидаемой единицы параметра приёмника с единицей выхода истока (после FR-029 `OutputSpec.unit`) нигде не выполняется.
- **Перегрузки** (`crates/canvas-core/src/expr/queueing.rs`, `EvalError::Overload`): попадают в `FlowOutputs` (`flow.rs:70`), но не агрегируются в отчёт о состоянии модели.
- **Дубль-входы**: FR-029 п.3 фиксирует для нескольких рёбер в один `to_param` политику «последний побеждает + warning» — строгая ошибка (детерминированная диагностика) здесь.

## Требуемые изменения

| # | Что → Где → Как |
|---|---|
| 1 | `crates/canvas-core/src/validate.rs` (новый) → `pub fn validate(canvas: &Canvas) -> Vec<ValidationIssue>`; чистая, без I/O и мутаций; внутри исполняет `propagate_with_lines` (перегрузки) и `topo_sort` (циклы). `ValidationIssue { severity, code, node_id: Option<String>, edge_id: Option<String>, message }`; сериализация в snake_case для MCP. |
| 2 | Коды (v1, стабильный контракт для рецепта агента): **E-CYCLE** — цикл в value-подграфе (участники из `cycle_participants`); **E-UNIT** — несовместимая размерность единицы выхода истока и параметра приёмника (`OutputSpec.unit` FR-029 против единицы параметра из текста ноды, `templates.rs:752` `params_from_text`); **E-PORT-UNKNOWN** — ребро адресует имя выхода/параметра, отсутствующее в снапшоте шаблона; **E-DOUBLE-INPUT** — два value-ребра в один `to_param` (указываются оба `edge_id`, детерминизм по порядку `canvas.edges`); **E-OVERLOAD** — нода в `EvalError::Overload` (параметр/формула в message); **W-AMBIGUOUS-SRC** — value-ребро без `from_output`/`from_line`, когда у истока > 1 формульной строки; **W-UNUSED-SLOT** — позиционный вход `$N` доставлен, но формула приёмника его не читает. |
| 3 | `crates/canvas-mcp/src/lib.rs` → схемы: `edges_list {}` → `[{id, from, to, kind, fromLine?, fromOutput?, toParam?, fromSide, toSide}]`; `edge_get {id}`; `graph_validate {}` → `{valid: bool, issues: [ValidationIssue]}`. TOOLS-тест (`lib.rs:854-925` паттерн) на все три схемы. |
| 4 | `crates/canvas-app/src/main.rs` → `mcp_dispatch`: три ветки; `graph_validate` прогоняет `validate::validate(&canvas)` и возвращает отчёт; `edges_list`/`edge_get` читают `canvas.edges` (поля FR-029 включаются автоматически — сериализация уже есть). |
| 5 | Юнит-тесты `validate.rs`: чистый эталон №1 → `issues == []`; по фикстуре на каждый код (подсаженная ошибка → точный код + node_id/edge_id); `E-DOUBLE-INPUT` на двух рёбрах в один `to_param` в разном порядке — стабильный отчёт. |
| 6 | MCP e2e (паттерн тестов `mcp_dispatch`): `graph_validate` на чистом графе → `valid: true`; сборка с `edge_create(toParam: "rps")` от выхода в `ms` → `E-UNIT` с `edge_id`. |
| 7 | Документация: `docs/SPEC.md` §MCP (+3 инструмента, контракт кодов); `docs/ACCEPTANCE.md` — раздел ручной проверки; `user-docs/agent-recipe.md` (R5) — финальный шаг «validate → отчёт пуст». |

## Точки входа

- `docs/SPEC.md` §MCP — схемы инструментов; §3 — модуль `validate.rs` в составе `canvas-core`.
- `docs/plans/product-roadmap.md` §4.2 (A2) — гейт: «валидатор ловит подсаженные ошибки на эталоне №1».
- `docs/ACCEPTANCE.md` — сценарий ручной проверки `graph_validate`.
- `docs/interface-objects/edge.md` — каноническая читаемая схема ребра.

## Проверка

- **Наглядная проверка (5 минут, демо-критерий этапа A2):** на эталоне №1 (ADR-0006) подсадить три ошибки — ребро с несовместимой единицей (`ms` → `rps`), второе ребро в занятый `toParam`, ребро на несуществующее имя выхода — один вызов `graph_validate` возвращает ровно три issue с точными `code`/`node_id`/`edge_id`; после исправления повторный вызов — `valid: true, issues: []`. Результат наблюдаем в ответе MCP-инструмента без чтения кода.
- Юнит: перечислены в п.5 (каждый код — отдельная фикстура).
- MCP e2e: п.6.
- Гейты: `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`.

## История изменений

- `2026-09-18` (3) — агент (интеграция CP1, после влития FR-029): чек-лист
  выполнен полностью: 1) `port_contract_issues` реализована — E-PORT-
  UNKNOWN (fromOutput без имени у истока / toParam без параметра шаблона /
  текстовый приёмник), E-DOUBLE-INPUT (второе ребро в тот же toParam),
  E-UNIT (несовместимость размерностей проливаемого значения и параметра
  — по мультимножеству размерностей, масштаб ms/s не важен); 2)
  W-AMBIGUOUS-SRC учитывает `from_output`; 3) W-UNUSED-SLOT исключает
  рёбра с `toParam` из позиционных слотов; 4) `mcp_edge_json` несёт
  `fromOutput`/`toParam` (edges_list/edge_get); 5) гейт A2 закрыт e2e
  `mcp_fr029_instagram_mvp_reference` (эталон ADR-0005: чистый — valid,
  3 подсаженные ошибки → ровно 3 issue с кодом/нодой/ребром, исправление
  → чисто). Коды портов в графе-описании модуля — «реализованы».

- `2026-09-18` (2) — агент (CP2, параллельно с CP0/CP1): реализована v1 —
  всё, что не зависит от полей FR-029. Ядро `canvas-core/src/validate.rs`:
  `ValidationIssue {severity, code, node_id, edge_id, message}` (сериализация
  snake_case) + `validate(&Canvas)`; реализованы `E-CYCLE` (при цикле отчёт
  ограничивается топологией — «почини цикл, потом остальное»), `E-OVERLOAD`,
  `W-AMBIGUOUS-SRC` (v1: без `fromOutput` — поля ещё нет), `W-UNUSED-SLOT`
  (в т.ч. диагностика G5: позиционное ребро в шаблонную ноду теряется).
  `E-UNIT`/`E-PORT-UNKNOWN`/`E-DOUBLE-INPUT` определены на `toParam`/
  `fromOutput`/`outputs` из FR-029 — точка добавления `port_contract_issues`
  (заглушка с контрактом в док-комментарии). MCP: TOOLS 22 → 25
  (`edges_list`, `edge_get`, `graph_validate`); dispatch: три ветки чтения
  (валидация не мутирует канвас — без undo/автосейва); каноническая схема
  ребра — `mcp_edge_json` (FR-029 добавит туда `toParam`/`fromOutput`).
  Тесты: 10 юнит (фикстура на каждый реализованный код, детерминизм,
  контракт сериализации) + 3 e2e `mcp_dispatch`. Гейты fmt/clippy/test
  зелёные. Дока: SPEC §4/§5.1, ACCEPTANCE §21, edge.md. **Интеграция CP1**
  (после влития FR-029): 1) заполнить `port_contract_issues`; 2) в
  W-AMBIGUOUS-SRC добавить `|| edge.from_output.is_some()`; 3) в
  W-UNUSED-SLOT исключить рёбра с `toParam` из позиционных слотов; 4)
  `mcp_edge_json`: поля `toParam`/`fromOutput`; 5) сценарии FR-032.10–12
  (ACCEPTANCE §21) и гейт A2 «3 подсаженные ошибки → ровно 3 issue»
  (до CP1 три из них определить не на чем — контракт портов ещё не существует).
- `2026-09-18` — агент: создан по R2 CR-013 (волна A продуктового роадмапа, этап A2); статус «выявлено»; контракт кодов E-*/W-* зафиксирован как стабильный API для рецепта агента (R5).

## Источники истины

- `docs/change-requests/cr-013-agent-architecture-composition.md` — G4/G7, дорожная карта R1–R5.
- `docs/plans/product-roadmap.md` — волна A (A2), §9 критический путь CP2.
- `crates/canvas-mcp/src/lib.rs:175-332,854-925` — TOOLS (22) и тест схем.
- `crates/canvas-app/src/main.rs:6282-6384` — `mcp_dispatch` (ветки рёбер/потока).
- `crates/canvas-core/src/flow.rs:70,76,130` — `FlowOutputs`, `topo_sort`, `cycle_participants`.
- `crates/canvas-core/src/templates.rs:752` — `params_from_text` (единицы параметров для E-UNIT).
- `docs/adr/adr-0006-reference-scenario-catalog.md` — эталон №1 (материал для наглядной проверки).
