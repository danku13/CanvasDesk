# T16. Шина системных событий (shell_events) — план (M4)

ЗАДАЧА: модуль `shell_events` в canvas-shell по TASKS T16 / SPEC §7.4 /
RECIPES R8, R11–R13. Обязательные источники изучены координатором до
посева: RECIPES R8 (WTS + гейт), R11 (таблица регистраций шины),
R12 (SHChangeNotify как дополнение к notify — корзина!), R13 (идиома
enum/трамплинов); источники §7 RECIPES — файлы Lively/Seelen —
недоступны в песочнице (GPL/AGPL: код не копируется, механика — только
из RECIPES). Рантайм-приёмка (lock/unlock, корзина, Alt+Tab) — за
владельцем; в песочнице — msvc-кросс-чек типов (win-check) + Linux-тесты
чистого ядра.

## 1. Цель и критерии приёмки

- Скрытое message-only окно (HWND_MESSAGE) на отдельном потоке с
  GetMessageW-циклом — единая точка системных событий (R11).
- События доставляются подписчикам через responder (`Arc<dyn Fn>` +
  EventLoopProxy — устоявшийся паттерн ThumbService/Search/Desktop; см.
  §8.1).
- Регистрации: WTS-сессии (сверка session id, R8), suspend/resume,
  TaskbarCreated, RegisterShellHookWindow, clipboard-листенер с UIPI-
  фильтром, SHChangeNotifyRegister (SHCNRF_ShellLevel, R12).
- Lock/unlock сессии — info-лог с корректным session id.
- Перед сном (PBT_APMSUSPEND) — форс-сейв .canvas (`SceneState::save_now`).
- Удаление файла в корзину из Explorer → brokenLink на карточке < 1 с
  (SHCNE_DELETE через шину → тот же конвейер `on_file_events`, что у T10).
- Атомик `IS_INTERACTIVE_SESSION` — экспорт для T18 (гейт фоновых потоков).
- Окно невидимо и не в Alt+Tab (HWND_MESSAGE — by construction; §9).
- ExplorerStarted, ShellHook, ClipboardUpdated — доставлены до
  подписчика (лог; потребители T17/T18/будущее — вне скоупа T16).

## 2. Что уже есть (инвентарь)

- `canvas-shell/src/desktop/monitor.rs` (T15) — эталонный паттерн
  «сервис-поток + GetMessageW + mpsc-команды + WM_APP_WAKE-будильник +
  handshake thread_id + thread_local responder» — shell_events
  переиспользует его, ОТЛИЧИЕ: у потока теперь ЕСТЬ окно → цикл обязан
  вызывать DispatchMessageW (сообщения окна идут в wndproc), а
  PostThreadMessageW-сообщения hwnd=None при диспатче — безвредный no-op.
- `watcher.rs` (T10) — `FileEvent`, sync_dirs-diff-паттерн, агрегатор
  батчей; `fs_events.rs` (core) — `apply_file_events`, `normalize_path`
  (реюз для путей из pidl), `NodeChange::Broken` — brokenLink уже
  реализован, T16 только доставляет события вторым каналом.
- `main.rs` (T15-E) — `AppEvent::Desktop`-ветка, спавн сервисов в
  `main()` рядом с прочими, `on_file_events` — готовый конвейер
  применения (идемпотентен к дублям: повторный Modify/Rename/Remove
  не порождает новых изменений); `SceneState::save_now()` — форс-сейв;
  `sync_watch_dirs()` — точка зеркалирования SyncFileDirs.
- windows-фичи в workspace Cargo.toml: `Win32_UI_WindowsAndMessaging`,
  `Win32_UI_Shell`, `Win32_System_Com`, `Win32_System_Threading` уже
  включены (T9/T15). Координатор добавил: `Win32_System_RemoteDesktop`
  (WTSRegisterSessionNotification, ProcessIdToSessionId),
  `Win32_System_Power` (RegisterSuspendResumeNotification),
  `Win32_System_DataExchange` (AddClipboardFormatListener).
- Верифицировано по исходникам windows-0.62.2 (registry): сигнатуры и
  константы — SHChangeNotifyRegister/SHChangeNotifyDeregister/
  SHChangeNotification_Lock/Unlock, SHGetPathFromIDListW,
  SHParseDisplayName (все Win32_UI_Shell); WTS_SESSION_*(=1..9) —
  Win32_UI_WindowsAndMessaging; WM_WTSSESSION_CHANGE=689,
  WM_POWERBROADCAST=536, PBT_APMSUSPEND=4, PBT_APMRESUMESUSPEND=7,
  PBT_APMRESUMEAUTOMATIC=18, WM_CLIPBOARDUPDATE=797,
  HWND_MESSAGE=(-3) — Win32_UI_WindowsAndMessaging; SHCNRF_ShellLevel=2,
  SHCNRF_NewDelivery=32768, SHCNE_* — Win32_UI_Shell.
- `win-check/` (вне репо) — воссоздан координатором этой сессии (в
  песочнице T15-зонд не сохранился): `#[path]`-включение реальных
  файлов shell_events → `cargo check --target x86_64-pc-windows-msvc`.

## 3. Архитектура

Новый модуль `canvas-shell/src/shell_events/` — 4 файла, зоны воркеров
не пересекаются (схема T15):

**`mod.rs` (T16-A, кроссплатформенное чистое ядро).** Компилируется и
тестируется на Linux (как desktop/mod.rs):
- `ShellEvent` — события шины наружу:
  - `Session { event: SessionEvent, session_id: u32 }`;
  - `Suspending`, `Resumed { kind: ResumeKind }`;
  - `ExplorerStarted` (TaskbarCreated, R7/R11; потребитель — T17);
  - `ShellHook { code: u32, hwnd: usize }` (HSHELL_* — сырой код, декод
    в T18; hwnd — значение HANDLE);
  - `ClipboardUpdated` (будущее «вставить как ноду», SPEC §7.6);
  - `FileEvents(Vec<canvas_core::FileEvent>)` (SHCNE-мост в T10-конвейер).
- `SessionEvent { Locked, Unlocked, Logon, Logoff, ConsoleConnect,
  ConsoleDisconnect, RemoteConnect, RemoteDisconnect }` +
  `gate_value() -> Option<bool>`: Locked/Logoff/ConsoleDisconnect/
  RemoteDisconnect → Some(false); Unlocked/Logon/ConsoleConnect/
  RemoteConnect → Some(true); (REMOTE_CONTROL не моделируем — §8.2).
- `classify_wts(code: u32, session: u32, own: u32) -> Option<SessionEvent>`
  — только своя сессия (R8: события приходят от ВСЕХ сессий), код →
  SessionEvent, неизвестный код → None.
- `SessionGate` — чистый конечный автомат `interactive: bool` (старт
  true — консольное приложение оптимистично; §8.3), `apply(event)`;
  глобальный атомик `IS_INTERACTIVE_SESSION: AtomicBool` + pub fn
  `is_interactive_session() -> bool` (экспорт для T18; на не-Windows —
  всегда true, атомик не обновляется).
- `classify_power(wparam: u32) -> Option<SuspendKind>`:
  PBT_APMSUSPEND → `SuspendKind::Suspending`; PBT_APMRESUMEAUTOMATIC →
  `Automatic`; PBT_APMRESUMESUSPEND → `User`; прочие PBT_* → None.
- `MsgRouter` — маршрутизация hwnd-сообщений по id: фиксированные
  `wts=689, power=536, clipboard=797` + динамические (заполняются из
  RegisterWindowMessageW в C): `taskbar_created`, `shellhook` и
  приватный `shell_file = WM_APP+0x16` (диапазон WM_APP — приватный,
  не конфликтует с регистрируемыми именами; 0x15 занят монитором T15).
  `route(msg) -> Option<MsgKind>`; инвариант: все id различны.
- `shell_file_change(mask: i32, old: Option<&Path>, new: Option<&Path>)
  -> Option<FileEvent>` — чистый маппинг SHCNE → FileEvent (приоритет
  при нескольких битах: Rename > Delete > Create > Modify; отсутствие
  нужного пути → None; неизвестная маска → None):
  SHCNE_RENAMEITEM|SHCNE_RENAMEFOLDER → Rename(old,new);
  SHCNE_DELETE|SHCNE_RMDIR → Remove; SHCNE_CREATE|SHCNE_MKDIR → Create;
  SHCNE_UPDATEITEM → Modify; SHCNE_UPDATEDIR → Modify(dir) (§8.4).
- Константы-значения (локальные копии WinUser.h/ShlObj.h с комментами —
  модуль не зависит от windows-crate на Linux; cfg(windows)-файлы сверяют
  значения с реальными константами `windows` через debug_assert в
  win-check-сборке — приём T15-A): WM_WTSSESSION_CHANGE, WM_POWERBROADCAST,
  WM_CLIPBOARDUPDATE, WM_APP, WTS_CONSOLE_CONNECT..WTS_SESSION_REMOTE_CONTROL
  (1–9), PBT_*, SHCNE_RENAMEITEM/RENAMEFOLDER/CREATE/DELETE/MKDIR/RMDIR/
  UPDATEITEM/UPDATEDIR, SHCNRF_ShellLevel/SHCNRF_NewDelivery,
  коалессинг `FILE_EVENT_COALESCE_MS = 150`.

**`window.rs` (T16-B, cfg(windows)) — окно-шина + поток.**
- `ShellResponder = Arc<dyn Fn(ShellEvent) + Send + Sync>` (в main —
  замыкание с EventLoopProxy → AppEvent::Shell).
- `ShellCommand { SyncFileDirs(Vec<PathBuf>), Shutdown }` (mpsc;
  `unsafe impl Send` с обоснованием — идиома T15 monitor).
- `ShellEventService::spawn(responder) -> Self` — Builder-спавн (провал
  → warn + деградация: команды теряются молча, R14), handshake
  thread_id, `command()` c WM_APP_WAKE-будильником (тело — try_recv-дрейн
  в цикле; копия идиомы monitor.rs).
- Тело потока: CoInitializeEx(STA) (нужно SHParseDisplayName в C) →
  регистрация класса «CanvasDeskShellEventsWnd» → CreateWindowExW
  (parent=HWND_MESSAGE, стиль WS_OVERLAPPED?—нет: style 0, ex 0; окно
  НЕ должно рисоваться/активироваться) → регистрации regs.rs (C) →
  ROUTER/RESPONDER/OWN_SESSION_ID thread_local → SetTimer коалессинга
  FILE_EVENT_COALESCE_MS → цикл GetMessageW: WM_APP_WAKE → дрен команд
  (SyncFileDirs → C-синк pidl-подписок; Shutdown → PostThreadMessageW
  WM_QUIT себе + снятие подписок/таймера + DestroyWindow), WM_QUIT →
  выход, WM_TIMER → фlush накопленных FileEvents батчем, ПРОЧИЕ →
  DispatchMessageW (в wndproc — единственный unsafe-трамплин, R13).
- `wndproc` (extern "system" static fn): ROUTER.route(msg) → декод по
  MsgKind: WtsSession → classify_wts(wParam.0, lParam.0 как u32 session,
  OWN_SESSION_ID) → Session-событие + обновление атомика
  IS_INTERACTIVE_SESSION (gate_value); Power → classify_power →
  Suspending/Resumed; TaskbarCreated → ExplorerStarted; ShellHook →
  ShellHook{code=wParam, hwnd=lParam} (сырой); Clipboard →
  ClipboardUpdated; ShellFile → D-декод → push в накопитель (отправка
  WM_TIMER-флашем — §3 коалессинг; Suspending/Session/пр. — сразу, без
  накопителя). Дефолт → DefWindowProcW.
- Коалессер: thread_local `Vec<FileEvent>`; WM_TIMER-флаш отправляет
  непустой батч `ShellEvent::FileEvents` (зеркало агрегатора T10 —
  сглаживает шквалы Explorer-масс-операций).
- SAFETY на каждый unsafe;wndproc не трогает Rust-ссылки — только
  примитивы + thread_local (схема T15 monitor, R13).

**`regs.rs` (T16-C, cfg(windows)) — регистрации Win32.**
- `struct ShellRegs` — чем владеем (для идемпотентного снятия):
  wts: bool, suspend: Option<HPOWERNOTIFY>, shell_hook: bool,
  clipboard: bool, класс окна/окно — владеет поток; msg-id из
  RegisterWindowMessageW («TaskbarCreated», «SHELLHOOK») → в MsgRouter.
- `register_all(hwnd) -> (ShellRegs, MsgRouter)`:
  - WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_ALL_SESSIONS)
    (R8; false → warn-деградация — живём без сессионных событий);
  - RegisterSuspendResumeNotification(hwnd,
    DEVICE_NOTIFY_WINDOW_HANDLE) → HPOWERNOTIFY (провал → warn);
  - RegisterWindowMessageW(«TaskbarCreated») и («SHELLHOOK») → id
    (провал → 0, MsgRouter-слот неактивен);
  - RegisterShellHookWindow(hwnd) (потребитель T18; провал → warn);
  - AddClipboardFormatListener(hwnd) +
    ChangeWindowMessageFilterEx(WM_CLIPBOARDUPDATE, MSGFLT_ALLOW)
    (UIPI — фильтр ОБЯЗАТЕЛЕН, R11) (провал → warn);
- `unregister_all(hwnd, &mut ShellRegs)` — WTSUnRegisterSessionNotification,
  UnregisterSuspendResumeNotification, DeregisterShellHookWindow,
  RemoveClipboardFormatListener; идемпотентно, ошибки — молча/warn.
- `own_session_id() -> u32` — ProcessIdToSessionId(GetCurrentProcessId)
  (обе — Win32_System_RemoteDesktop/Threading; 0 → все события
  «чужие» → шина живёт, гейт не обновляется — warn).
- SHChangeNotify-подписки (R12): `FileNotifySync` — таблица
  «нормализованный dir → registration id» + diff-sync (зеркало
  watcher.sync_dirs): `sync(&mut self, hwnd, msg, dirs)`:
  добавление — pidl каталога через SHParseDisplayName (COM STA уже
  есть в потоке) → SHChangeNotifyRegister(hwnd, SHCNRF_ShellLevel |
  SHCNRF_NewDelivery, SHCNE_DISKEVENTS, msg, 1, &[entry{pidl,
  fRecursive:false}]) → id; снятие — SHChangeNotifyDeregister(id);
  ошибки — warn, деградация (незарегистрированный dir не в таблице —
  следующий sync ретраит). `clear()` — на Shutdown.
- SHCNE_DISKEVENTS, не ALLEVENTS — §8.5; SHCNRF_NewDelivery — §8.6.

**`shfiles.rs` (T16-D, cfg(windows)) — декод SHChangeNotify-сообщения.**
- `decode(lparam, wparam) -> Option<FileEvent>`:
  1. SHCNRF_NewDelivery: lParam — HANDLE нотификации →
     SHChangeNotification_Lock → (**pppidl, &mut mask**) → Unlock в
     конце ВСЕГДА (Drop-гуард);
  2. mask (i32 из Lock) + пара pidl (old, new) →
     SHGetPathFromIDListW×2 → Option<PathBuf> (SHGetPathFromIDListW
     возвращает false на не-файловых pidl — legitimately None);
  3. чистый `shell_file_change(mask, old, new)` из A — единый маппинг.
- `pidl_to_path(pidl) -> Option<PathBuf>` — обёртка (MAX_PATH-буфер,
  wide→String);
- SAFETY: Lock возвращает сырые двойные указатели — читаем, копируем в
  PathBuf, Unlock; pidl НЕ освобождаем (владение у shell, Lock/Unlock —
  окно доступа); handle из lParam валиден только в течение wndproc —
  не сохраняем.

**Интеграция — T16-E (координатор, main.rs).**
- `AppEvent::Shell(canvas_shell::shell_events::ShellEvent)` —
  cfg(windows)-вариант (симметрия Desktop).
- `main()` (cfg(windows)): спавн ShellEventService БЕЗ привязки к
  --desktop (события сессии/сна/shell-файлы полезны в любом режиме;
  §8.7); responder → proxy.send_event(AppEvent::Shell). Провал спавна —
  warn, приложение живёт (R14).
- `App` поля (cfg(windows)): `shell_events: Option<ShellEventService>`,
  `session_interactive: bool` (последнее известное — лог/отладка/T18);
- `sync_watch_dirs()` — зеркалирование: watcher.sync_dirs + (windows)
  shell_events.command(SyncFileDirs(dirs)) — ЕДИНАЯ точка (все 5
  существующих вызовов остаются как есть).
- `on_shell_event(event)`:
  - Session → session_interactive = gate_value; info-лог
    («сессия locked/unlocked, id=N» — критерий TASKS);
  - Suspending → self.scene.save_now() (форс-сейв до сна — критерий);
    лог; бэкап уже внутри save_with_backup (SPEC §9);
  - Resumed → request_redraw (прогрев кадра после сна);
  - ExplorerStarted → info-лог («Explorer перезапущен»); потребитель —
    T17 (PID Shell_TrayWnd, анти-флуд) — НЕ реализуем в T16;
  - ShellHook/ClipboardUpdated → debug-лог (потребители T18/будущее);
  - FileEvents(batch) → self.on_file_events(batch) — конвейер T10
    (brokenLink по SHCNE_DELETE корзины; идемпотентен к дублям notify —
    повторное применение события к уже обновлённой модели = 0 изменений).
- Цикл: `AppEvent::Shell(event) => self.on_shell_event(event)`.

## 4. Пошаговый план

Контракты (публичные сигнатуры + доки) засеяны координатором в 4 файла
`shell_events/*.rs`, lib.rs (`pub mod shell_events;`), Cargo.toml
(3 новые фичи) и win-check (подключение mod.rs + фичи). Воркеры
реализуют тела и тесты, НЕ меняя публичных сигнатур, lib.rs, Cargo.toml.

1. **T16-A** (`shell_events/mod.rs`, только этот файл): чистое ядро §3 —
   ShellEvent/SessionEvent/classify_wts/SessionGate+атомик,
   classify_power, MsgRouter, shell_file_change, константы. Тесты
   (Linux): classify_wts (все 9 кодов, своя/чужая сессия, unknown),
   gate_value-таблица, SessionGate-переходы (старт true; lock→false;
   unlock→true; logoff→false; повторные), classify_power (3 PBT +
   посторонний), MsgRouter (route всех 6, коллизия id → паника-инвариант
   конструктора, неизвестный → None), shell_file_change (rename с двумя
   путями; delete/create/update по одному; rename без new → None;
   несколько бит → приоритет Rename>Delete>Create>Modify; неизвестная
   маска → None; RENAMEFOLDER/RMDIR/MKDIR/UPDATEDIR ветки), константы
   (сверка значений с эталонами WinUser.h, битовая непересекаемость
   SHCNE), is_interactive_session стартует true.
2. **T16-B** (`shell_events/window.rs`): окно+поток по §3 — паттерн
   monitor.rs + DispatchMessageW + wndproc + коалессер + команды.
   Верификация: win-check msvc-кросс-чек; debug_assert констант A.
3. **T16-C** (`shell_events/regs.rs`): регистрации по §3 — все 6
   регистраций + снятие + own_session_id + FileNotifySync-diff.
   Верификация: win-check; debug_assert констант.
4. **T16-D** (`shell_events/shfiles.rs`): декод по §3 — Lock/Unlock с
   Drop-гуардом, pidl→path, маппинг из A. Верификация: win-check.
5. **T16-E (координатор):** интеграция main.rs по §3, гейты (fmt/clippy/
   test на Linux workspace; win-check; CI windows-latest authoritative),
   §8, коммиты `docs(plans): T16 — шина системных событий; посев
   контрактов shell_events (4 зоны воркеров)` и `feat(shell,app): T16 —
   шина системных событий (HWND_MESSAGE-окно, WTS+гейт, suspend/resume,
   TaskbarCreated, shellhook, clipboard, SHChangeNotify-мост в T10)`.

## 5. Чек-лист RECIPES (приёмка кода, обязательный)

- [ ] Message-only окно: parent=HWND_MESSAGE (не WS_VISIBLE), не в
      Alt+Tab, EnumWindows его не видит (R11).
- [ ] Поток шины — владелец окна; GetMessageW + DispatchMessageW
      (сообщения окна → wndproc; потоковые — напрямую).
- [ ] WTSRegisterSessionNotification(NOTIFY_FOR_ALL_SESSIONS) — события
      от ВСЕХ сессий; сверка l_param session id со своей (R8) —
      classify_wts(session, own).
- [ ] Атомик IS_INTERACTIVE_SESSION обновляется ТОЛЬКО своей сессией
      (R8); экспорт для T18.
- [ ] RegisterSuspendResumeNotification: PBT_APMSUSPEND → форс-сейв
      .canvas ДО сна (TASKS T16; save_with_backup — SPEC §9).
- [ ] RegisterWindowMessage(«TaskbarCreated») → ExplorerStarted (R7/R11;
      потребитель T17 — не реализуем).
- [ ] RegisterShellHookWindow + «SHELLHOOK» (R11; потребитель T18).
- [ ] AddClipboardFormatListener + ChangeWindowMessageFilterEx(
      WM_CLIPBOARDUPDATE, MSGFLT_ALLOW) — UIPI-фильтр обязателен (R11).
- [ ] SHChangeNotifyRegister(SHCNRF_ShellLevel|NewDelivery,
      SHCNE_DISKEVENTS) на директории нод; дифф-синк зеркалом T10
      sync_dirs; корзина → SHCNE_DELETE → brokenLink (R12).
- [ ] Enum-обёртки/трамплины — идиома R13 (SAFETY, unsafe impl Send с
      обоснованием, safe API наружу).
- [ ] Деградация: любой отказ регистрации — warn, шина продолжает
      работать на остальных каналах (R14); краш потока шины не роняет
      приложение.

## 6. Сводка тестов

- canvas-shell (Linux): `cargo test -p canvas-shell` — ядро A ≈ 15–20
  тестов (классификации, гейт, роутер, маппинг, константы).
- Кросс-типы (B/C/D): win-check `cargo check --target
  x86_64-pc-windows-msvc` — обязательный локальный гейт каждого воркера.
- Рантайм-поведение (lock/unlock с session id, сон, корзина, Alt+Tab,
  перезапуск Explorer) — ручная приёмка владельца (§9); CI
  windows-latest — сборка и компиляция.

## 7. Риски, ограничения, фолбэки

- **SHChangeNotify-семантика доставки** (NewDelivery vs classic:
  классика: wParam=маска, lParam=указатель pidl-пары; NewDelivery:
  lParam=HANDLE, маска и pidl — из SHChangeNotification_Lock): фиксируем
  NewDelivery (§8.6); если на живой системе событие не декодируется —
  фолбэк-ветка «classic» в decode НЕ закладываем, диагностируем логом
  (маска/пути) на приёмке владельца.
- **Дубли с notify (T10)**: SHCNE-события пересекаются с RDCW-событиями
  вотчера. Применение через on_file_events идемпотентно (повтор = 0
  изменений); коалессер сглаживает шквалы. Отдельный дедуп НЕ строим
  (§8.8).
- **Корзина**: SHCNE_DELETE с путём исходного файла приходит с
  SHCNRF_ShellLevel; notify в этот же момент видит move в $Recycle.Bin
  (R12) — а томapply_file_events Rename в $Recycle.Bin НЕ матчится с
  нодой (путь ноды — исходный) → no-op; SHCNE_DELETE ставит Broken —
  как задумано. Возможен дубль Remove от notify (RDCW видел удаление
  источника) — идемпотентен.
- **COM STA на потоке шины**: SHParseDisplayName требует инициализации
  COM; CoInitializeEx(STA) на потоке — до регистраций. Отказ STA
  (уже занято MTA?) — наш поток новый, MTA-наследия нет; провал →
  warn, SHChangeNotify-подписки отключены, остальная шина живёт (R14).
- **Session id = 0** (ProcessIdToSessionId провал): все WTS-события
  фильтруются как «чужие» — гейт не обновляется; warn при старте.
  Крайне маловероятно (функция не падает на живом процессе).
- **Wndproc-реентерабельность**: сообщения шины не порождают
  вложенных циклов (нет модальных вызовов); обращения только к
  thread_local + responder — реентерабельно by construction.
- **Изменение набора директорий во время шквала**: SyncFileDirs
  обрабатывается в WM_APP_WAKE-дрейне — сериализовано с декодом (один
  поток) — гонок нет.

## 8. Отступления и решения (фиксация до старта)

1. **Responder вместо crossbeam-канала** (TASKS T16 упоминает
   crossbeam): в репо устоялся паттерн `Arc<dyn Fn(Event)>` +
   EventLoopProxy (ThumbService T6, watcher T10, search T14, desktop
   T15) — единая точка подписки из приложения достигается тем же;
   новая зависимость без функционального выигрыша нарушает
   AGENTS.md п.9. Фиксация комментарием в mod.rs.
2. **REMOTE_CONTROL (код 9) не моделируется**: гейт не меняет (R8
   трактует lock/unlock как основные), лог-достаточно classify→None.
3. **Гейт стартует true**: WTS-события приходят только на ИЗМЕНЕНИЯ;
   залоченная ДО старта сессия не пришлёт событие до unlock — при
   старте в залоченной сессии (служба автозапуска) шина считает
   сессию интерактивной до первого события; T18 компенсирует своим
   Z-порядком проверки (полный детект — вне скоупа T16).
4. **SHCNE_UPDATEDIR → Modify(dir)**: директории сами не ноды, apply
   даст 0 изменений; событие всё же доставляем (будущее: рефреш
   содержимого папки-ноды).
5. **SHCNE_DISKEVENTS вместо SHCNE_ALLEVENTS** (R12 цитирует
   ALLEVENTS у Seelen для мониторинга корзины): нам нужны только
   дисковые события на директориях нод — ALLEVENTS несёт shell-шум
   (ассоциации, environment); DISKEVENTS = точный суп нашей маски.
6. **SHCNRF_NewDelivery добавлен к SHCNRF_ShellLevel**: lParam=HANDLE
   вместо сырого указателя — безопаснее и верифицируемо через
   SHChangeNotification_Lock (windows-rs сигнатуры подтверждены).
7. **Спавн шины без --desktop**: события сна/сессии/корзины полезны в
   оконном режиме; M4-потребители (T17/T18) подключатся к той же шине.
8. **Без отдельного дедупа notify↔SHCNE**: идемпотентность apply +
   коалессер 150 мс; отдельная таблица дедупа усложнит T10 без
   пользовательски видимого выигрыша.
9. **WM_APP+0x16 для shell_file**: RegisterWindowMessageW глобален —
   одноимённая регистрация чужим процессом не должна ломать нас;
   WM_APP-диапазон приватен и не требует уникальных имён.

## 9. Чек-лист ручной приёмки (владелец, Win11 25H2 build 26200)

1. Запуск (окно и --desktop): в логе — «shell-шина: окно создано,
   регистрации OK (wts/power/taskbar/shellhook/clipboard), session id=N».
2. Win+L → лог «сессия locked, id=N» (N = своему); ввод пароля →
   «сессия unlocked, id=N» — гейт IS_INTERACTIVE_SESSION переключился.
3. Alt+Tab — окна CanvasDesk ShellEvents нет; Spy++ — окно только как
   message-only child of HWND_MESSAGE.
4. Прогнать `powercfg /requests` или просто сон/пробуждение: лог
   «suspending» → .canvas сохранён (mtime файла), «resumed» → кадр.
5. Картина канваса с файловой карточкой; удалить файл в Explorer
   (Delete → корзина) → карточка brokenLink < 1 с; восстановить из
   корзины → brokenLink снят.
6. Массовое выделение 20 файлов в наблюдаемой папке → перенос в
   подпапку из Explorer → ноды обновили пути (SHCNE_RENAMEITEM-мост),
   без фриза интерфейса (коалессер).
7. kill explorer.exe → автозапуск: в логе ExplorerStarted (TaskbarCreated).
8. Ctrl+C в буфере путь/текст → debug-лог ClipboardUpdated (без
   действий — задел «вставить как ноду»).
9. Гонка включена: cargo clippy/test/win-check зелёные (CI).

## 10. Маппинг файлов → воркеры (зоны)

| Файл | Воркер |
|---|---|
| `crates/canvas-shell/src/shell_events/mod.rs` | T16-A |
| `crates/canvas-shell/src/shell_events/window.rs` | T16-B |
| `crates/canvas-shell/src/shell_events/regs.rs` | T16-C |
| `crates/canvas-shell/src/shell_events/shfiles.rs` | T16-D |
| `crates/canvas-shell/src/lib.rs`, `Cargo.toml`, `win-check/*` | координатор (посев, заморожены) |
| `crates/canvas-app/src/main.rs` | T16-E (координатор) |
