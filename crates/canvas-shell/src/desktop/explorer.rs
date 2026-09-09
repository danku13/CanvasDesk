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
        // Шаг 1 — окно анти-флуда: моменты старше RESTART_WINDOW покидают
        // счётчик. Окно отталкивается от ТЕКУЩЕГО `now`, а не от первого
        // рестарта: «>1 рестарта за 30 с» обязано выполняться в любой
        // момент жизни трекера (Lively TaskbarCrashTimeOutDelay).
        self.restarts
            .retain(|&t| now.duration_since(t) < RESTART_WINDOW);

        // Шаг 2 — инициализация: первый вызов запоминает PID, но НЕ рестарт
        // (TaskbarCreated при старте приложения — Explorer давно жив,
        // этого события просто не было до нас).
        if self.last_pid.is_none() {
            self.last_pid = Some(now_pid);
            return ExplorerRestart::SameProcess;
        }

        // Шаг 3 — PID не сменился: TaskbarCreated от DPI-смены/темы (R7
        // п.2) — рестарт-момент не пишем, счётчик не растёт; флуд-флаг
        // (если стоит) НЕ сбрасывается — сброс только перезапуском
        // приложения (план §8.7).
        if self.last_pid == Some(now_pid) {
            return ExplorerRestart::SameProcess;
        }

        // Шаг 4 — настоящий рестарт: PID обновляем, момент пишем ВСЕГДА
        // (и после FloodStop — моменты нужны для диагностики причин
        // crash-loop в логах).
        self.restarts.push(now);
        self.last_pid = Some(now_pid);

        // Шаг 5 — анти-флуд: автоматика уже стоплена ИЛИ этот рестарт уже
        // не первый внутри окна → ставим/держим флаг и возвращаем
        // FloodStop (автоматика остановлена, только warn пользователю).
        if self.flood_stopped || self.restarts.len() >= 2 {
            self.flood_stopped = true;
            return ExplorerRestart::FloodStop;
        }
        ExplorerRestart::Restarted
    }

    /// Автоматическая реакция на рестарты Explorer остановлена
    /// (FloodStop ранее): дальнейшие Restarted логируются, но
    /// автоматикой не обрабатываются (план §3 T17-E).
    pub fn suppress_automatic(&self) -> bool {
        self.flood_stopped
    }
}

#[cfg(windows)]
/// PID процесса Explorer — владельца окна `Shell_TrayWnd` (R7 источник:
/// Lively GetTaskbarExplorerPid). None — окно не найдено (Explorer
/// перезапускается прямо сейчас / таскбар скрыт).
pub fn tray_pid() -> Option<u32> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetWindowThreadProcessId};

    // SAFETY: FindWindowW — чистый поиск top-level окна по имени класса
    // (lpwindowname = null: имя таскбара не ищем). Побочных эффектов нет;
    // Shell_TrayWnd — системное окно Explorer. Err (окна нет) — Explorer
    // мёртв/перезапускается прямо сейчас: R7-ветка «PID неизвестен».
    let hwnd = match unsafe { FindWindowW(w!("Shell_TrayWnd"), PCWSTR::null()) } {
        Ok(hwnd) => hwnd,
        Err(err) => {
            tracing::debug!(error = %err, "Shell_TrayWnd не найден — Explorer перезапускается?");
            return None;
        }
    };

    let mut pid = 0u32;
    // SAFETY: hwnd валиден (только что найден FindWindowW);
    // GetWindowThreadProcessId — чистое чтение без побочных эффектов;
    // out-указатель — на локальную переменную этого кадра стека.
    // Возвращаемый tid (поток-владелец таскбара) не нужен — игнорируем.
    let _thread_id = unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    Some(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Первый register (last_pid=None) — инициализация, а не рестарт:
    /// PID запоминается, SameProcess, счётчик рестартов пуст.
    #[test]
    fn first_register_is_initialization_not_restart() {
        let mut tracker = RestartTracker::default();
        let t = Instant::now();
        assert_eq!(tracker.register(100, t), ExplorerRestart::SameProcess);
        assert_eq!(tracker.last_pid, Some(100));
        assert!(tracker.restarts.is_empty());
        assert!(!tracker.suppress_automatic());
    }

    /// Тот же PID (TaskbarCreated от DPI-смены/темы, R7 п.2) —
    /// SameProcess: рестарт-момент не пишется, счётчик не растёт.
    #[test]
    fn same_pid_is_same_process() {
        let mut tracker = RestartTracker::default();
        let t = Instant::now();
        tracker.register(100, t);
        assert_eq!(
            tracker.register(100, t + Duration::from_secs(2)),
            ExplorerRestart::SameProcess
        );
        assert_eq!(tracker.last_pid, Some(100));
        assert!(tracker.restarts.is_empty());
        assert!(!tracker.suppress_automatic());
    }

    /// Смена PID — настоящий рестарт Explorer: Restarted (первый в окне),
    /// флуд не объявляется.
    #[test]
    fn pid_change_is_restart() {
        let mut tracker = RestartTracker::default();
        let t = Instant::now();
        tracker.register(100, t);
        assert_eq!(
            tracker.register(200, t + Duration::from_secs(5)),
            ExplorerRestart::Restarted
        );
        assert_eq!(tracker.last_pid, Some(200));
        assert_eq!(tracker.restarts.len(), 1);
        assert!(!tracker.suppress_automatic());
    }

    /// Два рестарта внутри 30 с — второй FloodStop: crash-loop,
    /// автоматика остановлена (R7 п.3).
    #[test]
    fn two_restarts_inside_window_flood_stop() {
        let mut tracker = RestartTracker::default();
        let t = Instant::now();
        tracker.register(100, t);
        assert_eq!(
            tracker.register(200, t + Duration::from_secs(5)),
            ExplorerRestart::Restarted
        );
        // второй рестарт в пределах RESTART_WINDOW от первого
        assert_eq!(
            tracker.register(300, t + Duration::from_secs(10)),
            ExplorerRestart::FloodStop
        );
        assert_eq!(tracker.last_pid, Some(300));
        assert_eq!(tracker.restarts.len(), 2);
        assert!(tracker.suppress_automatic());
    }

    /// Окно истекло: рестарт → пауза больше 30 с → рестарт — снова
    /// Restarted (момент первого рестарта покинул окно, счётчик сброшен
    /// чисткой; флуд-флаг не ставился).
    #[test]
    fn window_expiry_resets_restart_counter() {
        let mut tracker = RestartTracker::default();
        let t = Instant::now();
        tracker.register(100, t);
        assert_eq!(tracker.register(200, t), ExplorerRestart::Restarted);
        // пауза > RESTART_WINDOW: первый рестарт старше окна → удалён
        // чисткой до подсчёта
        assert_eq!(
            tracker.register(300, t + Duration::from_secs(31)),
            ExplorerRestart::Restarted
        );
        assert_eq!(tracker.restarts.len(), 1);
        assert!(!tracker.suppress_automatic());
    }

    /// FloodStop держится: same-PID событие (DPI) не сбрасывает флуд-флаг
    /// (сброс — только перезапуск приложения, план §8.7); настоящий
    /// рестарт в режиме флуда — по-прежнему FloodStop, но PID и момент
    /// обновляются (диагностика crash-loop).
    #[test]
    fn flood_flag_sticks_after_flood() {
        let mut tracker = RestartTracker::default();
        let t = Instant::now();
        tracker.register(100, t);
        tracker.register(200, t + Duration::from_secs(1));
        assert_eq!(
            tracker.register(300, t + Duration::from_secs(2)),
            ExplorerRestart::FloodStop
        );
        // same-PID (DPI-смена) — SameProcess, флаг НЕ сброшен
        assert_eq!(
            tracker.register(300, t + Duration::from_secs(3)),
            ExplorerRestart::SameProcess
        );
        assert!(tracker.suppress_automatic());
        // настоящий рестарт в режиме флуда — FloodStop (не Restarted);
        // PID и рестарт-момент всё равно записаны
        assert_eq!(
            tracker.register(400, t + Duration::from_secs(4)),
            ExplorerRestart::FloodStop
        );
        assert_eq!(tracker.last_pid, Some(400));
        assert_eq!(tracker.restarts.len(), 3);
        assert!(tracker.suppress_automatic());
    }
}
