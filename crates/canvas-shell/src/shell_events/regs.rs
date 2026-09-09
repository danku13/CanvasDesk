//! Регистрации окна шины (T16-C, cfg(windows), RECIPES R8/R11/R12):
//! шесть каналов системных событий + идемпотентное снятие + SHChangeNotify-
//! подписки на директории нод (diff-синк, зеркало watcher T10).
//!
//! Зона воркера T16-C: реализация по плану docs/plans/T16-shell-events.md
//! §3 (regs.rs). Каждая регистрация — warn-деградация при провале (R14:
//! шина живёт на остальных каналах). Значения констант ядра сверяются с
//! windows-crate debug_assert'ами (приём T15-A).

use std::collections::HashMap;
use std::path::PathBuf;

use windows::Win32::Foundation::HWND;

use super::MsgRouter;

/// Чем владеем для идемпотентного снятия при Shutdown/выходе.
/// Часть каналов не требует явного снятия (умирает с окном/потоком),
/// но WTS/suspend/shell-hook/clipboard снимаем явно — чистый выход.
/// SHChangeNotify-подписки — динамический набор, живёт в
/// [`FileNotifySync`].
#[derive(Debug, Default)]
pub struct ShellRegs {
    /// WTSRegisterSessionNotification прошла.
    pub wts: bool,
    /// HPOWERNOTIFY от RegisterSuspendResumeNotification.
    pub suspend: Option<windows::Win32::System::Power::HPOWERNOTIFY>,
    /// RegisterShellHookWindow прошла.
    pub shell_hook: bool,
    /// AddClipboardFormatListener прошёл.
    pub clipboard: bool,
}

/// Зарегистрировать все шесть каналов на окне шины:
/// 1) WTSRegisterSessionNotification(NOTIFY_FOR_ALL_SESSIONS) — R8;
/// 2) RegisterSuspendResumeNotification(DEVICE_NOTIFY_WINDOW_HANDLE) — R11;
/// 3) RegisterWindowMessageW("TaskbarCreated") — ExplorerStarted, R7/R11;
/// 4) RegisterWindowMessageW("SHELLHOOK") + RegisterShellHookWindow — R11;
/// 5) AddClipboardFormatListener +
///    ChangeWindowMessageFilterEx(WM_CLIPBOARDUPDATE, MSGFLT_ALLOW) — R11
///    (UIPI-фильтр ОБЯЗАТЕЛЕН);
/// 6) SHChangeNotify-подписки — отдельно через [`FileNotifySync::sync`]
///    (динамический набор директорий модели).
/// Возвращает (реестр снятия, MsgRouter с динамическими id).
pub fn register_all(hwnd: HWND) -> (ShellRegs, MsgRouter) {
    // TODO(T16-C): реализовать по плану §3 (все регистрации, warn при
    // провале каждой; динамические id → MsgRouter::new).
    let _ = hwnd;
    (ShellRegs::default(), MsgRouter::default())
}

/// Снять все регистрации (идемпотентно; ошибки — молча/warn):
/// WTSUnRegisterSessionNotification, UnregisterSuspendResumeNotification,
/// DeregisterShellHookWindow, RemoveClipboardFormatListener,
/// SHChangeNotifyDeregister (все id из реестра).
pub fn unregister_all(hwnd: HWND, regs: &mut ShellRegs) {
    // TODO(T16-C): реализовать по плану §3.
    let _ = (hwnd, regs);
}

/// Идентификатор своей сессии (R8): ProcessIdToSessionId(
/// GetCurrentProcessId()); 0 при провале — все WTS-события фильтруются
/// как «чужие» (warn; гейт не обновляется — шина живёт).
pub fn own_session_id() -> u32 {
    // TODO(T16-C): реализовать по плану §3.
    0
}

/// Дифф-синхронизация SHChangeNotify-подписок (R12): зеркало
/// WatchService::sync_dirs (T10) — те же желаемые директории модели.
/// Добавление: pidl каталога (SHParseDisplayName; COM STA потока уже
/// инициализирован) → SHChangeNotifyRegister(hwnd, SHCNRF_ShellLevel |
/// SHCNRF_NewDelivery, SHCNE_DISKEVENTS, msg, 1, entry { fRecursive:
/// false }); снятие: SHChangeNotifyDeregister(id). Ошибки — warn,
/// незарегистрированный dir не попадает в таблицу (ретрай следующим
/// синком). Карта — «нормализованный dir → registration id».
#[derive(Debug, Default)]
pub struct FileNotifySync {
    /// Активные подписки: dir → SHChangeNotifyRegister-id.
    entries: HashMap<PathBuf, u32>,
}

impl FileNotifySync {
    /// Новый пустой синкатор (подписок нет).
    pub fn new() -> Self {
        Self::default()
    }

    /// Синхронизировать подписки с желаемым набором директорий.
    /// `msg` — WM_APP_SHELL_FILE (куда шина положит нотификации).
    pub fn sync(&mut self, hwnd: HWND, msg: u32, dirs: &[PathBuf]) {
        // TODO(T16-C): реализовать по плану §3.
        let _ = (hwnd, msg, dirs);
    }

    /// Снять все подписки (Shutdown).
    pub fn clear(&mut self, hwnd: HWND) {
        // TODO(T16-C): реализовать по плану §3.
        let _ = hwnd;
    }
}
