# FR-029: Именованные порты значений — named outputs шаблонов и проливание входа в параметр

- **Статус:** выполнено (v1)
- **Тип:** FR
- **Приоритет:** критично (блокер агентной композиции — CR-013, шаг R1)
- **Владелец:** агент (оформление по запросу владельца)
- **Источник:** CR-013 (gap-анализ концепции «агентная сборка архитектур с проливанием значений», сессия 2026-09-17)
- **Связанные задачи:** CR-013; FR-014 (поток значений), FR-018 (манифест шаблона), FR-025 (построчные точки выхода), FR-032 (graph_validate — строгая диагностика дубль-входов и единиц поверх этого FR), FR-033 (graph_apply — батч поверх `edge_create` v2); `docs/adr/adr-0002-value-flow-composition.md`, `docs/adr/adr-0003-named-value-ports.md`
- **Создан:** 2026-09-17
- **Обновлён:** 2026-09-22 (перекрёстная проверка с другими FR — 13 расхождений, решения владельца и v2-контур UI вынесены в FR-050; ранее 2026-09-18 — реализация v1 CP1)

## Описание

Value-связь сегодня уносит значение ноды-истока целиком и кладёт его в позиционный слот `$1..$N` приёмника; шаблонная формула при этом читает только именованные `$param` — привязанное значение игнорируется, и «проливание» из ноды в ноду не работает без ручной правки формулы (CR-013, G1/G2/G3/G5). Требуется контракт **портов значений**:

1. **Named outputs** — манифест шаблона декларирует секцию `outputs`: именованные выходные характеристики (имя, единица, источник — строка Numi-листа или подвыражение), которые другие сервисы могут потреблять.
2. **Адресация рёбра портами** — value-ребро может указывать выход истока по имени (`fromOutput`, либо `fromLine` индексом — как в FR-025) и вход приёмника по имени параметра (`toParam`).
3. **Проливание** — значение ребра с `toParam` подставляется в окружение как `$<имя параметра>` приёмника, перекрывая локальное значение параметра, БЕЗ правки формулы шаблона.
4. **MCP-покрытие** — `edge_create` принимает `fromLine`/`fromOutput`/`toParam`/`kind` одним вызовом; новый `edges_list`; `flow_recalc` возвращает узловое значение + именованные выходы + построчные значения каждой ноды.

Владелец концепции: запрос от 2026-09-17 («собрать примеры архитектур исключительно с "проливанием" значений из нод, без ручного копирования значений»).

## Влияние

| Объект | Что меняется | Где в документации |
|---|---|---|
| `edge` | новые поля `toParam`/`fromOutput` (сериализация `toParam`/`fromOutput`), расширенный hit-путь создания | `docs/interface-objects/edge.md`, FR-014 |
| `template` | секция `outputs` в `template.json` + снапшот в `canvasdesk.template` | FR-018/FR-019, `assets/templates/` |
| `flow` | проливание в параметры, резолв имён выходов | FR-014, `crates/canvas-core/src/flow.rs` |
| `mcp` | `edge_create` v2, `edges_list`, `flow_recalc` v2 | `docs/SPEC.md` §MCP |
| UI | (v2, за флагом `line_ports`) подписи имён у построчных точек выхода | FR-025 |

## Анализ (что отсутствует)

- **Edge** (`crates/canvas-core/src/model.rs:436-447`): есть только `from_line: Option<usize>` — индекс строки истока (FR-025, UI-drag). Нет `from_output` (имя выхода) и нет адресации входа вообще: `to_node` адресует ноду целиком.
- **Env** (`crates/canvas-core/src/expr.rs:410-478`): `params` (шаблонные `$имя`) и `inbound` (`$1..$N`, `$in`) — раздельные карты; ссылки из формулы шаблона на входы не связаны со связями.
- **Flow** (`crates/canvas-core/src/flow.rs:264-350`): `inbound_slots_with_lines` маппит `edge.from_line → LineOutputs`; ребро без `from_line` уносит узловое значение. Механизма подстановки входа в именованный параметр нет.
- **Манифест** (`crates/canvas-core/src/templates.rs:88-228`): `params` + единственный `expr`; секции `outputs` нет; `template_list` (MCP) отдаёт только params/expr — агент не знает именованных выходов.
- **MCP** (`crates/canvas-mcp/src/lib.rs:175-332`): `edge_create` (строки 259-268) — без `fromLine/fromOutput/toParam/kind`; `flow_recalc` (284-289) — только узловые значения; инструмента чтения связей нет.
- **Dispatch** (`crates/canvas-app/src/main.rs:6282-6302`): `edge_create` строит `Edge::new(from, fromSide, to, toSide)` без потока и адресации; `flow_recalc` (`main.rs:6367-6384`) маппит `FlowOutputs`, игнорируя `FlowSolutions.lines`.

## Требуемые изменения

| Что → Где → Как |
|---|
| 1. `model.rs` → `Edge`: поля `to_param: Option<String>` (сериализация `toParam`, мягкое чтение — как `fromLine`, `model.rs:453-456`) и `from_output: Option<String>` (сериализация `fromOutput`). Инвариант round-trip: без полей — без ключей (старые `.canvas` байт-в-байт). Перепривязка from-конца (`edgegeom.rs:312`) сбрасывает и `from_line`, и `from_output`. |
| 2. `templates.rs` → `TemplateManifest`: `outputs: Vec<OutputSpec>` где `OutputSpec { name, unit: Option<String>, source }`, `source` — `Line(usize)` (индекс строки Numi-листа) или `Expr(String)` (подвыражение от параметров). Схема `template.json` 1.1 — поле опциональное (все 45 builtin-манифестов остаются валидными; добавить outputs приоритетно `api-gateway`, `lb`, `cache-redis`, `db-sql-*`). `instantiate`: снапшот outputs в `canvasdesk.template.outputs` (как expr, переживает удаление шаблона). |
| 3. `flow.rs` → резолв выходов: при пересчёте ребро с `from_output` резолвится по снапшоту outputs ноды-истока → индекс строки/подвыражение → значение (из `FlowSolutions.lines` или отдельным eval). Ребро с `to_param`: значение подставляется в `env.params[to_param]` ПОСЛЕ `with_param_map` (перекрытие локального параметра — семантика «проливание сильнее дефолта»). Позиционные слоты `$1..$N` для рёбер без `to_param` — без изменений (обратная совместимость FR-014). Несколько рёбер в один `to_param`: v1 — побеждает последнее по порядку `canvas.edges` (детерминировано), `flow_recalc` помечает узел warning; строгая ошибка — в `graph_validate` (FR-032, R2 CR-013). |
| 4. `canvas-mcp/src/lib.rs` → `edge_create`: опциональные `fromLine` (int ≥ 0), `fromOutput` (string), `toParam` (string), `kind` (`"value"|"control"`, дефолт `"control"`); взаимное исключение `fromLine`/`fromOutput` — ошибка схемы. Новый `edges_list`: `{id, from, to, kind, fromLine?, fromOutput?, toParam?, fromSide, toSide}`. `flow_recalc` v2: `{node_id: {value, unit, outputs: {имя: {value, unit}}, lines: [{index, value, unit}], warnings?}}`. |
| 5. `main.rs` → `mcp_dispatch`: ветки `edge_create` (валидация имён по снапшоту шаблона истока/приёмника — неизвестное имя выхода/параметра = ошибка вызова), `edges_list`, `flow_recalc` v2 (продолжать использовать `propagate_with_lines`). |
| 6. Тесты: round-trip `toParam`/`fromOutput` (в духе `model.rs:1299-1335`); flow — проливание в шаблон (`Трафик.peak_rps → gateway.rps` меняет итог `mm1`), перекрытие параметра, ребро без `to_param` — старое поведение; резолв `from_output` при правке текста (строки сдвинулись — связь жива); MCP — схемы `tools_list`, e2e `edges_list`/`edge_create(kind=value, toParam)` (паттерн тестов `mcp_dispatch` в `main.rs`). |
| 7. Документация: `user-docs/calculations.md` (семантика проливания), `docs/SPEC.md` §MCP (новые схемы), `assets/templates/` — outputs для базовых манифестов. |

Открытые вопросы (решить при реализации): конфликт «proливание против ручной правки параметра в UI» (кто сильнее при пересчёте после редактирования поля параметра) — предложение: ручная правка сбрасывает привязку ребра (с dialog-подтверждением в UI v2, в MCP v1 — silently unbind + warning в `flow_recalc`). **2026-09-22:** вопрос вынесен в FR-050 (Н1) вместе с остальными расхождениями перекрёстной проверки (13 пунктов) — см. `fr-050-spill-visibility-ui.md`.

## Проверка

- Юнит: round-trip сериализации новых полей; резолв `from_output` устойчив к сдвигу строк; перекрытие `to_param` > локального параметра; несколько рёбер в один `to_param` — детерминизм «последний побеждает»; `edge_create` валидирует имена портов.
- MCP e2e (чистые тесты `mcp_dispatch`): `template_instantiate` gateway → `edge_create {from: "tpl_1", to: "tpl_2", fromOutput: "peak_rps", toParam: "rps", kind: "value"}` → `flow_recalc` показывает значение gateway, зависящее от upstream, и `outputs` с именованными характеристиками.
- **Наглядная проверка (5 минут, демо-критерий этапа A1/CP1):** ручной мини-сценарий ADR-0005 — нода «Трафик» (`peak_rps` ≈ 1389 rps) → CDN (`origin_rps` ≈ 556 rps) → API Gateway (`to_param: rps`) — изменение DAU в «Трафик» одним вызовом `node_edit` меняет все downstream значения без правки связей/формул; числа на канвасе = таблице эталона (наблюдаемо без чтения кода).
- Гейты: `cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`.

## История изменений

- `2026-09-22` — агент (перекрёстная проверка): сметчинг с FR-014/FR-017/FR-025/
  FR-032/FR-033/FR-044/FR-045/FR-049 — выявлено 13 расхождений (приоритеты
  what-if/проливания/ручной правки; отсутствие UI-механизма `toParam`-связей;
  `line_ports` default OFF против «максимальной наглядности»; тихая деградация
  против видимого unmapped; дубль-входы; опциональность unit; схемы FR-049 на
  позиционных входах; длина нотации «Объект.Поле»; display-подстановка).
  Ответы владельца Q1–Q13 (сессия 2026-09-22) зафиксированы в **FR-050**
  (`fr-050-spill-visibility-ui.md`): Р-1 «what-if перекрывает всё», Р-2
  различение пролитых значений шрифтом/литералом, Р-3 подсветка + тултип
  «проблема + решение», Р-4 авто-numi-строка приёмника (новое требование).
  Настоящий FR остаётся записью реализованного v1 (MCP-контур).

- `2026-09-18` (2) — агент: реализация v1 (CP1 продуктового роадмапа §9):
  `Edge.from_output`/`to_param` (сериализация `fromOutput`/`toParam`, мягкое чтение,
  round-trip байт-в-байт старых `.canvas`; перепривязка истока сбрасывает и
  `fromOutput`), секция `outputs` манифеста (схема 1.1: `Line(i)`/`Expr(s)`,
  снапшот в `canvasdesk.template.outputs`, перенос при «Сохранить как шаблон»),
  проливание в параметры ПОСЛЕ локальных («проливание сильнее дефолта»),
  позиционные слоты без изменений для рёбер без `toParam` (обратная
  совместимость FR-014), именованные выходы текстовых нод — переменные
  Numi-листа (адресация `fromOutput` живёт при сдвиге строк), несколько рёбер
  в один `toParam` — последнее по `canvas.edges` + предупреждение в
  `flow_recalc` (строгая диагностика — FR-032), MCP `edge_create` v2
  (kind/fromLine/fromOutput/toParam, взаимные исключения, валидация имён по
  снапшотам), новый `edges_list`, `flow_recalc` v2 (value/unit + outputs +
  lines + warnings), `template_list` отдаёт outputs. Outputs добавлены в
  манифесты: cdn, tcp-lb (fan-out по долям), graphql (db_qps/events),
  db-sql-master (replica_load), db-sql-replica, queue-kafka (consume_rate),
  api-gateway, lb, cache-redis. Гейт CP1 закрыт автотестом
  `mcp_fr029_instagram_mvp_reference` (ADR-0005: 12 нод, 10 value-рёбер с
  адресацией, oracle ±1%, правка DAU одним `node_edit` удваивает цепочку).

- `2026-09-18` — агент: разметка связей — R2 оформлен как FR-032 (строгая диагностика дубль-входов/единиц), R3 — как FR-033 (батч поверх `edge_create` v2); позиция в критическом пути — CP1 (`docs/plans/product-roadmap.md` §9).
- `2026-09-17` — агент: создан из CR-013 (шаг R1 дорожной карты); статус «выявлено»; зафиксированы открытые вопросы по конфликту проливания и ручной правки.

## Источники истины

- `docs/change-requests/cr-013-agent-architecture-composition.md` — концепция, G1–G8, дорожная карта.
- `crates/canvas-core/src/model.rs:436-456,1299-1335` — `Edge::from_line`, паттерн мягкого чтения и round-trip тестов.
- `crates/canvas-core/src/flow.rs:231-350` — `FlowSolutions`, `inbound_slots_with_lines`.
- `crates/canvas-core/src/expr.rs:410-478` — `Env` (params/inbound).
- `crates/canvas-core/src/templates.rs:88-330` — манифест и `TemplateRef` (снапшоты).
- `crates/canvas-mcp/src/lib.rs:175-332,854-925` — TOOLS и тест схем.
- `crates/canvas-app/src/main.rs:6282-6384` — ветки `edge_create`/`flow_recalc`.
- `docs/adr/adr-0002-value-flow-composition.md`, `docs/adr/adr-0003-named-value-ports.md` — решения.
