//! FR-104 (мультиканвас C1): обратный канал App → web-слой — обработчик
//! [`WebRequest`]-ов. App платформенно-нейтрален (canvas-web не может быть
//! его зависимостью), поэтому запросы складываются в очередь на стороне
//! App (`pending_web_requests`) и дренажируются обёрткой `TourAwareApp`
//! после КАЖДОГО события цикла (паттерн tour-сигналов FR-028 v2 — latency
//! минимальна, `about_to_wait` был бы реже). Ответы возвращаются
//! `AppEvent`-ами через `EventLoopProxy`.
//!
//! Конвейер (план §3.2):
//! - `CanvasList` → свежий листинг workspace-хранилища (FR-106: режим
//!   папки — листинг granted-папки в зеркало `FsAccessStore`; иначе OPFS
//!   — глю `opfsList` → `OpfsStore`) → `AppEvent::CanvasList` +
//!   `AppEvent::StorageMode` (строка хранилища менеджера №51a);
//! - `CanvasOp` → `WorkspaceStore`-операция (валидация синхронно, мутация
//!   фоном) → `AppEvent::CanvasOpDone` (ошибка — человекочитаемый текст
//!   `WorkspaceError::to_string`); Delete дополнительно чистит recent
//!   (№15a, глю `recentRemove`);
//! - `CanvasFallback` (№35a → №31c) — открыть канвас-фолбэк: верхний из
//!   недавних, который существует и ≠ занятому имени; недавних нет —
//!   СВЕЖИЙ «Canvas N» (default.canvas может быть сам занят другой
//!   вкладкой — иначе модал зациклится, см. FR-104 §Фолбэк занятости).
//!   TODO(FR-104): C3 заменит на менеджер канвасов.
//!
//! FR-105 (мультиканвас C2): сюда же приходят действия хранилища рабочего
//! пространства (issue #6) — отдельный мост НЕ заводится, конвейер один:
//! - `StorageReconnect` — «Переподключить» баннера №44b (requestPermission
//!   в жесте клика);
//! - `StorageSwitchBrowser` — «Переключиться в браузерное» №44b;
//! - `MigrateList` — листинг OPFS для чекбокс-листа диалога миграции №42a;
//! - `MigrateRun` — пикер папки + исполнитель миграции №42a/№52a;
//! - `StorageReloadExternal` — перезагрузка после внешнего изменения №45b
//!   (локальные правки — сперва в `.bak`).
//! Исполнители — `crate::fs_folder` (granted-папка FS Access).
//!
//! FR-106 (мультиканвас C3, issue #7): действия менеджера канвасов —
//! тот же конвейер (отдельного моста не заводим):
//! - `CanvasPersist` — `navigator.storage.persist()` в точке первого
//!   открытия менеджера (R-T3; TODO из FR-104 — вызов переехал из
//!   init_scene);
//! - `CanvasOpen` — открыть канвас из списка (текст из workspace-хранилища
//!   → `OpenScene` + set_active + record_recent; механика CanvasFallback);
//! - `CanvasDuplicate` — копия ПОЛНОГО `.canvas` №8/№27a, сразу активна;
//! - `CanvasImport` — пикер файла №25b: копия в workspace-хранилище с
//!   авто-суффиксом коллизии №26b + тост «создана копия»;
//! - `CanvasOpenDisk` — пустое состояние №23a: «Открыть файл с диска…»
//!   (fs_access::open_from_disk — существующий путь);
//! - `CanvasExportActive` — «Экспорт» №25b (export.rs, download-blob);
//! - `CanvasRestoreBak` — undo мягкого удаления №15a (`.bak` → файл);
//! - `CanvasMoveCameraKey` — перенос ключа камеры при ренейме активного
//!   №30b (localStorage, формат C0 `canvasdesk.camera.<имя>`).
//!
//! FR-107 (мультиканвас C4, issue #8): чип/камера/title — тот же конвейер:
//! - `CanvasCameraSave` — снимок камеры активного канваса в localStorage
//!   (№12/№30b): уход с канваса и выгрузка/скрытие страницы;
//! - `CanvasCameraLoad` — чтение камеры при открытии → ответ
//!   `CanvasCameraRestored` (нет ключа — `None`: тихий дефолт);
//! - `CanvasActiveKind` — режим активного для чипа №21c (Disk — «только
//!   просмотр») → ответ `CanvasActiveKind`;
//! - ренейм АКТИВНОГО — web-зеркало переезжает на новое имя в той же
//!   точке (`set_active`: Web Locks + `?canvas=` + document.title №28a
//!   + недавние №15a).

#![cfg(target_arch = "wasm32")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use canvas_app::app::{AppEvent, WebRequest};
use canvas_core::workspace::{
    auto_name, collision_suffix, name_taken, CanvasEntry, CanvasOp, EntryKind,
};
use winit::event_loop::EventLoopProxy;

use crate::workspace::WorkspaceStore;

/// Обработать запрос App (fire-and-forget: каждый запрос — свой
/// spawn_local-таск; ответ приедет AppEvent-ом через прокси).
pub(crate) fn handle(request: WebRequest, proxy: &EventLoopProxy<AppEvent>) {
    let proxy = proxy.clone();
    wasm_bindgen_futures::spawn_local(async move {
        match request {
            WebRequest::CanvasList => {
                send_canvas_list(&proxy).await;
            }
            WebRequest::CanvasOp(op) => {
                let error = run_canvas_op(&op);
                let _ = proxy.send_event(AppEvent::CanvasOpDone { op, error });
            }
            WebRequest::CanvasFallback { avoid } => {
                open_fallback(&avoid, &proxy).await;
            }
            // --- FR-105 (C2): действия хранилища (issue #6) ---------------
            WebRequest::StorageReconnect => {
                if crate::fs_folder::reconnect_flow().await {
                    let _ = proxy.send_event(AppEvent::StorageReconnected);
                }
            }
            WebRequest::StorageSwitchBrowser { name, json } => {
                crate::fs_folder::switch_to_browser(&name, &json).await;
            }
            WebRequest::MigrateList => {
                let entries = crate::fs_folder::opfs_entries().await;
                let _ = proxy.send_event(AppEvent::MigrateOpfsList(entries));
            }
            WebRequest::MigrateRun { selected } => {
                crate::fs_folder::pick_folder_and_migrate(proxy, selected).await;
            }
            WebRequest::StorageReloadExternal { name, local_json } => {
                crate::fs_folder::reload_after_external(proxy, &name, local_json.as_deref()).await;
            }
            // --- FR-106 (C3, issue #7): действия менеджера канвасов --------
            WebRequest::CanvasPersist => {
                crate::opfs_store::request_storage_persist();
            }
            WebRequest::CanvasOpen { name } => {
                open_workspace_canvas(&name, &proxy).await;
            }
            WebRequest::CanvasDuplicate { from, to } => {
                duplicate_canvas(&from, &to, &proxy).await;
            }
            WebRequest::CanvasImport => {
                import_canvas_picker(&proxy).await;
            }
            WebRequest::CanvasOpenDisk => {
                crate::fs_access::open_from_disk(proxy).await;
            }
            WebRequest::CanvasExportActive => {
                crate::export::export_active().await;
            }
            WebRequest::CanvasRestoreBak { name } => {
                restore_bak(&name, &proxy).await;
            }
            WebRequest::CanvasMoveCameraKey { old, new } => {
                move_camera_key(&old, &new);
            }
            // --- FR-107 (C4, issue #8): чип/камера (№12/№30b/№21c) --------
            WebRequest::CanvasCameraSave { name, snapshot } => {
                crate::camera_web::save(&name, &snapshot);
            }
            WebRequest::CanvasCameraLoad { name } => {
                let snapshot = crate::camera_web::load(&name);
                let _ = proxy.send_event(AppEvent::CanvasCameraRestored { snapshot });
            }
            WebRequest::CanvasActiveKind => {
                let disk =
                    crate::web_state::active_kind() == Some(crate::web_state::ActiveKind::Disk);
                let _ = proxy.send_event(AppEvent::CanvasActiveKind { disk });
            }
        }
    });
}

// ============================================================================
// FR-106 (C3): workspace-хранилище текущего режима (папка/OPFS)
// ============================================================================

/// Режим папки сейчас: активный канвас живёт в granted-папке (ActiveKind
/// C2 + живой dir-хэндл) — все операции менеджера идут в папку, не в OPFS.
fn folder_mode() -> bool {
    crate::web_state::active_kind() == Some(crate::web_state::ActiveKind::Folder)
        && crate::web_state::folder_handle().is_some()
}

/// Свежий листинг workspace-хранилища ТЕКУЩЕГО режима: папка — листинг
/// granted-папки (зеркало `FsAccessStore` пересеивается); иначе OPFS
/// (тихий дефолт №41c, в т.ч. Firefox/Safari — им папки не обещаем).
async fn workspace_listing() -> Vec<CanvasEntry> {
    if folder_mode() {
        let dir = crate::web_state::folder_handle().expect("folder_mode — хэндл жив");
        let listing = crate::fs_folder::dir_list(&dir).await;
        crate::web_state::fs_store().seed_listing(&listing);
        return crate::web_state::fs_store().list();
    }
    crate::opfs_store::workspace_entries().await
}

/// Свежий листинг менеджеру + режим строки хранилища (№51a): вместе —
/// одно событие «мир изменился» (`CanvasList` + `StorageMode`).
async fn send_canvas_list(proxy: &EventLoopProxy<AppEvent>) {
    let entries = workspace_listing().await;
    let _ = proxy.send_event(AppEvent::StorageMode {
        folder: folder_mode(),
        fs_available: crate::fs_access::available(),
    });
    let _ = proxy.send_event(AppEvent::CanvasList(entries));
}

/// Пересеять зеркало workspace-хранилища после собственной записи
/// (дубликат/импорт/undo — иначе листинг отстанет на один запрос).
async fn refresh_workspace_mirror() {
    if folder_mode() {
        let dir = crate::web_state::folder_handle().expect("folder_mode — хэндл жив");
        let listing = crate::fs_folder::dir_list(&dir).await;
        crate::web_state::fs_store().seed_listing(&listing);
    } else {
        let _ = crate::opfs_store::seed_workspace().await;
    }
}

/// Прочитать текст файла workspace-хранилища (папка/OPFS); `Ok(None)` —
/// файла нет. `.bak`-близнецы читаются тем же путём (undo №15a).
async fn read_workspace_text(name: &str) -> Result<Option<String>, String> {
    if folder_mode() {
        let dir = crate::web_state::folder_handle().expect("folder_mode — хэндл жив");
        return Ok(crate::fs_folder::dir_read_text(&dir, name).await);
    }
    let root = crate::opfs::opfs_root()
        .await
        .map_err(|err| format!("OPFS root: {err:?}"))?;
    crate::opfs::read_opfs_text(&root, name)
        .await
        .map_err(|err| format!("чтение: {err:?}"))
}

/// Записать текст в файл workspace-хранилища (папка/OPFS, create:true —
/// файл создаётся при отсутствии; образец — drop-импорт).
async fn write_workspace_text(name: &str, text: &str) -> Result<(), String> {
    if folder_mode() {
        let dir = crate::web_state::folder_handle().expect("folder_mode — хэндл жив");
        return crate::fs_folder::dir_write_text(&dir, name, text)
            .await
            .map_err(|err| format!("запись в папку: {err:?}"));
    }
    let root = crate::opfs::opfs_root()
        .await
        .map_err(|err| format!("OPFS root: {err:?}"))?;
    crate::opfs::write_opfs_text(&root, name, text)
        .await
        .map_err(|err| format!("запись: {err:?}"))
}

/// Удалить файл workspace-хранилища (только `.bak`-близнецы undo №15a;
/// живые канвасы удаляет `WorkspaceStore::delete` — мягко, в `.bak`).
async fn remove_workspace_file(name: &str) -> Result<(), String> {
    if folder_mode() {
        let dir = crate::web_state::folder_handle().expect("folder_mode — хэндл жив");
        return crate::fs_folder::dir_remove(&dir, name)
            .await
            .map_err(|err| format!("удаление из папки: {err:?}"));
    }
    let root = crate::opfs::opfs_root()
        .await
        .map_err(|err| format!("OPFS root: {err:?}"))?;
    match wasm_bindgen_futures::JsFuture::from(root.remove_entry(name)).await {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("removeEntry: {err:?}")),
    }
}

// ============================================================================
// FR-106 (C3): открытые действия менеджера
// ============================================================================

/// Открыть канвас из списка менеджера: текст из workspace-хранилища →
/// персональное хранилище активного (seed_mirror) → `OpenScene` +
/// set_active + record_recent (механика CanvasFallback). Тихая потеря
/// undo №11 — уже семантика `OpenScene`.
async fn open_workspace_canvas(name: &str, proxy: &EventLoopProxy<AppEvent>) {
    let json = match read_workspace_text(name).await {
        Ok(Some(json)) => json,
        Ok(None) => {
            // Файла нет: гонка create→open (фон-запись stora ещё не дошла)
            // или файл исчез наружу — сеем пустой (нативная семантика
            // load_or_seed; прецедент — open_fallback/seed_canvas_text).
            let text = crate::workspace::empty_canvas_text();
            if let Err(err) = write_workspace_text(name, &text).await {
                tracing::error!(target: "canvas_web", file = name, %err, "менеджер: сеяние отсутствующего канваса не удалось");
                return;
            }
            refresh_workspace_mirror().await;
            text
        }
        Err(err) => {
            tracing::error!(target: "canvas_web", file = name, %err, "менеджер: чтение канваса не удалось");
            return;
        }
    };
    let kind = if folder_mode() {
        crate::web_state::ActiveKind::Folder
    } else {
        crate::web_state::ActiveKind::Opfs
    };
    let storage: Option<Arc<dyn canvas_core::CanvasStorage>> = if folder_mode() {
        let storage = Arc::new(crate::fs_access::FsAccessStorage::new());
        storage.seed_mirror(Path::new(name), &json);
        Some(storage)
    } else {
        crate::web_state::opfs_storage()
            .map(|storage| storage as Arc<dyn canvas_core::CanvasStorage>)
    };
    let Some(storage) = storage else {
        tracing::error!(target: "canvas_web", file = name, "менеджер: OPFS-хранилище не инициализировано");
        return;
    };
    crate::web_state::set_active(name.to_owned(), kind);
    crate::recent::record_recent(name).await;
    tracing::info!(target: "canvas_web", file = name, "менеджер: канвас открыт");
    let _ = proxy.send_event(AppEvent::OpenScene {
        path: PathBuf::from(name),
        json,
        storage: Some(storage),
    });
}

/// Дубликат (№8/№27a): копия ПОЛНОГО `.canvas` (сценарии/заморозки/extra —
/// сырой текст) `from` → `to`; копия сразу активна (`OpenScene`). Имя `to`
/// уже вычислено `copy_name` (суффикс из i18n, коллизии — авто-суффикс).
async fn duplicate_canvas(from: &str, to: &str, proxy: &EventLoopProxy<AppEvent>) {
    let Some(json) = read_workspace_text(from).await.ok().flatten() else {
        tracing::warn!(target: "canvas_web", from, to, "дубликат: исходный канвас не читается");
        return;
    };
    if let Err(err) = write_workspace_text(to, &json).await {
        tracing::error!(target: "canvas_web", from, to, %err, "дубликат: запись копии не удалась");
        return;
    }
    refresh_workspace_mirror().await;
    tracing::info!(target: "canvas_web", from, to, "дубликат создан (полный .canvas)");
    open_workspace_canvas(to, proxy).await;
    send_canvas_list(proxy).await;
}

/// «Импорт файла…» (№25b): системный пикер (жест кнопки менеджера) —
/// копия в workspace-хранилище (НЕ открытие как диск); коллизия имени —
/// авто-суффикс « (N)» + тост «создана копия» (№26b).
async fn import_canvas_picker(proxy: &EventLoopProxy<AppEvent>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let accept_raw = js_sys::Object::new();
    let extensions = js_sys::Array::from_iter([wasm_bindgen::JsValue::from_str(".canvas")]);
    let _ = js_sys::Reflect::set(
        &accept_raw,
        &wasm_bindgen::JsValue::from_str("application/json"),
        &extensions.into(),
    );
    let accept: js_sys::Object<js_sys::JsString> = accept_raw.unchecked_into();
    let accept_type = web_sys::FilePickerAcceptType::new();
    accept_type.set_accept(&accept);
    accept_type.set_description("Канвас CanvasDesk (.canvas)");
    let options = web_sys::OpenFilePickerOptions::new();
    options.set_types(&[accept_type]);
    let picked = match window.show_open_file_picker_with_options(&options) {
        Ok(promise) => promise,
        Err(err) => {
            tracing::warn!(target: "canvas_web", ?err, "импорт: пикер недоступен (нет FS Access?)");
            return;
        }
    };
    let handles = match wasm_bindgen_futures::JsFuture::from(picked).await {
        Ok(array) => array,
        Err(err) => {
            tracing::info!(target: "canvas_web", ?err, "импорт: пикер отменён/отказан");
            return;
        }
    };
    use wasm_bindgen::JsCast;
    let Some(handle) = handles
        .get(0)
        .dyn_into::<web_sys::FileSystemFileHandle>()
        .ok()
    else {
        tracing::warn!(target: "canvas_web", "импорт: пикер вернул не файл");
        return;
    };
    let file: web_sys::File = match wasm_bindgen_futures::JsFuture::from(handle.get_file()).await {
        Ok(file) => file.into(),
        Err(err) => {
            tracing::error!(target: "canvas_web", ?err, "импорт: чтение выбранного файла не удалось");
            return;
        }
    };
    let raw_name = file.name();
    let json = match wasm_bindgen_futures::JsFuture::from(file.text()).await {
        Ok(text) => text.as_string().unwrap_or_default(),
        Err(err) => {
            tracing::error!(target: "canvas_web", ?err, "импорт: чтение текста не удалось");
            return;
        }
    };
    import_to_workspace(proxy, &raw_name, json).await;
}

/// Импорт текста в workspace-хранилище: санитизация имени, коллизия —
/// авто-суффикс №26b, запись, открытие копии, тост «создана копия»
/// (после `OpenScene` — тост открытия не затирает его).
async fn import_to_workspace(proxy: &EventLoopProxy<AppEvent>, raw_name: &str, json: String) {
    let Some(sanitized) = crate::url_params::sanitize_canvas_name(raw_name) else {
        tracing::warn!(target: "canvas_web", file = %raw_name, "импорт: имя файла небезопасно — отклонено");
        return;
    };
    // Коллизия (№26b): «имя (1).canvas» + тост «создана копия».
    let listing = workspace_listing().await;
    let taken = name_taken(&sanitized, &listing);
    let name = if taken {
        collision_suffix(&sanitized, &listing)
    } else {
        sanitized
    };
    let renamed = taken;
    if let Err(err) = write_workspace_text(&name, &json).await {
        tracing::error!(target: "canvas_web", file = %name, %err, "импорт: запись в workspace не удалась");
        return;
    }
    refresh_workspace_mirror().await;
    tracing::info!(target: "canvas_web", file = %name, "импорт: копия в workspace-хранилище");
    open_workspace_canvas(&name, proxy).await;
    if renamed {
        let _ = proxy.send_event(AppEvent::CanvasSavedAsCopy { name });
    }
    send_canvas_list(proxy).await;
}

/// Undo мягкого удаления (№15a): `<name>.bak` → `<name>` (текст
/// восстанавливается, `.bak` убирается), свежий листинг менеджеру.
/// Сцена не переключается — восстановленный канвас просто снова в списке.
async fn restore_bak(name: &str, proxy: &EventLoopProxy<AppEvent>) {
    let bak = format!("{name}.bak");
    let Some(json) = read_workspace_text(&bak).await.ok().flatten() else {
        tracing::warn!(target: "canvas_web", file = %bak, "undo удаления: .bak не читается");
        return;
    };
    if let Err(err) = write_workspace_text(name, &json).await {
        tracing::error!(target: "canvas_web", file = name, %err, "undo удаления: запись не удалась");
        return;
    }
    if let Err(err) = remove_workspace_file(&bak).await {
        // Не блокируем: .bak скрыт из листинга белым списком .canvas.
        tracing::warn!(target: "canvas_web", file = %bak, %err, "undo удаления: .bak не убран");
    }
    refresh_workspace_mirror().await;
    tracing::info!(target: "canvas_web", file = name, "undo удаления: канвас восстановлен из .bak");
    send_canvas_list(proxy).await;
}

/// Перенос ключа камеры при ренейме АКТИВНОГО канваса (№12/№30b):
/// `canvasdesk.camera.<old>` → `canvasdesk.camera.<new>` в localStorage
/// (формат заморожен в C0). Ключа нет (камера дефолтная) — тихий no-op.
fn move_camera_key(old: &str, new: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(Some(storage)) = window.local_storage() else {
        tracing::warn!(target: "canvas_web", "ренейм: localStorage недоступен (ключ камеры не перенесён)");
        return;
    };
    let (old_key, new_key) = (
        canvas_core::workspace::camera_key_for(old),
        canvas_core::workspace::camera_key_for(new),
    );
    match storage
        .get_item(&old_key)
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
    {
        Some(value) => {
            let _ = storage.set_item(&new_key, &value);
            let _ = storage.remove_item(&old_key);
            tracing::info!(target: "canvas_web", %old_key, %new_key, "ренейм: ключ камеры перенесён");
        }
        None => {
            tracing::debug!(target: "canvas_web", %old_key, "ренейм: ключа камеры нет — перенос не нужен")
        }
    }
}

/// Выполнить операцию хранилища (валидация — синхронно по зеркалу;
/// сама мутация уходит фоном внутри стора). FR-106: стор — режим
/// ТЕКУЩЕГО workspace (папка/OPFS), не только OPFS. Отказ → текст ошибки
/// для `CanvasOpDone` (тосты менеджера). Delete дополнительно чистит
/// recent (№15a — запись не переживает свой файл).
fn run_canvas_op(op: &CanvasOp) -> Option<String> {
    let result = if folder_mode() {
        let store = crate::web_state::fs_store();
        match op {
            CanvasOp::Create { name } => store.create(name),
            CanvasOp::Rename { old, new } => store.rename(old, new),
            CanvasOp::Delete { name } => store.delete(name),
        }
    } else {
        let Some(store) = crate::web_state::opfs_workspace() else {
            return Some("OPFS-хранилище недоступно".to_owned());
        };
        match op {
            CanvasOp::Create { name } => store.create(name),
            CanvasOp::Rename { old, new } => store.rename(old, new),
            CanvasOp::Delete { name } => store.delete(name),
        }
    };
    // №15a: удалённый канвас уходит из недавних (fire-and-forget, как
    // record_recent — IndexedDB-запись не блокирует ответ).
    // FR-107 (C4, №28a): ренейм АКТИВНОГО — web-зеркало переезжает на новое
    // имя в той же точке: set_active переводит Web Lock, `?canvas=` и
    // заголовок вкладки (ренейм входит в крючок смены активного №28a),
    // недавние — старая запись не переживает имя (№15a-паттерн).
    if result.is_ok() {
        if let CanvasOp::Rename { old, new } = op {
            let kind = crate::web_state::active_kind();
            if kind.is_some()
                && crate::web_state::active_name()
                    .is_some_and(|active| active.eq_ignore_ascii_case(old))
            {
                crate::web_state::set_active(new.clone(), kind.expect("проверено выше"));
                let (old, new) = (old.clone(), new.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    crate::recent::remove_recent(&old).await;
                    crate::recent::record_recent(&new).await;
                });
            }
        }
        if let CanvasOp::Delete { name } = op {
            let name = name.clone();
            wasm_bindgen_futures::spawn_local(async move {
                crate::recent::remove_recent(&name).await;
            });
        }
    }
    result.err().map(|err| err.to_string())
}

/// Открыть канвас-фолбэк после «Выбрать другой» (№35a): механика битой
/// ссылки №31c — верхний существующий недавний ≠ `avoid`; недавних нет —
/// свежее автоимя «Canvas N» (НЕ default.canvas: он может быть сам
/// занят, зациклив модал). Открытие — как у DOM-drop/reopen: OPFS-текст
/// → зеркало `OpfsStorage` → `OpenScene`.
async fn open_fallback(avoid: &str, proxy: &EventLoopProxy<AppEvent>) {
    let Ok(root) = crate::opfs::opfs_root().await else {
        tracing::error!(target: "canvas_web", avoid, "фолбэк: OPFS недоступен");
        return;
    };
    // Свежий листинг — имена для выбора фолбэка/автоимени.
    let names = match crate::opfs_store::opfs_list().await {
        Ok(files) => files.into_iter().map(|(name, _)| name).collect::<Vec<_>>(),
        Err(err) => {
            tracing::error!(target: "canvas_web", %err, avoid, "фолбэк: листинг OPFS не получен");
            return;
        }
    };
    let recent = crate::recent::recent_list().await;
    let name = match crate::opfs_store::broken_link_fallback(avoid, &recent, &names) {
        Some(name) => name,
        None => fresh_canvas_name(&names),
    };
    // Текст: существующий файл читается; свежее автоимя сеется пустым
    // (default.canvas сюда не попадает — см. выше).
    let json = match crate::opfs::read_opfs_text(&root, &name).await {
        Ok(Some(text)) => text,
        Ok(None) => seed_canvas_text(&root, &name).await,
        Err(err) => {
            tracing::error!(
                target: "canvas_web",
                file = %name,
                error = ?err,
                "фолбэк: чтение не удалось — сцена не переключается"
            );
            return;
        }
    };
    let Some(storage) = crate::web_state::opfs_storage() else {
        tracing::error!(target: "canvas_web", file = %name, "фолбэк: OPFS-хранилище не инициализировано");
        return;
    };
    storage.seed_mirror(Path::new(&name), &json);
    crate::web_state::set_active(name.clone(), crate::web_state::ActiveKind::Opfs);
    crate::recent::record_recent(&name).await;
    tracing::info!(target: "canvas_web", file = %name, avoid, "фолбэк: открыт другой канвас");
    let _ = proxy.send_event(AppEvent::OpenScene {
        path: PathBuf::from(&name),
        json,
        storage: Some(storage),
    });
}

/// Свежее автоимя «Canvas»/«Canvas N» (№6/№39c) по текущему листингу:
/// `avoid` существует в листинге — auto_name с ним не столкнётся.
fn fresh_canvas_name(names: &[String]) -> String {
    let existing: Vec<CanvasEntry> = names
        .iter()
        .map(|name| CanvasEntry {
            name: name.clone(),
            ts: 0,
            kind: EntryKind::Opfs,
            repo: None,
        })
        .collect();
    auto_name(&existing)
}

/// Прописать новый пустой канвас в OPFS (сеяние свежего автоимени фолбэка)
/// и вернуть его текст для `OpenScene`.
async fn seed_canvas_text(root: &web_sys::FileSystemDirectoryHandle, name: &str) -> String {
    let text = crate::workspace::empty_canvas_text();
    if let Err(err) = crate::opfs::write_opfs_text(root, name, &text).await {
        tracing::warn!(
            target: "canvas_web",
            file = name,
            error = ?err,
            "фолбэк: сеяние нового канваса не записалось (автосейв повторит)"
        );
    }
    // Зеркало workspace должно увидеть новую запись сразу.
    let _ = crate::opfs_store::seed_workspace().await;
    text
}
