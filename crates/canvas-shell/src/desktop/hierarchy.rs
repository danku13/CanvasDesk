//! Детект иерархии окон десктопа (T15, RECIPES R1/R4, SPEC §7.4).
//!
//! Две схемы (Spy++-дампы из RECIPES §2):
//! - Классическая (Win10 / Win11 ≤ 23H2): top-level WorkerW, у которого
//!   child SHELLDLL_DefView; целевой WorkerW — следующий top-level sibling.
//! - Raised (Win11 24H2/25H2): Progman с WS_EX_NOREDIRECTIONBITMAP,
//!   SHELLDLL_DefView и WorkerW — ДЕТи Progman (WorkerW ниже DefView).
//!
//! Детект — объединение подходов Lively/Seelen (R1): маркер raised — один
//! вызов GetWindowLongPtrW; далее — целевой поиск по схеме. Спавн 0x052C —
//! идемпотентный (R4): только если WorkerW отсутствует.
//! Чистая реализация по описанию механики (RECIPES §0: GPL/AGPL-код
//! не копируется).

use std::thread;
use std::time::Duration;

use super::{
    EmbedStrategy, ScreenRect, DETECT_RETRIES, DETECT_RETRY_DELAY_MS, WS_EX_NOREDIRECTIONBITMAP,
};
use thiserror::Error;
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, FindWindowExW, GetClassNameW, GetShellWindow, GetWindowLongPtrW, IsWindow,
    PostMessageW, GWL_EXSTYLE,
};

/// Недокументированное сообщение Progman: команда shell породить WorkerW
/// (RECIPES R4; известно с 2013). Параметры — из рецепта: WPARAM=0xD,
/// LPARAM=0x1. Слать ТОЛЬКО при отсутствии WorkerW — иначе Explorer снесёт
/// существующий и пересоздаст (цикл create/destroy, Seelen ловил).
const WM_SPAWN_WORKERW: u32 = 0x052C;

/// Ошибка детекта иерархии → фолбэк на оконный режим (R14).
#[derive(Debug, Error)]
pub enum HierarchyError {
    /// Progman не найден (нет shell? RDP-сессия без десктопа?).
    #[error("Progman не найден — рабочий стол недоступен")]
    ProgmanNotFound,
    /// 0x052C отправлен, но WorkerW не появился за retry-окно (R1: 10×100 мс).
    #[error("WorkerW не появился после 0x052C за {0} мс")]
    WorkerWNotSpawned(u64),
    /// SHELLDLL_DefView не найден (неожидаемая иерархия — не описана в
    /// RECIPES/SPEC → непроверенная зона, только фолбэк, SPEC §11.5).
    #[error("SHELLDLL_DefView не найден — неизвестная иерархия десктопа")]
    DefViewNotFound,
    /// Хэндл Progman потерял валидность между шагами.
    #[error("Progman потерял валидность между шагами детекта")]
    ProgmanInvalidated,
}

/// Найденная иерархия десктопа: хэндлы + выбранная стратегия. Хэндлы —
/// копии значений HWND (не владеющие), валидность перечитывается перед
/// каждым использованием.
#[derive(Debug, Clone, Copy)]
pub struct DesktopHierarchy {
    /// Progman — корень иерархии (GetShellWindow).
    pub progman: HWND,
    /// SHELLDLL_DefView — слой иконок: наша Z-order-граница «сверху».
    pub def_view: HWND,
    /// Целевой WorkerW: обои (classic: родитель; raised: нижний сосед).
    pub worker_w: HWND,
    /// Стратегия по фактической иерархии (не по номеру сборки).
    pub strategy: super::EmbedStrategy,
}

/// HWND Progman: GetShellWindow + проверка класса "Progman".
pub fn find_progman() -> Result<HWND, HierarchyError> {
    // SAFETY: GetShellWindow — чтение глобального хэндла окна shell, без
    // параметров и побочных эффектов; unsafe — только из-за FFI-декларации.
    let hwnd = unsafe { GetShellWindow() };
    // HWND(0)/null — «десктопа нет» (shell не Explorer, headless-RDP)
    if hwnd.is_invalid() {
        return Err(HierarchyError::ProgmanNotFound);
    }
    // Класс обязаны подтвердить: подменённый/чужой корень ломает все
    // допущения R1/R2 об иерархии ниже него
    if !class_is(hwnd, "Progman") {
        return Err(HierarchyError::ProgmanNotFound);
    }
    Ok(hwnd)
}

/// Raised-детект (R1, маркер Lively): GWL_EXSTYLE Progman содержит
/// WS_EX_NOREDIRECTIONBITMAP. false — классическая схема.
pub fn is_raised(progman: HWND) -> bool {
    // Сверка локальной копии mod.rs (модуль не зависит от windows-crate)
    // с реальной константой WinUser.h из windows-crate: 0x0200_0000.
    // Ловит рассинхрон копий при апгрейде крейта (R1-маркер).
    debug_assert_eq!(
        WS_EX_NOREDIRECTIONBITMAP,
        windows::Win32::UI::WindowsAndMessaging::WS_EX_NOREDIRECTIONBITMAP.0
    );
    // SAFETY: progman — хэндл; висячий (окно умерло) даёт результат 0 без
    // UB; GetWindowLongPtrW — чтение поля окна, побочных эффектов нет.
    // GWL_EXSTYLE — DWORD в младших 32 битах LONG_PTR, старшие отбрасываем
    // приведением к u32 (знакового расширения маска не чувствует).
    let exstyle = unsafe { GetWindowLongPtrW(progman, GWL_EXSTYLE) } as u32;
    exstyle & WS_EX_NOREDIRECTIONBITMAP != 0
}

/// Детект полной иерархии БЕЗ спавна: is_raised → поиск по схеме
/// (raised: FindWindowEx(progman, …); classic: EnumWindows → DefView →
/// sibling WorkerW — идиома R13). Ошибки — HierarchyError.
pub fn detect(progman: HWND) -> Result<DesktopHierarchy, HierarchyError> {
    // Прогман мог умереть с момента find_progman (рестарт Explorer):
    // валидность перечитываем ДО поиска (R1 — хэндлы копии, не ссылки)...
    if !window_valid(progman) {
        return Err(HierarchyError::ProgmanInvalidated);
    }
    let found = if is_raised(progman) {
        find_raised(progman)
    } else {
        find_classic(progman)
    };
    // ...и ПОСЛЕ: если поиск не удался, а progman по пути умер — честная
    // причина ProgmanInvalidated, а не DefViewNotFound/WorkerWNotSpawned
    match found {
        Ok(hierarchy) => Ok(hierarchy),
        Err(_) if !window_valid(progman) => Err(HierarchyError::ProgmanInvalidated),
        Err(err) => Err(err),
    }
}

/// Детект + идемпотентный спавн WorkerW (R4): WorkerW отсутствует →
/// PostMessageW(progman, 0x052C, 0xD, 0x1) (ТОЛЬКО при отсутствии — иначе
/// Explorer снесёт существующий, бесконечный цикл create/destroy, Seelen
/// ловил) → retry-детект DETECT_RETRIES × DETECT_RETRY_DELAY_MS.
/// Присутствует → детект без отправки.
pub fn ensure_worker_w(progman: HWND) -> Result<DesktopHierarchy, HierarchyError> {
    match detect(progman) {
        // WorkerW есть — 0x052C НЕ шлём (R4): повторная отправка заставляет
        // Explorer снести и пересоздать WorkerW → наши дети гибнут →
        // remount → снова 0x052C → бесконечный цикл create/destroy
        Ok(hierarchy) => Ok(hierarchy),
        // WorkerW отсутствует (до спавна, поле 0) — идемпотентный spawn
        Err(HierarchyError::WorkerWNotSpawned(0)) => spawn_worker_w(progman),
        // Прочее (DefView пропал / progman умер) 0x052C не лечится — наружу
        Err(err) => Err(err),
    }
}

/// Спавн WorkerW сообщением 0x052C + retry-детект (R1: Seelen 10×100 мс —
/// окно появляется в очереди shell не мгновенно).
fn spawn_worker_w(progman: HWND) -> Result<DesktopHierarchy, HierarchyError> {
    // План §8.1: PostMessageW, НЕ SendMessageTimeout (SPEC §7.4) —
    // асинхронная постановка в очередь Explorer без ожидания обработки:
    // сразу уходим в retry-детект, результат тот же, hang-риск ниже.
    // SAFETY: progman валиден (только что проверен detect'ом); сообщение
    // 0x052C (WPARAM=0xD, LPARAM=0x1, R4) — команда породить WorkerW;
    // PostMessageW не блокирует вызывающий поток. Ошибка отправки
    // (переполнение очереди и т.п.) не ветвим: retry-детект ниже сам
    // разрулит — итог WorkerWNotSpawned за полное retry-окно.
    let _ = unsafe { PostMessageW(Some(progman), WM_SPAWN_WORKERW, WPARAM(0xD), LPARAM(0x1)) };
    for _ in 0..DETECT_RETRIES {
        // R1: пауза перед КАЖДЫМ повтором — Explorer обрабатывает 0x052C
        // асинхронно, мгновенной реакции нет
        thread::sleep(Duration::from_millis(DETECT_RETRY_DELAY_MS));
        match detect(progman) {
            Ok(hierarchy) => return Ok(hierarchy),
            // ещё не появился — к следующей попытке
            Err(HierarchyError::WorkerWNotSpawned(0)) => {}
            // серьёзная поломка (DefView пропал, progman умер) — не
            // дожимаем остаток retry-окна
            Err(err) => return Err(err),
        }
    }
    Err(HierarchyError::WorkerWNotSpawned(
        DETECT_RETRIES as u64 * DETECT_RETRY_DELAY_MS,
    ))
}

/// Raised-иерархия (R1, Spy++-дамп): DefView и WorkerW — прямые дети
/// Progman, WorkerW ниже DefView в Z-order.
fn find_raised(progman: HWND) -> Result<DesktopHierarchy, HierarchyError> {
    let def_view = find_child(Some(progman), None, w!("SHELLDLL_DefView"))
        .ok_or(HierarchyError::DefViewNotFound)?;
    // WorkerW может ещё не существовать: его порождает 0x052C (R4,
    // ensure_worker_w); 0 в поле ошибки = «до спавна»
    let worker_w = find_child(Some(progman), None, w!("WorkerW"))
        .ok_or(HierarchyError::WorkerWNotSpawned(0))?;
    Ok(DesktopHierarchy {
        progman,
        def_view,
        worker_w,
        strategy: EmbedStrategy::Raised,
    })
}

/// Классическая иерархия (R1): top-level владелец DefView (обычно WorkerW),
/// целевой WorkerW — его следующий top-level sibling (FindWindowEx
/// c child_after=владелец). Обход top-level окон — идиома R13.
fn find_classic(progman: HWND) -> Result<DesktopHierarchy, HierarchyError> {
    let mut owner_def_view: Option<(HWND, HWND)> = None;
    for_each_top_level(|top| {
        if let Some(def_view) = find_child(Some(top), None, w!("SHELLDLL_DefView")) {
            owner_def_view = Some((top, def_view));
        }
        // Полный обход без прерывания: top-level окон — десятки, поиск
        // копеечный; прерывание (FALSE из колбэка) в windows-rs неотличимо
        // от системной ошибки перечисления — не создаём двусмысленности
        true
    });
    let (owner, def_view) = owner_def_view.ok_or(HierarchyError::DefViewNotFound)?;
    // Целевой WorkerW — следующий sibling ПОСЛЕ владельца DefView:
    // parent=None (топ-уровень), child_after=owner стартует ниже него
    // в Z-order (R1: FindWindowEx(0, owner, "WorkerW", 0))
    let worker_w =
        find_child(None, Some(owner), w!("WorkerW")).ok_or(HierarchyError::WorkerWNotSpawned(0))?;
    Ok(DesktopHierarchy {
        progman,
        def_view,
        worker_w,
        strategy: EmbedStrategy::Classic,
    })
}

/// Виртуальный экран: EnumDisplayMonitors (идиома R13) + GetMonitorInfoW
/// (rcMonitor) → union (super::union_rects). Нет мониторов → None.
pub fn virtual_screen_rect() -> Option<ScreenRect> {
    let mut rects: Vec<ScreenRect> = Vec::new();
    let lparam = LPARAM(&mut rects as *mut Vec<ScreenRect> as isize);
    // SAFETY: перечисление всех мониторов (HDC=None, clip=None — без
    // фильтров); трамплин вызывается синхронно на этом же стеке/потоке,
    // rects живёт всё время вызова (см. unsafe impl Send у обёрток ниже);
    // возвращаемый FALSE (= Err) означал бы прерывание (наши трамплины
    // всегда TRUE) либо системную ошибку — обе эквивалентны «список
    // частичен», union строим по фактически собранным.
    let _ = unsafe { EnumDisplayMonitors(None, None, Some(enum_monitors_proc), lparam) };
    super::union_rects(&rects)
}

// ---------------------------------------------------------------------------
// Приватные Win32-хелперы (SAFETY на каждый unsafe, AGENTS.md правило 6)
// ---------------------------------------------------------------------------

/// Окно ещё живо (IsWindow): копия хэндла — не доказательство валидности.
fn window_valid(hwnd: HWND) -> bool {
    // SAFETY: чистая проверка валидности хэндла; висячий/чужой HWND даёт
    // FALSE без UB; unsafe — только из-за FFI-декларации.
    unsafe { IsWindow(Some(hwnd)) }.as_bool()
}

/// Класс окна совпадает с ожидаемым (GetClassNameW; сравнение UTF-16
/// код-единиц, без аллокаций).
fn class_is(hwnd: HWND, expected: &str) -> bool {
    // 64 код-единицы: классы shell короткие ("Progman", "WorkerW");
    // более длинный чужой класс обрежется и просто не совпадёт (false)
    let mut buf = [0u16; 64];
    // SAFETY: hwnd — хэндл (висячий → len 0 → false, не UB); buf — слайс
    // фиксированной длины, GetClassNameW пишет в него не более buf.len()
    // код-единиц (терминатор при успехе входит в лимит Win32).
    let len = unsafe { GetClassNameW(hwnd, &mut buf) };
    if len <= 0 {
        return false;
    }
    // len — скопированные символы БЕЗ терминатора
    expected
        .encode_utf16()
        .eq(buf[..len as usize].iter().copied())
}

/// Поиск окна по классу через FindWindowExW: Err windows-rs (= HWND 0) —
/// «не найдено» → None. parent=None — от топ-уровня; child_after — поиск
/// среди братьев ниже указанного в Z-order (R1: sibling WorkerW).
fn find_child(parent: Option<HWND>, child_after: Option<HWND>, class: PCWSTR) -> Option<HWND> {
    // SAFETY: class — литерал w! с null-терминатором; lpszwindow=null —
    // «любое имя окна»; parent/child_after — хэндлы или None (висячие
    // дают Err → None, не UB); чистый поиск в иерархии без побочных
    // эффектов.
    unsafe { FindWindowExW(parent, child_after, class, PCWSTR::null()).ok() }
}

// ---------------------------------------------------------------------------
// Enum-обёртки по идиоме RECIPES R13: boxed closure едет через LPARAM,
// статический extern "system" трамплин распаковывает его обратно; наружу —
// safe API. Одно место unsafe на семейство операций.
// ---------------------------------------------------------------------------

/// Контекст EnumWindows (R13): замыкание в Box, указатель передаётся
/// через LPARAM. Живёт только на стеке вызывающего потока; `'a` —
/// реальный borrow замыкания (короче вызова).
struct EnumWindowsCtx<'a> {
    f: Box<dyn FnMut(HWND) -> bool + 'a>,
}

// SAFETY: EnumWindows синхронен — трамплин вызывается на стеке вызывающего
// потока до возврата; указатель из LPARAM не покидает этот стек и не
// передаётся другим потокам. Замыкание само по себе не требует
// потокобезопасности (вызывается ровно одним потоком); HWND — плоское
// значение-копия без владения. unsafe impl формален: FFI-граница
// (LPARAM = isize) не отслеживает трейт-границы Rust.
unsafe impl Send for EnumWindowsCtx<'_> {}
// SAFETY: &EnumWindowsCtx никому не выдаётся (только &mut на собственном
// стеке через трамплин того же потока) — расшаривания ссылок нет; Sync
// декларативно завершает R13-обёртку для будущих reinterpret-кастов.
unsafe impl Sync for EnumWindowsCtx<'_> {}

/// Трамплин EnumWindows (R13): LPARAM → &mut EnumWindowsCtx → вызов
/// замыкания. Возврат FALSE прервал бы перечисление — транслируем
/// решение замыкания напрямую (наши колбэки всегда true, см. find_classic).
extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: lparam — &mut EnumWindowsCtx, положенный туда
    // for_each_top_level на ЭТОМ же стеке/потоке; EnumWindows синхронно
    // вызывает трамплин до своего возврата, Box жив всё это время; других
    // источников lparam у трамплина нет (private-модуль). 'static в касте
    // — формальность FFI-типа: lifetime-параметры в рантайме отсутствуют,
    // указатель используется только внутри одного синхронного вызова и не
    // переживает реальный (более короткий) borrow контекста.
    let ctx = unsafe { &mut *(lparam.0 as *mut EnumWindowsCtx<'static>) };
    (ctx.f)(hwnd).into()
}

/// Safe-обёртка EnumWindows: вызывает `f` для каждого top-level окна.
/// Полный обход (замыкание true — продолжать); прерывание не используем
/// (FALSE в windows-rs неотличим от ошибки, см. find_classic).
fn for_each_top_level(f: impl FnMut(HWND) -> bool) {
    let mut ctx = EnumWindowsCtx { f: Box::new(f) };
    let lparam = LPARAM(&mut ctx as *mut EnumWindowsCtx as isize);
    // SAFETY: трамплин соответствует WNDENUMPROC; ctx живёт на этом стеке
    // всё время вызова, EnumWindows синхронен (см. unsafe impl Send у
    // EnumWindowsCtx); Err (= FALSE) означает прерывание (наши трамплины
    // не прерывают) либо системную ошибку перечисления — обе трактовки
    // для вызывающего эквивалентны «обход завершён по факту», результат
    // сознательно не проверяем (недостающие окна дают лишь
    // DefViewNotFound → деградация R14).
    let _ = unsafe { EnumWindows(Some(enum_windows_proc), lparam) };
}

/// Трамплин EnumDisplayMonitors (R13): LPARAM → &mut Vec<ScreenRect>;
/// каждый монитор — GetMonitorInfoW → rcMonitor. Ошибка одного монитора —
/// пропуск (не падение); всегда TRUE (прерывание не нужно, мониторов
/// мало и нужен полный список для union).
extern "system" fn enum_monitors_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _clip: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    // SAFETY: lparam — &mut Vec<ScreenRect> со стека virtual_screen_rect
    // (того же потока/стека — см. SAFETY там); перечисление синхронно,
    // Vec жив всю итерацию.
    let rects = unsafe { &mut *(lparam.0 as *mut Vec<ScreenRect>) };
    // cbSize обязателен (иначе FALSE); MONITORINFO (не MONITORINFOEXW)
    // достаточно: нужен только rcMonitor — хвост с именем устройства не
    // запрашиваем, транспонирование структур не требуется (cbSize сам
    // выбирает формат заполнения)
    let mut info = MONITORINFO {
        cbSize: core::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: info корректно инициализирована (cbSize = sizeof);
    // hmonitor валиден — выдан перечислителем в этом же вызове; FALSE
    // (ошибка конкретного монитора) — просто пропускаем его, не падаем.
    let ok = unsafe { GetMonitorInfoW(hmonitor, &mut info) }.as_bool();
    if ok {
        let rc = info.rcMonitor;
        rects.push(ScreenRect::from_ltrb(rc.left, rc.top, rc.right, rc.bottom));
    }
    true.into()
}
