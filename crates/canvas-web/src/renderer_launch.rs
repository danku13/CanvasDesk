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
            // FR-091 v2: согласие analytics=true → сессия записывается
            // (PostHog session recording читает канвас растром). Сообщаем
            // рендереру «capture-режим»: create_gpu_web пропустит
            // WebGPU-ступень (растр webgpu-канваса рекордеру нечитаем —
            // posthog/posthog#57008), WebGL2 читается при
            // preserveDrawingBuffer:true (web-шим index.html ставит его
            // до создания контекста). Ставится до spawn_local — флаг
            // гарантированно прочитан внутри Renderer::new. Правило §3.1:
            // согласия/JS-состояние читает web-слой, ядро не знает про
            // PostHog.
            canvas_render::renderer::set_prefer_gl_for_capture(analytics_recording_active());
            // winit 0.30 создаёт canvas при create_window, но НЕ вставляет
            // его в DOM (with_append по умолчанию выключен; attrs строятся
            // в canvas-app — §3.1: web-знания туда не идут). Платформенный
            // слой вставляет сам, ДО async-init: ResizeObserver winit'а
            // успеет присылать Resized к готовности GPU — первый кадр
            // сразу с реальным размером (CSS: body > canvas на весь экран)
            attach_canvas_to_dom(&window);
            // W5 (ввод): фокус канваса сразу после вставки в DOM. winit
            // фокусирует canvas при create_window (with_active), но канвас
            // тогда ЕЩЁ НЕ В DOM — focus() на оторванном элементе
            // бессмыслен; keydown-листенеры winit висят на канвасе, без
            // фокуса клавиатура мертва до первого клика. focus_window()
            // — тот же canvas.focus() (winit WindowExtWebSys-внутренность)
            // + FocusEvent → Focused(true) → has_focus.
            window.focus_window();
            let deliver = slot.clone();
            let wake = window.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // FR-091 v2: capture-режим (согласие analytics) идёт в GL без
                // WebGPU-ретраев — адаптер разрешается за миллисекунды, и
                // Renderer::new успевает прочитать window.inner_size() ДО
                // первого layout'а канваса (ResizeObserver winit'а ещё не
                // сработал): 0×0 → surface клампится в 1×1 (предупреждение
                // «физический размер canvas превышает…»), корректный размер
                // возвращал бы только Resized — а он обрабатывается лишь при
                // живом рендерере (handler.rs), т.е. лечится гонкой порядка
                // событий, не гарантией. Дожидаемся двух анимационных кадров
                // ДО старта Renderer::new — layout канваса гарантированно
                // случился (rAF №1 выполняется до layout своего кадра,
                // ResizeObserver срабатывает в layout кадра №1; rAF №2 — уже
                // после него). ~32 мс к буту — незаметно; Resized приходит
                // позже и резайзит в тот же размер (идемпотентно).
                wait_canvas_layout().await;
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

/// FR-091 v2 (web): дождаться layout-кадра канваса — см. комментарий в
/// `launch`. Ошибки не блокируют бут: нет window — сразу вернуться;
/// rAF не запланировался (фоновая вкладка: колбэки не выполняются до
/// видимости) — промис резолвится сразу, Renderer::new читает текущий
/// размер, surface вылечится событием Resized (прежнее поведение).
#[cfg(target_arch = "wasm32")]
async fn wait_canvas_layout() {
    let Some(win) = web_sys::window() else {
        return;
    };
    for _ in 0..2 {
        let win = win.clone();
        let mut executor = move |resolve: js_sys::Function, _reject: js_sys::Function| {
            if win.request_animation_frame(&resolve).is_err() {
                // планирование сорвалось — не висим вечным промисом
                let _ = resolve.call0(&wasm_bindgen::JsValue::NULL);
            }
        };
        let promise = js_sys::Promise::new(&mut executor);
        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
    }
}

/// FR-091 v2 (web): согласие на аналитику → запись сеанса активна.
/// Источник правды — JSON `canvasdesk.consent` в localStorage (тот же,
/// что у JS-модуля телеметрии в index.html: `readConsent`). Семантика
/// зеркалит index.html: нет ни согласий, ни `canvasdesk.config` —
/// свежий визит, пикер ещё не показан, SDK не грузится → запись
/// невозможна → WebGPU остаётся. Запись без поля/битая — дефолт true
/// (как предвыбранные чекбоксы пикера и «старые конфиги считаются
/// согласившимися», FR-089).
#[cfg(target_arch = "wasm32")]
fn analytics_recording_active() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        return false;
    };
    let consent_raw = storage.get_item("canvasdesk.consent").ok().flatten();
    let config_raw = storage.get_item("canvasdesk.config").ok().flatten();
    if consent_raw.is_none() && config_raw.is_none() {
        return false;
    }
    consent_raw
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v.get("analytics").and_then(serde_json::Value::as_bool))
        .unwrap_or(true)
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
