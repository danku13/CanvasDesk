# T15. Встройка в WorkerW (режим --desktop) — план (M4)

ЗАДАЧА: режим десктопа по SPEC §7.4 / TASKS T15. Обязательные источники
изучены координатором до посева: RECIPES R1–R4, R6, R10, R13, R14; SPEC
§7.4 (таблица стратегий), §6.5 (DPI), §9 (риски); источники §7 RECIPES —
файлы Lively/Seelen недоступны в песочнице (GPL/AGPL: код не копируется,
механика — только из RECIPES). Первичная отладка — на dev-машине владельца
Win11 25H2 build 26200; в песочнице доступен только msvc-кросс-чек типов
(win-check) — рантайм-приёмка за владельцем.

## 1. Цель и критерии приёмки

- Флаг `--desktop`: окно канваса встраивается за иконки и перед обоями.
- Канвас за иконками (DefView поверх нас) и перед обоями (WorkerW ниже нас).
- Перезапуск Explorer → канвас вернулся (WinEventHook + поллинг-резерв).
- Alt+Tab окно не показывает (скраббинг −WS_EX_APPWINDOW).
- Стили после репарентинга верифицированы перечитыванием (R3).
- Любая ошибка любого шага → фолбэк на обычное окно + MessageBox (R14).
- Ручная приёмка (владелец, 26200): чек-лист §9.

## 2. Что уже есть (инвентарь)

- `canvas-shell/src/dragdrop/` — образец cfg(windows)-кода и SAFETY-идиом
  (T9); `service.rs`/`watcher.rs` — паттерн worker-сервиса (mpsc-команды,
  responder через `EventLoopProxy`).
- `win-check/` (вне репо) — зонд кросс-проверки cfg(windows)-файлов через
  `#[path]` на реальные файлы CanvasDesk: `cargo check --target
  x86_64-pc-windows-msvc`. Для T15 win-check подключает `desktop/mod.rs`
  (подмодули резолвятся автоматически) — засеяно координатором.
- `canvas-app/src/main.rs` — `resumed()` создаёт окно (winit attrs +
  `with_drag_and_drop(false)`), затем `dragdrop::install` по raw-window-handle
  (обход отсутствия HWND в публичном API winit 0.30) — тот же приём даёт HWND
  для T15. `parse_args` (CliArgs) — точка для `--desktop`. `AppEvent`-протокол
  сервисов; `ScaleFactorChanged` → `renderer.set_scale_factor` — пере-
  используем для поллинг-DPI.
- winit 0.30: окно создаётся на главном потоке; после чужого SetParent
  событийный цикл продолжает качать сообщения того же HWND — но
  `ScaleFactorChanged` после репарентинга НЕ приходит (R10) → свой поллинг.
- Windows-features в workspace Cargo.toml: добавлены координатором
  `Win32_UI_Hidpi` (GetDpiForWindow), `Win32_UI_Accessibility`
  (SetWinEventHook/UnhookWinEvent) — остальные уже есть (T9).

## 3. Архитектура

Новый модуль `canvas-shell/src/desktop/` — 4 файла, зоны воркеров не
пересекаются:

**`mod.rs` (T15-A, кроссплатформенное чистое ядро).** Типы и логика без
Win32-вызовов (компилируется и тестируется на Linux — canvas-shell
остаётся кроссплатформенным, как watcher):
- `EmbedStrategy { Classic, Raised }` — выбор по рантайм-детекту иерархии,
  НЕ по номеру сборки (SPEC §7.4).
- `DesktopEvent { WorkerWDestroyed, DpiChanged { dpi: u32 } }` — чистые
  события shell-монитора в event loop.
- `ScreenRect { left, top, right, bottom }` (физ. px) + `union_rects()`.
- Константы WS_* / WS_EX_* (локальные копии значений WinUser.h с
  комментариями — модуль не зависит от windows-crate; cfg(windows)-модули
  сверяют значения с реальными константами `windows` через debug_assert в
  win-check-сборке).
- `plan_style_scrub(style, exstyle) -> StylePlan` — Р3-маски:
  `style |= WS_CHILDWINDOW`, `style &= !WS_CLIPSIBLINGS`,
  `exstyle &= !(WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES)`,
  плюс `exstyle |= WS_EX_NOACTIVATE` (до первого клика, TASKS T15);
  для Raised `exstyle |= WS_EX_LAYERED` (R2 шаг 2).
- `verify_styles(style, exstyle, &plan) -> Result<(), StyleMismatch>` —
  перечитанные стили обязаны совпасть с планом ПОСЛЕ репарентинга (R3 —
  библиотеки окон перезаписывают стили асинхронно).
- `recovery_action(attached: Option<EmbedStrategy>) -> RecoveryAction` —
  R2: Raised → `ReZOrder` (только шаги 4–5), Classic → `FullReattach`,
  None → ничего.
- `dpi_to_scale(dpi) -> f64` (= dpi/96); тайминги: `DETECT_RETRIES=10`,
  `DETECT_RETRY_DELAY_MS=100` (R1), `PARENT_POLL_MS=2000` (резерв R6),
  `DPI_POLL_MS=500` (R10: 500–1000 мс).

**`hierarchy.rs` (T15-B, cfg(windows)) — детект иерархии, R1/R4.**
- `find_progman()` — `GetShellWindow` (+ проверка класса "Progman").
- `is_raised(progman)` — `GetWindowLongPtrW(GWL_EXSTYLE) &
  WS_EX_NOREDIRECTIONBITMAP != 0` (маркер Lively, один вызов).
- `detect(progman) -> DesktopHierarchy` — Classic: `EnumWindows` (идиома
  R13: boxed closure через LPARAM, `extern "system"`-трамплин, safe API) —
  top-level окно с child `SHELLDLL_DefView`, целевой WorkerW =
  `FindWindowExW(0, top, "WorkerW", None)` (следующий sibling). Raised:
  `FindWindowExW(progman, 0, "WorkerW", 0)` — прямой ребёнок Progman
  (DefView тоже ребёнок Progman). `DesktopHierarchy { progman, def_view,
  worker_w, strategy }`.
- `ensure_worker_w(progman) -> Result<DesktopHierarchy>` — R4:
  `detect`; WorkerW отсутствует → `PostMessageW(progman, 0x052C, 0xD, 0x1)`
  (слать ТОЛЬКО при отсутствии — иначе цикл пересоздания) → retry-детект
  10×100 мс. Присутствует → без отправки.
- `virtual_screen_rect() -> Option<ScreenRect>` — `EnumDisplayMonitors` +
  `GetMonitorInfoW` (rcMonitor) → union (математика — `union_rects` из A).

**`attach.rs` (T15-C, cfg(windows)) — встройка, R2/R3.**
- `attach(hwnd, &hierarchy, screen) -> Result<AttachOutcome, AttachError>`:
  1. scrub: `plan_style_scrub` → `SetWindowLongPtrW` (GWL_STYLE/GWL_EXSTYLE).
  2. Raised-порядок R2 СТРОГО: `WS_EX_LAYERED` +
     `SetLayeredWindowAttributes(bAlpha=255, 0)` ДО `SetParent`; затем
     `SetParent(hwnd, progman)` (parent = Progman, НЕ WorkerW); затем
     `SetWindowPos(hwnd, hWndInsertAfter=def_view, …NOMOVE|NOSIZE|
     NOACTIVATE)`; затем `ensure_worker_w_z_order` (WorkerW обязан
     остаться ПОСЛЕДНИМ ребёнком Progman: `GetWindow(progman, GW_CHILD)` →
     обход до `GW_HWNDLAST` ≠ worker_w → `SetWindowPos(worker_w,
     HWND_BOTTOM, NOACTIVATE|NOMOVE|NOSIZE)`).
     Classic: `SetParent(hwnd, worker_w)`.
  3. Окно на весь виртуальный экран: `SetWindowPos` с координатами
     `screen` (координаты WorkerW/Progman — экранные).
  4. Верификация: перечитать стили → `verify_styles`; несоответствие →
     `AttachError::StyleMismatch` (фолбэк).
- `refresh_z_order(hwnd, &hierarchy)` — R2-симметрия: перевыполнить шаги
  4–5 (Z-order) без re-parent (восстановление после destroy на Raised).
- `enable_activation(hwnd)` — снять `WS_EX_NOACTIVATE` (первый клик).
- `window_dpi(hwnd) -> u32` — `GetDpiForWindow` (Win32_UI_Hidpi).
- Каждая ошибка шага → `AttachError` (thiserror), фолбэк на уровне E.

**`monitor.rs` (T15-D, cfg(windows)) — watch WorkerW + DPI, R6/R10.**
- `DesktopMonitorService::spawn(responder)` — поток-владелец с собственным
  `GetMessageW`-циклом (WinEventHook с `WINEVENT_OUTOFCONTEXT` доставляет
  колбэки только в поток с message loop). Паттерн — `WatchService` (T10).
- Команды (mpsc): `Watch { progman, worker_w, ours }` — установить
  `SetWinEventHook(EVENT_OBJECT_DESTROY, idProcess=0, idThread=поток
  WorkerW через GetWindowThreadProcessId, WINEVENT_OUTOFCONTEXT)`; повторный
  Watch (после re-attach) → UnhookWinEvent + новый hook; `Shutdown` →
  `PostThreadMessageW(WM_QUIT)`.
- `SetTimer` в том же потоке: DPI_POLL_MS → `window_dpi(ours)`, изменение →
  `responder(DesktopEvent::DpiChanged)`; PARENT_POLL_MS → `IsWindow
  (worker_w)` false → `responder(WorkerWDestroyed)` (резервный канал, R6:
  hook — основной, поллинг — резерв).
- SAFETY-комментарии на каждый unsafe; hook-колбэк только сравнивает hwnd —
  без касания Rust-структур из колбэка кроме отправки события (пересылка
  через канал/атомик, никаких ссылок).

**Интеграция — T15-E (координатор, main.rs).**
- `parse_args`: `--desktop` → `CliArgs.desktop: bool`; на не-Windows —
  warn + обычный режим (деградация, SPEC §9).
- `App` поля (cfg(windows)): `desktop_hierarchy: Option<DesktopHierarchy>`,
  `desktop_monitor: Option<DesktopMonitorService>`, `desktop_attached:
  bool`, `activation_enabled: bool`; `AppEvent::Desktop(DesktopEvent)`
  (responder через `EventLoopProxy` — паттерн прочих сервисов).
- `resumed()` при `--desktop`: attrs = окно без декораций, размер/позиция =
  `virtual_screen_rect()` (PhysicalSize/PhysicalPosition), `with_active
  (false)`, `with_resizable(false)`; после `Renderer::new` и
  `dragdrop::install` (R3-урок: сначала полная настройка через winit,
  репарентинг — последним шагом) → `find_progman → ensure_worker_w →
  attach` по raw-window-handle HWND; успех → `Watch { … }` на монитор +
  info-лог (стратегия); ошибка ЛЮБОГО шага → warn + `MessageBoxW(0, …,
  MB_OK|MB_ICONWARNING)` — «работаем в оконном режиме» — окно остаётся
  top-level borderless (это и есть фолбэк-режим, R14), иерархия None.
- `AppEvent::Desktop`: `WorkerWDestroyed` → `recovery_action(attached)`:
  ReZOrder → `refresh_z_order` + повторный `Watch`; FullReattach →
  повтор `ensure_worker_w` (новый WorkerW) + `attach` + `Watch`; 3
  неудачи подряд → стоп автоматики + warn (анти-флуд упрощённый, полный —
  T17). `DpiChanged { dpi }` → `renderer.set_scale_factor(dpi_to_scale)`
  + `request_redraw` (R10: winit-события после репарентинга не приходят;
  пересоздание surface не требуется — размер HWND не изменился, минимап
  пересоберётся по сигнатуре).
- Первый `MouseInput` в desktop-режиме → `enable_activation(hwnd)` (однократно).
- winit-API окна после attach НЕ трогаем (set_fullscreen/decorations/
  request_inner_size перезапишут стили — R3): размеры только напрямую
  `SetWindowPos` (обёртка в attach.rs не требуется в T15-скоупе).
- Выход: без восстановления (мы ничего не ломали — иконки/обои это T17).

## 4. Пошаговый план

Контракты (публичные сигнатуры + доки) засеяны координатором в 4 файла
`desktop/*.rs`, lib.rs (`pub mod desktop;`), Cargo.toml (features) и
win-check (подключение mod.rs + фичи). Воркеры реализуют тела и тесты, НЕ
меняя публичных сигнатур, lib.rs и Cargo.toml.

1. **T15-A** (`desktop/mod.rs`, только этот файл): чистое ядро §3 — типы,
   union-математика, стиль-план/верификация, recovery, dpi, константы.
   Тесты (Linux): union (пусто → None, один, пересечение, разрозненные,
   дегенерированные min>max), plan_style_scrub (установка/снятие битов по
   всем маскам, идемпотентность, сохранение посторонних битов, Raised/Classic
   различие — layered), verify (совпадение/несовпадение с точным полем),
   recovery_action (3 ветки), dpi_to_scale (96/120/144/192), константы
   (bitflags-инварианты: WS_CHILDWINDOW и WS_CLIPSIBLINGS не пересекаются
   с WS_EX_*-набором).
2. **T15-B** (`desktop/hierarchy.rs`): детект по §3 — EnumWindows-идиома
   R13 (SAFETY, unsafe impl Send/Sync с обоснованием), retry-цикл
   `ensure_worker_w` (sleep — std::thread; количество из констант A),
   virtual_screen_rect. Верификация: `cargo check --target
   x86_64-pc-windows-msvc` в win-check; debug_assert-сверка локальных
   констант A с windows-crate. Windows-only юнит-тестов нет (ручная
   приёмка §9); smoke-тест find_progman на CI — НЕ добавлять (хрупко).
3. **T15-C** (`desktop/attach.rs`): встройка по §3 — порядок R2 (layered
   до SetParent), scrub+verify, обе стратегии, ensure_worker_w_z_order,
   screen-SetWindowPos, enable_activation, window_dpi, AttachError
   (thiserror). Верификация: win-check msvc-кросс-чек; debug_assert
   констант.
4. **T15-D** (`desktop/monitor.rs`): сервис-поток по §3 — WinEventHook +
   SetTimer×2 + GetMessageW-цикл, команды Watch/Shutdown, responder
   событий, SAFETY на каждый unsafe. Верификация: win-check msvc-кросс-чек.
5. **T15-E (координатор):** интеграция main.rs по §3, гейты (fmt/clippy/
   test на Linux workspace; win-check; CI windows-latest authoritative),
   §8, коммит `feat(shell,app): T15 — встройка в WorkerW (--desktop, детект
   иерархии, R2-порядок, WinEventHook, DPI-поллинг, фолбэк)`.

## 5. Чек-лист RECIPES (приёмка кода, обязательный)

- [ ] 0x052C (WPARAM=0xD, LPARAM=0x1) шлётся ТОЛЬКО если WorkerW
      отсутствует (R4 — цикл пересоздания).
- [ ] Детект WorkerW с retry 10×100 мс (R1).
- [ ] WS_EX_LAYERED + SetLayeredWindowAttributes(255) СТРОГО ДО SetParent
      (R2; parent — Progman на Raised, НЕ WorkerW).
- [ ] Стиль-скраббинг R3 (+WS_CHILDWINDOW, −WS_CLIPSIBLINGS,
      −WS_EX_APPWINDOW, −WS_EX_WINDOWEDGE, −WS_EX_ACCEPTFILES) и
      верификация перечитыванием ПОСЛЕ репарентинга.
- [ ] Z-order: insert-after=DefView + EnsureWorkerWZOrder (WorkerW —
      последний ребёнок Progman).
- [ ] WinEventHook EVENT_OBJECT_DESTROY на поток WorkerW
      (WINEVENT_OUTOFCONTEXT) + поллинг Progman 2 с — резерв (R6).
- [ ] DPI: не доверять window.scale_factor() после репарентинга; поллинг
      GetDpiForWindow 500 мс (R10) → set_scale_factor.
- [ ] Фолбэк на обычное окно при ЛЮБОЙ ошибке шага + MessageBox (R14).
- [ ] Окно на весь виртуальный экран (EnumDisplayMonitors), WS_EX_NOACTIVATE
      до первого клика.
- [ ] Alt+Tab: окно не показывается (−WS_EX_APPWINDOW).

## 6. Сводка тестов

- canvas-shell (Linux): `cargo test -p canvas-shell` — ядро A ≈ 12–16
  тестов (union/plan/verify/recovery/dpi/константы).
- Кросс-типы (B/C/D): win-check `cargo check --target
  x86_64-pc-windows-msvc` — обязательный локальный гейт каждого воркера.
- Рантайм-поведение (обе стратегии, Explorer-рестарт, Alt+Tab, DPI-прыжок)
  — ручная приёмка владельца на 26200 (§9); CI windows-latest — сборка и
  тесты компиляции.

## 7. Риски, ограничения, фолбэки

- **Недокументированная зона** (SPEC §11.5): каждое утверждение о
  Progman/WorkerW/DefView — только из RECIPES/SPEC или guarded фолбэком;
  неизвестная иерархия → `HierarchyError::UnknownDesktop` → фолбэк-режим.
- **winit перезаписывает стили** (R3, Seelen/tao-урок): attach — последним
  шагом после всей winit-настройки; после attach winit-окно не трогаем;
  верификация ловит рассинхрон; при StyleMismatch — фолбэк (одно окно
  пере-скраббинг НЕ ретраит в T15 — фиксируем в §8).
- **DPI-компромисс v1**: окно на весь виртуальный экран при пер-мониторных
  DPI: GetDpiForWindow вернёт DPI монитора с максимумом пересечения —
  единый scale на всё окно (winit-модель); текст на «чужих» мониторах
  крупнее/мельче. Ограничение документировано, полноценный пер-монитор —
  после v1 (с T19-конфигом).
- **SetParent cross-thread** (наш поток vs Explorer): приём из продакшена
  Lively/Seelen; AttachError → фолбэк.
- **MessageBox блокирует event loop** до клика — приемлемо на старте
  (однократно, до run_app-цикла RedrawRequested).
- **Поллинг PARENT_POLL_MS=2 с** будит поток — дёшево; основной канал —
  hook; в T18 пауза рендера утилизирует тот же монитор-поток.
- **Классическая схема непроверяема в песочнице** (нужна ≤23H2 VM):
  реализация по R1-описанию, помечена «проверка на VM» в §9.

## 8. Отступления и решения (фиксация до старта)

1. `PostMessageW` для 0x052C (R4, Seelen) вместо SendMessageTimeout (SPEC
   §7.4) — асинхронно, сразу retry-детект; результат тот же, hang-риск
   ниже. Фиксация в коде комментарием.
2. Фолбэк-«обычное окно» = созданное borderless top-level окно на весь
   виртуальный экран (не пересоздаём окно после winit-инициализации).
3. Анти-флуд re-attach: счётчик 3 подряд неудач → стоп автоматики + warn
   (полный анти-флуд по PID Shell_TrayWnd — T17/T16).
4. enable_activation — на первом MouseInput; до того WS_EX_NOACTIVATE
   держит окно вне фокуса (горячие клавиши не перехватываются — SPEC §9).
5. WS_EX_NOREDIRECTIONBITMAP никогда не ставим сами — только читаем как
   маркер raised (R1).
6. RevokeDragDrop/детач при выходе не делаем: HWND умирает вместе с
   процессом; стили/иконки не нами сломаны — восстановление не нужно.
7. v1-MVP: ручная приёмка только на 25H2 26200 (основная цель) + сборка
   на CI; ≤23H2-VM и 24H2-проверка — по доступу владельца, вне скоупа
   сессии.

## 9. Чек-лист ручной приёмки (владелец, Win11 25H2 build 26200)

1. `canvasdesk.exe --desktop` → канвас за иконками, перед обоями.
2. Иконки рабочего стола видны поверх канваса, кликабельны.
3. Alt+Tab — окна CanvasDesk нет; таскбар не показывает.
4. Драг карточек/зум работают (мышь в child-окне Progman).
5. Дроп файла из Explorer на канвас работает (T9 в M4-режиме).
6. Перезапуск Explorer (kill explorer.exe → autorun) → канвас вернулся
   < 3 с (hook + поллинг-резерв).
7. Смена масштаба монитора 100%↔150% → текст/минимап переградуированы
   (DPI-поллинг), без краша.
8. Мультимонитор: канвас на весь виртуальный экран, минимап в правом
   нижнем углу правого нижнего монитора.
9. Логи: `attach` (стратегия raised, шаги), `verify_styles OK`,
   `WatchWorkerW` установлены, `DpiChanged` при смене.
10. Провал встраивания (тестово: rename Progman-класс подменой? — нет,
    просто наблюдение): при ошибке — MessageBox + оконный режим.

## 10. Маппинг файлов → воркеры (зоны)

| Файл | Воркер |
|---|---|
| `crates/canvas-shell/src/desktop/mod.rs` | T15-A |
| `crates/canvas-shell/src/desktop/hierarchy.rs` | T15-B |
| `crates/canvas-shell/src/desktop/attach.rs` | T15-C |
| `crates/canvas-shell/src/desktop/monitor.rs` | T15-D |
| `crates/canvas-shell/src/lib.rs`, `Cargo.toml`, `win-check/*` | координатор (посев, заморожены) |
| `crates/canvas-app/src/main.rs` | T15-E (координатор) |
