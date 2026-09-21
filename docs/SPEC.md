# CanvasDesk — спецификация проекта

Рабочее название: **CanvasDesk**. **Визуальная система математического
моделирования** (ADR-0007): бесконечный зумируемый канвас, на котором
исполняемые математические модели строятся из расчётных нод (Numi-листы,
шаблоны), значения проливаются по value-связям, доменная математика
встроена в ядро, а ИИ-агент собирает и проверяет модели через MCP. Носитель
модели — файловый канвас поверх реальной файловой системы и (в режиме M4)
вместо стандартного рабочего стола.

> Этот документ описывает инфраструктурную спецификацию (носитель).
> Расчётное ядро (Numi, поток значений, шаблоны, доменные единицы) —
> волна FR-013…FR-029 (индекс — `docs/change-requests/index-cr-fr.md`),
> решения — `docs/adr/` (начать с ADR-0007, затем ADR-0002…ADR-0006).

---

## 1. Объём

**В объёме:**
- M1 — ядро канваса: камера (пан/зум), файловые карточки, системные тамбнейлы, сохранение в JSON Canvas
- M2 — текстовые заметки, связи (edges) между нодами, drag-drop из Explorer, вотчер файловой системы
- M3 — живые превью (изображения, PDF, текст/код, Office через preview handlers), миникарта, поиск
- M4 — режим встройки в рабочий стол (WorkerW): канвас за иконками, скрытие системных иконок, перехват контекстного меню
- M5 — движок расширений: виджеты как JS/HTML-микрофронтенды на канвасе (WebView2, манифест, bridge, sandbox)
- Волна моделирования (FR-013…FR-029) — Numi-движок в заметках, поток значений
  по value-рёбрам (DAG), доменные единицы и queueing-функции, библиотека
  шаблонов (45), палитра/wheel-UI, построчные выходы, юнит-экономика,
  онбординг и встроенная документация. Спецификация по FR — в самих
  документах; архитектурные решения — `docs/adr/` (ADR-0002…ADR-0007);
  состав и приёмка концепции композиции — CR-013
- M7 — кроссплатформенность: Windows 10/11, Linux (X11/Wayland), macOS — сборка,
  гейты CI и платформенные реализации (тамбнейлы, drag-drop, MCP) по плану
  `docs/plans/M7-crossplatform.md`

**Вне объёма (осознанно):**
- Замена shell (таскбар, трей остаются Explorer)
- Desktop-режим (встройка в рабочий стол) вне Windows — юникс-эквиваленты
  (layer-shell и т.п.) отложены до после v1.2; оконный режим — на всех ОС
- Живые виджеты на Linux/macOS до завершения M5 (T21/T22): вне Windows
  виджет-нода рендерится снапшотом/плейсхолдером
- Облачная синхронизация, мультипользовательский режим
- Редактирование содержимого документов внутри канваса
- Публичный каталог/маркет виджетов, облачные виджеты, удалённая загрузка JS — виджеты только локальные пакеты, устанавливаемые пользователем явно

## 2. Пользовательские сценарии

1. Пользователь перетаскивает папку проекта на канвас → файлы раскладываются сеткой, видны тамбнейлы
2. Пользователь группирует документы проекта в пространстве, соединяет связями, добавляет заметки-контексты
3. Приближает карточку PDF → карточка превращается в читаемое превью первой страницы
4. В режиме M4: загрузка ПК → вместо стандартного десктопа открыт последний канвас; двойной клик по файлу открывает его в ассоциированном приложении
5. Файл переименован в Explorer → карточка обновилась; файл удалён → карточка помечена «broken link», не исчезает молча
6. Пользователь ставит на канвас виджет (часы, календарь, собственный дашборд из Vite-билда): виджет живёт как нода — двигается, связывается рёбрами, при приближении становится интерактивным, при отдалении превращается в статичный снапшот

## 3. Технологический стек

| Слой | Решение | Почему |
|---|---|---|
| Язык | Rust stable (1.80+), edition 2021 | — |
| Окно/ввод | `winit` 0.30 | Кроссплатформенная основа, тачпад-жесты |
| GPU-рендер | `wgpu` 22+ | DX12/Vulkan бэкенды, батчинг, будущая портируемость |
| Текст | `glyphon` (поверх `cosmic-text`) | Нативная интеграция с wgpu, шейпинг, эмодзи |
| Пространственный индекс | `rstar` (R-tree) | Hit-testing, viewport culling на 5–10 тыс. нод |
| Формат канваса | JSON Canvas spec 1.0 (`serde_json`) | Совместимость с Obsidian, human-readable |
| Метаданные/кэш | `rusqlite` (bundled) | Тамбнейлы-кэш, индекс поиска, сессии |
| Файловый вотчер | `notify` 6+ | Три бэкенда одним API: ReadDirectoryChangesW (Win), inotify (Linux), FSEvents (macOS); различия нормализуются в canvas-shell |
| Win32/COM | `windows-rs` (features: Win32_UI_Shell, Win32_Graphics_Dwm, System_Com) | Тамбнейлы, preview handlers, WorkerW — Windows-слой |
| Тамбнейлы | `IShellItemImageFactory::GetImage` | Системный кэш, совпадает с Explorer |
| PDF | `pdfium-render` (бинарь pdfium, BSD-лицензия) | Быстрый рендер страниц в битмап |
| Изображения | `image` | Декод в RGBA → GPU-текстура |
| Preview handlers | `IPreviewHandler` в out-of-process хосте | Изоляция падающих COM-компонентов |
| Виджеты (M5) | WebView2 Evergreen + `webview2-com` | Живые HTML/JS-ноды; снапшоты через `ICoreWebView2::CapturePreview` |
| Мост виджетов | `postMessage` JSON-RPC (`serde_json`) | Узкий типизированный host API с permissions, без eval |
| Сериализация конфига | `serde` + `toml` | — |
| Логирование | `tracing` + `tracing-subscriber` | Диагностика на машинах пользователей |
| WASM-таргеты (FR-036, ADR-0011) | `wasm32-unknown-unknown` (продуктовый, план M8) + `wasm32-wasip1` (служебный тестовый, wasmtime) | Ядро и MCP-слой (core/render/widgets/mcp/scene/headless, FR-037) обязаны собираться и исполняться под wasm: гейты `scripts/wasm_gate.sh` + `scripts/mcp_wasm_gate.sh`, CI `wasm-check` |
| Упаковка | `cargo-wix` → MSI, Authenticode-подпись | M4 требует доверия системы |

## 4. Структура workspace

```
canvasdesk/
├── Cargo.toml               # workspace
├── crates/
│   ├── canvas-core/         # модель данных, JSON Canvas I/O, spatial index, Numi-движок (expr/),
│   │                        #   DAG-поток значений (flow.rs), валидация модели (validate.rs, FR-032),
│   │                        #   реестр шаблонов (templates.rs) — без ОС/GPU
│   ├── canvas-render/       # wgpu-рендер: камера, батчинг, текст, текстуры, LOD
│   ├── canvas-shell/        # Windows-only: тамбнейлы, preview handlers, drag-drop, WorkerW (cfg(windows))
│   ├── canvas-preview-host/ # отдельный exe — песочница для IPreviewHandler
│   ├── canvas-widgets/      # M5: WebView2-хост, bridge, манифесты, снапшоты (cfg(windows))
│   ├── canvas-mcp/          # MCP-посредник: stdio JSON-RPC ↔ named pipe, 36 инструментов канваса (FR-032: edges_list/edge_get/graph_validate; FR-033: graph_apply; FR-016: analyze_bottlenecks; FR-017: whatif_*)
│   ├── canvas-scene/        # модель сцены (SceneState) + mcp_dispatch — платформенно-нейтральный, wasm (FR-037/ADR-0012)
│   ├── canvas-mcp-headless/ # headless MCP-сервер для wasmtime/wasip1 (FR-037) — верификация сессий без Windows
│   └── canvas-app/          # приложение: event loop, команды, UI-состояние, mcp_dispatch, main()
├── assets/                  # шрифты, иконки нод, виджеты (widgets/), шаблоны (templates/)
├── docs/                    # SPEC.md, TASKS.md, RECIPES.md, adr/, change-requests/
└── AGENTS.md                # инструкции для агента
```

Правило границ: `canvas-core` не импортирует ничего из `canvas-shell` и `canvas-render`. Вся платформенная логика — за трейтами (`ThumbnailProvider`, `PreviewProvider`, `ShellIntegration`), чтобы core тестировался на любой ОС.

## 5. Модель данных

### 5.1. Файл канваса — `*.canvas` (JSON Canvas 1.0)

```json
{
  "nodes": [
    { "id": "n1", "type": "file", "file": "C:/Projects/alpha/spec.pdf",
      "x": 120, "y": 80, "width": 340, "height": 440 },
    { "id": "n2", "type": "text", "text": "Согласовать до пятницы",
      "x": 520, "y": 80, "width": 260, "height": 120, "color": "3" },
    { "id": "n3", "type": "group", "label": "Альфа-проект",
      "x": 60, "y": 20, "width": 800, "height": 600 },
    { "id": "n4", "type": "widget", "x": 920, "y": 80, "width": 320, "height": 200,
      "canvasdesk": { "widgetId": "com.example.clock", "props": { "timezone": "Europe/Moscow" } } }
  ],
  "edges": [
    { "id": "e1", "fromNode": "n2", "fromSide": "right",
      "toNode": "n1", "toSide": "top", "label": "блокирует" }
  ]
}
```

Расширения поверх spec (хранить в нодах, игнорируемые другими приложениями — в поле `canvasdesk` или по конвенции spec):
- `file` допускает абсолютные пути Windows (spec описывает пути внутри vault; отклонение документируем)
- `previewState`: `thumbnail | live | none` — последний уровень детализации ноды
- `brokenLink: true` — файл недоступен, карточка сохраняется с серой рамкой
- `type: "widget"` + объект `canvasdesk: { widgetId, props }` — виджет-нода (M5, §7.6); приложения, не знающие тип, пропускают такую ноду, файл остаётся валидным
- `canvasdesk: { expr }` на text-ноде — Numi-формула calc-ноды (FR-013); результат вычисляется приложением и в файл не пишется (инвариант 4)
- `canvasdesk: { flow: { kind: "value" | "control" } }` на связи — тип потока (FR-014). `value` — ребро переносит значение источника в `$in`/`$1..$N` формулы downstream; отсутствие поля и `control` — визуальная связь (дефолт, обратная совместимость). Граф value-рёбер — DAG: циклы блокируются при создании (диалог с фолбэком на control в UI, isError в MCP). Результаты пересчёта (live, propagator `canvas-core/src/flow.rs`) в файл не пишутся
- `fromLine: <uint>` на связи — построчный исток (FR-025): value-ребро переносит значение формульной строки `fromLine` Numi-листа источника (а не значение ноды целиком). Отсутствие поля — значение ноды (текущее поведение; старые файлы без изменений); битые значения (`-1`, дробные) читаются как отсутствие. Создаётся drag от построчного порта (фича-флаг `line_ports` в настройках, дефолт выкл); перепривязка from-конца сбрасывает поле
- `fromOutput: "<имя>"` на связи — именованный исток (FR-029): value-ребро переносит значение именованного выхода источника (шаблонная нода — секция `outputs` манифеста/снимка; текстовая — переменная Numi-листа, адресация живёт при сдвиге строк). Взаимно исключается с `fromLine` (проверяется MCP; приоритет модели — `fromLine`)
- `toParam: "<имя>"` на связи — проливание в параметр (FR-029): value-ребро подставляет значение в `$<имя>` шаблонной ноды-приёмника, ПЕРЕКРЫВАЯ локальное значение параметра («проливание сильнее дефолта»), без правки формулы. Рёбра с `toParam` не занимают позиционные слоты `$1..$N`; несколько рёбер в один параметр — побеждает последнее по `canvas.edges` (предупреждение в `flow_recalc`; строгая диагностика — FR-032 `graph_validate`)
- `canvasdesk: { pin_ports: ["from", "to"] }` на связи — закреплённые концы подключения (CR-008). Конец без пина подключается к порту кратчайшего пути (`best_sides`, пересчёт при перетаскивании нод и автораскладке — в файл не пишется); закреплённый — следует сохранённым `fromSide`/`toSide`. Массив может содержать один или оба конца; пустой/отсутствующий — оба конца авто. Снятие последнего пина удаляет поле (чистый round-trip)
- `canvasdesk: { template }` на text-ноде — снимок ссылки на шаблон (FR-018): `{ id, version, expr, params: { имя: { num, unit? } }, icon, color, outputs? }` — `outputs` (FR-029, схема манифеста 1.1): `[{ name, unit?, line | expr }]`, именованные выходы для адресации рёбрами `fromOutput`; ключ пишется только при непустой секции (round-trip старых файлов). `expr` — Numi-формула с `$param`-ссылками (результат — в футере карточки и в потоке FR-014, в файл не пишется); `icon`/`color` — снапшоты роли/категории (рендер шапки без реестра). Текст ноды — Numi-лист присваиваний параметров; правка текста синхронизирует `params`. Поле переживает round-trip (снимок, не ссылка на реестр)
- MCP-чтение и валидация графа (FR-032) — runtime, в файл не пишется: `edges_list`/`edge_get` отдают каноническую схему ребра `{id, from, to, kind, fromLine?, fromOutput?, toParam?, fromSide, toSide}` (адресация портов FR-029); `graph_validate` — отчёт `{valid, issues: [{severity, code, node_id, edge_id, message}]}` из чистой функции `canvas-core/src/validate.rs`. Коды — стабильный контракт для рецепта агента: `E-CYCLE` (цикл value-рёбер), `E-OVERLOAD` (ρ ≥ 1), `W-AMBIGUOUS-SRC` (многолинейный исток без адресации строки/выхода), `W-UNUSED-SLOT` (позиционный вход `$N` не читается формулой), `E-UNIT`/`E-PORT-UNKNOWN`/`E-DOUBLE-INPUT` (контракт портов FR-029 — реализованы при влитии CP1)
- `canvasdesk: { whatif: { scenarios: [{ name, overrides: [{ node, line, expr }] }] } }` в `Canvas.extra` (FR-017, CP6) — персистентные what-if сценарии (лимит 3): построчные подмены `(id ноды, индекс строки текста) → новый исходник`. Подмены активного сценария — runtime-only: propagator считает по виртуальному исходнику (`canvas-core/src/flow.rs`, `whatif_virtual_text`), `.canvas` без Apply не мутируется; `whatif_apply` пишет подмены в строки/params и удаляет сценарий (один undo-шаг). Протухшие подмены (нода/строка удалены, строка стала прозой) тихо пропускаются пересчётом и помечаются в списке overrides; пустой список сценариев удаляет поле (round-trip старых файлов чистый)

### 5.2. SQLite (`~/.canvasdesk/cache.db`)

- `thumb_cache(file_path, mtime, size_class, blob_hash)` — инвалидируется по mtime
- `search_index(file_path, display_name, extracted_text_fts5)` — FTS5 для Ctrl+F
- `sessions(canvas_path, camera_x, camera_y, zoom, opened_at)` — восстановление вида

### 5.3. Правило источника истины

Раскладка (координаты, размеры, связи) — только в `.canvas`-файле. SQLite — пересоздаваемый кэш, его удаление ничего не ломает.

## 6. Архитектура рендера

### 6.1. Кадр

```
input → camera update → world-space culling (rstar query по viewport)
→ LOD assignment по zoom → batch build (quads, текст, текстуры)
→ один render pass → minimap pass (offscreen → corner quad)
```

### 6.2. Уровни детализации (LOD)

| Zoom | Содержимое файловой карточки |
|---|---|
| < 0.25 | Цветной прямоугольник + иконка типа файла |
| 0.25–0.6 | + системный тамбнейл + имя файла |
| 0.6–1.5 | + превью содержимого (картинка, первая страница PDF, первые N строк текста) |
| > 1.5 | Живое превью с прокруткой (только для нод под курсором/в фокусе, максимум 3 одновременно) |

Живые превью — дорогие (текстуры 1024²+), их количество жёстко ограничено; при отдалении текстура освобождается, остаётся тамбнейл.

Виджеты (M5) — отдельная LOD-стратегия: zoom < 0.25 или вне viewport → snapshot-текстура; видим и zoom ≥ 0.25 → живой WebView2, лимит живых инстансов по §7.6.

**Структурная агрегация связей (FR-042)** — LOD, независимый от зума: связи одной упорядоченной пары нод (N ≥ 2) рисуются одной агрегированной линией с непрерывной толщиной по весу (`d = clamp(1.8 + (N−1)·0.9, 1.8, 8.0)`) и бейджем кратности `×N`; одиночные связи — прежний вид. Детализация пучка — модальный режим «main stage» (§8). Отключается настройкой «Агрегация связей».

### 6.3. Производительность — целевые метрики

- 60 fps при панорамировании/зуме на сцене из 5 000 нод (GPU уровня GTX 1050 / Iris Xe)
- Холодный старт до первого кадра < 2 с
- Открытие канваса на 1 000 нод < 500 мс (тамбнейлы — асинхронно, карточки появляются сразу)
- Память < 500 МБ на 5 000 нод без живых превью

### 6.4. Текстуры

Атлас менеджер: тамбнейлы укладываются в атласы 2048² (LRU-вытеснение), превью — отдельные текстуры. Формат RGBA8, mipmaps не нужны (LOD дискретный).

### 6.5. DPI

Манифест приложения — **Per-Monitor V2** DPI awareness. Все координаты канваса в логических пикселях (world-space), рендер — в физических (`scale_factor` из winit); при переносе окна между мониторами с разным масштабом — пересоздание surface и пересчёт размеров текста. Не полагаться на системное DPI-виртуальное масштабирование (bitmap-stretch) — текст будет мыльным.

### 6.6. Палитра и design-токены (PRD-0006, FR-046)

Единая точка правды визуальных характеристик — трёхслойная система design-токенов:

1. **Примитивы** — `design/tokens/{colors,dimensions,motion}.json` (структура в духе W3C Design Tokens, `$type`/`$value`/`$desc` с координатами источника). Зеркало — платформенно-нейтральный `canvas_core::tokens` (данные, wasm-совместимо); расхождение JSON↔Rust ловится паритет-тестами (FR-046 I-5).
2. **Семантика** — `canvas_render::theme::ThemeColors`: слоты палитры (фон/сетка/карточки/меню/текст/GFM/группы/направляющие FR-038 + v2: `accent`, `selection_fill`, `highlight`, `whatif_fill`, `whatif_badge`, `error`, `hud`); обе палитры (`dark()`/`light()`) собираются из примитивов. Контраст гарантируется машиной `contrast.rs` + тестами (CR-007); известные исключения задокументированы и имеют регрессионные границы.
3. **Потребители** — акцентное семейство (выделение/рёбра/draft/хром виджетов/drop-ghost/select-рамка/группы) — алиасы примитива `ACCENT` (единый источник, G4); минимапа/wheel/диалоги/тосты — значения из токенов; метрики карточек (`HEADER_HEIGHT`, `CORNER_RADIUS`) и типографика — алиасы `dimensions.json` (стык PRD-0004 F-1).

Правило потока: цвет приходит в шейдер только из инстанса/юниформа, заполненного из `ThemeColors`/токенов; литерал в билдере = дефект — ловится `scripts/token_lint.sh` (hex вне токенов/Win32-доменов/тестов/дата-контрактов = ошибка). Данные-домены с собственными hex-контрактами (манифесты шаблонов FR-018 — `templates::DEFAULT_TEMPLATE_COLOR`) вне рендер-палитры. Темы-пресеты как данные и пользовательские палитры — дорожная карта PRD-0006 §13 (D4/V2).

## 7. Shell-интеграция (crate `canvas-shell`)

### 7.1. Тамбнейлы

`IShellItemImageFactory::GetImage(SIIGBF_BIGGERSIZEOK | SIIGBF_THUMBNAILONLY)` → HBITMAP → RGBA → атлас. Запросы — в пуле потоков (4), результаты — через канал в рендер-поток. Приоритет очереди: видимые ноды → ближайшие к viewport.

### 7.2. Preview handlers (M3)

Отдельный процесс `canvas-preview-host.exe`:
1. Родитель передаёт путь файла и размер по named pipe
2. Хост резолвит `IPreviewHandler` по CLSID из реестра, рендерит в offscreen-окно, копирует битмап, возвращает пиксели
3. Таймаут 3 с → kill процесса, фолбэк на тамбнейл
4. Крэш хоста не влияет на канвас; хост перезапускается на следующий запрос

### 7.3. Drag-drop из Explorer (M2)

`IDropTarget` на окне: принимаем `CF_HDROP` и `FileGroupDescriptor`. Drop папки → рекурсивный обход (глубина 1), авто-раскладка сеткой с шагом по размеру карточки, начиная от точки дропа.

### 7.4. Режим десктопа (M4)

**Иерархия окон десктопа различается по версиям — это ключевая развилка реализации:**

- **Win10 / Win11 ≤ 23H2 (классическая схема):** шлём `Progman` сообщение `0x052C` → Explorer порождает top-level `WorkerW` позади `SHELLDLL_DefView`; наше окно делаем дочерним к этому `WorkerW` через `SetParent`.
- **Win11 24H2 / 25H2 (build ≥ 26100, включая целевую 26200):** Explorer изменил рендеринг фона ради HDR-обоев. `Progman` создаётся с `WS_EX_NOREDIRECTIONBITMAP`, `SHELLDLL_DefView` — `WS_EX_LAYERED` дочернее окно `Progman`, а `WorkerW` — дочернее окно `Progman` по Z-order **ниже** DefView. Сообщение `0x052C` больше не отделяет DefView в собственный top-level `WorkerW`. Рабочая стратегия: наше окно — **`WS_EX_LAYERED` дочернее окно `Progman` с Z-order между DefView (сверху) и WorkerW (снизу)**, выравнивание через серию `SetWindowPos` (`SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOMOVE`). DefView почти полностью прозрачен и рисует поверх нас только иконки и текст.

**Порядок встройки:**

1. Определить стратегию по фактической иерархии окон (детект, не гадание по номеру сборки)
2. Классическая схема: `SendMessageTimeout(0x052C)` → найти top-level `WorkerW` (перебором `EnumWindows`, у которого `SHELLDLL_DefView` — сосед) → `SetParent(наше_окно, workerw)`
3. Схема 24H2+: создать наше окно дочерним к `Progman` со стилем `WS_EX_LAYERED`, выставить Z-order: `DefView` → наше окно → `WorkerW`
4. Окно: на весь виртуальный экран (multi-monitor через `EnumDisplayMonitors`), без рамки, `WS_EX_NOACTIVATE` до первого клика
5. Скрытие системных иконок: `SHELLDLL_DefView` + `WM_COMMAND 0x7402` (toggle) — сохраняем исходное состояние, восстанавливаем при выходе
6. Контекстное меню десктопа: перехват `WM_RBUTTONUP` на нашем окне → своё меню (Открыть канвас / Новый файл / Показать иконки / Выход)
7. Двойной клик по файловой ноде → `ShellExecuteEx` с `SEE_MASK_INVOKEIDLIST` (поведение «как в Explorer»)

**Таблица стратегий (версионный гейт):**

| Версия | Build | Стратегия встройки | Статус |
|---|---|---|---|
| Windows 11 25H2 | 26200 (dev-машина: 26200.9168) | Схема 24H2+ (layered child of Progman) | **Основная цель разработки** |
| Windows 11 24H2 | 26100 | Схема 24H2+ | Обязательная проверка |
| Windows 11 23H2 | 22631 | Классическая (top-level WorkerW) | Проверка на VM |
| Windows 10 22H2 | 19045 | Классическая | Проверка на VM |
| Неизвестная / новее | — | Рантайм-детект иерархии (есть ли top-level WorkerW с DefView-соседом) → выбор схемы; при неудаче → оконный режим с предупреждением | Фолбэк |

Каждая стратегия — отдельный модуль за общим трейтом `DesktopEmbedder`. Референс-реализации для изучения перед кодингом: Lively Wallpaper (C#, поддерживает 24H2+), Seelen UI (Rust) — ссылки в §11. **Детальный разбор обоих проектов и готовые рецепты R1–R17 — в `docs/RECIPES.md`, обязателен к прочтению перед реализацией этого раздела и §7.4-связанных задач.**

### 7.5. Файловый вотчер (M2)

`notify` с `RecursiveMode::NonRecursive` на каждую директорию, из которой есть ноды. Debounce 300 мс. События:
- `Modify` → инвалидировать тамбнейл-кэш, обновить карточку
- `Rename` → если путь совпал с нодой — обновить `file` в модели (и в `.canvas`)
- `Remove` → `brokenLink: true`, карточка серая, связи сохраняются

### 7.6. Движок виджетов (M5)

**Концепция.** Виджет — самодостаточный микрофронтенд: папка с манифестом `widget.json` и статическими файлами (HTML/JS/CSS — билд любого фреймворка: React, Svelte, vanilla). Канвас — хост-оркестратор: размещает виджет как ноду, управляет жизненным циклом, изолирует. Пользователь может положить на канвас любой JS/HTML-объект — от часов до собственного микроприложения (дашборд воронки, панель контактов, мини-трекер).

**Манифест `widget.json`:**

```json
{
  "id": "com.example.clock",
  "name": "Clock",
  "version": "1.0.0",
  "entry": "index.html",
  "defaultSize": [320, 200],
  "permissions": ["canvas:read", "shell:open", "fs:read", "network"]
}
```

**Рендер.** WebView2 (Evergreen Runtime):

- На живой виджет — один WebView2 в дочернем HWND, позиционируемом точно над rect ноды; синхронизация с камерой — `SetWindowPos(SWP_ASYNCWINDOWPOS)` каждый кадр (дёшево), скругление углов — region на HWND
- **Airspace-ограничение:** HWND WebView2 рисуется поверх wgpu-канваса → оверлеи канваса (миникарта, панель поиска, контекстные меню) не должны перекрывать живые виджеты либо выводятся отдельными layered-окнами. Задокументированное архитектурное ограничение
- **LOD:** zoom < 0.25 или нода вне viewport → WebView2 скрывается и приостанавливается, вместо него snapshot-текстура (`CapturePreview` по событию изменения или раз в 5 с для анимированных); zoom ≥ 0.25 и видим → живой инстанс
- **Лимит живых инстансов** (по умолчанию 6, настраивается): LRU — давно не видимые переводятся в snapshot. Один user-data-folder на приложение → общие browser-процессы рантайма

**Мост (bridge).** Двусторонний JSON-RPC поверх `postMessage`/`WebMessageReceived`, типизированный (`serde`):

- host → widget: `init { nodeId, props, theme, zoom }`, `propsChanged`, `visibility { visible }`, `themeChanged`
- widget → host: `ready`, `resize { w, h }`, `setProps { ... }` (persist в `.canvas`), `openFile { path }` (perm `shell:open`), `readDir { path }` (perm `fs:read`, только allowlist-директории), `toast { text }`
- Каждый вызов проверяется против `permissions` манифеста; схемы сообщений валидируются десериализацией — невалидное сообщение = drop + warn, не паника

**Безопасность.**

- Виджеты — только локальные пакеты в `%APPDATA%/canvasdesk/widgets/<id>/`, установка = явное копирование папки пользователем (drag папки на канвас → предложение установить)
- Содержимое через `SetVirtualHostNameToFolderMapping` (виртуальный origin), навигация наружу пакета запрещена; CSP по умолчанию `default-src 'self'`
- permission `network` — opt-in: без неё все внешние `WebResourceRequested` блокируются
- Виджет по URL (remote JS) — **запрещён архитектурно**, только локальные bundle

**Данные.** `props` и геометрия — в ноде `.canvas` (§5.1), переживают экспорт в Obsidian как неизвестный тип. Объёмное состояние виджета — в SQLite `widget_state(node_id, key, value)`.

**Ввод.** Клик внутри виджета — фокус WebView2 (клавиатура/мышь уходят виджету); перемещение ноды — drag за рамку/заголовок, который рисует канвас поверх (полоса 24px); Esc — возврат фокуса канвасу. Рёбра к виджет-нодам работают как к обычным.

**Встроенные виджеты.** В `assets/widgets/` поставляются: часы/дата, календарь, заметки-стикеры. Они же — референсы для SDK (T22).

**Уточнения v1.1 (волна M5, детальный план — `docs/plans/M5-widgets.md`).**
Зафиксированные продуктовые и технические решения, закрывающие развилки этого раздела:

- **Зум контента — `ICoreWebView2Controller::ZoomFactor`** (кламп 0.25–5.0):
  CSS-viewport виджета равен мировому размеру области контента, контент
  масштабируется зумом канваса без reflow; при зуме > 5 контент клампится.
- **Хром ноды:** заголовок 28 world-px + инсет рамки 8 world-px — канвасные;
  WebView занимает внутреннюю область (уточнение «полосы 24px»).
- **Добавление на канвас:** ПКМ-меню канваса «Виджеты ▸» (установленные +
  встроенные); drag папки — путь установки (см. ниже).
- **Установка/обновление/удаление:** drag папки с валидным `widget.json` →
  in-canvas диалог Да/Нет (список permissions) → копирование пакета и нода в
  точке дропа; повторный drag новой версии → диалог обновления (файлы
  заменяются, ноды/props сохраняются); удаление пакета — через подменю
  «Виджеты ▸ <имя> ▸ Удалить пакет», ноды становятся «битыми».
- **Пакеты живут в `%USERPROFILE%\.canvasdesk\widgets\<id>\`** (единый корень
  данных приложения с `cache.db`; отклонение от «%APPDATA%/canvasdesk» выше —
  задокументировано). user-data-folder WebView2 — `~/.canvasdesk/webview2/`.
  Встроенные пакеты вшиты в бинарник и материализуются при старте (tombstone
  `widgets/.deleted/<id>` уважает ручное удаление до выхода новой версии).
- **`fs:read`-allowlist:** директории файловых нод текущего канваса + папка
  `.canvas`-файла (набор вотчера T10).
- **Airspace-политика:** при перекрытии live-виджета оверлеем канваса
  (миникарта, поиск, хоткеи, настройки, меню, диалоги, рамка выделения,
  протягивание ребра) виджет временно прячется, рисуется snapshot.
- **Тема:** `init`/`themeChanged` шлют `{ dark: bool, accent: "#hex" }` из темы
  приложения.
- **Bridge дополнен** методами `stateGet`/`stateSet` (доступ к
  `widget_state` SQLite, см. «Данные»); `canvas:read` зарезервировано.
- **MCP (v1.1):** инструменты `widget_list`, `widget_add`, `widget_set_props`.
- Снапшот: кламп 512×512, refresh 5 с только для видимых snapshot-виджетов
  при zoom ≥ 0.25 (за лимитом live); отдельные текстуры (не атлас), LRU-кэп 16.

## 8. Ввод

| Действие | Жест |
|---|---|
| Панорамирование | Средняя кнопка / Space+drag / двухпальцевый скролл тачпада |
| Зум | Ctrl+колесо (к курсору), pinch |
| Выделение | ЛКМ drag — рамка; Shift — добавить |
| Перемещение нод | drag ЛКМ |
| Связь | drag от порта ноды (появляются при наведении) |
| Контекстное меню ноды | ПКМ |
| Поиск | Ctrl+F |
| Обзор (fit to content) | Ctrl+0; миникарта — клик/драг viewport-прямоугольника |
| Двойной клик по файлу | Открыть в ассоциированном приложении |
| Настройки (FR-039) | Кнопка ⚙ / `Ctrl+,` — центрированная модалка над затемнением; левая навигация по 4 разделам, строки «лейбл + описание + контрол»: булевы — pill-тумблеры, многозначные — dropdown-кнопки (меню с клампом к окну, ширина контрола); клик по затемнению закрывает; размер адаптивный с потолками (FR-026-инварианты переносятся) |
| What-if режим (FR-017) | `Ctrl+Shift+I` / пилюля «What-if» / меню канваса; двойной клик по строке расчёта в режиме — подмена; Esc — выход |
| Меню помощи и документация | Кнопка «?» (кластер ⚙/тема): «Документация ▸» — 7 разделов во встроенном просмотрщике (правый док: колесо — прокрутка, внутренние ссылки — переход, × / Esc / клик вне — закрыть); «Пройти онбординг» (FR-031/FR-028) |
| Онбординг | Первый запуск — тур-карусель (8 шагов); «Пропустить»/Esc — отложить до следующего запуска (после 3 подряд — авто-показ молчит); повтор тура — «?» → «Пройти онбординг»; полный проход («Готово») выключает авто-показ навсегда (FR-028) |
| Язык интерфейса (FR-040) | Настройки → «Внешний вид» → dropdown «Язык» («русский»/«English», названия — в собственной локали); применяется на лету, сохраняется в конфиге (`language`, serde default `ru`); все UI-тексты — ключи таблицы `i18n` (RU/EN), fallback — RU |
| Ввод внутри виджета (M5) | Клик по виджету — фокус виджету; Esc — возврат фокуса канвасу |
| Перемещение виджета (M5) | drag за заголовок/рамку ноды |
| **Main stage (FR-042)** | Клик/ПКМ/двойной клик по агрегированной линии пучка — модальная детализация (rect ≤ 70% вьюпорта: обе ноды + все рёбра пучка веером, подписи `fromLine`/`fromOutput`/`toParam` + значения); клик внутри — выделение ребра живой связи; выход — `Esc` или клик по затемнённому фону; пан/зум/drag/правка внутри глушены; открытие любого оверлея закрывает stage; выключатель — настройка «Агрегация связей» (FR-039, раздел «Связи и порты») |

## 9. Риски и допущения

| Риск | Митигация |
|---|---|
| WorkerW ломается апдейтом Windows | Версионный гейт + фолбэк в оконный режим; канвас-файл не зависит от режима отображения |
| Крэш стороннего preview handler | Out-of-process хост с таймаутом и перезапуском |
| Антивирусный false positive (shell hooks) | Authenticode-подпись с M4, отправка в whitelisting до релиза |
| Производительность на больших сценах | LOD + culling + атласы; нагрузочный тест 5k нод — часть CI |
| Потеря данных канваса | Автосейв с debounce 2 с + `.bak` предыдущей версии; JSON human-readable |
| Конфликт хоткеев с Explorer в M4 | Окно не перехватывает клавиатуру без явного фокуса (клик по канвасу) |
| WebView2 Runtime отсутствует на машине | Evergreen bootstrapper в MSI (T19); без рантайма — виджеты недоступны с понятной ошибкой, канвас работает |
| Вредоносный виджет | Только локальные пакеты, permissions в манифесте, запрет remote JS и навигации, `network` opt-in (§7.6) |
| Деградация fps от множества WebView2 | Лимит живых инстансов + snapshot LOD (§7.6); нагрузочный тест 10 виджетов — часть приёмки M5 |
| Airspace: WebView2 поверх wgpu-канваса | Задокументированное ограничение (§7.6): оверлеи не перекрывают виджеты или выносятся в layered-окна |

## 10. Критерии готовности релизов

- **M1 done:** канвас открывает/сохраняет `.canvas`, 5 000 файловых карточек с тамбнейлами на 60 fps, файл открывается в Obsidian без ошибок
- **M2 done:** заметки редактируются инлайн, связи рисуются и сохраняются, drop папки из Explorer раскладывает файлы, переименование файла в Explorer обновляет карточку за < 1 с
- **M3 done:** PDF/изображения/текст показывают живое превью на zoom > 0.6, docx — через preview host, миникарта навигационна, поиск находит по имени и тексту (FTS5)
- **M4 done:** флаг `--desktop` встраивает канвас за иконками, иконки скрываются и восстанавливаются при выходе, пережит перезапуск Explorer, аварийное завершение не оставляет десктоп без иконок
- **M5 done:** виджет из локальной папки ставится на канвас, живой при zoom ≥ 0.25, snapshot на дальнем zoom, props переживают перезапуск, bridge-вызовы без permission блокируются, 10 виджетов на сцене не роняют fps ниже 60 за счёт лимита live-инстансов

## 11. Документация для агента

Обязательный чтение-лист перед кодингом соответствующего модуля. Агент обязан сверяться с этими источниками, а не с собственной памятью — API меняются, а недокументированные приёмы различаются по версиям ОС.

### 11.1. Целевая платформа

- Windows 11 release information (версии и номера сборок — для версионного гейта §7.4): https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information
- Windows 11 25H2 update history (изменения по KB на целевой сборке 26200.x): https://support.microsoft.com/en-us/servicing/os/windows-11/2025/07/windows-11-version-25h2-update-history

### 11.2. Форматы данных

- JSON Canvas spec 1.0 (источник истины по формату `.canvas`, §5.1): https://jsoncanvas.org/spec/1.0/
- SQLite FTS5 (поисковый индекс, §5.2): https://www.sqlite.org/fts5.html

### 11.3. Rust-экосистема (docs.rs — читать под зафиксированные в Cargo.toml версии)

- winit (окно, ввод, `ApplicationHandler` — event loop 0.30 отличается от 0.29): https://docs.rs/winit/latest/winit/
- wgpu (рендер): https://docs.rs/wgpu/latest/wgpu/ + учебник: https://sotrh.github.io/learn-wgpu/
- glyphon (текст поверх wgpu): https://docs.rs/glyphon/latest/glyphon/
- cosmic-text (шейпинг, layout): https://docs.rs/cosmic-text/latest/cosmic_text/
- rstar (R-tree, §6.1): https://docs.rs/rstar/latest/rstar/
- notify (файловый вотчер, §7.5): https://docs.rs/notify/latest/notify/
- rusqlite (§5.2): https://docs.rs/rusqlite/latest/rusqlite/
- serde / serde_json (round-trip без потерь, §5.1): https://docs.rs/serde_json/latest/serde_json/
- image (декод изображений): https://docs.rs/image/latest/image/
- pdfium-render (PDF-превью, §6.2): https://docs.rs/pdfium-render/latest/pdfium_render/
- thiserror / anyhow (ошибки): https://docs.rs/thiserror/latest/thiserror/ · https://docs.rs/anyhow/latest/anyhow/
- tracing (логирование): https://docs.rs/tracing/latest/tracing/
- cargo-wix (MSI): https://github.com/volks73/cargo-wix

### 11.4. Win32 / Shell (Microsoft Learn)

- windows-rs (API-метаданные и биндинги): https://github.com/microsoft/windows-rs · справочник: https://microsoft.github.io/windows-docs-rs/doc/windows/
- IShellItemImageFactory (тамбнейлы, §7.1): https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishellitemimagefactory
- IPreviewHandler и хостинг (§7.2): https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ipreviewhandler · https://learn.microsoft.com/en-us/windows/win32/shell/preview-handlers
- OLE Drag and Drop / IDropTarget (§7.3): https://learn.microsoft.com/en-us/windows/win32/com/drag-and-drop · https://learn.microsoft.com/en-us/windows/win32/api/oleidl/nn-oleidl-idroptarget · CF_HDROP: https://learn.microsoft.com/en-us/windows/win32/shell/clipboard
- ShellExecuteEx (§7.4): https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw
- ReadDirectoryChangesW (что делает notify под капотом, §7.5): https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw
- EnumDisplayMonitors / multi-monitor (§7.4): https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaymonitors
- High DPI / Per-Monitor V2 (§6.5): https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows
- Регистрация ассоциации `.canvas` / ProgID (M4): https://learn.microsoft.com/en-us/windows/win32/shell/fa-progids
- WebView2 (M5): обзор возможностей — https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/overview-features-capabilities · Get started Win32 — https://learn.microsoft.com/en-us/microsoft-edge/webview2/get-started/win32 · `ICoreWebView2` (postMessage, CapturePreview, WebResourceRequested) — https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2 · `SetVirtualHostNameToFolderMapping` — https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_3#setvirtualhostnametofoldermapping
- webview2-com (Rust-биндинги WebView2): https://docs.rs/webview2-com/latest/webview2_com/ · https://github.com/wravery/webview2-rs
- Evergreen Runtime bootstrapper (дистрибуция, T19): https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution

### 11.5. Встройка в десктоп (§7.4) — недокументированная зона

Официальной документации нет. Источники, по которым построена таблица стратегий §7.4:

- Классическая техника (≤ 23H2): «Draw Behind Desktop Icons in Windows», CodeProject: https://www.codeproject.com/Articles/856020/Draw-Behind-Desktop-Icons-in-Windows-plus
- Изменение иерархии в 24H2+ (Progman с `WS_EX_NOREDIRECTIONBITMAP`, DefView как layered child, WorkerW как child ниже DefView — разбор и рабочий код Z-order): https://learn.microsoft.com/en-us/answers/questions/1386630/doubts-about-the-window-of-program-manager · https://stackoverflow.com/questions/79763352/setting-a-window-as-wallpaper
- Референс-реализации, которые уже решают эту задачу на 24H2+/25H2 — изучить код до написания своего:
  - Lively Wallpaper (C#, активно поддерживает 24H2+): https://github.com/rocksdanister/lively
  - Seelen UI (Rust — заодно референс по windows-rs в проде): https://github.com/eythaann/Seelen-UI

Правило для агента: любое утверждение о поведении Progman/WorkerW/DefView, не подтверждённое этими источниками или собственным рантайм-детектом, считается непроверенным и реализуется только за фолбэком.

## 12. Post-Release Documentation Requirement (обязательно)

После каждого релиза агент ОБЯЗАН создать запись в девлоге —
`docs/devlog/` (одна волна/эпик = один файл; например `M5-widgets.md`):

1. Формат: дата, версия/тег, тип (micro | feature)
2. Обязательные поля:
   - **Проблема** — что не работало / чего не было
   - **Решение** — что сделали, 2–4 предложения, без жаргона стека
   - **Почему так, а не иначе** — альтернативы, компромисс
   - **Что было сложного / неожиданного** — для storytelling
   - **Метрика/результат**, если применимо (оценка, если факт неизвестен)

**Триггер «full article»** (не micro-note):
- закрыт epic/milestone из `docs/plans/product-roadmap.md` (волны 0/A/B/V/S)
  или волн T0–T22/M1–M7,
- ИЛИ добавлена фича, которая меняет user-facing поведение.

В этом случае агент готовит черновик статьи 800–1200 слов:
контекст задачи → путь решения → грабли → результат → что дальше.

## 13. MCP-инструменты канваса

Каталог инструментов — `crates/canvas-mcp/src/lib.rs` (константа `TOOLS`,
канонический источник; `tools/list` отдаёт те же схемы автоматически).
Раздел фиксирует контракты, критичные для агентной сборки (ADR-0004:
MCP — единственный канал; CR-013: волна A).

Канонический порядок вызовов для агентной сборки модели (разведка → ноды →
value-связи → пересчёт → батч → валидация) зафиксирован в рецепте
`user-docs/agent-recipe.md` (R5/CP4, CR-013). Коды ошибок `graph_validate`
и операций `graph_apply` — стабильный контракт рецепта: менять их только
вместе с рецептом.

### Headless-верификация MCP (FR-037, ADR-0012)

Слой инструментов (`canvas-scene`), протокольный мост (`canvas-mcp`) и
headless-сервер (`canvas-mcp-headless`) верифицируются в wasm-рантайме:
компиляция под `wasm32-unknown-unknown` (CI `wasm-check`), исполнение
тестов под `wasm32-wasip1` в wasmtime и полная MCP-сессия с реальным
клиентом (initialize → tools/list → tools/call → oracle-гейты эталонов
CP1/CP3/CP5 → негативные ветки) — гейт `scripts/mcp_wasm_gate.sh`, без
Windows и GUI. Живые ручные сессии владельца — официальным инспектором
`@modelcontextprotocol/inspector`: `scripts/mcp_wasm_inspector.sh`
(web UI; `--check` — автоприёмка oracle ±1 %; node/npx — вне гейтов, MW5).
`HeadlessSession` — серверная сторона будущего
WebSocket-моста (волна 2 плана M8).

### graph_apply (FR-033) — атомарная батч-композиция

Схема вызова: `graph_apply { operations: [Op; 1..=256] }`. `Op` — объект
с тегом `op`:

| op | Поля | Примечание |
|---|---|---|
| `node_create_note` | `ref?, x, y, text?, width?, height?` | строки «= …» — формулы (FR-013) |
| `node_create_file` | `ref?, x, y, path, width?, height?` | файл на диске не создаётся |
| `template_instantiate` | `ref?, template, params?, x, y` | `params` — `{имя: число \| {num, unit}}`; вне min/max — ошибка |
| `edge_create` | `fromRef\|from, toRef\|to, kind?, fromLine?, fromOutput?, toParam?, fromSide?, toSide?` | `kind`: `"value"\|"control"` (дефолт control); порты — контракт FR-029; `fromLine`/`fromOutput` взаимно исключительны; имена портов валидируются по снапшотам шаблонов |
| `param_set` | `ref\|id, param, value, unit?` | правит ровно одну строку «param = value unit» (текст + снапшот шаблона); параметра нет — ошибка (без append) |
| `node_move` | `ref\|id, x, y` | |

**Лимиты:** ≤ 256 операций, ≤ 128 новых нод на батч (защита live-бюджета
SPEC §6.3). Превышение — ошибка уровня вызова (isError).

**Транзакционная семантика:** операции применяются к клону канваса;
ошибка ЛЮБОЙ операции → `{ok: false, op_index, code, message}` и канвас
байт-в-байт прежний (клон отброшен); успех → канвас заменяется, ровно
**один** undo-шаг на весь батч (Ctrl+Z откатывает сборку целиком),
полный пересчёт потока, автосейв.

**Ответ (успех):** `{ok: true, created: [{op_index, ref?, node_id?, edge_id?}],
report: [{op_index, op, id}], flow: {node_id: {value, unit, outputs?,
lines?, error?}}}` — `flow` в формате flow_recalc v2 (FR-029): узловые
значения + именованные выходы + построчные значения; второй вызов
flow_recalc не нужен.

**Коды ошибок операций:** `E-BAD-OP` (форма/поля), `E-NOT-FOUND`
(ref/id/шаблон), `E-PORT-UNKNOWN` (неизвестный параметр/выход),
`E-CYCLE` (value-цикл, участники в message), `E-PARAM-UNKNOWN`
(нет строки параметра), `E-RANGE` (вне min/max). Нумерация
`op_index` — с 0; ref-ы живут только внутри батча (адресуют ноды,
созданные ранее в том же вызове).

### whatif_* (FR-017, CP6) — сценарии «а что если»

Девять инструментов поверх активного канваса. Подмены — runtime-only:
канвас без `whatif_apply` не мутируется (инвариант 2).

| Инструмент | Семантика |
|---|---|
| `whatif_set_override {node_id, line, expr}` | построчная подмена активного сценария; режим/сценарий поднимаются автоматически (неявный «Сценарий MCP»); `expr` нормализуется (литеральный `\n` → переводы строк) |
| `whatif_set_param {node_id, param, value}` | sugar: находит строку `param = …` и строит подмену |
| `whatif_scenario_list` | сценарии с числом подмен и маркерами протухших |
| `whatif_scenario_create {name}` | новый сценарий (лимит 3), сразу активен; мутация `canvasdesk.whatif` — один undo-шаг |
| `whatif_scenario_delete {name}` | удаление (undo-шаг) |
| `whatif_scenario_activate {name}` | переключение База ↔ сценарий; runtime-only, файл не трогает |
| `whatif_deltas` | дельты активного сценария — те же пары «было → стало», что видны на канвасе (инвариант 6) |
| `whatif_apply` | записать подмены в persisted-строки/params и удалить сценарий; один undo-шаг |
| `whatif_reset` | сброс подмен активного сценария (runtime) |

### analyze_bottlenecks (FR-016) — узкие места и риск очередей

Схема вызова: `analyze_bottlenecks {}` (без параметров — активный канвас).
Чтение: пересчёт свежий (как `flow_recalc`), канвас и undo не затрагиваются.

**Ответ:** `{nodes: [{id, severity, utilization?, queue_length?, wait_sec?,
badge}], thresholds}` — те же флаги, что видит пользователь на канвасе
(инвариант 4 FR-016: `badge` — строка бейджа канваса, например
`"OVERLOAD 223% · W: 1.2 s"`). `severity` — `none|warn|critical|overload`;
`utilization` — ρ (доля 0..1, > 1 при перегрузке); `wait_sec` — W в базовых
секундах; `thresholds` — пороги дефолта (0.7/0.9, 100 ms/1 s, 1/10).

**Детекция (анализатор `canvas-core/src/analyze.rs`):** значение ноды —
ошибка `Overload{ρ}` → `overload` (ρ из ошибки); именованный выход
`utilization` шаблона (13 queue-манифестов) или Percent-значение → пороги
0.7/0.9; Time-значение (W) → 100 ms/1 s; именованные выходы
`queue_length`/`wait_time` — точки расширения манифестов. Порядок —
`canvas.nodes` (детерминизм).

### Транспорт stdio (FR-034, ADR-0009)

Мост `canvasdesk-mcp` / `canvasdesk mcp` — самостоятельный MCP-сервер:
handshake не зависит от состояния GUI-приложения.

- **Версии протокола:** клиентская версия эхом, если поддерживается
  (`2025-06-18`, `2025-03-26`, `2024-11-05`); неизвестная → `2024-11-05`.
- **Batch-запросы:** JSON-RPC массив обрабатывается поэлементно; пустой
  массив → один ответ `-32600`; батч из уведомлений → тишина.
- **Результат tools/call:** `content[0].text` — чистый JSON результата,
  `structuredContent` — тот же объект (spec 2025-06-18); isError-результат
  приложения проходит насквозь.
- **Offline-режим:** pipe недоступен → `initialize` успешен, `tools/call`
  → isError «CanvasDesk не запущен…»; процесс моста живёт до закрытия
  stdio (никаких exit-кодов на недоступность приложения).
- **Reconnect:** перед каждым входным пакетом при мёртвом транспорте —
  короткая попытка подключения к pipe (500 мс, без автоспавна); приложение,
  поднявшееся позже моста, подхватывается без перезапуска MCP-сессии.
- **Толерантные заглушки:** `resources/list`, `prompts/list`,
  `resources/templates/list` → пустые списки; `logging/setLevel` → `{}`;
  `notifications/cancelled` → игнор.
- Фрейминг — newline-delimited JSON (без Content-Length); таймаут ответа
  приложения — 30 с → isError.

### Чистота stdout (FR-035, ADR-0010)

stdout процесса моста — **только** newline-delimited JSON-RPC; ни логи,
ни ANSI-последовательности, ни вывод дочерних процессов не имеют права
появляться в протокольном канале (нарушение ловится клиентом как
`Invalid JSON`).

- **Изоляция автоспавна:** сервис поднимается с
  `stdin/stdout/stderr = Stdio::null()` — наследование хэндлов моста
  исключено по построению (прежде GUI-логи tracing с ANSI попадали в
  JSON-RPC-поток).
- **Ориентация автоспавна:** единый бинарь `canvasdesk` спавнит сам себя
  (GUI-режим без аргументов); автономный `canvasdesk-mcp` ищет GUI-бинарь
  `canvasdesk.exe` в своём каталоге; соседа нет — offline-режим
  (ADR-0009), рекурсивный спавн исключён.
- **Диагностика — в stderr:** собственные сообщения моста и логи
  GUI (`tracing_subscriber::fmt().with_writer(io::stderr)`) идут в
  stderr; ANSI в логах GUI — только когда stderr — живой терминал.
