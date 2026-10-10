# Deep dive: Lively Wallpaper and Seelen UI methods as recipes for CanvasDesk

Analysis of the current codebases (commits as of September 2026). Place into the repository as `docs/RECIPES.md` next to `SPEC.md` and `TASKS.md`.

---

## 0. Licensing framework — read this first

| Project | License | What it means for CanvasDesk |
|---|---|---|
| Lively Wallpaper | **GPL-3.0** | Copying code → all of CanvasDesk is obliged to become GPL |
| Seelen UI | **AGPL-3.0** | Even stricter: copyleft triggers even on network use |

**Conclusion: we reuse methods, not code.** Everything described below consists of Win32 techniques that are not anyone's intellectual property (Microsoft publicly described the raised desktop; message 0x052C has been known since 2013). Rule for the agent: a clean implementation based on a description of the mechanics, without copying identifiers, structure, or comments from the sources. Seelen UI nevertheless remains valuable as **proof that the entire stack is implementable on windows-rs** — the same APIs, the same types (`HWND`, `SetWindowLongPtrW`, `FindWindowExA`).

## 1. Coverage map

| Area | Lively (C#) | Seelen UI (Rust) | Where in CanvasDesk |
|---|---|---|---|
| Desktop hierarchy detection | ✅ two cases | ✅ two cases | T15 |
| Embedding into the raised desktop (24H2/25H2) | ✅ production | ⚠️ simplified (see R3) | T15 |
| Icon hiding | ✅ idempotent | — | T16 |
| Watch for WorkerW death | ✅ WinEventHook | — | T15 |
| Explorer crash detection | ✅ with anti-flood | — | T16 |
| Session lock/unlock | ✅ | ✅ (WTS + thread gating) | T16 |
| Render pause (energy) | ✅ 4 algorithms | ✅ IS_INTERACTIVE_SESSION | new task |
| Background event window | partial | ✅ Rust reference | new task |
| Shell notifications (recycle bin!) | — | ✅ SHChangeNotifyRegister | T10 |
| DPI after reparenting | ✅ (workaround for WebView2) | ✅ (polling) | T15 |

---

## 2. Desktop-embedding recipes

### R1. Hierarchy detection: two independent sources agree

Both projects document the hierarchy with Spy++ dumps right in the code — use them as test fixtures:

```
Классическая (Win10 / Win11 ≤ 23H2):          Raised desktop (Win11 24H2 / 25H2):
WorkerW                                        Progman
  SHELLDLL_DefView                               SHELLDLL_DefView   <- иконки (WS_EX_LAYERED)
    FolderView (SysListView32)                     FolderView
WorkerW   <- целевой, top-level                  WorkerW           <- целевой, child of Progman
Progman                                        (Progman имеет WS_EX_NOREDIRECTIONBITMAP)
```

**Detection method (a union of both approaches, for T15):**
1. `is_raised = GetWindowLongPtrW(progman, GWL_EXSTYLE) & WS_EX_NOREDIRECTIONBITMAP != 0` — a single call, a reliable marker (Lively).
2. If not raised: the classic traversal — `EnumWindows`; for a top-level window look for the child `SHELLDLL_DefView`; the target `WorkerW` is the next sibling (`FindWindowEx(0, tophandle, "WorkerW", 0)`).
3. If raised: `WorkerW = FindWindowEx(progman, 0, "WorkerW", 0)` — a direct child of Progman. **With retry:** Seelen makes up to 10 attempts with a 100 ms pause — after message 0x052C the window does not appear instantly.

Sources: `Lively.Common/Helpers/Shell/DesktopUtil.cs`, `Lively/Core/WinDesktopCore.cs → SetupDesktopLayer()`, `Seelen-UI src/background/widgets/wallpaper_manager/mod.rs → detect_worker_w()`.

### R2. Embedding into the raised desktop — full mechanics (the core of T15)

This is Lively's most valuable block — the only open production implementation for 24H2+. `WinDesktopCore.cs` contains a verbatim Microsoft comment on the raised-desktop mechanics (the reason for the change — HDR wallpapers). Order of operations from `TryAttachToDesktop()`:

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

Symmetrically: on receiving the WorkerW destruction event (R6) on a raised desktop it is enough to re-execute steps 4–5; a full reset is not needed. On the classic scheme — a full reset (SetParent to the new WorkerW).

Source: `WinDesktopCore.cs → TryAttachToDesktop()`, `EnsureWorkerWZOrder()`.

### R3. Style scrubbing and the window initialization order (Seelen)

Before `SetParent` Seelen forcibly normalizes the styles (`try_set_under_desktop_items()`):

```
style   |= WS_CHILDWINDOW
style   &= !WS_CLIPSIBLINGS
exstyle &= !(WS_EX_ACCEPTFILES | WS_EX_APPWINDOW | WS_EX_WINDOWEDGE)
```

Why each item: `WS_EX_APPWINDOW` — so that the window does not appear in Alt+Tab or the taskbar; `WS_EX_WINDOWEDGE` — when present, the window **disappears from WorkerW after SetParent** (a recorded bug); `WS_EX_ACCEPTFILES` — so that the drop is not intercepted at the shell level before our logic (important for CanvasDesk: we have our own `IDropTarget`, T9).

**Lesson about windowing libraries (directly applicable to winit):** Seelen uses tao (a winit fork for Tauri), and tao asynchronously restores the styles via `SetWindowLongW(GWL_STYLE)` without `WS_CHILD` — "unaware" of the reparenting. Hence the rule for T15: **first fully configure the window through the library's API, then our reparenting as the last step, and after it — verification:** re-read `GWL_STYLE`/`GWL_EXSTYLE` and compare against the expected values; intercept any subsequent style change made by the library (resize, fullscreen toggle) and repeat the scrubbing.

Sources: `mod.rs → try_set_under_desktop_items()`, the comment in `src/ui/svelte/wallpaper-manager/index.ts`.

### R4. Idempotent WorkerW spawn (Seelen)

A critical detail missing from the older tutorials: **send message 0x052C (WPARAM=0xD, LPARAM=0x1) only if WorkerW is absent.** If the raised WorkerW already exists, re-sending makes Explorer tear it down and recreate it → our child window is destroyed → remount → 0x052C again → an infinite create/destroy loop. Seelen caught a real bug on this. Algorithm: `detect_worker_w()` → if `None` → `PostMessage(progman, 0x052C, 0xD, 1)` → re-detect with the retry from R1.

Source: `mod.rs → try_set_under_desktop_items()` (the comment on setup_desktop_layer).

### R5. Idempotent icon hiding (Lively)

```
чтение:  SHGetSetSettings(SHELLSTATE, SSF_HIDEICONS, fGet=TRUE) → fHideIcons
запись:  if (fHideIcons XOR хотим_скрыть):
             SendMessage(DefView, WM_COMMAND, 0x7402, 0)   // toggle
```

Command 0x7402 is a **toggle**, not a setter. Therefore we first read the state and send the toggle only on a mismatch — otherwise on a repeated run the icons would be turned on instead of off. `SHGetSetSettings` with `fSet=TRUE` does not work on Windows 10+ — do not waste time on it (Lively verified). For T16: save the original state at startup, restore it at exit; the crash-safe from TASKS.md complements this.

Source: `DesktopUtil.cs → GetDesktopIconVisibility()/SetDesktopIconVisibility()`.

### R6. Watch for WorkerW destruction (Lively)

Instead of polling — a **WinEventHook on the Explorer thread**: `SetWinEventHook(EVENT_OBJECT_DESTROY, ..., pid/tid of the WorkerW thread)`. The event arrives exactly at the moment the window dies (Explorer decided to rebuild the hierarchy — a wallpaper change, a settings change, sometimes just like that). The reaction differs by scheme: raised → re-detect + Z-order (R2 steps 4–5); classic → a full re-attach.

For Rust: `windows::Win32::UI::Accessibility::SetWinEventHook` + `WINEVENT_OUTOFCONTEXT` (the callback is in our process, no DLL injection). Keep the Progman polling from TASKS.md T15 as the backup channel; the hook is the primary one.

Source: `WinDesktopCore.cs → the constructor (workerWHook), WorkerWHook_EventReceived()`.

### R7. Explorer crash detection with anti-flood (Lively)

Three elements; copy them as a bundle:
1. A subscription to `RegisterWindowMessage("TaskbarCreated")` — Explorer broadcasts it at every taskbar startup.
2. Distinguishing a crash from a DPI change (which also sends TaskbarCreated): compare the PID of the process owning the `Shell_TrayWnd` window before/after. The PID changed → crash/restart.
3. Anti-flood: if there is more than 1 restart within ~30 s — do not react with automation, show an error and stop (in Lively, `TaskbarCrashTimeOutDelay`). Otherwise an Explorer crash-loop will drag us into an infinite re-attach.

Source: `WinDesktopCore.cs → WndProc_TaskbarCreated(), GetTaskbarExplorerPid()`.

### R8. Session lock/unlock/switch

- **Lively (SystemEvents.SessionSwitch):** after unlock check `IsWindow(workerW)` — the handle may have died while the lock screen was up; if it is invalid or the wallpaper "crashed after unlock" → a reset with a 1 s delay.
- **Seelen (the Rust reference):** `WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_ALL_SESSIONS)` → `WM_WTSSESSION_CHANGE` with `WTS_SESSION_LOCK/UNLOCK`, **always verify the `l_param` session id against our own session** (events arrive from all sessions). On top of that — the global atomic `IS_INTERACTIVE_SESSION`: a non-interactive session → **a pause of all background threads** (for them it is webviews; for us — rendering, the thumbnail pool, the watcher, the preview host).

For T16: the WTS + atomic-gate combo is at the same time the foundation of energy saving (R14).

Sources: `WinDesktopCore.cs → SystemEvents_SessionSwitch()`; `Seelen-UI src/background/windows_api/event_window.rs`.

### R9. The RefreshDesktop trap (both stumbled into it)

The classic way to "clean up" the desktop is `SystemParametersInfo(SPI_SETDESKWALLPAPER, ..., NULL)`. **On a raised desktop this destroys WorkerW**; on an MSIX build it makes the shell rebuild the entire hierarchy and kicks our window out of its parent. Both projects ended up with: on a raised desktop, do not call it at all (Lively — early return; Seelen — log-and-swallow). For clearing artifacts, `InvalidateRect(DefView) + UpdateWindow(DefView)` is enough (Seelen's `refresh_desktop()` method).

Sources: `WinDesktopCore.cs → RefreshDesktop()`; `handlers.rs` (the comment on MSIX).

### R10. DPI after reparenting (both)

A child window of WorkerW/Progman **does not receive the correct DPI or its change events**: Lively forcibly reads the scale of the target monitor and passes it to WebView2 ("when running as child of WorkerW/Progman, the WebView2 surface does not pick up the correct DPI"); Seelen found that `onScaleChanged` does not fire after reparenting and polls `devicePixelRatio` every 500 ms.

Recipe for wgpu/winit (T15): after reparenting do not trust `window.scale_factor()`; compute the scale ourselves — `GetDpiForSystem` or `GetDpiForWindow(progman)`; a subscription to `WM_DPICHANGED` will not work → **poll `GetDpiForWindow` every 500–1000 ms or on the display-change event**; on a change — recreate the surface and recompute the text sizes (see SPEC §6.5).

Sources: `Lively/Factories/WallpaperPluginFactory.cs` (the comment); `index.ts → lookupDPI()`.

---

## 3. Infrastructure recipes in Rust (Seelen UI)

### R11. The hidden system-events window — a reference for canvas-shell

Seelen keeps an invisible window on a separate thread with its own `GetMessageW` loop, used exclusively as a **system-events bus**. A single subscription point; events are broadcast to subscribers via a channel (`event_manager!` on crossbeam). What is registered on this window is a direct list for our crate `canvas-shell`:

| Registration | Events | Why CanvasDesk needs it |
|---|---|---|
| `RegisterShellHookWindow` + `RegisterWindowMessage("SHELLHOOK")` | shell window events | context for the pause logic |
| `RegisterSuspendResumeNotification` | sleep/resume | a correct .canvas save before sleep |
| `WTSRegisterSessionNotification` | lock/unlock/switch | R8, energy saving |
| `AddClipboardFormatListener` + `ChangeWindowMessageFilterEx(WM_CLIPBOARDUPDATE, MSGFLT_ALLOW)` | clipboard | the future "paste file as a node"; **the reception itself matters:** if the process is ever elevated, UIPI silently blocks messages — the filter is mandatory |
| `SHChangeNotifyRegister` | shell file events | see R12 |
| `RegisterWindowMessage("TaskbarCreated")` (add it for us) | Explorer crash | R7 |

**Decision for CanvasDesk:** add a `shell_events` module to `canvas-shell` following this pattern — a hidden message-only window (`HWND_MESSAGE` as parent) + a thread + a crossbeam channel into the application. This is a cheap, production-proven way to receive everything system-related in one place, without smearing Win32 callbacks across the code.

Source: `src/background/windows_api/event_window.rs` (exemplary as a whole).

### R12. SHChangeNotifyRegister as a complement to notify (T10)

Seelen tracks the recycle bin via `SHChangeNotifyRegister(SHCNRF_ShellLevel, SHCNE_ALLEVENTS)`. Why this matters for our watcher: **deleting a file into the recycle bin is not a Delete for ReadDirectoryChangesW** (it is a move into the system folder `$Recycle.Bin`), and notify will behave non-obviously. SHChangeNotify provides shell-level events (`SHCNE_DELETE`, `SHCNE_RENAMEITEM`, `SHCNE_UPDATEITEM`) — including operations done through Explorer, with already-normalized paths.

Decision: in T10 keep `notify` for Modify/Create (faster, more detailed), and complement Rename/Delete with an SHChangeNotify subscription on the node directories — especially the scenario "the user deleted a file into the recycle bin via Explorer" → a correct `brokenLink` per SPEC §7.5.

Source: `event_window.rs → register_shell_notifications()`.

### R13. The safe enum-wrapper pattern (copy as an idiom)

`WindowEnumerator`/`MonitorEnumerator` — the canonical safe pattern for passing a Rust closure into a Win32 enum callback: a boxed closure, the pointer via `LPARAM`, a static `extern "system" fn` trampoline, `unsafe impl Send/Sync` with a justification. Plus the recursive `for_each_and_descendants`. This is a ready-made idiom for all of our `canvas-shell` (EnumWindows, EnumChildWindows, EnumDisplayMonitors) — a clean safe API over unsafe calls, one unsafe spot per family of operations.

Source: `src/background/windows_api/iterator.rs`.

### R14. Graceful degradation when embedding fails

`handlers.rs → set_as_wallpaper()`: if the attach into the desktop hierarchy fails — not a panic and not a refusal, but a **fallback to absolute positioning** (an ordinary window over the entire virtual screen) + a warn in the log. This is exactly our principle "unknown version → windowed mode" from SPEC §7.4, but Seelen shows that the fallback is needed at **every step**, not only at version detection: any `SetParent`/`SetWindowPos` can return an error on a particular machine (antivirus, a custom shell, RDP).

---

## 4. Energy saving and render pause (Lively Playback)

For wallpapers this is a battery question; for us — likewise: the canvas in M4 mode renders constantly, and burning GPU while the desktop is not visible is unacceptable. Methods from `Lively/Core/Suspend/Playback.cs`:

### R15. A timer instead of WinEventHook for window tracking

Lively deliberately uses a **timer (a configurable interval, ~1 s) instead of `EVENT_OBJECT_LOCATIONCHANGE`** — the hook has too much noise even with filters, and on some systems it is unreliable. The same principle for our pause logic: a tick once per second → an evaluation → a state change. Cheap and predictable.

### R16. The hierarchy of pause conditions (priority top to bottom)

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

For CanvasDesk: the "pause" state = stopping the render loop (request_redraw is not planned), freezing the thumbnail pool and the preview host, taking down live previews. Wake-up — on the next tick with "desktop visible". Items 1–3 suffice; the grid algorithm is overkill.

### R17. IsDesktop() — the "we are in the foreground" check

`GetForegroundWindow()` and a comparison against `progman` and the **original** WorkerW (the one that contains DefView — cached at initialization, `original_WorkerW`). Lively uses it for input forwarding; for us — for hotkeys in M4: intercept Ctrl+F and the rest only when the foreground is the desktop, otherwise hand it to the system (SPEC §9, "hotkey conflict").

---

## 5. Mapping onto the development plan

| Recipe | Task | Action |
|---|---|---|
| R1–R4, R10, R14 | T15 | Fold into the T15 prompt as an acceptance checklist (see §6) |
| R5, R7, R8, R9 | T16 | The same for T16 |
| R6 (WinEventHook) | T15 | Replace "polling 2s" with a hook + polling backup |
| R11 (event window) | **new T15b** | A `shell_events` module in canvas-shell: hidden window + channel |
| R12 (SHChangeNotify) | T10 | Complement the watcher with shell events (recycle bin!) |
| R13 (enum idiom) | T15 | The idiom for all of canvas-shell, into AGENTS.md |
| R15–R17 (pause) | **new T16b** | Energy saving: a 1 s tick, a gate on rendering and the pools |

**Recommendation:** add a link to this document into SPEC §7.4 and two new tasks (T15b, T16b) into the plan — both are small (2–3 days each with Kimi Code), but they close classes of bugs that would otherwise surface at the M4 acceptance.

## 6. Prompt addenda (ready to paste into TASKS.md)

Add to T15 the checklist:
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

Add to T16:
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

## 7. Sources (files for the agent to read before T15/T16)

| File | What to take from it |
|---|---|
| `lively/src/Lively/Lively.Common/Helpers/Shell/DesktopUtil.cs` | WorkerW/DefView detection, icons |
| `lively/src/Lively/Lively.Core/WinDesktopCore.cs` | SetupDesktopLayer, TryAttachToDesktop, EnsureWorkerWZOrder, WorkerWHook, TaskbarCreated, SessionSwitch, the Microsoft comment on the raised desktop |
| `lively/src/Lively/Lively.Core/Suspend/Playback.cs` | Pause algorithms, gamemode, system conditions |
| `lively/src/Lively/Lively.Common/WindowClassExclusions.cs` | The list of "this is the desktop" classes |
| `Seelen-UI/src/background/widgets/wallpaper_manager/mod.rs` | detect_worker_w, try_set_under_desktop_items, refresh_desktop |
| `Seelen-UI/src/background/widgets/wallpaper_manager/handlers.rs` | The fallback to absolute positioning, the MSIX trap |
| `Seelen-UI/src/background/windows_api/event_window.rs` | The system-events bus (R11) |
| `Seelen-UI/src/background/windows_api/iterator.rs` | The enum idiom (R13) |
| `Seelen-UI/src/ui/svelte/wallpaper-manager/index.ts` | The window initialization order, DPI polling |

Reminder to the agent: read as documentation, write your own code (GPL-3.0 / AGPL-3.0, §0).

## 8. The widget permissions model (M5, T21)

A security cookbook for the host↔widget bridge (SPEC §7.6, the M5 plan §4.6/§4.7).
Principle: **enforcement on every call**; the permissions live in the package manifest
(`~/.canvasdesk/widgets/<id>/widget.json`), not in the widget's message.

### 8.1. The permission table

| Permission | Grants | Where it is checked |
|---|---|---|
| `shell:open` | `openFile` (open a file in a system application) | the host's bridge handler |
| `fs:read` | `readDir` (a listing of the allowlist directories) | bridge + the П4 allowlist |
| `network` | the widget's external http(s) requests (fetch) | the `WebResourceRequested` filter + CSP |
| `canvas:read` | reserved (in v1.1 no method requires it) | — |

Without a permission: `ready`/`resize`/`setProps`/`toast`/`stateGet`/`stateSet`
work — that is the widget's own data, there are no leaks.

### 8.2. The enforcement recipe (host)

```
1. permissions := манифест ПАКЕТА ноды (не сообщения!)
2. match required_permission(call):
   None                 -> выполнить
   Some(p) if allows(p) -> выполнить
   Some(p)              -> warn-лог + JSON-RPC error {"error": "нет permission: p"}
                           (для запросов с id; уведомления — только warn)
```

Typos in permission values are a manifest error (the package is not installed):
strict deserialization, not a silent skip.

### 8.3. The fs:read allowlist recipe (П4)

The widget does not know absolute paths: `readDir("")` → the canvas root,
relative paths are resolved from it. Only the following are allowed:
- the canvas root (the `.canvas` folder);
- the folders of the scene's file nodes (`watched_dirs`).

Recipe: `join` → `canonicalize()` (kills `..`, symlinks, mixed
separators) → `starts_with` against one of the canonical roots. A nonexistent
path is refused before the comparison. The response contains only `name` + `isDir` —
a minimal leak surface.

### 8.4. The network opt-in recipe

`network` does not open arbitrary requests: CSP `default-src 'self'` in
the response header + a `WebResourceRequested` filter (external http(s) — Cancel
without the permission). With the permission, `connect-src https:` is added to the CSP —
otherwise the widget's fetches are useless.

### 8.5. The state-isolation recipe (T21-E)

`widget_state(node_id, key, value)` in `cache.db`: the API accepts node_id
only from the instance's context (the manager substitutes it itself); the widget cannot
read someone else's rows. Recreatability: a failure = warn + an empty result.

### 8.6. Undo coalescing for setProps

A sticker with debounce sends setProps in a stream — without coalescing, the undo grows.
Recipe: a 1.5 s window per node — consecutive setProps of one node merge
into a single step (as in text editing); another node starts a new step.
