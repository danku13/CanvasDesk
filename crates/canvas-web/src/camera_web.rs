//! FR-107 (мультиканвас C4, №12/№30b): камера канваса (зум + центр) —
//! web-хуки localStorage. Ключ — [`camera_key_for`] (формат заморожен в
//! C0: `canvasdesk.camera.<имя файла>`), значение — [`encode_camera`]
//! («x;y;zoom», сериализация тестируется нативно в canvas-core).
//!
//! - СОХРАНЕНИЕ: уход с канваса (`on_open_scene` → `WebRequest::CanvasCameraSave`)
//!   и выгрузка/скрытие страницы (листенер [`install_flush`]:
//!   visibilitychange→hidden + pagehide → `AppEvent::CameraFlushRequested`
//!   → App немедленно отвечает CanvasCameraSave; дренаж TourAwareApp —
//!   тот же микротаск, localStorage-запись синхронна).
//!   Ренейм НЕ сохраняет — ключ переносится готовым `CanvasMoveCameraKey`
//!   (C3, web_requests).
//! - ЗАГРУЗКА: открытие канваса (`WebRequest::CanvasCameraLoad` → ответ
//!   `AppEvent::CanvasCameraRestored`) и старт страницы (сцена строится
//!   напрямую, без on_open_scene — `App::apply_startup_camera` в
//!   app_spawn).
//! - Деградация: нет localStorage / нет ключа / битая строка — тихий
//!   дефолт (`None` → камера как при первом открытии).
//!
//! Чисто web-модуль: web-sys/localStorage требуют JS-рунтайма (паттерн
//! `toolbar`/`web_requests`); ручные дым-сценарии — FR-107 §Проверка.

#![cfg(target_arch = "wasm32")]

use winit::event_loop::EventLoopProxy;

use canvas_app::app::AppEvent;
use canvas_core::workspace::CameraSnapshot;

/// Записать снимок камеры канваса в localStorage (№12/№30b). Ошибки — в
/// лог: камера — восстанавливаемое удобство, не критичные данные.
pub(crate) fn save(name: &str, snapshot: &CameraSnapshot) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        tracing::warn!(target: "canvas_web", "камера: localStorage недоступен (снимок не сохранён)");
        return;
    };
    let key = canvas_core::workspace::camera_key_for(name);
    let value = canvas_core::workspace::encode_camera(snapshot);
    if let Err(err) = storage.set_item(&key, &value) {
        tracing::warn!(target: "canvas_web", ?err, key, "камера: запись в localStorage не удалась");
    }
}

/// Прочитать снимок камеры канваса (№12/№30b): `None` — ключа нет или
/// строка битая (decode_camera) → потребитель тихо берёт дефолт.
pub(crate) fn load(name: &str) -> Option<CameraSnapshot> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok().flatten()?;
    let key = canvas_core::workspace::camera_key_for(name);
    let raw = storage.get_item(&key).ok().flatten()?;
    canvas_core::workspace::decode_camera(&raw)
}

/// Листенер выгрузки/скрытия страницы (№12/№30b): visibilitychange→hidden
/// и pagehide → `AppEvent::CameraFlushRequested` (App отвечает снимком
/// активного канваса — сохранение уходит в том же микротаске, до ухода
/// страницы). Паттерн `fs_folder::install_watch` (focus/visibilitychange).
pub(crate) fn install_flush(proxy: EventLoopProxy<AppEvent>) {
    use wasm_bindgen::prelude::Closure;
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else {
        return;
    };
    let send_flush = move || {
        let _ = proxy.send_event(AppEvent::CameraFlushRequested);
    };
    // visibilitychange: триггерится ДО выгрузки при уходе со страницы
    // (переключение вкладки/минимизация — страница ещё полностью жива,
    // микротаски дренажа гарантированно исполняются).
    let on_hidden = Closure::wrap(Box::new({
        let send_flush = send_flush.clone();
        move || {
            let hidden = web_sys::window()
                .and_then(|window| window.document())
                .map(|document| document.visibility_state() == web_sys::VisibilityState::Hidden)
                .unwrap_or(false);
            if hidden {
                send_flush();
            }
        }
    }) as Box<dyn FnMut()>);
    let target = window.unchecked_ref::<web_sys::EventTarget>();
    if let Err(err) = target.add_event_listener_with_callback(
        "visibilitychange",
        on_hidden.as_ref().unchecked_ref::<js_sys::Function>(),
    ) {
        tracing::warn!(target: "canvas_web", ?err, "камера: visibilitychange-листенер не установлен");
        return;
    }
    on_hidden.forget(); // singleton: живёт до выгрузки страницы
                        // pagehide: надёжный сигнал именно выгрузки (beforeunload в некоторых
                        // мобильных браузерах не приходит); дубль с visibilitychange безвреден —
                        // CameraFlushRequested идемпотентен (лишний CanvasCameraSave перезапишет
                        // тот же ключ тем же снимком).
    let on_pagehide = Closure::wrap(Box::new(send_flush) as Box<dyn FnMut()>);
    if let Err(err) = target.add_event_listener_with_callback(
        "pagehide",
        on_pagehide.as_ref().unchecked_ref::<js_sys::Function>(),
    ) {
        tracing::warn!(target: "canvas_web", ?err, "камера: pagehide-листенер не установлен");
        return;
    }
    on_pagehide.forget();
    tracing::info!(target: "canvas_web", "флеш камеры при выгрузке подключён (visibilitychange/pagehide)");
}
