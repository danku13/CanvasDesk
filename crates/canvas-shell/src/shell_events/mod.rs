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
    // R8: NOTIFY_FOR_ALL_SESSIONS доставляет события ВСЕХ сессий —
    // подписчики и гейт интересуются только своей (сверка l_param с
    // ProcessIdToSessionId-идентификатором из regs.rs).
    if session != own {
        return None;
    }
    match code {
        wts_code::CONSOLE_CONNECT => Some(SessionEvent::ConsoleConnect),
        wts_code::CONSOLE_DISCONNECT => Some(SessionEvent::ConsoleDisconnect),
        wts_code::REMOTE_CONNECT => Some(SessionEvent::RemoteConnect),
        wts_code::REMOTE_DISCONNECT => Some(SessionEvent::RemoteDisconnect),
        wts_code::LOGON => Some(SessionEvent::Logon),
        wts_code::LOGOFF => Some(SessionEvent::Logoff),
        wts_code::LOCK => Some(SessionEvent::Locked),
        wts_code::UNLOCK => Some(SessionEvent::Unlocked),
        // REMOTE_CONTROL (9) не моделируем (план §8.2: гейт он не меняет);
        // неизвестные коды будущих версий ОС — тоже None (тихая деградация).
        _ => None,
    }
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
    match wparam {
        power_code::SUSPEND => (true, None),
        power_code::RESUME_USER => (false, Some(ResumeKind::User)),
        power_code::RESUME_AUTOMATIC => (false, Some(ResumeKind::Automatic)),
        // Прочие PBT_* (APMBATTERYLOW, POWERSETTINGCHANGE, ...) шину не
        // касаются — «постороннее»: (false, None).
        _ => (false, None),
    }
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
/// и динамические из RegisterWindowMessageW (заполняет `regs.rs` при
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
        // Инвариант конфигурации (док типа): все АКТИВНЫЕ (id > 0) слоты
        // различны между собой и не задевают фиксированные сообщения шины.
        // 0 — провал RegisterWindowMessageW: неактивный слот, в проверках
        // не участвует (0 не совпадает ни с одним фиксированным id) и в
        // route не матчится. Физически коллизия невозможна: динамические
        // id выдаются в 0xC000..=0xFFFF — вне фиксированных сообщений и
        // WM_APP-диапазона; паника = баг-сигнал (ловится тестом
        // конструктора), production-путь не паникует.
        if taskbar_created != 0 && shellhook != 0 {
            assert_ne!(
                taskbar_created, shellhook,
                "MsgRouter: коллизия динамических id — TaskbarCreated и SHELLHOOK оба равны {taskbar_created}"
            );
        }
        for (slot, id) in [
            ("TaskbarCreated", taskbar_created),
            ("SHELLHOOK", shellhook),
        ] {
            for fixed in [
                WM_WTSSESSION_CHANGE,
                WM_POWERBROADCAST,
                WM_CLIPBOARDUPDATE,
                WM_APP_SHELL_FILE,
            ] {
                assert_ne!(
                    id, fixed,
                    "MsgRouter: динамический id {slot} = {id} конфликтует с фиксированным сообщением шины {fixed}"
                );
            }
        }
        Self {
            taskbar_created,
            shellhook,
        }
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
        // Фиксированные id — константные ветки; динамические — guard'ы
        // «слот активен и id совпал». Неактивный слот (0) не матчится
        // никогда: route(0) → None (0 — WM_NULL, в wndproc не приходит).
        match msg {
            WM_WTSSESSION_CHANGE => Some(MsgKind::WtsSession),
            WM_POWERBROADCAST => Some(MsgKind::Power),
            WM_CLIPBOARDUPDATE => Some(MsgKind::Clipboard),
            WM_APP_SHELL_FILE => Some(MsgKind::ShellFile),
            _ if self.taskbar_created != 0 && msg == self.taskbar_created => {
                Some(MsgKind::TaskbarCreated)
            }
            _ if self.shellhook != 0 && msg == self.shellhook => Some(MsgKind::ShellHook),
            _ => None,
        }
    }
}

/// Чистый маппинг SHCNE-маски (из SHChangeNotification_Lock) и путей из
/// pidl-пары в FileEvent (план §3): приоритет при нескольких битах
/// Rename > Delete > Create > Modify; отсутствие пути, нужного выбранной
/// ветке, → None; пустая/неизвестная маска → None. UPDATEDIR → Modify(dir).
pub fn shell_file_change(mask: i32, old: Option<&Path>, new: Option<&Path>) -> Option<FileEvent> {
    // Разбор по приоритету Rename > Delete > Create > Modify (план §3):
    // одна нотификация может нести несколько бит-причин, парная
    // (rename) информативнее однопутевых. Семантика pidl-пары ShlObj.h:
    // первый pidl — старый/целевой путь (old), второй — новый (new).
    // Неизвестные биты маппинг известных не запрещают (маска из будущего
    // SHCNE-набора с известным битом → маппим по известному); пустая
    // маска или только неизвестные биты → None (ветка ниже).

    // Rename (RENAMEITEM — файл, RENAMEFOLDER — каталог): нужны ОБА пути.
    // Нет пары (например, не-файловый второй pidl — SHGetPathFromIDListW
    // → None в shfiles.rs) — ветка ПРОПУСКАЕТСЯ: маска может объясняться
    // остальными битами (следующий приоритет).
    if mask & (shcne::RENAMEITEM | shcne::RENAMEFOLDER) != 0 {
        if let (Some(old), Some(new)) = (old, new) {
            return Some(FileEvent::Rename(old.to_path_buf(), new.to_path_buf()));
        }
    }

    // Delete (DELETE — файл, в корзину тоже, R12; RMDIR — каталог):
    // строго первый pidl; old нет → None (второй pidl Delete не описывает).
    if mask & (shcne::DELETE | shcne::RMDIR) != 0 {
        return old.map(|path| FileEvent::Remove(path.to_path_buf()));
    }

    // Create (CREATE — файл, MKDIR — каталог): первый pidl; old нет → None.
    if mask & (shcne::CREATE | shcne::MKDIR) != 0 {
        return old.map(|path| FileEvent::Create(path.to_path_buf()));
    }

    // Modify: UPDATEITEM — элемент, UPDATEDIR — каталог (§8.4: каталог не
    // нода, apply даст 0 изменений; событие всё равно доставляем — задел
    // рефреша содержимого папки-ноды). Первый pidl; old нет → None.
    if mask & (shcne::UPDATEITEM | shcne::UPDATEDIR) != 0 {
        return old.map(|path| FileEvent::Modify(path.to_path_buf()));
    }

    // Пустая маска / только неизвестные биты.
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
    use super::*;

    // ---------- classify_wts ----------

    /// Все 9 кодов для СВОЕЙ сессии: 1..8 → SessionEvent по таблице
    /// WinUser.h; REMOTE_CONTROL (9) не моделируется (план §8.2).
    #[test]
    fn classify_wts_all_codes_own_session() {
        let own = 7u32;
        let table = [
            (wts_code::CONSOLE_CONNECT, SessionEvent::ConsoleConnect),
            (
                wts_code::CONSOLE_DISCONNECT,
                SessionEvent::ConsoleDisconnect,
            ),
            (wts_code::REMOTE_CONNECT, SessionEvent::RemoteConnect),
            (wts_code::REMOTE_DISCONNECT, SessionEvent::RemoteDisconnect),
            (wts_code::LOGON, SessionEvent::Logon),
            (wts_code::LOGOFF, SessionEvent::Logoff),
            (wts_code::LOCK, SessionEvent::Locked),
            (wts_code::UNLOCK, SessionEvent::Unlocked),
        ];
        for (code, event) in table {
            assert_eq!(classify_wts(code, own, own), Some(event), "код {code}");
        }
        assert_eq!(classify_wts(wts_code::REMOTE_CONTROL, own, own), None);
    }

    /// Чужая сессия → None при любом коде (R8: шина получает события
    /// ВСЕХ сессий NOTIFY_FOR_ALL_SESSIONS — фильтруем по своему id).
    #[test]
    fn classify_wts_foreign_session_is_none() {
        for code in 1..=9u32 {
            assert_eq!(
                classify_wts(code, 3, 7),
                None,
                "код {code} прошёл чужую сессию"
            );
        }
    }

    /// Неизвестный код (вне 1..9) → None даже для своей сессии.
    #[test]
    fn classify_wts_unknown_code_is_none() {
        for code in [0u32, 10, 100, u32::MAX] {
            assert_eq!(
                classify_wts(code, 7, 7),
                None,
                "код {code} должен быть неизвестен"
            );
        }
    }

    // ---------- SessionEvent::gate_value / SessionGate ----------

    /// Полная таблица gate_value всех 8 SessionEvent: 4 гасят гейт
    /// (locked/logoff/отключения), 4 — включают (unlocked/logon/подключения).
    #[test]
    fn session_event_gate_value_table() {
        let off = [
            SessionEvent::Locked,
            SessionEvent::Logoff,
            SessionEvent::ConsoleDisconnect,
            SessionEvent::RemoteDisconnect,
        ];
        let on = [
            SessionEvent::Unlocked,
            SessionEvent::Logon,
            SessionEvent::ConsoleConnect,
            SessionEvent::RemoteConnect,
        ];
        for event in off {
            assert_eq!(
                event.gate_value(),
                Some(false),
                "{event:?} должен гасить гейт"
            );
        }
        for event in on {
            assert_eq!(
                event.gate_value(),
                Some(true),
                "{event:?} должен включать гейт"
            );
        }
    }

    /// Цикл lock/unlock: старт true (§8.3), Lock → false, Unlock → true.
    #[test]
    fn session_gate_lock_unlock_cycle() {
        let mut gate = SessionGate::default();
        assert!(gate.is_interactive());
        gate.apply(SessionEvent::Locked);
        assert!(!gate.is_interactive());
        gate.apply(SessionEvent::Unlocked);
        assert!(gate.is_interactive());
    }

    /// Повторные события не меняют состояние; оставшиеся 6 событий
    /// применяются по gate_value («нейтральных» в enum нет — все 8
    /// управляют гейтом, ход покрывает обе половины таблицы).
    #[test]
    fn session_gate_repeated_and_remaining_events() {
        let mut gate = SessionGate::default();
        gate.apply(SessionEvent::Locked);
        gate.apply(SessionEvent::Locked); // повторный lock — остаётся false
        gate.apply(SessionEvent::Logoff); // другое «гасящее» — всё ещё false
        assert!(!gate.is_interactive());
        gate.apply(SessionEvent::Logon);
        gate.apply(SessionEvent::Unlocked); // повторное «включающее» — true
        assert!(gate.is_interactive());
        gate.apply(SessionEvent::ConsoleDisconnect);
        assert!(!gate.is_interactive());
        gate.apply(SessionEvent::ConsoleConnect);
        assert!(gate.is_interactive());
        gate.apply(SessionEvent::RemoteDisconnect);
        assert!(!gate.is_interactive());
        gate.apply(SessionEvent::RemoteConnect);
        assert!(gate.is_interactive());
    }

    // ---------- classify_power ----------

    /// Таблица PBT_*: suspend → (true, None); оба резюма — свой
    /// ResumeKind; посторонние wparam → (false, None).
    #[test]
    fn classify_power_table() {
        assert_eq!(classify_power(power_code::SUSPEND), (true, None));
        assert_eq!(
            classify_power(power_code::RESUME_USER),
            (false, Some(ResumeKind::User))
        );
        assert_eq!(
            classify_power(power_code::RESUME_AUTOMATIC),
            (false, Some(ResumeKind::Automatic))
        );
        for wparam in [0u32, 5, 12, 999] {
            assert_eq!(
                classify_power(wparam),
                (false, None),
                "wparam {wparam} посторонний"
            );
        }
    }

    // ---------- атомик-гейт ----------

    /// Гейт IS_INTERACTIVE_SESSION стартует true и на не-Windows остаётся
    /// true (атомик никто не пишет — §3; запись — только wndproc шины).
    #[test]
    fn is_interactive_session_starts_true() {
        assert!(is_interactive_session());
    }

    // ---------- MsgRouter ----------

    /// Нулевые слоты — валидная конфигурация (провал
    /// RegisterWindowMessageW): фиксированные сообщения маршрутизируются,
    /// динамические — нет; route(0) → None.
    #[test]
    fn msg_router_zero_slots_route_fixed_only() {
        let router = MsgRouter::new(0, 0);
        assert_eq!(router.taskbar_created(), 0);
        assert_eq!(router.shellhook(), 0);
        assert_eq!(
            router.route(WM_WTSSESSION_CHANGE),
            Some(MsgKind::WtsSession)
        );
        assert_eq!(router.route(WM_POWERBROADCAST), Some(MsgKind::Power));
        assert_eq!(router.route(WM_CLIPBOARDUPDATE), Some(MsgKind::Clipboard));
        assert_eq!(router.route(WM_APP_SHELL_FILE), Some(MsgKind::ShellFile));
        // неактивные слоты не матчатся (и сам 0 — тоже)
        assert_eq!(router.route(0), None);
        assert_eq!(router.route(12345), None);
    }

    /// Все шесть видов при активных динамических слотах
    /// (RegisterWindowMessageW возвращает 0xC000..=0xFFFF); соседний
    /// незанятый id → None.
    #[test]
    fn msg_router_routes_all_six_kinds() {
        let router = MsgRouter::new(0xC123, 0xC124);
        assert_eq!(router.route(0xC123), Some(MsgKind::TaskbarCreated));
        assert_eq!(router.route(0xC124), Some(MsgKind::ShellHook));
        assert_eq!(
            router.route(WM_WTSSESSION_CHANGE),
            Some(MsgKind::WtsSession)
        );
        assert_eq!(router.route(WM_POWERBROADCAST), Some(MsgKind::Power));
        assert_eq!(router.route(WM_CLIPBOARDUPDATE), Some(MsgKind::Clipboard));
        assert_eq!(router.route(WM_APP_SHELL_FILE), Some(MsgKind::ShellFile));
        assert_eq!(router.route(0xC125), None);
    }

    /// Неактивный слот (0) не матчится даже route(0); активный соседний
    /// слот при этом работает.
    #[test]
    fn msg_router_inactive_slot_never_matches() {
        let router = MsgRouter::new(0, 0xC777);
        assert_eq!(router.route(0), None);
        assert_eq!(router.route(0xC777), Some(MsgKind::ShellHook));
        let router = MsgRouter::new(0xC778, 0);
        assert_eq!(router.route(0), None);
        assert_eq!(router.route(0xC778), Some(MsgKind::TaskbarCreated));
    }

    /// Коллизия двух активных динамических id — паника-инвариант
    /// конструктора (конфигурация статична, док типа MsgRouter).
    #[test]
    #[should_panic(expected = "коллизия динамических id")]
    fn msg_router_panics_on_dynamic_collision() {
        MsgRouter::new(0xC555, 0xC555);
    }

    /// Активный TaskbarCreated, совпавший с фиксированным id, — паника.
    #[test]
    #[should_panic(expected = "TaskbarCreated")]
    fn msg_router_panics_when_taskbar_created_hits_fixed() {
        MsgRouter::new(WM_WTSSESSION_CHANGE, 0);
    }

    /// Активный SHELLHOOK, совпавший с фиксированным id, — паника.
    #[test]
    #[should_panic(expected = "SHELLHOOK")]
    fn msg_router_panics_when_shellhook_hits_fixed() {
        MsgRouter::new(0, WM_APP_SHELL_FILE);
    }

    /// Полный перебор: каждый из четырёх фиксированных id паникует в
    /// каждом слоте (сверка, что new проверяет ВСЕ константы — список
    /// здесь продублирован намеренно); валидные комбинации — молча.
    #[test]
    fn msg_router_fixed_collision_checked_for_all_ids_and_slots() {
        let fixed_ids = [
            WM_WTSSESSION_CHANGE,
            WM_POWERBROADCAST,
            WM_CLIPBOARDUPDATE,
            WM_APP_SHELL_FILE,
        ];
        for &fixed in &fixed_ids {
            for (taskbar_created, shellhook) in [(fixed, 0u32), (0u32, fixed)] {
                let build = move || MsgRouter::new(taskbar_created, shellhook);
                let result = std::panic::catch_unwind(build);
                assert!(
                    result.is_err(),
                    "слоты ({taskbar_created}, {shellhook}) обязаны паниковать (id {fixed})"
                );
            }
        }
        // валидные комбинации (неактивные/различные динамические) не паникуют
        MsgRouter::new(0, 0);
        MsgRouter::new(0xC123, 0xC124);
        MsgRouter::new(0, 0xC124);
    }

    // ---------- shell_file_change ----------

    /// Rename: RENAMEITEM (файл) и RENAMEFOLDER (каталог) с парой путей
    /// (old — первый pidl, new — второй) → Rename(old, new).
    #[test]
    fn shell_file_change_rename_item_and_folder() {
        let old = Path::new(r"C:\dir\a.txt");
        let new = Path::new(r"C:\dir\b.txt");
        assert_eq!(
            shell_file_change(shcne::RENAMEITEM, Some(old), Some(new)),
            Some(FileEvent::Rename(old.to_path_buf(), new.to_path_buf()))
        );
        assert_eq!(
            shell_file_change(shcne::RENAMEFOLDER, Some(old), Some(new)),
            Some(FileEvent::Rename(old.to_path_buf(), new.to_path_buf()))
        );
    }

    /// Delete: SHCNE_DELETE (в корзину — тоже, R12) и SHCNE_RMDIR →
    /// Remove по первому pidl.
    #[test]
    fn shell_file_change_delete_and_rmdir_remove() {
        let old = Path::new(r"C:\dir\a.txt");
        assert_eq!(
            shell_file_change(shcne::DELETE, Some(old), None),
            Some(FileEvent::Remove(old.to_path_buf()))
        );
        assert_eq!(
            shell_file_change(shcne::RMDIR, Some(old), None),
            Some(FileEvent::Remove(old.to_path_buf()))
        );
    }

    /// Create: SHCNE_CREATE (файл) и SHCNE_MKDIR (каталог) → Create по
    /// первому pidl.
    #[test]
    fn shell_file_change_create_and_mkdir() {
        let old = Path::new(r"C:\dir\a.txt");
        assert_eq!(
            shell_file_change(shcne::CREATE, Some(old), None),
            Some(FileEvent::Create(old.to_path_buf()))
        );
        assert_eq!(
            shell_file_change(shcne::MKDIR, Some(old), None),
            Some(FileEvent::Create(old.to_path_buf()))
        );
    }

    /// Modify: SHCNE_UPDATEITEM — элемент; SHCNE_UPDATEDIR — каталог
    /// (§8.4: каталог сам не нода, событие доставляем как Modify(dir)).
    #[test]
    fn shell_file_change_update_item_and_dir() {
        let old = Path::new(r"C:\dir");
        assert_eq!(
            shell_file_change(shcne::UPDATEITEM, Some(old), None),
            Some(FileEvent::Modify(old.to_path_buf()))
        );
        assert_eq!(
            shell_file_change(shcne::UPDATEDIR, Some(old), None),
            Some(FileEvent::Modify(old.to_path_buf()))
        );
    }

    /// Rename без пары путей: ветка пропускается — без других битов итог
    /// None (не-файловый второй pidl — legitimately None у shfiles.rs).
    #[test]
    fn shell_file_change_rename_without_new_falls_through() {
        let old = Path::new(r"C:\dir\a.txt");
        let new = Path::new(r"C:\dir\b.txt");
        assert_eq!(shell_file_change(shcne::RENAMEITEM, Some(old), None), None);
        assert_eq!(
            shell_file_change(shcne::RENAMEFOLDER, Some(old), None),
            None
        );
        // Rename требует ОБА пути: нет старого — тоже проваливание
        assert_eq!(shell_file_change(shcne::RENAMEITEM, None, Some(new)), None);
    }

    /// Однопутевые ветки (Delete/Create/Modify) строго берут ПЕРВЫЙ pidl
    /// (old): его отсутствие → None, второй pidl (new) их не спасает
    /// (семантика SHCNE-пары).
    #[test]
    fn shell_file_change_single_path_kinds_require_old() {
        let new = Path::new(r"C:\dir\b.txt");
        let masks = [
            shcne::DELETE,
            shcne::RMDIR,
            shcne::CREATE,
            shcne::MKDIR,
            shcne::UPDATEITEM,
            shcne::UPDATEDIR,
        ];
        for mask in masks {
            assert_eq!(
                shell_file_change(mask, None, Some(new)),
                None,
                "маска {mask} не должна брать второй pidl"
            );
        }
    }

    /// Приоритет при нескольких битах: Rename > Delete > Create >
    /// Modify (маска Explorer'а может нести несколько причин).
    #[test]
    fn shell_file_change_priority_of_multiple_bits() {
        let old = Path::new(r"C:\dir\a.txt");
        let new = Path::new(r"C:\dir\b.txt");
        // Rename выигрывает у Delete при полной паре путей
        assert_eq!(
            shell_file_change(shcne::RENAMEITEM | shcne::DELETE, Some(old), Some(new)),
            Some(FileEvent::Rename(old.to_path_buf(), new.to_path_buf()))
        );
        // Rename без пары проваливается в Delete
        assert_eq!(
            shell_file_change(shcne::RENAMEITEM | shcne::DELETE, Some(old), None),
            Some(FileEvent::Remove(old.to_path_buf()))
        );
        // Delete выигрывает у Create/Modify (однопутевая ветка)
        assert_eq!(
            shell_file_change(shcne::DELETE | shcne::UPDATEITEM, Some(old), Some(new)),
            Some(FileEvent::Remove(old.to_path_buf()))
        );
        // Create выигрывает у Modify
        assert_eq!(
            shell_file_change(shcne::CREATE | shcne::UPDATEITEM, Some(old), None),
            Some(FileEvent::Create(old.to_path_buf()))
        );
    }

    /// Пустая маска и чисто неизвестные биты → None; неизвестный бит НЕ
    /// запрещает маппинг известного (маппим по известным).
    #[test]
    fn shell_file_change_empty_and_unknown_masks() {
        let old = Path::new(r"C:\dir\a.txt");
        assert_eq!(shell_file_change(0, Some(old), Some(old)), None);
        assert_eq!(shell_file_change(0x4000_0000, Some(old), Some(old)), None);
        assert_eq!(
            shell_file_change(shcne::DELETE | 0x4000_0000, Some(old), None),
            Some(FileEvent::Remove(old.to_path_buf()))
        );
    }

    // ---------- константы ----------

    /// Точные значения WinUser.h/ShlObj.h: локальные копии без
    /// windows-crate (сверка с реальными константами — debug_assert'ы
    /// cfg(windows)-модулей B/C, приём T15-A; координатор верифицировал
    /// значения по исходникам windows-0.62.2).
    #[test]
    fn constants_exact_winuser_shlobj_values() {
        assert_eq!(WM_APP, 0x8000);
        assert_eq!(WM_APP_SHELL_FILE, 0x8016);
        assert_eq!(WM_WTSSESSION_CHANGE, 689);
        assert_eq!(WM_POWERBROADCAST, 536);
        assert_eq!(WM_CLIPBOARDUPDATE, 797);
        assert_eq!(FILE_EVENT_COALESCE_MS, 150);
        // WTS-коды: 1..9 подряд (WinUser.h)
        let wts = [
            wts_code::CONSOLE_CONNECT,
            wts_code::CONSOLE_DISCONNECT,
            wts_code::REMOTE_CONNECT,
            wts_code::REMOTE_DISCONNECT,
            wts_code::LOGON,
            wts_code::LOGOFF,
            wts_code::LOCK,
            wts_code::UNLOCK,
            wts_code::REMOTE_CONTROL,
        ];
        for (i, &code) in wts.iter().enumerate() {
            assert_eq!(code, i as u32 + 1, "WTS-код #{i} нарушает порядок 1..9");
        }
        // PBT-коды
        assert_eq!(power_code::SUSPEND, 4);
        assert_eq!(power_code::RESUME_USER, 7);
        assert_eq!(power_code::RESUME_AUTOMATIC, 18);
        // SHCNE-маски
        assert_eq!(shcne::RENAMEITEM, 1);
        assert_eq!(shcne::CREATE, 2);
        assert_eq!(shcne::DELETE, 4);
        assert_eq!(shcne::MKDIR, 8);
        assert_eq!(shcne::RMDIR, 16);
        assert_eq!(shcne::UPDATEDIR, 4096);
        assert_eq!(shcne::UPDATEITEM, 8192);
        assert_eq!(shcne::RENAMEFOLDER, 0x20000);
    }

    /// Маппируемые SHCNE-биты попарно дизъюнктны (каждый — степень
    /// двойки): маска однозначно раскладывается на причины, приоритет
    /// веток стабильный; фиксированные id шины тоже попарно различны.
    #[test]
    fn constants_shcne_bits_disjoint_and_fixed_ids_distinct() {
        let bits = [
            shcne::RENAMEITEM,
            shcne::CREATE,
            shcne::DELETE,
            shcne::MKDIR,
            shcne::RMDIR,
            shcne::UPDATEDIR,
            shcne::UPDATEITEM,
            shcne::RENAMEFOLDER,
        ];
        for (i, &a) in bits.iter().enumerate() {
            assert_ne!(a, 0, "SHCNE-бит #{i} пуст");
            for &b in &bits[i + 1..] {
                assert_eq!(a & b, 0, "пересечение {a:#010x} и {b:#010x}");
            }
        }
        let ids = [
            WM_WTSSESSION_CHANGE,
            WM_POWERBROADCAST,
            WM_CLIPBOARDUPDATE,
            WM_APP_SHELL_FILE,
        ];
        for (i, &a) in ids.iter().enumerate() {
            for &b in &ids[i + 1..] {
                assert_ne!(a, b, "фиксированные id {a} и {b} совпадают");
            }
        }
    }
}
