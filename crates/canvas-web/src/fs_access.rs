//! M8/W6 (wasm-port §4.1/§4.2): FS Access API — основной путь для файлов
//! на настоящем диске (Chromium 86+). Пикер — только по жесту пользователя
//! (кнопка «Открыть с диска…» DOM-панели); хэндл персистентен в IndexedDB
//! (`handles`), reopen после перезагрузки — requestPermission одним
//! промисом внутри жеста кнопки «Недавние», без пикера.
//!
//! Отклонение от плана §4.2 п. 3 (документировано): браузер не отдаёт
//! родительский каталог файла-хэндла, «sibling-файл» `.bak` невозможен —
//! версия-назад уходит в OPFS-хранилище (страхование потери не хуже:
//! содержимое прежней версии доступно при следующем запуске).
//!
//! Отказ разрешения/пикера — деградация: «Открыть копию» в OPFS (текст
//! уже прочитан через `File.text()` — чтение не требует разрешения).

use std::path::Path;
use std::str::FromStr;
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use std::path::PathBuf;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::JsValue;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use canvas_core::{Canvas, CanvasStorage, CoreError};

#[cfg(target_arch = "wasm32")]
use crate::opfs::opfs_name;
use crate::opfs::MirrorStore;

// ============================================================================
// Хранилище активного дискового канваса
// ============================================================================

/// Дисковое хранилище: синхронное зеркало (как у OPFS) + фоновая запись
/// активного файла через `FileSystemFileHandle.createWritable()`; версия-
/// назад — в OPFS (см. отклонение выше). На нативе — только зеркало.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // потребители — пикер/reopen (wasm); натив: только тесты
pub(crate) struct FsAccessStorage {
    inner: Mutex<MirrorStore>,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. FsAccessStorage
impl FsAccessStorage {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(MirrorStore::default()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MirrorStore> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Наполнить зеркала при открытии (текст уже прочитан с диска).
    pub(crate) fn seed_mirror(&self, path: &Path, text: &str) {
        self.lock().seed_text(path, text);
    }

    fn drain(&self) {
        let batch: Vec<_> = {
            let mut inner = self.lock();
            inner.queue.drain(..).collect()
        };
        if batch.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        spawn_disk_flush(batch);
        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(
            target: "canvas_web",
            count = batch.len(),
            "native: запись на диск пропущена (web-only путь)"
        );
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. FsAccessStorage
impl CanvasStorage for FsAccessStorage {
    fn load(&self, path: &Path) -> Result<Canvas, CoreError> {
        Canvas::from_str(&self.lock().load_text(path)?)
    }

    fn save(&self, canvas: &Canvas, path: &Path) -> Result<(), CoreError> {
        let text = canvas.to_json()?;
        self.lock().save_text(path, &text);
        self.drain();
        Ok(())
    }
}

/// Сброс очереди: активное имя → диск (через хэндл), `.bak`-имя → OPFS.
#[cfg(target_arch = "wasm32")]
fn spawn_disk_flush(batch: Vec<(PathBuf, String)>) {
    wasm_bindgen_futures::spawn_local(async move {
        let active = crate::web_state::active_name();
        let handle = crate::web_state::disk_handle();
        for (path, text) in batch {
            let Some(name) = opfs_name(&path) else {
                continue;
            };
            let is_bak = name.ends_with(".bak");
            if !is_bak {
                // Активный канвас — на диск; чужое имя (сменили файл
                // между save и flush) — в OPFS-хранилище, данные не теряем
                if Some(&name) == active.as_ref() {
                    if let Some(handle) = &handle {
                        match write_disk_text(handle, &text).await {
                            Ok(()) => tracing::debug!(
                                target: "canvas_web",
                                file = %name,
                                bytes = text.len(),
                                "диск: автосейв записан"
                            ),
                            Err(err) => tracing::error!(
                                target: "canvas_web",
                                file = %name,
                                error = ?err,
                                "диск: автосейв не записан (разрешение отозвано?)"
                            ),
                        }
                        continue;
                    }
                    tracing::warn!(target: "canvas_web", file = %name, "хэндл диска утрачен — запись уходит в OPFS");
                }
            }
            // Версия-назад (и чужие имена) — в OPFS
            match crate::opfs::opfs_root().await {
                Ok(root) => {
                    if let Err(err) = crate::opfs::write_opfs_text(&root, &name, &text).await {
                        tracing::warn!(target: "canvas_web", file = %name, error = ?err, "OPFS (страховка): запись не удалась");
                    }
                }
                Err(err) => {
                    tracing::warn!(target: "canvas_web", file = %name, error = ?err, "OPFS (страховка) недоступен")
                }
            }
        }
    });
}

/// Записать текст через дисковый хэндл (createWritable → write → close).
#[cfg(target_arch = "wasm32")]
async fn write_disk_text(
    handle: &web_sys::FileSystemFileHandle,
    text: &str,
) -> Result<(), JsValue> {
    let writable: web_sys::FileSystemWritableFileStream =
        wasm_bindgen_futures::JsFuture::from(handle.create_writable())
            .await?
            .into();
    wasm_bindgen_futures::JsFuture::from(writable.write_with_str(text)?).await?;
    let stream = writable.unchecked_ref::<web_sys::WritableStream>();
    wasm_bindgen_futures::JsFuture::from(stream.close()).await?;
    Ok(())
}

/// Прочитать текст через дисковый хэндл (getFile → text).
#[cfg(target_arch = "wasm32")]
async fn read_disk_text(
    handle: &web_sys::FileSystemFileHandle,
) -> Result<(String, String), JsValue> {
    let file: web_sys::File = wasm_bindgen_futures::JsFuture::from(handle.get_file())
        .await?
        .into();
    let name = file.name();
    let text = wasm_bindgen_futures::JsFuture::from(file.text())
        .await?
        .as_string()
        .unwrap_or_default();
    Ok((name, text))
}

/// Текст дискового файла для экспорта (публичный срез приватного читателя).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_disk_text_for_export(
    handle: &web_sys::FileSystemFileHandle,
) -> Result<String, JsValue> {
    read_disk_text(handle).await.map(|(_, text)| text)
}

// ============================================================================
// Разрешения FS Access
// ============================================================================

/// Разрешение readwrite: query (request=false) или request (жест).
#[cfg(target_arch = "wasm32")]
async fn permission_granted(handle: &web_sys::FileSystemFileHandle, request: bool) -> bool {
    let descriptor = web_sys::FileSystemHandlePermissionDescriptor::new();
    descriptor.set_mode(web_sys::FileSystemPermissionMode::Readwrite);
    let promise = if request {
        handle.request_permission_with_descriptor(&descriptor)
    } else {
        handle.query_permission_with_descriptor(&descriptor)
    };
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .ok()
        .and_then(|value| value.as_string())
        .is_some_and(|state| state == "granted")
}

/// granted? иначе — попытка request (жест ещё жив после пикера/клика).
#[cfg(target_arch = "wasm32")]
async fn ensure_readwrite(handle: &web_sys::FileSystemFileHandle) -> bool {
    if permission_granted(handle, false).await {
        return true;
    }
    permission_granted(handle, true).await
}

/// Доступен ли FS Access (`'showOpenFilePicker' in window`) — выбор при
/// старте/показ кнопки (план §4.1: таргет — Chromium; OPFS гарантирует
/// полезность без него). W6: кнопки панели статичны (деградация «Открыть
/// копию» честная); детектор пригодится W12 для скрытия кнопок.
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)] // см. док: W12 скроет кнопки без FS Access
pub(crate) fn available() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    js_sys::Reflect::has(
        window.unchecked_ref::<js_sys::Object>(),
        &"showOpenFilePicker".into(),
    )
    .unwrap_or(false)
}

// ============================================================================
// Открытие с диска (пикер по жесту) и reopen из недавних
// ============================================================================

/// «Открыть с диска…» — пикер (жест кнопки), чтение текста, открытие
/// как активной сцены с дисковым хранилищем. Отказ разрешения — копия
/// в OPFS; отмена пикера — тихий info.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn open_from_disk(
    proxy: winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>,
) {
    let Some(window) = web_sys::window() else {
        return;
    };
    // Фильтр пикера: .canvas как JSON (описание на языке проекта).
    // accept — Map<string, string[]>: у биндинга типизирован как
    // Object<JsString> — строится через unchecked_into с сырого объекта.
    let accept_raw = js_sys::Object::new();
    let extensions = js_sys::Array::from_iter([JsValue::from_str(".canvas")]);
    let _ = js_sys::Reflect::set(
        &accept_raw,
        &JsValue::from_str("application/json"),
        &extensions.into(),
    );
    let accept: js_sys::Object<js_sys::JsString> = accept_raw.unchecked_into();
    let accept_type = web_sys::FilePickerAcceptType::new();
    accept_type.set_accept(&accept);
    accept_type.set_description("Канвас CanvasDesk (.canvas)");
    let options = web_sys::OpenFilePickerOptions::new();
    options.set_types(&[accept_type]);
    let picked = window.show_open_file_picker_with_options(&options);
    let handles = match picked {
        Ok(promise) => wasm_bindgen_futures::JsFuture::from(promise).await,
        Err(err) => {
            tracing::warn!(target: "canvas_web", error = ?err, "пикер недоступен (нет FS Access?)");
            return;
        }
    };
    let handles = match handles {
        Ok(array) => array,
        Err(err) => {
            // AbortError — пользователь отменил пикер (штатно)
            tracing::info!(target: "canvas_web", error = ?err, "пикер отменён/отказан");
            return;
        }
    };
    let Some(handle_value) = handles
        .get(0)
        .dyn_into::<web_sys::FileSystemFileHandle>()
        .ok()
    else {
        tracing::warn!(target: "canvas_web", "пикер вернул не файл");
        return;
    };
    let (file_name, json) = match read_disk_text(&handle_value).await {
        Ok(pair) => pair,
        Err(err) => {
            tracing::error!(target: "canvas_web", error = ?err, "чтение выбранного файла не удалось");
            return;
        }
    };
    if !ensure_readwrite(&handle_value).await {
        // Нет readwrite: «Открыть копию» в OPFS (текст уже прочитан)
        tracing::info!(target: "canvas_web", file = %file_name, "readwrite не выдан — открывается копия в OPFS");
        crate::drop_files::import_to_opfs(&proxy, &file_name, json).await;
        return;
    }
    let storage = Arc::new(FsAccessStorage::new());
    let path = PathBuf::from(&file_name);
    storage.seed_mirror(&path, &json);
    crate::web_state::set_disk_handle(handle_value.clone());
    crate::web_state::set_active(file_name.clone(), crate::web_state::ActiveKind::Disk);
    crate::recent::store_disk_handle(&file_name, &handle_value).await;
    crate::recent::record_recent(&file_name).await;
    crate::toolbar::set_recent_label(&file_name);
    tracing::info!(target: "canvas_web", file = %file_name, "канвас открыт с диска (автосейв включён)");
    let _ = proxy.send_event(canvas_app::app::AppEvent::OpenScene {
        path,
        json,
        storage: Some(storage),
    });
}

/// Reopen из недавних (жест кнопки «Недавние»): дисковый хэндл →
/// requestPermission → диск; иначе OPFS-копия; ничего — warn.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn reopen_recent(
    proxy: winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>,
) {
    let Some(name) = crate::recent::recent_top().await else {
        tracing::info!(target: "canvas_web", "недавних нет — reopen нечего открывать");
        return;
    };
    // 1) Дисковый хэндл (персистентен в IndexedDB)
    if let Some(handle) = crate::recent::disk_handle_of(&name).await {
        if ensure_readwrite(&handle).await {
            if let Ok((file_name, json)) = read_disk_text(&handle).await {
                let storage = Arc::new(FsAccessStorage::new());
                let path = PathBuf::from(&file_name);
                storage.seed_mirror(&path, &json);
                crate::web_state::set_disk_handle(handle);
                crate::web_state::set_active(file_name.clone(), crate::web_state::ActiveKind::Disk);
                crate::toolbar::set_recent_label(&file_name);
                tracing::info!(target: "canvas_web", file = %file_name, "недавний канвас переоткрыт с диска");
                let _ = proxy.send_event(canvas_app::app::AppEvent::OpenScene {
                    path,
                    json,
                    storage: Some(storage),
                });
                return;
            }
        }
    }
    // 2) OPFS-копия (разрешений не требует)
    if let Ok(root) = crate::opfs::opfs_root().await {
        if let Ok(Some(json)) = crate::opfs::read_opfs_text(&root, &name).await {
            if let Some(storage) = crate::web_state::opfs_storage() {
                storage.seed_mirror(Path::new(&name), &json);
                crate::web_state::set_active(name.clone(), crate::web_state::ActiveKind::Opfs);
                crate::toolbar::set_recent_label(&name);
                tracing::info!(target: "canvas_web", file = %name, "недавний канвас переоткрыт из OPFS");
                let _ = proxy.send_event(canvas_app::app::AppEvent::OpenScene {
                    path: PathBuf::from(&name),
                    json,
                    storage: Some(storage),
                });
                return;
            }
        }
    }
    tracing::warn!(target: "canvas_web", file = %name, "недавний канвас недоступен (нет хэндла/копии)");
}

// ============================================================================
// Нативные тесты (натив-путь зеркала идентичен OpfsStorage — проверяем
// контракт трейта)
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// FsAccessStorage: тот же контракт трейта, что у OPFS-версии —
    /// зеркальный roundtrip без диска (I/O web-only).
    #[test]
    fn fs_storage_trait_contract() {
        let storage = FsAccessStorage::new();
        let path = Path::new("диск.canvas");
        assert!(storage.load(path).is_err(), "пустое хранилище — ошибка");
        let mut canvas = Canvas::default();
        canvas.extra.insert("v".into(), serde_json::json!(1));
        storage.save(&canvas, path).expect("save");
        let loaded = storage.load(path).expect("roundtrip");
        assert_eq!(loaded.extra.get("v").and_then(|v| v.as_i64()), Some(1));
    }

    /// seed_mirror: текст, положенный при открытии, читается синхронно.
    #[test]
    fn seed_then_load() {
        let storage = FsAccessStorage::new();
        let path = Path::new("открытый.canvas");
        let json = r#"{"nodes":[]}"#;
        storage.seed_mirror(path, json);
        let loaded = storage.load(path).expect("зеркало отдало текст");
        assert!(loaded.nodes.is_empty());
    }
}
