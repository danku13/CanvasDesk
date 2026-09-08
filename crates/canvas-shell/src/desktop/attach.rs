//! Встройка окна канваса в иерархию десктопа (T15, RECIPES R2/R3).
//!
//! Порядок Raised (ядро T15, R2 — «TryAttachToDesktop» Lively):
//! 1. scrub: стиль-план R3 через SetWindowLongPtrW;
//! 2. WS_EX_LAYERED + SetLayeredWindowAttributes(bAlpha=255) СТРОГО ДО
//!    SetParent (layered-стили на детях Progman «silently dropped» после
//!    репарентинга; bAlpha=255 — полная непрозрачность, требование
//!    Microsoft для DX blt present);
//! 3. SetParent(hwnd, progman) — parent = Progman, НЕ WorkerW;
//! 4. SetWindowPos(hwnd, hWndInsertAfter = def_view, NOMOVE|NOSIZE|
//!    NOACTIVATE) — встаём в Z-order сразу ПОД слоем иконок;
//! 5. ensure_worker_w_z_order — WorkerW обязан остаться ПОСЛЕДНИМ
//!    ребёнком Progman (иначе обои перекроют нас; Lively: «Unexpected
//!    WorkerW Z-order» — случай реальный).
//! Classic: scrub → SetParent(hwnd, worker_w). Затем (обе схемы): окно на
//! весь виртуальный экран + верификация стилей перечитыванием (R3).
//! Любая ошибка шага → AttachError → фолбэк на обычное окно (R14).
//! Чистая реализация по описанию механики (RECIPES §0).

use super::hierarchy::{DesktopHierarchy, HierarchyError};
use thiserror::Error;
use windows::Win32::Foundation::HWND;

/// Ошибка встройки — любая ведёт к фолбэку на оконный режим (R14).
#[derive(Debug, Error)]
pub enum AttachError {
    #[error(transparent)]
    Hierarchy(#[from] HierarchyError),
    /// SetParent отказал (антивирус / кастомный shell / RDP).
    #[error("SetParent не удался")]
    SetParentFailed,
    /// SetWindowPos отказал (Z-order или экран).
    #[error("SetWindowPos не удался")]
    SetWindowPosFailed,
    /// SetLayeredWindowAttributes отказал (Raised-шаг 2).
    #[error("SetLayeredWindowAttributes не удался")]
    SetLayeredFailed,
    /// Перечитанные стили != план (R3: библиотека перезаписала — Seelen/tao).
    #[error("стили после репарентинга не совпали: {0:?}")]
    StyleMismatch(super::StyleMismatch),
}

/// Результат успешной встройки.
#[derive(Debug, Clone, Copy)]
pub struct AttachOutcome {
    pub strategy: super::EmbedStrategy,
    /// Прямоугольник, на который растянуто окно (виртуальный экран).
    pub screen: super::ScreenRect,
}

/// Полная встройка по иерархии: scrub → (Raised: layered-шаг) → SetParent →
/// (Raised: Z-order 4–5) → весь виртуальный экран → верификация (R3).
pub fn attach(
    hwnd: HWND,
    hierarchy: &DesktopHierarchy,
    screen: super::ScreenRect,
) -> Result<AttachOutcome, AttachError> {
    todo!("T15-C: порядок §файла-дока, шаги 1–5 + экран + verify")
}

/// WorkerW — последний ребёнок Progman (R2 шаг 5): GetWindow(progman,
/// GW_CHILD) → обход GW_HWNDNEXT до GW_HWNDLAST; если последний != worker_w
/// → SetWindowPos(worker_w, HWND_BOTTOM, NOACTIVATE|NOMOVE|NOSIZE).
/// Вызывается внутри attach и при refresh_z_order.
pub fn ensure_worker_w_z_order(hierarchy: &DesktopHierarchy) -> Result<(), AttachError> {
    todo!("T15-C")
}

/// Восстановление после разрушения WorkerW на Raised (R2-симметрия):
/// перевыполнить ТОЛЬКО шаги Z-order (4–5) — полный re-parent не нужен.
/// (Classic обрабатывается полным re-attach на уровне приложения.)
pub fn refresh_z_order(hwnd: HWND, hierarchy: &DesktopHierarchy) -> Result<(), AttachError> {
    todo!("T15-C: SetWindowPos(hwnd, insert_after=def_view) + ensure_worker_w_z_order")
}

/// Снять WS_EX_NOACTIVATE — первый клик пользователя по канвасу
/// (TASKS T15: «WS_EX_NOACTIVATE до первого клика»). Идемпотентен.
pub fn enable_activation(hwnd: HWND) -> Result<(), AttachError> {
    todo!("T15-C")
}

/// DPI окна через GetDpiForWindow (R10; после репарентинга
/// window.scale_factor() врёт — поллится монитором T15-D).
pub fn window_dpi(hwnd: HWND) -> u32 {
    todo!("T15-C")
}

// TODO(T15-C): debug_assert-сверка локальных констант super::WS_* с
// windows-crate; SAFETY на каждый unsafe; сообщение 0x052C НЕ здесь —
// в hierarchy (R4).
