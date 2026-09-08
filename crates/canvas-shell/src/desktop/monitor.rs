//! Shell-монитор режима десктопа (T15, RECIPES R6/R10): WinEventHook на
//! разрушение WorkerW + поллинг-резерв + DPI-поллинг.
//!
//! Сервис-поток с собственным GetMessageW-циклом (паттерн WatchService
//! T10; WINEVENT_OUTOFCONTEXT доставляет колбэки ТОЛЬКО в поток с
//! message loop — R6). События уходят подписчику через responder —
//! в приложение это AppEvent::Desktop через EventLoopProxy.
//! Задача чистой реализации (GPL/AGPL — не копируем, RECIPES §0).

use std::sync::mpsc::Sender;
use std::sync::Arc;
use windows::Win32::Foundation::HWND;

/// Подписчик на события десктопа (в main — замыкание с EventLoopProxy).
pub type DesktopResponder = Arc<dyn Fn(super::DesktopEvent) + Send + Sync>;

/// Команды сервису (mpsc; поток читает их в message loop —
/// PostThreadMessageW-«будильник» или PeekMessage-дрейн перед GetMessage).
#[derive(Debug)]
pub enum MonitorCommand {
    /// Установить/перенавесить слежку: WinEventHook(EVENT_OBJECT_DESTROY,
    /// поток WorkerW через GetWindowThreadProcessId, WINEVENT_OUTOFCONTEXT)
    /// + запомнить хэндлы для поллинга (IsWindow worker_w — резерв R6,
    /// GetDpiForWindow ours — R10). Повторный Watch → UnhookWinEvent +
    /// новый hook (WorkerW пересоздан — поток Explorer мог смениться).
    Watch {
        progman: HWND,
        worker_w: HWND,
        ours: HWND,
    },
    /// Остановить поток (PostThreadMessageW WM_QUIT + UnhookWinEvent).
    Shutdown,
}

/// Сервис-монитор: поток + канал команд. spawn — до знания хэндлов
/// (в main() рядом с прочими сервисами), Watch — после attach.
pub struct DesktopMonitorService {
    tx: Sender<MonitorCommand>,
}

impl DesktopMonitorService {
    /// Запуск потока-владельца hook'ов и таймеров. События — в responder.
    pub fn spawn(responder: DesktopResponder) -> Self {
        todo!("T15-D: поток + GetMessageW-цикл; SetTimer DPI_POLL_MS/PARENT_POLL_MS")
    }

    /// Послать команду (send-ошибка — молча: прототип watcher.rs T10 —
    /// приложение закрывается, монитору уже нечего делать).
    pub fn command(&self, command: MonitorCommand) {
        todo!("T15-D")
    }
}

// TODO(T15-D): SAFETY на каждый unsafe; hook-колбэк (extern "system") —
// только сравнение hwnd с целевым и отправка события: никаких ссылок на
// Rust-структуры; SetTimer-WM_TIMER в цикле; DPI-гистерезис не нужен
// (шаг DPI кратен 25%); Shutdown — UnhookWinEvent + KillTimer + WM_QUIT.
