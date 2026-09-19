//! M8/W6 (wasm-port §4.2): DOM-панель хранилища web-сборки (зеркало
//! файловых жестов нативной обвязки: «Открыть…»/недавние). Канвас —
//! GPU-UI winit, файловые действия браузера живут в DOM: пикер
//! (`showOpenFilePicker`), reopen недавних (requestPermission — жест),
//! экспорт (download-blob) — всё требует `window`/жеста, поэтому кнопки.
//!
//! Кнопки — статичная разметка index.html (W12 дорисует стиль); Rust
//! вешает листенеры и держит подпись «Недавние: <имя>» в синкре с
//! web_state (set_recent_label из каждой точки открытия).

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use winit::event_loop::EventLoopProxy;

use canvas_app::app::AppEvent;

/// Привязать кнопки панели к web-действиям (вызывается после event loop).
pub(crate) fn install(proxy: EventLoopProxy<AppEvent>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let recent_proxy = proxy.clone();
    bind(&document, "btn-open", move || {
        let proxy = proxy.clone();
        wasm_bindgen_futures::spawn_local(async move {
            crate::fs_access::open_from_disk(proxy).await;
        });
    });
    bind(&document, "btn-recent", move || {
        let proxy = recent_proxy.clone();
        wasm_bindgen_futures::spawn_local(async move {
            crate::fs_access::reopen_recent(proxy).await;
        });
    });
    bind(&document, "btn-export", || {
        wasm_bindgen_futures::spawn_local(async move {
            crate::export::export_active().await;
        });
    });
    tracing::info!(target: "canvas_web", "DOM-панель хранилища подключена");
}

/// Повесить обработчик клика на кнопку по id (отсутствие кнопки — warn:
/// битая разметка не должна валить запуск).
fn bind(document: &web_sys::Document, id: &str, handler: impl FnMut() + 'static) {
    let element = match document.get_element_by_id(id) {
        Some(element) => element,
        None => {
            tracing::warn!(target: "canvas_web", id, "кнопка панели не найдена в разметке");
            return;
        }
    };
    let button: web_sys::HtmlElement = match element.dyn_into() {
        Ok(button) => button,
        Err(_) => {
            tracing::warn!(target: "canvas_web", id, "элемент панели не кнопка");
            return;
        }
    };
    let closure = Closure::wrap(Box::new(handler) as Box<dyn FnMut()>);
    let _ = button.add_event_listener_with_callback(
        "click",
        closure.as_ref().unchecked_ref::<js_sys::Function>(),
    );
    closure.forget(); // singleton-панель: живёт до выгрузки страницы
}

/// Обновить подпись «Недавние» под имя активного канваса (синхронизация
/// DOM ↔ web_state; ошибки молча — декоративный элемент).
pub(crate) fn set_recent_label(name: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if let Some(element) = document.get_element_by_id("btn-recent") {
        element
            .unchecked_ref::<web_sys::Node>()
            .set_text_content(Some(&format!("Недавние: {name}")));
    }
}
