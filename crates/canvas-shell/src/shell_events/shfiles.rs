//! Декод SHChangeNotify-нотификаций (T16-D, cfg(windows), RECIPES R12):
//! lParam WM_APP_SHELL_FILE → (маска SHCNE, пара путей) → FileEvent.
//!
//! Режим доставки — SHCNRF_NewDelivery (план §8.6): lParam — HANDLE
//! нотификации; доступ — SHChangeNotification_Lock(handle, dwProcId =
//! GetCurrentProcessId(), &mut pppidl, &mut маска) → pidl-пара, закрытие —
//! SHChangeNotification_Unlock на ВСЕХ путях выхода (Drop-гуард
//! [`UnlockGuard`]). pidl → путь: SHGetPathFromIDListW (false на
//! не-файловых pidl — legitimately None). Итоговый маппинг — чистая
//! `shell_file_change` ядра (T16-A): единая точка семантики SHCNE →
//! FileEvent (в т.ч. корзина → SHCNE_DELETE → Remove → brokenLink).
//!
//! Зона воркера T16-D: реализация по плану docs/plans/T16-shell-events.md
//! §3 (shfiles.rs). SAFETY-комментарии обязательны (AGENTS.md п.6).

use std::path::PathBuf;

use canvas_core::FileEvent;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Shell::{
    SHChangeNotification_Lock, SHChangeNotification_Unlock, SHGetPathFromIDListW,
};

/// Имя события для сообщения «wMsg» SHChangeNotifyRegister (значение —
/// WM_APP_SHELL_FILE из ядра; константа здесь для самодокументируемости
/// регистрации в regs.rs).
pub const SHELL_FILE_MSG_NAME: &str = "WM_APP_SHELL_FILE (CanvasDesk)";

/// MAX_PATH (ShlObj.h): размер буфера SHGetPathFromIDListW — сигнатура
/// windows-0.62 принимает ровно `&mut [u16; 260]`.
const MAX_PATH: usize = 260;

/// RAII-гуард парного SHChangeNotification_Unlock (план §3): создаётся
/// только после успешного Lock и живёт до конца кадра `decode` — Unlock
/// случается на любом пути выхода, включая ранние возвраты и unwind.
struct UnlockGuard(HANDLE);

impl Drop for UnlockGuard {
    fn drop(&mut self) {
        // SAFETY: self.0 — хэндл успешного SHChangeNotification_Lock из
        // того же кадра decode; Unlock не принимает dwProcId и обязан
        // вызываться ровно один раз на хэндл — RAII гарантирует парность.
        // После возврата Drop данные нотификации никем не читаются (pidl
        // скопированы в PathBuf ещё в decode). FALSE (битый хэндл — не
        // воспроизводится на живом shell) — только warn: приложение
        // продолжает работать (деградация R14).
        let ok = unsafe { SHChangeNotification_Unlock(self.0) };
        if !ok.as_bool() {
            tracing::warn!(
                "SHChangeNotification_Unlock не удался — Lock-буфер мог остаться занятым"
            );
        }
    }
}

/// Декодировать нотификацию: Lock (dwProcId = GetCurrentProcessId) →
/// маска + pidl-пара → пути → shell_file_change. None — не-файловые pidl/
/// неизвестная маска/отсутствие нужного пути (в т.ч. Lock отказал —
/// warn, R14). `wparam` — wParam сообщения (в NewDelivery-режиме не
/// несёт маски; принят параметром для диагностики).
pub fn decode(lparam: isize, wparam: usize) -> Option<FileEvent> {
    // NewDelivery: lParam — HANDLE нотификации (план §8.6). Нулевой lParam —
    // защитная проверка: shell не выдаёт NULL-хэндл нотификации, мусорное
    // значение безопаснее игнорировать, чем тащить в FFI.
    if lparam == 0 {
        return None;
    }
    // wParam в NewDelivery НЕ несёт маски (маска — из Lock, §8.6); трассируем
    // только для диагностики ручной приёмки (риск §7: «событие не
    // декодируется» — логом).
    tracing::trace!(
        wparam,
        lparam,
        "SHChangeNotify: декод NewDelivery-нотификации"
    );

    // Win32-хэндл — указательное значение без внутренней структуры:
    // конвертация из lParam тривиальна (идиома T9: HWND из isize).
    // Хэндл валиден ТОЛЬКО в течение этого вызова wndproc — не сохраняем.
    let handle = HANDLE(lparam as *mut core::ffi::c_void);

    // SAFETY: GetCurrentProcessId предусловий и побочных эффектов не имеет
    // (просто возвращает pid процесса) — небезопасного кода по факту нет.
    // Lock привязан к процессу-регистратору: dwProcId обязан быть pid
    // процесса, окно которого получило сообщение.
    let pid = unsafe { GetCurrentProcessId() };

    // Out-параметры Lock: pppidl — адрес массива pidl-пары (двойной
    // указатель), event — маска SHCNE. Может остаться нулевым, если
    // нотификация без путей — проверяется в pidl_pair_paths.
    let mut pppidl: *mut *mut windows::Win32::UI::Shell::Common::ITEMIDLIST = std::ptr::null_mut();
    let mut event: i32 = 0;
    // SAFETY: handle — значение lParam WM_APP_SHELL_FILE (NewDelivery,
    // §8.6): shared-память нотификации живёт до возврата из wndproc, значит
    // валидна на протяжении всего вызова decode; pppidl/event — локальные
    // переменные, Lock только пишет в них. Провал Lock возвращает
    // недействительный хэндл (is_invalid) — ниже проверка, FFI по
    // недействительному хэндлу не зовём.
    let lock =
        unsafe { SHChangeNotification_Lock(handle, pid, Some(&mut pppidl), Some(&mut event)) };
    if lock.is_invalid() {
        tracing::warn!(
            lparam,
            "SHChangeNotification_Lock не удался — нотификация пропущена"
        );
        return None;
    }
    // Unlock на ВСЕХ путях выхода: RAII-гвард живёт до конца кадра, включая
    // ранний возврат и unwind (Lock/Unlock — окно доступа к pidl).
    let _unlock = UnlockGuard(lock);

    let (old, new) = pidl_pair_paths(pppidl);
    // Итоговая семантика — чистая функция ядра A (единая точка маппинга
    // SHCNE → FileEvent: приоритет Rename > Delete > Create > Modify).
    super::shell_file_change(event, old.as_deref(), new.as_deref())
}

/// Прочитать pidl-пару из Lock-буфера и перевести в пути. Контракт:
/// вызывается ТОЛЬКО между успешным Lock и его Unlock (гвард живёт в кадре
/// вызывающего decode) — pidl не переживают Unlock. `pppidl == NULL` или
/// пустой массив → (None, None): нотификация без путей — легитимный случай
/// (SHChangeNotify с dwItem1/dwItem2 = NULL, «виртуальные» события).
fn pidl_pair_paths(
    pppidl: *mut *mut windows::Win32::UI::Shell::Common::ITEMIDLIST,
) -> (Option<PathBuf>, Option<PathBuf>) {
    // Нотификация без путей: Lock оставляет двойной указатель нулевым
    // (SHChangeNotify с dwItem1/dwItem2 = NULL, «виртуальные» события).
    if pppidl.is_null() {
        return (None, None);
    }
    // SAFETY: pppidl ненулевой и записан успешным Lock (контракт выше:
    // валиден до Unlock); значение — PIDLIST** из документации Lock:
    // указатель на массив ровно из двух указателей pidl (dwItem1/dwItem2
    // нотификации: [0] — старый/целевой путь, [1] — новый; оба могут быть
    // NULL — проверяет pidl_to_path). Копируем из слайса только два
    // УКАЗАТЕЛЯ, а не структуры ITEMIDLIST: чтение самих pidl остаётся в
    // SHGetPathFromIDListW до Unlock (гвард в кадре decode). Памятью pidl
    // владеет shell — НИКОГДА не освобождаем (план §3).
    let pidls = unsafe { std::slice::from_raw_parts(pppidl, 2) };
    (pidl_to_path(pidls[0]), pidl_to_path(pidls[1]))
}

/// pidl → путь файловой системы (SHGetPathFromIDListW, буфер 260): None
/// для не-файловых pidl (объекты пространства имён shell и пр.).
/// Контракт: вызывается только по цепочке decode → pidl_pair_paths, т.е.
/// между Lock и Unlock — pidl не переживает Unlock.
fn pidl_to_path(pidl: *const windows::Win32::UI::Shell::Common::ITEMIDLIST) -> Option<PathBuf> {
    if pidl.is_null() {
        return None;
    }
    let mut buf = [0u16; MAX_PATH];
    // SAFETY: pidl ненулевой и получен из Lock-буфера (контракт выше:
    // валиден до Unlock); SHGetPathFromIDListW читает список ITEMID без
    // записи в него и пишет в buf null-terminated путь; тип [&mut [u16;
    // 260]] сама сигнатура ограничивает размер — переполнения нет. FALSE —
    // не-файловый pidl (виртуальный объект shell) — legitimately None,
    // не ошибка. Памятью pidl владеет shell: НИКОГДА не освобождаем
    // (владение у shell, Lock/Unlock — окно доступа, план §3).
    let ok = unsafe { SHGetPathFromIDListW(pidl, &mut buf) };
    if !ok.as_bool() {
        return None;
    }
    // Терминатор гарантирован контрактом SHGetPathFromIDListW; position +
    // unwrap_or(MAX_PATH) — страховка от битого буфера (не паникуем).
    let len = buf.iter().position(|&unit| unit == 0).unwrap_or(MAX_PATH);
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
}
