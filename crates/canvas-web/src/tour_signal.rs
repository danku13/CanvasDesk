//! FR-028 v2 (inline onboarding, `sdk/web-onboarding/`): Rust-side мост
//! к signal-bus движка тура. Модуль — pure addition: не модифицирует
//! существующие файлы canvas-web; подключается через `lib.rs` только на
//! wasm32 (нативная компиляция rlib-тестов остаётся зелёной).
//!
//! Контракт: WASM-bridge (canvas-app через `AppEvent`) шлёт события
//! жизни канваса (создание заметки / открытие палитры / start undo…)
//! сюда; модуль прокидывает их в JS-side `window.__canvasdeskTour.signal(name)`,
//! что продвигает `waitFor(signal)`-гейты в сценариях (см.
//! `sdk/web-onboarding/src/scenarios/first-run-inline.ts`).
//!
//! НЕ ВТОРГАЕТСЯ в `canvas-app`: весь вызов — через `web_sys::window()`
//! + `js_sys::Reflect::get`, как и `js_glue::call`. App остаётся
//! platform-нейтральным; вызов `emit(...)` идёт из `canvas-web` слоёв
//! (toolbar / app_spawn / ime / fs_access…), не из App.
//!
//! Сигналы (контракт): см. `docs/interface-objects/onboarding-v2-inline.md` §5.
//!
//! Точки эмиссии (TODO для будущих коммитов, скоупа v2 follow-up):
//! - `canvas-app/src/app.rs::create_note_at` → `canvas:note-created`
//! - palette-open hotkey handler → `canvas:palette-opened`
//! - search-open → `canvas:search-opened`
//! - docs-viewer-open → `canvas:docs-opened`
//!
//! Прокидка в эти точки — следующий PR (требует тестирования в
//! `trunk serve`); здесь только инфраструктура моста.

/// Эмитить сигнал в tour-bus. Без payload (большинство сигналов —
/// просто факт события). No-op вне wasm32 (нативный rlib — заглушка).
#[cfg(target_arch = "wasm32")]
pub fn emit(name: &str) {
    emit_with_payload(name, &None);
}

/// Эмитить сигнал с payload-строкой (для тегированных событий:
/// `canvas:note-activated` с id заметки). No-op вне wasm32.
#[cfg(target_arch = "wasm32")]
pub fn emit_with_payload(name: &str, payload: &Option<String>) {
    use wasm_bindgen::JsCast;
    let Some(window) = web_sys::window() else {
        // Не JS-рунтайм (нативный rlib-тест) — молча.
        return;
    };
    // Движок тура может быть ещё не инициализирован (index.html
    // подключает bundle в конце body — теоретически возможен вызов
    // до DOMContentLoaded). Reflect::get возвращает undefined —
    // gracefully выходим.
    let tour = js_sys::Reflect::get(&window, &"__canvasdeskTour".into())
        .ok();
    let Some(tour) = tour else { return };
    if tour.is_undefined() || tour.is_null() {
        return;
    }
    let signal_fn = js_sys::Reflect::get(&tour, &"signal".into()).ok();
    let Some(signal_fn) = signal_fn else { return };
    let Ok(signal_fn) = signal_fn.dyn_into::<js_sys::Function>() else {
        return;
    };
    let mut args = Vec::<wasm_bindgen::JsValue>::with_capacity(2);
    args.push(name.into());
    if let Some(p) = payload {
        args.push(p.into());
    }
    let arguments = js_sys::Array::from_iter(args.iter().cloned());
    // Контекст вызова: сам объект tour (signal — метод, expects `this`).
    let _ = signal_fn.apply(tour.unchecked_ref::<js_sys::Object>(), &arguments);
}

// ── Нативные заглушки (rlib-тесты на non-wasm — не падают) ──────────
#[cfg(not(target_arch = "wasm32"))]
pub fn emit(_name: &str) {}
#[cfg(not(target_arch = "wasm32"))]
pub fn emit_with_payload(_name: &str, _payload: &Option<String>) {}

#[cfg(test)]
mod tests {
    use super::*;

    /// Нативная заглушка не падает (контракт: pure addition, не
    /// инфицировать rlib-тесты canvas-web).
    #[test]
    fn emit_native_noop() {
        emit("canvas:test");
        emit_with_payload("canvas:test", &Some("payload".to_string()));
        emit_with_payload("canvas:test", &None);
    }
}
