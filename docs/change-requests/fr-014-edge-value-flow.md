# FR-014: Поток значений по рёбрам + DAG-движок + live-ревал

- **Статус:** выполнено
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-15): «возможность передавать финальное значение от ноды в следующую ноду. в следующей ноде принимать значение прошлой ноды и применять для следующего цикла вычислений». Уточнение владельца (2026-09-15): значение передаётся по рёбрам (edges); граф — DAG, циклы запрещены; пересчёт — live (как таблица); тестируемость — обязательный инвариант.
- **Связанные задачи:** FR-013 (calc-нода и `expr`), FR-015 (доменные функции для расчёта downstream), FR-016 (анализ bottleneck по результатам FR-014), FR-017 (what-if использует propagator FR-014), FR-005 (MCP), FR-006 (undo), CR-002 (перепривязка рёбер), CR-003 (зона портов), SPEC.md §5.1 (edges), §8 (ввод)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-15 (реализация)
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Несколько FR-013-calc-нод связаны рёбрами: `A → B → C`. Формула в B ссылается на
значение A через `$in` (если ребро одно) или `$1`, `$2` (по индексу входящих рёбер
с `flow.kind = "value"`). При изменении A — B (и его downstream) пересчитывается
автоматически, в пределах одного кадра (live, как электронная таблица). Граф
обязан быть DAG — при попытке создать цикл пользователь видит предупреждение,
ребро создаётся как `control` (визуальная связь без потока значений) либо
отменяется (open question).

**Границы v1 FR-014:**
- Поток значений — только по рёбрам, помеченным `canvasdesk.flow.kind = "value"`.
- Переменные `$in` / `$1..$N` в `expr` — обращение к значениям с входящих
  value-рёбер. Именованные ссылки (`@node_name`) — отложены до v2.
- Граф — DAG. Циклы — ошибка с указанием участников.
- Live-ревал — пересчёт происходит синхронно после commit правки формулы или
  после изменения топологии графа (добавление/удаление/перепривязка ребра).
- Рантайм-данные (`flow_results: HashMap<node_id, Value>`) — НЕ в `.canvas`;
  источник истины — формулы + топология рёбер.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `edge` | Расширение `canvasdesk.flow: { kind: "value" \| "control" }` (по умолчанию `control` — существующее поведение) | `docs/SPEC.md` §5.1, `docs/interface-objects/edge.md` |
| `text`-нода + `canvasdesk.expr` | В `expr` доступны `$in`, `$1..$N` — входящие значения | FR-013, `crates/canvas-core/src/expr.rs` |
| Undo | Один undo-шаг на правку формулы или на изменение топологии; live-пересчёт downstream — НЕ undo-able (только источник) | FR-006, `crates/canvas-app/src/main.rs:311-330` |
| Рендер edge | Для `flow.kind = "value"` — лейбл ребра показывает единицу + значение (напр. `1000 rps`); для `control` — как сейчас (просто `label`) | `crates/canvas-render/src/cards.rs` (edges), `edgegeom.rs` |
| MCP | `flow_set_kind(edge_id, kind)`, `flow_recalc(canvas_id)` — инструменты для AI-агентов | `crates/canvas-mcp/src/lib.rs`, `crates/canvas-app/src/main.rs:2872+` |
| Ввод | Создание value-ребра: `Shift + drag от порта` (FR-004 хоткеи) либо тогл через ПКМ-меню связи (FR-009) | `docs/SPEC.md` §8, `docs/interface-objects/edge.md` |

## Анализ (Root Cause)

В текущей реализации ребро (`Edge`, `model.rs:271+`) — визуальная связь без
семантики потока данных. Изменение одной ноды не влияет на другие. Точки
встраивания:

- **Нет модуля топосортировки.** Нужен `crates/canvas-core/src/flow.rs` с
  чистой функцией `topo_sort(canvas: &Canvas) -> Result<Vec<usize>, CycleError>`.
  Образец обхода по рёбрам — `focus.rs:56-97` (`focus_set`, обход окрестности
  ноды).
- **`Edge` без расширения `flow`.** `Edge` хранится в `Canvas.edges`
  (`model.rs:271+`); round-trip через `extra` (как у `Node`). Решение:
  `Edge::flow_kind() -> FlowKind` — accessor, читающий `extra["canvasdesk"]["flow"]["kind"]`.
  По умолчанию `FlowKind::Control` — существующее поведение, обратная
  совместимость сохраняется.
- **`expr` без переменных окружения.** FR-013 вводит `Env` (карта переменных);
  FR-014 расширяет: `Env::Inbound(Vec<Value>)` для `$in`/`$1..$N`. `expr::eval`
  остаётся чистой функцией — только `Env` становится шире.
- **`mark_dirty()` не запускает propagator.** `main.rs:277` — помечает канвас
  грязным для автосейва; точка вызова propagator — после `mark_dirty`, в
  `begin_editing` commit или при `add_edge` / `retarget_edge` / `remove_edge`.
- **Перепривязка (CR-002) и zone (CR-003) не затрагиваются.** `retarget_edge`
  (`edgegeom.rs`) меняет `fromNode`/`toNode`; propagator запускается после
  мутации, как и для `add_edge`.
- **Удаление ноды (FR-007 cut, `delete_selected` `main.rs:1419`).** Удаление
  ноды с входящими value-рёбрами — downstream-формулы, ссылающиеся на
  `$in`/`$N` этой ноды, получают `Value::Missing`; рендер — красная строка
  с тултипом «вход отсутствует». Поведение аналогично `brokenLink` для file-нод.

## Требуемые изменения (Changes)

1. **`canvas-core/src/flow.rs`** — новый модуль (чистый Rust, без I/O):
   - `pub enum FlowKind { Control, Value }`
   - `pub fn topo_sort(canvas: &Canvas) -> Result<Vec<usize>, CycleError>`
     (алгоритм Кана; участники цикла — SCC размера > 1 и петли, итеративный
     Тарьян; образец обхода — `focus.rs:56-97`).
   - `pub fn propagate(canvas: &Canvas, overrides: &HashMap<String, Value>) -> Result<FlowOutputs, CycleError>`
     с `pub type FlowOutputs = HashMap<String, Result<Value, EvalError>>` —
     для каждой ноды в topo-порядке: собрать слоты входящих value-рёбер,
     вызвать `expr::eval`, сохранить результат. Сигнатура уточнена при
     реализации по верификационному списку (ниже, «правка 1»): ошибка
     вычисления одной ноды — запись `Err` в карте, не падение всего графа;
     топ-уровень — только `CycleError`. `overrides` — what-if (FR-017):
     подменённое значение замещает формулу ноды и виден downstream'у.
   - `pub fn value_path(canvas, start, goal)` / `pub fn creates_value_cycle(canvas, from, to)`
     — участники цикла при добавлении value-ребра (UI-диалог и MCP).
   - `pub fn inbound_slots(canvas, node_id, outputs)` — слоты входов ноды
     для построчных результатов листов (`eval_lines_in`).
   - `pub struct CycleError { pub nodes: Vec<String> /*участники цикла*/ }`
     — пользователю показывается список участников для отладки.
   - **Тестируемость:** чистые функции, детерминированные; на входе `Canvas`,
     на выходе — `HashMap` (или `Result`). 0 side-эффектов.

2. **`canvas-core/src/model.rs`** — accessor для `flow_kind` на `Edge`:
   - `impl Edge { pub fn flow_kind(&self) -> FlowKind }` — чтение из
     `self.extra.get("canvasdesk")?.get("flow")?.get("kind")`.
   - `pub fn set_flow_kind(&mut self, kind: FlowKind)` — мутация `extra`.
   - Round-trip: `extra` сохраняется,Obsidian видит неизвестное поле (как
     `Node.canvasdesk`).

3. **`canvas-core/src/expr.rs`** — расширить `Env` (правка 1: полем
   `inbound: Option<Vec<Option<Value>>>` вместо enum — вся существующая
   работа с `Env` сохраняется, семантика по верификации та же;
   `Env::with_inbound(slots)` — конструктор со входами):
   - `$in` → `Expr::Inbound` (единственный вход; несколько —
     `Err(AmbiguousInbound)`); целое `$N` (N ≥ 1) → `Expr::DollarAmount(N)` —
     контекстное разрешение в eval: есть входы — ссылка на N-й вход,
     нет — валюта (регрессия FR-013 `$5` = 5 долларов; для валюты в ноде
     потока — `5 usd`). Дробные суммы (`$2.5`) и `$0` — всегда валюта.
   - `eval` без входа — `Err(EvalError::MissingInbound { index })`;
     слот «ребро есть, значения нет» — тоже `MissingInbound`.

4. **`canvas-app/src/main.rs`**:
   - `SceneState` (`193`) — добавить `flow_results: HashMap<String, Value>`
     (runtime-only, не в `.canvas`).
   - После `mark_dirty()` (`277`) — если изменена нода с `expr` или топология
     value-рёбер — вызвать `flow::propagate(&canvas, &HashMap::new())`,
     обновить `flow_results`, `request_redraw()`. propagator запускается
     синхронно (сцена на ≤1000 нод — <10 мс по целевой метрике SPEC §6.3).
   - Undo: `push_undo` перед правкой формулы / `add_edge` / `retarget_edge`
     (как сейчас); propagator НЕ отменяем — он вычисляется из
     persisted-state. Это сохраняет инвариант FR-006 (источник истины —
     `Canvas`, runtime-state — `SceneState`).
   - При `delete_selected` (`1419`): если удалённая нода имела исходящие
     value-рёбра — propagator перерасчитает downstream с
     `EvalError::MissingInbound`. Автоудаление рёбер — нет (как сейчас).

5. **`canvas-render/src/cards.rs`**:
   - Для edge с `flow_kind = Value` — лейбл показывает `{value} {unit}`
     (напр. `1000 rps`), в дополнение к пользовательскому `label`.
   - Для ноды с `flow_results[id] = Ok(Value)` — рендер результата (FR-013
     уже добавил строку; FR-014 использует `flow_results` вместо локального
     `expr_results` — unify: одна карта на сценарий).

6. **`canvas-app/src/main.rs`** — UI создания value-ребра:
   - `Shift + drag` от порта → `EdgeDrag::New` (`3720-3743`) с флагом
     `flow_kind = Value` (хранить в `EdgeDrag` как временное состояние).
   - Тогл `Control ↔ Value` через ПКМ-меню связи (FR-009) — пункт
     «Поток значений: вкл/выкл».
   - При попытке создать ребро, замыкающее цикл, — диалог:
     «Обнаружен цикл: A → B → A. Создать как контрольную связь (без потока
     значений)?» — Да / Нет.

7. **`canvas-mcp/src/lib.rs`** — новые инструменты:
   - `flow_set_kind(edge_id, kind)` — тогл Control/Value.
   - `flow_recalc()` — форс-пересчёт всего графа; возвращает карту
     `node_id → { value, unit }` (для AI-агентов, проверяющих сценарии).
   - `flow_cycle_check()` — возвращает `[]` или список участников цикла.

## Архитектура тестируемости (инварианты FR-014)

1. **`topo_sort` — чистая функция.** `(canvas: &Canvas) -> Result<Vec<usize>, CycleError>`.
   Тесты на: пустой граф, одна нода, цепочка A→B→C, дерево, DAG слияние
   (A→C, B→C), цикл 2-х, цикл 3-х.
2. **`propagate` — чистая функция.** Вход: `Canvas` + `overrides` (для
   what-if, FR-017). Выход: `HashMap<node_id, Value>` или `PropagateError`.
   Тесты на: цепочка формул, слияние потоков, missing inbound, ошибка
   парсинга в downstream.
3. **DAG-инвариант.** Цикл — всегда `Err(CycleError)`; пользователь не может
   создать цикл value-рёбер. UI блокирует создание, propagator не падает.
4. **Round-trip `flow.kind`.** `Edge::set_flow_kind(Value)` → serialize →
   deserialize → `flow_kind() == Value`. `Control` по умолчанию —
   обратно совместим со старыми `.canvas`.

## Точки входа (Entry Points)

- `docs/interface-objects/edge.md` — новый тип ребра (value/control),
  тогл через ПКМ, поведение при цикле.
- `docs/interface-objects/node.md` §5 (состояние calc-ноды с входящими
  value-рёбрами), §6 (edge как поток), §7 (точки входа).
- `docs/SPEC.md` §5.1 (расширение `canvasdesk.flow`), §6.3 (целевая
  метрика propagator <10 мс), §8 (ввод — `Shift + drag`).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-014.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — базовый calc-движок.
- `docs/change-requests/fr-015-domain-units-queueing.md` — доменные функции
  для расчётов на ландшафте.
- `docs/change-requests/cr-002-edge-rebind.md` — `retarget_edge` (запуск
  propagator после).
- `docs/change-requests/fr-009-node-context-menu-settings.md` — пункт
  «Поток значений».

## Проверка (Verification)

### Юнит-тесты

- `flow::topo_sort(empty_canvas) == Ok([])`.
- `flow::topo_sort(chain A→B→C) == Ok([A, B, C])` (или любой валидный topo-порядок).
- `flow::topo_sort(cycle A→B→A) == Err(CycleError { nodes: ["A", "B"] })`.
- `flow::topo_sort(diamond A→B, A→C, B→D, C→D) == Ok([A, B, C, D])` (D — последний).
- `flow::propagate(A={5}, B={$in × 2}, C={$in + 1})` — цепочка A→B→C value-рёбра
  → `Ok({"A": 5, "B": 10, "C": 11})`.
- `flow::propagate` с missing inbound (A удалена, B ссылается на `$in`) →
  `Ok({"B": Err(MissingInbound)})` (часть downstream — ошибка, не падение).
- `Edge::set_flow_kind(Value)` → serialize → deserialize →
  `flow_kind() == Value`. `Edge::default()` → `flow_kind() == Control`.
- `expr::eval(parse("$in × 2"), Env::WithInbound { inbound: vec![5] })` → `10`.
- `expr::eval(parse("$1 + $2"), Env::WithInbound { inbound: vec![3, 7] })` → `10`.

### Интеграционные тесты

- `crates/canvas-app/tests/integration_flow.rs`:
  - Создание 3-х calc-нод через MCP `node_edit { id, expr }`.
  - Создание 2-х value-рёбер через MCP `flow_set_kind`.
  - Проверка: `flow_recalc` возвращает `{A: 5, B: 10, C: 11}`.
  - Изменение `A.expr = 7` → `flow_recalc` возвращает `{A: 7, B: 14, C: 15}`.
  - Удаление ребра A→B → `flow_recalc` для B → `Err(MissingInbound)`.
  - Попытка создать цикл A→B→A → MCP возвращает `isError: true` с
    `CycleError`.

### Ручная приёмка

- 3 ноды на канвасе, формулы `A=5`, `B=$in × 2`, `C=$in + 1`. Соединить
  `A→B` и `B→C` как value-рёбра (`Shift + drag`).
- Под A — `5`, под B — `10`, под C — `11`. Меняем формулу в A на `7` →
  через 1 кадр под B — `14`, под C — `15`.
- Создаём ребро `C→A` (value) → диалог «Цикл: A→B→C→A. Создать как
  контрольную связь?» → Да. Ребро создано, propagator не падает, value-поток
  не замкнут.
- Тогл `Value → Control` через ПКМ по ребру — лейбл ребра теряет значение,
  downstream-формула получает `MissingInbound` (красная строка с тултипом).
- Undo (`Ctrl+Z`) после тогла — ребро снова `Value`, downstream пересчитан.
- MCP: `flow_recalc()` из AI-клиента возвращает JSON-карту значений.

## Открытые вопросы дизайна

- **Цикл: блокировать или降级ить до Control?** Предлагаемое по умолчанию —
  диалог с выбором (значение по умолчанию — «контрольная связь», безопасный
  путь). Альтернатива — жёсткий запрет. Решение на этапе прототипа.
- **`$in` для нескольких входящих value-рёбер.** Если входящих value-рёбер
  больше одного, `$in` — ошибка (нужно `$1`, `$2`). Или `$in` = `sum($1..$N)`
  — удобный default, но неявный. Предлагаемое: `$in` требует ровно одно
  value-ребро, иначе `Err(AmbiguousInbound)`.
- **Кэш propagator.** При 1000+ нод topo-сортировка на каждой правке — дорого.
  Решение: инкрементальный propagator (пересчёт только downstream-поддерева).
  v1 — полный пересчёт, инкрементальность — v2.
- **Параллельный пересчёт.** Топология DAG позволяет rayon-параллелить слои
  Кана. v1 — последовательно (простота); параллельность — v2 при throughput
 瓶颈.

## История изменений (Changelog)

- `2026-09-15` — агент: документ создан по запросу пользователя (поток значений
  по рёбрам, DAG, live-ревал). Зафиксированы 4 инварианта тестируемости (чистый
  `topo_sort`, чистый `propagate`, DAG-инвариант с `CycleError`, round-trip
  `flow.kind`). Статус `выявлено`. Зависимости: FR-013 (calc-нода), FR-015
  (доменные функции), FR-017 (what-if использует propagator с overrides).
- `2026-09-15` — агент: реализация (статус `выполнено`). Правка 1 (по
  верификационному списку): `propagate` возвращает
  `Result<FlowOutputs, CycleError>` с per-node `Result<Value, EvalError>`
  (ошибка части downstream — не падение графа), `Env` расширен полем
  `inbound` (не enum). Решения открытых вопросов: `$in` требует ровно одно
  входящее value-ребро — иначе `AmbiguousInbound`; цикл value-рёбер в UI —
  диалог с фолбэком на control-связь (Да/Нет), в MCP — isError; полный
  пересчёт (v1), кэш/параллельность — v2. Валюта vs вход: целое `$N` при
  наличии входов — ссылка на вход, без входов и дробные суммы — валюта
  (`5 usd` — надёжная форма валюты в ноде потока). Live-ревал: propagator
  запускается приложением после любой мутации формул/топологии
  (`SceneState::recompute_flow`), построчные результаты Numi-листов
  вычисляются со входами ноды (`eval_lines_in`); вытеснение футера
  построчными результатами — правило рендера (text.rs). Edge-лейбл
  value-ребра — значение источника (`label · 1000 rps`), цвет value-ребра —
  бирюзовый (FLOW_EDGE_COLOR), явный цвет пользователя приоритетен.
- `2026-09-15` — агент: правка 2 (дефект живого UI-пути, найден при
  визуальном демо под Linux). `propagate` брал значение ноды только из
  `canvasdesk.expr` (MCP-формула) — заметки с Numi-формулами в тексте
  («1200 + 480») не участвовали в потоке: получатель `$in / 3` оставался
  с ошибкой «вход отсутствует», хотя построчный итог источника рисовался.
  Исправление: значение text-ноды = последняя формульная строка
  (`eval_lines_in` со входами; проза значения не даёт; явная
  `canvasdesk.expr` приоритетна). Побочно value-лейблы рёбер получили
  значения источников-заметок. Тесты: 4 регрессионных в `flow.rs`
  (UI-цепочка, многострочный итог, ошибка строки → downstream,
  приоритет `expr`); `per_line_results_numi_sheet` обновлён под карту
  потока (итог в `expr_results`, футер по-прежнему гасится построчными).

## Источники истины (References)

- `crates/canvas-core/src/model.rs:113-119` — `CanvasdeskExt` (образец).
- `crates/canvas-core/src/model.rs:121-187` — `Node`, `Node::text()`, `extra`.
- `crates/canvas-core/src/model.rs:271+` — `Edge` (добавить accessor `flow_kind`).
- `crates/canvas-core/src/focus.rs:56-97` — образец обхода графа по рёбрам
  (`focus_set` — для `topo_sort`).
- `crates/canvas-core/src/edgegeom.rs` — `retarget_edge` (запуск propagator
  после).
- `crates/canvas-app/src/main.rs:193-228` — `SceneState` (добавить
  `flow_results: HashMap<String, Value>`).
- `crates/canvas-app/src/main.rs:277-282` — `mark_dirty` (точка триггера
  propagator).
- `crates/canvas-app/src/main.rs:311-330` — `push_undo`/`undo` (паттерн).
- `crates/canvas-app/src/main.rs:1419` — `delete_selected` (каскад missing
  inbound).
- `crates/canvas-app/src/main.rs:3720-3743, 3864+` — `EdgeDrag::New` / drop
  (точка создания value-ребра).
- `crates/canvas-app/src/main.rs:2872+` — `mcp_dispatch` (новые инструменты
  `flow_*`).
- `crates/canvas-render/src/cards.rs` — рендер edge-лейбла с value.
- `crates/canvas-mcp/src/lib.rs` — `TOOLS` (+2 инструмента).
- `crates/canvas-core/tests/json_canvas_io.rs` — round-trip `flow.kind`.
- `docs/SPEC.md` §5.1 (расширение `canvasdesk.flow`), §6.3 (целевая метрика
  propagator), §8 (ввод — `Shift + drag`).
- `docs/interface-objects/edge.md` — тип ребра (value/control).
- `docs/change-requests/fr-013-text-node-numi-expr.md` — базовый calc-движок.
- `docs/change-requests/cr-002-edge-rebind.md` — `retarget_edge`.
- `docs/change-requests/fr-009-node-context-menu-settings.md` — пункт
  «Поток значений».
- `docs/change-requests/fr-006-undo-stack.md` — undo-паттерн.
