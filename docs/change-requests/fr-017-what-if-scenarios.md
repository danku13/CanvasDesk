# FR-017: What-if режим (override + delta + freeze)

- **Статус:** выявлено
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-15): «эффект сценария что будет, если…». Уточнение владельца (2026-09-15): what-if — один из 3 индикаторов v1 (вместе с bottleneck и queue risk из FR-016); пересчёт live; переопределение входов без изменения `.canvas`.
- **Связанные задачи:** FR-013 (calc-движок — формулы), FR-014 (propagator — принимает `overrides`), FR-015 (доменные функции — `mm1/mmc` в what-if сценариях), FR-016 (анализ bottleneck — на what-if результатах), FR-006 (undo — what-if сценарий одним шагом), FR-009 (меню — кнопка «Apply scenario»), FR-004 (`Ctrl+W` — тогл what-if), SPEC.md §5.1 (без мутаций `.canvas`), §8 (ввод)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-15
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Архитектор смотрит на ландшафт сервиса с базовыми параметрами: `rps = 1000`,
`replicas = 3`, `latency = 50 ms`. Он хочет задать вопрос: «что будет, если RPS
вырастет в 2 раза?» или «что если выключить одну реплику?». FR-017 вводит
**what-if режим**:

- Пользователь жмёт `Ctrl+W` (или кнопку в панели) → режим what-if активен.
- В шапке каждой calc-ноды появляется поле override: можно подставить
  альтернативное значение входа (`rps = 2000`) или множитель (`× 2`).
- Propagator пересчитывает весь DAG с учётом override-ов — **не меняя `.canvas`**.
- На канвасе видны **дельты**: рядом с каждым значением — `было → стало`,
  например `utilization: 50% → 83%`.
- Overlay из FR-016 пересчитывается на what-if результатах: новые узкие места
  подсвечиваются; старые — исчезают (или остаются, но с пониженной
  severity).
- Кнопка «Apply scenario» — сохраняет what-if в `.canvas` (одним undo-шагом);
  «Reset» — сбрасывает без изменений.
- Можно сохранить what-if как отдельный сценарий (v2 — несколько сценариев
  с переключением).

**Границы v1 FR-017:**

- Один активный what-if сценарий за раз. Множественные сценарии (с
  переключением и сравнением) — v2.
- Override — на уровне **входа** ноды (формула целиком, или отдельная
  переменная через `name := value` syntax). v1 — формула целиком; точечный
  override переменных — v2.
- Дельта-визуализация — `было → стало` рядом со значением. Heatmap
  дельт — v2.
- Apply/Reset — обязательно; автосейв не мутирует `.canvas` в режиме what-if
  (источник истины — persisted-формулы, not override-значения).

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `text`-нода (calc) | Поле override в шапке (только в what-if режиме); дельта-строка под результатом | `docs/interface-objects/node.md` §3, §5, §7 |
| `SceneState` | Новые поля: `whatif_overrides: HashMap<String, Value>`, `whatif_active: bool`, `baseline_results: HashMap<String, Value>` (snapshot для дельты) | `crates/canvas-app/src/main.rs:193-228` |
| Propagator (FR-014) | `flow::propagate` уже принимает `overrides` (заложено в FR-014); FR-017 использует это | FR-014 |
| Analyzer (FR-016) | Запускается на what-if результате; overlay показывает новые severity | FR-016 |
| Рендер | Дельта-строка (`50% → 83%`); изменение цвета severity (было None → Warn — пульсирующая рамка) | `crates/canvas-render/src/cards.rs` |
| Ввод | `Ctrl+W` — тогл what-if; кнопка «Apply» / «Reset» в панели what-if (отдельный оверлей) | `docs/SPEC.md` §8, FR-004, FR-009 |
| Undo | Apply сценария — один undo-шаг (как `node_edit`); Reset — не нужен undo | FR-006, `main.rs:311-330` |
| MCP | `whatif_set_override(node_id, value)`, `whatif_apply()`, `whatif_reset()`, `whatif_baseline()` — инструменты для AI-агентов | `crates/canvas-mcp/src/lib.rs`, `main.rs:2872+` |
| Автосейв | В what-if режиме — НЕ мутирует `.canvas`; только при Apply | `crates/canvas-app/src/main.rs:277-282` |

## Анализ (Root Cause)

FR-013..016 дают: формулы → propagator → flow_results → analysis. Все эти
данные — производные от `.canvas` (источник истины). What-if — это **временная
подмена входов** без мутации источника. Точки встраивания:

- **`SceneState` без what-if полей.** Добавить (`193`):
  - `whatif_active: bool` (тогл режима).
  - `whatif_overrides: HashMap<String /*node_id*/, Value>` (подмены формул).
  - `baseline_results: HashMap<String, Value>` (snapshot `flow_results` до
    применения overrides — для дельты).
  - `baseline_analysis: HashMap<String, AnalysisFlags>` (snapshot FR-016).
- **`flow::propagate` уже принимает `overrides`.** FR-014 зафиксировал сигнатуру
  `propagate(canvas, env_overrides) -> HashMap`. FR-017 использует `overrides`
  как подмены `expr` целиком (а не переменных env — отличие от FR-014). Решение:
  расширить сигнатуру — `propagate(canvas, expr_overrides: &HashMap<String, String>, env_overrides: &HashMap<String, Value>)`.
  v1 FR-014 — `expr_overrides` пустой; v1 FR-017 — заполняется.
- **`begin_editing` (`main.rs:1058`).** В what-if режиме — двойной клик по
  ноде открывает override-поле, не редактирование persisted-формулы.
  Коммит override → `whatif_overrides[id] = Value`; propagator запускается;
  `baseline_results` НЕ меняется (для дельты).
- **`mark_dirty` / `autosave_if_due` (`277-282`).** В what-if режиме —
  `mark_dirty` подавляется: `if !self.whatif_active { self.scene.mark_dirty() }`.
  Применение сценария (Apply) — мутация `.canvas` одним undo-шагом; тогда
  `mark_dirty` срабатывает.
- **Рендер дельты.** `cards.rs` — для ноды с `whatif_overrides[id]`:
  - Рядом с persisted-результатом — дельта-строка: `50% → 83%` (если
    baseline ≠ whatif).
  - Если severity вырос (None → Warn, Warn → Critical) — рамка пульсирует
    (`animate.rs`, образец `focus` анимации камеры).
- **MCP.** `whatif_*` — для AI-агентов, проверяющих сценарии без UI.
  Например, AI-агент задаёт: «что если уронить одну реплику на ноде B?» —
  MCP `whatif_set_override("B", "replicas = 2")` → `flow_recalc` →
  `analyze_bottlenecks` → возвращает JSON с новыми severity.

## Требуемые изменения (Changes)

1. **`canvas-core/src/flow.rs`** — расширить `propagate`:
   ```rust
   pub fn propagate(
       canvas: &Canvas,
       expr_overrides: &HashMap<String /*node_id*/, String /*expr*/>,
       env_overrides: &HashMap<String, Value>,
   ) -> Result<HashMap<String, Value>, PropagateError>
   ```
   - Для ноды с `expr_overrides[id]` — парсить/eval overrides, не
     `Node::expr()`.
   - Для остальных — как раньше.
   - `expr_overrides` пустой → поведение идентично FR-014 (обратная
     совместимость).

2. **`canvas-app/src/main.rs`**:
   - `SceneState` (`193`) — добавить `whatif_active: bool`, `whatif_overrides:
     HashMap<String, String>`, `baseline_results: HashMap<String, Value>`,
     `baseline_analysis: HashMap<String, AnalysisFlags>`.
   - `Ctrl+W` handler в `on_key` (`3177+`):
     - Тогл `whatif_active`.
     - При включении — snapshot `flow_results` → `baseline_results`;
       snapshot `analysis_state` → `baseline_analysis`.
     - При выключении без Apply — сброс `whatif_overrides`, восстановление
       `flow_results` из `baseline_results`.
   - `begin_editing` (`1058`): в what-if режиме — открыть override-input
     вместо редактирования persisted-формулы. Коммит →
     `whatif_overrides[id] = expr_string` → propagator → обновить
     `flow_results` (НЕ `baseline`) → `request_redraw`.
   - `mark_dirty` (`277`): подавление в what-if режиме — `if !self.whatif_active
     { self.scene.mark_dirty() }`.
   - Apply сценария (кнопка / хоткей / MCP `whatif_apply`):
     - `push_undo` (snapshot Canvas) — как у `node_edit` (FR-006).
     - Для каждой `whatif_overrides[id]` — `Node::set_expr(Some(override))`.
     - `mark_dirty` → автосейв.
     - Очистить `whatif_overrides`, `baseline_results`, `baseline_analysis`.
     - `whatif_active = false`.
   - Reset сценария (кнопка / MCP `whatif_reset`):
     - Очистить `whatif_overrides`.
     - Восстановить `flow_results` из `baseline_results`.
     - Восстановить `analysis_state` из `baseline_analysis`.
     - `whatif_active` может остаться true (для следующего сценария) или
       false — `open question`.

3. **`canvas-render/src/cards.rs`** — what-if визуализация:
   - В what-if режиме — для ноды с `whatif_overrides[id]`:
     - Override-поле в шапке (показывает формулу-override).
     - Дельта-строка под результатом: `было: 50% → стало: 83%` (если
       значения различаются).
     - Если `baseline_severity != whatif_severity` — пульсирующая рамка
       (animate.rs, ~1 Гц).
   - Без what-if — как обычно (FR-013..016).

4. **`canvas-render/src/animate.rs`** — пульсация рамки при смене severity
   (образец — фокус-анимация камеры, ~300 мс; для what-if — зацикленная).

5. **`canvas-mcp/src/lib.rs`** — 4 новых инструмента:
   - `whatif_set_override(node_id: str, expr: str)` — задать override.
   - `whatif_apply()` — применить сценарий к `.canvas` (одним undo-шагом).
   - `whatif_reset()` — сбросить overrides без изменений.
   - `whatif_baseline()` — вернуть JSON с baseline-результатами и
     whatif-результатами для сравнения (для AI-анализа дельт).

6. **`canvas-app/src/main.rs` (UI panel)** — панель what-if (отдельный
   оверлей, как `settings_overlay` `3243+`):
   - Кнопки «Apply scenario», «Reset», «Close what-if».
   - Список overrides: `node_id → expr (override)` с кнопкой «✕» для
     удаления.

7. **FR-004 (хоткеи)** — добавить `Ctrl+W` в `HOTKEYS` (24-я запись).

8. **FR-009 (меню)** — добавить пункт «What-if режим (Ctrl+W)» и «Apply
   scenario».

## Архитектура тестируемости (инварианты FR-017)

1. **`propagate` с overrides — чистая функция.** `expr_overrides` —
   параметр, не глобальное состояние. Тесты: один и тот же `canvas` с
   разными `expr_overrides` → разные результаты; `baseline` ≠ `whatif`
   детерминированно.
2. **What-if не мутирует `.canvas`.** Тест: до/после what-if без Apply —
   `.canvas` файл идентичен (хеш-сравнение). После Apply — мутируется
   одним undo-шагом (undo восстанавливает).
3. **Дельта — явное значение.** `baseline_results` хранится отдельно от
   `flow_results`; дельта — `whatif - baseline`. Тест: изменение
   `expr_overrides` → новая `flow_results`; `baseline` не меняется.
4. **MCP-видимость эквивалентна UI.** `whatif_baseline()` отдаёт тот же
   JSON, что видит пользователь в дельтах. Это делает тестирование через
   AI-клиент проверкой UX: если AI видит дельту, пользователь её видит тоже.

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §3 (визуальная структура — what-if
  overlay), §5 (состояние what-if), §7 (точки входа).
- `docs/SPEC.md` §5.1 (без мутаций в what-if), §6.2 (LOD для дельт), §8
  (ввод — `Ctrl+W`).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-017.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — calc-движок.
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator (с
  `expr_overrides`).
- `docs/change-requests/fr-015-domain-units-queueing.md` — доменные функции
  в сценариях.
- `docs/change-requests/fr-016-bottleneck-queue-risk.md` — overlay
  пересчитывается на what-if результатах.
- `docs/change-requests/fr-004-hotkeys-overlay.md` — `Ctrl+W`.
- `docs/change-requests/fr-009-node-context-menu-settings.md` — кнопки
  «Apply scenario» / «Reset».
- `docs/change-requests/fr-006-undo-stack.md` — Apply — один undo-шаг.

## Проверка (Verification)

### Юнит-тесты

- `flow::propagate(canvas, empty_overrides, empty_env)` → `baseline` (как
  FR-014).
- `flow::propagate(canvas, {"B": "$in × 3"}, empty_env)` → B пересчитан с
  множителем 3; A, C — как baseline.
- `flow::propagate(canvas, {"A": "2000"}, ...)` → A = 2000, B и C —
  пересчитаны downstream.
- `apply_scenario(canvas, overrides)` → мутация `Node::expr` для каждой
  ноды в `overrides`; возвращается новый `Canvas`.
- `reset_scenario` — `flow_results` восстановлены из `baseline_results`.

### Интеграционные тесты

- `crates/canvas-app/tests/integration_whatif.rs`:
  - Создание 3-х calc-нод A→B→C (A=`5`, B=`$in × 2`, C=`$in + 1`).
  - `flow_recalc` → baseline `{A:5, B:10, C:11}`.
  - `whatif_set_override("A", "20")` → `flow_recalc` → `{A:20, B:40, C:41}`.
  - Хеш `.canvas` файла — до и после whatif (без Apply) — идентичен.
  - `whatif_apply()` → `.canvas` мутирован (`A.expr = "20"`); `Ctrl+Z`
    восстанавливает исходное состояние.
  - `whatif_reset()` после нового whatif (без Apply) → `flow_results`
    восстановлены из `baseline`.

### Ручная приёмка

- 5 нод-сервисов с формулами `mm1(...)`. Жмём `Ctrl+W` → в шапке каждой
  ноды — поле override.
- Вводим в ноде A: `arrival_rate = 2000 rps` → propagator пересчитывает
  всю цепочку; рядом с каждым значением — дельта:
  `utilization: 50% → 83%`, `queue_length: 0.5 → 4.2`, `wait_time: 1 ms
  → 4.2 ms`.
- Overlay FR-016 обновляется: нода A теперь `Warn` (была `None`); нода B
  теперь `Critical` (была `Warn`); рамки пульсируют.
- Жмём «Reset» → всё возвращается к baseline (дельты пропадают, severity
  восстанавливается).
- Вводим новый override + жмём «Apply scenario» → `.canvas` мутирован;
  `Ctrl+Z` возвращает baseline.
- MCP: `whatif_set_override("A", "2000 rps")` → `whatif_baseline()` →
  JSON с `baseline: {A:5, B:10, C:11}, whatif: {A:2000, B:4000, C:4001}`.
- Автосейв: после whatif без Apply — `.canvas` файл не меняется (хеш
  идентичен); после Apply — обновляется.

## Открытые вопросы дизайна

- **Override переменных, не всей формулы.** v1 — формула целиком
  (`arrival_rate = 2000 rps`). v2 — точечный override переменных
  (`arrival_rate := 2000 rps` syntax, остальная формула сохраняется).
  Решение зависит от UX-тестинга.
- **Множественные сценарии.** v1 — один активный. v2 — список сценариев
  с переключением; v3 — сравнение 2-3 сценариев на одном канвасе
  (split-view).
- **Применение частичного сценария.** Apply — все overrides. Reset —
  все. Частичное (Apply только для ноды A, Reset для ноды B) — v2.
- **Производительность.** Propagator на 1000 нод с overrides — тот же
  overhead, что FR-014 (<10 мс). Пульсация рамок (animate.rs) — для 100+
  пульсирующих нод может дать лишний redraw; решение — ограничить FPS
  пульсации до 4 Гц (визуально достаточно).
- **Apply без правки `.canvas`.** Альтернатива — сохранять сценарий как
  отдельный `.whatif.json` (sidecar), не трогая `.canvas`. v1 — Apply
  мутирует `.canvas` (простота); v2 — sidecar для сравнения сценариев
  без committed-мутаций.
- **Имена сценариев.** v1 — безымянный (`whatif_active: bool`); v2 —
  `whatif_name: Option<String>` + список (`whatif_scenarios:
  Vec<Scenario>`).

## История изменений (Changelog)

- `2026-09-15` — агент: документ создан по запросу пользователя (what-if
  сценарии — один из 3 индикаторов v1). Зафиксированы 4 инварианта
  тестируемости (чистый `propagate` с overrides, без мутаций `.canvas` до
  Apply, явная дельта, MCP-видимость эквивалентна UI). Статус `выявлено`.
  Зависимости: FR-013 (calc-движок), FR-014 (propagator с overrides),
  FR-015 (доменные функции), FR-016 (overlay на what-if результатах).

## Источники истины (References)

- `crates/canvas-core/src/flow.rs` (FR-014) — `propagate` (расширить
  `expr_overrides`).
- `crates/canvas-core/src/analyze.rs` (FR-016) — `analyze` (на what-if
  результатах).
- `crates/canvas-core/src/model.rs:121-187` — `Node::expr()` / `set_expr()`
  (Apply мутирует).
- `crates/canvas-app/src/main.rs:193-228` — `SceneState` (добавить
  `whatif_active`, `whatif_overrides`, `baseline_results`,
  `baseline_analysis`).
- `crates/canvas-app/src/main.rs:277-282` — `mark_dirty` (подавление в
  what-if).
- `crates/canvas-app/src/main.rs:311-330` — `push_undo` (Apply — один
  undo-шаг).
- `crates/canvas-app/src/main.rs:1058` — `begin_editing` (в what-if —
  override-input).
- `crates/canvas-app/src/main.rs:3177-3269, 3243+` — `on_key` / overlay
  panel (Ctrl+W, what-if панель).
- `crates/canvas-app/src/main.rs:2872+` — `mcp_dispatch` (`whatif_*`).
- `crates/canvas-render/src/cards.rs` — дельта-строка, override-поле.
- `crates/canvas-render/src/animate.rs` — пульсация рамки.
- `crates/canvas-mcp/src/lib.rs` — `TOOLS` (+4 инструмента).
- `docs/SPEC.md` §5.1 (без мутаций в what-if), §6.2 (LOD), §8 (ввод).
- `docs/interface-objects/node.md` §3 (визуальная структура), §5 (состояние
  what-if), §7 (точки входа).
- `docs/change-requests/fr-013-text-node-numi-expr.md` — calc-движок.
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator с
  overrides.
- `docs/change-requests/fr-015-domain-units-queueing.md` — доменные функции.
- `docs/change-requests/fr-016-bottleneck-queue-risk.md` — overlay на
  what-if.
- `docs/change-requests/fr-004-hotkeys-overlay.md` — `Ctrl+W`.
- `docs/change-requests/fr-009-node-context-menu-settings.md` — Apply /
  Reset.
- `docs/change-requests/fr-006-undo-stack.md` — Apply — один undo-шаг.
