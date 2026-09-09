# T17. Иконки, меню, выход, краш-сейф — план (M4)

ЗАДАЧА: финальный слой UX режима десктопа по TASKS T17 / SPEC §7.4 п.5–7
/ RECIPES R5 (идемпотентное скрытие иконок), R7 (детект краша Explorer
с анти-флудом), R9 (ловушка RefreshDesktop). Источники §7 RECIPES
(Lively/Seelen) — недоступны в песочнице (GPL/AGPL: код не копируется,
механика — только из RECIPES). Рантайм-приёмка (Win11 25H2) — за
владельцем; в песочнице — msvc-кросс-чек типов (win-check) + Linux-тесты
чистых частей.

## 1. Цель и критерии приёмки

- Системные иконки десктопа скрываются в --desktop и ВОССТАНАВЛИВАЮТСЯ
  при штатном выходе; повторные запуски не «мигают» иконками (R5:
  toggle только при расхождении состояний).
- Краш-сейф: kill -9 процесса → следующий запуск (любой, даже оконный)
  форс-восстанавливает иконки по sentinel-файлу.
- Краш/перезапуск Explorer не вешает приложение: TaskbarCreated (из
  шины T16) + PID Shell_TrayWnd до/после → восстановление встройки;
  анти-флуд: >1 настоящий рестарт за 30 с → стоп автоматики + warn
  (R7).
- ПКМ по пустому месту канваса в --desktop → системное меню: Открыть
  канвас / Новый текстовый файл / Показать системные иконки (toggle,
  галочка) / Запускать с Windows (toggle, галочка) / Выход.
- Двойной клик по файловой ноде → ShellExecuteEx(SEE_MASK_INVOKEIDLIST)
  — «как в Explorer» (SPEC §7.4 п.7); в оконном режиме тоже.
- Автозапуск: HKCU\Software\Microsoft\Windows\CurrentVersion\Run,
  значение CanvasDesk = "<exe>" --desktop (SPEC §9/§7.4).
- Рефреш десктопа — ТОЛЬКО InvalidateRect+UpdateWindow(DefView) (R9);
  SPI_SETDESKWALLPAPER под запретом (на raised разрушает WorkerW).
- Все Win32-отказы — warn + деградация (R14): приложение живёт без
  скрытия/меню/автозапуска.

## 2. Что уже есть (инвентарь)

- `desktop/hierarchy.rs` (T15) — `DesktopHierarchy.def_view: HWND`
  (слой иконок, цель toggle 0x7402), find_progman/ensure_worker_w;
  виртуальный экран.
- `desktop/attach.rs` (T15) — идиома SendMessageTimeoutW (0x052C),
  fallback_message_box; `desktop/monitor.rs` (T15) — DesktopEvent/
  Watch/поллинг. `shell_events` (T16) — ShellEvent::ExplorerStarted
  уже доставляется в on_shell_event (TaskbarCreated).
- `main.rs` — attach_desktop (точка «после успешного attach»),
  on_right_button (ПКМ по ноде → меню T7; по пустому — сброс),
  double_click-детектор (по ноде → begin_editing), CloseRequested
  (форс-сейв + exit), on_shell_event, desktop_recover_failures ≥3 —
  стоп recovery (T15, упрощённый анти-флуд).
- `canvas_core` — модель нод (`node.file`), создание file-нод при
  дропе (T9) — образец для «Новый текстовый файл».
- windows-фичи workspace: Win32_UI_Shell (ShellExecuteExW,
  SHGetSetSettings, SHELLEXECUTEINFOW), Win32_UI_WindowsAndMessaging
  (CreatePopupMenu/TrackPopupMenu/AppendMenuW/DestroyMenu, FindWindowW,
  GetWindowThreadProcessId, InvalidateRect/UpdateWindow,
  SetForegroundWindow, GetCursorPos) уже включены (T9/T15).
  Координатор добавляет: Win32_System_Registry (Reg*W для автозапуска).
- Верифицировано по исходникам windows-0.62.2 (registry):
  - `SHGetSetSettings(Option<*mut SHELLSTATEA>, SSF_MASK, bool)` —
    чтение при bset=false; SHELLSTATEA — repr(C, packed(1)):
    fHideIcons — бит 7 `_bitfield1` (маска 0x80); SSF_HIDEICONS =
    SSF_MASK(16384). Запись через fSet=TRUE в Win10+ НЕ работает (R5) —
    только чтение.
  - `ShellExecuteExW(*mut SHELLEXECUTEINFOW) -> Result<()>`;
    SEE_MASK_INVOKEIDLIST = 12.
  - Registry: RegCreateKeyW (legacy — СОЗДАЁТ/открывает ключ, НЕ
    требует фичи Win32_Security; RegCreateKeyExW гейтован ею —
    координаторская проверка реестром)/RegOpenKeyExW/RegSetValueExW/
    RegQueryValueExW/RegDeleteValueW/RegCloseKey,
    HKEY_CURRENT_USER — фича Win32_System_Registry.
  - `TrackPopupMenu(...) -> BOOL` — с TPM_RETURNCMD возвращает id
    команды напрямую (без WM_COMMAND в очередь — идеально для winit).
  - `FindWindowW(class, name) -> Result<HWND>`;
    `GetWindowThreadProcessId(hwnd, Option<*mut u32>) -> u32`.
- `win-check/` — зонд msvc-кросс-чека (desktop + shell_events уже
  подключены; добавить фичу Registry).

## 3. Архитектура

Новые файлы `canvas-shell/src/desktop/`: 4 зоны воркеров. В отличие от
T15/T16 (файлы целиком cfg(windows)), каждый файл T17 СМЕШАННЫЙ:
кроссплатформенные чистые части (решения/маппинги/пути — тесты на
Linux) + cfg(windows)-блоки Win32-механики в том же файле (рекомендация
T15-A «чистое ядро + cfg-блоки»; файлы объявляются в mod.rs БЕЗ cfg).

**`icons.rs` (T17-A). Скрытие иконок + sentinel-краш-сейф + рефреш.**
- Чистое ядро:
  - `fn hide_icons_state(bitfield1: i32) -> bool` — бит 7 (0x80);
    `fn should_toggle(hidden_now: bool, want_hidden: bool) -> bool`
    (XOR — R5);
  - `fn sentinel_path(dir: &Path) -> PathBuf` —
    `<dir>/desktop-icons.sentinel` (dir — default_cache_dir, единый
    источник T6/T14);
  - `struct SentinelSent { .. }`/функции файлового протокола:
    `sentinel_exists`, `sentinel_create`, `sentinel_remove` (fs; на
    Linux тестируются напрямую), содержимое — версия+UTC-timestamp
    (диагностика, не парсится логикой).
- cfg(windows):
  - `fn read_hide_icons() -> Option<bool>` — SHGetSetSettings(SSF_
    HIDEICONS, bset=false) → SHELLSTATEA._bitfield1 → hide_icons_
    state (packed: читать ПО ЗНАЧЕНИЮ, ссылок на поля не брать).
  - `fn toggle_icons(def_view: HWND)` — SendMessageTimeoutW(DefView,
    WM_COMMAND, 0x7402, 0) (идиома attach.rs; hang-защита).
  - `pub fn refresh_desktop(def_view: HWND)` — InvalidateRect(None,
    false) + UpdateWindow (R9; комментарий-запрет SPI_SETDESKWALLPAPER).
  - `pub struct IconGuard { def_view: HWND, hidden_by_us: bool }`:
    - `pub fn capture(def_view: HWND) -> Self` — read_hide_icons;
      лог исходного состояния;
    - `pub fn hide(&mut self)` — идемпотентно: should_toggle(false→
      hidden)? → toggle + refresh + sentinel_create; hidden_by_us=
      true; если уже скрыты — «ничего не делаем» (состояние юзера
      трогать нельзя — R5);
    - `pub fn show(&mut self)` — зеркально (для toggle-пункта меню);
      sentinel_remove при показе;
    - `pub fn restore(&mut self)` — вернуть исходное (для выхода):
      hidden_by_us → toggle + refresh + sentinel_remove; идемпотентен;
    - `impl Drop` — restore() (страховка от unwind; kill -9 обходит —
      на то sentinel). SAFETY: Drop без паник внутри.
  - `pub fn crash_recovery() -> bool` — при старте: sentinel_exists +
    read_hide_icons==Some(true) → toggle + refresh (форс-восстановление
    «иконки спрятаны мёртвой сессией») + sentinel_remove → true (лог
    «восстановлено после краша»); None-состояние — только sentinel_
    remove (пользователь сам показал). Вызывается в main() ДО attach,
    независимо от --desktop (TASKS: «следующий запуск восстанавливает»).
- Win32-отказы (read None/таймаут) — warn + деградация R14 (guard
  переходит в «no-op»-режим: hidden_by_us=false).

**`explorer.rs` (T17-B). Детект краша Explorer + анти-флуд (R7).**
- Чистое ядро: `pub enum ExplorerRestart { SameProcess, Restarted,
  FloodStop }`; `pub struct RestartTracker { last_pid:
  Option<u32>, restarts: Vec<Instant> }`:
  - `pub fn register(&mut self, now_pid: u32, now: Instant) ->
    ExplorerRestart` — R7-связка: last_pid сменился → рестарт (окно
    30 с отталкивается от deque рестартов; >1 за 30 с → FloodStop —
    не Restarted); same pid → SameProcess (TaskbarCreated от DPI-смены
    — R7 п.2). `pub const RESTART_WINDOW: Duration = 30 s`.
  - Метод `pub fn suppress_automatic(&self) -> bool` — после FloodStop
    держим флаг (клир только вручную/перезапуском).
- cfg(windows): `pub fn tray_pid() -> Option<u32>` — FindWindowW(
  "Shell_TrayWnd") → GetWindowThreadProcessId (хэндл→pid; R7 источник
  Lively GetTaskbarExplorerPid).
- Linux-тесты: same-pid → SameProcess; смена pid → Restarted; два
  рестарта <30 с → второй FloodStop; окно >30 с — счётчик гаснет;
  FloodStop держится; пустой старт (None → first pid) → SameProcess-
  класс (инициализация — не рестарт).

**`menu.rs` (T17-C). Системное контекстное меню десктопа.**
- `pub enum DesktopMenuCommand { OpenCanvas, NewTextFile, ToggleIcons,
  ToggleAutostart, Exit }` — Debug/Clone/Copy/PartialEq.
- Чистое: `const MENU_IDS: …` (id 100..104) + `pub fn command_from_
  id(id: usize) -> Option<DesktopMenuCommand>` — тестируемый маппинг;
  id <100/>104 → None.
- cfg(windows): `pub fn popup(hwnd: HWND, icons_hidden: bool,
  autostart_on: bool) -> Option<DesktopMenuCommand>`:
  1. CreatePopupMenu → HMENU;
  2. AppendMenuW ×5 (MF_STRING): Открыть канвас / Новый текстовый
     файл / Показать системные иконки (MF_CHECKED, если скрыты —
     пункт-инверсия: галочка = «сейчас скрыты») / Запускать с Windows
     (MF_CHECKED если autostart_on) / Выход;
  3. GetCursorPos (экранные координаты);
  4. tray-идиома dismiss'а: SetForegroundWindow(hwnd) → TrackPopupMenu
     (TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN) →
     PostMessageW(hwnd, WM_NULL) (классический паттерн tray-меню —
     без него меню не закрывается кликом мимо);
  5. BOOL → id → command_from_id; DestroyMenu ВСЕГДА (Drop-гуард);
  6. TrackPopupMenu BOOL==0 (отмена) → None.
- SAFETY: hwnd — окно нашего процесса/потока; меню — владение
  HMENU наше (DestroyMenu обязателен).

**`interop.rs` (T17-D). ShellExecute + автозапуск.**
- `pub fn open_file(path: &Path) -> Result<(), String>` (cfg(windows))
  — SHELLEXECUTEINFOW { cbSize, fMask = SEE_MASK_INVOKEIDLIST (12;
  контекстное меню Explorer «перейти/открыть/свойства» по типу), verb =
  None (дефолт), file = путь } → ShellExecuteExW → Err(строка) при
  отказе (деградация: warn + негативный тост не строим — лог).
- `pub fn spawn_window_instance(canvas_path: &Path)` (cfg(windows)) —
  «Открыть канвас» из десктопа: второй экземпляр процесса в оконном
  режиме: std::env::current_exe() + ShellExecuteW(open, exe, arg=
  canvas_path, show SW_SHOWNORMAL); провал — warn (R14).
- Чистое: `pub fn autostart_command(exe: &str) -> String` —
  «"C:\…exe" --desktop» (кавычки при пробелах — всегда, парсер Run
  допускает; ТЕСТ: содержит кавычки и флаг). `pub const AUTOSTART_
  RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run"`,
  `pub const AUTOSTART_VALUE: &str = "CanvasDesk"`.
- cfg(windows): `pub fn autostart_enabled() -> bool` — RegOpenKeyExW
  (HKCU, KEY_READ) → RegQueryValueExW(CanvasDesk) → есть и непусто;
  любые WIN32_ERROR≠0 → false (кроме DELETE-кейсов — просто false).
  `pub fn set_autostart(enable: bool) -> Result<(), String>`: enable →
  RegCreateKeyW + RegSetValueExW(REG_SZ, autostart_command(current_
  exe)); disable → RegOpenKeyExW + RegDeleteValueW (ERROR_SUCCESS;
  VALUE_NOT_EXIST → Ok — идемпотентность). RegCloseKey — Drop-гуард.
  Отказы — Err(строка) → warn (R14).
- Linux-тесты: autostart_command (кавычки/флаг), константы путей.

**Интеграция — T17-E (координатор, main.rs).**
- `App` поля (cfg(windows)): `icon_guard: Option<desktop::icons::
  IconGuard>`, `explorer_tracker: desktop::explorer::RestartTracker`,
  `desktop_menu_flood_stopped: bool` (после FloodStop — стоп
  автоматической реакции; T15-recovery работает только по
  WorkerWDestroyed).
- `main()`: (cfg(windows)) `desktop::icons::crash_recovery()` ДО
  event loop (независимо от --desktop — TASKS-критерий kill -9).
- `attach_desktop` (успех): capture(def_view из hier) → hide() →
  хранить в App.
- `on_right_button` (промах по нодам): в --desktop при живом
  desktop_hierarchy → menu::popup(hwnd, icons_hidden = guard.hidden_
  by_us-признак, autostart = interop::autostart_enabled()) → match
  команды: OpenCanvas → spawn_window_instance(&self.scene.path);
  NewTextFile → файл «Новая заметка N.txt» в canvas_dir (fs::write
  пустого) + create_file_node в позиции ПКМ (образец T9-дропа) +
  mark_dirty + sync_watch_dirs; ToggleIcons → guard.show()/hide()
  (toggle); ToggleAutostart → set_autostart(!enabled) (лог); Exit →
  scene.save_now() + guard.restore() + event_loop.exit(). Меню T7
  (по ноде) и не-desktop поведение — без изменений.
- Двойной клик: `hit` Some(index) и `node.file.is_some()` →
  interop::open_file(resolve_node_path) (SPEC §7.4 п.7) — ДО
  begin_editing; text-ноды — как было.
- `on_shell_event` ExplorerStarted: cfg(windows) → explorer::tray_pid
  → tracker.register → Restarted → recover_desktop(FullReattach) +
  повторное watch_worker_w (иерархия пересоздана — хэндлы новые;
  recover_desktop уже это делает) + повторный crash_recovery не нужен
  (SHELLSTATE персистентен — новые DefView рисует по состоянию);
  SameProcess → debug-лог (DPI-смена — T15-поллинг сам догонит);
  FloodStop → desktop_menu_flood_stopped=true (в дальнейших
  Restarted — только warn, без автоматики) + warn «crash-loop».
- CloseRequested: (cfg(windows), desktop) guard.restore() — ДО
  exit (Drop-страховка остаётся на паниках).
- Exit из меню — тот же путь (функция `shutdown_desktop(&mut self)` —
  единая точка: restore + exit после save).

## 4. Пошаговый план

Контракты (публичные сигнатуры + доки) сеет координатор в 4 файла
`desktop/{icons,explorer,menu,interop}.rs`, mod.rs (4 pub mod БЕЗ cfg),
Cargo.toml (+Win32_System_Registry), win-check (+фича, + касания).
Воркеры реализуют тела и тесты, НЕ меняя публичных сигнатур/lib.rs/
Cargo.toml (отступления — через worklog, как T16-B/C).

1. **T17-A** (`desktop/icons.rs`): R5+sentinel+R9 по §3. Тесты Linux:
   hide_icons_state (бит 7), should_toggle (XOR), sentinel-протокол
   (tmp-каталог: create→exists→remove→!exists), sentinel_path.
   Верификация: win-check msvc.
2. **T17-B** (`desktop/explorer.rs`): трекер R7 по §3. Тесты Linux:
   все ветки register + окно 30 с + suppress. Верификация: win-check.
3. **T17-C** (`desktop/menu.rs`): меню по §3. Тесты Linux:
   command_from_id (5 команд, границы, мусор). Верификация: win-check.
4. **T17-D** (`desktop/interop.rs`): ShellExecute/автозапуск по §3.
   Тесты Linux: autostart_command/константы. Верификация: win-check.
5. **T17-E (координатор):** интеграция main.rs по §3, гейты (fmt/
   clippy/test Linux; win-check; CI windows-latest authoritative),
   коммиты `docs(plans): T17 — иконки/меню/выход/краш-сейф; посев
   контрактов desktop (4 зоны воркеров)` и `feat(shell,app): T17 —
   иконки+sentinel, ПКМ-меню, ShellExecute-двойной-клик, автозапуск,
   детект краша Explorer (R5/R7/R9)`.

## 5. Чек-лист RECIPES (приёмка кода, обязательный)

- [ ] R5: toggle ТОЛЬКО при расхождении (should_toggle XOR); чтение
      SHGetSetSettings ДО toggle; fSet=TRUE не используем (Win10+
      не работает); исходное состояние в guard.
- [ ] R5: sentinel создан ТОЛЬКО когда реально тогглили; штатный выход
      → restore + sentinel_remove; kill -9 → sentinel жив →
      crash_recovery следующего запуска.
- [ ] R7: TaskbarCreated-источник — ExplorerStarted из T16 (не своя
      регистрация); PID Shell_TrayWnd до/после; анти-флуд 30 с;
      SameProcess (DPI) ≠ рестарт.
- [ ] R9: НИ одного SPI_SETDESKWALLPAPER (grep-гейт!); рефреш —
      InvalidateRect+UpdateWindow(DefView).
- [ ] Меню: TPM_RETURNCMD (id напрямую), DestroyMenu всегда,
      tray-идиома SetForegroundWindow+WM_NULL.
- [ ] ShellExecuteEx: SEE_MASK_INVOKEIDLIST; провал — warn (R14).
- [ ] Автозапуск: HKCU Run, значение «"exe" --desktop», идемпотентный
      set/unset,galочка из факта реестра.
- [ ] Все unsafe — SAFETY-комментарии; packed SHELLSTATEA читается по
      значению (ссылки на поля packed — UB).

## 6. Сводка тестов

- canvas-shell (Linux): +≈10–14 тестов (icons 5, explorer 6, menu 3,
  interop 2) — итого ~98–102.
- Кросс-типы: win-check msvc — гейт каждого воркера.
- Рантайм (иконки прячутся/возвращаются; kill -9; kill explorer.exe;
  двойной клик; галочки) — ручная приёмка §9.

## 7. Риски, ограничения, фолбэки

- **Смерть Progman при краше Explorer**: наше окно — child чужого
  процесса; user32 при cleanup чужого процесса НЕ уничтожает наши
  child-окна (DestroyWindow процессно-локален) — окно выживает
  orphan'ом, ExplorerStarted → FullReattach перевешивает. Если
  платформа всё же уничтожит (WM_DESTROY) — приложение штатно
  завершится через winit (иконки восстановит новый Explorer:
  SSF_HIDEICONS персистентен, но DefView новый нарисует по состоянию —
  crash_recovery следующего запуска дочистит). Диагностика — §9 п.7.
- **TaskbarCreated от чужих причин** (DPI, тема): R7-фильтр по PID
  отсекает — SameProcess → игнор.
- **TrackPopupMenu модальный цикл**: диспетчеризирует ожидающие
  сообщения (T16-шина wndproc — реентерабелен by construction, план
  T16 §7); блокировка on_right_button на время меню — приемлема
  (ввод пользователя и так в меню).
- **SHELLSTATEA packed(1)**: ссылки на поля — UB (unaligned); только
  копирование структуры/чтение _bitfield1 по значению.
- **RegSetValueExW data &\[u8\]**: REG_SZ — UTF-16 + нулевой
  терминатор (cb в байтах, включая терминатор) — иначе Run-запись
  мусорная.
- **GetCursorPos против масштаба**: меню — экранные физические
  координаты; GetCursorPos даёт их сам (конверсия не нужна).
- **Двойной клик по file-ноде в редакторе путей**: file-нода не
  редактируется текстом (T7-редактор для text) — конфликтов нет.

## 8. Отступления и решения (фиксация до старта)

1. **TPM_RETURNCMD вместо WM_COMMAND-перехвата**: SPEC «перехват
   WM_RBUTTONUP → своё меню» — выполняется на уровне winit-события
   (уже есть on_right_button); WM_COMMAND-воронку не строим —
   TrackPopupMenu с RETURNCMD возвращает команду напрямую (проще,
   без wndproc-хука нашего окна).
2. **Меню системное (Win32), не canvas-рисованное**: консистентность
   стиля канваса vs нативность в desktop-режиме — выбрана нативность
   (галочки/локализация/клавиатура из коробки; Lively/Seelen так же).
   Меню T7 (цвета нод) остаётся рисованым — зоны не пересекаются.
3. **«Открыть канвас» = второй экземпляр в оконном режиме**: SPEC не
   уточняет семантику; самый полезный смысл — редактирование в окне
   (ShellExecuteW на current_exe + путь). Не «показать в Explorer».
4. **«Новый текстовый файл» — файл + нода в canvas_dir** (не в CWD):
   файл попадает под вотчер T10/поиск T14 автоматически; позиция ноды
   — точка ПКМ (как дроп T9).
5. **Sentinel в default_cache_dir** (~/.canvasdesk): гарантированно
   записываем (конфиг-каталог уже используется T6/T14); %APPDATA%
   переезд — T19, миграция sentinel не нужна (краш-сейф одноразовый).
6. **crash_recovery в main() до attach, без --desktop-гейта**: TASKS
   «следующий запуск восстанавливает» — включая оконный запуск после
   kill -9 десктоп-сессии.
7. **Анти-флуд держим отдельно от T15-recover_failures**: T15-счётчик
   — про re-attach-отказы, T17 — про рестарты Explorer; разные
   причины/сброс. Оба ведут к «стоп автоматики + warn».
8. **ShellExecuteW для spawn_window_instance**: вместо CreateProcessW
   — короче (SEE-маски не нужны), show-параметр из коробки.

## 9. Чек-лист ручной приёмки (владелец, Win11 25H2 build 26200)

1. `--desktop`: иконки скрылись; лог «исходное состояние fHideIcons».
   Выход (Alt+F4/крестик/пункт меню) — иконки вернулись.
2. Повторный запуск подряд ×3 — иконки НЕ мигают (идемпотентность R5).
3. В запущенном --desktop скрыть иконки руками юзера невозможно
   (DefView наш toggle не конфликтует) — ПКМ-меню «Показать системные
   иконки» (галочка) → иконки видны поверх канваса; повтор — скрыты.
4. `taskkill /F /IM canvas-app.exe` в --desktop (иконки скрыты) →
   запуск ЛЮБОГО режима → иконки восстановлены, sentinel удалён, лог
   «восстановлено после краша».
5. `taskkill /F /IM explorer.exe` → автоперезапуск: лог
   «ExplorerRestarted (PID …→…)», канвас пере-встроен, иконки
   согласованы; двойной kill <30 с → «FloodStop: автоматика стоп»,
   приложение живо.
6. Двойной клик по file-ноде (картинка/pdf) — открывается ассоциацией
   (SEE_MASK: как в Explorer, для pdf — ассоциированное приложение).
7. ПКМ по пустому месту: все 5 пунктов; «Запускать с Windows» →
   галочка; regedit HKCU Run — «"…canvas-app.exe" --desktop»; снять
   галочку — значение удалено.
8. Меню «Выход»: канвас сохранён (mtime), иконки восстановлены,
   процесс завершён.
9. grep-гейт: в коде нет SPI_SETDESKWALLPAPER (R9).

## 10. Маппинг файлов → воркеры (зоны)

| Файл | Воркер |
|---|---|
| `crates/canvas-shell/src/desktop/icons.rs` | T17-A |
| `crates/canvas-shell/src/desktop/explorer.rs` | T17-B |
| `crates/canvas-shell/src/desktop/menu.rs` | T17-C |
| `crates/canvas-shell/src/desktop/interop.rs` | T17-D |
| `desktop/mod.rs` (подключение), `Cargo.toml`, `win-check/*` | координатор (посев, заморожены) |
| `crates/canvas-app/src/main.rs` | T17-E (координатор) |
