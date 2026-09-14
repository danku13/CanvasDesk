# FR-016: Bottleneck + Queue risk индикаторы на канвасе

- **Статус:** выявлено
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-15): «CanvasDesk сразу показывает узкое место, риск очереди, SLA и эффект сценария что будет, если…». Уточнение владельца (2026-09-15): индикаторы v1 — bottleneck detection + queue risk + what-if (последний — FR-017); визуализация — поверх существующих нод (overlay), не отдельный объект.
- **Связанные задачи:** FR-013 (calc-движок — генерирует `Value::Struct` с `utilization`, `queue_length`, `wait_time`), FR-014 (поток значений — propagator обновляет `flow_results` для всех нод), FR-015 (доменные функции — `mm1/mmc` возвращают Struct), FR-017 (what-if — использует те же правила детекции), FR-009 (пункты меню — тогл режима анализа), FR-004 (хоткеи — `Ctrl+B` для тогла bottleneck-overlay), CR-001 (выделение — overlay не конфликтует с `selected_nodes`), SPEC.md §6.2 (LOD), §6.4 (текстуры overlay)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-15
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

На ландшафте из 5–50 calc-нод архитектор должен сразу видеть, **какая нода —
узкое место**, не читая каждую формулу. FR-016 вводит **analyzer-модуль** и
**визуальный overlay**:

- Нода с `utilization > 0.9` → **красная** рамка + badge `⚠ 95%`.
- Нода с `utilization > 0.7` (но ≤ 0.9) → **жёлтая** рамка + badge `80%`.
- Нода с `queue_length > 0` → badge `Q: 4.2` (длина очереди).
- Нода с `wait_time > SLA_target` (если SLA задан в формуле) →
  badge `⏱ 4.2 ms > 3 ms SLA`.
- Нода в `Overload` (ρ ≥ 1, `mm1`/`mmc` вернул ошибку) → **тёмно-красная**
  рамка + badge `OVERLOAD`.

Overlay — отдельный слой рендера (как `brokenLink`), не меняет `.canvas`
полей ноды (источник истины — формулы и результаты propagator-а). Режим
можно включать/выключать (`Ctrl+B` или пункт меню канваса), чтобы не мешать
обычной работе.

**Границы v1 FR-016:**

- Правила детекции — фиксированный набор (см. ниже «Требуемые изменения»).
  Кастомные правила (через манифест) — v2.
- Overlay — рамка + badges в шапке ноды. Тепловая карта (heatmap по
  utilization всей сцены) — v2.
- Анализ — синхронный после propagator (FR-014), не асинхронный.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `text`-нода (calc) | Новый overlay-слой: цветная рамка + badges | `docs/interface-objects/node.md` §3 (визуальная структура), §5 (состояния), §7 (точки входа) |
| Рендер | Новый проход `analysis_overlay` в `cards.rs` (после text-рендера, до selected-рамки); badge-инстансы | `crates/canvas-render/src/cards.rs`, `crates/canvas-render/src/zorder.rs` |
| `SceneState` | Новое поле `analysis_state: HashMap<String, AnalysisFlags>` (runtime-only, не в `.canvas`) | `crates/canvas-app/src/main.rs:193-228` |
| Ввод | `Ctrl+B` — тогл режима bottleneck-overlay; FR-009 пункт меню «Показать узкие места» | `docs/SPEC.md` §8, FR-004 (хоткеи), FR-009 (меню) |
| Undo | Overlay — НЕ undo-able (runtime-state, как `selected`); правки формул — undo-able через FR-006 | FR-006, `main.rs:311-330` |
| MCP | `analyze_bottlenecks()` — возвращает JSON-карту `{node_id: {utilization, queue_length, wait_time, severity}}` | `crates/canvas-mcp/src/lib.rs`, `main.rs:2872+` |

## Анализ (Root Cause)

FR-013..015 дают значения `utilization`, `queue_length`, `wait_time` в
`flow_results` (`SceneState`, FR-014). FR-016 — это слой интерпретации и
визуализации. Точки встраивания:

- **Нет analyzer-модуля.** Нужен `crates/canvas-core/src/analyze.rs` с
  чистой функцией `analyze(canvas: &Canvas, flow_results: &HashMap<String, Value>) -> HashMap<String, AnalysisFlags>`.
  Образец чистого модуля — `focus.rs:56-97`.
- **`AnalysisFlags` структура:**
  ```rust
  pub struct AnalysisFlags {
      pub utilization: Option<f64>,         // 0..1 (или >1 при overload)
      pub queue_length: Option<f64>,        // L_q
      pub wait_time: Option<Value>,          // W_q с единицей
      pub sla_target: Option<Value>,         // если задано в формуле
      pub severity: Severity,                // None | Warn | Critical | Overload
  }
  pub enum Severity { None, Warn, Critical, Overload }
  ```
- **`SceneState` без `analysis_state`.** Добавить в `main.rs:193` поле
  `analysis_state: HashMap<String, AnalysisFlags>` (runtime-only).
- **Рендер.** `cards.rs` строит инстансы для нод; overlay — отдельный слой
  после текста, до рамки `selected`. Образец — `brokenLink` (серая рамка) и
  `T23` (подсветка связей). `zorder.rs` — управляет порядком; overlay должен
  быть над текстом, под `selected` (чтобы выделение было видно).
- **Badge-инстансы.** Маленькие метки в шапке ноды (правый верхний угол).
  Рендер — как edge-лейблы (отдельные `card_instances` для badges).
- **Тогл режима.** `Ctrl+B` — добавление в HOTKEYS (`lib.rs` ui
  `HOTKEYS`, FR-004); пункт меню канваса (FR-009). Аналог `settings_overlay`
  в `main.rs`.
- **MCP.** `analyze_bottlenecks()` — для AI-агентов, проверяющих сценарии
  без UI. Возвращает JSON-карту с `severity` для каждой ноды.

## Требуемые изменения (Changes)

1. **`crates/canvas-core/src/analyze.rs`** — новый модуль (чистый):
   ```rust
   pub struct AnalysisFlags { ... }
   pub enum Severity { None, Warn, Critical, Overload }

   pub fn analyze(
       canvas: &Canvas,
       flow_results: &HashMap<String, Value>,
       config: &AnalysisConfig,
   ) -> HashMap<String, AnalysisFlags>
   ```
   - `AnalysisConfig` — пороги (по умолчанию: `warn_util: 0.7`,
     `critical_util: 0.9`, `warn_queue: 1.0`, `critical_queue: 10.0`,
     `warn_wait: 100 ms`, `critical_wait: 1 sec`). Кастомизация — v2.
   - Логика детекции (для каждой ноды с `flow_results`):
     - `Value::Struct` (FR-015, `mm1/mmc`) → извлечь `utilization`,
       `queue_length`, `wait_time` по ключам.
     - Скалярный `Value` с `unit = Percent` → трактовать как utilization.
     - `Value::Error(Overload)` → `severity = Overload`.
     - Сравнить с порогами → выставить `Severity`.
   - **Тестируемость:** чистая функция, детерминированная. Тесты на все 4
     уровня severity + на отсутствующие значения (без формулы →
     `AnalysisFlags::default()` с `severity = None`).

2. **`canvas-core/src/model.rs`** — без изменений (overlay не сериализуется).
   `Node` уже имеет `extra` для расширений; analyzer читает только
   `flow_results`, не трогает persisted state.

3. **`canvas-app/src/main.rs`**:
   - `SceneState` (`193`) — добавить `analysis_state: HashMap<String, AnalysisFlags>`.
   - После `flow::propagate` (FR-014) — вызвать `analyze::analyze(&canvas, &flow_results, &config)`
     → обновить `analysis_state` → `request_redraw()`.
   - `analysis_overlay_enabled: bool` в `SceneState` — тогл через `Ctrl+B`
     или меню. По умолчанию `false` (не мешает обычной работе); если на
     канвасе есть calc-ноды — авто-включение при первом запуске propagator.
   - `Ctrl+B` handler в `on_key` (`3177+`): тогл флага, `request_redraw`.
     Образец: `settings_overlay` (`main.rs:3243+`).
   - Пункт меню канваса (FR-009): «↻ Показать узкие места (Ctrl+B)» —
     чекбокс.

4. **`canvas-render/src/cards.rs`** — новый слой overlay:
   - Для каждой ноды с `analysis_state[id].severity != None`:
     - Рамка цвета по severity (warn — жёлтый `#F5A623`, critical —
       красный `#D0021B`, overload — тёмно-красный `#7A0010`).
     - Badge в шапке: `{utilization}%`, `Q: {queue_length}`,
       `⏱ {wait_time}` (показываем только непустые).
   - LOD: для `zoom < 0.6` — только цветная рамка (без badges); для
     `zoom < 0.25` — только overload (тёмно-красная).
   - Порядок слоёв (`zorder.rs`): фон → текст → result-строка (FR-013) →
     analysis-overlay → selected-рамка → edge-хэндлы.

5. **`canvas-render/src/zorder.rs`** — обновить порядок слоёв: overlay
   между `text` и `selected`.

6. **`canvas-mcp/src/lib.rs`** — новый инструмент `analyze_bottlenecks`:
   - Input: `{ canvas_id?: string }` (по умолчанию — активный).
   - Output: `{ nodes: [{ id, severity, utilization, queue_length, wait_time }] }`.
   - Не мутирует состояние — только чтение `analysis_state`.

7. **FR-004 (хоткеи)** — добавить `Ctrl+B` в `HOTKEYS` (23-я запись).

8. **FR-009 (меню)** — добавить пункт «Показать узкие места».

## Архитектура тестируемости (инварианты FR-016)

1. **`analyze` — чистая функция.** Вход: `Canvas`, `flow_results`,
   `AnalysisConfig`. Выход: `HashMap<node_id, AnalysisFlags>`. 0 side-эффектов.
   Тесты на все 4 severity + на `None` (без формулы) + на `Overload` (ρ ≥ 1).
2. **Пороги — в `AnalysisConfig`, не в коде.** Тесты с разными порогами →
   разные `severity` для одной и той же сцены. Это делает FR-017 (what-if)
   прозрачным: сценарий «что если поднять порог до 0.95» — просто другой
   `config`.
3. **Overlay — отдельный слой рендера.** Не меняет `Node` в `.canvas`, не
   мешает выделению (CR-001), не мешает рёбрам (CR-002/003). Тесты
   рендера (`render_smoke.rs`, `zorder_smoke.rs`) — дополняются: нода с
   `severity = Critical` → красная рамка, текст виден, selected-рамка
   видна поверх.
4. **MCP-видимость.** `analyze_bottlenecks` отдаёт тот же результат, что
   видит пользователь — это делает тестирование через AI-клиент
   эквивалентным ручной приёмке.

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §3 (визуальная структура — новый слой
  overlay), §5 (состояние `severity`), §7 (точки входа), §8 (чек-лист
  расширения — overlay не ломает LOD/текстуры).
- `docs/SPEC.md` §6.2 (LOD для overlay), §6.4 (текстуры — badges в атлас),
  §8 (ввод — `Ctrl+B`).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-016.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — calc-движок
  (источник `flow_results`).
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator (обновляет
  `flow_results`).
- `docs/change-requests/fr-015-domain-units-queueing.md` — доменные функции
  (`mm1/mmc` возвращают `Value::Struct`).
- `docs/change-requests/fr-017-what-if-scenarios.md` — what-if использует те
  же правила детекции на override-результатах.
- `docs/change-requests/fr-004-hotkeys-overlay.md` — `Ctrl+B`.
- `docs/change-requests/fr-009-node-context-menu-settings.md` — пункт
  «Показать узкие места».
- `docs/change-requests/cr-001-selection-multi.md` — overlay не конфликтует с
  `selected_nodes`.

## Проверка (Verification)

### Юнит-тесты

- `analyze::analyze(empty_canvas, empty_results, default_config)` → `HashMap::new()`.
- `analyze` с одной нодой, `flow_results[id] = Value::Struct { utilization: 0.95, queue_length: 5.0, wait_time: 5 ms }`
  → `severity == Critical`, `utilization == 0.95`, `queue_length == 5.0`.
- `analyze` с `utilization: 0.8` → `severity == Warn`.
- `analyze` с `utilization: 0.5` → `severity == None`.
- `analyze` с `flow_results[id] = Value::Error(Overload)` → `severity == Overload`.
- `analyze` с кастомным конфигом (`critical_util: 0.95`) → та же нода с
  `utilization: 0.9` теперь `severity == Warn` (не Critical).
- `analyze` с нодой без `expr` → `severity == None` (нет формулы — нет анализа).

### Интеграционные тесты

- `crates/canvas-app/tests/integration_analyze.rs`:
  - Создание 5 calc-нод с разными формулами (3 — `mm1` с разной нагрузкой,
    1 — скаляр `utilization: 0.5`, 1 — без формулы).
  - `flow_recalc` → `analyze_bottlenecks` через MCP → JSON содержит 5
    записей; severity распределено: 1 Critical, 1 Warn, 1 Overload, 1 None,
    1 None (без формулы).
  - Изменение формулы в Critical-ноде (снижение `arrival_rate`) → пересчёт
    → severity меняется на `Warn` или `None`.

### Ручная приёмка

- На канвасе 5 нод-сервисов с формулами `mm1(...)`. После `Ctrl+B`:
  - Нода с `ρ = 0.95` — красная рамка + badge `⚠ 95%`, `Q: 5.0`, `⏱ 5 ms`.
  - Нода с `ρ = 0.8` — жёлтая рамка + badge `80%`.
  - Нода с `ρ = 0.5` — без overlay.
  - Нода с `ρ = 2.0` (overload) — тёмно-красная рамка + badge `OVERLOAD`.
  - Нода без формулы — без overlay.
- Отключение `Ctrl+B` → overlay пропадает, формулы и тексты на месте.
- Выделение ноды (CR-001) — поверх overlay (видно обе рамки: красную +
  синюю selected).
- MCP: `analyze_bottlenecks` из AI-клиента — JSON-карта с 5 записями,
  severity соответствует визуалу.
- LOD: зум `< 0.6` — только цветные рамки (без badges); зум `< 0.25` —
  только overload.

## Открытые вопросы дизайна

- **Авто-включение overlay.** Если на канвасе есть calc-ноды, но `Ctrl+B`
  выключен — пользователь не видит узких мест. Решение: при первом запуске
  propagator на канвасе с calc-нодами — авто-включить overlay + toast
  «Включён режим анализа (Ctrl+B — выключить)».
- **Тепловая карта (heatmap) всей сцены.** v1 — overlay на ноды; v2 —
  фоновая подсветка областей с высокой плотностью узких мест (gradient
  поверх канваса).
- **Кастомные правила детекции.** v1 — фиксированный набор (utilization,
  queue_length, wait_time, overload). v2 — манифест правил (как `widget.json`
  для виджетов): пользователь задаёт `rule: { expr: "queue_length > 10",
  severity: Critical, label: "Длинная очередь" }`.
- **Производительность на 1000+ нод.** `analyze` — O(N) по нодам; для 1000
  нод — <1 мс. Рендер overlay — N инстансов; лимит — те же 5000 нод (SPEC
  §6.3).
- **Цветовая слепота.** Жёлтый/красный — различимы для большинства, но
  не для всех (дейтеранопия). v1 — цвета + текстовые badges (`⚠`, `Q:`,
  `⏱`); v2 — паттерны рамки (пунктир/штрих) как дополнительный сигнал.

## История изменений (Changelog)

- `2026-09-15` — агент: документ создан по запросу пользователя (bottleneck
  detection + queue risk + визуализация overlay). Зафиксированы 4 инварианта
  тестируемости (чистый `analyze`, пороги в config, overlay — отдельный
  слой, MCP-видимость эквивалентна UI). Статус `выявлено`. Зависимости:
  FR-013 (calc-движок), FR-014 (propagator), FR-015 (доменные функции
  `Value::Struct`), FR-017 (what-if — те же правила на override-результатах).

## Источники истины (References)

- `crates/canvas-core/src/analyze.rs` (новый) — чистый analyzer.
- `crates/canvas-core/src/focus.rs:56-97` — образец чистого модуля с обходом
  графа.
- `crates/canvas-core/src/model.rs:121-187` — `Node` (analyzer читает
  `Node.expr()`, не мутирует).
- `crates/canvas-app/src/main.rs:193-228` — `SceneState` (добавить
  `analysis_state`, `analysis_overlay_enabled`).
- `crates/canvas-app/src/main.rs:277-282` — `mark_dirty` (точка вызова
  `analyze` после propagator).
- `crates/canvas-app/src/main.rs:3177-3269` — `on_key` (добавить `Ctrl+B`).
- `crates/canvas-app/src/main.rs:3243+` — приглушение канвас-хоткеев (образец
  для overlay-тогла).
- `crates/canvas-app/src/main.rs:2872+` — `mcp_dispatch` (`analyze_bottlenecks`).
- `crates/canvas-render/src/cards.rs` — overlay-инстансы + badges.
- `crates/canvas-render/src/zorder.rs` — порядок слоёв.
- `crates/canvas-render/tests/render_smoke.rs`, `zorder_smoke.rs` — тесты
  рендера (дополнить).
- `crates/canvas-mcp/src/lib.rs` — `TOOLS` (+1 инструмент).
- `crates/canvas-render/src/theme.rs` — цвета severity.
- `docs/SPEC.md` §6.2 (LOD для overlay), §6.4 (текстуры — badges в атлас),
  §8 (ввод — `Ctrl+B`).
- `docs/interface-objects/node.md` §3 (визуальная структура), §5 (состояние
  `severity`), §8 (чек-лист расширения).
- `docs/change-requests/fr-013-text-node-numi-expr.md` — calc-движок.
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator (источник
  `flow_results`).
- `docs/change-requests/fr-015-domain-units-queueing.md` — доменные функции
  (`Value::Struct`).
- `docs/change-requests/fr-017-what-if-scenarios.md` — what-if использует те
  же правила.
- `docs/change-requests/fr-004-hotkeys-overlay.md` — `Ctrl+B`.
- `docs/change-requests/fr-009-node-context-menu-settings.md` — пункт меню.
- `docs/change-requests/cr-001-selection-multi.md` — overlay не конфликтует
  с `selected_nodes`.
