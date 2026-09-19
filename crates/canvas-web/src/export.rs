//! M8/W6 (wasm-port §4.2 п. 4): экспорт активного канваса — download-blob
//! (для OPFS-канвасов, где «настоящий» файл недоступен пользователю).
//!
//! Экспортируется ПОСЛЕДНЯЯ СОХРАНЁННАЯ версия: чтение идёт из хранилища
//! (OPFS-файл или дисковый хэндл), а не из живой сцены (доступ к модели
//! после построения App был бы вторым каналом состояния — сознательно не
//! заводим; отставание ≤ автосейв-debounce 2 с, SPEC §9).

use wasm_bindgen::prelude::JsValue;
use wasm_bindgen::JsCast;

/// «Экспорт .canvas» — кнопка DOM-панели. Источник: активный канвас
/// (диск → хэндл с granted-разрешением; OPFS → файл). Ничего активного —
/// честный info-лог.
pub(crate) async fn export_active() {
    let Some((name, kind)) = crate::web_state::active_name().map(|name| {
        let kind = crate::web_state::active_kind();
        (name, kind)
    }) else {
        tracing::info!(target: "canvas_web", "экспорт: активного канваса нет");
        return;
    };
    let text = match kind {
        Some(crate::web_state::ActiveKind::Disk) => {
            let Some(handle) = crate::web_state::disk_handle() else {
                tracing::warn!(target: "canvas_web", file = %name, "экспорт: хэндл диска утрачен");
                return;
            };
            match crate::fs_access::read_disk_text_for_export(&handle).await {
                Ok(text) => text,
                Err(err) => {
                    tracing::warn!(target: "canvas_web", file = %name, error = ?err, "экспорт: чтение диска не удалось");
                    return;
                }
            }
        }
        _ => {
            // OPFS (или неизвестный тип — OPFS безопаснее): файл origin'а
            let Ok(root) = crate::opfs::opfs_root().await else {
                tracing::warn!(target: "canvas_web", "экспорт: OPFS недоступен");
                return;
            };
            match crate::opfs::read_opfs_text(&root, &name).await {
                Ok(Some(text)) => text,
                Ok(None) => {
                    tracing::warn!(target: "canvas_web", file = %name, "экспорт: файла нет в OPFS");
                    return;
                }
                Err(err) => {
                    tracing::warn!(target: "canvas_web", file = %name, error = ?err, "экспорт: чтение OPFS не удалось");
                    return;
                }
            }
        }
    };
    match download_blob(&name, &text) {
        Ok(()) => {
            tracing::info!(target: "canvas_web", file = %name, bytes = text.len(), "экспорт: download-blob отдан браузеру")
        }
        Err(err) => {
            tracing::error!(target: "canvas_web", file = %name, error = ?err, "экспорт не удался")
        }
    }
}

/// Скачивание: Blob → objectURL → клик по временному `<a download>`.
fn download_blob(name: &str, text: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("нет window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("нет document"))?;
    let parts = js_sys::Array::from_iter([JsValue::from_str(text)]);
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type("application/json");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &bag)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let anchor: web_sys::HtmlAnchorElement = document
        .create_element("a")?
        .dyn_into()
        .map_err(|err| JsValue::from(format!("a: {err:?}")))?;
    anchor.set_href(&url);
    anchor.set_download(name);
    anchor.click();
    web_sys::Url::revoke_object_url(&url)?;
    anchor.remove();
    Ok(())
}
