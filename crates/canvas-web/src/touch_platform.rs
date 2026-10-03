//! FR-096/097 (мобильный web): платформенные будилки тач-слоя.
//!
//! - Будилка long-press: палец без движения событий касания не рождает,
//!   поэтому порог [`canvas_core::touch::LONG_PRESS_MS`] доставляет
//!   платформа. canvas-app инкрементирует
//!   [`canvas_core::web_bridge::TOUCH_PRESS_GEN`] на каждом `Action::Press`
//!   машины жеста; здесь setInterval замечает смену поколения и ставит
//!   одноразовый setTimeout → `AppEvent::LongPressPoll` — решение
//!   принимает машина ([`TouchGesture::poll_long_press`]): палец поднят /
//!   ушёл за slop / второй палец / уже выдано — тишина. Лишние тики
//!   бесплатны (poll идемпотентен после выдачи), таймеры не отменяются.
//! - Coarse-указатель: `matchMedia("(pointer: coarse)")` →
//!   [`canvas_core::web_bridge::POINTER_COARSE`]; потребители — hit-тесты
//!   canvas-app (FR-097, тач-цели ≥ 44 лог. px). Листенер `change` —
//!   режим ввода может смениться (отстёгнулась мышь планшета).
//!
//! Прецедент таймеров — `widgets_web::web::install_tick` (setInterval,
//! `closure.forget()` — singleton на время жизни страницы).

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;

use canvas_app::app::AppEvent;
use canvas_core::web_bridge;

/// Период опроса поколения нажатия, мс. Компромисс: латентность старта
/// будилки (≤ период) против частоты пробуждения вкладки. 50 мс —
/// незаметно на фоне порога long-press (550 мс).
const POLL_INTERVAL_MS: i32 = 50;

/// Запас над порогом long-press, мс: будилка срабатывает СТРОГО после
/// порога (детект поколения опаздывает до POLL_INTERVAL_MS; машина
/// сравнивает монотонное время сама, ранний тик был бы отброшен —
/// `due()` требует `>=`).
const WAKE_SLACK_MS: i32 = 25;

/// Установить платформенные тач-будилки (один раз при спавне приложения;
/// замыкания живут до выгрузки страницы — `forget`, как у тика виджетов).
pub(crate) fn install(
    window: &web_sys::Window,
    proxy: &winit::event_loop::EventLoopProxy<AppEvent>,
) {
    install_long_press_wake(window, proxy);
    install_coarse_pointer(window);
}

/// FR-096: setInterval(POLL_INTERVAL_MS) следит за поколением нажатий;
/// смена → setTimeout(LONG_PRESS_MS + WAKE_SLACK_MS) → LongPressPoll.
fn install_long_press_wake(
    window: &web_sys::Window,
    proxy: &winit::event_loop::EventLoopProxy<AppEvent>,
) {
    let proxy = proxy.clone();
    let mut last_gen = web_bridge::touch_press_gen();
    let poll: Closure<dyn FnMut()> = Closure::new(move || {
        let gen = web_bridge::touch_press_gen();
        if gen == last_gen {
            return;
        }
        last_gen = gen;
        // Новое нажатие: одноразовый таймер порога. Каждое нажатие —
        // свой замыкание (утечка ограничена: одно на нажатие, живёт
        // до выгрузки страницы — тот же контракт, что у install_tick).
        let proxy = proxy.clone();
        let wake: Closure<dyn FnMut()> = Closure::new(move || {
            let _ = proxy.send_event(AppEvent::LongPressPoll);
        });
        let callback = wake.as_ref().unchecked_ref::<js_sys::Function>();
        let _ = window_set_timeout(callback);
        wake.forget(); // таймер живёт до выстрела/выгрузки — замыкание не дропаем
    });
    let callback = poll.as_ref().unchecked_ref::<js_sys::Function>();
    if window
        .set_interval_with_callback_and_timeout_and_arguments(
            callback,
            POLL_INTERVAL_MS,
            &js_sys::Array::new(),
        )
        .is_err()
    {
        tracing::warn!(target: "canvas_web", "будилка long-press не установлена — long-press срабатывает только по дрожанию пальца (Move-путь)");
        return; // closure дропнется — деградация, не поломка
    }
    poll.forget(); // singleton на время жизни страницы
}

/// FR-097: matchMedia("(pointer: coarse)") → POINTER_COARSE (+ change).
fn install_coarse_pointer(window: &web_sys::Window) {
    let Some(media) = window.match_media("(pointer: coarse)").ok().flatten() else {
        // Нет matchMedia/QueryList — считаем указатель точным (десктоп).
        web_bridge::set_pointer_coarse(false);
        return;
    };
    web_bridge::set_pointer_coarse(media.matches());
    let Ok(media) = media.dyn_into::<web_sys::MediaQueryList>() else {
        return;
    };
    let on_change: Closure<dyn FnMut(web_sys::Event)> = Closure::new(|_event: web_sys::Event| {
        let Some(window) = web_sys::window() else {
            return;
        };
        if let Ok(Some(media)) = window.match_media("(pointer: coarse)") {
            web_bridge::set_pointer_coarse(media.matches());
        }
    });
    if media
        .add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())
        .is_err()
    {
        tracing::warn!(target: "canvas_web", "change-листенер pointer:coarse не установлен — флаг зафиксирован на старте");
    }
    std::mem::forget(on_change); // живёт до выгрузки страницы
}

/// Одноразовый таймер web-sys 0.3 (пара к set_interval_… widgets_web).
fn window_set_timeout(callback: &js_sys::Function) -> Result<i32, wasm_bindgen::JsValue> {
    let window = web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("no window"))?;
    window.set_timeout_with_callback_and_timeout_and_arguments(
        callback,
        wake_delay_ms(),
        &js_sys::Array::new(),
    )
}

/// Задержка будилки, мс: порог машины + запас.
fn wake_delay_ms() -> i32 {
    i32::try_from(canvas_core::touch::LONG_PRESS_MS).unwrap_or(i32::MAX - 1) + WAKE_SLACK_MS
}
