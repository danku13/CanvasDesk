//! Внешние интеграции десктопа (T17-D): ShellExecute-запуск, второй
//! экземпляр, автозапуск из HKCU Run (TASKS T17 / SPEC §7.4 п.7, §9).
//!
//! - `open_file`: двойной клик по файловой ноде → ShellExecuteEx с
//!   SEE_MASK_INVOKEIDLIST — поведение «как в Explorer» (контекстное
//!   меню ассоциации: открыть/перейти/свойства).
//! - `spawn_window_instance`: «Открыть канвас» — второй экземпляр
//!   нашего exe в оконном режиме с текущим путём (план §8.3).
//! - Автозапуск: HKCU\Software\Microsoft\Windows\CurrentVersion\Run,
//!   значение CanvasDesk = «"…exe" --desktop» (REG_SZ, UTF-16 +
//!   терминатор в байтах — план §7).
//!
//! Файл смешанный: строка-команда/константы — чистые (тесты Linux),
//! Win32/реестр — cfg(windows). Зона воркера T17-D: реализация по плану
//! docs/plans/T17-desktop-polish.md §3 (interop.rs) — публичные
//! сигнатуры заморожены координатором.

/// Путь реестра автозапуска (SPEC §9): HKCU + этот подключ.
pub const AUTOSTART_RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

/// Имя значения автозапуска (одно на приложение).
pub const AUTOSTART_VALUE: &str = "CanvasDesk";

/// Аргумент запуска десктоп-режима (дублирует CLI парсинг main.rs).
pub const AUTOSTART_ARG: &str = "--desktop";

#[cfg(windows)]
use std::path::Path;

/// Строка-команда значения реестра: кавычки вокруг пути exe ВСЕГДА
/// (пробелы в пути) + ` --desktop`. Чистая функция (тесты Linux).
pub fn autostart_command(exe: &str) -> String {
    todo!("T17-D")
}

/// Открыть файл ассоциацией «как в Explorer» (SPEC §7.4 п.7):
/// ShellExecuteExW, fMask = SEE_MASK_INVOKEIDLIST (= 12: задействует
/// контекстное меню ассоциации — поведение двойного клика Explorer).
/// Провал — Err(строка) → warn вызывающего (R14: деградация без
/// падения).
#[cfg(windows)]
pub fn open_file(path: &Path) -> Result<(), String> {
    todo!("T17-D")
}

/// «Открыть канвас» из десктоп-меню: запустить второй экземпляр
/// процесса в ОКОННОМ режиме (без --desktop) с данным канвасом —
/// ShellExecuteW(open, current_exe, args = путь, SW_SHOWNORMAL).
/// Провал — warn (R14); результат не критичен для вызывающего.
#[cfg(windows)]
pub fn spawn_window_instance(canvas_path: &Path) {
    todo!("T17-D")
}

/// Автозапуск включён (значение CanvasDesk в HKCU Run существует и
/// непусто)? Любая ошибка реестра → false (деградация R14 — меню без
/// галочки, но живо).
#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    todo!("T17-D")
}

/// Установить/снять автозапуск (идемпотентно): enable → RegCreateKeyExW
/// + RegSetValueExW(REG_SZ, autostart_command(current_exe)); disable →
/// RegDeleteValueW (отсутствие значения — успех, не ошибка). Отказы —
/// Err(строка) → warn (R14). RegCloseKey — Drop-гуард на всех путях.
#[cfg(windows)]
pub fn set_autostart(enable: bool) -> Result<(), String> {
    todo!("T17-D")
}
