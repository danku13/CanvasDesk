# FR-020: Custom templates — сохранение ноды как шаблон + конвертация Numi-expr в params-схему

- **Статус:** выявлено
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-15): «набор шаблонных архитектурных нод». Уточнения (3 серии вопросов, 2026-09-15):
  - Custom templates создаются из существующей ноды: ПКМ → «Сохранить как шаблон» → диалог → файл в `%APPDATA%/canvasdesk/templates/<id>/template.json`.
  - Параметры должны быть numi-like для пользователя (в UI выглядят как Numi-выражения), даже если манифест хранит params-схему.
  - Нода, которую надо превращать в шаблон, должна как-то конвертироваться из numi-like.
  - Связь ноды с шаблоном — linked + ручное update.
  - Тесты — golden fixtures + schema-тесты + snapshot-тесты (как FR-019).
- **Связанные задачи:** FR-013 (`canvasdesk.expr`, Numi-парсер — используется для извлечения params), FR-014 (поток значений — custom templates совместимы), FR-015 (доменные функции в custom шаблонах), FR-016 (bottleneck overlay на custom-нодах), FR-017 (what-if на custom), FR-018 (UI: wheel/палитра/шапка — добавляет custom в категорию «Custom»), FR-019 (built-in библиотека — образец структуры `template.json`), FR-005 (MCP `node_edit` — точка для `template_create_from_node`), FR-009 (ПКМ-меню ноды — пункт «Сохранить как шаблон»), FR-006 (undo — создание шаблона = один undo-шаг на ноду, не на файл), SPEC.md §5.1, §7.6 (образец `%APPDATA%/canvasdesk/widgets/`)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-15
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Пользователь нарисовал ландшафт сервиса, настроил несколько text-нод с
Numi-формулами (через FR-013). Один из узлов оказался удачной моделью —
например, кастомный «Throttled API Gateway» с формулой
`mm1(rps × 0.8, service_rate, servers)` и переменными `rps = 1000`,
`servers = 3`, `service_rate = 1200 rps`. Пользователь хочет сохранить
этот узор как **custom template**, чтобы переиспользовать в других канвасах.

FR-020 вводит **conversions Numi-expr → params-схема** и **UI создания
custom template из существующей ноды**:

1. ПКМ по ноде с `canvasdesk.expr` → пункт «Сохранить как шаблон».
2. Открывается диалог: имя, категория (по умолчанию `custom`), описание,
   выбор параметров (какие Numi-переменные станут params шаблона).
3. Конвертация: Numi-выражение разбирается, переменные `rps = 1000 rps`,
   `servers = 3` извлекаются как `params: [{name: "rps", default: 1000,
   unit: "rps"}, {name: "servers", default: 3, type: "count"}]`; формула
   `mm1(rps × 0.8, service_rate, servers)` превращается в `expr:
   "mm1($rps × 0.8, $service_rate, $servers)"` (переменные заменены на
   `$param`-синтаксис).
4. Файл сохраняется в `%APPDATA%/canvasdesk/templates/<id>/template.json`
   + SVG-иконка (по умолчанию — `custom.svg`, можно заменить).
5. Custom template появляется в wheel-меню (категория «Custom») и в
   палитре. Round-trip с Obsidian (как built-in, через `extra`).
6. MCP `template_list` отдаёт built-in + custom (как единый реестр).

**Границы FR-020:**

- Конвертация: Numi-выражение → params-схема + expr. Все Numi-переменные
  становятся params (можно отключить галочкой в диалоге).
- Хранение: `%APPDATA%/canvasdesk/templates/<id>/template.json` + опционально
  SVG. Custom templates не зашиты в бинарник.
- UI: ПКМ-меню ноды (FR-009) + диалог сохранения + категория «Custom» в
  wheel/палитре.
- Update-индикатор (FR-019) работает и для custom: если пользователь
  отредактировал custom template напрямую (в файле) — ноды показывают
  «доступна новая версия».
- MCP: только `template_list` (custom входят в общий список); отдельный
  MCP-инструмент `template_create_from_node` — отложен (v2). В v1 —
  создание только через UI.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `text`-нода с `canvasdesk.expr` | ПКМ-меню (FR-009) — новый пункт «Сохранить как шаблон» | `docs/interface-objects/node.md` §4, §7 |
| `TemplateRegistry` (FR-018/019) | Сканирует не только built-in, но и `%APPDATA%/canvasdesk/templates/` (custom) | `crates/canvas-core/src/templates.rs` |
| `expr` (FR-013) | Новый модуль `expr/extract.rs` — извлечение переменных из Numi-expr | `crates/canvas-core/src/expr/extract.rs` |
| Рендер wheel/палитры | Категория «Custom» в wheel (6-й сектор) + custom templates в палитре | FR-018 |
| Undo | Создание шаблона — НЕ undo-able (файловая операция, не нода). Undo применяется к ноде, если пользователь решит «применить шаблон к текущей ноде» (replace) | FR-006 |
| MCP | `template_list` отдаёт built-in + custom (без разделения) | FR-019 |
| Формат `.canvas` | Custom templates сериализуются так же, как built-in (`canvasdesk.template: { id, version, params }`); Obsidian видит неизвестное поле | `docs/SPEC.md` §5.1 |
| Файловая система | Новая папка `%APPDATA%/canvasdesk/templates/<id>/` (как `widgets/`) | `docs/SPEC.md` §7.6 (образец) |

## Анализ (Root Cause)

FR-019 даёт built-in реестр из 15 шаблонов (через `include_dir!` из
`assets/templates/`). FR-020 добавляет **runtime-загрузку custom templates
из `%APPDATA%/canvasdesk/templates/`** + **конвертацию Numi-expr в
params-схему**. Точки встраивания:

- **`TemplateRegistry` (FR-018/019).** Сейчас — `builtin()` через
  `include_dir!`. FR-020 добавляет `custom()` — скан `%APPDATA%/canvasdesk/templates/`,
  парсинг каждого `template.json`. Образец — `WidgetRegistry::scan`
  (`registry.rs:55-60`, см. `InstalledWidget` с `builtin: bool`).
- **Конвертация Numi-expr → params-схема.** FR-013 даёт `expr::parse` и
  `Expr` AST. Нужен `expr::extract_variables(expr: &Expr) -> Vec<(String,
  Value)>` — обходит AST, находит `Expr::Assign(name, value)` (присваивания
  вида `rps = 1000 rps`). Возвращает список переменных с их значениями.
  Затем `expr::parametrize(expr: &Expr, vars: &[String]) -> Expr` —
  заменяет `Expr::Var(name)` на `Expr::Param(name)` для указанных переменных
  (превращает `mm1(rps × 0.8, ...)` в `mm1($rps × 0.8, ...)`).
- **Диалог сохранения.** Новый overlay в `template_ui.rs` (FR-018):
  `SaveTemplateDialog { node_id, name, category, description, selected_params:
  HashSet<String>, icon: String }`. Образец — `hotkeys_panel` (`lib.rs:440`),
  `SearchPanel` (`search_ui.rs:188`).
- **ПКМ-меню ноды.** FR-009 даёт контекстное меню; FR-020 добавляет пункт
  «Сохранить как шаблон» для нод с `canvasdesk.expr`. Точка — `on_right_button`
  (`main.rs:3959+`), образец — `ContextMenu` (`lib.rs:257`).
- **Файловая операция.** Сохранение `template.json` в
  `%APPDATA%/canvasdesk/templates/<id>/` — через `std::fs` (как
  `WidgetRegistry` install, `registry.rs`). НЕ undo-able (вне `Canvas`).
- **MCP.** `template_list` (FR-019) уже возвращает все шаблоны; FR-020
  расширяет список custom-ами. Отдельный `template_create_from_node` —
  отложен (v2).

## Требуемые изменения (Changes)

1. **`crates/canvas-core/src/expr/extract.rs`** — новый подмодуль (чистый):
   ```rust
   /// Извлекает Numi-присваивания из выражения.
   /// `rps = 1000 rps\nservers = 3\nmm1(...)` →
   ///   [("rps", 1000 rps), ("servers", 3)]
   pub fn extract_variables(expr: &Expr) -> Vec<(String, Value)>

   /// Заменяет переменные на $param-синтаксис.
   /// `mm1(rps × 0.8, service_rate, servers)` + vars ["rps", "servers"] →
   ///   `mm1($rps × 0.8, service_rate, $servers)`
   pub fn parametrize(expr: &Expr, vars: &[String]) -> Expr

   /// Выводит тип параметра по значению.
   /// 1000 rps → Rate, 3 → Count, 50 ms → Time, 0.85 → Percent
   pub fn infer_param_type(value: &Value) -> ParamType
   ```
   - **Тестируемость:** чистые функции, детерминированные. Тесты на
     извлечение, parametrize, infer_type.

2. **`crates/canvas-core/src/templates.rs`** (FR-018/019) — расширить:
   - `pub fn custom(root: &Path) -> Vec<TemplateManifest>` — скан
     `%APPDATA%/canvasdesk/templates/`, парсинг каждого `template.json`.
     Образец — `WidgetRegistry::scan` (`registry.rs`).
   - `pub fn all(root: &Path) -> Vec<TemplateManifest>` — `builtin() +
     custom(root)` (дедупликация по `id`; custom выигрывает, если `id`
     совпадает с built-in — позволяет override built-in).
   - `pub fn save_custom(template: &TemplateManifest, root: &Path) ->
     Result<(), SaveError>` — запись `template.json` + дефолтная иконка
     `custom.svg` (если `icon` не указан). Образец — `WidgetRegistry::install`.
   - `pub fn delete_custom(id: &str, root: &Path) -> Result<(), IoError>` —
     удаление папки `<root>/<id>/`.

3. **`crates/canvas-render/src/template_ui.rs`** (FR-018) — добавить:
   - `pub struct SaveTemplateDialog { node_id, name, category, description,
     selected_params: HashSet<String>, extracted_params: Vec<(String, Value)> }`
   - `pub fn draw_save_dialog(dialog: &SaveTemplateDialog, viewport: Vec2)
     -> SaveDialogInput` — рендер формы.
   - Открытие: из `on_right_button` (`main.rs:3959+`) при выборе пункта
     «Сохранить как шаблон».
   - В диалоге:
     - Поле `name` (default: `node_id` или "Custom Template").
     - Поле `category` (default: "custom").
     - Список извлечённых params с чекбоксами (default: все выбраны).
     - Поле `description` (опционально).
     - Кнопки «Save» / «Cancel».
   - Save → `expr::parametrize(node.expr(), selected_params)` →
     `TemplateManifest { id: generated_id, params: selected_params, expr:
     parametrized, ... }` → `templates::save_custom(manifest, root)` →
     обновить `TemplateRegistry` → закрыть диалог.

4. **`crates/canvas-app/src/lib.rs`** — добавить пункт в контекстное меню
   ноды (`ContextMenu`, `lib.rs:257`):
   - `CanvasMenuItem::SaveAsTemplate` — доступен только для нод с
     `canvasdesk.expr` (не для всех text-нод).
   - Триггер: ПКМ по ноде → меню → «Сохранить как шаблон».

5. **`crates/canvas-app/src/main.rs`** — обработка:
   - `SceneState` (`193`) — добавить `save_template_dialog:
     Option<SaveTemplateDialog>`.
   - `on_right_button` (`3959+`): при выборе «SaveAsTemplate» → открыть
     диалог, заполнить `extracted_params` из `expr::extract_variables(node.expr())`.
   - В `update` (цикл событий): если `save_template_dialog` открыт —
     обработать ввод (имя, чекбоксы, кнопки). Save → `templates::save_custom`
     → `templates::all()` → обновить `TemplateRegistry` → закрыть диалог.
   - НЕ undo-able: файловая операция. Undo применяется только если
     пользователь выбрал «применить шаблон к текущей ноде» (replace) —
     тогда `push_undo` перед мутацией ноды.

6. **`crates/canvas-mcp/src/lib.rs`** — `template_list` (FR-019)
   расширяется: читает built-in + custom, возвращает единый список с
   полем `source: "builtin" | "custom"`.

7. **`assets/templates/icons/custom.svg`** — дефолтная иконка для custom
   templates (если `icon` не указан в манифесте). Простая SVG
   (напр., звёздочка или шестерёнка с плюсом).

## Архитектура тестируемости (инварианты FR-020)

1. **`extract_variables` / `parametrize` / `infer_param_type` — чистые
   функции.** Вход: `Expr` AST. Выход: `Vec<(String, Value)>` / `Expr` /
   `ParamType`. 0 I/O. Тесты:
   - `extract_variables(parse("rps = 1000 rps\nservers = 3\nmm1(...)"))` →
     `[("rps", 1000 rps), ("servers", 3)]`.
   - `parametrize(parse("mm1(rps × 0.8, ...)"), ["rps"])` → `mm1($rps × 0.8, ...)`.
   - `infer_param_type(1000 rps)` → `Rate`; `infer_param_type(3)` → `Count`.
2. **Round-trip custom template.** Создание custom template из ноды →
   сохранение в `%APPDATA%/.../templates/<id>/template.json` → перечитывание
   через `templates::custom(root)` → манифест идентичен. Тест на `tempdir`.
3. **Override built-in.** Если custom template имеет тот же `id` что и
   built-in (напр. `com.canvasdesk.lb`) — custom выигрывает (`templates::all`
   дедуплицирует). Тест: built-in + custom с тем же `id` → `all()` возвращает
   custom.
4. **MCP-видимость.** `template_list` отдаёт custom templates наравне с
   built-in. Тест: создать custom → `template_list` содержит его с
   `source: "custom"`.

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §4 (взаимодействия — ПКМ «Сохранить как
  шаблон»), §7 (точки входа — custom templates).
- `docs/SPEC.md` §5.1 (расширение `canvasdesk.template`), §7.6 (образец
  `%APPDATA%/canvasdesk/widgets/` для `templates/`).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-020.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — `expr` парсер
  (используется в `extract_variables`).
- `docs/change-requests/fr-018-template-palette-wheel-ui.md` — UI
  wheel/палитра (категория «Custom»).
- `docs/change-requests/fr-019-builtin-template-library.md` — built-in
  library (образец структуры `template.json`).
- `docs/change-requests/fr-009-node-context-menu-settings.md` — ПКМ-меню
  (пункт «Сохранить как шаблон»).
- `docs/change-requests/fr-006-undo-stack.md` — undo (файловые операции
  НЕ undo-able).
- `docs/change-requests/fr-005-mcp-node-edit.md` — образец MCP-инструмента.

## Проверка (Verification)

### Юнит-тесты (`expr/extract.rs`)

- `extract_variables(parse("rps = 1000 rps"))` → `[("rps", Value::Rate(1000, "rps"))]`.
- `extract_variables(parse("mm1(1000 rps, 1200 rps, 2)"))` → `[]` (нет
  присваиваний).
- `extract_variables(parse("rps = 1000 rps\nservers = 3\nmm1(rps, 1200, servers)"))` →
  `[("rps", 1000 rps), ("servers", 3)]`.
- `parametrize(parse("mm1(rps, 1200, servers)"), ["rps", "servers"])` →
  `parse("mm1($rps, 1200, $servers)")`.
- `parametrize(parse("mm1(rps, 1200, servers)"), ["rps"])` →
  `parse("mm1($rps, 1200, servers)")` (только указанные vars).
- `infer_param_type(Value::Rate(1000, "rps"))` → `ParamType::Rate`.
- `infer_param_type(Value::Count(3))` → `ParamType::Count`.
- `infer_param_type(Value::Time(50, "ms"))` → `ParamType::Time`.

### Интеграционные тесты (`integration_custom_templates.rs`)

- Создать text-ноду с `expr = "rps = 1000 rps\nservers = 3\nmm1(rps, 1200
  rps, servers)"`.
- Вызвать `save_template_from_node(node_id, name="My LB", category="custom")`.
- Проверить: файл `%APPDATA%/canvasdesk/templates/my-lb/template.json`
  существует, содержит `params: [{name: "rps", type: "rate", default: 1000,
  unit: "rps"}, {name: "servers", type: "count", default: 3}]` и `expr:
  "mm1($rps, 1200 rps, $servers)"`.
- `templates::all(root)` возвращает built-in (15) + custom (1) = 16.
- `template_instantiate("my-lb", {rps: 2000, servers: 5}, 0, 0)` → нода с
  `canvasdesk.template = { id: "my-lb", version: "1.0.0", params: {rps:
  2000, servers: 5} }` и `expr = "mm1(2000 rps, 1200 rps, 5)"`.
- Override: создать custom с `id = "com.canvasdesk.lb"` → `templates::all`
  возвращает custom (а не built-in) для этого `id`.

### Round-trip тесты (`json_canvas_io.rs`)

- Fixture `text_with_custom_template.canvas`: text-нода с
  `canvasdesk.template = { id: "my-lb", version: "1.0.0", params: {...} }`.
- Round-trip через `serde_json` → поле сохранено, Obsidian-формат валиден.

### Ручная приёмка

- Создать text-ноду, написать формулу `rps = 1000 rps\nservers = 3\nmm1(rps,
  1200 rps, servers)` (через FR-013 редактор).
- ПКМ по ноде → «Сохранить как шаблон» → диалог:
  - Имя: "My Custom LB"
  - Категория: "custom" (default)
  - Список params: `[✓] rps (rate, 1000 rps)`, `[✓] servers (count, 3)`
  - Описание: "Custom LB with 0.8 throttling" (опционально)
  - Кнопка «Save»
- Проверить: файл `%APPDATA%/canvasdesk/templates/my-custom-lb/template.json`
  создан, содержит params-схему + `expr: "mm1($rps, 1200 rps, $servers)"`.
- Wheel-меню (`Shift+клик`) → сектор «Custom» → «My Custom LB» доступен.
- Instantiate «My Custom LB» → нода с теми же params и формулой.
- MCP `template_list` → 16 шаблонов (15 built-in + 1 custom), custom
  помечен `source: "custom"`.
- Override: создать custom с `id = "com.canvasdesk.lb"` (переопределение
  built-in) → wheel показывает custom (не built-in).
- Перезапуск CanvasDesk → custom template всё ещё доступен (persisted в
  `%APPDATA%/`).
- Удаление: удалить папку `%APPDATA%/canvasdesk/templates/my-custom-lb/` →
  при следующем открытии wheel-меню custom template отсутствует; ноды с
  этим `template_id` показывают «⚡ template removed» (FR-019 update-индикатор).

## Открытые вопросы дизайна

- **Генерация `id` из `name`.** "My Custom LB" → `my-custom-lb` (kebab-case,
   латиница). Кириллица — транслитерация или запрет? Решение: транслитерация
   (напр. "Нагрузка" → "nagruzka"); конфликт `id` → суффикс `-2`, `-3`.
- **Все Numi-переменные → params?** По умолчанию — да (все `rps = ...`,
   `servers = ...` становятся params). Но некоторые переменные —
   вспомогательные (напр. `total = rps × servers` — не param, а вычисление).
   Решение: в диалоге — чекбоксы на каждую переменную; пользователь
   выбирает, какие станут params (остальные остаются в `expr` как локальные
   переменные).
- **`expr` с `$param` vs локальной переменной.** После parametrize:
   `mm1($rps, total)` где `total = $rps × 2` — смешанный. Парсер FR-013
   должен поддержать оба (нужно расширить грамматику).
- **Иконка.** По умолчанию — `custom.svg` (звёздочка). Пользователь может
   заменить SVG-файл в `%APPDATA%/canvasdesk/templates/<id>/`. UI выбора
   иконки — отложен (v2).
- **Миграция custom templates между версиями CanvasDesk.** Если в v2
   формат `template.json` изменился — миграция. v1 — не предусмотрен; v2
   — `template_migrate()` инструмент.
- **Шаринг custom templates.** Пользователь копирует папку из
   `%APPDATA%/canvasdesk/templates/` → другой пользователь кладёт в свою
   `templates/` → шаблон доступен. UI импорт/экспорт (.zip) — отложен (v2).
- **Конфликт `id` с built-in.** Custom с `id = "com.canvasdesk.lb"`
   переопределяет built-in. Это желаемое поведение (override) или баг?
   Решение: желаемое (override); пользователь видит в `template_list`
   `source: "custom (overrides builtin)"`.
- **Undo для создания шаблона.** Файловая операция — НЕ undo-able. Если
   пользователь хочет отменить — удалить через UI (пункт «Удалить шаблон»
   в палитре, отложен до v2) или вручную из `%APPDATA%/`.

## История изменений (Changelog)

- `2026-09-15` — агент: документ создан по запросу пользователя (3 серии
  вопросов). Зафиксированы: конвертация Numi-expr → params-схема через
  `extract_variables` + `parametrize` + `infer_param_type`; ПКМ «Сохранить
  как шаблон» + диалог; хранение в `%APPDATA%/canvasdesk/templates/`;
  override built-in; 4 инварианта тестируемости. Статус `выявлено`.
  Зависимости: FR-013 (expr парсер), FR-018 (UI + wheel/палитра категория
  «Custom»), FR-019 (built-in library — образец структуры), FR-009 (ПКМ-меню).

## Источники истины (References)

- `crates/canvas-core/src/expr.rs` (FR-013) — `Expr` AST, `expr::parse`
  (точка расширения для `extract_variables`).
- `crates/canvas-core/src/expr/extract.rs` (новый) — `extract_variables`,
  `parametrize`, `infer_param_type`.
- `crates/canvas-core/src/templates.rs` (FR-018/019) — `TemplateRegistry`,
  `TemplateManifest` (расширить `custom(root)`, `all(root)`, `save_custom`,
  `delete_custom`).
- `crates/canvas-core/src/model.rs:113-119, 150-153` — `CanvasdeskExt`,
  `Node.canvasdesk`, `extra` (round-trip `canvasdesk.template`).
- `crates/canvas-widgets/src/registry.rs:12-13, 55-60` — `EMBEDDED_WIDGETS`,
  `WidgetRegistry` (образец файлового реестра + tombstone).
- `crates/canvas-widgets/src/manifest.rs:30-78` — `WidgetManifest` (образец
  парсера манифеста).
- `crates/canvas-render/src/template_ui.rs` (FR-018) — `WheelMenu`,
  `PalettePanel` (расширить `SaveTemplateDialog`).
- `crates/canvas-app/src/lib.rs:257, 406` — `ContextMenu`, `HOTKEYS` (пункт
  «Сохранить как шаблон»).
- `crates/canvas-app/src/main.rs:193-228` — `SceneState` (добавить
  `save_template_dialog`).
- `crates/canvas-app/src/main.rs:3959+` — `on_right_button` (обработка
  пункта меню).
- `crates/canvas-app/src/main.rs:2872+` — `mcp_dispatch` (`template_list`
  расширяется custom).
- `crates/canvas-render/src/search_ui.rs:188` — `SearchPanel` (образец
  overlay-диалога).
- `crates/canvas-core/tests/json_canvas_io.rs` — round-trip custom template.
- `crates/canvas-core/tests/expr_extract.rs` (новый) — тесты `extract_variables`.
- `crates/canvas-app/tests/integration_custom_templates.rs` (новый) —
  создание + round-trip + override.
- `docs/SPEC.md` §5.1 (расширения `.canvas`), §7.6 (образец
  `%APPDATA%/canvasdesk/widgets/`).
- `docs/interface-objects/node.md` §4 (ПКМ-меню), §7 (точки входа).
- `docs/change-requests/cr-template.md` — шаблон.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — `expr` парсер.
- `docs/change-requests/fr-018-template-palette-wheel-ui.md` — UI.
- `docs/change-requests/fr-019-builtin-template-library.md` — built-in
  library (образец `template.json`).
- `docs/change-requests/fr-009-node-context-menu-settings.md` — ПКМ-меню.
- `docs/change-requests/fr-005-mcp-node-edit.md` — образец MCP.
- `docs/change-requests/fr-006-undo-stack.md` — undo (файловые операции
  НЕ undo-able).
