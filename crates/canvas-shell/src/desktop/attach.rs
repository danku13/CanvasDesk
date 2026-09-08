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
//!
//! Classic: scrub → SetParent(hwnd, worker_w). Затем (обе схемы): окно на
//! весь виртуальный экран + верификация стилей перечитыванием (R3).
//! Любая ошибка шага → AttachError → фолбэк на обычное окно (R14).
//! Чистая реализация по описанию механики (RECIPES §0).

use super::hierarchy::{DesktopHierarchy, HierarchyError};
use thiserror::Error;
use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindow, GetWindowLongPtrW, SetLayeredWindowAttributes, SetParent, SetWindowLongPtrW,
    SetWindowPos, GWL_EXSTYLE, GWL_STYLE, GW_CHILD, GW_HWNDNEXT, HWND_BOTTOM, LWA_ALPHA,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WINDOW_LONG_PTR_INDEX, WS_EX_LAYERED,
    WS_EX_NOACTIVATE,
};

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
    let raised = matches!(hierarchy.strategy, super::EmbedStrategy::Raised);

    // ---- R2 шаг 1 / R3: стиль-скраббинг ДО SetParent -------------------
    // Текущие стили читаются первым делом: план (WS_CHILDWINDOW, −CLIP-
    // SIBLINGS, −APPWINDOW/WINDOWEDGE/ACCEPTFILES, +NOACTIVATE, для Raised
    // +LAYERED) применяется ОБА поля; сбой чтения/записи ловится либо
    // немедленной перечиткой в write_style_field, либо верификацией шага 7
    // (провал чтения даёт 0 → план ≠ факта → StyleMismatch → фолбэк R14).
    let cur_style = read_style_field(hwnd, GWL_STYLE);
    let cur_exstyle = read_style_field(hwnd, GWL_EXSTYLE);
    let plan = super::plan_style_scrub(cur_style, cur_exstyle, raised);
    write_style_field(hwnd, GWL_STYLE, plan.style, super::StyleField::Style)?;
    write_style_field(hwnd, GWL_EXSTYLE, plan.exstyle, super::StyleField::ExStyle)?;
    tracing::debug!(style = plan.style, exstyle = plan.exstyle, "attach: scrub");

    // ---- R2 шаг 2 (ТОЛЬКО Raised): layered ДО SetParent ----------------
    // Бит WS_EX_LAYERED уже установлен scrub-планом (raised=true); здесь
    // — layered-атрибуты: bAlpha=255 — полная непрозрачность (DX blt
    // present, требование Microsoft). На Classic шаг пропускается.
    if raised {
        // Сверка локальной константы модуля A со значением windows-crate
        // (WinUser.h): расхождение — баг посева констант, ловим в debug.
        debug_assert_eq!(
            super::WS_EX_LAYERED,
            WS_EX_LAYERED.0,
            "WS_EX_LAYERED разошёлся с windows-crate"
        );
        // SAFETY: hwnd валиден (живое winit-окно этого процесса); crkey
        // при LWA_ALPHA не используется (не цветовой ключ), COLORREF(0) —
        // нейтральное значение; bAlpha=255 — полная непрозрачность.
        if let Err(err) = unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) } {
            tracing::warn!(%err, "attach: SetLayeredWindowAttributes провален (R2 шаг 2)");
            return Err(AttachError::SetLayeredFailed);
        }
        tracing::debug!("attach: layered (bAlpha=255) до SetParent");
    }

    // ---- R2 шаг 3: SetParent -------------------------------------------
    // Raised: parent = Progman (НЕ WorkerW!); Classic: parent = WorkerW.
    let parent = if raised {
        hierarchy.progman
    } else {
        hierarchy.worker_w
    };
    // SAFETY: оба хэндла из детекта иерархии (T15-B, валидность
    // перечитывается детектом); смена родителя — единственный необратимый
    // шаг attach, провал уводит в фолбэк R14 (окно остаётся top-level,
    // обрабатывает T15-E). Err от windows-rs покрывает и NULL-возврат.
    if let Err(err) = unsafe { SetParent(hwnd, Some(parent)) } {
        tracing::warn!(%err, "attach: SetParent провален (R2 шаг 3)");
        return Err(AttachError::SetParentFailed);
    }
    tracing::debug!(parent = ?parent, "attach: set_parent");

    // ---- R2 шаги 4–5 (ТОЛЬКО Raised): Z-order --------------------------
    // Шаг 4: insert-after = def_view — встаём в Z-order сразу ПОД слоем
    // иконок (DefView поверх нас). Шаг 5 (ensure_worker_w_z_order) —
    // WorkerW остаётся ПОСЛЕДНИМ ребёнком Progman (обои под нами).
    if raised {
        // SAFETY: hwnd/def_view валидны (детект T15-B); флаги
        // NOMOVE|NOSIZE|NOACTIVATE — меняется только Z-order, ни позиция,
        // ни размер, ни фокус не затрагиваются.
        if let Err(err) = unsafe {
            SetWindowPos(
                hwnd,
                Some(hierarchy.def_view),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        } {
            tracing::warn!(%err, "attach: SetWindowPos Z-order провален (R2 шаг 4)");
            return Err(AttachError::SetWindowPosFailed);
        }
        tracing::debug!("attach: z_order под DefView");
        // R2 шаг 5 — WorkerW последним ребёнком Progman.
        ensure_worker_w_z_order(hierarchy)?;
    }

    // ---- Обе схемы: окно на весь виртуальный экран ----------------------
    // Координаты — экранные: клиентская область WorkerW/Progman совпадает
    // с виртуальным экраном (план §3). SWP_NOZORDER — Z-order уже выставлен
    // шагом 4 (Raised) либо не требуется (Classic — единственный ребёнок
    // top-level WorkerW); при NOZORDER hWndInsertAfter игнорируется (None).
    // SAFETY: hwnd валиден; width/height виртуального экрана (T15-B,
    // EnumDisplayMonitors) неотрицательны; set_parent уже прошёл —
    // координаты интерпретируются относительно нового родителя.
    if let Err(err) = unsafe {
        SetWindowPos(
            hwnd,
            None,
            screen.left,
            screen.top,
            screen.width(),
            screen.height(),
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } {
        tracing::warn!(%err, "attach: SetWindowPos (виртуальный экран) провален");
        return Err(AttachError::SetWindowPosFailed);
    }
    tracing::debug!(w = screen.width(), h = screen.height(), "attach: screen");

    // ---- R3: верификация стилей перечитыванием ПОСЛЕ репарентинга ------
    // winit/tao восстанавливают стили асинхронно, «не зная» о репарентинге
    // (урок Seelen) — перечитать ОБА поля и сверить с планом; расхождение
    // → фолбэк-причина (R14: повторный scrub НЕ ретраится в T15, план §7).
    let fact_style = read_style_field(hwnd, GWL_STYLE);
    let fact_exstyle = read_style_field(hwnd, GWL_EXSTYLE);
    if let Err(mismatch) = super::verify_styles(fact_style, fact_exstyle, &plan) {
        tracing::warn!(
            ?mismatch,
            "attach: стили после репарентинга не совпали (R3)"
        );
        return Err(AttachError::StyleMismatch(mismatch));
    }
    tracing::debug!("attach: verify_ok");

    Ok(AttachOutcome {
        strategy: hierarchy.strategy,
        screen,
    })
}

/// WorkerW — последний ребёнок Progman (R2 шаг 5): GetWindow(progman,
/// GW_CHILD) → обход GW_HWNDNEXT до GW_HWNDLAST; если последний != worker_w
/// → SetWindowPos(worker_w, HWND_BOTTOM, NOACTIVATE|NOMOVE|NOSIZE).
/// Вызывается внутри attach и при refresh_z_order.
pub fn ensure_worker_w_z_order(hierarchy: &DesktopHierarchy) -> Result<(), AttachError> {
    // SAFETY: progman из детекта (T15-B); GetWindow — чистое чтение
    // родственных связей, состояние окон не меняет.
    let first = unsafe { GetWindow(hierarchy.progman, GW_CHILD) };
    // Err от GetWindow = NULL: у Progman нет ИЛИ недоступен первый ребёнок
    // (иерархия сломана — DefView исчез?) — фиксировать некого; Ok без
    // действий: recovery придёт от монитора (WorkerWDestroyed, R6).
    let Ok(mut last) = first else {
        tracing::warn!("Progman без детей — Z-order WorkerW не проверяем (recovery: монитор R6)");
        return Ok(());
    };
    // Последний ребёнок: линейный обход GW_HWNDNEXT до NULL (Err — конец
    // списка; список детей линейный, зацикливание исключено).
    // SAFETY: last — хэндл из этого же обхода (действительное окно);
    // чистое чтение, состояние не меняет.
    while let Ok(next) = unsafe { GetWindow(last, GW_HWNDNEXT) } {
        last = next;
    }
    if last != hierarchy.worker_w {
        // Случай реальный (Lively: «Unexpected WorkerW Z-order») — WorkerW
        // оказался не последним ребёнком; принудительно в самый низ, иначе
        // обои перекроют наш канвас.
        tracing::warn!("Unexpected WorkerW Z-order — принудительно в HWND_BOTTOM");
        // SAFETY: worker_w из детекта; HWND_BOTTOM — псевдо-хэндл (HWND(1)),
        // не владеющий; NOMOVE|NOSIZE|NOACTIVATE — меняется только Z-order.
        if let Err(err) = unsafe {
            SetWindowPos(
                hierarchy.worker_w,
                Some(HWND_BOTTOM),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )
        } {
            tracing::warn!(%err, "ensure_worker_w_z_order: SetWindowPos провален");
            return Err(AttachError::SetWindowPosFailed);
        }
    }
    Ok(())
}

/// Восстановление после разрушения WorkerW на Raised (R2-симметрия):
/// перевыполнить ТОЛЬКО шаги Z-order (4–5) — полный re-parent не нужен.
/// (Classic обрабатывается полным re-attach на уровне приложения.)
pub fn refresh_z_order(hwnd: HWND, hierarchy: &DesktopHierarchy) -> Result<(), AttachError> {
    // R2: «на raised достаточно перевыполнить шаги 4–5, полный reset не
    // нужен». На Classic Z-order-фикс неприменим (worker_w — top-level,
    // а не ребёнок Progman: insert-after def_view из чужой sibling-группы
    // и HWND_BOTTOM на top-level разрушили бы раскладку) — здесь только
    // no-op: recovery_action (модуль A) отдаёт Classic FullReattach.
    if !matches!(hierarchy.strategy, super::EmbedStrategy::Raised) {
        tracing::warn!("refresh_z_order на Classic пропущен — нужен полный re-attach (R2)");
        return Ok(());
    }
    // R2 шаг 4: снова под DefView (WorkerW пересоздан мог сдвинуть нас).
    // SAFETY: hwnd — наше окно (живо, иначе recovery не дошёл бы),
    // def_view из детекта; NOMOVE|NOSIZE|NOACTIVATE — только Z-order.
    if let Err(err) = unsafe {
        SetWindowPos(
            hwnd,
            Some(hierarchy.def_view),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    } {
        tracing::warn!(%err, "refresh_z_order: SetWindowPos Z-order провален");
        return Err(AttachError::SetWindowPosFailed);
    }
    tracing::debug!("refresh_z_order: окно под DefView");
    // R2 шаг 5: WorkerW — последний ребёнок Progman.
    ensure_worker_w_z_order(hierarchy)?;
    Ok(())
}

/// Снять WS_EX_NOACTIVATE — первый клик пользователя по канвасу
/// (TASKS T15: «WS_EX_NOACTIVATE до первого клика»). Идемпотентен.
pub fn enable_activation(hwnd: HWND) -> Result<(), AttachError> {
    // Сверка локальной константы модуля A со значением windows-crate.
    debug_assert_eq!(
        super::WS_EX_NOACTIVATE,
        WS_EX_NOACTIVATE.0,
        "WS_EX_NOACTIVATE разошёлся с windows-crate"
    );
    // SAFETY: hwnd валиден (окно живо — пользователь только что кликнул);
    // GWL_EXSTYLE — константа индекса; чтение без побочных эффектов.
    let exstyle = (unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) }) as u32;
    let cleared = exstyle & !super::WS_EX_NOACTIVATE;
    if cleared == exstyle {
        return Ok(()); // бит уже снят — идемпотентность
    }
    // SAFETY: hwnd валиден; cleared — 32-битное поле в LONG_PTR (старшие
    // биты нулевые, стили WinUser.h — u32); затрагивается только это окно.
    if unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, cleared as isize) } == 0 {
        // 0 = провал ИЛИ легальное прежнее пустое поле (см.
        // write_style_field): перечитываем; бит ещё стоит — warn, окно
        // остаётся NOACTIVATE и повторный клик повторит вызов (не ошибка
        // встройки — фолбэк R14 не нужен).
        // SAFETY: то же чтение, что и выше.
        if (unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) }) as u32 & super::WS_EX_NOACTIVATE != 0 {
            tracing::warn!("enable_activation: WS_EX_NOACTIVATE снять не удалось");
        }
        return Ok(());
    }
    tracing::debug!("enable_activation: WS_EX_NOACTIVATE снят");
    Ok(())
}

/// DPI окна через GetDpiForWindow (R10; после репарентинга
/// window.scale_factor() врёт — поллится монитором T15-D).
pub fn window_dpi(hwnd: HWND) -> u32 {
    // SAFETY: чистое чтение DPI; hwnd мог потерять валидность между тиками
    // поллинга монитора (T15-D) — тогда GetDpiForWindow вернёт 0.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    // 0 — деградация (окно потерялось): возвращаем КАК ЕСТЬ — решение за
    // вызывающим: монитор (T15-D) держит последний валидный DPI и ноль
    // игнорирует; деградационную политику здесь не прячем.
    dpi
}

/// Чтение GWL_STYLE/GWL_EXSTYLE как u32: стили — 32-битные поля WinUser.h,
/// старшие 32 бита LONG_PTR для этих индексов всегда нулевые. Провал чтения
/// (MS-контракт: 0) отдельно не диагностируется — scrub + верификация R3
/// замыкают цепочку: 0 уходит в план → фактические стили ≠ плану →
/// AttachError::StyleMismatch → фолбэк R14.
fn read_style_field(hwnd: HWND, index: WINDOW_LONG_PTR_INDEX) -> u32 {
    // SAFETY: hwnd обязан быть живым окном этого процесса (создан winit в
    // resumed(), attach вызывается сразу после); index — константа
    // GWL_STYLE/GWL_EXSTYLE; вызов ничего не выделяет и не владеет.
    (unsafe { GetWindowLongPtrW(hwnd, index) }) as u32
}

/// Запись GWL_STYLE/GWL_EXSTYLE по плану: MS-контракт SetWindowLongPtrW —
/// «возврат = прежнее значение, 0 = провал», но ноль бывает и легальным
/// прежним значением (пустое поле) — при 0 поле перечитывается: расхождение
/// с планом и есть честная диагностика (StyleMismatch, R14), совпадение —
/// значение фактически применено (успех).
fn write_style_field(
    hwnd: HWND,
    index: WINDOW_LONG_PTR_INDEX,
    planned: u32,
    field: super::StyleField,
) -> Result<(), AttachError> {
    // SAFETY: hwnd — живое winit-окно этого процесса; index — константа
    // GWL_STYLE/GWL_EXSTYLE; planned расширяется до isize без потерь
    // (старшие биты нулевые); запись затрагивает только это окно.
    let prev = unsafe { SetWindowLongPtrW(hwnd, index, planned as isize) };
    if prev != 0 {
        return Ok(()); // MS-контракт: ненулевой возврат = успех
    }
    // SAFETY: то же чтение, что и в read_style_field.
    let actual = (unsafe { GetWindowLongPtrW(hwnd, index) }) as u32;
    if actual != planned {
        return Err(AttachError::StyleMismatch(super::StyleMismatch {
            field,
            expected: planned,
            actual,
        }));
    }
    Ok(())
}

/// Сообщение фолбэка (R14, координаторская интеграционная точка T15-E):
/// любая ошибка шага встройки → пользователь обязан увидеть, почему
/// приложение работает в оконном режиме (TASKS T15), а не молчаливый warn.
/// MessageBoxW блокирует до клика — вызывать один раз при провале attach.
// SAFETY: MessageBoxW с owner HWND(0) безопасен из любого потока; текст
// и заголовок — валидные null-terminated UTF-16, собранные из &str.
pub fn fallback_message_box(reason: &str) {
    use windows::core::PCWSTR;
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
    let text: Vec<u16> = format!(
        "CanvasDesk: не удалось встроить канвас в рабочий стол.\n{reason}\n\n\
         Приложение продолжит работу в оконном режиме."
    )
    .encode_utf16()
    .chain(std::iter::once(0))
    .collect();
    let caption: Vec<u16> = "CanvasDesk"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    // SAFETY: см. док-блок.
    unsafe {
        MessageBoxW(
            None,
            PCWSTR::from_raw(text.as_ptr()),
            PCWSTR::from_raw(caption.as_ptr()),
            MB_OK | MB_ICONWARNING,
        );
    }
}
