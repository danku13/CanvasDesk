# CanvasDesk — план разработки с Kimi Code

23 задачи (T0–T22), пять milestone'ов (M1–M5). Каждая задача — одна сессия Kimi Code = один коммит (или PR). Порядок строгий внутри milestone'а: каждая задача опирается на результат предыдущих. Перед стартом — положить в корень репозитория: `docs/SPEC.md` (спецификация), `docs/RECIPES.md` (рецепты из Lively/Seelen), `AGENTS.md` (шаблон в конце документа).

Принцип работы: одна задача = одна сессия. Не давать Kimi Code несколько задач сразу — деградация качества на длинных контекстах.

Milestones: **M1** T1–T6 (ядро), **M2** T7–T10 (контент и ФС), **M3** T11–T14 (превью и навигация), **M4** T15–T19 (режим десктопа), **M5** T20–T22 (движок виджетов). M5 независим от M4 — при желании его можно делать сразу после M3, раньше режима десктопа.

---

## Фаза 0. Фундамент

### T0. Инициализация workspace

**Цель:** cargo workspace из 6 crates по SPEC §4, пустые заглушки трейтов, CI-сборка.

**Промпт:**
```
Создай cargo workspace canvasdesk по структуре из docs/SPEC.md §4.
Каждый crate — lib с минимальным публичным API-скелетом:
- canvas-core: трейты ThumbnailProvider, PreviewProvider, ShellIntegration; структуры Node, Edge, Canvas (serde); без зависимостей на wgpu/windows
- canvas-render, canvas-shell, canvas-widgets, canvas-app, canvas-preview-host — пустые lib/bin с зависимостями из SPEC §3
canvas-app — бинарь, который собирается и выводит версию.
Настрой rust-toolchain.toml (stable), .gitignore, базовый GitHub Actions workflow: cargo check + cargo test + clippy -D warnings на windows-latest.
Критерий: cargo build --workspace проходит, cargo test --workspace проходит.
```

**Приёмка:** `cargo build --workspace`, `cargo test --workspace`, clippy чисто.

---

## M1. Ядро канваса

### T1. Окно и рендер-цикл

**Промпт:**
```
В canvas-app подними окно winit 0.30 + wgpu 22 (см. docs/SPEC.md §3).
Требования: перерасчёт surface на resize, vsync present mode, цикл render по request_redraw (не непрерывный), очистка экрана тёмным фоном (#1e1e22), обработка CloseRequested.
tracing-логирование инициализации: выбранный GPU-адаптер, backend, формат surface.
Критерий: окно открывается, ресайзится без артефактов, закрывается корректно.
```

### T2. Камера и сетка

**Промпт:**
```
Добавь в canvas-render камеру по SPEC §8: панорамирование (средняя кнопка, Space+drag, скролл тачпада), зум Ctrl+колесо к позиции курсора (диапазон 0.05–4.0), pinch.
Преобразования screen↔world — отдельный модуль с юнит-тестами (round-trip точности).
Отрисуй бесконечную сетку в world-space: мелкая 20px, крупная 100px, линии тоньше на мелком zoom, без мерцания при зуме (толщина в screen-space через производные или пересчёт).
Критерий: плавный пан/зум 60fps, юнит-тесты трансформаций зелёные.
```

### T3. Модель и JSON Canvas I/O

**Промпт:**
```
В canvas-core реализуй модель по SPEC §5.1: Node (file/text/group), Edge, координаты f32, цвета по JSON Canvas spec.
Сериализация/десериализация .canvas файла (serde_json, pretty). Неизвестные поля и неизвестные типы нод сохранять (flatten + Map) — round-trip файла из Obsidian не должен терять данные (это же гарантирует совместимость с widget-нодами из SPEC §7.6 в будущем).
Расширения: абсолютные пути в file, previewState, brokenLink, объект canvasdesk.
Тесты: парсинг examples с jsoncanvas.org, round-trip без потерь, broken файл → понятная ошибка с указанием поля.
Критерий: cargo test -p canvas-core зелёный, .canvas из приложения открывается в Obsidian.
```

### T4. Файловые карточки и текст

**Промпт:**
```
Рендер нод в canvas-render:
- карточка: скруглённый прямоугольник (SDF-шейдер или 9-slice), тень, рамка выделения
- заголовок = имя файла (эллипсис при нехватке ширины), иконка-заглушка по расширению
- текст через glyphon: один TextAtlas на сцену, батчинг по шрифту
- выделение ЛКМ (point hit-test), drag перемещения ноды с обновлением модели
Автосейв .canvas с debounce 2s и .bak предыдущей версии (SPEC §9).
Критерий: создать/подвигать карточки, перезапуск → раскладка на месте.
```

### T5. Spatial index и culling

**Промпт:**
```
Интегрируй rstar в canvas-core: R-tree над AABB нод, инкрементальное обновление при перемещении (remove+insert, не rebuild).
В рендере — culling: запрос видимых нод по viewport каждый кадр, построение батчей только для видимых.
Добавь нагрузочный тест-генератор: команда --stress N создаёт сцену из N нод-случайных прямоугольников с текстом.
Замер: HUD с fps (клавиша F3), p95 frame time за последние 300 кадров.
Критерий: 5000 нод, панорамирование 60fps на интегрированной графике; при 100 нодах вне viewport они не попадают в батчи (проверить счётчиком).
```

### T6. Системные тамбнейлы

**Промпт:**
```
В canvas-shell (windows-only) реализуй ShellThumbnailProvider по SPEC §7.1:
IShellItemImageFactory::GetImage → HBITMAP → RGBA8. Пул из 4 потоков, очередь с приоритетом видимых нод, результаты через канал в рендер-поток.
В canvas-render — текстурный атлас 2048² с LRU-вытеснением (SPEC §6.4): карточка сначала показывает иконку-заглушку, тамбнейл подгружается асинхронно.
Кэш в SQLite по SPEC §5.2 (инвалидация по mtime).
Критерий: drop на канвас папки с 200 фото — карточки появляются мгновенно, тамбнейлы подтягиваются постепенно, повторный запуск — из кэша.
```

**M1 done → коммит, тег v0.1.**

---

## M2. Заметки, связи, интеграция с файловой системой

### T7. Текстовые заметки

**Промпт:**
```
Текстовые ноды по SPEC: двойной клик по пустому месту → новая заметка в точке клика, инлайн-редактирование (многострочное, курсор, выделение, базовые сочетания), Enter/Esc — завершить.
Реализация ввода: собственный лёгкий text editing state (не брать тяжёлый редактор), шейпинг через cosmic-text, IME через winit не требуется на этом этапе.
Цвет заметки — из палитры JSON Canvas ("1".."6"), выбор через контекстное меню ПКМ.
Критерий: заметки создаются, редактируются, переживают сохранение/загрузку, кириллица корректна.
```

### T8. Связи (edges)

**Промпт:**
```
Edges по SPEC §5.1: у ноды при наведении появляются порты (top/right/bottom/left), drag от порта → резиновая линия к курсору → drop на другую ноду создаёт edge.
Рендер: кривая Безье между портами, стрелка на конце, лейбл по двойному клику на линии (инлайн-редактирование как в T7).
Hit-test линии: расстояние до кривой < 6px, выделение, Del удаляет.
При перемещении ноды связанные edges перерисовываются в реальном времени.
Критерий: связи создаются/удаляются/сохраняются, открываются в Obsidian с корректными fromSide/toSide.
```

### T9. Drag-drop из Explorer

**Промпт:**
```
IDropTarget на окне canvas-app по SPEC §7.3: CF_HDROP (файлы и папки).
Drop файлов → ноды с авто-раскладкой сеткой от точки дропа (шаг = размер карточки + 24px, перенос строки после 5).
Drop папки → обход глубины 1, игнорировать скрытые/системные.
Drop текстового URL → нода-заметка с текстом ссылки.
Визуальная обратная связь: подсветка зоны дропа во время DragOver.
Критерий: перетаскивание из Explorer работает, включая множественный выбор; позиция сетки предсказуема.
```

### T10. Файловый вотчер

**Промпт:**
```
notify 6+ по SPEC §7.5: watcher на каждую директорию нод, debounce 300мс, события в модель через очередь.
Modify → инвалидировать тамбнейл-кэш, перезапросить.
Rename/Move → обновить file путь в модели и .canvas (rename detection по notify events both mode).
Remove → brokenLink: true, карточка: серая рамка, иконка "файл недоступен", тултип со старым путём. При восстановлении файла по тому же пути — автоматическое снятие флага.
Тесты: интеграционный на tempdir — rename/remove/restore.
Известное ограничение (не чинить здесь): удаление в корзину через Explorer видно косвенно — полноценно закрывается в T16 через SHChangeNotifyRegister (docs/RECIPES.md R12).
Критерий: все три сценария из SPEC §2 (п.5) работают < 1с.
```

**M2 done → коммит, тег v0.2.**

---

## M3. Превью и навигация

> **Переупорядочивание (решение владельца, сессия планирования T13/T14):**
> T11 и T12 отложены на период после релиза v1.0. M3 кодируется как T13+T14
> (навигация), тег v0.3 — по приёмке T13/T14. Следствия: извлечение текста PDF
> для поиска (T14) недоступно до возврата T11 — см. docs/plans/T14-search.md §8.

### T11. Живые превью: изображения, текст, PDF

**Промпт:**
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

### T12. Preview host для Office (out-of-process)

**Промпт:**
```
crate canvas-preview-host: отдельный exe по SPEC §7.2.
Протокол по named pipe: запрос {path, width, height} → ответ {rgba pixels} | {error}.
Внутри: резолв IPreviewHandler по расширению из реестра, offscreen-окно, отрисовка, копия битмапа (PrintWindow или DWM thumbnail — выбрать по стабильности, решение зафиксировать в коде).
Родитель: таймаут 3с → kill + фолбэк на тамбнейл; крэш хоста → перезапуск на следующем запросе, счётчик крэшей по расширению (3 крэша → blacklist расширения до перезапуска приложения).
Критерий: .docx/.xlsx показывают превью; зависший handler не вешает канвас; task manager показывает отдельный процесс.
```

### T13. Миникарта

**Промпт:**
```
Миникарта по SPEC §6.1: offscreen render pass, сцена масштабируется в quad 220x140px в правом нижнем углу (отступ 16px, полупрозрачный фон, скругление).
Содержимое: упрощённые прямоугольники нод (без текста и текстур, цвет по типу), edges линиями 1px — только если нод < 500, иначе только ноды.
Белая рамка = текущий viewport. Клик по миникарте → центрирование камеры; drag рамки → панорамирование.
Обновление: не каждый кадр, а по изменению сцены или камеры (флаг dirty).
Критерий: на 5000 нод миникарта не съедает > 2ms кадра; навигация кликом точна.
```

### T14. Поиск

**Промпт:**
```
Ctrl+F: панель поиска поверх канваса (egui-панель или собственный overlay — выбрать, что проще интегрировать с текстовым вводом из T7).
Индекс SQLite FTS5 по SPEC §5.2: имя файла + извлечённый текст (txt/md полностью, pdf — первая страница, остальное — имя).
Результаты списком; Enter/клик → камера плавно летит к ноде (анимация 300ms, ease-out), нода подсвечивается пульсом.
F3/Shift+F3 — цикл по результатам.
Критерий: поиск "смета" находит смета_2026.xlsx; переход к результату плавный и точный.
```

**M3 done → коммит, тег v0.3.**

---

## M4. Режим десктопа

### T15. Встройка в WorkerW

**Промпт:**
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

### T16. Шина системных событий (shell_events)

**Промпт:**
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

### T17. Иконки, меню, выход, краш-сейф

**Промпт:**
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

### T18. Энергосбережение (пауза рендера)

**Промпт:**
```
Pause-контроллер по docs/RECIPES.md R15–R17 и SPEC §9:
- Тик 1с (таймер, НЕ WinEventHook по LOCATIONCHANGE — шумно, R15).
- Иерархия условий паузы: 1) !IS_INTERACTIVE_SESSION (из T16) или RDP; 2) батарея offline / Battery Saver (опции в конфиге, GetSystemPowerStatus); 3) SHQueryUserNotificationState == QUNS_RUNNING_D3D_FULL_SCREEN; 4) foreground-окно покрывает рабочую область своего монитора (исключения классов: Progman, WorkerW, Windows.UI.Core.CoreWindow — список "это десктоп").
- IsDesktop() (R17): foreground ∈ {progman, original_WorkerW} — использовать также для гейта хоткеев в M4 (SPEC §9).
- Пауза = стоп request_redraw (рендер-цикл спит), заморозка тамбнейл-пула, preview host и (с M5) live-виджетов; пробуждение по тику с "десктоп виден" — один принудительный кадр.
- Перед sleep (событие из T16) — форс-сейв .canvas.
Критерий: полноэкранное окно поверх → GPU-нагрузка приложения ~0 (Process Explorer); локскрин → пауза; возврат → мгновенное восстановление без мерцания.
```

### T19. Упаковка

**Промпт:**
```
MSI через cargo-wix: canvas-app.exe + canvas-preview-host.exe + pdfium binary + assets (включая assets/widgets/) + WebView2 Evergreen bootstrapper (SPEC §9, офлайн-фолбэк — ссылка на скачивание), ярлык, автозапуск опционально (checkbox).
Конфиг %APPDATA%/canvasdesk/config.toml: последний канвас, режим (window/desktop), лимиты превью и виджетов.
Структура %APPDATA%/canvasdesk/widgets/ для пользовательских виджетов (SPEC §7.6).
Ассоциация .canvas → canvasdesk (ProgID, иконка).
Инструкция по Authenticode-подписи в docs/RELEASE.md (сертификат покупает владелец, CI-шаг signtool описать).
Версия из git tag → ресурсы exe.
Критерий: чистая Win11 VM → установка → --desktop работает → деинсталляция не оставляет следов (кроме %APPDATA%).
```

**M4 done → коммит, тег v1.0.**

---

## M5. Движок виджетов (JS/HTML-микрофронтенды)

### T20. Widget runtime (WebView2-хост)

**Промпт:**
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

### T21. Bridge, манифест, permissions

**Промпт:**
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

### T22. Widget SDK и примеры

**Промпт:**
```
- docs/WIDGETS.md: формат пакета, манифест, bridge API (таблица сообщений), permissions, LOD-жизненный цикл, ограничения (airspace, лимиты)
- Шаблон виджета: Vite + TypeScript, билд в статику, npm-скрипт pack → папка для установки; клиентская обёртка над postMessage bridge (типизированный canvasdesk.ts, < 150 строк)
- Два примера: 1) панель задач с сохранением в props (offline), 2) дашборд с fetch наружу (демонстрация permission network)
- Пример "микрофронтенд": собрать один и тот же bundle как standalone-страницу и как виджет — разница только в адаптере инициализации
Критерий: по docs/WIDGETS.md новый виджет собирается и ставится на канвас за 30 минут без чтения исходников хоста.
```

**M5 done → коммит, тег v1.1.**

---

## Шаблон AGENTS.md (положить в корень репозитория)

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

## Оценка сроков с Kimi Code

| Фаза | Задачи | Оценка (календарь, 1 разработчик + Kimi Code) |
|---|---|---|
| 0 + M1 | T0–T6 | 2–3 недели |
| M2 | T7–T10 | 1,5–2 недели |
| M3 | T11–T14 | 2–3 недели |
| M4 | T15–T19 | 3–4 недели |
| M5 | T20–T22 | 2–3 недели |
| **Итого** | T0–T22 | **10–14 недель** |

M4 и M5 независимы друг от друга (оба опираются на M3) — при двух потоках или смене приоритетов M5 можно делать раньше M4. Основной риск-буфер держать на T15/T17: отладка недокументированного поведения Windows на живой системе ИИ не ускоряет.

## Порядок старта

1. Создать репозиторий, положить `docs/SPEC.md`, `docs/RECIPES.md`, этот файл как `docs/TASKS.md`, `AGENTS.md` в корень
2. `rustup default stable`, Windows SDK (через Visual Studio Build Tools)
3. Запустить Kimi Code с T0 → прогнать T0–T6, проверить M1-критерии руками
4. После каждого milestone — ручная приёмка по SPEC §10, только потом следующая фаза
