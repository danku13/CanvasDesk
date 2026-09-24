# CanvasDesk — анализ многослойного / 3D канваса

**Дата:** 2026-09-25
**Тип:** анализ (без реализации)
**Заказчик:** владелец продукта
**Источники:** аудит репозитория CanvasDesk @ commit `079f000` + `9ea0dc4` (post-refactor)

## 0. Постановка задачи

Проанализировать возможность превратить однослойный 2D-канвас в:
- **(a)** многослойный 2D-канвас с независимыми z-слоями и cross-layer операциями, ИЛИ
- **(b)** 3D-канвас с нарезкой по слоям для снятия пересечений графов в (псевдо-)3D пространстве.

Заказчик явно упомянул «снимать пересечения графов» — это смысловой центр задачи. Разработки НЕТ, только анализ.

---

## 1. Текущее состояние (факты из кода)

### 1.1. Модель данных (canvas-core/src/model.rs)

```
Canvas { nodes: Vec<Node>, edges: Vec<Edge>, extra: Map<String, Value> }
Node   { id, type, x, y, width, height, ... canvasdesk: Option<CanvasdeskExt> }
Edge   { id, from_node, to_node, from_side, to_side, label, color, ... }
```

- **НЕТ** z-индекса у Node.
- **НЕТ** поля `layer` или `z` ни у Node, ни у Edge.
- **НЕТ** 3D-координат — только `(x, y)` ∈ ℝ².
- Формат `.canvas` = JSON Canvas 1.0 (Obsidian-совместимый), плоский массив без слоёв.
- Расширения изолированы в `canvasdesk` (round-trip чистый с Obsidian).

### 1.2. Геометрия (canvas-core/src/edgegeom.rs, spatial.rs)

- `SpatialIndex` — **R-tree над 2D AABB** (через `rstar`), хранит индексы Node в `Vec`.
- `edge_at(canvas, point, avoid)` — 2D hit-test точки о кривую Безье (допуск 6 px).
- `EdgeBundleIndex` — группировка рёбер по упорядоченной паре `(from, to)` (для LOD-0 агрегации FR-042). Чисто runtime, не сериализуется.
- Все функции — чистая 2D-математика без GPU/ОС.

### 1.3. Рендер (canvas-render/)

- `GpuContext` — wgpu 22, формат `Rgba8UnormSrgb`. **НЕТ depth buffer** ни в одном pipeline.
- `Camera` — чистая 2D-математика: `position: [f32; 2]` + `zoom: f32`. Преобразование `screen ↔ world` — тривиальное линейное:
  ```
  screen = (world - position) * effective_zoom + viewport/2
  ```
- **Все 6 шейдеров** (`cards.wgsl`, `grid.wgsl`, `guides.wgsl`, `sectors.wgsl`, `thumbs.wgsl`, `minimap.wgsl`) используют ту же 2D-формулу. Вершинные данные — `vec2<f32>`, нет `vec3`/`vec4` с z-компонентой, нет матрицы проекции.
- ВСЕ pipelines: `depth_stencil: None`, `primitive: TriangleList` (default), без `cull_mode`, без `polygon_mode`.
- **Z-порядок — ручной** через `zorder.rs` — сегментация видимых нод на группы «карточки → тамбнейлы → тексты», рисуется в порядке массива инстансов с alpha-blending.
- `MainStage` (FR-042) — реализован как **«виртуальная камера»** через `StageTransform { scale, origin }` (масштаб + смещение в screen-px), **без второго GPU-бинда камеры**. Это важно: даже «второй слой детализации» сделан через композицию 2D, а не через 3D-проекцию.
- Миникарта — CPU-растеризация плоских прямоугольников + 1px-линий, не GPU-3D.

### 1.4. Сцена, UI, App

- `SceneState` (canvas-scene/src/scene.rs): `canvas + spatial + bundles + flow_baseline/active + whatif + view-модели`. Всё плоское.
- `canvas-ui` — слои экрана (overlay/UI-registry), **НЕ слои контента канваса**. HitStack, KeyboardRouter — все работают в screen-координатах.
- `App` — events/selection/drag — всё в 2D world-координатах.

### 1.5. Существующие механизмы «среза графа»

CanvasDesk уже умеет брать срезы графа, но в 2D-плоскости:

- **`Lineage` (lineage.rs)** — дерево происхождения цифры: DFS вверх по value-рёбрам от корня. Это и есть «срез» — подграф зависимостей. Используется в explain-панели (`lineage` MCP tool).
- **`WhatIf/Scenario` (whatif.rs)** — multi-scenario через override строк + freeze. Сравнение сценариев в `ScenarioComparison` — таблица (колонки = сценарии, строки = union переменных), **БЕЗ пространственной дельты**.
- **`EdgeBundleIndex`** — группировка параллельных рёбер для LOD-агрегации; одна из форм «граф-пересечения» (когда N рёбер между одной парой нод визуально сливаются).

**Вывод §1:** Текущий канвас полностью 2D — ни глубины, ни слоёв в модели/рендере/формате. Однако в кодовой базе уже есть семантические «срезы» графа (`Lineage`, `WhatIf`, `EdgeBundleIndex`) — но они либо плоские (`Lineage`), либо табличные (`WhatIf`), либо агрегирующие (`EdgeBundleIndex`). Пространственного измерения для этих срезов нет.

---

## 2. Опции архитектуры

### Опция A — Многослойный 2D-канвас

**Идея:** Canvas остаётся 2D, но каждая нода/ребро принадлежит одному из N слоёв. Слои переключаются (один активный), видны частично (полупрозрачные back-слои), либо видны «стопкой» (как слои в Photoshop/Figma). Cross-layer рёбра рисуются между слоями через 2D-проекцию.

**Хранение:**
- Node: `canvasdesk.layer: Option<String>` — id слоя (по умолчанию `"default"`).
- Новый root-объект `.canvas`: `canvasdesk.layers: [{id, name, color, visible, opacity, z_order}]`.
- Cross-layer edge: `from_layer`/`to_layer` выводятся из нод (явно не задаётся).

**Рендер:** тот же 2D-pipeline. Для «стопки» — слои рисуются последовательно с offset (pan/zoom вбирает смещение). Cross-layer edge — ломаная с точкой перегиба на границе слоёв (как GIMP onion-skin). Дешево по импактию на wgpu.

**UI:** панель слоёв (как Photoshop), переключение видимости, перетаскивание между слоями.

**Cross-section viewport:** отдельный режим, показывающий все слои «бок-о-бок» (мозаика) или «рядом» (split-view) — это уже не совсем 3D, а multiple viewports.

**Плюсы:**
- Минимальный импакт на GPU/wgpu-пайплайн — всё ещё 2D-ortho.
- Читаемый текст (карточки не наклонены).
- Совместимость с Obsidian/JSON Canvas 1.0 — `canvasdesk.layer` хранится как неизвестное поле, round-trip чистый.
- Лёгкая миграция существующих `.canvas` (авто-обёртка в 1 слой `"default"`).
- Web/wasm-friendly (нет depth buffer, нет perspective).

**Минусы:**
- **НЕ даёт «пересечения графов в 3D-пространстве»** — слои плоско переключаются, без ощущения глубины.
- Cross-layer edges — визуально грязные (точки перегиба, перетяжки).
- Сравнение сценариев через параллельные слои — нужно дублировать ноды, что дорого по памяти.

**Реальный use case:** «архитектурные уровни» (бизнес-логика / инфраструктура / данные — каждый на своём слое, видны по выбору). Это **не** пересечение графов, это их слоистая декомпозиция.

---

### Опция B — Псевдо-3D (2D-рендер с z-tilt / parallax)

**Идея:** Рендер остаётся 2D, но слои получают z-offset; камера может наклоняться («обзорная перспектива»), превращая слои в стопку наклонённых плоскостей. Cross-section viewport = ortho projection сбоку, показывающая слои как горизонтальные полосы + cross-layer edges как вертикальные связи.

**Хранение:** как в опции A — `canvasdesk.layers` + `canvasdesk.layer` per node.

**Рендер:**
- В обычном режиме: тот же 2D-pipeline (orthographic, без изменений).
- В «3D-режиме»: добавляется матрица проекции `mat4` (ортогональная с наклоном) — слои получают `z = layer_index * LAYER_SPACING`, проецируются матрицей `tilt_matrix * translate_layer_z`. Шейдеры апгрейдятся до `vec3` позиции + `mat4 camera` (но **без perspective divide** — орто сохраняет читаемость текста).
- Cross-section viewport — ortho сбоку: `x` сохраняется, `y` отображается в z, слои видны как параллельные полосы.

**Импакт на шейдеры:**
- `cards.wgsl`: `world: vec3<f32>`, `camera: mat4` (ортогональная). Vertex shader: `clip = camera * vec4(world, 1.0)`. Fragment shader — без изменений (SDF остаётся 2D в plane-координатах).
- `grid.wgsl`, `guides.wgsl`, `sectors.wgsl`, `thumbs.wgsl`, `minimap.wgsl` — аналогично.
- ВСЕ pipelines: `depth_stencil: Some(DepthStencilState { format: Depth32Float, ... })`, `cull_mode: None` (нам не нужен back-face culling — слои прозрачные).

**Плюсы:**
- Даёт **визуальное ощущение 3D-пространства** — слои видны как стопка наклонённых плоскостей, cross-section понятен.
- Сохраняет читаемость текста (ортогональная проекция, без perspective).
- Умеренный импакт на рендер — матрица + depth buffer, но без полного 3D-pipeline.
- Совместимость с Obsidian: расширения в `canvasdesk.layers`, round-trip чистый.
- Web/wasm: depth buffer + матрица — дёшево, доступно на WebGL2.

**Минусы:**
- Все шейдеры апгрейдятся (vec2 → vec3, добавляется mat4 bind) — это планомерная работа, но не переписывание.
- Hit-test: остаётся 2D для обычного режима (layer aware), но в 3D-режиме нужен raycast или projection-обратный маппинг.
- Drag между слоями — нужен явный z-shift жест (Shift+drag = «перетащить в другой слой»).
- Текст на наклонённой плоскости в крайних углах наклона искажается (нужен кламп угла наклона ≤ 30°).

**Реальный use case:** «сравнить 3 сценария / 3 архитектуры как стопку прозрачных слоёв, посмотреть пересечения и отличия». Это именно то, что просил заказчик.

---

### Опция C — Полный 3D-канвас (wgpu 3D pipeline)

**Идея:** Полноценный 3D — perspective camera, depth buffer, 3D-координаты у нод, ноды — 3D-боксы (или плоскости в 3D), слои = z-плоскости. Cross-section = любой 3D-cut (например, ortho-plane через всё пространство).

**Хранение:**
- Node: `x, y, z, width, height, depth` — 3D-размеры. Либо слои = z-срезы, либо ноды свободны в 3D.
- Edge: `from_node, to_node, from_side, to_side` — стороны 3D-бокса (6 вариантов вместо 4).

**Рендер:**
- wgpu 3D-pipeline: perspective camera с FOV, depth buffer (Depth24Plus), back-face culling.
- Все шейдеры: `vec3` позиция, `mat4 view_projection`, перспективное деление.
- Карточки: 3D-боксы с текстурой текста на лицевой грани (или 2D-billboards, всегда повёрнутые к камере).
- Cross-section: ortho-projection из произвольной точки (либо slice-plane shader, режущий всё что вне плоскости).

**Плюсы:**
- Полная 3D-свобода — можно разместить слои в любом направлении, наклонить камеру, сделать «прозрения» через перспективу.
- Cross-section viewport — настоящая 3D-cut плоскость (а не бок-о-бок).
- 3D-эффект «полёта через канвас» (как Miro perspective / Prezi).

**Минусы:**
- **Громадный импакт:** переписываются ВСЕ шейдеры (vec2 → vec3, mat4, perspective divide, нормали, освещение по желанию).
- **Текст на 3D-плоскости** — проблема: под углом читаемость падает; нужны billboards (но это ломает «канвас как плоскость»).
- **SpatialIndex** — из R-tree в 2D-AABB нужно превращать в **3D-AABB R-tree** (rstar это умеет, но все вызовы переписываются).
- **Hit-test** — из 2D-точка-в-AABB становится 3D raycast (намного дороже).
- **Drag/selection** — 3D-проекция на 2D-экран требует unproject; snapping в 3D сложнее.
- **Обратная совместимость с Obsidian/JSON Canvas 1.0** — формат не имеет z; расширения `canvasdesk.z` и `canvasdesk.layers` будут храниться как неизвестные поля, но **Obsidian покажет канвас плоским** (потеря 3D-информации при открытии в чужом редакторе).
- **Web/wasm** — perspective + depth + сложные шейдеры — приемлемо на WebGL2, но дороже, чем 2D.
- **Огромная трудоёмкость:** по оценке ≥ 3-5× времени опции B.

**Реальный use case:** «я хочу полноценный 3D-редактор графа». Но для CanvasDesk, который позиционируется как «расчётный канвас с Numi-формулами», 3D-ноды плохо читаются — карточки с текстом трудно прочитать под наклоном. Это избыточно для основной задачи.

---

## 3. Анализ импакта по крейтам

| Крейт | Опция A | Опция B | Опция C |
|---|---|---|---|
| **canvas-core/model.rs** | Node + `layer: Option<String>`; Canvas + `canvasdesk.layers` | То же что A | Node + `z: f32` + `depth: f32`; Edge + 3D-side enum |
| **canvas-core/spatial.rs** | Per-layer SpatialIndex ИЛИ один индекс с layer-фильтром | То же что A | 3D-AABB R-tree (`rstar::AABB<[f32;3]>`); все вызовы переписываются |
| **canvas-core/edgegeom.rs** | Cross-layer edge — новая функция (ломаная с z-shift) | То же что A + 3D-проекция ломаной через матрицу | Безье в 3D; все `port_point`, `edge_at` переписываются под 3D |
| **canvas-core/lineage.rs** | Пер-слой `LineageNodeId::Layer(layer_id)` — lineage может трассировать внутри слоя | То же что A | 3D-дерево lineage — расширение, но не критично |
| **canvas-core/whatif.rs** | Scenario → Layer (один сценарий = один слой) — это даёт «stack of whatifs» | То же что A | Полная 3D-дельта, не очень осмысленно |
| **canvas-core/bundles.rs** | Per-layer EdgeBundleIndex | То же что A | 3D-bundles (если слои = z-плоскости) |
| **canvas-render/camera.rs** | Без изменений (2D ortho) | `Camera { position: [f32;3], target, up, ortho_height }` — орто-проекция с наклоном | Полная perspective camera (`position, target, up, fov, aspect, near, far`) |
| **canvas-render/cards.wgsl** | Без изменений | `vec3` позиция + `mat4 view_proj` uniform + depth output | То же + perspective divide + normals (если свет) |
| **canvas-render/grid/guides/sectors/thumbs/minimap.wgsl** | Без изменений | Аналогично cards.wgsl | Аналогично + 3D-сетка |
| **canvas-render/stage.rs (MainStage)** | Без изменений | Stage может быть «слой» с z-offset; StageTransform становится матрицей | Stage = 3D-cut plane |
| **canvas-render/zorder.rs** | Layered zorder: пер-слой сегментация | То же что A + z-поиск по матрице | Глубинная сортировка через depth buffer (zorder.rs упрощается) |
| **canvas-scene/scene.rs** | `SceneState.canvas: LayeredCanvas` (Vec<Canvas> или Canvas + layer metadata) | То же что A | То же что A |
| **canvas-scene/measure.rs** | Per-layer fit_node_size | То же что A | 3D-bbox fit |
| **canvas-ui/hit.rs** | Layer-aware 2D hit | Layer-aware 2D hit + 3D raycast в 3D-режиме | Полный 3D raycast (canvas-ui переписывается) |
| **canvas-ui/layer.rs** | Без изменений (это UI overlay layers, не canvas layers) | Без изменений | Без изменений |
| **canvas-app/input.rs** | Layer-aware selection/drag; layer panel UI | + z-shift жест (Shift+drag) для перетаскивания между слоями; cross-section viewport toggling | 3D unproject для всех жестов |
| **canvas-app/app.rs** | `App.layers: Vec<LayerState>` + `active_layer: usize` | + `cross_section_mode: bool` | + `camera_mode: Ortho\|Perspective\|CrossSection` |
| **.canvas формат** | `canvasdesk.layers` + per-node `canvasdesk.layer` — Obsidian-совместимо (как unknown) | То же что A | + `canvasdesk.z`, `canvasdesk.depth` — Obsidian-плоско |
| **MCP** | `layers_list`, `layer_set_visible`, `node_move_to_layer`; `edges_list` возвращает layer | То же что A | `node_set_z`; `camera_set_perspective`; cross-section tools |
| **Web/wasm (canvas-web)** | Совместимо (без изменений GPU) | Совместимо (depth + mat4 — дёшево на WebGL2) | Дорого, но возможно; нужен WebGL2 + больше памяти |

---

## 4. Trade-offs

### 4.1. Производительность

- **A (multi-layer 2D):** 0% дополнительного GPU-cost — те же 2D-pipelines. Per-layer R-tree — O(N) памяти + переключение. Для 1000 нод × 5 слоёв — приемлемо.
- **B (pseudo-3D):** + depth buffer (~4 байта/пиксель для 1080p = 8 МБ), + 1 mat4 uniform per frame. Для сложных сцен (10000 нод × 5 слоёв) — alpha-blending + depth test вместе могут стать узким местом; нужен profiling.
- **C (full 3D):** perspective + depth + сложные шейдеры — для web/wasm может быть 2-3× дороже; для натива приемлемо, но не нужно для расчётного канваса.

### 4.2. Сложность реализации (оценка)

- **A:** ~2-3 недели (модель + .canvas extension + UI panel + per-layer logic в scene/app + миграция существующих .canvas). Низкий риск — почти всё существующее сохраняется.
- **B:** ~5-8 недель (всё из A + переписать 6 шейдеров под mat4 + 3D-Camera + depth-buffer pipelines + 2 режима рендера + cross-section viewport + z-shift жесты). Средний риск — точные тесты рендера.
- **C:** ~12-20 недель (переписать почти весь canvas-render, canvas-ui hit-test, 3D-edge geometry, 3D-format, миграция, перспективное редактирование). Высокий риск — много деталей.

### 4.3. Обратная совместимость

- **A:** полная — Obsidian читает `.canvas` плоско (все ноды в одном слое по умолчанию = `"default"`).
- **B:** полная — то же что A; 3D-режим — это UI-presentation, не формат.
- **C:** частичная — ноды с `z` и `depth` Obsidian покажет плоскими (z проигнорируется); cross-layer edges — 2D-плоские. Но 3D-bbox/3D-side расширения потеряются при открытии в Obsidian.

### 4.4. Миграция существующих `.canvas`

- **A, B:** авто-обёртка в 1 слой `"default"` — ничего не меняется визуально.
- **C:** потеря 3D-info при открытии в старых клиентах; если использовать только 2D-subset — авто-обёртка, как в A.

### 4.5. Web/wasm constraints

- **A:** 0 изменений.
- **B:** depth buffer + mat4 uniform — поддерживается WebGL2 (через wgpu), дёшево.
- **C:** perspective + depth — поддерживается, но дороже; bilboarding текста на web может тормозить при больших сценах.

### 4.6. Тестирование

- **A:** unit-тесты на per-layer функции (SpatialIndex, EdgeBundleIndex, lineage) — легко. Существующие 350+ тестов не трогаются.
- **B:** + headless render smoke с 3D-матрицей; нужен golden-image тест на cross-section viewport.
- **C:** переписываются почти все render smoke-тесты; нужны 3D-raycast тесты.

---

## 5. Рекомендация

### Что реально нужно заказчику?

Из формулировки «снимать пересечения графов на трёхмерном или условно трёхмерном пространстве» — это:
- Несколько графов (или один граф в нескольких состояниях) нужно видеть **одновременно**.
- Их нужно **пространственно разнести** (не просто таблично сравнить, как `ScenarioComparison`).
- Нужно уметь брать **cross-section** — то есть смотреть «сбоку», видеть все слои разом и связи между ними.

### Что не нужно?

- Полноценный 3D-perspective с редактируемыми 3D-объектами — для расчётного канваса с текстом на карточках это плохо читается.
- Свободная 3D-навигация камерой — избыточна; cross-section достаточно.

### Рекомендованный путь: B = Многослойная модель (A как data) + Псевдо-3D представление (B как view)

**Архитектура:**

1. **Data layer (как A):**
   - `Node.canvasdesk.layer: Option<String>` (по умолчанию `"default"`).
   - `Canvas.canvasdesk.layers: Vec<LayerMetadata>` (id, name, color, visible, opacity, z_order).
   - Cross-layer edge — обычное ребро, `from_layer`/`to_layer` выводятся из нод.

2. **View layer (как B):**
   - Добавить **второй режим рендера**: «Layered 3D» — ortho projection с наклоном (без perspective).
   - Слои получают `z = layer_index * LAYER_SPACING` (например, 200 world-px).
   - Шейдеры апгрейдятся до `vec3 + mat4` (depth buffer включается только в этом режиме).
   - Cross-section viewport — ortho сбоку (camera target = origin, up = z-axis); cross-layer edges видны как вертикальные связи.

3. **UI:**
   - Панель слоёв (как Figma/Photoshop) — переключение видимости, opacity, активного слоя.
   - Тоггл «Cross-section view» — переключение камеры в side-projection.
   - Жест Shift+drag ноды = «переместить в соседний слой» (z-shift на ±LAYER_SPACING).

4. **MCP:**
   - `layers_list` — все слои канваса.
   - `layer_set_visible` — тоггл видимости (для агента: «покажи только инфраструктурный слой»).
   - `node_move_to_layer {node_id, layer_id}` — операция перемещения ноды.
   - `lineage` расширяется опциональным `layer_id` — трассировка в пределах одного слоя.
   - `whatif_scenario_to_layer {scenario_id}` — материализовать сценарий как отдельный слой для визуального сравнения.

### Альтернативная интерпретация (если «пересечение графов» = «поиск common nodes/edges между двумя моделями»)

Если заказчик хочет сравнивать **разные `.canvas`-файлы** (например, импорт из Structurizr + своя модель) — то multi-layer не обязателен. Можно:
- Импорт foreign-модели в текущий канвас как **read-only слой**.
- Cross-canvas overlay: наложение двух слоёв с автоматическим matching по id нод (intersection detection).
- Highlight: общие ноды/рёбра — зелёным, уникальные — серым/янтарным.

Это надстройка над опцией A+B, реализуется как `canvasdesk.foreign_layer {source_ref, match_by: "id"}`.

### Почему НЕ рекомендована C (полный 3D)

1. **Читаемость текста** на наклонённых 3D-карточках падает; для Numi-формул и табличных рёбер — критично.
2. **Огромная трудоёмкость** (12-20 недель vs 5-8 для B) — несоразмерно пользе.
3. **Web/wasm constraints** — perspective + bilboarding толстеет; текущий wasm-port ADR-0011 уже имеет гейты.
4. **Obsidian-совместимость теряется** для z/depth полей.
5. **SpatialIndex / hit-test / drag** — всё переписывается под 3D-raycast — высокая регрессионная поверхность.

### Этапы (если заказчик подтвердит рекомендованный путь B)

1. **Phase 1 — Data layer (опция A):** расширение `.canvas` + `Canvas` модели + per-layer SpatialIndex/EdgeBundleIndex/Lineage + MCP `layers_*`. Без UI. ~2-3 недели.
2. **Phase 2 — Layer panel UI:** visibility/opacity/active переключатели; LayeredCanvas в SceneState; миграция существующих `.canvas` (авто-обёртка в `default` слой). ~1-2 недели.
3. **Phase 3 — Pseudo-3D рендер:** mat4 uniform во всех 6 шейдерах; depth buffer в pipeline; Camera extension (ortho-with-tilt). Старый 2D-режим сохраняется как default. ~2-3 недели.
4. **Phase 4 — Cross-section viewport:** second camera mode (side ortho); cross-layer edge rendering (вертикальные связи между z-плоскостями). ~1-2 недели.
5. **Phase 5 — Z-shift gestures:** Shift+drag ноды = переход в соседний слой; cross-section manipulation; MCP `node_move_to_layer`. ~1 неделя.
6. **Phase 6 — WhatIf → Layer bridge:** `whatif_scenario_to_layer` MCP — материализация сценария как слоя для визуального сравнения. ~1 неделя.

**Итого: ~8-12 недель** на полный layered+psevdo-3D kanvas с cross-section.

### Риски

- **Сложность alpha-blending + depth** в WebGL2 при больших сценах — нужно профилирование на Phase 3.
- **z-fighting** между карточками слоёв при близких z — обязателен polygon offset / кламп z-spacing ≥ 50 px.
- **Текст на наклонённой плоскости** — для углов наклона > 30° нужна перпектива (Option C), либо билборд. Кламп угла наклона в B — строго ≤ 30°.
- **Cross-layer edges** — геометрически это 3D-Безье (или ломаная с z-shift); hit-test в 3D — новая функция. Можно упростить: cross-layer edges — только в cross-section viewport (в обычном виде — 2D-проекция на активный слой).

### Альтернатива «минимально» (если 8-12 недель дорого)

Если заказчик хочет быстрый MVP — ограничиться Phase 1-2 (опция A только, без 3D-view): многослойный 2D-канвас с переключением. Это даёт слоистую декомпозицию, но **без** «пересечения в 3D». Стоимость ~3-5 недель. Но это не закрывает исходную задачу — для пересечений нужен B.

---

## 6. Сводка

- CanvasDesk сегодня полностью 2D — ни глубины, ни слоёв.
- Опция A (multi-layer 2D) — дешёвая, но не даёт «3D-пересечения».
- Опция B (pseudo-3D: layered data + ortho-with-tilt render + cross-section viewport) — **рекомендована**: даёт визуальное 3D-ощущение при умеренной сложности.
- Опция C (full 3D) — избыточна для расчётного канваса; высокая трудоёмкость и регрессионная поверхность.
- Существующие `Lineage`, `WhatIf`, `EdgeBundleIndex` — готовая семантическая база для срезов; layered view сделает их пространственными.

**Следующие шаги:** подтвердить рекомендацию B → написать PRD-0010 (layered+psevdo-3D) с фазами и приёмкой.
