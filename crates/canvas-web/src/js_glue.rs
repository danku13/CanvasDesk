//! M8/W6: мост к JS-глю `window.__canvasdesk` (index.html). Через него
//! идут компактные JS-механики, для которых биндинги/бойлерплейт в Rust
//! непропорционально тяжелы: IndexedDB (недавние + хэндлы FS Access).
//! Вызов — `Promise` → [`wasm_bindgen_futures::JsFuture`]; отсутствие глю
//! (старый кэш страницы, натив-заглушка) — `None` (все потребители
//! деградируют честно).

/// Вызвать `window.__canvasdesk[name](...args)`; результат — await промиса.
/// `None`: нет window/глю/функции (не JS-рунтайм или битая страница).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn call(
    name: &str,
    args: &[wasm_bindgen::JsValue],
) -> Option<Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>> {
    use wasm_bindgen::JsCast;
    let window = web_sys::window()?;
    let glue = js_sys::Reflect::get(&window, &"__canvasdesk".into()).ok()?;
    if glue.is_undefined() || glue.is_null() {
        return None;
    }
    let function = js_sys::Reflect::get(&glue, &name.into())
        .ok()?
        .dyn_into::<js_sys::Function>()
        .ok()?;
    let arguments = js_sys::Array::from_iter(args.iter().cloned());
    let promise = function
        .apply(glue.unchecked_ref::<js_sys::Object>(), &arguments)
        .ok()?
        .dyn_into::<js_sys::Promise>()
        .ok()?;
    Some(wasm_bindgen_futures::JsFuture::from(promise).await)
}
