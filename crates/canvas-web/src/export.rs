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
    match download_blob(&name, &text, "application/json") {
        Ok(()) => {
            // FR-090: продуктовое событие экспорта .canvas (мост
            // canvasdesk:track; ключ/согласия — на стороне JS, FR-089)
            let bytes = text.len().to_string();
            canvas_app::app::telemetry::track("export_canvas", &[("bytes", bytes.as_str())]);
            tracing::info!(target: "canvas_web", file = %name, bytes = text.len(), "экспорт: download-blob отдан браузеру")
        }
        Err(err) => {
            tracing::error!(target: "canvas_web", file = %name, error = ?err, "экспорт не удался")
        }
    }
}

/// FR-076: «Экспорт HTML» — кнопка DOM-панели. Самодостаточный офлайн-
/// артефакт защиты (GAP-01): SVG-снимок + значения + what-if таблица.
/// Источник — та же ПОСЛЕДНЯЯ СОХРАНЁННАЯ версия (архитектурное решение
/// `export_active`: второго канала к живой сцене сознательно нет);
/// пересчёт и сценарии считаются чистыми функциями ядра здесь же —
/// детерминизм канваса ⇒ те же значения, что увидит пользователь после
/// автосейва (движок побитово воспроизводим, инвариант 2 FR-050).
pub(crate) async fn export_html_active() {
    let Some(name) = crate::web_state::active_name() else {
        tracing::info!(target: "canvas_web", "экспорт HTML: активного канваса нет");
        return;
    };
    let kind = crate::web_state::active_kind();
    let text = match kind {
        Some(crate::web_state::ActiveKind::Disk) => {
            let Some(handle) = crate::web_state::disk_handle() else {
                tracing::warn!(target: "canvas_web", file = %name, "экспорт HTML: хэндл диска утрачен");
                return;
            };
            match crate::fs_access::read_disk_text_for_export(&handle).await {
                Ok(text) => text,
                Err(err) => {
                    tracing::warn!(target: "canvas_web", file = %name, error = ?err, "экспорт HTML: чтение диска не удалось");
                    return;
                }
            }
        }
        _ => {
            let Ok(root) = crate::opfs::opfs_root().await else {
                tracing::warn!(target: "canvas_web", "экспорт HTML: OPFS недоступен");
                return;
            };
            match crate::opfs::read_opfs_text(&root, &name).await {
                Ok(Some(text)) => text,
                Ok(None) => {
                    tracing::warn!(target: "canvas_web", file = %name, "экспорт HTML: файла нет в OPFS");
                    return;
                }
                Err(err) => {
                    tracing::warn!(target: "canvas_web", file = %name, error = ?err, "экспорт HTML: чтение OPFS не удалось");
                    return;
                }
            }
        }
    };
    // Сборка артефакта — чистые функции ядра (canvas-core: без GPU/ОС)
    let Ok(canvas) = text.parse::<canvas_core::Canvas>() else {
        tracing::warn!(target: "canvas_web", file = %name, "экспорт HTML: канвас не парсится");
        return;
    };
    let Ok(base) = canvas_core::flow::propagate_with_lines(&canvas, &Default::default()) else {
        tracing::warn!(target: "canvas_web", file = %name, "экспорт HTML: цикл потока — артефакт без значений не собираем");
        return;
    };
    let comparison = canvas_core::export_html::scenario_comparison_for_export(&canvas, &base);
    let title = name.strip_suffix(".canvas").unwrap_or(&name).to_owned();
    let options = canvas_core::export_html::ExportHtmlOptions { title, dark: true };
    let html = canvas_core::export_html::export_html(&canvas, &base, comparison.as_ref(), &options);
    let file_name = format!("{}.html", name.strip_suffix(".canvas").unwrap_or(&name));
    match download_blob(&file_name, &html, "text/html") {
        Ok(()) => {
            // FR-090: продуктовое событие экспорта HTML-артефакта (GAP-01)
            let bytes = html.len().to_string();
            canvas_app::app::telemetry::track("export_html", &[("bytes", bytes.as_str())]);
            tracing::info!(
                target: "canvas_web",
                file = %file_name,
                bytes = html.len(),
                "экспорт HTML: артефакт отдан браузеру"
            )
        }
        Err(err) => {
            tracing::error!(target: "canvas_web", file = %file_name, error = ?err, "экспорт HTML не удался")
        }
    }
}

/// Скачивание: Blob → objectURL → клик по временному `<a download>`.
/// MIME — параметр (`.canvas` — JSON, FR-076 артефакт — text/html).
fn download_blob(name: &str, text: &str, mime: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("нет window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("нет document"))?;
    let parts = js_sys::Array::from_iter([JsValue::from_str(text)]);
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type(mime);
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
