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

/// Число пунктов меню — длина таблицы [`COMMANDS`] и cfg(windows)-таблицы
/// подписей `menu_labels` (индекс = смещение id от [`MENU_ID_BASE`]).
const MENU_LEN: usize = 5;

/// Команды в порядке следования пунктов меню: 100→OpenCanvas, …, 104→Exit
/// (по порядку объявления enum). Единый источник порядка для маппинга
/// [`command_from_id`] и построения меню в cfg(windows)-`popup`.
const COMMANDS: [DesktopMenuCommand; MENU_LEN] = [
    DesktopMenuCommand::OpenCanvas,
    DesktopMenuCommand::NewTextFile,
    DesktopMenuCommand::ToggleIcons,
    DesktopMenuCommand::ToggleAutostart,
    DesktopMenuCommand::Exit,
];

/// Маппинг id TrackPopupMenu → команда (чистая часть, тесты Linux):
/// 100..104 → команды по порядку объявления enum, прочее → None.
pub fn command_from_id(id: usize) -> Option<DesktopMenuCommand> {
    // checked_sub вместо вычитания: id < MENU_ID_BASE (в т.ч. 0) → None без
    // риска переполнения usize; дальше — индекс таблицы, за границей → None.
    let offset = id.checked_sub(MENU_ID_BASE)?;
    COMMANDS.get(offset).copied()
}

#[cfg(windows)]
use windows::core::{w, PCWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, PostMessageW, SetForegroundWindow,
    TrackPopupMenu, HMENU, MF_CHECKED, MF_STRING, TPM_BOTTOMALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
    WM_NULL,
};

/// RAII-владелец HMENU: `DestroyMenu` на ВСЕХ путях выхода `popup` (план §3
/// — гейт ревью; владение меню наше с момента `CreatePopupMenu`). Drop без
/// паник: отказ — только warn (R14).
#[cfg(windows)]
struct MenuGuard {
    hmenu: HMENU,
}

#[cfg(windows)]
impl Drop for MenuGuard {
    fn drop(&mut self) {
        // SAFETY: hmenu создан CreatePopupMenu в этом же потоке; MenuGuard —
        // единственная копия хэндла (двойного DestroyMenu не бывает); в
        // момент drop меню НЕ отображается: TrackPopupMenu либо уже вернулся
        // (модальный цикл закрыл меню), либо не вызывался вовсе.
        if let Err(err) = unsafe { DestroyMenu(self.hmenu) } {
            tracing::warn!(%err, "DestroyMenu не выполнен — дескриптор меню утекает (R14)");
        }
    }
}

/// Подписи пунктов (порядок — [`COMMANDS`]; русские подписи плана §3).
/// w!-литералы — статические null-terminated UTF-16, живут вечно — годятся
/// как PCWSTR для AppendMenuW без копий.
#[cfg(windows)]
fn menu_labels() -> [PCWSTR; MENU_LEN] {
    [
        w!("Открыть канвас"),
        w!("Новый текстовый файл"),
        w!("Показать системные иконки"),
        w!("Запускать с Windows"),
        w!("Выход"),
    ]
}

/// Добавить все 5 пунктов меню (план §3): MF_STRING, id 100..104 (база +
/// индекс); галочки MF_CHECKED — «Показать системные иконки» при
/// icons_hidden (семантика пункта инверсная: галочка = «сейчас скрыты»),
/// «Запускать с Windows» при autostart_on. Отказ AppendMenuW → Err —
/// вызывающий деградирует в None (DestroyMenu — на MenuGuard вызывающего).
#[cfg(windows)]
fn append_menu_items(
    menu: HMENU,
    icons_hidden: bool,
    autostart_on: bool,
) -> windows::core::Result<()> {
    // zip: подпись и команда идут из одной ячейки таблиц — рассинхрон длины
    // исключён конструктивно (обе [..; MENU_LEN]).
    for (offset, (&command, label)) in COMMANDS.iter().zip(menu_labels()).enumerate() {
        // Галочка пункта — по команде (не по индексу): ToggleIcons ←
        // icons_hidden, ToggleAutostart ← autostart_on, остальные — без.
        let checked = match command {
            DesktopMenuCommand::ToggleIcons => icons_hidden,
            DesktopMenuCommand::ToggleAutostart => autostart_on,
            _ => false,
        };
        let flags = if checked {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        // SAFETY: menu — хэндл из CreatePopupMenu вызывающего (жив: MenuGuard
        // ещё не дропнут); label — статический w!-литерал (null-terminated
        // UTF-16, копирования нет); id = MENU_ID_BASE + offset — уникальные
        // 100..104 (usize по сигнатуре AppendMenuW).
        unsafe { AppendMenuW(menu, flags, MENU_ID_BASE + offset, label) }?;
    }
    Ok(())
}

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
    // ---- 1. Меню: CreatePopupMenu; владение хэндлом наше ---------------
    // MenuGuard держит DestroyMenu на всех путях выхода ниже (гейт ревью).
    // SAFETY: без предусловий; Err (практически недостижим) — warn и
    // деградация без меню (R14).
    let menu = match unsafe { CreatePopupMenu() } {
        Ok(hmenu) => MenuGuard { hmenu },
        Err(err) => {
            tracing::warn!(%err, "CreatePopupMenu отказал — контекстное меню недоступно (R14)");
            return None;
        }
    };
    // ---- 2. Пункты 100..104 с галочками состояний -----------------------
    if let Err(err) = append_menu_items(menu.hmenu, icons_hidden, autostart_on) {
        tracing::warn!(%err, "AppendMenuW отказал — контекстное меню недоступно (R14)");
        return None; // MenuGuard → DestroyMenu
    }
    // ---- 3. Позиция: экранные координаты курсора ------------------------
    // GetCursorPos сам даёт физические экранные координаты (конверсия
    // масштаба не нужна — план §7). Отказ — warn + (0,0): меню остаётся
    // валидным, откроется в левом верхнем углу (деградация R14).
    let mut point = POINT { x: 0, y: 0 };
    // SAFETY: point — локальный POD-буфер; чистое чтение позиции курсора.
    // Биндинг windows-0.62.2 возвращает Result<()> (Err = Win32 FALSE).
    if let Err(err) = unsafe { GetCursorPos(&mut point) } {
        tracing::warn!(%err, "GetCursorPos отказал — меню откроется в (0,0)");
    }
    // ---- 4. Tray-идиома dismiss'а (план §3 п.4) --------------------------
    // SetForegroundWindow ДО TrackPopupMenu — без этого меню не закрывается
    // кликом мимо (классика tray-меню). Отказ игнорируем: передний план
    // может законно не выдаться (фокус у чужого процесса) — меню всё равно
    // показывается, dismiss дублируется WM_NULL-хвостом.
    // SAFETY: hwnd — окно этого процесса (передан из on_right_button);
    // активация — без владения и без побочных выделений, отказ не фатален.
    let _ = unsafe { SetForegroundWindow(hwnd) };
    // SAFETY: menu.hmenu жив (guard не дропнут); hwnd — окно этого потока
    // (popup зовётся из event loop). TPM_RETURNCMD — id выбранного пункта
    // вернётся ЗНАЧЕНИЕМ (воронка WM_COMMAND не строится, §8.1);
    // TPM_RIGHTBUTTON — выбор любой кнопкой мыши; TPM_BOTTOMALIGN — меню
    // раскрывается вверх от точки (ПКМ обычно у нижней кромки экрана);
    // nreserved/prcrect = None — без зарезервированных аргументов и без
    // ограничения областью. Возврат: 0 — отмена/отказ, иное — id пункта.
    let choice = unsafe {
        TrackPopupMenu(
            menu.hmenu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            point.x,
            point.y,
            None,
            hwnd,
            None,
        )
    };
    // ---- 5. Хвост tray-идиомы: WM_NULL после модального цикла ------------
    // Даёт потоку-владельцу доработать очередь сообщений после dismiss'а
    // меню. Результат игнорируем — документированная идиома (план §3 п.4).
    // SAFETY: hwnd — окно этого процесса/потока; WM_NULL — no-op-сообщение
    // (w/l — нули); очередь не блокируется (пост, не send).
    let _ = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };
    // ---- 6. Выбор → команда ----------------------------------------------
    // BOOL == 0 — отмена (клик мимо / Esc / системный отказ). Отрицательное
    // значение невозможно по контракту, но cast в usize на всякий случай
    // уводит мусор в None веткой ниже (мусорный id → warn, R14).
    if choice.0 == 0 {
        return None; // MenuGuard → DestroyMenu
    }
    match command_from_id(choice.0 as usize) {
        Some(command) => Some(command),
        None => {
            tracing::warn!(id = choice.0, "TrackPopupMenu вернул неизвестный id пункта");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 100..104 → все 5 команд по порядку объявления enum: id = MENU_ID_BASE
    /// + индекс, без пропусков и перестановок.
    #[test]
    fn command_from_id_maps_all_five_in_order() {
        assert_eq!(command_from_id(100), Some(DesktopMenuCommand::OpenCanvas));
        assert_eq!(command_from_id(101), Some(DesktopMenuCommand::NewTextFile));
        assert_eq!(command_from_id(102), Some(DesktopMenuCommand::ToggleIcons));
        assert_eq!(
            command_from_id(103),
            Some(DesktopMenuCommand::ToggleAutostart)
        );
        assert_eq!(command_from_id(104), Some(DesktopMenuCommand::Exit));
    }

    /// Границы и мусор: id < 100 и > 104 → None (99, 105, 0, usize::MAX,
    /// первый id за диапазоном).
    #[test]
    fn command_from_id_out_of_range_is_none() {
        for id in [0usize, 99, 105, usize::MAX, MENU_ID_BASE + MENU_LEN] {
            assert_eq!(command_from_id(id), None, "id {id} не должен маппиться");
        }
    }

    /// Таблица-инвариант: ровно MENU_LEN команд, без повторов — порядок id
    /// не может «съехать» дубликатами (маппинг идёт по индексу).
    #[test]
    fn commands_table_full_and_unique() {
        assert_eq!(COMMANDS.len(), MENU_LEN);
        for (i, &a) in COMMANDS.iter().enumerate() {
            for &b in &COMMANDS[i + 1..] {
                assert_ne!(a, b, "дубль команды в таблице пунктов меню");
            }
        }
    }

    /// MENU_ID_BASE == 100: диапазон 100..104 не пересекается с системными
    /// кодами WM_COMMAND (защита от дрейфа константы).
    #[test]
    fn menu_id_base_is_hundred() {
        assert_eq!(MENU_ID_BASE, 100);
    }
}
