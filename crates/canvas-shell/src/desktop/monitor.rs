//! Shell-монитор режима десктопа (T15, RECIPES R6/R10): WinEventHook на
//! разрушение WorkerW + поллинг-резерв + DPI-поллинг.
//!
//! Сервис-поток с собственным GetMessageW-циклом (паттерн WatchService
//! T10; WINEVENT_OUTOFCONTEXT доставляет колбэки ТОЛЬКО в поток с
//! message loop — R6). События уходят подписчику через responder —
//! в приложение это AppEvent::Desktop через EventLoopProxy.
//! Задача чистой реализации (GPL/AGPL — не копируем, RECIPES §0).
//!
//! Архитектура потока: окон НЕТ — «пустой» message-only цикл. Команды
//! приходят по mpsc, спящий в GetMessageW поток будит
//! PostThreadMessageW(WM_APP_WAKE) («будильник»; тело команды читается
//! try_recv-дрейном). Тики — потоковые SetTimer(None, id, …): SetTimer с
//! hwnd=None привязывает таймер к потоку, WM_TIMER приходит прямо в
//! GetMessageW (выбор SetTimer вместо счёта тиков по
//! MsgWaitForMultipleObjectsEx зафиксирован планом T15 §3 — проще и
//! точнее). Весь mutable-статус (цели Watch, responder) — thread_local:
//! колбэк WinEventHook с WINEVENT_OUTOFCONTEXT и WM_TIMER доставляются в
//! поток, установивший их (R6), поэтому RefCell без блокировок безопасен
//! by construction — обращений из других потоков нет.
//!
//! DispatchMessageW не вызывается: у потока нет окон, все сообщения —
//! потоковые (WM_APP_WAKE/WM_QUIT от PostThreadMessageW, WM_TIMER от
//! потоковых таймеров); DispatchMessage их только выбросил бы.

use std::cell::RefCell;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, GetWindowThreadProcessId, IsWindow, KillTimer, PostThreadMessageW, SetTimer,
    EVENT_OBJECT_DESTROY, MSG, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT, WM_APP, WM_QUIT, WM_TIMER,
};

use super::attach::window_dpi;
use super::{DPI_POLL_MS, PARENT_POLL_MS};

/// Подписчик на события десктопа (в main — замыкание с EventLoopProxy).
pub type DesktopResponder = Arc<dyn Fn(super::DesktopEvent) + Send + Sync>;

/// «Будильник» поток-монитора: команда уже лежит в mpsc, поток спит в
/// GetMessageW — PostThreadMessageW(WM_APP_WAKE) выводит его на дрен.
/// Диапазон WM_APP..WM_APP+0xBFFF зарезервирован приложениями для
/// приватных сообщений (не конфликтует с системой).
const WM_APP_WAKE: u32 = WM_APP + 0x15;

/// Идентификаторы потоковых таймеров (см. модульный комментарий):
/// DPI-поллинг (R10) и поллинг-резерв родителей (R6).
const DPI_TIMER_ID: usize = 1;
const PARENT_TIMER_ID: usize = 2;

/// Команды сервису (mpsc; поток читает их в message loop —
/// PostThreadMessageW-«будильник» или PeekMessage-дрейн перед GetMessage).
#[derive(Debug)]
pub enum MonitorCommand {
    /// Установить/перенавесить слежку: WinEventHook(EVENT_OBJECT_DESTROY,
    /// поток WorkerW через GetWindowThreadProcessId, WINEVENT_OUTOFCONTEXT)
    /// и запомнить хэндлы для поллинга (IsWindow worker_w — резерв R6,
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

// SAFETY: HWND — значение хэндла ядра (указатель без Rust-семантики
// владения), а не ссылка: команды с HWND пересекают потоки как копии
// значений. Все Win32-вызовы по этим хэндлам (IsWindow,
// GetWindowThreadProcessId, GetDpiForWindow) поток-агностичны;
// thread-аффинные операции (SetWinEventHook/SetTimer/GetMessageW)
// выполняются только внутри монитор-потока над локальными копиями.
// Разыменования как указателей нет ни в одном пути. (Идиома R13:
// unsafe impl Send с обоснованием; иначе mpsc-канал команд нельзя
// передать в поток-монитор.)
unsafe impl Send for MonitorCommand {}

/// Цели одного Watch — thread-local состояние потока-монитора (см.
/// TARGETS): копии значений HWND без владения, валидность перечитывается
/// IsWindow/GetDpiForWindow перед каждым использованием.
struct MonitorTargets {
    /// Progman — корень иерархии: IsWindow-резерв (умер → полный re-attach).
    progman: HWND,
    /// WorkerW — цель hook'а EVENT_OBJECT_DESTROY и основного поллинга.
    worker_w: HWND,
    /// Наше окно — DPI-поллинг GetDpiForWindow (R10).
    ours: HWND,
    /// Последний замер DPI; 0 = бейзлайн ещё не снят (первый замер после
    /// Watch фиксируется молча — событие только при реальном изменении).
    last_dpi: u32,
    /// WorkerWDestroyed уже отправлен (анти-дубль: hook и поллинг-резерв
    /// могут сработать по одному разрушению); сбрасывается новым Watch.
    destroyed_sent: bool,
}

thread_local! {
    /// Цели слежки текущего Watch. Thread_local — ключевое упрощение R6:
    /// колбэк WinEventHook (WINEVENT_OUTOFCONTEXT) вызывается в потоке,
    /// установившем hook (наш монитор-поток), WM_TIMER — в потоке
    /// таймеров; гонок с другими потоками нет, RefCell достаточен.
    static TARGETS: RefCell<Option<MonitorTargets>> = const { RefCell::new(None) };

    /// Responder (клон Arc) для колбэка hook'а: extern "system" fn не
    /// захватывает Rust-состояние — событие уходит через этот thread_local.
    static RESPONDER: RefCell<Option<DesktopResponder>> = const { RefCell::new(None) };
}

/// Сервис-монитор: поток + канал команд. spawn — до знания хэндлов
/// (в main() рядом с прочими сервисами), Watch — после attach.
#[derive(Clone)]
pub struct DesktopMonitorService {
    tx: Sender<MonitorCommand>,
    /// Windows thread id монитор-потока (handshake при spawn) — адрес
    /// «будильника» PostThreadMessageW в command(). 0 — поток не жив
    /// (не стартовал/умер): команды копятся и теряются молча.
    thread_id: u32,
}

impl DesktopMonitorService {
    /// Запуск потока-владельца hook'ов и таймеров. События — в responder.
    pub fn spawn(responder: DesktopResponder) -> Self {
        let (tx, rx) = mpsc::channel::<MonitorCommand>();
        // Handshake-канал: поток сообщает свой Windows thread id — он нужен
        // command() для «будильника», а GetCurrentThreadId доступен только
        // изнутри самого потока (см. current_thread_id).
        let (ready_tx, ready_rx) = mpsc::channel::<u32>();
        // Builder, а не std::thread::spawn: провал создания потока — warn и
        // деградация без паники в вызывающем потоке (урок T14-A).
        let spawned = std::thread::Builder::new()
            .name("desktop-monitor".to_string())
            .spawn(move || monitor_loop(rx, ready_tx, responder));
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
                tracing::warn!(%err, "desktop-монитор не запущен — слежение за WorkerW отключено");
                // rx и ready_tx умерли вместе с замыканием: send в tx теперь
                // ошибается, command() игнорирует молча (деградация R14).
                Self { tx, thread_id: 0 }
            }
        }
    }

    /// Послать команду (send-ошибка — молча: прототип watcher.rs T10 —
    /// приложение закрывается, монитору уже нечего делать).
    pub fn command(&self, command: MonitorCommand) {
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

/// Тело монитор-потока: handshake → потоковые таймеры → message-only цикл
/// GetMessageW. Выход — WM_QUIT: снятие hook и таймеров (финальный клинап
/// в одной точке — идемпотентен и для Shutdown, и для внешнего WM_QUIT).
fn monitor_loop(rx: Receiver<MonitorCommand>, ready: Sender<u32>, responder: DesktopResponder) {
    // SAFETY: см. current_thread_id — вызов в своём потоке, без предусловий.
    let self_id = unsafe { current_thread_id() };
    // Ошибка send в одноразовый handshake-канал — молча: rx может умереть
    // только вместе с сервисом (spawn тогда получит 0 — деградация).
    let _ = ready.send(self_id);
    // Responder — thread_local: колбэк hook'а отправляет события, не
    // имея параметров-состояний (SAFETY-схема в модульном комментарии).
    RESPONDER.with(|r| *r.borrow_mut() = Some(responder));
    // Потоковые таймеры: DPI-поллинг (R10) и поллинг-резерв (R6). Цели ещё
    // не заданы (Watch придёт после attach) — тики до Watch — no-op.
    // SAFETY: hwnd=None привязывает таймеры к ЭТОМУ потоку, callback нет
    // (TIMERPROC=None) → WM_TIMER придёт в GetMessageW-цикл ниже. Отказ
    // установки (возврат 0) — warn, живём на остальных каналах слежения.
    if unsafe { SetTimer(None, DPI_TIMER_ID, DPI_POLL_MS, None) } == 0 {
        tracing::warn!("DPI-таймер не установлен — смена DPI не отслеживается");
    }
    // SAFETY: то же, что DPI-таймер выше (потоковый, без callback).
    if unsafe { SetTimer(None, PARENT_TIMER_ID, PARENT_POLL_MS, None) } == 0 {
        tracing::warn!("таймер поллинга родителей не установлен — резерв R6 отключён");
    }

    // Текущий hook (перевешивается каждым Watch); владеет им цикл.
    let mut hook: Option<HWINEVENTHOOK> = None;
    let mut msg = MSG::default();
    loop {
        // SAFETY: msg — локальный POD-буфер; фильтры 0..0 = все сообщения
        // потока. BOOL трактуем по документации GetMessageW: -1 = ошибка
        // (битое сообщение — warn и продолжаем), 0 = WM_QUIT, иначе
        // сообщение получено. Сравнение по .0: as_bool() счёл бы -1
        // «истиной».
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
            // Тики потоковых таймеров: wParam — идентификатор таймера.
            WM_TIMER => match msg.wParam.0 {
                DPI_TIMER_ID => on_dpi_tick(),
                PARENT_TIMER_ID => on_parent_tick(),
                _ => {}
            },
            // «Будильник» из command(): дрен всех накопленных команд.
            WM_APP_WAKE => {
                while let Ok(command) = rx.try_recv() {
                    match command {
                        MonitorCommand::Shutdown => {
                            // WM_QUIT себе: GetMessageW вернёт 0, финальный
                            // клинап — в точке выхода цикла (одна копия
                            // логики). Цели снимаем, чтобы уже стоящие в
                            // очереди WM_TIMER не отправляли событий
                            // закрывающемуся приложению.
                            // SAFETY: self_id — идентификатор ЭТОГО потока
                            // (получен внутри него); повторный WM_QUIT
                            // безвреден — GetMessageW вернёт 0 лишь раз.
                            let _ = unsafe {
                                PostThreadMessageW(self_id, WM_QUIT, WPARAM(0), LPARAM(0))
                            };
                            TARGETS.with(|t| *t.borrow_mut() = None);
                            // Команды после Shutdown не обрабатываем.
                            break;
                        }
                        MonitorCommand::Watch {
                            progman,
                            worker_w,
                            ours,
                        } => {
                            // Перенавесить hook (WorkerW пересоздан — поток
                            // Explorer мог смениться, R6) + обновить цели.
                            unhook(&mut hook);
                            hook = install_hook(worker_w);
                            set_targets(progman, worker_w, ours);
                        }
                    }
                }
            }
            // Посторонние потоковые сообщения — игнорируем (чужих окон нет).
            _ => {}
        }
    }
    // Выход: снять hook и убить таймеры (идемпотентно: после Shutdown hook
    // уже None; KillTimer несуществующего таймера — ошибка, молча).
    unhook(&mut hook);
    // SAFETY: таймеры установлены этим же потоком, id — наши константы.
    let _ = unsafe { KillTimer(None, DPI_TIMER_ID) };
    // SAFETY: см. выше — тот же поток, та же схема.
    let _ = unsafe { KillTimer(None, PARENT_TIMER_ID) };
}

/// Навесить hook EVENT_OBJECT_DESTROY на поток WorkerW (R6: «на поток
/// WorkerW», не на процесс). Отказ (нет потока / хэндл невалиден) → None —
/// живём на поллинг-резерве (R6: hook — основной канал, поллинг — резерв).
/// Порядок аргументов SetWinEventHook сверен по исходникам windows-0.62.2
/// (UI/Accessibility/mod.rs:222): eventmin, eventmax, hmodwineventproc,
/// pfnwineventproc, idprocess, idthread, dwflags — фильтра по idObject в
/// API НЕТ, отсев идёт в колбэке (OBJID_WINDOW/id_child).
fn install_hook(worker_w: HWND) -> Option<HWINEVENTHOOK> {
    // Поток-владелец окна WorkerW — адрес hook-фильтра.
    // SAFETY: worker_w — хэндл из Watch (приложение получило его детектом
    // иерархии); GetWindowThreadProcessId — чистое чтение без побочных
    // эффектов, невалидный хэндл → 0 (ветка ниже).
    let thread_id = unsafe { GetWindowThreadProcessId(worker_w, None) };
    if thread_id == 0 {
        tracing::warn!("поток WorkerW не определён — слежение только поллингом (R6-резерв)");
        return None;
    }
    // SAFETY: WINEVENT_OUTOFCONTEXT — колбэк вызывается в нашем процессе
    // (DLL не инжектится в Explorer), hmod для этого режима не нужен
    // (None). Фильтры: единственное событие EVENT_OBJECT_DESTROY, поток
    // WorkerW (idProcess=0 — без ограничения по процессу, режем по
    // потоку). Невалидный возврат — отказ установки (не ошибка процесса):
    // warn и поллинг-резерв продолжает слежение.
    let hook = unsafe {
        SetWinEventHook(
            EVENT_OBJECT_DESTROY,
            EVENT_OBJECT_DESTROY,
            None,
            Some(win_event_callback),
            0,
            thread_id,
            WINEVENT_OUTOFCONTEXT,
        )
    };
    if hook.is_invalid() {
        tracing::warn!("SetWinEventHook не установлен — слежение только поллингом (R6-резерв)");
        return None;
    }
    Some(hook)
}

/// Снять текущий hook (повторный Watch перевешивает; Shutdown/выход —
/// финал). Отказ UnhookWinEvent — warn: хук мог умереть вместе с потоком
/// Explorer, критичного ничего нет.
fn unhook(hook: &mut Option<HWINEVENTHOOK>) {
    if let Some(old) = hook.take() {
        // SAFETY: хэндл получен SetWinEventHook в ЭТОМ потоке и снимается
        // в нём же; take() исключает двойное снятие.
        if let Err(err) = unsafe { UnhookWinEvent(old) }.ok() {
            tracing::warn!(%err, "снятие WinEventHook не удалось");
        }
    }
}

/// Обновить thread_local-цели слежки. Новый Watch = новый WorkerW после
/// восстановления: анти-дубль destroy и DPI-бейзлайн стартуют заново.
fn set_targets(progman: HWND, worker_w: HWND, ours: HWND) {
    TARGETS.with(|t| {
        *t.borrow_mut() = Some(MonitorTargets {
            progman,
            worker_w,
            ours,
            last_dpi: 0,
            destroyed_sent: false,
        });
    });
}

/// Колбэк SetWinEventHook (тип WINEVENTPROC windows-0.62.2: HWINEVENTHOOK,
/// u32 event, HWND, i32 id_object, i32 id_child, u32 thread, u32 time).
/// События WINEVENT_OUTOFCONTEXT приходят в поток, установивший hook —
/// монитор-поток: TARGETS/RESPONDER — его thread_local, захватов Rust
/// через аргументы нет (только примитивы + HWND-копия).
unsafe extern "system" fn win_event_callback(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    // Отсев шума: только целые окна (OBJID_WINDOW, id_child=0) и только
    // destroy (hook и так подписан на одно событие — проверка защитная).
    if event != EVENT_OBJECT_DESTROY || id_object != OBJID_WINDOW.0 || id_child != 0 {
        return;
    }
    // Цель + анти-дубль — одним borrow_mut; событие отправляем уже вне
    // borrow (responder не должен видеть занятый RefCell).
    let destroyed = TARGETS.with(|t| {
        if let Some(targets) = t.borrow_mut().as_mut() {
            if hwnd == targets.worker_w && !targets.destroyed_sent {
                targets.destroyed_sent = true;
                return true;
            }
        }
        false
    });
    if destroyed {
        send_event(super::DesktopEvent::WorkerWDestroyed);
    }
}

/// Тик DPI-поллинга (R10): GetDpiForWindow(ours); первый замер после
/// Watch — бейзлайн (молча), дальше — событие только при изменении.
/// Гистерезис не нужен: DPI ОС меняется кратными 25% шагами (96/120/144/…).
fn on_dpi_tick() {
    let event = TARGETS.with(|t| {
        // RefMut держим именованным: временная в выражении умирает в конце
        // инструкции (E0716), а borrow нужен до конца замыкания.
        let mut borrow = t.borrow_mut();
        let targets = borrow.as_mut()?; // нет Watch / после Shutdown — no-op
                                        // SAFETY: ours — хэндл из Watch; GetDpiForWindow — чистое чтение,
                                        // невалидный (умерший) hwnd → 0 (ветка ниже), не ошибка.
        let dpi = window_dpi(targets.ours);
        if dpi == 0 {
            return None; // окно потерялось — не трогаем бейзлайн
        }
        if targets.last_dpi == 0 {
            targets.last_dpi = dpi; // бейзлайн первого замера — молча
            None
        } else if dpi != targets.last_dpi {
            targets.last_dpi = dpi;
            Some(super::DesktopEvent::DpiChanged { dpi })
        } else {
            None
        }
    });
    if let Some(event) = event {
        send_event(event);
    }
}

/// Тик поллинг-резерва (R6): IsWindow(worker_w) и IsWindow(progman) —
/// смерть любого из них означает разрушение иерархии десктопа →
/// WorkerWDestroyed (восстановление — задача приложения: recovery_action).
fn on_parent_tick() {
    let event = TARGETS.with(|t| {
        // RefMut — именованный (см. on_dpi_tick): живёт до конца замыкания.
        let mut borrow = t.borrow_mut();
        let targets = borrow.as_mut()?; // нет Watch / после Shutdown — no-op
        if targets.destroyed_sent {
            return None; // уже отправлено (hook или прошлый тик)
        }
        // SAFETY: progman/worker_w — хэндлы-копии из Watch; IsWindow —
        // чистая проверка, невалидный хэндл → false без ошибок.
        let worker_alive = unsafe { IsWindow(Some(targets.worker_w)) }.as_bool();
        // SAFETY: то же — progman из того же Watch.
        let progman_alive = unsafe { IsWindow(Some(targets.progman)) }.as_bool();
        if worker_alive && progman_alive {
            return None;
        }
        targets.destroyed_sent = true;
        Some(super::DesktopEvent::WorkerWDestroyed)
    });
    if let Some(event) = event {
        send_event(event);
    }
}

/// Доставить событие подписчику (responder — thread_local, см. RESPONDER).
fn send_event(event: super::DesktopEvent) {
    RESPONDER.with(|r| {
        if let Some(responder) = r.borrow().as_ref() {
            responder(event);
        }
    });
}

/// Текущий Windows thread id (Win32::System::Threading). Координатор T15-E
/// включил фичу `Win32_System_Threading` в workspace-Cargo.toml — штатный
/// биндинг вместо временной `windows::core::link!`-декларации воркера T15-D.
// SAFETY: тривиальный геттер без предусловий и побочных эффектов; вызов
// только внутри монитор-потока — берём СВОЙ идентификатор.
unsafe fn current_thread_id() -> u32 {
    windows::Win32::System::Threading::GetCurrentThreadId()
}
