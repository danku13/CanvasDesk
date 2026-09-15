# FR-018: Палитра шаблонов — wheel-меню + боковая панель + шапка с inline Numi-полями

- **Статус:** выполнено (v1)
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (анализ)
- **Источник:** сообщение пользователя (сессия 2026-09-15): «нужно написать ещё один документ по реализации набора шаблонных архитектурных нод». Уточнения (3 серии вопросов, 2026-09-15):
  - Суть шаблонов — готовые роли (Load Balancer, DB, Cache и т.д.), каждая = одна text-нода с предзаполненным `canvasdesk.expr` и params-схемой.
  - Домен v1 — backend-сервисы + network/transport (15 шаблонов, см. FR-019).
  - Хранение — папка `templates/` (как widgets); built-in зашит в бинарник (FR-019), custom — `%APPDATA%/canvasdesk/templates/` (FR-020).
  - UX — wheel-меню (`Shift+клик`) + боковая палитра (как Miro/Figma); параметры — numi-like inline-блок в шапке ноды.
  - Связь с FR-013..017 — текст-нода + `canvasdesk.expr` + `canvasdesk.template`; после instantiate — linked с ручным update.
  - Сериализация — text-нода + `canvasdesk.template: { id, version, params }` (round-trip с Obsidian, образец `CanvasdeskExt`).
  - Иконки — inline SVG в `assets/templates/icons/`; уникальная иконка на роль.
  - MCP — `template_list()` + `template_instantiate(id, params, x, y)`.
  - Тесты — golden fixtures `.canvas` + schema-тесты + snapshot-тесты.
  - Scope FR-018 — UI-first с mock-шаблонами (3-5 заглушек в коде); FR-019 подключает built-in библиотеку; FR-020 — custom templates.
- **Связанные задачи:** FR-013 (`canvasdesk.expr` — базовый calc-движок), FR-014 (поток значений — params передаются по рёбрам через `$in`/`$1..$N`), FR-015 (доменные функции — `mm1/mmc` в шаблонах), FR-016 (анализ — overlay на шаблонных нодах), FR-017 (what-if — overrides на шаблонных нодах), FR-019 (built-in библиотека), FR-020 (custom templates), FR-004 (хоткеи — `Shift+клик`, `Ctrl+P` палитра), FR-009 (ПКМ-меню — «Сохранить как шаблон»), SPEC.md §5.1 (расширения `.canvas`), §6.2 (LOD), §7.6 (образец расширения виджета)
- **Создан:** 2026-09-15
- **Обновлён:** 2026-09-16 (реализация v1)
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Архитектор рисует ландшафт сервиса. Вместо того чтобы каждый раз писать
Numi-формулы с нуля, он открывает **wheel-меню** (`Shift+клик` по пустому
месту) или **боковую палитру** (`Ctrl+P`) → выбирает роль (Load Balancer,
DB, Cache и т.д.) → на канвасе появляется text-нода с предзаполненной
формулой и параметрами. В шапке ноды — inline Numi-блок с параметрами
(напр. `rps = 1000 rps`, `servers = 2`), которые пользователь редактирует
как обычный Numi-текст. Под параметрами — результирующая формула (read-only),
которая пересчитывается live (FR-013..014). Нода визуально отличается
от обычной text-ноды: уникальная SVG-иконка роли + цветная полоса по
категории.

**Границы FR-018 (UI-first):**

- Wheel-меню + боковая палитра + рендер шапки с Numi-параметрами + иконки
  + цвет по категории.
- 3-5 mock-шаблонов зашиты в код (для тестирования UI без зависимости от
  FR-019): `mock.lb`, `mock.db`, `mock.cache`, `mock.http`, `mock.queue`.
- Реальные built-in шаблоны (15 шт.) — FR-019; custom — FR-020.
- Связь ноды с шаблоном: `canvasdesk.template: { id, version, params }` —
  linked, с ручным update при новой версии шаблона (но сам UI update —
  в FR-019, т.к. зависит от реестра версий).
- MCP `template_list` / `template_instantiate` — в FR-018 (mock-данные);
  FR-019 расширяет list реальными шаблонами.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `text`-нода | Новое расширение `canvasdesk.template: { id, version, params }`; визуально — иконка + цвет + Numi-параметры в шапке | `docs/interface-objects/node.md` §2, §3, §5, §7 |
| Рендер | Новый слой в `cards.rs`: иконка роли + цветная полоса + inline Numi-параметры; LOD-стратегия для шапки | `docs/SPEC.md` §6.2, `crates/canvas-render/src/cards.rs` |
| Ввод | `Shift+клик` по пустому месту → wheel-меню; `Ctrl+P` → боковая палитра; двойной клик по шапке шаблонной ноды → редактирование Numi-параметров | `docs/SPEC.md` §8, `crates/canvas-app/src/main.rs:3387+` |
| Undo | Один undo-шаг на instantiate (создание ноды) и на правку параметров (как FR-006) | FR-006, `main.rs:311-330` |
| MCP | 2 новых инструмента: `template_list`, `template_instantiate` | `crates/canvas-mcp/src/lib.rs`, `main.rs:2872+` |
| Хоткеи | `Shift+клик` (wheel), `Ctrl+P` (палитра) — добавить в `HOTKEYS` | FR-004, `crates/canvas-app/src/lib.rs:406` |
| Ассеты | Новая папка `assets/templates/icons/` с SVG-иконками ролей | `assets/templates/icons/` |
| Формат `.canvas` | Новое поле `canvasdesk.template` внутри text-ноды; round-trip через `extra` (как `CanvasdeskExt`) | `docs/SPEC.md` §5.1, `crates/canvas-core/tests/json_canvas_io.rs` |

## Анализ (Root Cause)

В текущей реализации нет UI для быстрого добавления предзаполненных нод.
Точки встраивания:

- **`Node.canvasdesk`** (`crates/canvas-core/src/model.rs:150`) —
  `Option<CanvasdeskExt>`, где `CanvasdeskExt` (`model.rs:113-119`) —
  структура для виджет-ноды (`widget_id`, `props`). Для шаблонной ноды —
  расширить `extra` (round-trip через `extra: Map<String, Value>`,
  `model.rs:152-153`) под ключом `canvasdesk.template`. В runtime —
  accessor `Node::template() -> Option<TemplateRef>`.
  ```rust
  pub struct TemplateRef {
      pub id: String,
      pub version: String,
      pub params: HashMap<String, Value>,  // Numi-значения
  }
  ```
- **Нет реестра шаблонов.** Нужен `crates/canvas-core/src/templates.rs`
  (чистый модуль, без I/O) + `crates/canvas-widgets/src/registry.rs`-образец
  для файлового реестра (FR-019/020). В FR-018 — `TemplateRegistry` с
  mock-данными (3-5 шаблонов в коде).
- **Wheel-меню.** Новый UI-компонент в `crates/canvas-render/src/` (или
  `crates/canvas-app/src/lib.rs` рядом с `ContextMenu`, `lib.rs:257`).
  Триггер: `Shift+клик` по пустому месту — добавить в `on_left_button`
  (`main.rs:3387+`) проверку `self.modifiers.shift_key()` + клик по пустому
  (не по ноде). Образец overlay — `search_ui.rs:188` (`SearchPanel`),
  `hotkeys_panel_rect` (`lib.rs:440`).
- **Боковая палитра.** Новый компонент — список шаблонов с поиском.
  Открытие: `Ctrl+P` (новый хоткей в `HOTKEYS`, `lib.rs:406`). Рендер —
  как `hotkeys_panel` (`lib.rs:440-447`), но с фильтром.
- **Шапка с Numi-параметрами.** В `cards.rs` — для ноды с `canvasdesk.template`:
  - Иконка роли (SVG из `assets/templates/icons/`) — левый верхний угол.
  - Цветная полоса по категории (Backend=синий, Network=зелёный и т.д.) —
    образец `color` поля text-ноды (`node.md` §5).
  - Inline Numi-блок параметров (формат: `rps = 1000 rps\nservers = 2`).
  - Результирующая формула (read-only) — под параметрами.
- **Редактирование параметров.** Двойной клик по шапке → инлайн-редактор
  Numi-блока (расширение `begin_editing`, `main.rs:1058`). При commit —
  `Node::set_template_params(new_params)` → `mark_dirty` → propagator
  (FR-014) пересчитывает downstream.
- **MCP.** `template_list()` — возвращает mock-шаблоны; `template_instantiate(id, params, x, y)` —
  создаёт text-ноду с `canvasdesk.template`. Образец MCP-инструмента —
  `node_edit` (FR-005, `mcp_dispatch` `main.rs:2872+`).
- **Иконки.** Папка `assets/templates/icons/` (как `assets/fonts/`,
  `assets/widgets/`). Inline SVG, загружаются через `include_dir!` (как
  `EMBEDDED_WIDGETS`, `registry.rs:12-13`). Рендер — как текстуры в атлас
  (`SPEC.md` §6.4).

## Требуемые изменения (Changes)

1. **`crates/canvas-core/src/templates.rs`** — новый модуль (чистый):
   - `pub struct TemplateManifest { id, name, version, category, role,
     description, params: Vec<ParamSpec>, expr, color, icon }`
   - `pub struct ParamSpec { name, type: ParamType, default: Value, unit:
     Option<String>, min: Option<f64>, max: Option<f64> }`
   - `pub enum ParamType { Rate, Count, Time, Bytes, Percent, Scalar }`
   - `pub struct TemplateRef { id, version, params: HashMap<String, Value> }`
     (хранится в `Node.extra["canvasdesk"]["template"]`).
   - `pub struct TemplateRegistry { templates: HashMap<String, TemplateManifest> }`
   - `impl TemplateRegistry { pub fn mock() -> Self }` — 3-5 mock-шаблонов
     (`mock.lb`, `mock.db`, `mock.cache`, `mock.http`, `mock.queue`) с
     захардкоженными формулами.
   - `pub fn instantiate(registry: &TemplateRegistry, template_id: &str,
     params: HashMap<String, Value>, x: f32, y: f32) -> Result<Node, InstantiateError>`
     — создаёт text-ноду с `canvasdesk.template` + `canvasdesk.expr` (из
     манифеста с подстановкой params).
   - **Тестируемость:** чистые функции, детерминированные. 0 I/O.

2. **`crates/canvas-core/src/model.rs`** — accessor для `template`:
   - `impl Node { pub fn template(&self) -> Option<TemplateRef> }` — чтение
     из `self.extra.get("canvasdesk")?.get("template")`.
   - `pub fn set_template(&mut self, template: Option<TemplateRef>)` —
     мутация `extra`.
   - `pub fn template_params(&self) -> HashMap<String, Value>` — convenience.
   - `pub fn set_template_params(&mut self, params: HashMap<String, Value>)` —
     обновление params (версия шаблона сохраняется).

3. **`crates/canvas-render/src/template_ui.rs`** — UI wheel + палитра:
   - `pub struct WheelMenu { center: Vec2, category: Option<Category>,
     templates: Vec<&TemplateManifest> }`
   - `pub fn draw_wheel(menu: &WheelMenu, viewport: Vec2) -> WheelInput`
     — рендер двойного кольца: внешнее = 6 категорий, внутреннее = список
     шаблонов выбранной категории.
   - `pub struct PalettePanel { open: bool, filter: String, templates: Vec<&TemplateManifest> }`
   - `pub fn draw_palette(panel: &PalettePanel, viewport: Vec2) -> PaletteInput`
     — рендер боковой панели с поиском.
   - Образец overlay — `search_ui.rs:188` (`SearchPanel`), `hotkeys_panel_rect`
     (`lib.rs:440`).

4. **`crates/canvas-render/src/cards.rs`** — рендер шапки шаблонной ноды:
   - Для ноды с `canvasdesk.template`:
     - Иконка роли (SVG-текстура из `assets/templates/icons/`) — левый
       верхний угол, 24×24 px.
     - Цветная полоса по категории — сверху, 4 px высотой (Backend=`#4A90E2`,
       Network=`#7ED321`, Storage=`#F5A623`, Queue=`#9013FE`, Cache=`#BD10E0`,
       Custom=`#9B9B9B`).
     - Inline Numi-блок параметров: `rps = 1000 rps\nservers = 2` — под
       полосой, шрифт как у text-ноды.
     - Результирующая формула (read-only) — под параметрами (FR-013 style).
   - LOD: `zoom < 0.6` — только иконка + цвет (без Numi-блока); `zoom < 0.25` —
     только цветная полоса.

5. **`crates/canvas-app/src/main.rs`** — обработка ввода:
   - `SceneState` (`193`) — добавить `wheel_menu: Option<WheelMenu>`,
     `palette_panel: Option<PalettePanel>`.
   - `on_left_button` (`3387+`): если `self.modifiers.shift_key()` && клик
     по пустому месту → открыть `wheel_menu` в позиции курсора.
   - `on_key` (`3177+`): `Ctrl+P` → тогл `palette_panel`.
   - Wheel-выбор шаблона → `template_instantiate` → `add_node` →
     `push_undo` (один шаг).
   - Двойной клик по шапке шаблонной ноды → `begin_editing` (`1058`) в
     режиме Numi-параметров; commit → `set_template_params` → propagator
     (FR-014) → `request_redraw`.

6. **`crates/canvas-app/src/lib.rs`** — добавить хоткеи в `HOTKEYS` (`406`):
   - `("Shift + клик", "wheel-меню шаблонов")`
   - `("Ctrl + P", "палитра шаблонов")`

7. **`crates/canvas-mcp/src/lib.rs`** — 2 новых инструмента:
   - `template_list() -> Vec<TemplateSummary>` — список mock-шаблонов.
   - `template_instantiate(id, params, x, y) -> NodeSummary` — создание
     ноды.

8. **`assets/templates/icons/`** — 5 SVG-иконок для mock-шаблонов:
   `lb.svg` (весы), `db.svg` (цилиндр), `cache.svg` (молния),
   `http.svg` (глобус), `queue.svg` (стопка).

## Архитектура тестируемости (инварианты FR-018)

1. **`TemplateRegistry` — чистая структура.** `mock()` возвращает
   детерминированный набор 3-5 шаблонов. Тесты: `instantiate(mock.lb,
   {rps: 1000}, 0, 0)` → text-нода с `canvasdesk.template = { id: "mock.lb",
   version: "1.0.0", params: {rps: 1000} }` и `canvasdesk.expr = "mm1(1000
   rps, 1200 rps, 2)"` (подстановка params в expr).
2. **Round-trip `canvasdesk.template`.** `Node::set_template(Some(TemplateRef))` →
   serialize → deserialize → `template()` возвращает тот же `TemplateRef`.
   Тест в `json_canvas_io.rs`: fixture `text_with_template.canvas` —
   round-trip без потерь, Obsidian-формат валиден.
3. **Wheel и палитра — отдельные UI-компоненты.** Тесты рендера
   (`render_smoke.rs`): wheel открыт → 6 секторов; палитра открыта →
   список с фильтром. Не зависят от реестра (mock-данные).
4. **MCP-видимость эквивалентна UI.** `template_list()` отдаёт те же
   шаблоны, что видит пользователь в wheel/палитре. `template_instantiate`
   создаёт ноду, идентичную созданной через wheel.

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` §2 (типы — note про text+template),
  §3 (визуальная структура — иконка + цвет + Numi-параметры), §5 (состояние
  `template`), §7 (точки входа).
- `docs/SPEC.md` §5.1 (расширение `canvasdesk.template`), §6.2 (LOD для
  шапки шаблонной ноды), §8 (ввод — `Shift+клик`, `Ctrl+P`).
- `docs/ACCEPTANCE.md` — чек-лист приёмки FR-018.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — `canvasdesk.expr`
  (используется в шаблонной ноде).
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator (пересчёт
  при изменении params).
- `docs/change-requests/fr-019-builtin-template-library.md` — real built-in
  library (заменяет mock-данные).
- `docs/change-requests/fr-020-custom-templates.md` — custom templates.
- `docs/change-requests/fr-004-hotkeys-overlay.md` — `Shift+клик`, `Ctrl+P`.
- `docs/change-requests/fr-009-node-context-menu-settings.md` — пункт
  «Сохранить как шаблон» (FR-020).

## Проверка (Verification)

### Юнит-тесты

- `templates::TemplateRegistry::mock()` — возвращает 5 шаблонов (`mock.lb`,
  `mock.db`, `mock.cache`, `mock.http`, `mock.queue`).
- `templates::instantiate(mock.lb, {rps: 1000, servers: 2}, 0, 0)` →
  `Node` с `node_type = "text"`, `canvasdesk.template = { id: "mock.lb",
  version: "1.0.0", params: {rps: 1000, servers: 2} }`, `canvasdesk.expr =
  "mm1(1000 rps, 1200 rps, 2)"` (params подставлены в expr).
- `Node::set_template(Some(TemplateRef { id: "mock.lb", version: "1.0.0",
  params: {rps: 1000} }))` → `Node::template() == Some(...)`.
- Round-trip: serialize → deserialize → `template()` возвращает тот же
  `TemplateRef` (без потерь).
- `template_instantiate` с несуществующим id → `Err(InstantiateError::NotFound)`.
- `template_instantiate` с невалидным params (отрицательный rps при
  `min: 0` в манифесте) → `Err(InstantiateError::ParamOutOfRange)`.

### Интеграционные тесты

- `crates/canvas-core/tests/json_canvas_io.rs` — fixture
  `text_with_template.canvas`: text-нода с `canvasdesk.template`;
  round-trip через `serde_json` — поле сохранено, Obsidian-формат валиден.
- `crates/canvas-app/tests/integration_templates.rs`:
  - MCP `template_list()` → 5 mock-шаблонов.
  - MCP `template_instantiate("mock.lb", {rps: 1000, servers: 2}, 100, 100)`
    → нода создана, `expr` подставлен, propagator пересчитал downstream.
  - Undo (`Ctrl+Z`) → нода удалена; redo → восстановлена.
  - Двойной клик по шапке → Numi-редактор params; изменить `rps = 2000` →
    propagator пересчитал downstream.

### Ручная приёмка

- `Shift+клик` по пустому месту → wheel-меню с 6 секторами (Backend /
  Network / Storage / Queue / Cache / Custom). Внешнее кольцо — категории;
  клик по сектору → внутреннее кольцо со списком шаблонов категории.
- Выбор `mock.lb` → на канвасе появляется нода с иконкой весов, синей
  полосой (Backend), Numi-блоком `rps = 1000 rps\nservers = 2` и формулой
  `mm1(1000 rps, 1200 rps, 2)` (read-only).
- `Ctrl+P` → боковая палитра с поиском; ввод "lb" → фильтр показывает
  `mock.lb`.
- Двойной клик по шапке → инлайн-редактор Numi-блока; изменить `rps = 2000` →
  формула пересчиталась `mm1(2000 rps, 1200 rps, 2)`.
- Связь с другой нодой (FR-014) → значение `rps` передаётся downstream.
- Undo после правки params — возвращает предыдущие значения.
- LOD: зум `< 0.6` — только иконка + цвет; зум `< 0.25` — только цветная
  полоса.
- MCP: `template_list` из AI-клиента → 5 шаблонов;
  `template_instantiate("mock.db", {qps: 500}, 200, 200)` → нода создана.

## Открытые вопросы дизайна

- **Mock-шаблоны vs реальные.** FR-018 использует mock-данные (3-5 шаблонов
  в коде) для изоляции UI от FR-019. После FR-019 mock-данные удаляются,
  UI работает с real built-in library. Решение: mock-данные в `templates::mock()`
  помечены `#[cfg(test)]` или отдельным feature flag.
- **Wheel-меню позиционирование.** Центр wheel — в точке клика. Если клик
  у края экрана — wheel смещается, чтобы не выйти за viewport. Образец —
  `hotkeys_panel_rect` (`lib.rs:440`) с clamp к viewport.
- **Палитра — слева или справа?** По умолчанию — справа (как Miro). v1 —
  фиксировано; v2 — настройка в `config.toml` (`settings.rs`).
- **Numi-блок — моноширинный или пропорциональный?** Моноширинный (как
  код) — выравнивание `=`. v1 — моноширинный; v2 — настройка.
- **Иконки — `include_dir!` или lazy-load?** `include_dir!` (как widgets)
  — зашиты в бинарник, нулевая задержка. v1 — `include_dir!`.
- **Редактирование params — двойной клик или single?** Двойной клик (как
  text-нода) — консистентно. v1 — двойной клик по шапке.

## История изменений (Changelog)
- `2026-09-16` — агент (аудит реализации всех CR/FR, main `984ca6b`): реализация не начата — `templates.rs`/wheel-меню/`canvasdesk.template`/inline Numi-поля отсутствуют; слово «палитра» в коде относится к палитре выделения (FR-009/010), не к шаблонам. Статус `выявлено` сохранён.


- `2026-09-15` — агент: документ создан по запросу пользователя (3 серии
  вопросов). Зафиксированы решения: wheel+палитра, numi-params в шапке,
  linked-нода с ручным update, SVG-иконки, text+canvasdesk.template
  сериализация, UI-first scope с mock-шаблонами. 4 инварианта тестируемости
  (чистый registry, round-trip, UI-компоненты, MCP-эквивалентность).
  Статус `выявлено`. Зависимости: FR-013 (expr), FR-014 (propagator),
  FR-019 (real built-in), FR-020 (custom).
- `2026-09-16` — агент: **реализация v1 выполнена** (коммит FR-018).
  Реализовано: `templates.rs` (манифест/реестр/instantiate, mock из 5
  шаблонов), `$param`-грамматика в Numi (`$rps` — параметр шаблонной ноды;
  имена с префиксом `in` зарезервированы под валюту FR-013), поток
  (flow: template-формула + params в Env — приоритет над canvasdesk.expr),
  шапка шаблонной ноды в рендере (цветная полоса категории + квад-иконка
  роли + итог формулы в футере), правка параметров — редактирование
  Numi-листа текста ноды (синхронизация `set_template_params`), панель
  `Ctrl+P` (фильтр + чипы категорий + строки с иконками) и wheel по
  `Shift+клику` по пустому месту (2 кольца: категории → шаблоны; мишень
  инстанциации — world-точка клика), MCP `template_list` /
  `template_instantiate` (22 инструмента).
  **Уточнения владельца в ходе реализации:** иконки — квад-иконки (как в
  палитре действий, БЕЗ SVG/resvg — исходное решение про SVG в assets
  заменено).
  **Отклонения от исходного плана (осознанные, v1):**
  - `TemplateRef` — снимок `{id, version, expr, params, icon, color}`:
    icon/color добавлены в снимок (рендер шапки не обращается к реестру;
    переживает удаление шаблона из реестра). Хранение — типизированное
    поле `CanvasdeskExt.template` (сырой JSON), как `expr` в FR-013 —
    а НЕ свободный ключ в `extra`: serde-flatten вынес бы `canvasdesk`
    в типизированное поле и потерял бы template при round-trip.
  - Текст шаблонной ноды — Numi-лист присваиваний параметров
    (`rps = 1000 rps`); правка текста синхронизирует params (eval_lines);
    формула — снимок в `canvasdesk.template.expr` (не редактируется с
    канваса в v1).
  - Результирующая формула: в футере карточки — значение (итог формулы
    шаблона; не глушится построчными результатами параметров);
    полный текст формулы — в палитре/wheel и MCP template_list.
  - Сектора wheel — плашки-квады в центрах секторов (GPU-пайплайн без
    поворотов/вееров), hover-подсветка; rings: WHEEL_INNER_R 58 /
    WHEEL_OUTER_R 118.
  - LOD шапки (иконка/полоса при зуме) — отложен во FR-019 (когда полоса
    и иконка станут не декоративными, а навигационными).
  Тесты: core (templates ×6, expr +2 — параметры/масштаб единиц, model
  round-trip +2 в json_canvas_io), app (template_ui ×9, MCP ×3,
  template_params ×1), render (иконки/бэнд), clippy/fmt/test — зелёные.
  Фикс по пути: нормализация `Count¹·Time⁻¹ → Rate¹` в Numi-единицах
  сохраняла базу только для базовых секунд (`1 req / 10 ms` считалось как
  «0.1 req/s» вместо «100 req/s») — num результата теперь переводится в
  масштаб итоговой единицы (регресс-тест division_rescales_normalized_rate).

## Источники истины (References)

- `crates/canvas-core/src/model.rs:113-119` — `CanvasdeskExt` (образец
  расширения).
- `crates/canvas-core/src/model.rs:121-187` — `Node`, `Node::text()`,
  `extra: Map<String, Value>` (точка хранения `canvasdesk.template`).
- `crates/canvas-core/src/model.rs:150-153` — `canvasdesk` поле + `extra`
  flatten (round-trip).
- `crates/canvas-widgets/src/manifest.rs:30-78` — `WidgetManifest` (образец
  парсера манифеста с валидацией).
- `crates/canvas-widgets/src/registry.rs:12-13, 55-60` — `EMBEDDED_WIDGETS`
  (`include_dir!`), `WidgetRegistry` (образец реестра).
- `crates/canvas-render/src/search_ui.rs:40, 188, 203` — `SearchInput`,
  `SearchPanel` (образец overlay UI).
- `crates/canvas-app/src/lib.rs:257, 393-447, 406` — `ContextMenu`,
  `HOTKEYS_PANEL*`, `HOTKEYS` (образец overlay + хоткеи).
- `crates/canvas-app/src/main.rs:193-228` — `SceneState` (добавить
  `wheel_menu`, `palette_panel`).
- `crates/canvas-app/src/main.rs:311-330` — `push_undo`/`undo` (паттерн).
- `crates/canvas-app/src/main.rs:1058` — `begin_editing` (точка UI-ввода
  params).
- `crates/canvas-app/src/main.rs:2872+` — `mcp_dispatch` (`template_list`,
  `template_instantiate`).
- `crates/canvas-app/src/main.rs:3177-3269` — `on_key` (`Ctrl+P`).
- `crates/canvas-app/src/main.rs:3387+` — `on_left_button` (`Shift+клик`).
- `crates/canvas-render/src/cards.rs` — рендер text-ноды (добавить шапку
  шаблонной ноды).
- `crates/canvas-mcp/src/lib.rs` — `TOOLS` (+2 инструмента).
- `crates/canvas-core/tests/json_canvas_io.rs` — round-trip `canvasdesk.template`.
- `docs/SPEC.md` §5.1 (расширения `.canvas`), §6.2 (LOD), §6.4 (текстуры
  для иконок), §7.6 (образец расширения виджет-ноды), §8 (ввод).
- `docs/interface-objects/node.md` §2, §3, §5, §7 (точки входа).
- `docs/change-requests/cr-template.md` — шаблон.
- `docs/change-requests/fr-013-text-node-numi-expr.md` — `canvasdesk.expr`.
- `docs/change-requests/fr-014-edge-value-flow.md` — propagator.
- `docs/change-requests/fr-019-builtin-template-library.md` — real built-in.
- `docs/change-requests/fr-020-custom-templates.md` — custom.
- `docs/change-requests/fr-004-hotkeys-overlay.md` — хоткеи.
- `docs/change-requests/fr-009-node-context-menu-settings.md` — ПКМ-меню.
- `docs/change-requests/fr-006-undo-stack.md` — undo-паттерн.
- `docs/change-requests/fr-011-mindmap-object.md` — образец FR с расширением
  `.canvas`.
