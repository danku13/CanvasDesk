//! Декод SHChangeNotify-нотификаций (T16-D, cfg(windows), RECIPES R12):
//! lParam WM_APP_SHELL_FILE → (маска SHCNE, пара путей) → FileEvent.
//!
//! Режим доставки — SHCNRF_NewDelivery (план §8.6): lParam — HANDLE
//! нотификации; доступ — SHChangeNotification_Lock(handle, dwProcId =
//! GetCurrentProcessId(), &mut pppidl, &mut маска) → pidl-пара, закрытие —
//! SHChangeNotification_Unlock на ВСЕХ путях выхода (Drop-гуард).
//! pidl → путь: SHGetPathFromIDListW (false на не-файловых pidl —
//! legitimately None). Итоговый маппинг — чистая `shell_file_change`
//! ядра (T16-A): единая точка семантики SHCNE → FileEvent (в т.ч.
//! корзина → SHCNE_DELETE → Remove → brokenLink).
//!
//! Зона воркера T16-D: реализация по плану docs/plans/T16-shell-events.md
//! §3 (shfiles.rs). SAFETY-комментарии обязательны (AGENTS.md п.6).

use std::path::PathBuf;

use canvas_core::FileEvent;

/// Имя события для сообщения «wMsg» SHChangeNotifyRegister (значение —
/// WM_APP_SHELL_FILE из ядра; константа здесь для самодокументируемости
/// регистрации в regs.rs).
pub const SHELL_FILE_MSG_NAME: &str = "WM_APP_SHELL_FILE (CanvasDesk)";

/// Декодировать нотификацию: Lock (dwProcId = GetCurrentProcessId) →
/// маска + pidl-пара → пути → shell_file_change. None — не-файловые pidl/
/// неизвестная маска/отсутствие нужного пути (в т.ч. Lock отказал —
/// warn, R14). `wparam` — wParam сообщения (в NewDelivery-режиме не
/// несёт маски; принят параметром для диагностики).
pub fn decode(lparam: isize, wparam: usize) -> Option<FileEvent> {
    // TODO(T16-D): реализовать по плану §3.
    let _ = (lparam, wparam);
    None
}

/// pidl → путь файловой системы (SHGetPathFromIDListW, буфер 260): None
/// для не-файловых pidl (объекты пространства имён shell и пр.).
fn pidl_to_path(pidl: *const windows::Win32::UI::Shell::Common::ITEMIDLIST) -> Option<PathBuf> {
    // TODO(T16-D): реализовать по плану §3.
    let _ = pidl;
    None
}
