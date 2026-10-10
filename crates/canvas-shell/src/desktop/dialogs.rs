//! FR-108 (мультиканвас C5, issue #9): нативные файловые диалоги
//! «Открыть…» / «Создать» (решение №34a) — IFileOpenDialog/IFileSaveDialog
//! (COM, windows crate), БЕЗ новых зависимостей (крейт уже в дереве).
//!
//! Паттерн модуля — `menu.rs` (T17-C): чистые функции (фильтры, суффиксы,
//! дефолтные имена — тестируются на Linux) + `cfg(windows)`-механика с
//! SAFETY-комментариями. НЕ-Windows-натив (Linux dev/CI) — заглушка: warn +
//! None (CLI-путь и drag-drop остаются рабочими); web не доходит сюда
//! (свои пикеры — FS Access, WebRequest-конвейер FR-104/106).
//!
//! Диалоги модальные в UI-потоке: синхронный вызов из обработчика клика
//! (ShellExecute-паттерн menu.rs) — winit event loop заморожен на время
//! диалога; это осознанное решение волны C5 (задокументировано в FR-108).
//!
//! HWND приходит сырым `isize` (0 — без родителя): winit 0.30 публично
//! HWND не отдаёт, а свою зависимость `windows` в canvas-app не тащим —
//! конвертация в типизированный HWND здесь (идиома dragdrop::install).

use std::path::PathBuf;

/// Подпись фильтра диалога («CanvasDesk canvas|*.canvas», №34a).
pub const FILTER_LABEL: &str = "CanvasDesk canvas";
/// Маска фильтра — только файлы канвасов.
pub const FILTER_MASK: &str = "*.canvas";
/// Расширение канваса (без точки) — дефолтное для save-диалога.
pub const CANVAS_EXT: &str = "canvas";

/// Расширение выбора save-диалога: имя без хвостового `.canvas` получает
/// его (дефолтное расширение при вводе без него, №34a). Чистая функция —
/// safety-net поверх `SetDefaultExtension` (диалог уже дописывает
/// расширение к имени без точки; здесь страхуем остальные случаи, включая
/// «имя.чтото» → «имя.чтото.canvas»).
pub fn ensure_canvas_extension(file_name: &str) -> String {
    if file_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .ends_with(CANVAS_EXT)
    {
        file_name.to_owned()
    } else {
        format!("{file_name}.{CANVAS_EXT}")
    }
}

#[cfg(windows)]
use windows::core::{HSTRING, PCWSTR};
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
#[cfg(windows)]
use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
#[cfg(windows)]
use windows::Win32::UI::Shell::{
    FileOpenDialog, FileSaveDialog, IFileDialog, IShellItem, FOS_FILEMUSTEXIST,
    FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
};

/// «Открыть…»: модальный IFileOpenDialog с фильтром `.canvas`. None —
/// отмена пользователя (тихо) или отказ платформы (warn внутри).
/// `raw_hwnd` — родительское окно (0 — без родителя).
pub fn open_canvas_dialog(raw_hwnd: isize) -> Option<PathBuf> {
    #[cfg(windows)]
    return open_save_impl(raw_hwnd, DialogKind::Open, "");
    #[cfg(not(windows))]
    {
        let _ = raw_hwnd;
        warn_unsupported("Открыть…");
        None
    }
}

/// «Создать» (№34a): модальный IFileSaveDialog (подтверждение перезаписи —
/// системное, FOS_OVERWRITEPROMPT) с дефолтным именем `default_name` и
/// дефолтным расширением `.canvas`. None — отмена/отказ (см.
/// [`open_canvas_dialog`]).
pub fn save_canvas_dialog(raw_hwnd: isize, default_name: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    return open_save_impl(raw_hwnd, DialogKind::Save, default_name);
    #[cfg(not(windows))]
    {
        let _ = (raw_hwnd, default_name);
        warn_unsupported("Создать");
        None
    }
}

/// Заглушка не-Windows-платформ (Linux dev/CI): диалоги недоступны —
/// единый warn (бриф C5: CLI-путь и drag-drop остаются рабочими).
#[cfg(not(windows))]
fn warn_unsupported(action: &str) {
    tracing::warn!(
        action,
        "нативные файловые диалоги недоступны на этой платформе (FR-108: IFileOpenDialog — Windows)"
    );
}

// --- cfg(windows): COM-механика --------------------------------------------

#[cfg(windows)]
enum DialogKind {
    Open,
    Save,
}

/// Общий каркас обоих диалогов: CoInitializeEx (балансный CoUninitialize)
/// → CoCreateInstance → фильтр/опции/дефолтное имя → Show (модально, UI
/// поток) → GetResult → путь файловой системы. Отмена — тихий None.
#[cfg(windows)]
fn open_save_impl(raw_hwnd: isize, kind: DialogKind, default_name: &str) -> Option<PathBuf> {
    // SAFETY: STA потока event loop уже стоит OleInitialize'ом drag-drop
    // (T9, resumed) — этот вызов вернёт S_FALSE и лишь поднимет ref-count;
    // при первом заходе он сам создаст квартиру. Err (RPC_E_CHANGED_MODE,
    // поток MTA) — единственная ошибка, при которой диалогу нельзя
    // продолжать: выходим с warn (R14). Парный CoUninitialize ниже
    // балансирует и S_OK, и S_FALSE (контракт MSDN).
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    if hr.is_err() {
        tracing::warn!(
            code = hr.0,
            "CoInitializeEx(STA) отказал — файловый диалог недоступен (R14)"
        );
        return None;
    }
    let result = run_dialog(raw_hwnd, &kind, default_name);
    // SAFETY: балансировка успешного CoInitializeEx выше (S_OK/S_FALSE
    // оба инкрементировали счётчик квартиры; чужая инициализация drag-drop
    // не разрушается — её ref держит OleInitialize).
    unsafe { CoUninitialize() };
    result
}

/// Диалог без COM-инициализации (картирование вызова); ошибки — None+warn,
/// отмена пользователя — тихий None (ERROR_CANCELLED — не ошибка для нас).
#[cfg(windows)]
fn run_dialog(raw_hwnd: isize, kind: &DialogKind, default_name: &str) -> Option<PathBuf> {
    // SAFETY: CoCreateInstance с CLSID штатных диалогов Shell — без
    // предусловий; punkouter=None (агрегация не нужна). S_OK/S_FALSE → Ok.
    let dialog: IFileDialog = match kind {
        DialogKind::Open => unsafe {
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
        },
        DialogKind::Save => unsafe {
            CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)
        },
    }
    .inspect_err(|err| tracing::warn!(%err, "CoCreateInstance диалога отказал (R14)"))
    .ok()?;
    // Фильтр «CanvasDesk canvas|*.canvas»: подпись и маска — HSTRING,
    // живут до конца вызова SetFileTypes (COMDLG_FILTERSPEC держит PCWSTR).
    let label = HSTRING::from(FILTER_LABEL);
    let mask = HSTRING::from(FILTER_MASK);
    let filters = [COMDLG_FILTERSPEC {
        pszName: PCWSTR(label.as_ptr()),
        pszSpec: PCWSTR(mask.as_ptr()),
    }];
    // SAFETY: dialog жив (владение у нас); срез filters — валидные
    // COMDLG_FILTERSPEC на локальных HSTRING выше.
    if let Err(err) = unsafe { dialog.SetFileTypes(&filters) } {
        tracing::warn!(%err, "SetFileTypes отказал — диалог без фильтра .canvas (R14)");
    }
    // Опции: только файловая система + существующий путь; Open — файл
    // обязан существовать; Save — системное подтверждение перезаписи (№34a:
    // выбор существующего файла честно спрашивает, «тихой» перезаписи нет).
    let options = match kind {
        DialogKind::Open => FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_FILEMUSTEXIST,
        DialogKind::Save => FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_OVERWRITEPROMPT,
    };
    // SAFETY: dialog жив; опции — bitmask-константы Shell.
    if let Err(err) = unsafe { dialog.SetOptions(options) } {
        tracing::warn!(%err, "SetOptions отказал (R14)");
    }
    if let DialogKind::Save = kind {
        if !default_name.is_empty() {
            let name = HSTRING::from(default_name);
            // SAFETY: name — локальный HSTRING (null-terminated UTF-16).
            if let Err(err) = unsafe { dialog.SetFileName(PCWSTR(name.as_ptr())) } {
                tracing::warn!(%err, "SetFileName отказал — диалог без дефолтного имени (R14)");
            }
        }
        // Дефолтное расширение: ввод без точки получает «.canvas» (№34a).
        let ext = HSTRING::from(CANVAS_EXT);
        // SAFETY: ext — локальный HSTRING.
        if let Err(err) = unsafe { dialog.SetDefaultExtension(PCWSTR(ext.as_ptr())) } {
            tracing::warn!(%err, "SetDefaultExtension отказал (R14)");
        }
    }
    // Модальный показ: UI-поток (winit event loop) заморожен до выбора —
    // осознанное решение C5 (паттерн TrackPopupMenu menu.rs; FR-108).
    // Отмена (ERROR_CANCELLED = 0x800704C9) — тихий None, не warn.
    let parent = (raw_hwnd != 0).then_some(windows::Win32::Foundation::HWND(
        // Win32 HWND — указатель без внутренней структуры (идиома dragdrop).
        raw_hwnd as *mut core::ffi::c_void,
    ));
    // SAFETY: parent — окно этого процесса (передано из клика) или None;
    // модальный цикл крутится до выбора пользователя.
    if let Err(err) = unsafe { dialog.Show(parent) } {
        // Отмена (ERROR_CANCELLED = 0x800704C9) — тихий None, не warn.
        // HRESULT — i32 (windows-rs): литерал без знака сравниваем через
        // u32-каст (чистый литерал 0x8007_04C9 в i32 не влезает).
        if err.code().0 as u32 != 0x8007_04C9 {
            tracing::warn!(%err, "файловый диалог отказал (R14)");
        }
        return None;
    }
    // SAFETY: GetResult возвращает IShellItem выбора (владение — наше).
    let item: IShellItem = match unsafe { dialog.GetResult() } {
        Ok(item) => item,
        Err(err) => {
            tracing::warn!(%err, "GetResult отказал (R14)");
            return None;
        }
    };
    // SAFETY: GetDisplayName выделяет PWSTR (CoTaskMem) — читаем до nul и
    // освобождаем CoTaskMemFree сами (windows-rs PWSTR память не владеет).
    let pw = match unsafe { item.GetDisplayName(SIGDN_FILESYSPATH) } {
        Ok(pw) => pw,
        Err(err) => {
            tracing::warn!(%err, "GetDisplayName(SIGDN_FILESYSPATH) отказал (R14)");
            return None;
        }
    };
    let path = pwstr_to_path(pw.0);
    // SAFETY: pw выделен CoTaskMemAlloc'ом внутри GetDisplayName; копия
    // уже сделана — освобождаем ровно один раз.
    unsafe { CoTaskMemFree(Some(pw.0.cast())) };
    path
}

/// PWSTR (null-terminated UTF-16) → PathBuf; пустая/битая строка — None
/// (путь файловой системы обязана вернуть система — сюрпризов не ждём).
#[cfg(windows)]
fn pwstr_to_path(pw: *mut u16) -> Option<PathBuf> {
    if pw.is_null() {
        return None;
    }
    // SAFETY: GetDisplayName вернул валидную null-terminated строку —
    // идём до nul, копируем в String, ничего не мутируем.
    let path = unsafe {
        let mut len = 0usize;
        while *pw.add(len) != 0 {
            len += 1;
        }
        let wide = std::slice::from_raw_parts(pw, len);
        String::from_utf16_lossy(wide)
    };
    if path.is_empty() {
        None
    } else {
        Some(PathBuf::from(path))
    }
}

// ============================================================================
// Чистые тесты (натив, любая ОС)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Дефолтное расширение save-диалога: имя без `.canvas` получает его.
    #[test]
    fn ensure_extension_appends_canvas() {
        assert_eq!(ensure_canvas_extension("Отчёт"), "Отчёт.canvas");
        assert_eq!(ensure_canvas_extension("Canvas 2"), "Canvas 2.canvas");
        // Уже с расширением (в любом регистре) — не трогаем
        assert_eq!(ensure_canvas_extension("notes.canvas"), "notes.canvas");
        assert_eq!(ensure_canvas_extension("NOTES.CANVAS"), "NOTES.CANVAS");
        // Чужое расширение — дописываем поверх (lossless, не замещаем)
        assert_eq!(ensure_canvas_extension("архив.zip"), "архив.zip.canvas");
    }

    /// Константы фильтра — маска и подпись разделу диалога (контракт UI).
    #[test]
    fn filter_constants() {
        assert_eq!(FILTER_MASK, "*.canvas");
        assert!(!FILTER_LABEL.is_empty());
        assert_eq!(CANVAS_EXT, "canvas");
    }
}
