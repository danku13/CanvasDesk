# CanvasDesk — спецификация проекта

Рабочее название: **CanvasDesk**. Десктоп Windows как бесконечный зумируемый канвас с документами, заметками и связями. Аналог Obsidian Canvas / Miro, но поверх реальной файловой системы и (в режиме M4) вместо стандартного рабочего стола.

---

## 1. Объём

**В объёме:**
- M1 — ядро канваса: камера (пан/зум), файловые карточки, системные тамбнейлы, сохранение в JSON Canvas
- M2 — текстовые заметки, связи (edges) между нодами, drag-drop из Explorer, вотчер файловой системы
- M3 — живые превью (изображения, PDF, текст/код, Office через preview handlers), миникарта, поиск
- M4 — режим встройки в рабочий стол (WorkerW): канвас за иконками, скрытие системных иконок, перехват контекстного меню
- M5 — движок расширений: виджеты как JS/HTML-микрофронтенды на канвасе (WebView2, манифест, bridge, sandbox)

**Вне объёма (осознанно):**
- Замена shell (таскбар, трей остаются Explorer)
- macOS / Linux (архитектура не должна это блокировать, но реализация — только Windows 10/11 x64)
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
| Файловый вотчер | `notify` 6+ | ReadDirectoryChangesW под капотом |
| Win32/COM | `windows-rs` (features: Win32_UI_Shell, Win32_Graphics_Dwm, System_Com) | Тамбнейлы, preview handlers, WorkerW |
| Тамбнейлы | `IShellItemImageFactory::GetImage` | Системный кэш, совпадает с Explorer |
| PDF | `pdfium-render` (бинарь pdfium, BSD-лицензия) | Быстрый рендер страниц в битмап |
| Изображения | `image` | Декод в RGBA → GPU-текстура |
| Preview handlers | `IPreviewHandler` в out-of-process хосте | Изоляция падающих COM-компонентов |
| Виджеты (M5) | WebView2 Evergreen + `webview2-com` | Живые HTML/JS-ноды; снапшоты через `ICoreWebView2::CapturePreview` |
| Мост виджетов | `postMessage` JSON-RPC (`serde_json`) | Узкий типизированный host API с permissions, без eval |
| Сериализация конфига | `serde` + `toml` | — |
| Логирование | `tracing` + `tracing-subscriber` | Диагностика на машинах пользователей |
| Упаковка | `cargo-wix` → MSI, Authenticode-подпись | M4 требует доверия системы |

## 4. Структура workspace

```
canvasdesk/
├── Cargo.toml               # workspace
├── crates/
│   ├── canvas-core/         # модель данных, JSON Canvas I/O, spatial index — без зависимостей от ОС и GPU
│   ├── canvas-render/       # wgpu-рендер: камера, батчинг, текст, текстуры, LOD
│   ├── canvas-shell/        # Windows-only: тамбнейлы, preview handlers, drag-drop, WorkerW (cfg(windows))
│   ├── canvas-preview-host/ # отдельный exe — песочница для IPreviewHandler
│   ├── canvas-widgets/      # M5: WebView2-хост, bridge, манифесты, снапшоты (cfg(windows))
│   └── canvas-app/          # приложение: event loop, команды, UI-состояние, main()
├── assets/                  # шрифты, иконки нод
├── docs/                    # PRD, ARCHITECTURE, TASKS
└── AGENTS.md                # инструкции для Kimi Code
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

### 6.3. Производительность — целевые метрики

- 60 fps при панорамировании/зуме на сцене из 5 000 нод (GPU уровня GTX 1050 / Iris Xe)
- Холодный старт до первого кадра < 2 с
- Открытие канваса на 1 000 нод < 500 мс (тамбнейлы — асинхронно, карточки появляются сразу)
- Память < 500 МБ на 5 000 нод без живых превью

### 6.4. Текстуры

Атлас менеджер: тамбнейлы укладываются в атласы 2048² (LRU-вытеснение), превью — отдельные текстуры. Формат RGBA8, mipmaps не нужны (LOD дискретный).

### 6.5. DPI

Манифест приложения — **Per-Monitor V2** DPI awareness. Все координаты канваса в логических пикселях (world-space), рендер — в физических (`scale_factor` из winit); при переносе окна между мониторами с разным масштабом — пересоздание surface и пересчёт размеров текста. Не полагаться на системное DPI-виртуальное масштабирование (bitmap-stretch) — текст будет мыльным.

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
| Ввод внутри виджета (M5) | Клик по виджету — фокус виджету; Esc — возврат фокуса канвасу |
| Перемещение виджета (M5) | drag за заголовок/рамку ноды |

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
