//! M8/W6 (wasm-port §4.2 п. 4) + M8/W10 (§4.3): приём файлов drag-ом на
//! окно. winit web `DroppedFile` не даёт (план §7) — DOM-листенеры на
//! window: `dragover` глушит (разрешаем дроп), `drop` разбирает
//! `dataTransfer.files`.
//!
//! `.canvas`-файл → «Импортировать копию»: текст читается через
//! `File.text()`, копия пишется в OPFS (санитизация имени из
//! [`crate::url_params::sanitize_canvas_name`]), сцена открывается
//! (`AppEvent::OpenScene`) с OPFS-хранилищем — автосейв продолжает жить
//! для копии (первый `.canvas` выигрывает).
//!
//! Прочие файлы (W10) → файл-ноды с превью: байты пишутся в OPFS
//! подкаталог `files/` (санитизация [`crate::url_params::sanitize_file_name`],
//! коллизии — кандидаты `-N`), затем в event loop уходит тот же
//! `DragEvent::Drop` с `DragData::Paths` (нейтральный протокол T9 —
//! dragdrop.rs прямо предписывает canvas-web производить его события) —
//! план вставки/ноды/поиск делаем общим кодом canvas-app; превью закажет
//! `order_thumbnails`, декод — `WebImageThumbnailProvider` (web_thumbs).
//!
//! Осознанная деградация (план §4.2): превью дропа («призраки» T9) на web
//! нет — содержимое файлов доступно браузеру только в момент drop
//! (`getAsFileSystemHandle` тоже лишь на drop), Enter-фаза данных не даёт.

const MAX_DROP_FILE_BYTES: u64 = 64 * 1024 * 1024; // паритет лимита пакета виджетов

use std::path::PathBuf;

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

/// Разбор drop: первый `.canvas` — импорт; прочие — файл-ноды с превью
/// (W10): OPFS `files/` + DragEvent::Drop с готовыми путями.
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
    let client_pt = drop_client_pt(&event);
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
    }
    // W10: не-канвасные файлы — в OPFS files/ и в DragData::Paths
    let mut paths: Vec<PathBuf> = Vec::new();
    for index in 0..count {
        let Some(file) = files.item(index) else {
            continue;
        };
        let name = file.name();
        if name.to_ascii_lowercase().ends_with(".canvas") {
            continue; // уже обработан выше
        }
        if file.size() as u64 > MAX_DROP_FILE_BYTES {
            tracing::warn!(target: "canvas_web", file = %name, size = file.size(),
                "файл больше лимита 64 МБ — пропущен");
            continue;
        }
        match accept_file_to_opfs(&file, &name).await {
            Ok(Some(path)) => paths.push(path.into()),
            Ok(None) => {}
            Err(err) => tracing::error!(
                target: "canvas_web",
                file = %name,
                error = ?err,
                "запись брошенного файла в OPFS не удалась"
            ),
        }
    }
    if paths.is_empty() {
        return;
    }
    tracing::info!(target: "canvas_web", files = paths.len(), "файлы приняты в OPFS (file-ноды с превью)");
    let _ = proxy.send_event(AppEvent::Drag(canvas_core::dragdrop::DragEvent::Drop {
        data: canvas_core::dragdrop::DragData::Paths(paths),
        client_pt,
    }));
}

/// Позиция drop в ФИЗИЧЕСКИХ пикселях клиентской области (контракт
/// DragEvent T9: shell даёт ScreenToClient; DOM clientX/Y — CSS-пиксели
/// от окна, canvas занимает всю страницу, dpr = scale_factor).
fn drop_client_pt(event: &web_sys::DragEvent) -> (f32, f32) {
    let x = event.client_x() as f32;
    let y = event.client_y() as f32;
    let dpr = web_sys::window()
        .map(|w| w.device_pixel_ratio())
        .unwrap_or(1.0);
    (x * dpr as f32, y * dpr as f32)
}

/// Сохранить файл в OPFS `files/` (санитизация + свободное имя) и
/// вернуть путь для `DragData::Paths` (абсолютный OPFS-путь `/files/<имя>`
/// — `resolve_node_path` такие возвращает как есть, без current_dir,
/// который на wasm паникует). `Ok(None)` — отклонено (имя/нет OPFS),
/// решение залогировано.
async fn accept_file_to_opfs(
    file: &web_sys::File,
    raw_name: &str,
) -> Result<Option<String>, wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    let Some(name) = crate::url_params::sanitize_file_name(raw_name) else {
        tracing::warn!(target: "canvas_web", file = %raw_name, "имя файла небезопасно — приём отклонён");
        return Ok(None);
    };
    let root = crate::opfs::opfs_root().await?;
    // Подкаталог files/ — создать при необходимости
    let options = web_sys::FileSystemGetDirectoryOptions::new();
    options.set_create(true);
    let files_dir: web_sys::FileSystemDirectoryHandle = wasm_bindgen_futures::JsFuture::from(
        root.get_directory_handle_with_options("files", &options),
    )
    .await?
    .dyn_into()
    .map_err(|err| wasm_bindgen::JsValue::from(format!("files/ не каталог: {err:?}")))?;
    // Свободное имя: probe get_file_handle — NotFoundError = свободно
    let candidates = crate::url_params::file_name_candidates(&name, 512);
    let mut chosen = None;
    for candidate in &candidates {
        match wasm_bindgen_futures::JsFuture::from(files_dir.get_file_handle(candidate)).await {
            Ok(_) => continue, // занято
            Err(err) => {
                let not_found = err
                    .dyn_ref::<js_sys::Error>()
                    .and_then(|e| e.name().as_string())
                    .is_some_and(|n| n == "NotFoundError");
                if not_found {
                    chosen = Some(candidate.clone());
                    break;
                }
                return Err(err); // прочая ошибка OPFS — наверх
            }
        }
    }
    let Some(name) = chosen else {
        tracing::warn!(target: "canvas_web", file = %raw_name, "не удалось подобрать свободное имя — файл пропущен");
        return Ok(None);
    };
    let create = web_sys::FileSystemGetFileOptions::new();
    create.set_create(true);
    let handle: web_sys::FileSystemFileHandle = wasm_bindgen_futures::JsFuture::from(
        files_dir.get_file_handle_with_options(&name, &create),
    )
    .await?
    .dyn_into()
    .map_err(|err| wasm_bindgen::JsValue::from(format!("хэндл не файл: {err:?}")))?;
    let writable: web_sys::FileSystemWritableFileStream =
        wasm_bindgen_futures::JsFuture::from(handle.create_writable())
            .await?
            .into();
    // Байты: file — Blob (IDL-иерархия), write(blob) пишет целиком
    let blob: &web_sys::Blob = file.unchecked_ref::<web_sys::Blob>();
    wasm_bindgen_futures::JsFuture::from(writable.write_with_blob(blob)?).await?;
    let stream = writable.unchecked_ref::<web_sys::WritableStream>();
    wasm_bindgen_futures::JsFuture::from(stream.close()).await?;
    tracing::info!(target: "canvas_web", file = %name, "файл сохранён в OPFS (превью — WebImageThumbs)");
    Ok(Some(format!("/files/{name}")))
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
