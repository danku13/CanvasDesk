//! Детект краша/перезапуска Explorer с анти-флудом (T17-B, RECIPES R7).
//!
//! R7-связка из трёх элементов (копировать вместе, Lively
//! WinDesktopCore.cs):
//! 1. Подписка на TaskbarCreated — УЖЕ есть в шине T16 (ShellEvent::
//!    ExplorerStarted), своя регистрация не нужна.
//! 2. Отличие краша от DPI-смены (обе шлю TaskbarCreated): сравнение
//!    PID окна `Shell_TrayWnd` до/после. PID сменился → настоящий
//!    перезапуск Explorer.
//! 3. Анти-флуд: >1 настоящий рестарт за [`RESTART_WINDOW`] →
//!    [`ExplorerRestart::FloodStop`] — автоматическая реакция
//!    останавливается (crash-loop Explorer не должен утащить нас в
//!    бесконечный re-attach), только warn пользователю.
//!
//! Файл смешанный: трекер — чистое ядро (тесты на Linux), Win32-источник
//! PID — cfg(windows). Зона воркера T17-B: реализация по плану
//! docs/plans/T17-desktop-polish.md §3 (explorer.rs) — публичные
//! сигнатуры заморожены координатором.

use std::time::{Duration, Instant};

/// Окно анти-флуда: рестарты считаются внутри этого окна (Lively
/// TaskbarCrashTimeOutDelay, ~30 с; TASKS T17).
pub const RESTART_WINDOW: Duration = Duration::from_secs(30);

/// Решение по факту TaskbarCreated (R7): чистый результат для
/// интеграции T17-E.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExplorerRestart {
    /// PID Shell_TrayWnd не сменился — TaskbarCreated от DPI-смены/темы:
    /// реакции не нужно (DPI ловит поллинг T15, R10).
    SameProcess,
    /// Explorer действительно перезапустился (PID сменился) — встройку
    /// восстановить (FullReattach + повторное watch).
    Restarted,
    /// Второй рестарт внутри окна 30 с — crash-loop: автоматика стоп,
    /// warn пользователю (R7 п.3).
    FloodStop,
}

/// Трекер рестартов Explorer (R7): держит последний PID таскбара и
/// моменты последних рестартов. `register` вызывается на КАЖДЫЙ
/// ExplorerStarted из шины T16 (текущий PID опрашивается вызывающим
/// через [`tray_pid`] перед вызовом).
#[derive(Debug, Default)]
pub struct RestartTracker {
    /// PID Shell_TrayWnd последнего зарегистрированного TaskbarCreated
    /// (None — первый вызов: инициализация, не рестарт).
    last_pid: Option<u32>,
    /// Моменты последних настоящих рестартов (окно RESTART_WINDOW).
    restarts: Vec<Instant>,
    /// Автоматика остановлена после FloodStop (сброс — только
    /// перезапуск приложения; план §8.7).
    flood_stopped: bool,
}

impl RestartTracker {
    /// Зарегистрировать TaskbarCreated с текущим PID таскбара.
    /// R7-логика: первый вызов (last_pid=None) → SameProcess-класс
    /// (инициализация, не рестарт); PID сменился → рестарт (но если
    /// второй+ внутри RESTART_WINDOW → FloodStop); PID тот же →
    /// SameProcess (DPI-смена).
    pub fn register(&mut self, now_pid: u32, now: Instant) -> ExplorerRestart {
        todo!("T17-B")
    }

    /// Автоматическая реакция на рестарты Explorer остановлена
    /// (FloodStop ранее): дальнейшие Restarted логируются, но
    /// автоматикой не обрабатываются (план §3 T17-E).
    pub fn suppress_automatic(&self) -> bool {
        todo!("T17-B")
    }
}

#[cfg(windows)]
/// PID процесса Explorer — владельца окна `Shell_TrayWnd` (R7 источник:
/// Lively GetTaskbarExplorerPid). None — окно не найдено (Explorer
/// перезапускается прямо сейчас / таскбар скрыт).
pub fn tray_pid() -> Option<u32> {
    todo!("T17-B")
}
