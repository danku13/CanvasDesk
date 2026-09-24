//! Мост текстового ввода DOM → App (wasm-аудит 2026-09-25).
//!
//! Проблема: winit-web слушает ТОЛЬКО `keydown`/`keyup`
//! (`web_sys::canvas::on_keyboard_press`). Chromium ввод кириллицы
//! (Playwright `keyboard.type()` и реальная IME/раскладка без латинского
//! key-маппинга) доставляет через `insertText` БЕЗ keydown — winit-событий
//! не возникает вовсе, и кириллица не вводилась ни в поиск, ни в редактор
//! заметки (латиница шла keydown'ом и работала — отсюда «половинчатость»
//! бага на скриншотах 12b/34).
//!
//! Решение: слушать `beforeinput` на document; `inputType == "insertText"`
//! с непустым `data` → `AppEvent::ImeCommit(data)` → общий маршрут
//! `App::insert_committed_text` (редактор → поиск → explain).
//!
//! Дедупликация: обычное нажатие латинской клавиши порождает И keydown
//! (ввод уже сделал App), И `beforeinput`-insertText — повторный ввод
//! задвоил бы символ. Перехватчик keydown (capture, до winit) запоминает
//! момент последнего одиночного символа; `beforeinput` в пределах
//! [`DEDUP_WINDOW_MS`] после него глотается. IME-композиция и CDP
//! insertText keydown'а не имеют — проходят.

use std::cell::RefCell;
use std::rc::Rc;
// std::time::Instant на wasm32-unknown-unknown паникует — проектный alias
// (canvas_core::time::Instant = web_time::Instant, performance.now()) —
// урок первого прогона моста: паника «time not implemented» валила rAF.
use canvas_core::time::Instant;

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use winit::event_loop::EventLoopProxy;

use canvas_app::app::AppEvent;

/// Окно дедупликации «keydown уже доставил этот символ», мс.
const DEDUP_WINDOW_MS: u128 = 120;

/// Установить DOM-листенеры `beforeinput`/`keydown` на document (вызывается
/// после построения event loop — текст шлётся в proxy).
pub(crate) fn install(proxy: EventLoopProxy<AppEvent>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let document = window.document();
    let target = window.unchecked_ref::<web_sys::EventTarget>();

    // Момент последнего одиночного символьного keydown (для дедупликации).
    let last_symbol_keydown: Rc<RefCell<Option<Instant>>> = Rc::new(RefCell::new(None));
    {
        let last = Rc::clone(&last_symbol_keydown);
        let keydown = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |event: web_sys::KeyboardEvent| {
                // Одиночный текстовый символ без композиции — его уже обработает
                // winit (и приложение) как Key::Character.
                let is_symbol = event.key().chars().count() == 1 && !event.is_composing();
                if is_symbol {
                    *last.borrow_mut() = Some(Instant::now());
                }
            },
        );
        let _ =
            target.add_event_listener_with_callback("keydown", keydown.as_ref().unchecked_ref());
        // Слушатель живёт весь процесс страницы — утечка осознана (как у
        // остальных install* canvas-web).
        std::mem::forget(keydown);
    }

    let beforeinput = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
        // web-sys этой версии не экспортирует InputEvent — читаем
        // `inputType`/`data` отражением (спека Input Event Level 2).
        let get = |key: &str| js_sys::Reflect::get(event.as_ref(), &key.into()).ok();
        let input_type = get("inputType")
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        // Целевые пути: обычный посимвольный ввод и финал IME-композиции.
        if input_type != "insertText" && input_type != "insertCompositionText" {
            return;
        }
        let data = get("data").and_then(|v| v.as_string()).unwrap_or_default();
        if data.is_empty() {
            return;
        }
        // Дедупликация: keydown этого символа уже ушёл в winit/App.
        if let Some(t) = *last_symbol_keydown.borrow() {
            if t.elapsed().as_millis() <= DEDUP_WINDOW_MS {
                return;
            }
        }
        let _ = proxy.send_event(AppEvent::ImeCommit(data));
    });
    if let Some(document) = document {
        let _ = document
            .add_event_listener_with_callback("beforeinput", beforeinput.as_ref().unchecked_ref());
        std::mem::forget(beforeinput);
    }
}
