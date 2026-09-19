//! M8/W4 (прошивка, wasm-port §3.4): web-стратегия async-инициализации
//! Renderer — `wasm_bindgen_futures::spawn_local` + слот доставки.
//!
//! Браузерный главный поток нельзя блокировать: futура `Renderer::new`
//! (wgpu-web: адаптер/устройство) разрешается только оборотом JS event
//! loop — `pollster::block_on` закрутит вечное ожидание и заморозит
//! страницу (план §7, риск «pollster-блокировка потока на wasm»).
//! Поэтому: футура уходит в `spawn_local`, результат кладётся в слот
//! (`RendererSlot`), кадр-цикл будится `request_redraw` — App забирает
//! слот первым `RedrawRequested` после готовности GPU и рисует первый
//! кадр (кадры до готовности пропускаются — renderer `None`, R14).
//!
//! Нативная компиляция (rlib-тесты каркаса): футура не стартует —
//! `launch` возвращает `Pending` с пустым слотом (контракт типа без
//! JS-рунтайма); web-путь — под `cfg(target_arch = "wasm32")` (правило
//! §3.1: платформенный код живёт в canvas-web).

use std::sync::Arc;

use canvas_render::renderer_init::{RendererLaunch, RendererLauncher, RendererSlot};
use winit::window::Window;

/// Web-стратегия запуска инициализации Renderer (инъекция в `App::new`,
/// паттерн W3-сервисов; натив — `BlockOnRendererLaunch` в canvas-render).
pub struct SpawnLocalRendererLaunch;

impl RendererLauncher for SpawnLocalRendererLaunch {
    fn launch(&self, window: Arc<Window>, prefer_dx12: bool) -> RendererLaunch {
        let slot = RendererSlot::new();
        #[cfg(target_arch = "wasm32")]
        {
            // winit 0.30 создаёт canvas при create_window, но НЕ вставляет
            // его в DOM (with_append по умолчанию выключен; attrs строятся
            // в canvas-app — §3.1: web-знания туда не идут). Платформенный
            // слой вставляет сам, ДО async-init: ResizeObserver winit'а
            // успеет присылать Resized к готовности GPU — первый кадр
            // сразу с реальным размером (CSS: body > canvas на весь экран)
            attach_canvas_to_dom(&window);
            let deliver = slot.clone();
            let wake = window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let result = canvas_render::Renderer::new(window, prefer_dx12).await;
                deliver.put(result);
                // GPU готов: разбудить кадр-цикл — App заберёт слот в
                // RedrawRequested и нарисует первый кадр
                wake.request_redraw();
            });
        }
        // Натив (rlib-тесты каркаса): футура не стартует — параметры не
        // нужны, слот навсегда пуст; контракт launch на типе соблюдён
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (&window, prefer_dx12);
        RendererLaunch::Pending(slot)
    }
}

/// Вставка winit-канваса в DOM (wasm): winit 0.30 не делает этого сам
/// (with_append выключен по умолчанию, attrs — в платформенно-нейтральном
/// canvas-app). Идемпотентно: если канвас уже в документе — ничего не
/// делает (перезапуск модуля, повторный launch).
#[cfg(target_arch = "wasm32")]
fn attach_canvas_to_dom(window: &Window) {
    use wasm_bindgen::JsCast;
    use winit::platform::web::WindowExtWebSys;
    let Some(canvas) = window.canvas() else {
        tracing::warn!("winit-окно без канваса — вставка в DOM пропущена");
        return;
    };
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        tracing::warn!("document недоступен — канвас не вставлен в DOM");
        return;
    };
    let node = canvas.unchecked_into::<web_sys::Node>();
    if document.contains(Some(&node)) {
        return;
    }
    let Some(body) = document.body() else {
        tracing::warn!("body недоступен — канвас не вставлен в DOM");
        return;
    };
    if let Err(err) = body.append_child(&node) {
        tracing::warn!(?err, "не удалось вставить канвас в DOM");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Нативная компиляция (rlib-тесты каркаса): стратегия реализует
    /// контракт `RendererLauncher`. Вызов `launch` требует живого event
    /// loop и окна — полная цепочка проверяется браузерным дымом
    /// canvas-web (приёмка W4).
    #[test]
    fn implements_renderer_launcher_contract() {
        fn assert_impl<L: RendererLauncher>(_: &L) {}
        assert_impl(&SpawnLocalRendererLaunch);
    }
}
