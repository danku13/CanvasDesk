//! FR-WASM-02: pre-flight проверка GPU-возможностей браузера + DOM-заглушка.
//!
//! Чёрный экран «GPU-адаптер не найден» (консоль — единственный канал
//! ошибки) закрывает пользовательский опыт, даже если причина понятна из
//! логов. Модуль зеркалит двухступенчатый выбор `canvas_render` (FR-WASM-02,
//! `create_gpu_web`): WebGPU → WebGL2 — и ДО старта App решает:
//! - есть WebGPU-адаптер или WebGL2-контекст → приложение стартует
//!   (wgpu выберет бэкенд сам, те же две ступени);
//! - не работает ни то, ни другое → читаемая DOM-заглушка с причинами и
//!   советами, приложение не стартует (у wgpu всё равно не было бы адаптера).
//!
//! Пробы дешёвые (один requestAdapter-промис + один throwaway-canvas) и
//! точные: повторяют ровно те же браузерные вызовы, что делает wgpu.
//!
//! Правило §3.1: web-костыли живут в web-слое (canvas-web), ядро не патчится.

use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// Проверить GPU-возможности; `false` — ни WebGPU, ни WebGL2 (заглушка
/// уже показана). Вызывается из `spawn_desk_web` до построения App.
pub(crate) async fn ensure_gpu_or_show_overlay() -> bool {
    if probe_webgpu().await || probe_webgl2() {
        return true;
    }
    show_fallback_overlay();
    false
}

/// WebGPU-проба: `navigator.gpu?.requestAdapter()` разрешается не-null?
/// Отсутствие `navigator.gpu`, не-функция `requestAdapter` и отклонённый
/// промис трактованы как «нет адаптера» — то же, что увидит wgpu.
async fn probe_webgpu() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let navigator = window.navigator();
    let Ok(gpu) = js_sys::Reflect::get(&navigator, &"gpu".into()) else {
        return false;
    };
    if gpu.is_null() || gpu.is_undefined() {
        return false;
    }
    let request = js_sys::Reflect::get(&gpu, &"requestAdapter".into())
        .ok()
        .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
    let Some(request) = request else {
        return false;
    };
    let promise = request
        .call0(&gpu)
        .ok()
        .map(|v| v.unchecked_into::<js_sys::Promise>());
    let Some(promise) = promise else {
        return false;
    };
    matches!(JsFuture::from(promise).await, Ok(adapter) if adapter.is_truthy())
}

/// WebGL2-проба: throwaway-канвас получает контекст `webgl2`?
/// wgpu GLES-бэкенд создаёт контекст тем же вызовом на рабочем канвасе;
/// отдельный элемент не конфликтует с ним (контексты по канвасам независимы).
fn probe_webgl2() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Some(document) = window.document() else {
        return false;
    };
    let Ok(canvas) = document.create_element("canvas") else {
        return false;
    };
    let canvas: web_sys::HtmlCanvasElement = canvas.unchecked_into();
    // web-sys 0.3.104: get_context негенериковый — возвращает Option<Object>
    matches!(canvas.get_context("webgl2"), Ok(Some(_)))
}

/// DOM-заглушка вместо молчаливого чёрного экрана: оба бэкенда недоступны.
/// Стили инлайном (заглушка живёт до перезагрузки, CSS index.html её не
/// знает); палитра — тон продукта (#14161a/#d5d9e0, SPEC §6).
fn show_fallback_overlay() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(body) = document.body() else {
        return;
    };
    let Ok(box_el) = document.create_element("div") else {
        return;
    };
    let _ = box_el.set_attribute(
        "style",
        "position:fixed;inset:0;z-index:100;display:flex;align-items:center;\
         justify-content:center;background:#14161a;color:#d5d9e0;\
         font:15px/1.6 system-ui,sans-serif;padding:24px;text-align:center;",
    );
    box_el.set_inner_html(
        "<div style=\"max-width:600px\">\
<b>CanvasDesk не смог инициализировать графику.</b><br>\
Браузер не предоставил ни WebGPU, ни WebGL2.<br><br>\
Откройте приложение в свежем Chrome, Edge или Firefox; если графика \
отключена — включите аппаратное ускорение (Chrome: \
chrome://settings/system).<br><br>\
<i style=\"opacity:.7\">CanvasDesk could not initialize graphics: \
neither WebGPU nor WebGL2 is available in this browser.</i>\
</div>",
    );
    let _ = body.append_child(&box_el);
}
