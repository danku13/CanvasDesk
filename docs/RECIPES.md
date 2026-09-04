# Deep dive: методы Lively Wallpaper и Seelen UI как рецепты для CanvasDesk

Анализ актуальных кодовых баз (коммиты на сентябрь 2026). Положить в репозиторий как `docs/RECIPES.md` рядом с `SPEC.md` и `TASKS.md`.

---

## 0. Лицензионная рамка — прочитать первым

| Проект | Лицензия | Что это значит для CanvasDesk |
|---|---|---|
| Lively Wallpaper | **GPL-3.0** | Копирование кода → весь CanvasDesk обязан стать GPL |
| Seelen UI | **AGPL-3.0** | Ещё жёстче: copyleft срабатывает даже при сетевом использовании |

**Вывод: переиспользуем методы, а не код.** Всё нижелоеописанное — техники Win32, которые не являются чьей-либо интеллектуальной собственностью (Microsoft публично описала raised desktop; сообщение 0x052C известно с 2013 года). Правило для агента: чистая реализация по описанию механики, без копирования идентификаторов, структуры и комментариев из исходников. Seelen UI при этом остаётся ценным как **доказательство, что весь стек реализуем на windows-rs** — те же API, те же типы (`HWND`, `SetWindowLongPtrW`, `FindWindowExA`).

## 1. Карта покрытия

| Область | Lively (C#) | Seelen UI (Rust) | Куда в CanvasDesk |
|---|---|---|---|
| Детект иерархии десктопа | ✅ два случая | ✅ два случая | T15 |
| Встройка в raised desktop (24H2/25H2) | ✅ продакшен | ⚠️ упрощённо (см. R3) | T15 |
| Скрытие иконок | ✅ идемпотентное | — | T16 |
| Watch на гибель WorkerW | ✅ WinEventHook | — | T15 |
| Детект краша Explorer | ✅ с анти-флудом | — | T16 |
| Session lock/unlock | ✅ | ✅ (WTS + гейтинг потоков) | T16 |
| Пауза рендера (энергия) | ✅ 4 алгоритма | ✅ IS_INTERACTIVE_SESSION | новая задача |
| Фоновое окно событий | частично | ✅ эталон на Rust | новая задача |
| Shell-нотификации (корзина!) | — | ✅ SHChangeNotifyRegister | T10 |
| DPI после репарентинга | ✅ (костыль для WebView2) | ✅ (polling) | T15 |

---

## 2. Рецепты встройки в десктоп

### R1. Детект иерархии: два независимых источника сходятся

Оба проекта документируют иерархию Spy++-дампами прямо в коде — использовать их как тестовые фикстуры:

```
Классическая (Win10 / Win11 ≤ 23H2):          Raised desktop (Win11 24H2 / 25H2):
WorkerW                                        Progman
  SHELLDLL_DefView                               SHELLDLL_DefView   <- иконки (WS_EX_LAYERED)
    FolderView (SysListView32)                     FolderView
WorkerW   <- целевой, top-level                  WorkerW           <- целевой, child of Progman
Progman                                        (Progman имеет WS_EX_NOREDIRECTIONBITMAP)
```

**Метод детекта (объединение обоих подходов, для T15):**
1. `is_raised = GetWindowLongPtrW(progman, GWL_EXSTYLE) & WS_EX_NOREDIRECTIONBITMAP != 0` — один вызов, надёжный маркер (Lively).
2. Если не raised: классический обход — `EnumWindows`, у top-level окна ищем child `SHELLDLL_DefView`, целевой `WorkerW` — следующий sibling (`FindWindowEx(0, tophandle, "WorkerW", 0)`).
3. Если raised: `WorkerW = FindWindowEx(progman, 0, "WorkerW", 0)` — прямой ребёнок Progman. **С retry:** Seelen делает до 10 попыток с паузой 100 мс — после сообщения 0x052C окно появляется не мгновенно.

Источники: `Lively.Common/Helpers/Shell/DesktopUtil.cs`, `Lively/Core/WinDesktopCore.cs → SetupDesktopLayer()`, `Seelen-UI src/background/widgets/wallpaper_manager/mod.rs → detect_worker_w()`.

### R2. Встройка в raised desktop — полная механика (ядро T15)

Это самый ценный блок Lively — единственная открытая продакшен-реализация под 24H2+. В `WinDesktopCore.cs` есть дословный комментарий Microsoft о механике raised desktop (причина изменения — HDR-обои). Порядок операций из `TryAttachToDesktop()`:

```
1. Установить WS_CHILD (убрать попутно лишнее — см. R3)
2. Установить WS_EX_LAYERED и SetLayeredWindowAttributes(bAlpha = 255)
   ⚠️ СТРОГО ДО SetParent — Lively фиксирует баг: Godot не может
   применить WS_EX_LAYERED после репарентинга. Причина: у WorkerW/Progman
   WS_EX_NOREDIRECTIONBITMAP, layered-стили на детях "silently dropped".
   Полная непрозрачность (bAlpha=255) — требование Microsoft: иначе
   невозможен эффективный DX blt present.
3. SetParent(hwnd, progman)          // parent = Progman, НЕ WorkerW
4. SetWindowPos(hwnd, hWndInsertAfter = shellDLL_DefView, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE)
   // встаём в Z-order сразу ПОД DefView (иконки поверх нас)
5. EnsureWorkerWZOrder():
   // WorkerW обязан оставаться последним ребёнком Progman (под нами).
   // Если GetWindow(progman, GW_CHILD) → GW_HWNDLAST != workerW →
   // SetWindowPos(workerW, HWND_BOTTOM, NOACTIVATE|NOMOVE|NOSIZE)
   // Lively логирует это как "Unexpected WorkerW Z-order" — случай реальный.
```

Симметрично: при получении события разрушения WorkerW (R6) на raised desktop достаточно перевыполнить шаги 4–5, полный reset не нужен. На классической схеме — полный reset (SetParent на новый WorkerW).

Источник: `WinDesktopCore.cs → TryAttachToDesktop()`, `EnsureWorkerWZOrder()`.

### R3. Стиль-скраббинг и порядок инициализации окна (Seelen)

Перед `SetParent` Seelen принудительно нормализует стили (`try_set_under_desktop_items()`):

```
style   |= WS_CHILDWINDOW
style   &= !WS_CLIPSIBLINGS
exstyle &= !(WS_EX_ACCEPTFILES | WS_EX_APPWINDOW | WS_EX_WINDOWEDGE)
```

Зачем каждый пункт: `WS_EX_APPWINDOW` — чтобы окно не появлялось в Alt+Tab и таскбаре; `WS_EX_WINDOWEDGE` — при его наличии окно **исчезает из WorkerW после SetParent** (зафиксированный баг); `WS_EX_ACCEPTFILES` — чтобы дроп не перехватывался на уровне shell до нашей логики (для CanvasDesk важно: у нас свой `IDropTarget`, T9).

**Урок про библиотеки окон (применимо к winit напрямую):** Seelen использует tao (форк winit для Tauri), и tao асинхронно восстанавливает стили через `SetWindowLongW(GWL_STYLE)` без `WS_CHILD` — «не зная» о репарентинге. Отсюда правило для T15: **сначала полностью настроить окно через API библиотеки, затем наш репарентинг последним шагом, и после него — верификация:** перечитать `GWL_STYLE`/`GWL_EXSTYLE` и сравнить с ожидаемым; любую последующую смену стилей библиотекой (resize, fullscreen toggle) перехватывать и повторять скраббинг.

Источники: `mod.rs → try_set_under_desktop_items()`, комментарий в `src/ui/svelte/wallpaper-manager/index.ts`.

### R4. Идемпотентный спавн WorkerW (Seelen)

Критичная деталь, которой нет в старых туториалах: **сообщение 0x052C (WPARAM=0xD, LPARAM=0x1) слать только если WorkerW отсутствует.** Если raised WorkerW уже существует, повторная отправка заставляет Explorer снести и пересоздать его → наше окно-ребёнок уничтожается → remount → снова 0x052C → бесконечный цикл create/destroy. Seelen на этом поймал реальный баг. Алгоритм: `detect_worker_w()` → если `None` → `PostMessage(progman, 0x052C, 0xD, 1)` → повторный детект с retry из R1.

Источник: `mod.rs → try_set_under_desktop_items()` (комментарий к setup_desktop_layer).

### R5. Идемпотентное скрытие иконок (Lively)

```
чтение:  SHGetSetSettings(SHELLSTATE, SSF_HIDEICONS, fGet=TRUE) → fHideIcons
запись:  if (fHideIcons XOR хотим_скрыть):
             SendMessage(DefView, WM_COMMAND, 0x7402, 0)   // toggle
```

Команда 0x7402 — **переключатель**, а не установка. Поэтому сначала читаем состояние и шлём toggle только при расхождении — иначе при повторном запуске иконки включатся вместо выключения. `SHGetSetSettings` с `fSet=TRUE` в Windows 10+ не работает — не тратить время (Lively проверил). Для T16: сохраняем исходное состояние при старте, восстанавливаем при выходе; краш-сейф из TASKS.md дополняет это.

Источник: `DesktopUtil.cs → GetDesktopIconVisibility()/SetDesktopIconVisibility()`.

### R6. Watch на разрушение WorkerW (Lively)

Вместо поллинга — **WinEventHook на поток Explorer**: `SetWinEventHook(EVENT_OBJECT_DESTROY, ..., pid/tid потока WorkerW)`. Событие приходит точно в момент гибели окна (Explorer решил перестроить иерархию — смена обоев, настроек, иногда просто так). Реакция различается по схеме: raised → повторный детект + Z-order (R2 шаги 4–5); классическая → полный re-attach.

Для Rust: `windows::Win32::UI::Accessibility::SetWinEventHook` + `WINEVENT_OUTOFCONTEXT` (колбэк в нашем процессе, без инжекта DLL). Поллинг Progman из TASKS.md T15 оставить как резервный канал, основной — hook.

Источник: `WinDesktopCore.cs → конструктор (workerWHook), WorkerWHook_EventReceived()`.

### R7. Детект краша Explorer с анти-флудом (Lively)

Три элемента, копировать связкой:
1. Подписка на `RegisterWindowMessage("TaskbarCreated")` — Explorer рассылает при каждом старте таскбара.
2. Отличие краша от DPI-смены (которая тоже шлёт TaskbarCreated): сравнение PID процесса окна `Shell_TrayWnd` до/после. PID сменился → краш/перезапуск.
3. Анти-флуд: если перезапусков > 1 за ~30 с — не реагировать автоматикой, показать ошибку и остановиться (у Lively `TaskbarCrashTimeOutDelay`). Иначе crash-loop Explorer утащит нас в бесконечный re-attach.

Источник: `WinDesktopCore.cs → WndProc_TaskbarCreated(), GetTaskbarExplorerPid()`.

### R8. Session lock/unlock/switch

- **Lively (SystemEvents.SessionSwitch):** после unlock проверить `IsWindow(workerW)` — handle мог умереть за время локскрина; если невалиден или обои «crashed after unlock» → reset с задержкой 1 с.
- **Seelen (Rust-эталон):** `WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_ALL_SESSIONS)` → `WM_WTSSESSION_CHANGE` с `WTS_SESSION_LOCK/UNLOCK`, **обязательно сверять `l_param` session id со своей сессией** (события приходят от всех сессий). На основе этого — глобальный атомик `IS_INTERACTIVE_SESSION`: неинтерактивная сессия → **пауза всех фоновых потоков** (у них — вебвью, у нас — рендер, тамбнейл-пул, вотчер, preview host).

Для T16: связка WTS + атомик-гейт — это заодно основа энергосбережения (R14).

Источники: `WinDesktopCore.cs → SystemEvents_SessionSwitch()`; `Seelen-UI src/background/windows_api/event_window.rs`.

### R9. Ловушка RefreshDesktop (оба споткнулись)

Классический способ «почистить» десктоп — `SystemParametersInfo(SPI_SETDESKWALLPAPER, ..., NULL)`. **На raised desktop это разрушает WorkerW**, на MSIX-сборке — заставляет shell перестроить всю иерархию и выкидывает наше окно из parent. Оба проекта в итоге: на raised desktop не вызывать вообще (Lively — early return; Seelen — log-and-swallow). Для очистки артефактов достаточно `InvalidateRect(DefView) + UpdateWindow(DefView)` (метод Seelen `refresh_desktop()`).

Источники: `WinDesktopCore.cs → RefreshDesktop()`; `handlers.rs` (комментарий про MSIX).

### R10. DPI после репарентинга (оба)

Дочернее окно WorkerW/Progman **не получает корректный DPI и события его смены**: Lively принудительно считывает scale целевого монитора и передаёт в WebView2 («when running as child of WorkerW/Progman, the WebView2 surface does not pick up the correct DPI»); Seelen обнаружил, что `onScaleChanged` после репарентинга не стреляет, и поллит `devicePixelRatio` каждые 500 мс.

Рецепт для wgpu/winit (T15): после репарентинга не доверять `window.scale_factor()`; вычислять масштаб сами — `GetDpiForSystem` или `GetDpiForWindow(progman)` + подписка на `WM_DPICHANGED` не сработает → **поллинг `GetDpiForWindow` раз в 500–1000 мс или по событию display change**; при смене — пересоздание surface и пересчёт текстовых размеров (см. SPEC §6.5).

Источники: `Lively/Factories/WallpaperPluginFactory.cs` (комментарий); `index.ts → lookupDPI()`.

---

## 3. Инфраструктурные рецепты на Rust (Seelen UI)

### R11. Скрытое окно системных событий — эталон для canvas-shell

Seelen держит невидимое окно на отдельном потоке с собственным `GetMessageW`-циклом исключительно как **шину системных событий**. Одна точка подписки, события рассылаются подписчикам через channel (`event_manager!` на crossbeam). Что регистрируется на этом окне — прямой список для нашего crate `canvas-shell`:

| Регистрация | События | Зачем CanvasDesk |
|---|---|---|
| `RegisterShellHookWindow` + `RegisterWindowMessage("SHELLHOOK")` | shell-события окон | контекст для pause-логики |
| `RegisterSuspendResumeNotification` | sleep/resume | корректный сейв .canvas перед сном |
| `WTSRegisterSessionNotification` | lock/unlock/switch | R8, энергосбережение |
| `AddClipboardFormatListener` + `ChangeWindowMessageFilterEx(WM_CLIPBOARDUPDATE, MSGFLT_ALLOW)` | буфер обмена | будущее «вставить файл как ноду»; **важен сам приём:** если процесс когда-либо elevated, UIPI молча блокирует сообщения — фильтр обязателен |
| `SHChangeNotifyRegister` | shell file events | см. R12 |
| `RegisterWindowMessage("TaskbarCreated")` (добавить нам) | краш Explorer | R7 |

**Решение для CanvasDesk:** добавить в `canvas-shell` модуль `shell_events` по этому образцу — скрытое message-only окно (`HWND_MESSAGE` как parent) + поток + crossbeam-канал в приложение. Это дешёвый, проверенный в продакшене способ получать всё системное в одном месте, не размазывая Win32-колбэки по коду.

Источник: `src/background/windows_api/event_window.rs` (целиком образцовый).

### R12. SHChangeNotifyRegister как дополнение к notify (T10)

Seelen отслеживает корзину через `SHChangeNotifyRegister(SHCNRF_ShellLevel, SHCNE_ALLEVENTS)`. Почему это важно для нашего вотчера: **удаление файла в корзину — это не Delete для ReadDirectoryChangesW** (это move в системную папку `$Recycle.Bin`), и notify поведёт себя неочевидно. SHChangeNotify даёт события уровня shell (`SHCNE_DELETE`, `SHCNE_RENAMEITEM`, `SHCNE_UPDATEITEM`) — включая операции, сделанные через Explorer, с уже нормализованными путями.

Решение: в T10 оставить `notify` для Modify/Create (быстрее, детальнее), а Rename/Delete дополнить подпиской SHChangeNotify на директории нод — особенно сценарий «пользователь удалил файл в корзину через Explorer» → корректный `brokenLink` по SPEC §7.5.

Источник: `event_window.rs → register_shell_notifications()`.

### R13. Паттерн безопасных enum-обёрток (копировать как идиому)

`WindowEnumerator`/`MonitorEnumerator` — канонический безопасный паттерн передачи Rust-замыкания в Win32 enum-колбэк: boxed closure, указатель через `LPARAM`, статическая `extern "system" fn` трамплин, `unsafe impl Send/Sync` с обоснованием. Плюс рекурсивный `for_each_and_descendants`. Это готовая идиома для всего нашего `canvas-shell` (EnumWindows, EnumChildWindows, EnumDisplayMonitors) — чистый safe API поверх unsafe-вызовов, по одному месту unsafe на семейство операций.

Источник: `src/background/windows_api/iterator.rs`.

### R14. Graceful degradation при сбое встройки

`handlers.rs → set_as_wallpaper()`: если attach в иерархию десктопа не удался — не паника и не отказ, а **фолбэк на абсолютное позиционирование** (обычное окно на весь виртуальный экран) + warn в лог. Это ровно наш принцип «неизвестная версия → оконный режим» из SPEC §7.4, но Seelen показывает, что фолбэк нужен на **каждом шаге**, а не только на детекте версии: любой `SetParent`/`SetWindowPos` может вернуть ошибку на конкретной машине (антивирус, кастомный shell, RDP).

---

## 4. Энергосбережение и пауза рендера (Lively Playback)

Для обоев это вопрос батареи; для нас — тоже: канвас в режиме M4 рендерит постоянно, и жечь GPU, когда десктоп не виден, недопустимо. Методы из `Lively/Core/Suspend/Playback.cs`:

### R15. Таймер вместо WinEventHook для оконного трекинга

Lively осознанно использует **таймер (настраиваемый интервал, ~1 с) вместо `EVENT_OBJECT_LOCATIONCHANGE`** — у хука слишком много шума даже с фильтрами, на части систем он ненадёжен. Тот же принцип для нашей pause-логики: тик раз в секунду → оценка → смена состояния. Дёшево и предсказуемо.

### R16. Иерархия условий паузы (приоритет сверху вниз)

```
1. Системное состояние (пауза всегда):
   - сессия залочена (R8) или RDP-сессия (RemoteDesktopPause)
   - батарея: GetSystemPowerStatus → ACLineStatus offline (опция)
   - Battery Saver включён (опция)
2. Полноэкранное 3D-приложение:
     SHQueryUserNotificationState() == QUNS_RUNNING_D3D_FULL_SCREEN
   (game mode — один дешёвый вызов, не нужен анализ окон)
3. Покрытие десктопа окнами (алгоритм "foreground"):
   - hwnd = GetForegroundWindow; если это Progman/WorkerW/GetShellWindow → десктоп виден → рендерим
   - иначе: окно покрывает рабочую область своего монитора → пауза
   - исключения по классам окон: "WorkerW", "Progman", "Windows.UI.Core.CoreWindow"
     (старт/тасквью/центр уведомлений считаются "десктопом") — список в WindowClassExclusions.cs
4. (Опционально, алгоритм "all") видимые top-level окна, сгруппированные по мониторам;
   покрытие считается пересечением rect'ов; есть и grid-вариант с порогом покрытия в %
```

Для CanvasDesk: состояние «пауза» = остановка рендер-цикла (request_redraw не планируется), заморозка тамбнейл-пула и preview host, снятие живых превью. Просыпание — по следующему тику с «десктоп виден». Достаточно пунктов 1–3; grid-алгоритм — overkill.

### R17. IsDesktop() — проверка «мы на переднем плане»

`GetForegroundWindow()` и сравнение с `progman` и **оригинальным** WorkerW (тем, что содержит DefView — кэшируется при инициализации, `original_WorkerW`). Используется для input forwarding у Lively; нам — для хоткеев в M4: перехватывать Ctrl+F и прочее только когда foreground — десктоп, иначе отдать системе (SPEC §9, «конфликт хоткеев»).

---

## 5. Маппинг на план разработки

| Рецепт | Задача | Действие |
|---|---|---|
| R1–R4, R10, R14 | T15 | Влить в промпт T15 как чек-лист приёмки (см. §6) |
| R5, R7, R8, R9 | T16 | То же для T16 |
| R6 (WinEventHook) | T15 | Заменить «поллинг 2с» на hook + поллинг-резерв |
| R11 (event window) | **новая T15b** | Модуль `shell_events` в canvas-shell: скрытое окно + канал |
| R12 (SHChangeNotify) | T10 | Дополнить вотчер shell-событиями (корзина!) |
| R13 (enum-идиома) | T15 | Идиома для всего canvas-shell, в AGENTS.md |
| R15–R17 (пауза) | **новая T16b** | Энергосбережение: тик 1с, гейт рендера и пулов |

**Рекомендация:** добавить в SPEC §7.4 ссылку на этот документ и две новые задачи (T15b, T16b) в план — обе маленькие (2–3 дня с Kimi Code каждая), но закрывают классы багов, которые иначе всплывут на приёмке M4.

## 6. Дополнение к промптам (готово к вставке в TASKS.md)

В T15 добавить чек-лист:
```
Реализация по docs/RECIPES.md R1–R4, R10:
- детект raised desktop через WS_EX_NOREDIRECTIONBITMAP на Progman
- 0x052C (WPARAM=0xD, LPARAM=0x1) слать ТОЛЬКО если WorkerW отсутствует
- детект WorkerW с retry 10×100мс
- WS_EX_LAYERED + SetLayeredWindowAttributes(255) СТРОГО ДО SetParent
- стиль-скраббинг: +WS_CHILDWINDOW, -WS_CLIPSIBLINGS, -WS_EX_APPWINDOW,
  -WS_EX_WINDOWEDGE, -WS_EX_ACCEPTFILES; верификация стилей ПОСЛЕ репарентинга
- Z-order: SetWindowPos(hwnd, DefView как insert-after), затем EnsureWorkerWZOrder
  (WorkerW обязан быть последним ребёнком Progman)
- WinEventHook EVENT_OBJECT_DESTROY на поток WorkerW (+поллинг резерв)
- DPI: после репарентинга не доверять scale_factor окна; поллинг GetDpiForWindow
- фолбэк на обычное окно при ЛЮБОЙ ошибке шага (не только при неизвестной версии)
```

В T16 добавить:
```
- скрытие иконок идемпотентно: SHGetSetSettings(SSF_HIDEICONS) read → toggle 0x7402
  только при расхождении состояний (RECIPES R5)
- НЕ использовать SPI_SETDESKWALLPAPER на raised desktop (RECIPES R9); для рефреша —
  InvalidateRect+UpdateWindow на DefView
- детект краша Explorer: RegisterWindowMessage("TaskbarCreated") + сравнение PID
  Shell_TrayWnd + анти-флуд 30с (RECIPES R7)
- WTSRegisterSessionNotification: lock/unlock со сверкой session id; гейт
  IS_INTERACTIVE_SESSION для фоновых потоков (RECIPES R8)
```

## 7. Источники (файлы для чтения агентом перед T15/T16)

| Файл | Что брать |
|---|---|
| `lively/src/Lively/Lively.Common/Helpers/Shell/DesktopUtil.cs` | Детект WorkerW/DefView, иконки |
| `lively/src/Lively/Lively/Core/WinDesktopCore.cs` | SetupDesktopLayer, TryAttachToDesktop, EnsureWorkerWZOrder, WorkerWHook, TaskbarCreated, SessionSwitch, комментарий Microsoft про raised desktop |
| `lively/src/Lively/Lively/Core/Suspend/Playback.cs` | Алгоритмы паузы, gamemode, системные условия |
| `lively/src/Lively/Lively.Common/WindowClassExclusions.cs` | Список классов «это десктоп» |
| `Seelen-UI/src/background/widgets/wallpaper_manager/mod.rs` | detect_worker_w, try_set_under_desktop_items, refresh_desktop |
| `Seelen-UI/src/background/widgets/wallpaper_manager/handlers.rs` | Фолбэк на absolute positioning, MSIX-ловушка |
| `Seelen-UI/src/background/windows_api/event_window.rs` | Шина системных событий (R11) |
| `Seelen-UI/src/background/windows_api/iterator.rs` | Enum-идиома (R13) |
| `Seelen-UI/src/ui/svelte/wallpaper-manager/index.ts` | Порядок инициализации окна, DPI-polling |

Напоминание агенту: читаем как документацию, пишем свой код (GPL-3.0 / AGPL-3.0, §0).
