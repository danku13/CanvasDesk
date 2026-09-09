//! Окно-шина системных событий (T16-B, cfg(windows), RECIPES R11):
//! скрытое message-only окно (parent = HWND_MESSAGE) на отдельном потоке
//! с собственным GetMessageW-циклом.
//!
//! Отличие от монитор-потока T15 (паттерн-основа): у потока теперь ЕСТЬ
//! окно — сообщения окна доставляются DispatchMessageW в `wndproc`
//! (единственный unsafe-трамплин, идиома R13); потоковые сообщения
//! (WM_APP_WAKE-будильник, WM_QUIT) обрабатываются в цикле напрямую.
//! Весь mutable-статус (ROUTER/RESPONDER/свой session id/накопитель
//! коалессинга/таблица SHChangeNotify-подписок) — thread_local: wndproc и
//! цикл живут в одном потоке, RefCell без блокировок безопасен by
//! construction (схема T15 monitor).
//!
//! Регистрации — `regs.rs` (T16-C); декод SHChangeNotify — `shfiles.rs`
//! (T16-D); классификация/маппинг — чистые функции ядра `mod.rs` (T16-A).
//!
//! Зона воркера T16-B: реализация по плану docs/plans/T16-shell-events.md
//! §3 (window.rs) — публичные сигнатуры заморожены координатором.

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use canvas_core::FileEvent;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::HBRUSH;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
    PostThreadMessageW, RegisterClassW, SetTimer, HCURSOR, HICON, HWND_MESSAGE, MSG,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_QUIT, WM_TIMER, WNDCLASSW, WNDCLASS_STYLES,
};

use super::regs;
use super::shfiles;
use super::{
    classify_power, classify_wts, store_interactive_session, MsgKind, MsgRouter, ShellEvent,
    FILE_EVENT_COALESCE_MS, WM_APP_SHELL_FILE,
};

use windows::Win32::System::LibraryLoader::GetModuleHandleW;

/// «Будильник» потока шины (копия идиомы monitor.rs T15): команда уже
/// лежит в mpsc, поток спит в GetMessageW — PostThreadMessageW(WM_APP_WAKE)
/// выводит его на try_recv-дрен. Смещение 0x17: 0x15 занят монитор-потоком
/// T15, 0x16 — WM_APP_SHELL_FILE (T16-A); диапазон WM_APP — приватный
/// (WinUser.h), с регистрациями RegisterWindowMessageW не пересекается.
const WM_APP_WAKE: u32 = WM_APP + 0x17;

/// Идентификатор ОКОННОГО таймера коалессинга файл-событий. В отличие от
/// потоковых таймеров T15-монитора (SetTimer(None, …)) ставится на окно
/// шины: WM_TIMER доставляется DispatchMessageW в wndproc (план §3 п.6).
const TIMER_ID_COALESCE: usize = 1;

thread_local! {
    /// Роутер сообщений шины (id шести каналов из regs::register_all).
    /// Thread_local: wndproc и цикл живут в одном потоке — RefCell без
    /// блокировок безопасен by construction (схема T15 monitor).
    /// Ленивая инициализация дефолтом важна: wndproc вызывается уже ВО
    /// ВРЕМЯ CreateWindowExW (WM_NCCREATE/WM_CREATE) — до записи роутера;
    /// route → None → DefWindowProcW — безопасно.
    static ROUTER: RefCell<MsgRouter> = RefCell::new(MsgRouter::default());

    /// Responder (клон Arc): extern "system"-трамплин не захватывает
    /// Rust-состояние — события уходят через этот thread_local.
    static RESPONDER: RefCell<Option<ShellResponder>> = const { RefCell::new(None) };

    /// Идентификатор своей сессии (R8): 0 — не определён, WTS-события
    /// фильтруются classify_wts как «чужие».
    static OWN_SESSION: Cell<u32> = const { Cell::new(0) };

    /// Накопитель коалессинга файл-событий (флаш — WM_TIMER в wndproc,
    /// зеркало агрегатора T10 — сглаживает шквалы Explorer-масс-операций).
    static FILE_BUF: RefCell<Vec<FileEvent>> = const { RefCell::new(Vec::new()) };

    /// Таблица SHChangeNotify-подписок (diff-синк зеркала T10, R12).
    /// Инвариант: инициализируется и читается только при живом окне
    /// (синк/clear — в ветках с hwnd; без окна подписки невозможны).
    static FILE_NOTIFY_SYNC: RefCell<regs::FileNotifySync>
        = RefCell::new(regs::FileNotifySync::new());

    /// Таймер коалессинга установлен (фолбэк без таймера — файл-события
    /// отправляются поштучно сразу при декоде, план §3 п.6).
    static COALESCE_ON: Cell<bool> = const { Cell::new(false) };
}

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
        let (tx, rx) = mpsc::channel::<ShellCommand>();
        // Handshake-канал: поток сообщает свой Windows thread id — он нужен
        // command() для «будильника», а GetCurrentThreadId доступен только
        // изнутри самого потока (идиома monitor.rs T15).
        let (ready_tx, ready_rx) = mpsc::channel::<u32>();
        // Builder, а не std::thread::spawn: провал создания потока — warn и
        // деградация без паники в вызывающем потоке (урок T14-A).
        let spawned = std::thread::Builder::new()
            .name("shell-events".to_string())
            .spawn(move || shell_events_loop(rx, ready_tx, responder));
        match spawned {
            Ok(_join) => {
                // JoinHandle не хранится: сервис живёт до конца процесса
                // (паттерн watcher.rs T10 — join не делаем).
                // recv — мгновенно: thread id — первое, что шлёт поток;
                // смерть потока до send → 0 → «будильник» отключён.
                let thread_id = ready_rx.recv().unwrap_or(0);
                Self { tx, thread_id }
            }
            Err(err) => {
                tracing::warn!(%err, "shell-шина не запущена — системные события отключены");
                // rx и ready_tx умерли вместе с замыканием: send в tx теперь
                // ошибается, command() игнорирует молча (деградация R14).
                Self { tx, thread_id: 0 }
            }
        }
    }

    /// Послать команду (send-ошибка — молча: приложение закрывается /
    /// поток умер; идиома T15 monitor).
    pub fn command(&self, command: ShellCommand) {
        // Ошибка send — молча: поток вышел / приложение закрывается.
        let _ = self.tx.send(command);
        // «Будильник»: поток спит в GetMessageW — будим WM_APP_WAKE, тело
        // команды он возьмёт из канала try_recv-дрейном.
        if self.thread_id != 0 {
            // SAFETY: thread_id получен handshake'ом от потока; если поток
            // уже завершился, PostThreadMessageW откажет — молча (ошибка
            // далее игнорируется, команда останется в канале мёртвого rx).
            let _ =
                unsafe { PostThreadMessageW(self.thread_id, WM_APP_WAKE, WPARAM(0), LPARAM(0)) };
        }
    }
}

/// Локальный mutable-статус цикла шины (НЕ thread_local: доступен только
/// телу потока; wndproc обходит через ROUTER/FILE_BUF-статус выше).
/// hwnd в Option — «статус потребления» идемпотентного клинапа:
/// Shutdown-ветка делает take(), точка после цикла видит None → no-op.
struct BusState {
    /// Message-only окно шины (None — не создано или уже разрушено).
    hwnd: Option<HWND>,
    /// Реестр регистраций шести каналов (regs.rs; снятие идемпотентно).
    regs: regs::ShellRegs,
}

/// Тело потока шины: handshake → COM(STA) → message-only окно →
/// регистрации → таймер коалессинга → цикл GetMessageW +
/// DispatchMessageW → единый идемпотентный клинап.
fn shell_events_loop(rx: Receiver<ShellCommand>, ready: Sender<u32>, responder: ShellResponder) {
    // Handshake: первое дело — свой thread id для command()-«будильника».
    // SAFETY: тривиальный геттер без предусловий и побочных эффектов;
    // вызов только внутри потока шины — берём СВОЙ идентификатор
    // (см. current_thread_id в desktop/monitor.rs).
    let self_id = unsafe { GetCurrentThreadId() };
    // Ошибка send в одноразовый handshake-канал — молча: rx может умереть
    // только вместе с сервисом (spawn тогда получит 0 — деградация).
    let _ = ready.send(self_id);
    // Responder — thread_local: wndproc отправляет события, не имея
    // параметров-состояний (SAFETY-схема в модульном комментарии).
    RESPONDER.with(|r| *r.borrow_mut() = Some(responder));

    // COM STA: нужен SHParseDisplayName в FileNotifySync::sync (T16-C,
    // R12). Отказ — warn и деградация: SHChangeNotify-подписки не
    // заработают, окно и остальные регистрации живут (R14).
    // SAFETY: CoInitializeEx на новом потоке (наследия MTA нет),
    // pvreserved — NULL по контракту. windows-0.62.2 возвращает «сырой»
    // HRESULT: is_ok() (= код >= 0) покрывает и S_OK, и S_FALSE («COM уже
    // инициализирован» — успех по COM-контракту, балансируется
    // CoUninitialize так же); отрицательные коды (RPC_E_CHANGED_MODE и
    // пр.) — отказ → com_ok = false.
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let com_ok = hr.is_ok();
    if !com_ok {
        tracing::warn!(
            code = hr.0,
            "CoInitializeEx(STA) отказал — SHChangeNotify-подписки отключены (R14)"
        );
    }

    // Message-only окно (R11): невидимо, не в Alt+Tab, EnumWindows его
    // не видит — by construction (parent = HWND_MESSAGE, стиль 0).
    let mut state = match create_message_window() {
        Some(hwnd) => {
            // Регистрации шести каналов + роутер + своя сессия — только при
            // живом окне (без окна сообщения каналов некуда доставлять).
            let (regs, router) = regs::register_all(hwnd);
            let own = regs::own_session_id();
            if own == 0 {
                tracing::warn!("session id не определён — WTS-события будут фильтроваться (R8)");
            }
            // Thread_local-статус шины: роутер/сессия/таблица подписок.
            // FILE_NOTIFY_SYNC инициализируем здесь — инвариант «доступ
            // только при живом окне» (см. комментарий статика).
            ROUTER.with(|r| *r.borrow_mut() = router);
            OWN_SESSION.with(|s| s.set(own));
            FILE_NOTIFY_SYNC.with(|s| *s.borrow_mut() = regs::FileNotifySync::new());
            // Таймер коалессинга: окно живо → WM_TIMER придёт в wndproc.
            start_coalesce_timer(hwnd);
            tracing::info!(
                session_id = own,
                "shell-шина: message-only окно создано, регистрации выполнены"
            );
            BusState {
                hwnd: Some(hwnd),
                regs,
            }
        }
        None => {
            // Деградация (warn уже записан в create_message_window): шина
            // неактивна, поток живёт для команд (крайний случай R14).
            BusState {
                hwnd: None,
                regs: regs::ShellRegs::default(),
            }
        }
    };

    // Цикл: GetMessageW (сон без CPU) + WM_APP_WAKE-дрен команд; оконные
    // сообщения — DispatchMessageW → wndproc. Потоковые посторонние при
    // диспатче — безвредный no-op (hwnd = None). WM_QUIT (от любого
    // источника) GetMessageW возвращает как 0 — до match, выход по break.
    let mut msg = MSG::default();
    loop {
        // SAFETY: msg — локальный POD-буфер; фильтры 0..0 = все сообщения
        // потока. BOOL трактуем по документации GetMessageW: -1 = ошибка
        // (битое сообщение — warn и продолжаем), 0 = WM_QUIT, иначе
        // сообщение получено. Сравнение по .0: as_bool() счёл бы -1
        // «истиной» (копия monitor.rs).
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        if result.0 == -1 {
            // Практически недостижимо с фильтрами 0..0; сообщение могло
            // прийти битым — пропускаем, цикл продолжает жить.
            tracing::warn!("GetMessageW вернул -1 — сообщение пропущено");
            continue;
        }
        if result.0 == 0 {
            break; // WM_QUIT — выход, клинап ниже
        }
        match msg.message {
            // «Будильник» из command(): дрен всех накопленных команд.
            WM_APP_WAKE => {
                while let Ok(command) = rx.try_recv() {
                    match command {
                        ShellCommand::SyncFileDirs(dirs) => {
                            // Подписки — только при живом окне (деградация
                            // R14: команды впустую, warn не плодим). Дрен
                            // сериализован с декодом одним потоком — гонок
                            // нет (план §7).
                            if let Some(hwnd) = state.hwnd {
                                FILE_NOTIFY_SYNC
                                    .with(|s| s.borrow_mut().sync(hwnd, WM_APP_SHELL_FILE, &dirs));
                            }
                        }
                        ShellCommand::Shutdown => {
                            // Полный клинап здесь (идемпотентный хелпер:
                            // единая точка после цикла станет no-op) и
                            // WM_QUIT себе: GetMessageW вернёт 0 → выход.
                            shutdown_bus(&mut state);
                            // SAFETY: self_id — идентификатор ЭТОГО потока
                            // (получен внутри него); повторный WM_QUIT
                            // безвреден — GetMessageW вернёт 0 лишь раз.
                            let _ = unsafe {
                                PostThreadMessageW(self_id, WM_QUIT, WPARAM(0), LPARAM(0))
                            };
                            // Команды после Shutdown не обрабатываем.
                            break;
                        }
                    }
                }
            }
            // Оконные сообщения (WM_TIMER, WM_WTSSESSION_CHANGE, …) — в
            // wndproc через DispatchMessageW; потоковые посторонние — no-op.
            _ => {
                // SAFETY: msg — валидное сообщение из GetMessageW;
                // DispatchMessageW вызывает wndproc окна msg.hwnd (для
                // потоковых сообщений hwnd = None — документированный no-op).
                let _ = unsafe { DispatchMessageW(&msg) };
            }
        }
    }

    // Единая точка клинапа (идемпотентно): после Shutdown-ветки hwnd уже
    // None; внешний WM_QUIT без Shutdown чистит всё здесь.
    shutdown_bus(&mut state);
    // COM-баланс: ровно один CoUninitialize на успешный CoInitializeEx
    // (S_OK и S_FALSE — оба «инициализировано», контракт COM).
    if com_ok {
        // SAFETY: парен успешному CoInitializeEx ЭТОГО потока (com_ok);
        // после CoUninitialize COM-объекты потока не используются.
        unsafe { CoUninitialize() };
    }
}

/// Создать message-only окно шины (R11): RegisterClassW +
/// CreateWindowExW(parent = HWND_MESSAGE; стиль/расширение 0 — никаких
/// WS_VISIBLE, геометрия игнорируется). None — деградация (warn уже
/// записан): шина без окна — регистрации невозможны, поток живёт для
/// команд (крайний случай R14).
fn create_message_window() -> Option<HWND> {
    // HINSTANCE процесса — ключ регистрации класса и создания окна.
    // SAFETY: штатный биндинг (фича Win32_System_LibraryLoader включена
    // координатором вместо временной link!-декларации — тот же путь, что
    // GetCurrentThreadId в T15-D): NULL-имя = модуль exe — документированный
    // вызов без предусловий; отказ (для NULL-имени практически невозможен) —
    // Err — ветка ниже (деградация R14).
    let module = match unsafe { GetModuleHandleW(PCWSTR::null()) } {
        Ok(module) => module,
        Err(err) => {
            tracing::warn!(%err, "HINSTANCE процесса не получен — окно шины не создано (R14)");
            return None;
        }
    };
    let instance = HINSTANCE::from(module);
    let class_name = w!("CanvasDeskShellEventsWnd");
    let class = WNDCLASSW {
        // Без CS_*: message-only окно не рисуется и не активируется.
        style: WNDCLASS_STYLES(0),
        // Единственный unsafe-трамплин R13: Rust-состояние — thread_local.
        lpfnWndProc: Some(shell_events_wndproc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instance,
        // Иконка/курсор/фон не нужны: окно невидимо (HWND_MESSAGE).
        hIcon: HICON::default(),
        hCursor: HCURSOR::default(),
        hbrBackground: HBRUSH::default(),
        lpszMenuName: PCWSTR::null(),
        lpszClassName: class_name,
    };
    // SAFETY: class — локальный POD (указатели — на статические w!-литералы
    // и наш trampoline); RegisterClassW копирует содержимое в свою таблицу.
    // Возврат 0 = отказ (класс уже зарегистрирован и пр.) — деградация.
    let atom = unsafe { RegisterClassW(&class) };
    if atom == 0 {
        tracing::warn!("класс окна шины не зарегистрирован — шина без окна (R14)");
        return None;
    }
    // Имя класса и окна — один литерал (CreateWindowExW требует оба).
    // SAFETY: class_name — статический w!-литерал; instance — хэндл модуля
    // выше; hwndparent = HWND_MESSAGE переключает окно в message-only-режим
    // (R11). Err = отказ создания — деградация (зарегистрированный класс
    // остаётся: одна шина на процесс, повторный спавн честно деградирует).
    match unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            class_name,
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance),
            None,
        )
    } {
        Ok(hwnd) => Some(hwnd),
        Err(err) => {
            tracing::warn!(%err, "окно шины не создано — регистрации невозможны (R14)");
            None
        }
    }
}

/// Поставить оконный таймер коалессинга файл-событий
/// (FILE_EVENT_COALESCE_MS, T16-A): SetTimer с hwnd = Some и
/// TIMERPROC = None → WM_TIMER доставляется DispatchMessageW в wndproc
/// (в отличие от потоковых таймеров T15-монитора). Отказ — warn,
/// файл-события идут поштучно сразу при декоде (фолбэк R14).
fn start_coalesce_timer(hwnd: HWND) {
    // SAFETY: hwnd — окно ЭТОГО потока (создано выше в нём); id/период —
    // наши константы; TIMERPROC = None → без callback-трамплина.
    if unsafe { SetTimer(Some(hwnd), TIMER_ID_COALESCE, FILE_EVENT_COALESCE_MS, None) } == 0 {
        tracing::warn!("таймер коалессинга не установлен — файл-события пойдут поштучно");
    } else {
        COALESCE_ON.with(|c| c.set(true));
    }
}

/// Идемпотентный клинап шины: SHChangeNotify-подписки + регистрации
/// каналов + таймер + окно. `state.hwnd.take()` — статус потребления:
/// повторный вызов (после Shutdown-ветки) — no-op; окно/регистрации могли
/// и не быть (деградация создания). Класс окна не снимаем (UnregisterClassW):
/// процесс завершается, система вычистит; хранить atom/instance ради этого
/// — лишний статус (окно единственное на класс).
fn shutdown_bus(state: &mut BusState) {
    if let Some(hwnd) = state.hwnd.take() {
        // Подписки SHChangeNotify: таблица thread_local инициализирована
        // (окно было создано → setup прошёл).
        FILE_NOTIFY_SYNC.with(|s| s.borrow_mut().clear(hwnd));
        // Регистрации шести каналов (regs.rs идемпотентен сам по флагам).
        regs::unregister_all(hwnd, &mut state.regs);
        // Таймер: снятие безусловно — KillTimer неустановленного таймера
        // просто откажет (молча, как в monitor.rs T15).
        // SAFETY: таймер ставился этим потоком на это окно; после
        // DestroyWindow ниже сообщений WM_TIMER уже не будет.
        let _ = unsafe { KillTimer(Some(hwnd), TIMER_ID_COALESCE) };
        COALESCE_ON.with(|c| c.set(false));
        // SAFETY: DestroyWindow вызывается из потока-владельца окна (мы в
        // нём); очередь сообщений разрушаемого окна чистится системой.
        if let Err(err) = unsafe { DestroyWindow(hwnd) } {
            tracing::warn!(%err, "окно шины не разрушено");
        }
    }
}

/// wndproc окна шины: маршрутизация MsgRouter → классификация ядра →
/// responder; дефолт — DefWindowProcW. extern "system"-трамплин (R13):
/// Rust-состояние — только через thread_local. ВНИМАНИЕ: вызывается и ВО
/// ВРЕМЯ CreateWindowExW (WM_NCCREATE/WM_CREATE) — до записи ROUTER:
/// thread_local лениво инициализируются дефолтами, route → None →
/// DefWindowProcW — безопасно by construction.
unsafe extern "system" fn shell_events_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Коалессер — ДО маршрутизации (отдельная ветка, план §3 п.6):
    // route() знает только id шести каналов шины, WM_TIMER среди них нет;
    // wParam у WM_TIMER — идентификатор таймера.
    if msg == WM_TIMER && wparam.0 == TIMER_ID_COALESCE {
        flush_file_events();
        return LRESULT(0);
    }
    // Роутер — Copy-значение; borrow держим только на route (схема
    // monitor.rs: responder — вне borrow).
    let kind = ROUTER.with(|r| r.borrow().route(msg));
    let Some(kind) = kind else {
        // SAFETY: параметры трамплина валидны (hwnd из DispatchMessageW);
        // DefWindowProcW — документированный дефолт-обработчик без
        // предусловий (в т.ч. для чужих WM_TIMER и WM_NCCREATE).
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    };
    match kind {
        // classify_wts отсекает чужие сессии (R8: NOTIFY_FOR_ALL_SESSIONS
        // шлёт все): Some — только своя; гейт-атомик обновляется тоже
        // только событием своей сессии.
        MsgKind::WtsSession => {
            let own = OWN_SESSION.with(|s| s.get());
            if let Some(event) = classify_wts(wparam.0 as u32, lparam.0 as u32, own) {
                if let Some(value) = event.gate_value() {
                    store_interactive_session(value);
                }
                send_event(ShellEvent::Session {
                    event,
                    session_id: lparam.0 as u32,
                });
            }
        }
        // (true, None) — сон; (false, Some(kind)) — пробуждение; прочие
        // PBT_* — (false, None) — ничего (T16-A).
        MsgKind::Power => {
            let (is_suspending, resume_kind) = classify_power(wparam.0 as u32);
            if is_suspending {
                send_event(ShellEvent::Suspending);
            } else if let Some(kind) = resume_kind {
                send_event(ShellEvent::Resumed { kind });
            }
        }
        // TaskbarCreated: Explorer (пере)запущен (R7/R11; потребитель T17).
        MsgKind::TaskbarCreated => {
            send_event(ShellEvent::ExplorerStarted);
        }
        // Сырой HSHELL_*-код + hwnd (значение HANDLE): декод — T18.
        MsgKind::ShellHook => {
            send_event(ShellEvent::ShellHook {
                code: wparam.0 as u32,
                hwnd: lparam.0 as usize,
            });
        }
        MsgKind::Clipboard => {
            send_event(ShellEvent::ClipboardUpdated);
        }
        // Декод T16-D (NewDelivery: lParam — HANDLE нотификации): маска +
        // pidl-пара → FileEvent. Таймер жив → накопитель (WM_TIMER-флаш
        // соберёт батч); нет таймера → фолбэк «поштучно сразу» (план §3).
        MsgKind::ShellFile => {
            if let Some(event) = shfiles::decode(lparam.0, wparam.0) {
                if COALESCE_ON.with(|c| c.get()) {
                    FILE_BUF.with(|b| b.borrow_mut().push(event));
                } else {
                    send_event(ShellEvent::FileEvents(vec![event]));
                }
            }
        }
    }
    // Обработанные сообщения шины: документация WM_WTSSESSION_CHANGE /
    // WM_POWERBROADCAST / WM_CLIPBOARDUPDATE не требует специального
    // возвращаемого значения — 0.
    LRESULT(0)
}

/// Флаш накопителя коалессинга: забрать батч (borrow — только на take,
/// responder зовём вне borrow_mut, схема monitor.rs) и отправить непустой.
/// Вызывается из wndproc по WM_TIMER (TIMER_ID_COALESCE).
fn flush_file_events() {
    let batch = FILE_BUF.with(|b| std::mem::take(&mut *b.borrow_mut()));
    if !batch.is_empty() {
        send_event(ShellEvent::FileEvents(batch));
    }
}

/// Доставить событие подписчику (RESPONDER — thread_local, см. блок выше).
fn send_event(event: ShellEvent) {
    RESPONDER.with(|r| {
        if let Some(responder) = r.borrow().as_ref() {
            responder(event);
        }
    });
}
