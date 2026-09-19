//! M8/W6 (wasm-port §4.2 п. 4): приём файлов drag-ом на окно. winit web
//! `DroppedFile` не даёт (план §7) — DOM-листенеры на window: `dragover`
//! глушит (разрешаем дроп), `drop` разбирает `dataTransfer.files`.
//!
//! `.canvas`-файл → «Импортировать копию»: текст читается через
//! `File.text()`, копия пишется в OPFS (санитизация имени из
//! [`crate::url_params::sanitize_canvas_name`]), сцена открывается
//! (`AppEvent::OpenScene`) с OPFS-хранилищем — автосейв продолжает жить
//! для копии. Прочие типы (картинки и т.п.) — info-лог: превью — W10
//! (план §4.3).
//!
//! Осознанная деградация (план §4.2): превью дропа («призраки» T9) на web
//! нет — содержимое файлов доступно браузеру только в момент drop
//! (`getAsFileSystemHandle` тоже лишь на drop), Enter-фаза данных не даёт.

use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use winit::event_loop::EventLoopProxy;

use canvas_app::app::AppEvent;

/// Установить DOM-листенеры drag/drop на window (вызывается после
/// построения event loop — события шлются в proxy).
pub(crate) fn install(proxy: EventLoopProxy<AppEvent>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let target = window.unchecked_ref::<web_sys::EventTarget>();
    // dragover: prevent_default обязателен, иначе браузер уводит курсор
    // «отдать файл ОС» и drop на страницу не приходит
    attach(
        target,
        "dragover",
        Closure::wrap(Box::new(move |event: web_sys::DragEvent| {
            event.prevent_default();
        }) as Box<dyn FnMut(web_sys::DragEvent)>),
    );
    attach(
        target,
        "drop",
        Closure::wrap(Box::new(move |event: web_sys::DragEvent| {
            event.prevent_default();
            let proxy = proxy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                on_drop(event, proxy).await;
            });
        }) as Box<dyn FnMut(web_sys::DragEvent)>),
    );
    tracing::info!(target: "canvas_web", "DOM-drop приём файлов установлен");
}

fn attach(
    target: &web_sys::EventTarget,
    kind: &str,
    closure: Closure<dyn FnMut(web_sys::DragEvent)>,
) {
    if let Err(err) = target.add_event_listener_with_callback(
        kind,
        closure.as_ref().unchecked_ref::<js_sys::Function>(),
    ) {
        tracing::warn!(target: "canvas_web", kind, error = ?err, "листенер не установлен");
    }
    closure.forget(); // живёт до выгрузки страницы (singleton-панель)
}

/// Разбор drop: первый `.canvas` — импорт; прочие — лог (W10).
async fn on_drop(event: web_sys::DragEvent, proxy: EventLoopProxy<AppEvent>) {
    let Some(transfer) = event.data_transfer() else {
        return;
    };
    let Some(files) = transfer.files() else {
        return;
    };
    let count = files.length();
    if count == 0 {
        tracing::debug!(target: "canvas_web", "drop без файлов");
        return;
    }
    for index in 0..count {
        let Some(file) = files.item(index) else {
            continue;
        };
        let name = file.name();
        if name.to_ascii_lowercase().ends_with(".canvas") {
            match wasm_bindgen_futures::JsFuture::from(file.text()).await {
                Ok(text) => {
                    let json = text.as_string().unwrap_or_default();
                    import_to_opfs(&proxy, &name, json).await;
                }
                Err(err) => tracing::error!(
                    target: "canvas_web",
                    file = %name,
                    error = ?err,
                    "чтение брошенного файла не удалось"
                ),
            }
            return; // первый .canvas выигрывает
        }
        tracing::info!(
            target: "canvas_web",
            file = %name,
            "файл не .canvas — превью картинок придёт в W10 (план §4.3)"
        );
    }
}

/// Импортировать канвас как копию: безопасное имя, OPFS, недавние,
/// открытие сцены с OPFS-хранилищем. Общий путь для DOM-drop и «Открыть
/// копию» (fallback «Открыть с диска» без readwrite-разрешения).
pub(crate) async fn import_to_opfs(proxy: &EventLoopProxy<AppEvent>, raw_name: &str, json: String) {
    let Some(name) = crate::url_params::sanitize_canvas_name(raw_name) else {
        tracing::warn!(target: "canvas_web", file = %raw_name, "имя файла небезопасно — импорт отклонён");
        return;
    };
    let Ok(root) = crate::opfs::opfs_root().await else {
        tracing::error!(target: "canvas_web", "OPFS недоступен — импорт невозможен");
        return;
    };
    if let Err(err) = crate::opfs::write_opfs_text(&root, &name, &json).await {
        tracing::error!(target: "canvas_web", file = %name, error = ?err, "запись копии в OPFS не удалась");
        return;
    }
    let Some(storage) = crate::web_state::opfs_storage() else {
        tracing::error!(target: "canvas_web", "OPFS-хранилище не инициализировано");
        return;
    };
    storage.seed_mirror(std::path::Path::new(&name), &json);
    crate::web_state::set_active(name.clone(), crate::web_state::ActiveKind::Opfs);
    crate::recent::record_recent(&name).await;
    crate::toolbar::set_recent_label(&name);
    tracing::info!(target: "canvas_web", file = %name, "канвас импортирован копией в OPFS");
    let _ = proxy.send_event(AppEvent::OpenScene {
        path: std::path::PathBuf::from(name),
        json,
        storage: Some(storage),
    });
}
