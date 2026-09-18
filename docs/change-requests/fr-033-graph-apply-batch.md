# FR-033: graph_apply — атомарная батч-композиция графа одним вызовом

- **Статус:** реализовано (v1, 2026-09-18 — ветка `feature/fr-033-graph-apply`, CP3 критического пути; минимальный контур FR-029 в core включён для гейта — см. Changelog)
- **Тип:** FR
- **Приоритет:** критично (шаг R3 волны A продуктового роадмапа — `docs/plans/product-roadmap.md` §4.2; без батча сборка эталона = 50+ round-trips без атомарности)
- **Владелец:** агент (оформление по решению владельца — продуктовое интервью 2026-09-18)
- **Источник:** CR-013 (G6 — нет батч-операций; «дорожная карта» R3); продуктовый роадмап (волна A, этап A3)
- **Связанные задачи:** CR-013; FR-029 (порты значений — адресация `fromOutput`/`toParam` в операциях `edge_create`); FR-032 (`graph_validate` — вызывается агентом после сборки); FR-006 (undo — один шаг на батч); FR-018/FR-019 (`instantiate` с overrides, `templates.rs:797`); `docs/plans/product-roadmap.md` §9 (CP3)
- **Создан:** 2026-09-18
- **Обновлён:** 2026-09-18 (реализация v1)

## Описание

Сборка эталонной архитектуры (Instagram MVP — ~12 нод и ~15 рёбер) сегодня
требует от агента 25–50 отдельных MCP-вызовов (`node_create_*`/
`template_instantiate`/`edge_create`/`flow_set_kind`): между вызовами состояние
графа меняется, сбой в середине оставляет полусобранную модель, undo размазан
на десятки шагов, а счётчик round-trips делает «агент собирает за минуты»
медленным и хрупким (CR-013 G6). Требуется инструмент **`graph_apply`**:
список операций, применяемый к канвасу **атомарно** — либо весь граф собран и
пересчитан, либо канвас байт-в-байт прежний; успешный батч = **один** шаг undo.

## Влияние

| Объект | Что меняется | Где в документации |
|---|---|---|
| `mcp` | +1 инструмент `graph_apply` (TOOLS 25 → 26 после FR-032); операция `edge_create` внутри батча — полная схема FR-029 | `docs/SPEC.md` §MCP |
| `canvas-app` | транзакционное применение батча: клон `Canvas` → apply → push undo → пересчёт | SPEC §MCP, FR-006 |
| undo | один `undo_snapshot` на батч (`main.rs:880`), отмена всей сборки одним Ctrl+Z | FR-006, `user-docs/hotkeys.md` |
| агентный рецепт | `graph_apply` — основной шаг рецепта (R5, A4): сборка эталона = 1 вызов + `graph_validate` | `docs/plans/product-roadmap.md` §4.2 |

## Анализ (что отсутствует)

- **MCP TOOLS** (`crates/canvas-mcp/src/lib.rs:175-332`): все инструменты пооперационные; батча нет — G6.
- **Dispatch** (`crates/canvas-app/src/main.rs:6282-6384`): каждая ветка мутирует `canvas` независимо; общего транзакционного контейнера нет.
- **Undo** (`crates/canvas-app/src/main.rs:567,880,889,898`): `undo_stack: VecDeque<Canvas>`, снапшот на каждое действие — батч без специальной обвязки породил бы N шагов.
- **Переиспользуемое (есть):** `instantiate(manifest, overrides, …)` (`templates.rs:797`) уже принимает переопределения параметров; `params_from_text` (`templates.rs:752`) — образец для `param_set`; `Edge` с адресацией портов — FR-029; `propagate_with_lines` (`flow.rs:250`) — полный пересчёт после батча.

## Требуемые изменения

| # | Что → Где → Как |
|---|---|
| 1 | `crates/canvas-mcp/src/lib.rs` → схема `graph_apply {operations: [Op; 1..=256]}`; `Op` — enum из тегированных вариантов: `node_create_note {ref?, x, y, text, width?}`; `node_create_file {ref?, x, y, path}`; `template_instantiate {ref?, template, params?, x, y}`; `edge_create {fromRef|from, toRef|to, kind, fromLine?, fromOutput?, toParam?, fromSide?, toSide?}`; `param_set {ref|id, param, value, unit?}`; `node_move {ref|id, x, y}`. Лимиты схемы: ≤ 256 операций, ≤ 128 нод на батч (защита от деградации live-бюджета SPEC §6.3). |
| 2 | `crates/canvas-app/src/main.rs` → `mcp_dispatch` ветка `graph_apply`: (а) снимок `undo` НЕ пушится до успеха; (б) операции применяются к клону `Canvas` в памяти: `ref`-резолв через карту `ref → node id`, `fromRef/toRef` для рёбер; (в) любая ошибка операции → ответ `{ok: false, op_index, code, message}` и клон отбрасывается — оригинальный `canvas` байт-в-байт прежний (инвариант атомарности, проверяемый сравнением сериализации в тестах); (г) успех → клон заменяет канвас, **один** `undo_snapshot` (`main.rs:880`), полный `propagate_with_lines`, автосейв. |
| 3 | Ответ: `{ok: true, created: [{op_index, ref?, node_id, edge_id?}], report: [{op_index, op, id}], flow: {…}}` — `flow` в формате `flow_recalc` v2 (FR-029) для всех нод: сборка сразу возвращает числа, агент не делает второй вызов. |
| 4 | `param_set` (семантика): правит ровно одну строку `param = value unit` в тексте ноды (по образцу `params_from_text`, `templates.rs:752`), остальные строки не трогает; параметра нет в тексте — ошибка операции (консервативно: без append). |
| 5 | Юнит-тесты: атомарность (ошибка на k-й операции → сериализация канваса до/после идентична); ref-резолв (ребро на `fromRef` ноды, созданной в этом же батче, позицией раньше); `param_set` меняет одну строку; лимиты (257-я операция → ошибка схемы); undo после батча = 1 шаг (`undo_stack.len()` вырос на 1). |
| 6 | MCP e2e (паттерн тестов `mcp_dispatch`): (а) батч ~20 операций собирает мини-эталон «Трафик → CDN → Gateway» (ADR-0005), `flow` в ответе = oracle ±1 %; (б) батч с подсаженной ошибкой в середине → `{ok: false, op_index}` и канвас не изменился; (в) `undo` после успешного батча возвращает исходное состояние. |
| 7 | Документация: `docs/SPEC.md` §MCP (схема, лимиты, транзакционная семантика); `docs/ACCEPTANCE.md` — сценарий; `user-docs/agent-recipe.md` (R5) — основной шаг рецепта. |

## Точки входа

- `docs/SPEC.md` §MCP — инструмент, лимиты, семантика атомарности.
- `docs/plans/product-roadmap.md` §4.2 (A3) — гейт: «агент собирает эталон №1 одним вызовом; undo откатывает сборку целиком»; §9 CP3.
- `docs/ACCEPTANCE.md` — ручная проверка (демо-сценарий).
- `user-docs/agent-recipe.md` (создаётся в R5/A4) — вызов `graph_apply` как ядро рецепта.

## Проверка

- **Наглядная проверка (5 минут, демо-критерий этапа A3):** в MCP-клиенте один вызов `graph_apply` с ~20 операциями эталона №1 (ADR-0006) → открыть сохранённый `.canvas` в приложении: граф собран, числа нод совпадают с таблицей эталона; **Ctrl+Z — вся сборка исчезает одним шагом**, повторный Ctrl+Y восстанавливает; повтор того же батча с намеренно сломанной операцией в середине → ответ `{ok: false, op_index}` и канвас не изменился (визуально пуст/прежний). Всё наблюдаемо без чтения кода.
- Юнит: п.5. MCP e2e: п.6.
- Гейты: `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`.

## История изменений

- `2026-09-18` (2) — агент: **реализовано (v1)** в ветке `feature/fr-033-graph-apply` (CP3, параллельно с CP0/CP1/CP2). Код: `canvas-mcp` — инструмент `graph_apply` (TOOLS 22 → 23 на ветке); `canvas-app` — `mcp_graph_apply`/`batch_apply_op`/`mcp_flow_v2` (транзакция клоном, ref-карта, `param_set` с синхронизацией снапшота, лимиты 256/128, один undo-шаг, ответ с flow v2); `canvas-core` — **минимальный контур FR-029** (поля `Edge.to_param`/`Edge.from_output`, секция `outputs` в манифесте и снапшоте `TemplateRef`, проливание `to_param` и резолв `from_output` в `flow.rs`, именованные выходы в `FlowSolutions.named`) — включён ПОТОМУ ЧТО гейт CP3 (oracle-числа через проливание) невозможен без R1; при интеграции контур уступает полной реализации FR-029 ветки CP1 (контракт совпадает: имена полей/семантика из FR-029). Манифест `com.canvasdesk.cdn` — выход `origin` (для демо мини-эталона). Тесты: 7 MCP e2e/юнит (`graph_apply_*`) + 7 flow-тестов проливания + round-trip портов; гейты зелёные (fmt/clippy -D warnings/test, 988 тестов). Документация: SPEC §13 (graph_apply), ACCEPTANCE §22, index-cr-fr.
- `2026-09-18` — агент: создан по R3 CR-013 (волна A продуктового роадмапа, этап A3); статус «выявлено»; транзакционная семантика «всё или ничего» + один undo-шаг зафиксированы как инварианты.

## Источники истины

- `docs/change-requests/cr-013-agent-architecture-composition.md` — G6, дорожная карта R3.
- `docs/plans/product-roadmap.md` — волна A (A3), §9 критический путь CP3.
- `crates/canvas-mcp/src/lib.rs:175-332,854-925` — TOOLS и тест схем.
- `crates/canvas-app/src/main.rs:567,880,889,898` — `undo_stack`, снапшот/undo/redo; `6282-6384` — `mcp_dispatch`.
- `crates/canvas-core/src/templates.rs:752,797` — `params_from_text`, `instantiate(manifest, overrides, …)`.
- `crates/canvas-core/src/flow.rs:250` — `propagate_with_lines` (пересчёт после батча).
- `docs/adr/adr-0005-instagram-mvp-reference.md`, `docs/adr/adr-0006-reference-scenario-catalog.md` — oracle-числа для e2e и наглядной проверки.
