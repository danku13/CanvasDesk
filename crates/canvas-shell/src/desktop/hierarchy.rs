//! Детект иерархии окон десктопа (T15, RECIPES R1/R4, SPEC §7.4).
//!
//! Две схемы (Spy++-дампы из RECIPES §2):
//! - Классическая (Win10 / Win11 ≤ 23H2): top-level WorkerW, у которого
//!   child SHELLDLL_DefView; целевой WorkerW — следующий top-level sibling.
//! - Raised (Win11 24H2/25H2): Progman с WS_EX_NOREDIRECTIONBITMAP,
//!   SHELLDLL_DefView и WorkerW — ДЕТи Progman (WorkerW ниже DefView).
//! Детект — объединение подходов Lively/Seelen (R1): маркер raised — один
//! вызов GetWindowLongPtrW; далее — целевой поиск по схеме. Спавн 0x052C —
//! идемпотентный (R4): только если WorkerW отсутствует.
//! Чистая реализация по описанию механики (RECIPES §0: GPL/AGPL-код
//! не копируется).

use super::ScreenRect;
use thiserror::Error;
use windows::Win32::Foundation::HWND;

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
    todo!("T15-B: GetShellWindow; GetClassNameW == \"Progman\"")
}

/// Raised-детект (R1, маркер Lively): GWL_EXSTYLE Progman содержит
/// WS_EX_NOREDIRECTIONBITMAP. false — классическая схема.
pub fn is_raised(progman: HWND) -> bool {
    todo!("T15-B: GetWindowLongPtrW(GWL_EXSTYLE) & WS_EX_NOREDIRECTIONBITMAP")
}

/// Детект полной иерархии БЕЗ спавна: is_raised → поиск по схеме
/// (raised: FindWindowEx(progman, …); classic: EnumWindows → DefView →
/// sibling WorkerW — идиома R13). Ошибки — HierarchyError.
pub fn detect(progman: HWND) -> Result<DesktopHierarchy, HierarchyError> {
    todo!("T15-B")
}

/// Детект + идемпотентный спавн WorkerW (R4): WorkerW отсутствует →
/// PostMessageW(progman, 0x052C, 0xD, 0x1) (ТОЛЬКО при отсутствии — иначе
/// Explorer снесёт существующий, бесконечный цикл create/destroy, Seelen
/// ловил) → retry-детект DETECT_RETRIES × DETECT_RETRY_DELAY_MS.
/// Присутствует → детект без отправки.
pub fn ensure_worker_w(progman: HWND) -> Result<DesktopHierarchy, HierarchyError> {
    todo!("T15-B: detect; если нет WorkerW -> 0x052C -> retry; §8 плана: PostMessageW")
}

/// Виртуальный экран: EnumDisplayMonitors (идиома R13) + GetMonitorInfoW
/// (rcMonitor) → union (super::union_rects). Нет мониторов → None.
pub fn virtual_screen_rect() -> Option<ScreenRect> {
    todo!("T15-B")
}

// TODO(T15-B): debug_assert-сверка локальных констант super::WS_* с
// windows-crate (Win32::UI::WindowsAndMessaging) — значения WinUser.h;
// enum-обёртки Win32 — по идиоме RECIPES R13 (SAFETY на каждый unsafe).
