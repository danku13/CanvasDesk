//! Регистрации окна шины (T16-C, cfg(windows), RECIPES R8/R11/R12):
//! шесть каналов системных событий + идемпотентное снятие + SHChangeNotify-
//! подписки на директории нод (diff-синк, зеркало watcher T10).
//!
//! Зона воркера T16-C: реализация по плану docs/plans/T16-shell-events.md
//! §3 (regs.rs). Каждая регистрация — warn-деградация при провале (R14:
//! шина живёт на остальных каналах). Значения констант ядра сверяются с
//! windows-crate debug_assert'ами (приём T15-A).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use canvas_core::normalize_path;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{CO_E_NOTINITIALIZED, HANDLE, HWND};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows::Win32::System::Power::{
    RegisterSuspendResumeNotification, UnregisterSuspendResumeNotification,
};
use windows::Win32::System::RemoteDesktop::{
    ProcessIdToSessionId, WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
    NOTIFY_FOR_ALL_SESSIONS,
};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ILFree, SHCNRF_NewDelivery, SHCNRF_ShellLevel, SHChangeNotifyDeregister, SHChangeNotifyEntry,
    SHChangeNotifyRegister, SHParseDisplayName,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, DeregisterShellHookWindow, RegisterShellHookWindow,
    RegisterWindowMessageW, DEVICE_NOTIFY_WINDOW_HANDLE, MSGFLT_ALLOW,
};

use super::{shcne, MsgRouter, WM_CLIPBOARDUPDATE, WM_POWERBROADCAST, WM_WTSSESSION_CHANGE};

/// Чем владеем для идемпотентного снятия при Shutdown/выходе.
/// Часть каналов не требует явного снятия (умирает с окном/потоком),
/// но WTS/suspend/shell-hook/clipboard снимаем явно — чистый выход.
/// SHChangeNotify-подписки — динамический набор, живёт в
/// [`FileNotifySync`].
#[derive(Debug, Default)]
pub struct ShellRegs {
    /// WTSRegisterSessionNotification прошла.
    pub wts: bool,
    /// HPOWERNOTIFY от RegisterSuspendResumeNotification.
    pub suspend: Option<windows::Win32::System::Power::HPOWERNOTIFY>,
    /// RegisterShellHookWindow прошла.
    pub shell_hook: bool,
    /// AddClipboardFormatListener прошёл.
    pub clipboard: bool,
}

/// Зарегистрировать все шесть каналов на окне шины:
/// 1) WTSRegisterSessionNotification(NOTIFY_FOR_ALL_SESSIONS) — R8;
/// 2) RegisterSuspendResumeNotification(DEVICE_NOTIFY_WINDOW_HANDLE) — R11;
/// 3) RegisterWindowMessageW("TaskbarCreated") — ExplorerStarted, R7/R11;
/// 4) RegisterWindowMessageW("SHELLHOOK") + RegisterShellHookWindow — R11;
/// 5) AddClipboardFormatListener +
///    ChangeWindowMessageFilterEx(WM_CLIPBOARDUPDATE, MSGFLT_ALLOW) — R11
///    (UIPI-фильтр ОБЯЗАТЕЛЕН);
/// 6) SHChangeNotify-подписки — отдельно через [`FileNotifySync::sync`]
///    (динамический набор директорий модели).
///
/// Возвращает (реестр снятия, MsgRouter с динамическими id).
pub fn register_all(hwnd: HWND) -> (ShellRegs, MsgRouter) {
    // Сверка локальных констант ядра (mod.rs не зависит от windows-crate)
    // с реальными значениями WinUser.h: расхождение — баг посева констант,
    // ловим в debug (приём T15-A; win-check собирает msvc-debug).
    debug_assert_eq!(
        WM_WTSSESSION_CHANGE,
        windows::Win32::UI::WindowsAndMessaging::WM_WTSSESSION_CHANGE
    );
    debug_assert_eq!(
        WM_POWERBROADCAST,
        windows::Win32::UI::WindowsAndMessaging::WM_POWERBROADCAST
    );
    debug_assert_eq!(
        WM_CLIPBOARDUPDATE,
        windows::Win32::UI::WindowsAndMessaging::WM_CLIPBOARDUPDATE
    );

    let mut regs = ShellRegs::default();

    // ---- 1) WTS-сессии (R8) --------------------------------------------
    // Регистрируемся на ВСЕ сессии (NOTIFY_FOR_ALL_SESSIONS): события чужих
    // тоже придут — wndproc фильтрует своей (classify_wts + own_session_id).
    // SAFETY: hwnd — живое окно шины этого процесса (создано в shell_events_
    // loop ДО register_all); NOTIFY_FOR_ALL_SESSIONS — скалярный флаг; вызов
    // только ставит окно в WTS-список, парное снятие — WTSUnRegisterSession
    // Notification в unregister_all; провал — Err (без UB).
    match unsafe { WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_ALL_SESSIONS) } {
        Ok(()) => regs.wts = true,
        Err(err) => tracing::warn!(
            %err,
            "shell-шина: WTS-события сессии недоступны (R8) — шина живёт без них (R14)"
        ),
    }

    // ---- 2) Suspend/resume (R11) ----------------------------------------
    // HWND→HANDLE: оба — прозрачные обёртки указателя окна; DEVICE_NOTIFY_
    // WINDOW_HANDLE — «получатель нотификаций = окно» (PBT_* придут в
    // wndproc как WM_POWERBROADCAST).
    // SAFETY: hwnd — живое окно шины (см. п.1); HPOWERNOTIFY из Ok-возврата
    // сохраняется в regs и снимается ровно один раз (take) в unregister_all.
    match unsafe { RegisterSuspendResumeNotification(HANDLE(hwnd.0), DEVICE_NOTIFY_WINDOW_HANDLE) }
    {
        Ok(handle) => regs.suspend = Some(handle),
        Err(err) => tracing::warn!(
            %err,
            "shell-шина: suspend/resume-события недоступны (R11) — шина живёт без них (R14)"
        ),
    }

    // ---- 3) TaskbarCreated (R7/R11) -------------------------------------
    // Глобальное имя сообщения: id одинаков у всех процессов, явного снятия
    // не требует (умирает с системой). 0 = провал регистрации.
    // SAFETY: w!-литерал — null-terminated UTF-16; чистая регистрация имени
    // в системной таблице, больше никаких побочных эффектов нет.
    let taskbar_id = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    if taskbar_id == 0 {
        tracing::warn!(
            "shell-шина: TaskbarCreated не зарегистрирован (R7/R11) — события перезапуска Explorer недоступны (R14)"
        );
    }

    // ---- 4) SHELLHOOK + RegisterShellHookWindow (R11) --------------------
    // Сначала id: shell шлёт HSHELL_*-события окнам по сообщению с
    // зарегистрированным именем «SHELLHOOK»; id=0 (имя не встало) → hook
    // не ставим — сообщения некуда слать (бессмысленная регистрация), warn.
    // SAFETY: w!-литерал — null-terminated UTF-16; см. п.3.
    let shellhook_id = unsafe { RegisterWindowMessageW(w!("SHELLHOOK")) };
    if shellhook_id == 0 {
        tracing::warn!(
            "shell-шина: SHELLHOOK не зарегистрирован (R11) — shell-hook не ставим (R14)"
        );
    } else {
        // SAFETY: hwnd — живое окно шины (см. п.1); ставит окно в системный
        // shell-hook-список, парное снятие — DeregisterShellHookWindow в
        // unregister_all; FALSE = провал (без UB).
        if unsafe { RegisterShellHookWindow(hwnd) }.as_bool() {
            regs.shell_hook = true;
        } else {
            tracing::warn!(
                "shell-шина: RegisterShellHookWindow провален (R11) — шина живёт без shell-hook (R14)"
            );
        }
    }

    // ---- 5) Clipboard + UIPI-фильтр (R11) --------------------------------
    // SAFETY: hwnd — живое окно шины (см. п.1); подписка на смену форматов
    // буфера обмена, парное снятие — RemoveClipboardFormatListener в
    // unregister_all; провал — Err (без UB).
    match unsafe { AddClipboardFormatListener(hwnd) } {
        Ok(()) => {
            regs.clipboard = true;
            // UIPI-фильтр ОБЯЗАТЕЛЕН (R11): без него elevated-источник молча
            // блокирует WM_CLIPBOARDUPDATE для нашего не-elevated окна —
            // ставим всегда при живом clipboard-канале.
            // SAFETY: hwnd — живое окно шины; WM_CLIPBOARDUPDATE — константа
            // ядра (сверена debug_assert'ом выше); MSGFLT_ALLOW — разрешающий
            // экшен; pchangefilterstruct не запрашиваем (старое состояние не
            // нужно) — None; Err = провал фильтра (без UB).
            if let Err(err) =
                unsafe { ChangeWindowMessageFilterEx(hwnd, WM_CLIPBOARDUPDATE, MSGFLT_ALLOW, None) }
            {
                tracing::warn!(
                    %err,
                    "UIPI-фильтр не установлен: elevated-сессия может молча терять clipboard-сообщения (R11)"
                );
            }
        }
        Err(err) => tracing::warn!(
            %err,
            "shell-шина: clipboard-канал недоступен (R11) — шина живёт без него (R14)"
        ),
    }

    // ---- 6) MsgRouter: динамические id ------------------------------------
    // Коллизии id — инвариант конструктора ядра (T16-A, паника = баг-сигнал);
    // неактивные слоты (id=0) роутер трактует сам.
    let router = MsgRouter::new(taskbar_id, shellhook_id);

    tracing::debug!(
        taskbar_id,
        shellhook_id,
        "shell-шина: динамические msg-id RegisterWindowMessageW"
    );
    tracing::info!(
        wts = regs.wts,
        suspend = regs.suspend.is_some(),
        taskbar = taskbar_id != 0,
        shellhook = regs.shell_hook,
        clipboard = regs.clipboard,
        "shell-шина: регистрации"
    );

    (regs, router)
}

/// Снять все регистрации (идемпотентно; ошибки — молча/warn):
/// WTSUnRegisterSessionNotification, UnregisterSuspendResumeNotification,
/// DeregisterShellHookWindow, RemoveClipboardFormatListener,
/// SHChangeNotifyDeregister (все id из реестра).
pub fn unregister_all(hwnd: HWND, regs: &mut ShellRegs) {
    // Идемпотентность: после снятия флаги обнуляются — повторный вызов no-op.
    // Ошибки снятия — warn: Shutdown-путь не должен падать (R14), окно всё
    // равно умирает и система снимет регистрации сама — лог для диагностики.
    if regs.wts {
        // SAFETY: hwnd — окно шины, ещё не разрушено (снятие ДО DestroyWindow
        // в Shutdown-клинапе потока, T16-B); повторный вызов после смерти окна
        // — Err → warn, без UB.
        if let Err(err) = unsafe { WTSUnRegisterSessionNotification(hwnd) } {
            tracing::warn!(%err, "shell-шина: WTSUnRegisterSessionNotification");
        }
        regs.wts = false;
    }
    if let Some(handle) = regs.suspend.take() {
        // SAFETY: handle — HPOWERNOTIFY из успешной RegisterSuspendResume-
        // Notification; take() изымает — ровно одно снятие; провал — Err → warn.
        if let Err(err) = unsafe { UnregisterSuspendResumeNotification(handle) } {
            tracing::warn!(%err, "shell-шина: UnregisterSuspendResumeNotification");
        }
    }
    if regs.shell_hook {
        // SAFETY: hwnd — окно шины (см. выше); снятие shell-hook-регистрации;
        // FALSE = провал (без UB) — warn.
        if !unsafe { DeregisterShellHookWindow(hwnd) }.as_bool() {
            tracing::warn!("shell-шина: DeregisterShellHookWindow вернул FALSE");
        }
        regs.shell_hook = false;
    }
    if regs.clipboard {
        // SAFETY: hwnd — окно шины (см. выше); отписка clipboard-листенера;
        // провал — Err → warn (без UB).
        if let Err(err) = unsafe { RemoveClipboardFormatListener(hwnd) } {
            tracing::warn!(%err, "shell-шина: RemoveClipboardFormatListener");
        }
        regs.clipboard = false;
    }
}

/// Идентификатор своей сессии (R8): ProcessIdToSessionId(
/// GetCurrentProcessId()); 0 при провале — все WTS-события фильтруются
/// как «чужие» (warn; гейт не обновляется — шина живёт).
pub fn own_session_id() -> u32 {
    // SAFETY: GetCurrentProcessId — чистое чтение идентификатора текущего
    // процесса: ошибок и побочных эффектов нет, unsafe — только из-за
    // FFI-декларации.
    let pid = unsafe { GetCurrentProcessId() };
    let mut session_id: u32 = 0;
    // SAFETY: pid — идентификатор живого (текущего) процесса; session_id —
    // валидный локальный out-указатель, функция пишет ровно одно u32.
    match unsafe { ProcessIdToSessionId(pid, &mut session_id) } {
        Ok(()) if session_id != 0 => session_id,
        // 0 = «не определён»: интерактивные сессии имеют ненулевые id
        // (ноль — сервисная сессия); все события будут «чужими» — гейт не
        // обновится, шина живёт (R8/R14, план §7 «Session id = 0»).
        Ok(()) => {
            tracing::warn!("session id не определён (0): WTS-события будут фильтроваться (R8)");
            0
        }
        Err(err) => {
            tracing::warn!(%err, "session id не определён: WTS-события будут фильтроваться (R8)");
            0
        }
    }
}

/// Дифф-синхронизация SHChangeNotify-подписок (R12): зеркало
/// WatchService::sync_dirs (T10) — те же желаемые директории модели.
/// Добавление: pidl каталога (SHParseDisplayName; COM STA потока уже
/// инициализирован) → SHChangeNotifyRegister(hwnd, SHCNRF_ShellLevel |
/// SHCNRF_NewDelivery, SHCNE_DISKEVENTS, msg, 1, entry { fRecursive:
/// false }); снятие: SHChangeNotifyDeregister(id). Ошибки — warn,
/// незарегистрированный dir не попадает в таблицу (ретрай следующим
/// синком). Карта — «нормализованный dir → registration id».
#[derive(Debug, Default)]
pub struct FileNotifySync {
    /// Активные подписки: dir → SHChangeNotifyRegister-id.
    entries: HashMap<PathBuf, u32>,
}

impl FileNotifySync {
    /// Новый пустой синкатор (подписок нет).
    pub fn new() -> Self {
        Self::default()
    }

    /// Синхронизировать подписки с желаемым набором директорий.
    /// `msg` — WM_APP_SHELL_FILE (куда шина положит нотификации).
    pub fn sync(&mut self, hwnd: HWND, msg: u32, dirs: &[PathBuf]) {
        // Желаемый набор — нормализация через canvas_core::normalize_path
        // (единый источник с T10) + HashSet-уникальность: и ключи карты, и
        // сравнение — в одном каноническом виде.
        let desired: HashSet<PathBuf> = dirs.iter().map(|dir| normalize_path(dir)).collect();

        // ---- Снятие лишних (подписка есть, директории в желаемом нет) ----
        let stale: Vec<PathBuf> = self
            .entries
            .keys()
            .filter(|dir| !desired.contains(*dir))
            .cloned()
            .collect();
        let mut removed = 0usize;
        for dir in stale {
            if let Some(id) = self.entries.remove(&dir) {
                removed += 1;
                deregister_id(id, &dir);
            }
        }

        // ---- Добавление недостающих (в desired, но нет в карте) ----------
        let mut added = 0usize;
        for dir in &desired {
            if self.entries.contains_key(dir) {
                continue; // уже подписаны — дифф, не трогаем
            }
            // SHParseDisplayName требует инициализированного COM на ЭТОМ
            // потоке (STA ставится в shell_events_loop ДО регистраций,
            // T16-B). Путь — широкая строка с backslash: normalize_path
            // вернула прямые слэши, а shell-парсер отклоняет смешанные
            // разделители (приём thumbs.rs, T6).
            let wide = HSTRING::from(dir.to_string_lossy().replace('/', "\\"));
            let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
            // SAFETY: wide — HSTRING (null-terminated UTF-16), живёт до конца
            // вызова; pbc=None (бинд-контекст не нужен); sfgaoin=0 (атрибуты
            // не запрашиваем), psfgaoout=None; pidl — валидный out-указатель:
            // при успехе shell пишет туда свежий ITEMIDLIST, ownership — наш
            // (освобождение ILFree ниже); провал — Err, pidl остаётся null.
            match unsafe { SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None) } {
                Err(err) => {
                    if err.code() == CO_E_NOTINITIALIZED {
                        // COM на потоке шины не встал (провал CoInitializeEx в
                        // window.rs, план §7): ВСЕ парсы этого синка провалятся
                        // той же ошибкой — причина одна, диагностируем её.
                        tracing::warn!(
                            %err,
                            "SHChangeNotify: COM на потоке шины не инициализирован (R12, подписки отключены)"
                        );
                    } else {
                        tracing::warn!(
                            %err,
                            dir = %dir.display(),
                            "SHChangeNotify: директория не парсится (R12, ретрай следующим синком)"
                        );
                    }
                }
                Ok(()) => {
                    // Подписка (R12): ShellLevel (события shell, вкл. корзину)
                    // | NewDelivery (lParam = HANDLE нотификации — план §8.6);
                    // fRecursive=false — рекурсия по поддереву дала бы шум
                    // массовых операций Explorer (коалессер сгладит, но
                    // подписываемся точечно, как T10 — по каталогу ноды).
                    let entry = SHChangeNotifyEntry {
                        pidl,
                        fRecursive: false.into(),
                    };
                    // SAFETY: hwnd — живое окно шины; entry — корректно
                    // инициализированная запись с pidl из успешного парса
                    // (единственный владелец — этот кадр стека); msg —
                    // приватный WM_APP-ид; Register копирует данные entry в
                    // свою регистрацию (pidl остаётся нашим — ILFree ниже
                    // ВСЕГДА); 0 = провал (без UB).
                    let id = unsafe {
                        SHChangeNotifyRegister(
                            hwnd,
                            SHCNRF_ShellLevel | SHCNRF_NewDelivery,
                            disk_events_mask(),
                            msg,
                            1,
                            &entry,
                        )
                    };
                    // SAFETY: pidl — наш ownership после успешного
                    // SHParseDisplayName (Register копирует данные внутрь
                    // подписки — и при успехе, и при провале); освобождаем
                    // ВСЕГДА, чтобы не течь на каждом ретрае.
                    unsafe { ILFree(Some(pidl)) };
                    if id != 0 {
                        added += 1;
                        self.entries.insert(dir.clone(), id);
                    } else {
                        // Не в карте → следующий sync ретраит (R14)
                        tracing::warn!(
                            dir = %dir.display(),
                            "SHChangeNotifyRegister провален (R12, ретрай следующим синком)"
                        );
                    }
                }
            }
        }

        if added > 0 || removed > 0 {
            tracing::debug!(
                added,
                removed,
                active = self.entries.len(),
                "FileNotifySync: дифф-синк SHChangeNotify-подписок"
            );
        }
    }

    /// Снять все подписки (Shutdown).
    pub fn clear(&mut self, hwnd: HWND) {
        // Снятие подписок адресуется registration id (hwnd не участвует) —
        // параметр сохранён для API-симметрии с sync (док-коммент модуля).
        let _ = hwnd;
        let entries = std::mem::take(&mut self.entries);
        for (dir, id) in entries {
            deregister_id(id, &dir);
        }
    }
}

/// Маска дисковых SHCNE-событий для SHChangeNotifyRegister (R12, план §8.5):
/// суп констант ядра `shcne` (mod.rs) = ShlObj.h SHCNE_DISKEVENTS
/// (0x0002381F = 145439) БЕЗ SHCNE_ATTRIBUTES (0x800) — атрибутные события
/// не мапятся в FileEvent (ядро T16-A их не декодирует), подписка на них
/// была бы шумом без потребителя. В windows-crate SHCNE_DISKEVENTS есть
/// только как SHCNE_ID(u32) при сигнатуре Register(fevents: i32) — маску
/// собираем из констант ядра и сверяем со значением крейта.
fn disk_events_mask() -> i32 {
    let mask = shcne::RENAMEITEM
        | shcne::CREATE
        | shcne::DELETE
        | shcne::MKDIR
        | shcne::RMDIR
        | shcne::UPDATEDIR
        | shcne::UPDATEITEM
        | shcne::RENAMEFOLDER;
    // Сверка с windows-crate (ShlObj.h): маска = DISKEVENTS минус атрибутный
    // бит — ловит рассинхрон констант ядра при апгрейде крейта (T15-A).
    debug_assert_eq!(
        mask as u32,
        windows::Win32::UI::Shell::SHCNE_DISKEVENTS.0
            & !windows::Win32::UI::Shell::SHCNE_ATTRIBUTES.0,
        "SHCNE-маска ядра разошлась с SHCNE_DISKEVENTS windows-crate"
    );
    mask
}

/// Снять одну SHChangeNotify-подписку (R12): подписки адресуются
/// registration id (hwnd не нужен). FALSE = провал — warn (подписка могла
/// уже умереть вместе с shell/недоступна — не фатально, R14).
fn deregister_id(id: u32, dir: &Path) {
    // SAFETY: id — валидный идентификатор активной подписки (получен из
    // SHChangeNotifyRegister, изымается из карты ровно один раз);
    // устаревший/чужой id — FALSE → warn, без UB.
    if !unsafe { SHChangeNotifyDeregister(id) }.as_bool() {
        tracing::warn!(dir = %dir.display(), "SHChangeNotifyDeregister вернул FALSE");
    }
}
