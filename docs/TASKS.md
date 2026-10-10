# CanvasDesk — Development Plan with Kimi Code

23 tasks (T0–T22), five milestones (M1–M5). Each task is one Kimi Code session = one commit (or PR). The order within a milestone is strict: every task builds on the results of the previous ones. Before starting, place in the repository root: `docs/SPEC.md` (the specification), `docs/RECIPES.md` (recipes from Lively/Seelen), `AGENTS.md` (the template at the end of this document).

Working principle: one task = one session. Do not give Kimi Code several tasks at once — quality degrades on long contexts.

Milestones: **M1** T1–T6 (core), **M2** T7–T10 (content and FS), **M3** T11–T14 (previews and navigation), **M4** T15–T19 (desktop mode), **M5** T20–T22 (widget engine). M5 is independent of M4 — if desired, it can be done right after M3, earlier than the desktop mode.

---

## Phase 0. Foundation

### T0. Workspace initialization

**Goal:** a cargo workspace of 6 crates per SPEC §4, empty trait stubs, a CI build.

**Prompt:**
```
Создай cargo workspace canvasdesk по структуре из docs/SPEC.md §4.
Каждый crate — lib с минимальным публичным API-скелетом:
- canvas-core: трейты ThumbnailProvider, PreviewProvider, ShellIntegration; структуры Node, Edge, Canvas (serde); без зависимостей на wgpu/windows
- canvas-render, canvas-shell, canvas-widgets, canvas-app, canvas-preview-host — пустые lib/bin с зависимостями из SPEC §3
canvas-app — бинарь, который собирается и выводит версию.
Настрой rust-toolchain.toml (stable), .gitignore, базовый GitHub Actions workflow: cargo check + cargo test + clippy -D warnings на windows-latest.
Критерий: cargo build --workspace проходит, cargo test --workspace проходит.
```

**Acceptance:** `cargo build --workspace`, `cargo test --workspace`, clippy clean.

---

## M1. Canvas core

### T1. Window and render loop

**Prompt:**
```
В canvas-app подними окно winit 0.30 + wgpu 22 (см. docs/SPEC.md §3).
Требования: перерасчёт surface на resize, vsync present mode, цикл render по request_redraw (не непрерывный), очистка экрана тёмным фоном (#1e1e22), обработка CloseRequested.
tracing-логирование инициализации: выбранный GPU-адаптер, backend, формат surface.
Критерий: окно открывается, ресайзится без артефактов, закрывается корректно.
```

### T2. Camera and grid

**Prompt:**
```
Добавь в canvas-render камеру по SPEC §8: панорамирование (средняя кнопка, Space+drag, скролл тачпада), зум Ctrl+колесо к позиции курсора (диапазон 0.05–4.0), pinch.
Преобразования screen↔world — отдельный модуль с юнит-тестами (round-trip точности).
Отрисуй бесконечную сетку в world-space: мелкая 20px, крупная 100px, линии тоньше на мелком zoom, без мерцания при зуме (толщина в screen-space через производные или пересчёт).
Критерий: плавный пан/зум 60fps, юнит-тесты трансформаций зелёные.
```

### T3. Model and JSON Canvas I/O

**Prompt:**
```
В canvas-core реализуй модель по SPEC §5.1: Node (file/text/group), Edge, координаты f32, цвета по JSON Canvas spec.
Сериализация/десериализация .canvas файла (serde_json, pretty). Неизвестные поля и неизвестные типы нод сохранять (flatten + Map) — round-trip файла из Obsidian не должен терять данные (это же гарантирует совместимость с widget-нодами из SPEC §7.6 в будущем).
Расширения: абсолютные пути в file, previewState, brokenLink, объект canvasdesk.
Тесты: парсинг examples с jsoncanvas.org, round-trip без потерь, broken файл → понятная ошибка с указанием поля.
Критерий: cargo test -p canvas-core зелёный, .canvas из приложения открывается в Obsidian.
```

### T4. File cards and text

**Prompt:**
```
Рендер нод в canvas-render:
- карточка: скруглённый прямоугольник (SDF-шейдер или 9-slice), тень, рамка выделения
- заголовок = имя файла (эллипсис при нехватке ширины), иконка-заглушка по расширению
- текст через glyphon: один TextAtlas на сцену, батчинг по шрифту
- выделение ЛКМ (point hit-test), drag перемещения ноды с обновлением модели
Автосейв .canvas с debounce 2s и .bak предыдущей версии (SPEC §9).
Критерий: создать/подвигать карточки, перезапуск → раскладка на месте.
```

### T5. Spatial index and culling

**Prompt:**
```
Интегрируй rstar в canvas-core: R-tree над AABB нод, инкрементальное обновление при перемещении (remove+insert, не rebuild).
В рендере — culling: запрос видимых нод по viewport каждый кадр, построение батчей только для видимых.
Добавь нагрузочный тест-генератор: команда --stress N создаёт сцену из N нод-случайных прямоугольников с текстом.
Замер: HUD с fps (клавиша F3), p95 frame time за последние 300 кадров.
Критерий: 5000 нод, панорамирование 60fps на интегрированной графике; при 100 нодах вне viewport они не попадают в батчи (проверить счётчиком).
```

### T6. System thumbnails

**Prompt:**
```
В canvas-shell (windows-only) реализуй ShellThumbnailProvider по SPEC §7.1:
IShellItemImageFactory::GetImage → HBITMAP → RGBA8. Пул из 4 потоков, очередь с приоритетом видимых нод, результаты через канал в рендер-поток.
В canvas-render — текстурный атлас 2048² с LRU-вытеснением (SPEC §6.4): карточка сначала показывает иконку-заглушку, тамбнейл подгружается асинхронно.
Кэш в SQLite по SPEC §5.2 (инвалидация по mtime).
Критерий: drop на канвас папки с 200 фото — карточки появляются мгновенно, тамбнейлы подтягиваются постепенно, повторный запуск — из кэша.
```

**M1 done → commit, tag v0.1.**

---

## M2. Notes, edges, file system integration

### T7. Text notes

**Prompt:**
```
Текстовые ноды по SPEC: двойной клик по пустому месту → новая заметка в точке клика, инлайн-редактирование (многострочное, курсор, выделение, базовые сочетания), Enter/Esc — завершить.
Реализация ввода: собственный лёгкий text editing state (не брать тяжёлый редактор), шейпинг через cosmic-text, IME через winit не требуется на этом этапе.
Цвет заметки — из палитры JSON Canvas ("1".."6"), выбор через контекстное меню ПКМ.
Критерий: заметки создаются, редактируются, переживают сохранение/загрузку, кириллица корректна.
```

### T8. Edges

**Prompt:**
```
Edges по SPEC §5.1: у ноды при наведении появляются порты (top/right/bottom/left), drag от порта → резиновая линия к курсору → drop на другую ноду создаёт edge.
Рендер: кривая Безье между портами, стрелка на конце, лейбл по двойному клику на линии (инлайн-редактирование как в T7).
Hit-test линии: расстояние до кривой < 6px, выделение, Del удаляет.
При перемещении ноды связанные edges перерисовываются в реальном времени.
Критерий: связи создаются/удаляются/сохраняются, открываются в Obsidian с корректными fromSide/toSide.
```

### T9. Drag-drop from Explorer

**Prompt:**
```
IDropTarget на окне canvas-app по SPEC §7.3: CF_HDROP (файлы и папки).
Drop файлов → ноды с авто-раскладкой сеткой от точки дропа (шаг = размер карточки + 24px, перенос строки после 5).
Drop папки → обход глубины 1, игнорировать скрытые/системные.
Drop текстового URL → нода-заметка с текстом ссылки.
Визуальная обратная связь: подсветка зоны дропа во время DragOver.
Критерий: перетаскивание из Explorer работает, включая множественный выбор; позиция сетки предсказуема.
```

### T10. File watcher

**Prompt:**
```
notify 6+ по SPEC §7.5: watcher на каждую директорию нод, debounce 300мс, события в модель через очередь.
Modify → инвалидировать тамбнейл-кэш, перезапросить.
Rename/Move → обновить file путь в модели и .canvas (rename detection по notify events both mode).
Remove → brokenLink: true, карточка: серая рамка, иконка "файл недоступен", тултип со старым путём. При восстановлении файла по тому же пути — автоматическое снятие флага.
Тесты: интеграционный на tempdir — rename/remove/restore.
Известное ограничение (не чинить здесь): удаление в корзину через Explorer видно косвенно — полноценно закрывается в T16 через SHChangeNotifyRegister (docs/RECIPES.md R12).
Критерий: все три сценария из SPEC §2 (п.5) работают < 1с.
```

**M2 done → commit, tag v0.2.**

---

## M3. Previews and navigation

> **Reordering (owner decision, T13/T14 planning session):**
> T11 and T12 are postponed until after the v1.0 release. M3 is coded as T13+T14
> (navigation), tag v0.3 — upon acceptance of T13/T14. Consequences: PDF text
> extraction for search (T14) is unavailable until T11 returns — see docs/plans/T14-search.md §8.

### T11. Live previews: images, text, PDF

**Prompt:**
```
LOD-система по SPEC §6.2: уровни детализации по zoom с гистерезисом 10% (чтобы не дёргалось на границе).
PreviewProvider-реализации:
- Image: image crate → RGBA → текстура (макс 2048 по длинной стороне)
- Text/Code: чтение первых 80 строк, моноширинный рендер
- PDF: pdfium-render, первая страница, масштаб под ширину карточки
Лимит живых превью: 3 одновременно (SPEC §6.2), при отдалении текстура освобождается.
Прокрутка превью текста/PDF колесом при hover (zoom > 1.5).
Критерий: zoom в PDF → читаемая первая страница; память не растёт при циклах зума туда-сюда (проверить 20 циклов).
```

### T12. Preview host for Office (out-of-process)

**Prompt:**
```
crate canvas-preview-host: отдельный exe по SPEC §7.2.
Протокол по named pipe: запрос {path, width, height} → ответ {rgba pixels} | {error}.
Внутри: резолв IPreviewHandler по расширению из реестра, offscreen-окно, отрисовка, копия битмапа (PrintWindow или DWM thumbnail — выбрать по стабильности, решение зафиксировать в коде).
Родитель: таймаут 3с → kill + фолбэк на тамбнейл; крэш хоста → перезапуск на следующем запросе, счётчик крэшей по расширению (3 крэша → blacklist расширения до перезапуска приложения).
Критерий: .docx/.xlsx показывают превью; зависший handler не вешает канвас; task manager показывает отдельный процесс.
```

### T13. Minimap

**Prompt:**
```
Миникарта по SPEC §6.1: offscreen render pass, сцена масштабируется в quad 220x140px в правом нижнем углу (отступ 16px, полупрозрачный фон, скругление).
Содержимое: упрощённые прямоугольники нод (без текста и текстур, цвет по типу), edges линиями 1px — только если нод < 500, иначе только ноды.
Белая рамка = текущий viewport. Клик по миникарте → центрирование камеры; drag рамки → панорамирование.
Обновление: не каждый кадр, а по изменению сцены или камеры (флаг dirty).
Критерий: на 5000 нод миникарта не съедает > 2ms кадра; навигация кликом точна.
```

### T14. Search

**Prompt:**
```
Ctrl+F: панель поиска поверх канваса (egui-панель или собственный overlay — выбрать, что проще интегрировать с текстовым вводом из T7).
Индекс SQLite FTS5 по SPEC §5.2: имя файла + извлечённый текст (txt/md полностью, pdf — первая страница, остальное — имя).
Результаты списком; Enter/клик → камера плавно летит к ноде (анимация 300ms, ease-out), нода подсвечивается пульсом.
F3/Shift+F3 — цикл по результатам.
Критерий: поиск "смета" находит смета_2026.xlsx; переход к результату плавный и точный.
```

**M3 done → commit, tag v0.3.**

---

## M4. Desktop mode

### T15. Embedding into WorkerW

**Prompt:**
```
Режим --desktop по SPEC §7.4. ОБЯЗАТЕЛЬНО перед кодингом: docs/RECIPES.md R1–R4, R6, R10, R13, R14 и файлы-источники из §7 RECIPES.
Трейт DesktopEmbedder, две стратегии, выбор рантайм-детектом (не номером сборки):
- классическая (≤23H2): 0x052C → top-level WorkerW → SetParent
- 24H2+/25H2: raised desktop (детект по WS_EX_NOREDIRECTIONBITMAP на Progman): WS_EX_LAYERED + SetLayeredWindowAttributes(bAlpha=255) СТРОГО ДО SetParent(progman), затем Z-order — SetWindowPos(hwnd, insert-after=DefView, NOACTIVATE|NOMOVE|NOSIZE) и EnsureWorkerWZOrder (WorkerW обязан оставаться последним ребёнком Progman, иначе HWND_BOTTOM)
Чек-лист реализации (из RECIPES):
- 0x052C (WPARAM=0xD, LPARAM=0x1) слать ТОЛЬКО если WorkerW отсутствует (иначе цикл пересоздания)
- детект WorkerW с retry 10×100мс
- стиль-скраббинг: +WS_CHILDWINDOW, -WS_CLIPSIBLINGS, -WS_EX_APPWINDOW, -WS_EX_WINDOWEDGE, -WS_EX_ACCEPTFILES; после репарентинга — перечитать GWL_STYLE/GWL_EXSTYLE и сверить с ожидаемым (библиотеки окон могут асинхронно перезаписывать стили)
- окно на весь виртуальный экран (EnumDisplayMonitors), WS_EX_NOACTIVATE до первого клика
- watch на разрушение WorkerW: SetWinEventHook(EVENT_OBJECT_DESTROY, поток WorkerW, WINEVENT_OUTOFCONTEXT) + поллинг Progman 2с как резерв; raised → перевыполнить Z-order, классика → полный re-attach
- DPI: после репарентинга не доверять scale_factor окна; поллинг GetDpiForWindow 500–1000мс, при смене — пересоздание surface (SPEC §6.5)
- ЛЮБАЯ ошибка шага → фолбэк на обычное окно + MessageBox (не только при неизвестной версии)
Первичная отладка — на dev-машине Win11 25H2 build 26200.
Критерий: канвас за иконками и перед обоями; перезапуск Explorer → канвас вернулся; Alt+Tab окно не показывает; стили после репарентинга верифицированы.
```

### T16. System event bus (shell_events)

**Prompt:**
```
Модуль shell_events в canvas-shell по образцу docs/RECIPES.md R11 (чистая реализация, не копия):
скрытое message-only окно (HWND_MESSAGE) на отдельном потоке с собственным GetMessageW-циклом; события рассылаются подписчикам через crossbeam-канал; единая точка подписки из приложения.
Регистрации на окне:
- WTSRegisterSessionNotification(NOTIFY_FOR_ALL_SESSIONS) → WM_WTSSESSION_CHANGE, сверка l_param session id со своей сессией → атомик IS_INTERACTIVE_SESSION
- RegisterSuspendResumeNotification → sleep/resume (перед сном — форс-сейв .canvas)
- RegisterWindowMessage("TaskbarCreated") → событие ExplorerStarted (потребитель — T17)
- RegisterShellHookWindow → shell-события окон (потребитель — T18)
- AddClipboardFormatListener + ChangeWindowMessageFilterEx(WM_CLIPBOARDUPDATE, MSGFLT_ALLOW) — на будущее "вставить как ноду"
- SHChangeNotifyRegister(SHCNRF_ShellLevel) → shell file events: проброс в вотчер из T10; удаление в корзину через Explorer → brokenLink (RECIPES R12)
Enum-обёртки по идиоме RECIPES R13: boxed closure через LPARAM, extern "system" трамплин, safe API наружу.
Критерий: lock/unlock сессии виден в логе с корректным session id; удаление файла в корзину из Explorer → brokenLink на карточке < 1с; окно невидимо и не в Alt+Tab.
```

### T17. Icons, menu, exit, crash safety

**Prompt:**
```
По SPEC §7.4 п.5–7 и docs/RECIPES.md R5, R7, R9:
- Скрытие иконок идемпотентно (R5): SHGetSetSettings(SSF_HIDEICONS) чтение → toggle WM_COMMAND 0x7402 на SHELLDLL_DefView ТОЛЬКО при расхождении состояний; исходное состояние сохранить, восстановить при штатном выходе.
- Краш-сейф: sentinel-файл "предыдущая сессия завершилась аварийно" → форс-восстановление иконок при старте.
- Детект краша Explorer (R7): событие ExplorerStarted из T16 + сравнение PID Shell_TrayWnd до/после (TaskbarCreated приходит и на DPI-смену); анти-флуд: >1 перезапуска за 30с → стоп автоматики, сообщение пользователю.
- НЕ использовать SPI_SETDESKWALLPAPER на raised desktop (R9); рефреш — InvalidateRect+UpdateWindow на DefView.
- Своё контекстное меню по ПКМ: Открыть канвас / Новый текстовый файл / Показать системные иконки (toggle) / Выход.
- Двойной клик по файловой ноде → ShellExecuteEx SEE_MASK_INVOKEIDLIST.
- Автозапуск: HKCU\...\Run с флагом --desktop, пункт меню "Запускать с Windows".
Критерий: иконки прячутся и возвращаются; kill -9 процесса → следующий запуск восстанавливает иконки; двойной клик открывает файл; crash-loop Explorer не вешает приложение.
```

### T18. Power saving (render pause)

**Prompt:**
```
Pause-контроллер по docs/RECIPES.md R15–R17 и SPEC §9:
- Тик 1с (таймер, НЕ WinEventHook по LOCATIONCHANGE — шумно, R15).
- Иерархия условий паузы: 1) !IS_INTERACTIVE_SESSION (из T16) или RDP; 2) батарея offline / Battery Saver (опции в конфиге, GetSystemPowerStatus); 3) SHQueryUserNotificationState == QUNS_RUNNING_D3D_FULL_SCREEN; 4) foreground-окно покрывает рабочую область своего монитора (исключения классов: Progman, WorkerW, Windows.UI.Core.CoreWindow — список "это десктоп").
- IsDesktop() (R17): foreground ∈ {progman, original_WorkerW} — использовать также для гейта хоткеев в M4 (SPEC §9).
- Пауза = стоп request_redraw (рендер-цикл спит), заморозка тамбнейл-пула, preview host и (с M5) live-виджетов; пробуждение по тику с "десктоп виден" — один принудительный кадр.
- Перед sleep (событие из T16) — форс-сейв .canvas.
Критерий: полноэкранное окно поверх → GPU-нагрузка приложения ~0 (Process Explorer); локскрин → пауза; возврат → мгновенное восстановление без мерцания.
```

### T19. Packaging

**Prompt:**
```
MSI через cargo-wix: canvasdesk.exe + canvas-preview-host.exe + pdfium binary + assets (включая assets/widgets/) + WebView2 Evergreen bootstrapper (SPEC §9, офлайн-фолбэк — ссылка на скачивание), ярлык, автозапуск опционально (checkbox).
Конфиг %APPDATA%/canvasdesk/config.toml: последний канвас, режим (window/desktop), лимиты превью и виджетов.
Структура %APPDATA%/canvasdesk/widgets/ для пользовательских виджетов (SPEC §7.6).
Ассоциация .canvas → canvasdesk (ProgID, иконка).
Инструкция по Authenticode-подписи в docs/RELEASE.md (сертификат покупает владелец, CI-шаг signtool описать).
Версия из git tag → ресурсы exe.
Критерий: чистая Win11 VM → установка → --desktop работает → деинсталляция не оставляет следов (кроме %APPDATA%).
```

**M4 done → commit, tag v1.0.**

---

## M5. Widget engine (JS/HTML micro-frontends)

> **Wave M5 plan:** `docs/plans/M5-widgets.md` — detailed architecture,
> product decisions (П1–П11), file zones and tests for T20–T22. Early references to
> `%APPDATA%/canvasdesk/widgets` are clarified: the actual root of the packages is
> `~/.canvasdesk/widgets` (SPEC §7.6, «Уточнения v1.1»).

### T20. Widget runtime (WebView2 host)

**Prompt:**
```
crate canvas-widgets по SPEC §7.6. Рендер виджет-ноды:
- WebView2 Evergreen через webview2-com: создание инстанса в дочернем HWND, общий user-data-folder на приложение
- синхронизация rect ноды с камерой каждый кадр: SetWindowPos(SWP_ASYNCWINDOWPOS); скругление — SetWindowRgn
- LOD: zoom < 0.25 или нода вне viewport → WebView2 скрыт+приостановлен, рендерится snapshot-текстура (CapturePreview → RGBA → атлас; обновление по событию изменения или раз в 5с); возврат в live — по порогу с гистерезисом
- лимит live-инстансов (6, конфиг): LRU-выталкивание в snapshot
- заголовок/рамка ноды рисуются канвасом (зона drag 24px), ввод внутри — WebView2, Esc возвращает фокус канвасу (SPEC §8)
- airspace-ограничение из SPEC §7.6 зафиксировать в коде: миникарта/поиск/меню не перекрывают live-виджеты
Демо: статичный test-widget (index.html с часами на JS) ставится нодой на канвас, двигается, связывается ребром.
Критерий: виджет живой на zoom≥0.25, snapshot на дальнем; 10 виджетов на сцене — 60fps за счёт лимита live; props-геометрия переживает перезапуск.
```

### T21. Bridge, manifest, permissions

**Prompt:**
```
По SPEC §7.6:
- Парсинг и валидация widget.json (serde, строгая схема, неизвестные поля — warn)
- Двусторонний JSON-RPC поверх postMessage/WebMessageReceived: host→widget {init, propsChanged, visibility, themeChanged}, widget→host {ready, resize, setProps, openFile, readDir, toast}; все сообщения десериализуются по схеме, невалидные — drop+warn
- Enforcement permissions манифеста на КАЖДЫЙ вызов; без network — блокировка внешних WebResourceRequested; навигация вне пакета запрещена; SetVirtualHostNameToFolderMapping для origin пакета; CSP default-src 'self'
- setProps → persist в .canvas (canvasdesk.props), widget_state в SQLite для объёмного состояния
- установка виджета: drag папки с widget.json на канвас → копирование в %APPDATA%/canvasdesk/widgets/<id>/ → нода на канвасе
- 3 встроенных виджета в assets/widgets/: часы/дата, календарь, стикер
Критерий: вызов без permission блокируется и логируется; remote URL как виджет отвергается; props переживают экспорт/импорт .canvas; встроенные виджеты работают.
```

### T22. Widget SDK and examples

**Prompt:**
```
- docs/WIDGETS.md: формат пакета, манифест, bridge API (таблица сообщений), permissions, LOD-жизненный цикл, ограничения (airspace, лимиты)
- Шаблон виджета: Vite + TypeScript, билд в статику, npm-скрипт pack → папка для установки; клиентская обёртка над postMessage bridge (типизированный canvasdesk.ts, < 150 строк)
- Два примера: 1) панель задач с сохранением в props (offline), 2) дашборд с fetch наружу (демонстрация permission network)
- Пример "микрофронтенд": собрать один и тот же bundle как standalone-страницу и как виджет — разница только в адаптере инициализации
Критерий: по docs/WIDGETS.md новый виджет собирается и ставится на канвас за 30 минут без чтения исходников хоста.
```

**M5 done → commit, tag v1.1.**

---

## M6. Extensions and intelligence (BYOK / agent integration)

> A new milestone — it goes beyond the base specification M1–M5. Two tasks: dynamic edge highlighting (a brainstorming aid) and the BYOK format / MCP server for working with Hermes Agent (generating Mindmaps and structured notes from any text).


### T23. Dynamic edge highlighting (brainstorm-focus)

**Prompt:**
```

По наведению курсора на любую ноду канваса (file/text/group/widget) автоматически подсвечиваются все связанные ноды (через Edge) на глубину 1 (прямые связи) или настраиваемую глубину 1–3. Неподсвеченные ноды затемняются (dim) с настраиваемой интенсивностью 30–70% и возможностью полного скрытия. Настройки: клавиша Shift — глубина 2; Ctrl — глубина 3; без модификатора — глубина 1. Конфиг в config.toml: focus_depth_default (1/2/3), focus_dim_alpha (0.3–0.7), focus_hide_others (bool). При наведении на edge — подсвечиваются обе связанные ноды и все их прямые связи (звезда). Реализация: в canvas-core — метод calc_connected_nodes(node_id, max_depth); в canvas-render — шейдер или пост-процесс для затемнения: ноды вне подсвеченного набора рисуются с уменьшенным alpha или вообще пропускаются в батч; подсвеченные ноды получают яркую рамку и лёгкую тень. Визуальная обратная связь — плавная анимация перехода 150мс (ease-in-out). Критерий: на сцене из 200 нод с 150 связями — наведение работает < 1 кадр (60fps); настройка затемнения меняет визуал мгновенно; мозговой штурм с фокусом на одной ноде виден наглядно (неподсвеченные ноды тускнеют или исчезают по конфигу).
```

**Acceptance:** `F3` or the `B` key enables focus mode; edge highlighting works with the modifiers; dimming is adjusted in `config.toml`; performance does not drop on 5000 nodes.

---

### T24. BYOK format and MCP server for Hermes Agent

**Prompt:**
```
Ввести BYOK (Bring Your Own Knowledge) формат — расширение .canvas или отдельный .byok-файл, который описывает входной текст/документ и желаемую структуру выхода. Формат JSON: { "source": "text или путь", "mode": "mindmap | structured_notes | outline | summary", "target_nodes": [ { "id": "...", "label": "...", "parent": "...", "content": "..." } ], "params": { "language": "ru/en", "max_depth": 3, "detail_level": "brief | full" } }. MCP-сервер (Model Context Protocol) в отдельном crate canvas-mcp (или как модуль в canvas-app) — локальный stdio/HTTP-сервер, слушающий команды Hermes Agent: команда /canvasdesk.generate из агента → MCP получает текст или путь к .canvas → возвращает JSON с предложением новых нод/связей или полный .canvas-файл. Реализация: 1) docs/BYOK.md — спецификация формата и примеры; 2) crate canvas-mcp с трейтом ByokGenerator, реализация для режима mindmap (дерево нод с родительскими ссылками) и structured_notes (плоский список с тегами); 3) интеграция в canvas-app: пункт контекстного меню "Сгенерировать заметки (BYOK)" или команда CLI --byok-generate <file>; 4) Hermes Agent вызывает MCP-сервер (localhost или stdio), получает .canvas-фрагмент и вставляет его в текущий канвас с сохранением неизвестных полей (round-trip). Безопасность: MCP-сервер работает только локально, без сетевых вызовов; входной текст читается из файла или stdin; выход — только JSON в .canvas-совместимом формате. Критерий: Hermes Agent через MCP-сервер генерирует из текста "Проект Альфа: цели, риски, сроки" mindmap из 8–12 нод с корректными связями, которую можно вставить в канвас и открыть в Obsidian; structured_notes из того же текста — список заметок с тегами.
```

**Acceptance:** the file `docs/BYOK.md` describes the format; `cargo build -p canvas-mcp` passes; the CLI command `--byok-generate docs/sample.md --mode mindmap` creates a `.canvas` with a tree of nodes; insertion into the current canvas via the context menu works; unknown fields from .canvas are preserved (compatibility). The MCP server answers a Hermes Agent request in the JSON-RPC format.

---

### T25. (Optional) SDK for BYOK widgets

**Goal:** a widget template that uses the BYOK format to generate dynamic notes or knowledge maps from local text.

**Prompt:**
```
В assets/widgets/byok-generator/ — виджет, принимающий текстовый ввод (textarea) и через bridge-вызов byokGenerate отправляющий текст на локальный MCP-сервер; ответ — JSON-структура нод, которую виджет рендерит как мини-карту прямо в своей области или предлагает вставить в основной канвас через postMessage. Виджет работает офлайн, не требует network-пермишна (данные не покидают машину). Критерий: виджет устанавливается из папки, работает в sandbox, возвращает структуру .canvas.
```

**M6 done → commit, tag v1.2 (optionally v1.1.1 if M6 ships without T25).**

## M7. Cross-platform (Windows / Linux / macOS)

> **Wave M7 plan:** `docs/plans/M7-crossplatform.md` — the platform
> matrix, the porting architecture (traits + cfg, without sprawl across
> canvas-app), a full dependency audit and a decision on every gap
> (drag-drop via winit events, FDO thumbnails, the UDS transport for MCP).
> Owner directive (2026-09-14): the application is cross-platform,
> built on Windows, Linux and macOS. Desktop mode and live widgets
> outside Windows are out of scope for v1.2 (see plan §1).

### T26. Dependency hygiene and clippy pass

**Prompt:**
```
По плану M7 §4–§5 (T26):
- canvas-app: cosmic-text перенести из [dependencies] в [dev-dependencies]
  (используется только в integration-тестах: FontSystem, Action)
- локальная win-проба становится clippy-пробой:
  cargo clippy --target x86_64-pc-windows-msvc -p canvas-widgets -p canvas-core -p canvas-render
  (урок a9488ae: check без clippy пропустил unused_mut)
Критерий: поведение не изменилось; fmt/clippy/test зелёные; diff — только Cargo.toml.
```

### T27. Unix thumbnails (FDO cache + image decoding)

**Prompt:**
```
По плану M7 §3.2: UnixThumbnailProvider за трейтом ThumbnailProvider:
- изображения — декод через image (расширить фичи jpeg/gif/webp с обоснованием)
- прочее — MIME-тип → заглушка-иконка
- кэш по спецификации freedesktop thumbnails (~/.cache/thumbnails/{normal,large}),
  инвалидация mtime, повреждённые записи — игнор с warn
- выбор реализации в main по cfg (образец Shell/Noop-провайдеров)
Критерий: на Linux папка с изображениями показывает тамбнейлы (не заглушки); тесты tempdir на чтение/запись/инвалидацию FDO-кэша.
```

### T28. Unix drag-drop (winit events)

**Prompt:**
```
По плану M7 §3.2: на не-Windows принимать WindowEvent::HoveredFile/DroppedFile;
классификация — существующие plan_drop/expand_drop_paths (чистые функции);
призрак-план — общий код без Win32-модификаторов. Windows-путь (IDropTarget)
не трогаем.
Критерий: drag папки из Nautilus/Dolphin разворачивается в ноды; тесты маршрута событий и ghost-состояний зелёные на Linux.
```

### T29. MCP on Unix (Unix domain socket)

**Prompt:**
```
По плану M7 §3.2: транспорт ~/.canvasdesk/mcp.sock (UDS) — зеркало mcp_pipe:
- сервер в canvas-app (cfg(unix)), line-framing JSON без изменений
- canvasdesk-mcp: детект ОС, коннект к сокету; автостарт сервиса как на Windows
Критерий: canvasdesk mcp работает с реальным AI-клиентом на Linux; тест round-trip initialize/tools-call на tempdir-сокете.
```

### T30. Unix distribution

**Prompt:**
```
По плану M7 §5: rust-toolchain.toml (pin minor), артефакты tar.gz (Linux) +
zip (macOS) в build-all, ENVIRONMENT.md — секции Linux/macOS.
Критерий: релизная сборка на трёх ОС одним workflow; свежая машина Linux собирается по ENVIRONMENT.md без дополнительных инструкций.
```

**M7 done → commit, tag v1.3. CI gates — on three OSes (branch ci-matrix, owner push — C-1).**


---

## AGENTS.md template (place in the repository root)

```markdown
# CanvasDesk — инструкции для агента

## Контекст
Читай docs/SPEC.md перед любой задачей. Для задач shell-интеграции (T15–T18) — также
docs/RECIPES.md (рецепты R1–R17 и файлы-источники). Это единственные источники истины.

## Стек
Rust stable, edition 2021. winit + wgpu + glyphon (рендер), windows-rs (shell),
webview2-com (виджеты), notify, rusqlite, rstar, serde_json. Версии — в Cargo.toml.

## Правила
1. canvas-core не зависит от ОС и GPU. Платформенное — только за трейтами.
2. Windows-only код — в canvas-shell и canvas-widgets под cfg(windows). Core собирается на Linux.
3. Недокументированные Win32-приёмы (WorkerW, 0x7402, 0x052C) — только за рантайм-детектом
   с фолбэком, по рецептам docs/RECIPES.md. Каждый — с комментарием-ссылкой на рецепт.
4. Рецепты из RECIPES.md реализуются чисто: Lively — GPL-3.0, Seelen UI — AGPL-3.0,
   копирование кода/идентификаторов запрещено, переносим только механику.
5. Никаких unwrap/expect в production-путях — thiserror + anyhow на границах.
6. unsafe — только в canvas-shell/canvas-widgets, каждый блок с SAFETY-комментарием.
7. Enum-обёртки Win32 — по идиоме RECIPES R13 (boxed closure через LPARAM, safe API).
8. Виджеты: только локальные пакеты; permissions манифеста проверяются на каждый вызов;
   никаких remote URL; все postMessage валидируются serde-схемами, невалидное — drop+warn.
9. Новые зависимости — только с обоснованием в описании PR.

## Качество
- cargo fmt --check, cargo clippy -- -D warnings, cargo test --workspace — обязательно зелёные.
- Юнит-тесты для canvas-core обязательны; canvas-shell — интеграционные там, где возможно.
- Каждая задача завершается коммитом с сообщением по conventional commits.

## Не делать
- Не менять формат .canvas несовместимо с jsoncanvas.org без явной задачи.
- Не добавлять сетевые вызовы в хост — продукт локальный (сеть только внутри виджетов
  с permission network).
- Не блокировать рендер-поток: всё I/O и COM — в worker-потоках.
- Не слать 0x052C при существующем WorkerW (RECIPES R4), не использовать
  SPI_SETDESKWALLPAPER на raised desktop (RECIPES R9).
```

---

## Timeline estimate with Kimi Code

| Phase | Tasks | Estimate (calendar, 1 developer + Kimi Code) |
|---|---|---|
| 0 + M1 | T0–T6 | 2–3 weeks |
| M2 | T7–T10 | 1.5–2 weeks |
| M3 | T11–T14 | 2–3 weeks |
| M4 | T15–T19 | 3–4 weeks |
| M5 | T20–T22 | 2–3 weeks |
| **Total** | T0–T22 | **10–14 weeks** |

M4 and M5 are independent of each other (both build on M3) — with two threads or a change of priorities, M5 can be done earlier than M4. Keep the main risk buffer on T15/T17: AI does not speed up debugging of undocumented Windows behavior on a live system.

## Startup order

1. Create the repository, place `docs/SPEC.md`, `docs/RECIPES.md`, this file as `docs/TASKS.md`, `AGENTS.md` in the root
2. `rustup default stable`, Windows SDK (via Visual Studio Build Tools)
3. Start Kimi Code with T0 → run through T0–T6, check the M1 criteria by hand
4. After each milestone — manual acceptance per SPEC §10, and only then the next phase
