//! Режим десктопа (T15, SPEC §7.4): встройка канваса в WorkerW — за
//! иконками рабочего стола и перед обоями.
//!
//! Модуль разбит на чистое ядро (этот файл — кроссплатформенные типы и
//! логика, тестируется на любой ОС) и cfg(windows)-дочерние модули с
//! Win32-механикой: `hierarchy` (детект Progman/DefView/WorkerW, RECIPES
//! R1/R4), `attach` (встройка и стиль-скраббинг, R2/R3), `monitor`
//! (WinEventHook + DPI-поллинг, R6/R10). Все unsafe — только в дочерних
//! модулях, с SAFETY-комментариями (AGENTS.md).

/// Стратегия встраивания — выбор по рантайм-детекту иерархии окон, НЕ по
/// номеру сборки Windows (SPEC §7.4, таблица стратегий).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbedStrategy {
    /// Win10 / Win11 ≤ 23H2: SetParent на top-level WorkerW (0x052C →
    /// Explorer порождает WorkerW позади SHELLDLL_DefView).
    Classic,
    /// Win11 24H2/25H2: WS_EX_LAYERED-ребёнок Progman, Z-order между
    /// SHELLDLL_DefView (иконки, сверху) и WorkerW (обои, снизу).
    Raised,
}

/// Чистое событие shell-монитора (модуль `monitor`) в event loop
/// приложения. Хэндлы сюда не попадают — только решения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopEvent {
    /// WorkerW разрушен (WinEventHook R6 — основной канал; поллинг
    /// IsWindow — резерв). Реакция — `recovery_action`.
    WorkerWDestroyed,
    /// GetDpiForWindow изменился: после репарентинга winit-события
    /// ScaleFactorChanged НЕ приходят (RECIPES R10) → свой поллинг.
    DpiChanged { dpi: u32 },
}

/// Прямоугольник в ФИЗИЧЕСКИХ пикселях (виртуальный экран, MONITORINFO).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ScreenRect {
    /// Конструктор из left/top/right/bottom (границы как Win32-RECT:
    /// right/bottom — первый пиксель ЗА прямоугольником).
    pub fn from_ltrb(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// Объединение прямоугольников мониторов (EnumDisplayMonitors → union).
/// Пустой список → None.
pub fn union_rects(rects: &[ScreenRect]) -> Option<ScreenRect> {
    todo!("T15-A: fold по max/min сторонам; пусто -> None")
}

// Win32-константы стилей — локальные копии значений WinUser.h: этот чистый
// модуль не зависит от windows-crate (canvas-shell компилируется на Linux).
// cfg(windows)-модули сверяют значения с windows-crate debug_assert'ами
// (см. attach.rs / hierarchy.rs).
pub const WS_CHILDWINDOW: u32 = 0x4000_0000; // WS_CHILD: окно-ребёнок родителя
pub const WS_CLIPSIBLINGS: u32 = 0x0400_0000; // клиппинг о братьях — снять (R3)
pub const WS_EX_ACCEPTFILES: u32 = 0x0000_0010; // shell-дроп до нашей логики (R3)
pub const WS_EX_APPWINDOW: u32 = 0x0004_0000; // Alt+Tab/таскбар (R3: снять)
pub const WS_EX_WINDOWEDGE: u32 = 0x0000_0100; // окно «исчезает» из WorkerW (R3: снять)
pub const WS_EX_NOACTIVATE: u32 = 0x0800_0000; // не активируется кликом (до первого клика)
pub const WS_EX_LAYERED: u32 = 0x0008_0000; // R2 шаг 2: ДО SetParent на Raised
pub const WS_EX_NOREDIRECTIONBITMAP: u32 = 0x0200_0000; // R1-маркер raised на Progman

/// Ожидаемые стили окна после скраббинга (R3): верифицируются ПЕРЕЧИТЫ-
/// ВАНИЕМ после репарентинга — библиотеки окон перезаписывают стили
/// асинхронно (урок tao/Seelen).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StylePlan {
    pub style: u32,
    pub exstyle: u32,
}

/// Поле стиля для диагностики несоответствия.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleField {
    Style,
    ExStyle,
}

/// Несоответствие фактических стилей плану (причина фолбэка, R14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleMismatch {
    pub field: StyleField,
    pub expected: u32,
    pub actual: u32,
}

/// План стиль-скраббинга перед SetParent (RECIPES R3):
/// `style |= WS_CHILDWINDOW`, `style &= !WS_CLIPSIBLINGS`,
/// `exstyle &= !(WS_EX_APPWINDOW | WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES)`
/// — Alt+Tab не показывает окно, дроп не перехватывается shell-слоем,
/// WINDOWEDGE не «выбрасывает» окно из WorkerW. Всегда
/// `exstyle |= WS_EX_NOACTIVATE` (TASKS T15: до первого клика).
/// Для Raised дополнительно `exstyle |= WS_EX_LAYERED` (R2 шаг 2).
/// Идемпотентен; посторонние биты не трогает.
pub fn plan_style_scrub(style: u32, exstyle: u32, raised: bool) -> StylePlan {
    todo!("T15-A")
}

/// Верификация: фактические (перечитанные GWL_STYLE/GWL_EXSTYLE) стили
/// обязаны точно совпадать с планом. Отличие → StyleMismatch с полем.
pub fn verify_styles(style: u32, exstyle: u32, plan: &StylePlan) -> Result<(), StyleMismatch> {
    todo!("T15-A")
}

/// Действие по разрушению WorkerW (RECIPES R2, симметрия восстановления).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Ничего (мы не встроены / фолбэк).
    None,
    /// Raised: перевыполнить только Z-order (шаги 4–5 attach), без
    /// re-parent — R2: «полный reset не нужен».
    ReZOrder,
    /// Classic: полный re-attach (SetParent на новый WorkerW).
    FullReattach,
}

/// Выбор действия по текущей стратегии. R2: raised → ReZOrder,
/// classic → FullReattach, не встроены → None.
pub fn recovery_action(attached: Option<EmbedStrategy>) -> RecoveryAction {
    todo!("T15-A")
}

/// DPI (96 = 100%) → winit-scale (f64, как window.scale_factor()).
pub fn dpi_to_scale(dpi: u32) -> f64 {
    todo!("T15-A: dpi / 96.0")
}

/// Детект WorkerW после 0x052C: retry 10 × 100 мс (RECIPES R1, Seelen).
pub const DETECT_RETRIES: u32 = 10;
pub const DETECT_RETRY_DELAY_MS: u64 = 100;
/// Резервный канал watch: поллинг IsWindow(родителей) (RECIPES R6).
pub const PARENT_POLL_MS: u32 = 2000;
/// DPI-поллинг после репарентинга (RECIPES R10: 500–1000 мс).
pub const DPI_POLL_MS: u32 = 500;

#[cfg(windows)]
pub mod attach;
#[cfg(windows)]
pub mod hierarchy;
#[cfg(windows)]
pub mod monitor;

#[cfg(test)]
mod tests {
    use super::*;

    // TODO(T15-A): тесты по плану docs/plans/T15-desktop-embed.md §4.1:
    // union_rects, plan_style_scrub, verify_styles, recovery_action,
    // dpi_to_scale, инварианты констант.
}
