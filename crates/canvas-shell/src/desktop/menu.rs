//! Системное контекстное меню десктопа (T17-C, SPEC §7.4 п.6):
//! нативное Win32-меню по ПКМ на пустом месте канваса в --desktop.
//!
//! Пункты: Открыть канвас / Новый текстовый файл / Показать системные
//! иконки (toggle, галочка = «сейчас скрыты») / Запускать с Windows
//! (toggle, галочка по факту реестра) / Выход.
//!
//! Отступление §8.1–8.2 плана: перехват ПКМ — на уровне winit-события
//! on_right_button (SPEC «WM_RBUTTONUP» покрывается им), выбор команды —
//! TrackPopupMenu(TPM_RETURNCMD) возвращает id напрямую, воронка
//! WM_COMMAND не строится. Меню системное (нативность в desktop-режиме),
//! рисованное меню T7 (цвета нод) не трогаем — зоны не пересекаются.
//!
//! Файл смешанный: enum/маппинг — чистые (тесты на Linux), popup —
//! cfg(windows). Зона воркера T17-C: реализация по плану
//! docs/plans/T17-desktop-polish.md §3 (menu.rs) — публичные
//! сигнатуры заморожены координатором.

/// Команда контекстного меню десктопа (выбор пользователя).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopMenuCommand {
    /// Открыть текущий канвас вторым экземпляром в оконном режиме
    /// (interop::spawn_window_instance, план §8.3).
    OpenCanvas,
    /// Создать текстовый файл в каталоге канваса + ноду в точке ПКМ
    /// (план §8.4; вотчер T10/поиск T14 подхватят автоматически).
    NewTextFile,
    /// Показать/скрыть системные иконки (toggle; галочка меню = «сейчас
    /// скрыты» — инверсная семантика пункта «Показать»).
    ToggleIcons,
    /// Включить/выключить автозапуск с Windows (HKCU Run; галочка
    /// меню = автозапуск включён).
    ToggleAutostart,
    /// Штатный выход: форс-сейв + восстановление иконок + завершение.
    Exit,
}

/// Младший id пунктов меню (100..104; TrackPopupMenu возвращает id с
/// TPM_RETURNCMD — диапазон не пересекается с системными кодами).
pub const MENU_ID_BASE: usize = 100;

/// Маппинг id TrackPopupMenu → команда (чистая часть, тесты Linux):
/// 100..104 → команды по порядку объявления enum, прочее → None.
pub fn command_from_id(id: usize) -> Option<DesktopMenuCommand> {
    todo!("T17-C")
}

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Показать меню в текущей позиции курсора и вернуть выбор.
///
/// `icons_hidden` — галочка пункта «Показать системные иконки» (сейчас
/// скрыты — семантика инверсная, план §3); `autostart_on` — галочка
/// «Запускать с Windows». None — отмена (клик мимо/Esc).
///
/// Реализация (план §3): CreatePopupMenu → AppendMenuW ×5 (MF_STRING,
/// MF_CHECKED для включённых галочек) → GetCursorPos → tray-идиома
/// dismiss'а: SetForegroundWindow(hwnd) → TrackPopupMenu(TPM_RETURNCMD
/// | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN) → PostMessageW(hwnd, WM_NULL) →
/// DestroyMenu ВСЕГДА (Drop-гуард, владение HMENU наше).
#[cfg(windows)]
pub fn popup(hwnd: HWND, icons_hidden: bool, autostart_on: bool) -> Option<DesktopMenuCommand> {
    todo!("T17-C")
}
