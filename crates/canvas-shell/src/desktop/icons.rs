//! Скрытие системных иконок десктопа + sentinel-краш-сейф (T17-A,
//! RECIPES R5/R9, SPEC §7.4 п.5).
//!
//! R5 (идемпотентность): `WM_COMMAND 0x7402` на SHELLDLL_DefView —
//! ПЕРЕКЛЮЧАТЕЛЬ, а не установка. Перед отправкой читаем фактическое
//! состояние `SHGetSetSettings(SSF_HIDEICONS)` и шлём toggle только при
//! расхождении с желаемым — иначе повторный запуск ВКЛЮЧИТ иконки
//! вместо выключения. Запись через `fSet=TRUE` в Win10+ не работает —
//! не пытаться (проверено Lively). Исходное состояние фиксируем в
//! [`IconGuard`] и восстанавливаем при штатном выходе.
//!
//! Краш-сейф: если мы скрыли иконки, создаём sentinel-файл в
//! `default_cache_dir()`; kill -9 обходит Drop-страховку — следующий
//! запуск (любой режим, [`crash_recovery`]) видит sentinel и
//! форс-восстанавливает иконки.
//!
//! R9: рефреш десктопа — ТОЛЬКО `InvalidateRect+UpdateWindow(DefView)`;
//! `SPI_SETDESKWALLPAPER` под ЗАПРЕТОМ (на raised desktop разрушает
//! WorkerW, SPEC §7.4 / RECIPES R9).
//!
//! Файл смешанный (рекомендация T15-A): чистые решения/протокол
//! sentinel — кроссплатформенны (тесты на Linux); Win32-механика —
//! cfg(windows)-блоки. Зона воркера T17-A: реализация по плану
//! docs/plans/T17-desktop-polish.md §3 (icons.rs) — публичные
//! сигнатуры заморожены координатором.

use std::path::{Path, PathBuf};

/// Бит `fHideIcons` в `_bitfield1` SHELLSTATEA (ShlObj.h, бит 7):
/// чистое декодирование состояния из битового поля (packed-структуру
/// читаем по значению — ссылки на поля packed — UB, план §7).
pub const HIDE_ICONS_BIT: i32 = 0x80;

/// Команда-toggle иконок на SHELLDLL_DefView (Lively DesktopUtil.cs,
/// RECIPES R5): WM_COMMAND с этим параметром переключает видимость
/// иконок десктопа.
pub const TOGGLE_ICONS_COMMAND: usize = 0x7402;

/// Декодировать fHideIcons из `_bitfield1` SHELLSTATEA (бит 7).
pub fn hide_icons_state(bitfield1: i32) -> bool {
    todo!("T17-A")
}

/// Нужно ли слать toggle (R5): только при расхождении фактического
/// состояния с желаемым (XOR).
pub fn should_toggle(hidden_now: bool, want_hidden: bool) -> bool {
    todo!("T17-A")
}

/// Путь sentinel-файла краш-сейва в каталоге приложения (каталог —
/// `canvas_shell::default_cache_dir()`, единый источник T6/T14).
pub fn sentinel_path(dir: &Path) -> PathBuf {
    todo!("T17-A")
}

/// Sentinel существует — прошлая сессия умерла, не восстановив иконки.
pub fn sentinel_exists(dir: &Path) -> bool {
    todo!("T17-A")
}

/// Создать sentinel (содержимое — UTC-штамп для диагностики; логика
/// читает только факт существования). Ошибка записи — деградация:
/// краш-сейф не сработает, но работу не рушит (R14).
pub fn sentinel_create(dir: &Path) {
    todo!("T17-A")
}

/// Удалить sentinel (штатное восстановление). Отсутствие файла — не
/// ошибка (идемпотентность).
pub fn sentinel_remove(dir: &Path) {
    todo!("T17-A")
}

#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Владелец скрытия иконок (R5): помнит исходное состояние, шлёт
/// toggle только при расхождении, восстанавливает идемпотентно.
/// Drop-страховка от unwind (паники); kill -9 покрыт sentinel'ом.
#[cfg(windows)]
pub struct IconGuard {
    /// DefView-слой иконок (из DesktopHierarchy, T15).
    def_view: HWND,
    /// Иконки скрыли МЫ (исходное состояние = показаны): только тогда
    /// restore трогает систему. Пользовательское «скрыто» не наше.
    hidden_by_us: bool,
    /// Каталог sentinel-файла (default_cache_dir приложения).
    sentinel_dir: Option<PathBuf>,
}

#[cfg(windows)]
impl IconGuard {
    /// Зафиксировать исходное состояние иконок (до скрытия). Отказ
    /// чтения SHGetSetSettings — warn + guard в no-op-режиме (R14).
    pub fn capture(def_view: HWND) -> Self {
        todo!("T17-A")
    }

    /// Скрыть иконки (идемпотентно; R5 — toggle только при
    /// расхождении; sentinel создаём только если реально тогглили).
    pub fn hide(&mut self) {
        todo!("T17-A")
    }

    /// Показать иконки (toggle-пункт меню; зеркально hide).
    pub fn show(&mut self) {
        todo!("T17-A")
    }

    /// Восстановить исходное состояние (штатный выход): идемпотентно,
    /// sentinel снимается. Выхода из режима не делает — только иконки.
    pub fn restore(&mut self) {
        todo!("T17-A")
    }

    /// Иконки сейчас скрыты нами (для галочки пункта меню).
    pub fn hidden_by_us(&self) -> bool {
        todo!("T17-A")
    }
}

/// Drop-страховка от unwind: то же, что restore (паники в event loop);
/// kill -9 обходит Drop — на то sentinel (план §3).
#[cfg(windows)]
impl Drop for IconGuard {
    fn drop(&mut self) {
        todo!("T17-A")
    }
}

/// Рефреш десктопа после toggle (R9): InvalidateRect(DefView, None,
/// false) + UpdateWindow(DefView). SPI_SETDESKWALLPAPER НЕ вызывать
/// никогда (raised-разрушение WorkerW, RECIPES R9).
#[cfg(windows)]
pub fn refresh_desktop(def_view: HWND) {
    todo!("T17-A")
}

/// Краш-сейф при старте (TASKS T17): sentinel существует и иконки
/// сейчас скрыты → форс-восстановление (toggle + refresh + снять
/// sentinel) → true; sentinel без скрытых иконок → просто снять (юзер
/// показал сам) → false. Вызывается из main() ДО attach, без гейта
/// --desktop (план §8.6). Возвращает факт восстановления (для лога).
#[cfg(windows)]
pub fn crash_recovery() -> bool {
    todo!("T17-A")
}
