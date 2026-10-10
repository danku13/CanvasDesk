//! FR-104 (мультиканвас C1, №14b/№35a): Web Locks — детект «этот канвас
//! уже открыт в другой вкладке». Лок захватывается на имя активного
//! канваса при каждом переключении (`web_state::set_active` — единая
//! точка открытия на web); предыдущий лок отпускается (внутри глю
//! `locksAcquire`, см. index.html). Занятость → модал №35a
//! (`AppEvent::CanvasLockBusy` → `AppDialog::CanvasTabBusy`).
//!
//! Имя лока — чистая функция с префиксом и скоупом хранилища: OPFS- и
//! дисковые канвасы с одинаковым именем файла — РАЗНЫЕ канвасы (лочить
//! друг друга не должны), а вот два таба на один OPFS-файл — одна и та
//! же запись конфликта (тихий last-write-wins, который детектим).
//!
//! API: биндинг `web_sys::LockManager` существует, но требует
//! `--cfg=web_sys_unstable_apis` (сборка его не передаёт) — поэтому захват
//! идёт через JS-глю `locksAcquire` (сериализованная цепочка, лок
//! удерживается до переключения/выгрузки страницы). Firefox/Safari без
//! `navigator.locks` — честный `Unsupported` (детекта нет, прежнее
//! поведение last-write-wins).

use crate::web_state::ActiveKind;

/// Имя Web Lock для канваса (чистая функция, нативные тесты):
/// `canvasdesk.canvas.<opfs|disk>.<имя файла>` — префикс исключает
/// коллизии с будущими локами приложения, скоуп — разделяет хранилища
/// (OPFS `x.canvas` ≠ дисковый `x.canvas`).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // потребитель — imp (wasm); натив: только тесты
pub(crate) fn lock_name_for(kind: ActiveKind, file_name: &str) -> String {
    let scope = match kind {
        ActiveKind::Opfs => "opfs",
        ActiveKind::Disk => "disk",
    };
    format!("canvasdesk.canvas.{scope}.{file_name}")
}

// ============================================================================
// Wasm-часть: захват лока через JS-глю
// ============================================================================

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::lock_name_for;
    use crate::web_state::ActiveKind;
    use canvas_app::app::AppEvent;
    use std::cell::RefCell;

    /// Исход захвата (строка-протокол глю `locksAcquire`).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum LockOutcome {
        /// Лок взят и удерживается этой вкладкой.
        Held,
        /// Занят другой вкладкой — показать модал №35a.
        Busy,
        /// `navigator.locks` недоступен (Safari/Firefox) — без детекта.
        Unsupported,
    }

    thread_local! {
        /// Занятость, обнаруженная ДО регистрации event-прокси (старт:
        /// init_scene → set_active раньше построения event loop) —
        /// spawn_desk_web заберёт и отдаст App как pending-флаг модала.
        static PENDING_BUSY: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    /// Забрать накопленную на старте занятость (идемпотентно).
    pub(crate) fn take_pending_busy() -> Option<String> {
        PENDING_BUSY.with(|cell| cell.borrow_mut().take())
    }

    /// Захват лока при смене активного канваса (вызывает
    /// `web_state::set_active`; fire-and-forget): занятость → событие
    /// `CanvasLockBusy` (через event-прокси; на старте — pending).
    pub(crate) fn on_active_change(name: &str, kind: ActiveKind) {
        let lock_name = lock_name_for(kind, name);
        let canvas = name.to_owned();
        wasm_bindgen_futures::spawn_local(async move {
            match acquire(&lock_name).await {
                LockOutcome::Held => tracing::debug!(
                    target: "canvas_web",
                    canvas = %canvas,
                    "Web Locks: лок удержан"
                ),
                LockOutcome::Unsupported => tracing::debug!(
                    target: "canvas_web",
                    canvas = %canvas,
                    "Web Locks не поддержан браузером — без детекта вкладок"
                ),
                LockOutcome::Busy => {
                    tracing::warn!(
                        target: "canvas_web",
                        canvas = %canvas,
                        "Web Locks: канвас уже открыт в другой вкладке"
                    );
                    match crate::web_state::event_proxy() {
                        Some(proxy) => {
                            let _ = proxy.send_event(AppEvent::CanvasLockBusy { name: canvas });
                        }
                        // Старт: прокси ещё не создан — занятость копится,
                        // App получит её pending-флагом (первый кадр).
                        None => PENDING_BUSY.with(|cell| {
                            *cell.borrow_mut() = Some(canvas);
                        }),
                    }
                }
            }
        });
    }

    /// Захват через глю: `"held" | "busy" | "unsupported"` → исход.
    async fn acquire(lock_name: &str) -> LockOutcome {
        let Some(Ok(answer)) = crate::js_glue::call("locksAcquire", &[lock_name.into()]).await
        else {
            return LockOutcome::Unsupported;
        };
        match answer.as_string().as_deref() {
            Some("held") => LockOutcome::Held,
            Some("busy") => LockOutcome::Busy,
            _ => LockOutcome::Unsupported,
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) use imp::{on_active_change, take_pending_busy};

// ============================================================================
// Нативные тесты: имя лока (чистая функция)
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// №14b: формат имени — префикс приложения + скоуп хранилища + файл.
    #[test]
    fn lock_name_scopes_storage_kind() {
        assert_eq!(
            lock_name_for(ActiveKind::Opfs, "default.canvas"),
            "canvasdesk.canvas.opfs.default.canvas"
        );
        assert_eq!(
            lock_name_for(ActiveKind::Disk, "default.canvas"),
            "canvasdesk.canvas.disk.default.canvas"
        );
        // Одинаковое имя в разных хранилищах — РАЗНЫЕ локи (не конфликтуют)
        assert_ne!(
            lock_name_for(ActiveKind::Opfs, "x.canvas"),
            lock_name_for(ActiveKind::Disk, "x.canvas")
        );
        // Кириллица и пробелы проходят как есть (Web Locks — произвольные
        // строки, это не URL)
        assert_eq!(
            lock_name_for(ActiveKind::Opfs, "мой проект.canvas"),
            "canvasdesk.canvas.opfs.мой проект.canvas"
        );
        // Разные канвасы одного хранилища — разные локи
        assert_ne!(
            lock_name_for(ActiveKind::Opfs, "a.canvas"),
            lock_name_for(ActiveKind::Opfs, "b.canvas")
        );
    }
}
