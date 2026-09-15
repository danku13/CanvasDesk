# FR-019: Built-in библиотека из 15 архитектурных шаблонов

- **Статус:** выявлено
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-15): «набор шаблонных архитектурных нод». Уточнения (3 серии вопросов, 2026-09-15):
  - Built-in v1 — 10+5 (15 шаблонов): 10 backend + 5 network/transport.
  - Все 15 шаблонов фиксируются в FR-019 с конкретными дефолтами и формулами (тестируемость).
  - Связь ноды с шаблоном — linked + ручное update при новой версии.
  - Тесты — golden fixtures `.canvas` + schema-тесты + snapshot-тесты (3-уровневая стратегия).
  - Хранение — `assets/templates/` (как `assets/widgets/`), зашит в бинарник через `include_dir!`.
- **Связанные задачи:** FR-013 (`canvasdesk.expr`, `mm1/mmc` — доменные функции), FR-014 (поток значений — params передаются по рёбрам), FR-015 (queueing-theory функции в формулах шаблонов), FR-016 (bottleneck overlay на результатах), FR-017 (what-if на params), FR-018 (UI: wheel + палитра + шапка), FR-020 (custom templates), SPEC.md §5.1, §7.6 (образец `assets/widgets/`)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-15
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

FR-018 даёт UI (wheel, палитра, шапка) с mock-шаблонами. FR-019 поставляет
**реальную built-in библиотеку из 15 шаблонов** — 10 backend-ролей и 5
network/transport-ролей. Каждый шаблон — это `template.json` манифест с
params-схемой + `expr` формулой + SVG-иконкой + цветом по категории. Все 15
шаблонов зашиты в бинарник CanvasDesk через `include_dir!` (как
`assets/widgets/`), материализуются при старте в `%APPDATA%/canvasdesk/templates/`
(если их там нет — tombstone уважает ручное удаление, образец `WidgetRegistry`).

**Связь ноды с шаблоном — linked + ручное update:**
- При `template_instantiate` нода хранит `canvasdesk.template: { id, version, params }`.
- Если в новой версии CanvasDesk шаблон обновился (другой `version`) — в
  шапке ноды появляется индикатор «доступна новая версия v2».
- Пользователь жмёт «Update» → `expr` обновляется из нового манифеста, params
  сохраняются (если имена не изменились). Если в новой версии params-схема
  изменилась — миграционный диалог (отсутствующие params — дефолт из манифеста).

**Границы FR-019:**

- 15 шаблонов с конкретными params/expr/иконками (см. «Каталог шаблонов»).
- Round-trip с Obsidian (через `extra`, как `CanvasdeskExt`).
- UI update-индикатор (новая версия шаблона).
- MCP `template_list` отдаёт 15 реальных шаблонов (заменяет mock из FR-018).
- 3-уровневая стратегия тестирования: golden fixtures + schema-тесты + snapshot-тесты.

## Каталог шаблонов (15 шт.)

### Backend (10 шаблонов)

| # | ID | Name | Category | Color | Icon | Params | Expr (v1) |
|---|---|---|---|---|---|---|---|
| 1 | `com.canvasdesk.lb` | Load Balancer | backend | `#4A90E2` | `lb.svg` (весы) | `rps: rate, default 1000 rps, min 0`<br>`servers: count, default 2, min 1, max 100`<br>`service_rate: rate, default 1200 rps, min 0` | `mm1($rps, $service_rate, $servers)` |
| 2 | `com.canvasdesk.api-gateway` | API Gateway | backend | `#4A90E2` | `gateway.svg` (арка) | `rps: rate, default 1000 rps`<br>`latency_budget: time, default 50 ms`<br>`auth_overhead: time, default 5 ms` | `mm1($rps, 1 / ($latency_budget - $auth_overhead))` |
| 3 | `com.canvasdesk.cache-redis` | Cache (Redis) | backend | `#BD10E0` | `cache.svg` (молния) | `qps: rate, default 5000 rps`<br>`hit_rate: percent, default 0.85`<br>`eviction_latency: time, default 0.5 ms` | `mm1($qps × $hit_rate, 1 / $eviction_latency)` |
| 4 | `com.canvasdesk.db-sql-master` | DB SQL (master) | backend | `#F5A623` | `db.svg` (цилиндр) | `qps: rate, default 500 rps`<br>`query_time: time, default 10 ms`<br>`replicas: count, default 1` | `mm1($qps, 1 / $query_time, $replicas)` |
| 5 | `com.canvasdesk.db-sql-replica` | DB SQL (replica) | backend | `#F5A623` | `db-replica.svg` (цилиндр с правой полосой) | `qps: rate, default 300 rps`<br>`query_time: time, default 12 ms`<br>`replicas: count, default 3`<br>`lag: time, default 100 ms` | `mm1($qps, 1 / $query_time, $replicas)` |
| 6 | `com.canvasdesk.queue-kafka` | Queue (Kafka) | backend | `#9013FE` | `queue.svg` (стопка) | `produce_rate: rate, default 2000 rps`<br>`consume_rate: rate, default 1800 rps`<br>`partitions: count, default 6`<br>`retention: time, default 7 days` | `mm1($produce_rate, $consume_rate / $partitions)` |
| 7 | `com.canvasdesk.worker` | Worker | backend | `#4A90E2` | `worker.svg` (шестерёнка) | `tasks_per_sec: rate, default 100 rps`<br>`processing_time: time, default 50 ms`<br>`workers: count, default 4` | `mm1($tasks_per_sec, 1 / $processing_time, $workers)` |
| 8 | `com.canvasdesk.cdn` | CDN | backend | `#7ED321` | `cdn.svg` (глобус) | `rps: rate, default 10000 rps`<br>`cache_hit: percent, default 0.92`<br>`origin_latency: time, default 200 ms` | `mm1($rps × (1 - $cache_hit), 1 / $origin_latency)` |
| 9 | `com.canvasdesk.storage-s3` | Storage (S3) | backend | `#F5A623` | `storage.svg` (ведро) | `requests_per_sec: rate, default 500 rps`<br>`latency: time, default 30 ms`<br>`throughput: rate, default 100 MB/s` | `mm1($requests_per_sec, 1 / $latency)` |
| 10 | `com.canvasdesk.auth-service` | Auth Service | backend | `#4A90E2` | `auth.svg` (ключ) | `rps: rate, default 200 rps`<br>`token_verify: time, default 2 ms`<br>`cache_ttl: time, default 5 min` | `mm1($rps, 1 / $token_verify)` |

### Network / Transport (5 шаблонов)

| # | ID | Name | Category | Color | Icon | Params | Expr (v1) |
|---|---|---|---|---|---|---|---|
| 11 | `com.canvasdesk.http-endpoint` | HTTP Endpoint | network | `#7ED321` | `http.svg` (глобус) | `rps: rate, default 1000 rps`<br>`timeout: time, default 30 sec`<br>`keepalive: time, default 60 sec`<br>`max_connections: count, default 1000` | `mm1($rps, 1 / $timeout × $max_connections)` |
| 12 | `com.canvasdesk.grpc-service` | gRPC Service | network | `#7ED321` | `grpc.svg` (RPC стрелки) | `rps: rate, default 2000 rps`<br>`timeout: time, default 10 sec`<br>`stream_window: bytes, default 64 KB` | `mm1($rps, 1 / $timeout)` |
| 13 | `com.canvasdesk.websocket` | WebSocket | network | `#7ED321` | `websocket.svg` (волна) | `connections: count, default 5000`<br>`messages_per_sec: rate, default 10 rps`<br>`heartbeat: time, default 30 sec` | `mm1($connections × $messages_per_sec, 1 / $heartbeat)` |
| 14 | `com.canvasdesk.graphql` | GraphQL | network | `#7ED321` | `graphql.svg` (граф) | `rps: rate, default 500 rps`<br>`avg_complexity: count, default 10`<br>`resolver_time: time, default 5 ms` | `mm1($rps × $avg_complexity, 1 / $resolver_time)` |
| 15 | `com.canvasdesk.tcp-lb` | TCP Load Balancer | network | `#7ED321` | `tcp-lb.svg` (весы + TCP) | `connections: count, default 10000`<br>`bytes_per_conn: rate, default 1 MB/s`<br>`servers: count, default 3` | `mm1($connections × $bytes_per_conn, $servers)` |

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `assets/templates/` | 15 папок с `template.json` + SVG-иконками | `docs/SPEC.md` §7.6 (образец `assets/widgets/`) |
| `TemplateRegistry` (FR-018) | Замена mock-данных на real built-in через `include_dir!` | `crates/canvas-core/src/templates.rs` |
| `Node.canvasdesk.template` | UI update-индикатор при новой версии шаблона | `crates/canvas-render/src/cards.rs` |
| MCP `template_list` | Возвращает 15 реальных шаблонов | `crates/canvas-mcp/src/lib.rs` |
| Round-trip | 15 fixtures `tests/fixtures/templates/*.canvas` | `crates/canvas-core/tests/json_canvas_io.rs` |
| Тесты | 3 набора: golden (15 fixtures) + schema (15 манифестов) + snapshot (15 снапшотов) | `crates/canvas-core/tests/templates_*.rs` |

## Анализ (Root Cause)

FR-018 даёт mock-реестр. FR-019 заменяет mock на real built-in через
`include_dir!` (как `EMBEDDED_WIDGETS`, `registry.rs:12-13`). Точки встраивания:

- **`assets/templates/`** — новая папка в корне репо (как `assets/widgets/`,
  `assets/fonts/`). Структура:
  ```
  assets/templates/
  ├── com.canvasdesk.lb/template.json + lb.svg
  ├── com.canvasdesk.api-gateway/template.json + gateway.svg
  ├── ... (15 шт.)
  ```
- **`include_dir!`** — в `crates/canvas-core/src/templates.rs` (FR-018):
  ```rust
  static EMBEDDED_TEMPLATES: include_dir::Dir<'_> =
      include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/templates");
  ```
  Образец — `registry.rs:12-13`.
- **Материализация при старте.** При запуске CanvasDesk — копирует
  `assets/templates/*` в `%APPDATA%/canvasdesk/templates/`, если их там
  нет; tombstone уважает ручное удаление (образец `WidgetRegistry`).
- **`TemplateManifest` парсер.** В FR-018 — структура; FR-019 —
  `TemplateManifest::from_dir(dir: &Path) -> Result<Self, ManifestError>`
  (как `WidgetManifest::from_dir`, `manifest.rs:73-78`). Валидация:
  id, version, params (типы, min/max), expr (парсится Numi-парсером FR-013),
  icon (файл существует).
- **UI update-индикатор.** При загрузке `.canvas` с нодой, имеющей
  `canvasdesk.template.version = "1.0.0"`, а в текущем реестре — `"1.1.0"` →
  в шапке ноды — индикатор «⚡ v1.1.0 available». Клик → диалог миграции:
  показывает diff params-схемы (новые/удалённые params) + кнопку «Update».
- **MCP `template_list`** — возвращает 15 шаблонов с метаданными (id, name,
  version, category, params summary).

## Требуемые изменения (Changes)

1. **`assets/templates/`** — создать 15 папок с `template.json` + SVG-иконкой.
   Каждый `template.json` по схеме из FR-018:
   ```json
   {
     "id": "com.canvasdesk.lb",
     "name": "Load Balancer",
     "version": "1.0.0",
     "category": "backend",
     "role": "lb",
     "description": "L7 load balancer with M/M/c queueing model.",
     "params": [
       { "name": "rps", "type": "rate", "default": 1000, "unit": "rps", "min": 0 },
       { "name": "servers", "type": "count", "default": 2, "min": 1, "max": 100 },
       { "name": "service_rate", "type": "rate", "default": 1200, "unit": "rps", "min": 0 }
     ],
     "expr": "mm1($rps, $service_rate, $servers)",
     "color": "#4A90E2",
     "icon": "lb.svg"
   }
   ```

2. **`crates/canvas-core/src/templates.rs`** (FR-018) — заменить `mock()` на:
   - `static EMBEDDED_TEMPLATES: include_dir::Dir = include_dir!("...assets/templates");`
   - `pub fn builtin() -> Vec<TemplateManifest>` — парсит все 15 `template.json`
     из `EMBEDDED_TEMPLATES`, возвращает валидные.
   - `pub fn materialize_builtin(target_dir: &Path) -> Result<(), IoError>` —
     копирует `assets/templates/*` в `%APPDATA%/canvasdesk/templates/`, если
     там нет (tombstone уважает ручное удаление — образец `WidgetRegistry`).
   - `TemplateManifest::from_dir(dir: &Path) -> Result<Self, ManifestError>`
     — парсит `template.json` + проверяет, что `icon` файл существует в `dir`.

3. **`crates/canvas-render/src/cards.rs`** — UI update-индикатор:
   - Для ноды с `canvasdesk.template.version ≠ registry_version` — иконка «⚡»
     в шапке + tooltip «доступна новая версия v1.1.0».
   - Клик по «⚡» → диалог миграции (overlay): список diff params (новые /
     удалённые / изменённые дефолты) + кнопки «Update» / «Skip».
   - Update → `Node::set_template(new_version, migrated_params)` → `expr`
     обновляется из нового манифеста → propagator пересчитывает.

4. **`crates/canvas-mcp/src/lib.rs`** — `template_list` отдаёт 15 реальных
   шаблонов (заменяет mock из FR-018).

5. **`crates/canvas-core/tests/`** — 3 набора тестов (см. «Проверка»).

## Архитектура тестируемости (3-уровневая стратегия)

### Уровень 1: Schema-тесты (валидация манифестов)

- **Что:** для каждого из 15 `template.json` проверяется, что:
  - Манифест парсится без ошибок (`TemplateManifest::from_dir`).
  - `id` валиден (3..64 символа, `[a-z0-9.-]`, без `..`).
  - `version` — семантическая (semver).
  - Все `params` имеют валидные `type` (rate/count/time/bytes/percent/scalar),
    `default` соответствует типу, `min ≤ default ≤ max` (если заданы).
  - `expr` парсится Numi-парсером (FR-013) без ошибок.
  - `expr` ссылается только на params, объявленные в манифесте
    (нет неизвестных `$var`).
  - `icon` файл существует в папке шаблона.
- **Где:** `crates/canvas-core/tests/templates_schema.rs`.
- **Тестов:** 15 (по одному на шаблон) + общие тесты парсера.

### Уровень 2: Golden fixtures `.canvas`

- **Что:** для каждого из 15 шаблонов — fixture
  `tests/fixtures/templates/<id>.canvas` с нодой, инстанциированной из
  шаблона с дефолтными params. Тест:
  ```rust
  #[test]
  fn lb_canvas_fixture_matches() {
      let canvas = load_fixture("templates/com.canvasdesk.lb.canvas");
      let node = &canvas.nodes[0];
      assert_eq!(node.template().unwrap().id, "com.canvasdesk.lb");
      assert_eq!(node.template().unwrap().params["rps"], Value::Rate(1000.0, "rps"));
      let result = flow::propagate(&canvas, &HashMap::new(), &HashMap::new());
      assert_eq!(result["node_id"], Value::Struct { utilization: 0.833, ... });
  }
  ```
- **Где:** `crates/canvas-core/tests/templates_golden.rs` + 15 fixtures в
  `tests/fixtures/templates/`.
- **Тестов:** 15 golden + 1 test-runner.

### Уровень 3: Snapshot-тесты (регрессии)

- **Что:** для каждого из 15 шаблонов — `instantiate(template_id, defaults, 0, 0)` →
  `serde_json::to_string_pretty(&node)` → сравнение с эталоном
  `tests/snapshots/templates/<id>.snap`. Любое изменение в `template.json`
  (новый param, изменённый дефолт) → тест падает, видно в git diff, нужно
  обновить snapshot (`UPDATE_SNAPSHOTS=1 cargo test`).
- **Где:** `crates/canvas-core/tests/templates_snapshot.rs` + 15 `.snap` файлов.
- **Тестов:** 15 snapshot + 1 test-runner.

### Уровень 4: Интеграционный smoke-тест (опционально)

- **Что:** один тест на весь pipeline: `template_instantiate("com.canvasdesk.lb",
  {rps: 2000})` → propagator → `analyze_bottlenecks` → assert severity.
  Гарантирует, что шаблон работает во всём стеке FR-013..017.
- **Где:** `crates/canvas-app/tests/integration_templates_smoke.rs`.

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §7 (точки входа — 15 built-in шаблонов).
- `docs/SPEC.md` §5.1 (расширение `canvasdesk.template`), §7.6 (образец
  `assets/widgets/` для `assets/templates/`).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-019 (15 шаблонов + 3 набора
  тестов).
- `docs/change-requests/fr-013-text-node-numi-expr.md` — `expr` парсер
  (используется в schema-тестах).
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator (в golden
  fixtures).
- `docs/change-requests/fr-015-domain-units-queueing.md` — `mm1/mmc` в
  формулах шаблонов.
- `docs/change-requests/fr-018-template-palette-wheel-ui.md` — UI + mock
  (заменяется на real built-in).
- `docs/change-requests/fr-020-custom-templates.md` — custom templates
  (расширяют реестр).
- `crates/canvas-widgets/src/registry.rs:12-13, 55-60` — образец
  `include_dir!` + `WidgetRegistry` (для `TemplateRegistry`).
- `crates/canvas-widgets/src/manifest.rs:30-78` — образец парсера манифеста.

## Проверка (Verification)

### Уровень 1: Schema-тесты (`templates_schema.rs`)

- [ ] Все 15 `template.json` парсятся без ошибок.
- [ ] Все `id` валидны (3..64, `[a-z0-9.-]`, без `..`).
- [ ] Все `version` — semver.
- [ ] Все `params` имеют валидные `type` и `default` соответствует типу.
- [ ] Все `min ≤ default ≤ max` (если заданы).
- [ ] Все `expr` парсятся Numi-парсером FR-013.
- [ ] Все `expr` ссылаются только на объявленные params (нет неизвестных
  `$var`).
- [ ] Все `icon` файлы существуют в `assets/templates/<id>/`.

### Уровень 2: Golden fixtures (`templates_golden.rs` + 15 `.canvas`)

- [ ] Для каждого шаблона — fixture в `tests/fixtures/templates/<id>.canvas`.
- [ ] Загрузка fixture → нода имеет `canvasdesk.template.id == <id>`.
- [ ] `params` подставлены из манифеста (дефолты).
- [ ] `expr` вычисляется без ошибок через `flow::propagate`.
- [ ] Результат (utilization, queue_length, wait_time) соответствует
  ожидаемым значениям (допуск ±0.01).

### Уровень 3: Snapshot-тесты (`templates_snapshot.rs` + 15 `.snap`)

- [ ] Для каждого шаблона — snapshot в `tests/snapshots/templates/<id>.snap`.
- [ ] `instantiate(template_id, defaults, 0, 0)` → `serde_json::to_string_pretty`
  → совпадает с snapshot.
- [ ] Изменение в `template.json` → тест падает → нужно обновить snapshot
  (`UPDATE_SNAPSHOTS=1 cargo test`).

### Уровень 4: Smoke-тест (опционально)

- [ ] `template_instantiate("com.canvasdesk.lb", {rps: 2000})` → propagator
  → `analyze_bottlenecks` → severity обновляется.

### Ручная приёмка

- Wheel-меню (`Shift+клик`) → 6 секторов; в Backend — 10 шаблонов, в
  Network — 5. Все 15 доступны через wheel и палитру.
- Instantiate `com.canvasdesk.lb` → нода с иконкой весов, синей полосой,
  Numi-блоком `rps = 1000 rps\nservers = 2\nservice_rate = 1200 rps` и
  формулой `mm1(1000 rps, 1200 rps, 2)`.
- Изменение `rps = 2000` → формула пересчиталась `mm1(2000 rps, 1200 rps, 2)`,
  overlay FR-016 показывает новый severity.
- MCP `template_list` → 15 шаблонов с метаданными.
- MCP `template_instantiate("com.canvasdesk.db-sql-master", {qps: 500,
  query_time: 10 ms, replicas: 1}, 100, 100)` → нода создана.
- Update-индикатор: изменить `version` в `assets/templates/com.canvasdesk.lb/template.json`
  на `"1.1.0"`, перезапустить CanvasDesk → в шапке ноды — «⚡ v1.1.0
  available». Клик → диалог миграции → «Update» → `expr` обновлён, params
  сохранены.

## Открытые вопросы дизайна

- **Единицы в `default`.** В манифесте `default: 1000` + `unit: "rps"` или
  `default: "1000 rps"` (строка)? Решение: число + отдельное поле `unit`
  (как `ParamSpec` в FR-018). Это позволяет валидировать `min/max` как числа.
- **`expr` с `$param` vs `param`.** В Numi — `mm1($rps, ...)` или
  `mm1(rps, ...)`? Решение: `$param` (явный маркер переменной, не путается
  с единицами `rps`). Парсер FR-013 должен поддержать `$`-префикс.
- **Сложные шаблоны (CDN, GraphQL).** `mm1($rps × (1 - $cache_hit), 1 /
  $origin_latency)` — умножение rate на scalar. Парсер FR-013 должен
  поддержать. Если нет — упростить формулу до `mm1($rps, $service_rate)` с
  предвычисленным `service_rate` в params.
- **Миграция params при breaking change.** Если в v2 шаблона параметр
  `rps` переименован в `arrival_rate` — миграция: старый `rps` теряется,
  `arrival_rate` = дефолт из манифеста. Диалог показывает «param `rps`
  removed, `arrival_rate` added with default 1000 rps». Подтверждение
  пользователя обязательно.
- **Локализация name/description.** v1 — английский (`name: "Load Balancer"`).
  v2 — `name_ru`, `name_en` в манифесте.
- **Tombstone для удалённых шаблонов.** Если в новой версии CanvasDesk
  шаблон `com.canvasdesk.cdn` удалён — ноды с этим `template_id` показывают
  «⚡ template removed». `expr` остаётся (snapshot), но update невозможен.
  Образец — `WidgetRegistry` tombstone.

## История изменений (Changelog)
- `2026-09-16` — агент (аудит реализации всех CR/FR, main `984ca6b`): реализация не начата — `assets/templates/` нет, ни один из 15 шаблонов не найден, `EMBEDDED_TEMPLATES`/`template_list`/`template_instantiate` отсутствуют (в `assets/` только widgets/ и fonts/). Статус `выявлено` сохранён.


- `2026-09-15` — агент: документ создан по запросу пользователя (3 серии
  вопросов). Зафиксированы: 15 шаблонов (10 backend + 5 network) с
  конкретными params/expr/иконками; linked-нода с ручным update; 3-уровневая
  стратегия тестирования (schema + golden + snapshot). Статус `выявлено`.
  Зависимости: FR-013 (expr парсер), FR-014 (propagator), FR-015 (mm1/mmc),
  FR-018 (UI + mock, заменяется на real built-in), FR-020 (custom).

## Источники истины (References)

- `assets/templates/` (новая папка, 15 подпапок с `template.json` + SVG).
- `assets/widgets/` — образец структуры (clock, sticker, calendar).
- `crates/canvas-core/src/templates.rs` (FR-018) — `TemplateRegistry`,
  `TemplateManifest`, `instantiate` (расширить `builtin()`, `materialize_builtin`).
- `crates/canvas-core/src/model.rs:113-119, 150-153` — `CanvasdeskExt`,
  `Node.canvasdesk`, `extra` (round-trip `canvasdesk.template`).
- `crates/canvas-widgets/src/manifest.rs:30-78` — `WidgetManifest` (образец
  парсера манифеста с валидацией).
- `crates/canvas-widgets/src/registry.rs:12-13, 55-60` — `EMBEDDED_WIDGETS`
  (`include_dir!`), `WidgetRegistry` (образец для `TemplateRegistry`).
- `crates/canvas-render/src/cards.rs` — рендер шапки (update-индикатор).
- `crates/canvas-mcp/src/lib.rs` — `template_list` (real 15 шаблонов).
- `crates/canvas-core/tests/json_canvas_io.rs` — round-trip `canvasdesk.template`.
- `crates/canvas-core/tests/templates_schema.rs` (новый) — schema-тесты.
- `crates/canvas-core/tests/templates_golden.rs` (новый) — golden fixtures.
- `crates/canvas-core/tests/templates_snapshot.rs` (новый) — snapshot-тесты.
- `crates/canvas-core/tests/fixtures/templates/*.canvas` (15 новых fixtures).
- `crates/canvas-core/tests/snapshots/templates/*.snap` (15 новых snapshots).
- `docs/SPEC.md` §5.1 (расширения `.canvas`), §7.6 (образец `assets/widgets/`).
- `docs/interface-objects/node.md` §7 (точки входа — 15 built-in шаблонов).
- `docs/change-requests/cr-template.md` — шаблон.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — `expr` парсер.
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator.
- `docs/change-requests/fr-015-domain-units-queueing.md` — `mm1/mmc`.
- `docs/change-requests/fr-018-template-palette-wheel-ui.md` — UI + mock
  (заменяется на real built-in).
- `docs/change-requests/fr-020-custom-templates.md` — custom templates.
- Внешние источники:
  - Erlang-C / M/M/c: `https://en.wikipedia.org/wiki/Erlang_(unit)#Erlang_C_formula`.
  - Little's law: `https://en.wikipedia.org/wiki/Little%27s_law`.
  - JSON Canvas spec: `https://jsoncanvas.org/spec/1.0/`.
