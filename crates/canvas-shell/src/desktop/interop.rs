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

/// Строка-команда значения реестра: кавычки вокруг пути exe ВСЕГДА
/// (пробелы в пути) + ` --desktop`. Чистая функция (тесты Linux).
pub fn autostart_command(exe: &str) -> String {
    // Кавычки всегда: HKCU Run-парсер допускает их при любом пути, а без
    // них путь с пробелами развалился бы на токены — формат безусловный
    // (SPEC §9), экранирование не нужно (кавычки в Windows-путях
    // невозможны). Аргумент — константой (единый источник с CLI main.rs).
    format!("\"{exe}\" {AUTOSTART_ARG}")
}

#[cfg(windows)]
use std::path::Path;

#[cfg(windows)]
use windows::core::{w, HSTRING, PCWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
#[cfg(windows)]
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_SZ,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    ShellExecuteExW, ShellExecuteW, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW,
};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// RAII-гуард RegCloseKey (идиома UnlockGuard T16-D / shfiles.rs):
/// закрывает ключ на ЛЮБОМ пути выхода — ранние возвраты и unwind.
/// Создаётся только по валидному хэндлу (после ERROR_SUCCESS открытия/
/// создания) — RegCloseKey по нулевому хэндлу не зовём.
#[cfg(windows)]
struct RegKeyGuard(HKEY);

#[cfg(windows)]
impl Drop for RegKeyGuard {
    fn drop(&mut self) {
        // SAFETY: self.0 — хэндл успешного RegOpenKeyExW/RegCreateKeyW из
        // того же кадра, где создан гвард; RegCloseKey обязан вызываться
        // ровно один раз на хэндл — RAII гарантирует парность на всех
        // путях выхода. Провал (битый хэндл — не воспроизводится на живом
        // ключе) — только warn: ключи HKCU OS вычищает при смерти
        // процесса, приложение продолжает работать (деградация R14).
        let err = unsafe { RegCloseKey(self.0) };
        if err != ERROR_SUCCESS {
            tracing::warn!(
                code = err.0,
                "RegCloseKey не удался — хэндл ключа реестра утёк"
            );
        }
    }
}

/// Открыть файл ассоциацией «как в Explorer» (SPEC §7.4 п.7):
/// ShellExecuteExW, fMask = SEE_MASK_INVOKEIDLIST (= 12: задействует
/// контекстное меню ассоциации — поведение двойного клика Explorer).
/// Провал — Err(строка) → warn вызывающего (R14: деградация без
/// падения).
#[cfg(windows)]
pub fn open_file(path: &Path) -> Result<(), String> {
    // Путь → нул-terminated UTF-16: HSTRING владеет буфером и живёт до
    // конца кадра — синхронный ShellExecuteExW читает его внутри вызова
    // (не сохраняет: SEE_MASK_NOCLOSEPROCESS не выставлена, hProcess
    // остаётся нулевым).
    let file = HSTRING::from(path.as_os_str());
    // Default обнуляет структуру (hwnd = null, lpParameters/lpDirectory =
    // null, Anonymous = нули); заполняем только контракт вызова.
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST,
        // null — документированное «действие по умолчанию» (open) для
        // типа файла: то же, что делает Explorer двойным кликом.
        lpVerb: PCWSTR::null(),
        lpFile: PCWSTR(file.as_ptr()),
        // nShow структуры — сырой i32 (WinUser.h); SW_SHOWNORMAL несёт
        // его значение (1) в SHOW_WINDOW_CMD-обёртке.
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    // SAFETY: info — локальная структура кадра, ShellExecuteExW пишет
    // только в неё (out-поля); file жив до возврата из функции, указатели
    // внутри структуры не переживают кадр; cbSize/fMask соответствуют
    // контракту MS (полный размер структуры, INVOKEIDLIST для
    // контекстного меню ассоциации).
    if let Err(err) = unsafe { ShellExecuteExW(&mut info) } {
        return Err(format!("ShellExecuteEx: {err}"));
    }
    Ok(())
}

/// «Открыть канвас» из десктоп-меню: запустить второй экземпляр
/// процесса в ОКОННОМ режиме (без --desktop) с данным канвасом —
/// ShellExecuteW(open, current_exe, args = путь, SW_SHOWNORMAL).
/// Провал — warn (R14); результат не критичен для вызывающего.
#[cfg(windows)]
pub fn spawn_window_instance(canvas_path: &Path) {
    // Путь текущего exe — единственный надёжный источник (не реконструкция
    // из argv: путь канваса может быть занят кавычками/пробелами). Провал
    // (удалённый бинарник, песочница) — warn + тихий отказ: пункт меню
    // не критичен для жизни приложения (R14).
    let Ok(exe) = std::env::current_exe() else {
        tracing::warn!("current_exe недоступен — второй экземпляр не запущен (R14)");
        return;
    };
    let exe_w = HSTRING::from(exe.as_os_str());
    let canvas_w = HSTRING::from(canvas_path.as_os_str());
    // SAFETY: verb/file/params — валидные нул-terminated UTF-16 (HSTRING
    // живёт до конца кадра — ShellExecuteW копирует их в порождённый
    // процесс синхронно); hwnd null — без owner-окна (родителя-модали нет);
    // directory null — рабочий каталог по умолчанию (для запуска exe
    // безразличен); SW_SHOWNORMAL — обычное видимое окно.
    let inst = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(exe_w.as_ptr()),
            PCWSTR(canvas_w.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    // Факт-фикс: windows-0.62.2 возвращает из ShellExecuteW сырой HINSTANCE,
    // а не Result — MS-контракт этого API: значение > 32 = успех, <= 32 —
    // код ошибки SE_ERR_* (WinError.h). Проверка порога канонична.
    if (inst.0 as isize) <= 32 {
        tracing::warn!(
            code = inst.0 as isize,
            "ShellExecuteW провален — канвас не открыт в окне (R14)"
        );
    }
}

/// Автозапуск включён (значение CanvasDesk в HKCU Run существует и
/// непусто)? Любая ошибка реестра → false (деградация R14 — меню без
/// галочки, но живо).
#[cfg(windows)]
pub fn autostart_enabled() -> bool {
    let key = HSTRING::from(AUTOSTART_RUN_KEY);
    let value = HSTRING::from(AUTOSTART_VALUE);
    let mut hkey = HKEY::default();
    // SAFETY: hkey — out-параметр локального кадра; при провале остаётся
    // нулевым и далее не используется (гвард не создаём); HKEY_CURRENT_USER
    // — предопределённый ключ, закрывать его не нужно; KEY_READ —
    // минимальные права под запрос значения.
    let err = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            None,
            KEY_READ,
            &mut hkey,
        )
    };
    if err != ERROR_SUCCESS {
        // «Ключа нет» — штатное состояние выключенного автозапуска: это не
        // ошибка и не warn — вызывается при каждом ПКМ-меню, спам недопустим
        // (план §3); максимум debug.
        tracing::debug!(code = err.0, "HKCU Run не открыт — автозапуск выключен");
        return false;
    }
    // RegCloseKey на всех путях выхода: RAII-гвард (план §3, идиома T16-D).
    let _close = RegKeyGuard(hkey);
    // Запрашиваем ТОЛЬКО размер (lpdata/lptype = null — документированный
    // режим RegQueryValueExW): байты команды не нужны — решение «включён»
    // принимается по факту существования непустой REG_SZ. cb — в БАЙТАХ;
    // непустая строка = 2 байта символа + 2 байта терминатора (минимум 4),
    // порог > 2 отсекает пустую/только-терминатор запись.
    let mut cb: u32 = 0;
    // SAFETY: cb — out-параметр локального кадра; hkey — валидный хэндл
    // открытого ключа (гвард живёт в кадре); lpdata/lptype null — режим
    // «только размер», записи нет; lpreserved null по контракту.
    let err = unsafe {
        RegQueryValueExW(
            hkey,
            PCWSTR(value.as_ptr()),
            None,
            None,
            None,
            Some(&mut cb),
        )
    };
    if err != ERROR_SUCCESS {
        // «Значения нет» — тот же штатный случай выключенного автозапуска:
        // debug, не warn (анти-спам, план §3).
        tracing::debug!(code = err.0, "Значение CanvasDesk в HKCU Run не найдено");
        return false;
    }
    cb > 2
}

/// Установить/снять автозапуск (идемпотентно): enable → RegCreateKeyW
/// (legacy-API — создаёт/открывает ключ, не тянет фичу Win32_Security,
/// в отличие от гейтованного RegCreateKeyExW) + RegSetValueExW(REG_SZ,
/// autostart_command(current_exe)); disable → RegDeleteValueW
/// (отсутствие значения — успех, не ошибка). Отказы — Err(строка) →
/// warn (R14). RegCloseKey — Drop-гуард на всех путях.
#[cfg(windows)]
pub fn set_autostart(enable: bool) -> Result<(), String> {
    if enable {
        set_autostart_enabled()
    } else {
        set_autostart_disabled()
    }
}

/// Ветка enable: создать/открыть Run-ключ и записать команду.
/// REG_SZ — UTF-16LE + терминатор: cb в байтах ОБЯЗАН включать два
/// нулевых байта терминатора, иначе Run-запись читается мусором (план
/// §7). Размер передаётся сигнатурой по длине слайса lpdata.
#[cfg(windows)]
fn set_autostart_enabled() -> Result<(), String> {
    // current_exe — тот же источник, что spawn_window_instance; провал
    // здесь фатален для ветки (записывать пустую команду бессмысленно) —
    // Err вызывающему (warn на его уровне, R14).
    let exe = std::env::current_exe().map_err(|err| format!("current_exe: {err}"))?;
    // to_string_lossy: Windows-путь — корректный UTF-16 в OsStr, потери
    // возможны только на непарных суррогатах (косметика реестра, не
    // функциональность запуска).
    let command = autostart_command(&exe.to_string_lossy());
    let mut bytes: Vec<u8> = command.encode_utf16().flat_map(u16::to_le_bytes).collect();
    // Терминатор REG_SZ: ровно два нулевых байта (один u16 = 0).
    bytes.extend_from_slice(&[0, 0]);

    let key = HSTRING::from(AUTOSTART_RUN_KEY);
    let value = HSTRING::from(AUTOSTART_VALUE);
    let mut hkey = HKEY::default();
    // SAFETY: hkey — out-параметр локального кадра. RegCreateKeyW —
    // legacy-API (план §2/§3): СОЗДАЁТ ключ или ОТКРЫВАЕТ существующий
    // (идемпотентен), в отличие от RegCreateKeyExW, гейтованного фичей
    // Win32_Security (не входит в дерево Win32_System_Registry —
    // координаторская верификация T17-coord); Security-типов не требует.
    let err = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, PCWSTR(key.as_ptr()), &mut hkey) };
    if err != ERROR_SUCCESS {
        return Err(format!("RegCreateKeyW: WIN32_ERROR({})", err.0));
    }
    // RegCloseKey на всех путях: гвард живёт до конца ветки (включая Err).
    let _close = RegKeyGuard(hkey);
    // SAFETY: hkey — валидный хэндл (гвард в кадре); bytes — слайс кадра,
    // длина (включая терминатор) передаётся самой сигнатурой lpdata:
    // Option<&[u8]> → cb = bytes.len(); reserved null по контракту
    // (обязан быть нулевым); REG_SZ соответствует UTF-16-содержимому.
    let err = unsafe { RegSetValueExW(hkey, PCWSTR(value.as_ptr()), None, REG_SZ, Some(&bytes)) };
    if err != ERROR_SUCCESS {
        return Err(format!("RegSetValueExW: WIN32_ERROR({})", err.0));
    }
    Ok(())
}

/// Ветка disable: удалить значение; отсутствие значения — успех
/// (идемпотентность: повторное выключение не ошибка, план §3).
#[cfg(windows)]
fn set_autostart_disabled() -> Result<(), String> {
    let key = HSTRING::from(AUTOSTART_RUN_KEY);
    let value = HSTRING::from(AUTOSTART_VALUE);
    let mut hkey = HKEY::default();
    // SAFETY: hkey — out-параметр локального кадра; KEY_SET_VALUE —
    // минимальные права под удаление значения. Провал открытия здесь —
    // ошибка ветки: Run-ключ существует на любой Windows (создаётся
    // системой), его отсутствие — диагностируемая аномалия (Err).
    let err = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            None,
            KEY_SET_VALUE,
            &mut hkey,
        )
    };
    if err != ERROR_SUCCESS {
        return Err(format!("RegOpenKeyExW: WIN32_ERROR({})", err.0));
    }
    let _close = RegKeyGuard(hkey);
    // SAFETY: hkey — валидный хэндл (гвард в кадре); имя значения —
    // нул-terminated UTF-16 локального кадра; удаление затрагивает только
    // значение CanvasDesk (имя-владение наше, TASKS T17).
    let err = unsafe { RegDeleteValueW(hkey, PCWSTR(value.as_ptr())) };
    match err {
        ERROR_SUCCESS => Ok(()),
        // Значения нет — цель «выключить» уже достигнута: идемпотентность
        // (ERROR_FILE_NOT_FOUND = 2, Win32::Foundation).
        ERROR_FILE_NOT_FOUND => Ok(()),
        err => Err(format!("RegDeleteValueW: WIN32_ERROR({})", err.0)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Простой путь без пробелов: точная строка «"exe" --desktop»,
    /// кавычки по краям пути, аргумент после закрывающей кавычки —
    /// Run-парсер видит exe одним токеном.
    #[test]
    fn autostart_command_simple_path() {
        let cmd = autostart_command(r"C:\CanvasDesk\canvas-app.exe");
        assert_eq!(cmd, r#""C:\CanvasDesk\canvas-app.exe" --desktop"#);
        // кавычки обрамляют именно путь (первый символ — кавычка,
        // закрывающая — сразу после .exe, до аргумента)
        assert!(cmd.starts_with(r#""C:\CanvasDesk\canvas-app.exe""#));
        assert!(cmd.ends_with(&format!(" {AUTOSTART_ARG}")));
    }

    /// Путь с пробелами: формат НЕ условный — кавычки всегда (без них
    /// команда развалилась бы на токены); ровно пара кавычек, путь внутри
    /// не искажён.
    #[test]
    fn autostart_command_path_with_spaces() {
        let exe = r"C:\Program Files\Canvas Desk\canvas-app.exe";
        let cmd = autostart_command(exe);
        assert_eq!(cmd, format!("\"{exe}\" {AUTOSTART_ARG}"));
        assert_eq!(cmd.matches('"').count(), 2);
        // путь целиком внутри кавычек (не разрезан пробелом)
        assert!(cmd.contains(&format!("\"{exe}\"")));
    }

    /// Пустой exe: вырожденный, но валидный минимум «"" --desktop» —
    /// кавычки не условлены содержимым (инвариант формата).
    #[test]
    fn autostart_command_empty_exe() {
        assert_eq!(autostart_command(""), format!("\"\" {AUTOSTART_ARG}"));
    }

    /// Суффикс — ровно «" --desktop»: закрывающая кавычка + ОДИН пробел-
    /// разделитель (двойных пробелов/табов нет — реестр читается
    /// CommandLineToArgvW, лишние разделители меняли бы токенизацию).
    #[test]
    fn autostart_command_single_space_separator() {
        let cmd = autostart_command("app.exe");
        assert_eq!(cmd, "\"app.exe\" --desktop");
        assert!(cmd.ends_with("\" --desktop"));
    }

    /// Константы непустые и точные: ветка HKCU Run (SPEC §9), имя
    /// значения одно на приложение, аргумент совпадает с CLI main.rs.
    #[test]
    fn autostart_constants_exact() {
        assert_eq!(AUTOSTART_VALUE, "CanvasDesk");
        assert_eq!(AUTOSTART_ARG, "--desktop");
        // полный канонический подключ Run (последний сегмент — «Run»)
        assert_eq!(
            AUTOSTART_RUN_KEY,
            r"Software\Microsoft\Windows\CurrentVersion\Run"
        );
        assert!(AUTOSTART_RUN_KEY.ends_with("Run"));
    }
}
