//! Окно-шина системных событий (T16-B, cfg(windows), RECIPES R11):
//! скрытое message-only окно (parent = HWND_MESSAGE) на отдельном потоке
//! с собственным GetMessageW-циклом.
//!
//! Отличие от монитор-потока T15 (паттерн-основа): у потока теперь ЕСТЬ
//! окно — сообщения окна доставляются DispatchMessageW в `wndproc`
//! (единственный unsafe-трамплин, идиома R13); потоковые сообщения
//! (WM_APP_WAKE-будильник, WM_QUIT) обрабатываются в цикле напрямую.
//! Весь mutable-статус (ROUTER/RESPONDER/свой session id/накопитель
//! коалессинга) — thread_local: wndproc и цикл живут в одном потоке,
//! RefCell без блокировок безопасен by construction (схема T15 monitor).
//!
//! Регистрации — `regs.rs` (T16-C); декод SHChangeNotify — `shfiles.rs`
//! (T16-D); классификация/маппинг — чистые функции ядра `mod.rs` (T16-A).
//!
//! Зона воркера T16-B: реализация по плану docs/plans/T16-shell-events.md
//! §3 (window.rs) — публичные сигнатуры заморожены координатором.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};

use super::ShellEvent;

/// Подписчик на события шины (в main — замыкание с EventLoopProxy →
/// AppEvent::Shell). Единая точка подписки из приложения (R11).
pub type ShellResponder = Arc<dyn Fn(ShellEvent) + Send + Sync>;

/// Команды сервису (mpsc; поток спит в GetMessageW — будильник
/// WM_APP_WAKE, тело команды читается try_recv-дрейном; идиома T15
/// monitor).
#[derive(Debug)]
pub enum ShellCommand {
    /// Синхронизировать набор директорий SHChangeNotify-подписок с
    /// желаемым (зеркало WatchService::sync_dirs T10 — те же dirs от
    /// watched_dirs модели).
    SyncFileDirs(Vec<PathBuf>),
    /// Остановить поток: снять регистрации, DestroyWindow, WM_QUIT.
    Shutdown,
}

// SAFETY: PathBuf/Send-примитивы; HWND в командах НЕ хранится (синк
// происходит на потоке шины, окно — его собственность). Трамплин
// wndproc не пересекает потоки (thread_local-статус). unsafe impl —
// идиома R13 для mpsc-передачи в поток-владелец.
unsafe impl Send for ShellCommand {}

/// Сервис-шина: поток + канал команд. `spawn` — в main() рядом с прочими
/// сервисами (без привязки к --desktop, план §8.7); SyncFileDirs — из
/// sync_watch_dirs при изменении набора директорий модели.
#[derive(Clone)]
pub struct ShellEventService {
    tx: Sender<ShellCommand>,
    /// Windows thread id потока шины (handshake при spawn) — адрес
    /// «будильника» WM_APP_WAKE. 0 — поток не жив: команды теряются
    /// молча (деградация R14).
    thread_id: u32,
}

impl ShellEventService {
    /// Запустить шину: окно + регистрации + цикл. События — в responder.
    pub fn spawn(responder: ShellResponder) -> Self {
        // TODO(T16-B): Builder-спавн + handshake thread_id + деградация
        // (провал — warn, Self { tx, thread_id: 0 }) по образцу
        // DesktopMonitorService::spawn (план §3).
        let (tx, _rx) = mpsc::channel::<ShellCommand>();
        let _ = responder;
        Self { tx, thread_id: 0 }
    }

    /// Послать команду (send-ошибка — молча: приложение закрывается /
    /// поток умер; идиома T15 monitor).
    pub fn command(&self, command: ShellCommand) {
        // TODO(T16-B): send + WM_APP_WAKE-будильник (план §3).
        let _ = command;
    }
}

/// Тело потока шины: CoInitializeEx(STA) → окно (HWND_MESSAGE) →
/// регистрации → цикл GetMessageW + DispatchMessageW → клинап.
fn shell_events_loop(_rx: Receiver<ShellCommand>, _ready: Sender<u32>, _responder: ShellResponder) {
    // TODO(T16-B): реализовать по плану §3 (последовательность в доке
    // модуля; WM_TIMER-флаш коалессера FILE_EVENT_COALESCE_MS).
}

/// wndproc окна шины: маршрутизация MsgRouter → классификация ядра →
/// responder; дефолт — DefWindowProcW. extern "system"-трамплин (R13):
/// Rust-состояние — только через thread_local.
unsafe extern "system" fn shell_events_wndproc(
    _hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // TODO(T16-B): реализовать по плану §3 (все шесть MsgKind; WTS —
    // classify_wts + гейт-атомик; ShellFile — декод D + накопитель).
    let _ = (msg, wparam, lparam);
    windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(_hwnd, msg, wparam, lparam)
}
