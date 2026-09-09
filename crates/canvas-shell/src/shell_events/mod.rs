//! Шина системных событий (T16, RECIPES R11): чистое ядро — типы событий,
//! классификаторы сообщений и глобальный гейт интерактивной сессии.
//!
//! Механика (по образцу Seelen event_window, чистая реализация — GPL/AGPL,
//! код не копируется, RECIPES §0): скрытое message-only окно на отдельном
//! потоке (`window.rs`, cfg(windows)) получает Win32-сообщения шести
//! регистраций (`regs.rs`) и рассылает их подписчикам через responder —
//! в приложении это `AppEvent::Shell` через EventLoopProxy (единая точка
//! подписки). Отступление от промпта TASKS: crossbeam-канал заменён
//! устоявшимся паттерном responder'ов (ThumbService T6 / watcher T10 /
//! search T14 / desktop T15) — новая зависимость без функционального
//! выигрыша нарушает AGENTS.md п.9 (план T16 §8.1).
//!
//! Это ядро НЕ зависит от windows-crate: константы сообщений — локальные
//! копии значений WinUser.h/ShlObj.h с комментариями; cfg(windows)-модули
//! сверяют значения с реальными константами `windows` через debug_assert
//! (приём T15-A, включается win-check-сборкой). Всё тестируется на Linux.
//!
//! SHCNE-мост (RECIPES R12): shell-события файловой системы декодируются
//! (`shfiles.rs`, cfg(windows)) и маппятся чистой функцией [`shell_file_change`]
//! в [`canvas_core::FileEvent`] — тот же конвейер, что у вотчера T10
//! (идемпотентное применение к модели; корзина → brokenLink).

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use canvas_core::FileEvent;

/// Окно коалессинга файл-событий шины: WM_TIMER-флаш накопителя
/// `Vec<FileEvent>` (зеркало агрегатора T10 — сглаживает шквалы
/// Explorer-масс-операций, план T16 §3).
pub const FILE_EVENT_COALESCE_MS: u32 = 150;

/// WM_APP = 0x8000 (WinUser.h): база приватного диапазона сообщений.
const WM_APP: u32 = 0x8000;

/// Приватное сообщение «SHChangeNotify-нотификация» (регистрируется как
/// wMsg в SHChangeNotifyRegister): WM_APP+0x16 (0x15 занят будильником
/// монитор-потока T15). Локальное значение — WinUser.h WM_APP.
pub const WM_APP_SHELL_FILE: u32 = WM_APP + 0x16;

/// WM_WTSSESSION_CHANGE = 689 (0x2B1, WinUser.h): событие смены состояния
/// сессии от WTSRegisterSessionNotification (R8).
pub const WM_WTSSESSION_CHANGE: u32 = 689;

/// WM_POWERBROADCAST = 536 (0x218, WinUser.h): suspend/resume от
/// RegisterSuspendResumeNotification (R11).
pub const WM_POWERBROADCAST: u32 = 536;

/// WM_CLIPBOARDUPDATE = 797 (0x31D, WinUser.h): смена буфера обмена от
/// AddClipboardFormatListener (R11).
pub const WM_CLIPBOARDUPDATE: u32 = 797;

/// Коды WPARAM WM_WTSSESSION_CHANGE (WinUser.h): 1..9.
pub mod wts_code {
    /// WTS_CONSOLE_CONNECT = 1.
    pub const CONSOLE_CONNECT: u32 = 1;
    /// WTS_CONSOLE_DISCONNECT = 2.
    pub const CONSOLE_DISCONNECT: u32 = 2;
    /// WTS_REMOTE_CONNECT = 3.
    pub const REMOTE_CONNECT: u32 = 3;
    /// WTS_REMOTE_DISCONNECT = 4.
    pub const REMOTE_DISCONNECT: u32 = 4;
    /// WTS_SESSION_LOGON = 5.
    pub const LOGON: u32 = 5;
    /// WTS_SESSION_LOGOFF = 6.
    pub const LOGOFF: u32 = 6;
    /// WTS_SESSION_LOCK = 7.
    pub const LOCK: u32 = 7;
    /// WTS_SESSION_UNLOCK = 8.
    pub const UNLOCK: u32 = 8;
    /// WTS_SESSION_REMOTE_CONTROL = 9 (не моделируем — план §8.2).
    pub const REMOTE_CONTROL: u32 = 9;
}

/// Коды WPARAM WM_POWERBROADCAST (WinUser.h): PBT_*.
pub mod power_code {
    /// PBT_APMSUSPEND = 4: система уходит в сон.
    pub const SUSPEND: u32 = 4;
    /// PBT_APMRESUMESUSPEND = 7: пробуждение по требованию пользователя.
    pub const RESUME_USER: u32 = 7;
    /// PBT_APMRESUMEAUTOMATIC = 18: автоматическое пробуждение.
    pub const RESUME_AUTOMATIC: u32 = 18;
}

/// Битовые маски SHCNE_* (ShlObj.h, i32 в сигнатуре SHChangeNotifyRegister).
pub mod shcne {
    /// SHCNE_RENAMEITEM = 1: переименование элемента.
    pub const RENAMEITEM: i32 = 1;
    /// SHCNE_CREATE = 2: создан элемент.
    pub const CREATE: i32 = 2;
    /// SHCNE_DELETE = 4: удалён элемент (в корзину — тоже; R12).
    pub const DELETE: i32 = 4;
    /// SHCNE_MKDIR = 8: создан каталог.
    pub const MKDIR: i32 = 8;
    /// SHCNE_RMDIR = 16: удалён каталог.
    pub const RMDIR: i32 = 16;
    /// SHCNE_UPDATEDIR = 4096: изменился каталог.
    pub const UPDATEDIR: i32 = 4096;
    /// SHCNE_UPDATEITEM = 8192: изменился элемент.
    pub const UPDATEITEM: i32 = 8192;
    /// SHCNE_RENAMEFOLDER = 0x20000: переименован каталог.
    pub const RENAMEFOLDER: i32 = 0x20_000;
}

/// Событие шины, доставляемое подписчику (в main — AppEvent::Shell).
/// Потребители в M4: T17 (ExplorerStarted), T18 (Session/ShellHook);
/// ClipboardUpdated — задел под «вставить как ноду» (SPEC §7.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    /// Смена состояния сессии: только своя сессия (R8 — сверка id).
    Session {
        /// Что произошло.
        event: SessionEvent,
        /// Идентификатор сессии (из l_param WM_WTSSESSION_CHANGE).
        session_id: u32,
    },
    /// Система уходит в сон: потребитель обязан форс-сейв .canvas (TASKS T16).
    Suspending,
    /// Система проснулась.
    Resumed {
        /// Вид пробуждения.
        kind: ResumeKind,
    },
    /// TaskbarCreated (R7/R11): Explorer (пере)запущен — потребитель T17.
    ExplorerStarted,
    /// SHELLHOOK-событие (RegisterShellHookWindow): сырой HSHELL_*-код
    /// и hwnd — декодирует потребитель T18 (план §3).
    ShellHook {
        /// HSHELL_*-код из wParam (WinUser.h).
        code: u32,
        /// Значение HWND из lParam (не указатель Rust — число).
        hwnd: usize,
    },
    /// Буфер обмена изменился (AddClipboardFormatListener, R11).
    ClipboardUpdated,
    /// Батч файл-событий SHCNE-моста (R12) — конвейер T10 `on_file_events`.
    FileEvents(Vec<FileEvent>),
}

/// Состояние сессии из WPARAM WM_WTSSESSION_CHANGE (WinUser.h 1..8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    /// WTS_SESSION_LOCK = 7.
    Locked,
    /// WTS_SESSION_UNLOCK = 8.
    Unlocked,
    /// WTS_SESSION_LOGON = 5.
    Logon,
    /// WTS_SESSION_LOGOFF = 6.
    Logoff,
    /// WTS_CONSOLE_CONNECT = 1.
    ConsoleConnect,
    /// WTS_CONSOLE_DISCONNECT = 2.
    ConsoleDisconnect,
    /// WTS_REMOTE_CONNECT = 3.
    RemoteConnect,
    /// WTS_REMOTE_DISCONNECT = 4.
    RemoteDisconnect,
}

impl SessionEvent {
    /// Значение гейта IS_INTERACTIVE_SESSION после события: Some(false) —
    /// сессия не интерактивна (залочена/отключена), Some(true) — наоборот;
    /// None — событие не управляет гейтом.
    pub fn gate_value(self) -> Option<bool> {
        match self {
            Self::Locked | Self::Logoff | Self::ConsoleDisconnect | Self::RemoteDisconnect => {
                Some(false)
            }
            Self::Unlocked | Self::Logon | Self::ConsoleConnect | Self::RemoteConnect => Some(true),
        }
    }
}

/// Классифицировать WPARAM WM_WTSSESSION_CHANGE: только события СВОЕЙ
/// сессии (R8 — регистрация NOTIFY_FOR_ALL_SESSIONS шлёт все), неизвестный
/// код или чужая сессия → None. `session` — l_param сообщения.
pub fn classify_wts(code: u32, session: u32, own: u32) -> Option<SessionEvent> {
    // TODO(T16-A): реализовать по плану §3 (таблица кодов wts_code).
    let _ = (code, session, own);
    None
}

/// Вид пробуждения (WPARAM WM_POWERBROADCAST, PBT_*).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeKind {
    /// PBT_APMRESUMEAUTOMATIC = 18.
    Automatic,
    /// PBT_APMRESUMESUSPEND = 7.
    User,
}

/// Классифицировать WPARAM WM_POWERBROADCAST: PBT_APMSUSPEND →
/// Suspending-семантика, PBT_APMRESUME* → резюм-вид. Возвращает пару
/// (это_сон, вид_пробуждения): (true, None) — сон; (false, Some(kind)) —
/// пробуждение; (false, None) — посторонний PBT_*.
pub fn classify_power(wparam: u32) -> (bool, Option<ResumeKind>) {
    // TODO(T16-A): реализовать по плану §3.
    let _ = wparam;
    (false, None)
}

/// Гейт интерактивной сессии (R8): атомик обновляется только событиями
/// СВОЕЙ сессии в wndproc шины; потребители (T18 — гейт фоновых потоков)
/// читают [`is_interactive_session`]. На не-Windows всегда true (атомик
/// никто не обновляет).
static IS_INTERACTIVE_SESSION: AtomicBool = AtomicBool::new(true);

/// Читатель гейта IS_INTERACTIVE_SESSION (R8): false — сессия залочена/
/// отключена, фоновые потоки должны спать (потребление — T18).
pub fn is_interactive_session() -> bool {
    IS_INTERACTIVE_SESSION.load(Ordering::Relaxed)
}

/// Обновить гейт из wndproc шины (cfg(windows), только события своей
/// сессии); pub(crate) — внешний мир читает, не пишет.
// cfg_attr: вызов — только из window.rs (cfg(windows)); на Linux функция
// legitimately не используется (гейт навсегда true).
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn store_interactive_session(value: bool) {
    IS_INTERACTIVE_SESSION.store(value, Ordering::Relaxed);
}

/// Чистый автомат гейта сессии (для логики и тестов; окно-шина применяет
/// его к событиям своей сессии, а результат пишет в атомик).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionGate {
    interactive: bool,
}

impl Default for SessionGate {
    /// Старт — true: WTS шлёт только ИЗМЕНЕНИЯ, залоченная до старта сессия
    /// не пришлёт события до unlock (план §8.3; T18 компенсирует).
    fn default() -> Self {
        Self { interactive: true }
    }
}

impl SessionGate {
    /// Применить событие своей сессии; события без gate_value игнорируются.
    pub fn apply(&mut self, event: SessionEvent) {
        if let Some(value) = event.gate_value() {
            self.interactive = value;
        }
    }

    /// Текущее состояние гейта.
    pub fn is_interactive(&self) -> bool {
        self.interactive
    }
}

/// Класс сообщения окна шины (маршрутизация в wndproc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgKind {
    /// WM_WTSSESSION_CHANGE (фиксированный id).
    WtsSession,
    /// WM_POWERBROADCAST (фиксированный id).
    Power,
    /// WM_CLIPBOARDUPDATE (фиксированный id).
    Clipboard,
    /// RegisterWindowMessageW("TaskbarCreated") — динамический id.
    TaskbarCreated,
    /// RegisterWindowMessageW("SHELLHOOK") — динамический id.
    ShellHook,
    /// WM_APP_SHELL_FILE (фиксированный приватный id).
    ShellFile,
}

/// Маршрутизатор сообщений окна шины: фиксированные id (константы модуля)
/// + динамические из RegisterWindowMessageW (заполняет `regs.rs` при
/// регистрации). Инвариант: все активные id различны (конструктор паникует
/// на коллизию — конфигурация статична, паника = баг-сигнал тесту).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MsgRouter {
    taskbar_created: u32,
    shellhook: u32,
}

impl Default for MsgRouter {
    /// Динамические id ещё не зарегистрированы (0 — неактивные слоты).
    fn default() -> Self {
        Self {
            taskbar_created: 0,
            shellhook: 0,
        }
    }
}

impl MsgRouter {
    /// Собрать роутер из динамических id RegisterWindowMessageW
    /// (провал регистрации → 0: слот неактивен, сообщения просто не
    /// приходят). Паникует при коллизии активных id.
    pub fn new(taskbar_created: u32, shellhook: u32) -> Self {
        // TODO(T16-A): инвариант-проверки + поля (план §3, тесты-коллизии).
        let _ = (taskbar_created, shellhook);
        Self::default()
    }

    /// Динамический id TaskbarCreated (0 — не зарегистрирован).
    pub fn taskbar_created(&self) -> u32 {
        self.taskbar_created
    }

    /// Динамический id SHELLHOOK (0 — не зарегистрирован).
    pub fn shellhook(&self) -> u32 {
        self.shellhook
    }

    /// Определить класс сообщения по id; None — чужое сообщение.
    pub fn route(&self, msg: u32) -> Option<MsgKind> {
        // TODO(T16-A): реализовать по плану §3.
        let _ = msg;
        None
    }
}

/// Чистый маппинг SHCNE-маски (из SHChangeNotification_Lock) и путей из
/// pidl-пары в FileEvent (план §3): приоритет при нескольких битах
/// Rename > Delete > Create > Modify; отсутствие пути, нужного выбранной
/// ветке, → None; пустая/неизвестная маска → None. UPDATEDIR → Modify(dir).
pub fn shell_file_change(mask: i32, old: Option<&Path>, new: Option<&Path>) -> Option<FileEvent> {
    // TODO(T16-A): реализовать по плану §3.
    let _ = (mask, old, new);
    None
}

// cfg(windows)-подмодули: окно-шина (B), регистрации (C), декод
// SHChangeNotify (D) — зоны воркеров T16-B/C/D (план §3, §10).
#[cfg(windows)]
pub mod regs;
#[cfg(windows)]
pub mod shfiles;
#[cfg(windows)]
pub mod window;

#[cfg(test)]
mod tests {
    // TODO(T16-A): тесты по плану §4 п.1 (classify_wts, gate, power,
    // router, shell_file_change, константы).
}
