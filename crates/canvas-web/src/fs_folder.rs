//! FR-105 (мультиканвас C2, issue #6): рабочее пространство канвасов в
//! granted-папке через FS Access API (Chromium 86+; в Firefox/Safari
//! режима папки нет — всегда OPFS, №41c).
//!
//! Состав волны:
//! - [`FsAccessStore`] — реализация `WorkspaceStore` над папкой:
//!   синхронное зеркало «имя → ts» (паттерн `MirrorStore` из `opfs.rs`),
//!   поверх — очередь платформенных операций ([`FolderOp`]), сливаемая
//!   фоновым `spawn_local`-таском; сигнатуры трейта заморожены в C0
//!   (не менялись);
//! - [`choose_start_mode`] — чистая функция тихого выбора режима старта
//!   (№41c: сохранённый dir-хэндл И query-разрешение granted → папка;
//!   иначе тихо OPFS; `requestPermission` на старте НЕ вызывается —
//!   требует жеста);
//! - миграция OPFS → папка (№42a/№52a): пикер папки + исполнитель
//!   [`crate::workspace::execute_migration`] + переключение режима;
//! - watch внешних изменений (№45b/№53b): poll `lastModified` на
//!   focus/visibilitychange → сравнение снимков (чистая часть —
//!   `canvas_core::workspace::snapshot_changed`) → событие
//!   `AppEvent::ExtFileChanged`; «правки — в `.bak`» при перезагрузке;
//! - баннер потери доступа (№44b): детект (queryPermission/отказ записи
//!   NotAllowedError) → `AppEvent::StorageAccessLost`.
//!
//! Хэндл папки — JS-значение (`!Send`): живёт в `thread_local`
//! `web_state` (как DISK_HANDLE), в трейт-объекты не попадает. Персист
//! хэндла — IndexedDB через JS-глю `dirHandlePut`/`dirHandleGet`
//! (index.html, ключ `"workspace"` — по образцу `handlePut`/`handleGet`).

use std::collections::BTreeMap;
// wasm-only части (пикер/миграция/watch/активация папки)
#[cfg(target_arch = "wasm32")]
use std::path::Path;
#[cfg(target_arch = "wasm32")]
use std::str::FromStr;
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

use canvas_core::workspace::{CanvasEntry, EntryKind, MAX_CANVASES};
use canvas_core::Canvas;

use crate::workspace::{WorkspaceError, WorkspaceStore};

// Ключ персиста dir-хэндла в IndexedDB — "workspace" (задан в JS-глю
// `dirHandlePut`/`dirHandleGet`, index.html — по образцу handlePut/Get;
// константа живёт на стороне JS, дубль в Rust не нужен).

// ============================================================================
// Чистая часть: выбор режима старта (№41c)
// ============================================================================

/// Режим рабочего пространства канвасов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StartMode {
    /// Granted-папка FS Access (после переезда/разрешения).
    Folder,
    /// Браузерное OPFS-хранилище (тихий дефолт, №41c).
    Opfs,
}

/// Тихий выбор режима старта (№41c): папка — ТОЛЬКО если FS Access
/// доступен, есть сохранённый dir-хэндл и readwrite уже granted
/// (query, без жеста — `requestPermission` на старте не вызывается).
/// Любое «нет» — тихо OPFS (браузерная матрица: Firefox/Safari всегда
/// OPFS — строка хранилища не обещает диск).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // натив: только тесты
pub(crate) fn choose_start_mode(
    fs_available: bool,
    saved_handle: bool,
    granted: bool,
) -> StartMode {
    if fs_available && saved_handle && granted {
        StartMode::Folder
    } else {
        StartMode::Opfs
    }
}

// ============================================================================
// FsAccessStore — WorkspaceStore над granted-папкой
// ============================================================================

/// Платформенная операция очереди (синхронное зеркало обновляется сразу,
/// I/O уезжает фоновым таском — паттерн `MirrorStore`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum FolderOp {
    /// Создать/перезаписать файл текстом (create).
    Write { name: String, text: String },
    /// Переименовать: файл + `.bak`-близнец (R-T6), `handle.move()` с
    /// фолбэком write+delete — исполнитель ниже.
    Rename { old: String, new: String },
    /// Мягкое удаление (№15a): содержимое → `<name>.bak`, файл удаляется.
    Delete { name: String },
}

/// Синхронное состояние папки: имена файлов → ts (белый список `.canvas`
/// фильтруется в `list`; `.bak`-близнецы живут в зеркале, но скрыты).
#[derive(Debug, Default)]
struct FolderState {
    entries: BTreeMap<String, u64>,
    /// Монотонные ts для операций зеркала (детерминизм тестов).
    clock: u64,
    /// Очередь платформенных операций.
    outbox: Vec<FolderOp>,
}

impl FolderState {
    fn next_ts(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }
}

/// Хранилище рабочего стола канвасов над granted-папкой FS Access (C2).
/// Синхронное зеркало + фоновый сброс очереди (wasm — JS-операции папки;
/// натив — debug-лог, I/O web-only — контракт тот же, что у
/// `OpfsStorage`). Семантика = `MemWorkspaceStore` (эталон C0).
pub(crate) struct FsAccessStore {
    inner: Mutex<FolderState>,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // wasm: пикер/миграция/watch; натив: тесты
impl FsAccessStore {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(FolderState::default()),
        }
    }

    /// Отравленный Mutex не роняет хранилище (паттерн ядра).
    fn lock(&self) -> std::sync::MutexGuard<'_, FolderState> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Перестроить зеркало из листинга папки `[(имя, ts)]` (после старта/
    /// миграции/переподключения; `.canvas`-фильтр не нужен — кладём как
    /// есть, листинг фильтрует сам).
    pub(crate) fn seed_listing(&self, listing: &[(String, u64)]) {
        let mut inner = self.lock();
        inner.entries = listing.iter().cloned().collect();
    }

    /// Слить очередь: wasm — фоновый таск в папку; натив — debug-лог
    /// (контракт зеркала идентичен, I/O web-only).
    fn drain(&self) {
        let batch: Vec<_> = {
            let mut inner = self.lock();
            std::mem::take(&mut inner.outbox)
        };
        if batch.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        spawn_folder_ops(batch);
        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(
            target: "canvas_web",
            count = batch.len(),
            "native: операции папки пропущены (web-only путь)"
        );
    }

    /// Число операций в очереди (нативные тесты).
    #[cfg(test)]
    fn queued(&self) -> usize {
        self.lock().outbox.len()
    }
}

impl WorkspaceStore for FsAccessStore {
    fn list(&self) -> Vec<CanvasEntry> {
        self.lock()
            .entries
            .iter()
            .filter(|(name, _)| name.ends_with(".canvas"))
            .map(|(name, ts)| CanvasEntry {
                name: name.clone(),
                ts: *ts,
                kind: EntryKind::Folder,
                repo: None,
            })
            .collect()
    }

    fn create(&self, name: &str) -> Result<(), WorkspaceError> {
        if name.is_empty() || !name.ends_with(".canvas") {
            return Err(WorkspaceError::NameInvalid(name.to_owned()));
        }
        let mut inner = self.lock();
        let lower = name.to_lowercase();
        if inner
            .entries
            .keys()
            .any(|existing| existing.to_lowercase() == lower)
        {
            return Err(WorkspaceError::NameTaken(name.to_owned()));
        }
        if inner.entries.len() >= MAX_CANVASES {
            return Err(WorkspaceError::LimitReached);
        }
        let ts = inner.next_ts();
        inner.entries.insert(name.to_owned(), ts);
        inner.outbox.push(FolderOp::Write {
            name: name.to_owned(),
            text: empty_canvas_text(),
        });
        drop(inner);
        self.drain();
        Ok(())
    }

    fn rename(&self, old: &str, new: &str) -> Result<(), WorkspaceError> {
        if new.is_empty() || !new.ends_with(".canvas") {
            return Err(WorkspaceError::NameInvalid(new.to_owned()));
        }
        let mut inner = self.lock();
        if old == new && inner.entries.contains_key(old) {
            return Ok(()); // идемпотентный no-op (контракт C0)
        }
        let lower = new.to_lowercase();
        if inner
            .entries
            .keys()
            .any(|existing| existing.to_lowercase() == lower)
        {
            return Err(WorkspaceError::NameTaken(new.to_owned()));
        }
        let ts = inner
            .entries
            .remove(old)
            .ok_or_else(|| WorkspaceError::NotFound(old.to_owned()))?;
        inner.entries.insert(new.to_owned(), ts);
        // .bak-близнец переезжает вместе с файлом (контракт rename, R-T6).
        if let Some(bak_ts) = inner.entries.remove(&format!("{old}.bak")) {
            inner.entries.insert(format!("{new}.bak"), bak_ts);
        }
        inner.outbox.push(FolderOp::Rename {
            old: old.to_owned(),
            new: new.to_owned(),
        });
        drop(inner);
        self.drain();
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), WorkspaceError> {
        let mut inner = self.lock();
        inner
            .entries
            .remove(name)
            .ok_or_else(|| WorkspaceError::NotFound(name.to_owned()))?;
        // Мягкое удаление (№15a): из листинга исчезает, содержимое — в
        // .bak (замещает прежний; в зеркале — ts файла).
        let ts = inner.next_ts();
        inner.entries.insert(format!("{name}.bak"), ts);
        inner.outbox.push(FolderOp::Delete {
            name: name.to_owned(),
        });
        drop(inner);
        self.drain();
        Ok(())
    }

    fn exists(&self, name: &str) -> bool {
        self.lock().entries.contains_key(name)
    }
}

/// Пустой канвас как текст (сеяние `create`).
fn empty_canvas_text() -> String {
    Canvas::default()
        .to_json()
        .unwrap_or_else(|_| "{}".to_owned())
}

// ============================================================================
// wasm: платформенные операции папки (JS-рунтайм)
// ============================================================================

/// Слить очередь операций в папку (фоновый таск). Отказы I/O: потеря
/// доступа (NotAllowedError) → событие баннера №44b, прочее — warn-лог
/// (автосейв повторится следующей правкой).
#[cfg(target_arch = "wasm32")]
fn spawn_folder_ops(batch: Vec<FolderOp>) {
    wasm_bindgen_futures::spawn_local(async move {
        let Some(dir) = crate::web_state::folder_handle() else {
            tracing::warn!(target: "canvas_web", count = batch.len(), "папка не подключена — операции отброшены");
            return;
        };
        let mut touched = false;
        for op in batch {
            match op {
                FolderOp::Write { name, text } => {
                    if let Err(err) = dir_write_text(&dir, &name, &text).await {
                        report_folder_error("запись", &name, &err);
                    } else {
                        touched = true;
                    }
                }
                FolderOp::Rename { old, new } => {
                    if let Err(err) = dir_rename(&dir, &old, &new).await {
                        report_folder_error("переименование", &old, &err);
                    } else {
                        touched = true;
                    }
                }
                FolderOp::Delete { name } => {
                    // Мягкое удаление: текст (если файл жив) → .bak, файл — вон.
                    let text = dir_read_text(&dir, &name).await;
                    if let Some(text) = text {
                        if let Err(err) = dir_write_text(&dir, &format!("{name}.bak"), &text).await
                        {
                            report_folder_error("мягкое удаление (.bak)", &name, &err);
                            continue; // не удаляем оригинал без страховки
                        }
                    }
                    if let Err(err) = dir_remove(&dir, &name).await {
                        report_folder_error("удаление", &name, &err);
                    } else {
                        touched = true;
                    }
                }
            }
        }
        // Собственные операции меняют lastModified/состав папки — база
        // watch обновляется, чтобы poll на focus не счёл их внешними
        // изменениями (№45b; тот же инвариант у автосейва в fs_access).
        if touched {
            schedule_watch_refresh();
        }
    });
}

/// Отказ папочной операции: NotAllowedError — потеря доступа (баннер
/// №44b через AppEvent), прочее — warn.
#[cfg(target_arch = "wasm32")]
fn report_folder_error(what: &str, name: &str, err: &wasm_bindgen::JsValue) {
    let kind = err
        .dyn_ref::<js_sys::Error>()
        .and_then(|e| e.name().as_string())
        .unwrap_or_default();
    if kind == "NotAllowedError" {
        tracing::warn!(target: "canvas_web", file = name, what, "папка: доступ потерян (№44b)");
        crate::web_state::send_event(canvas_app::app::AppEvent::StorageAccessLost {
            detail: name.to_owned(),
        });
    } else {
        tracing::warn!(target: "canvas_web", file = name, what, error = ?err, "папка: операция не удалась");
    }
}

/// Прочитать текст файла папки; файла нет — `Ok(None)`.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn dir_read_text(
    dir: &web_sys::FileSystemDirectoryHandle,
    name: &str,
) -> Option<String> {
    let handle = match wasm_bindgen_futures::JsFuture::from(dir.get_file_handle(name)).await {
        Ok(value) => value,
        Err(_) => return None, // NotFound (штатно) или недоступен — None
    };
    let handle: web_sys::FileSystemFileHandle = handle.unchecked_into();
    let file: web_sys::File = wasm_bindgen_futures::JsFuture::from(handle.get_file())
        .await
        .ok()?
        .into();
    let text = wasm_bindgen_futures::JsFuture::from(file.text())
        .await
        .ok()?;
    text.as_string()
}

/// Записать текст в файл папки (create: true, createWritable → write → close).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn dir_write_text(
    dir: &web_sys::FileSystemDirectoryHandle,
    name: &str,
    text: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    let options = web_sys::FileSystemGetFileOptions::new();
    options.set_create(true);
    let handle: web_sys::FileSystemFileHandle =
        wasm_bindgen_futures::JsFuture::from(dir.get_file_handle_with_options(name, &options))
            .await?
            .into();
    let writable: web_sys::FileSystemWritableFileStream =
        wasm_bindgen_futures::JsFuture::from(handle.create_writable())
            .await?
            .into();
    wasm_bindgen_futures::JsFuture::from(writable.write_with_str(text)?).await?;
    let stream = writable.unchecked_ref::<web_sys::WritableStream>();
    wasm_bindgen_futures::JsFuture::from(stream.close()).await?;
    Ok(())
}

/// Удалить файл папки (removeEntry; NotFound — уже нет, Ok).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn dir_remove(
    dir: &web_sys::FileSystemDirectoryHandle,
    name: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    match wasm_bindgen_futures::JsFuture::from(dir.remove_entry(name)).await {
        Ok(_) => Ok(()),
        Err(err) => {
            let not_found = err
                .dyn_ref::<js_sys::Error>()
                .and_then(|e| e.name().as_string())
                .is_some_and(|kind| kind == "NotFoundError");
            if not_found {
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}

/// Переименовать файл в папке: `handle.move()` (нестандартный API
/// Chromium — через JS-глю `handleMove`), фолбэк write+delete; ОБЯЗАТЕЛЬНО
/// перенос `.bak`-близнеца (R-T6). Файла нет — тихий Ok (зеркало уже
/// отработало; гонка листинга не валит операцию).
#[cfg(target_arch = "wasm32")]
async fn dir_rename(
    dir: &web_sys::FileSystemDirectoryHandle,
    old: &str,
    new: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    for (from, to) in [(old, new), (&format!("{old}.bak"), &format!("{new}.bak"))] {
        let Ok(handle_value) =
            wasm_bindgen_futures::JsFuture::from(dir.get_file_handle(from)).await
        else {
            continue; // нет такого файла/близнеца — нечего переносить
        };
        let handle: web_sys::FileSystemFileHandle = handle_value.into();
        // 1) handle.move(newName) — быстрый путь (только имя, внутри той
        //    же папки; глю принимает (handle, newName) — оба аргумента)
        let to_value = wasm_bindgen::JsValue::from(to);
        if let Some(Ok(answer)) =
            crate::js_glue::call("handleMove", &[handle.clone().into(), to_value]).await
        {
            if answer.as_string().as_deref() == Some("moved") {
                continue;
            }
        }
        // 2) Фолбэк write+delete (move нет/отказал)
        let file: web_sys::File = wasm_bindgen_futures::JsFuture::from(handle.get_file())
            .await?
            .into();
        let text = wasm_bindgen_futures::JsFuture::from(file.text())
            .await?
            .as_string()
            .unwrap_or_default();
        dir_write_text(dir, to, &text).await?;
        dir_remove(dir, from).await?;
    }
    Ok(())
}

// ============================================================================
// wasm: листинг папки, разрешения, пикер
// ============================================================================

/// Листинг папки `[(имя, lastModified ms)]` — JS-глю `dirList`
/// (по образцу opfsList волны C1; фильтр `.canvas` — в Rust, ниже).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn dir_list(dir: &web_sys::FileSystemDirectoryHandle) -> Vec<(String, u64)> {
    let Some(Ok(value)) = crate::js_glue::call("dirList", &[dir.into()]).await else {
        return Vec::new();
    };
    let Ok(array) = value.dyn_into::<js_sys::Array>() else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|entry| {
            let object = entry.dyn_into::<js_sys::Object>().ok()?;
            let name = js_sys::Reflect::get(&object, &"name".into())
                .ok()?
                .as_string()?;
            let ts = js_sys::Reflect::get(&object, &"ts".into())
                .ok()?
                .as_f64()
                .map(|ts| ts.max(0.0) as u64)
                .unwrap_or(0);
            Some((name, ts))
        })
        .collect()
}

/// Снимок папки для watch (№45b): только белый список `.canvas`.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn folder_watch_snapshot(
    dir: &web_sys::FileSystemDirectoryHandle,
) -> canvas_core::workspace::WatchSnapshot {
    dir_list(dir)
        .await
        .into_iter()
        .filter(|(name, _)| name.ends_with(".canvas"))
        .collect()
}

/// query-разрешение readwrite у dir-хэндла (БЕЗ запроса — для тихого
/// старта №41c и watch-поллинга; `request` — только в жесте, см.
/// [`request_dir_granted`]). web-sys отдаёт типизированный
/// `Promise<JsString>` — ответ приходит как `JsString` → String.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn dir_query_granted(dir: &web_sys::FileSystemDirectoryHandle) -> bool {
    let handle: &web_sys::FileSystemHandle = dir.unchecked_ref();
    let descriptor = web_sys::FileSystemHandlePermissionDescriptor::new();
    descriptor.set_mode(web_sys::FileSystemPermissionMode::Readwrite);
    let promise = handle.query_permission_with_descriptor(&descriptor);
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .ok()
        .map(String::from)
        .is_some_and(|state| state == "granted")
}

/// request-разрешение readwrite (ТОЛЬКО в жесте пользователя — кнопка
/// «Переподключить» баннера №44b / согласие после пикера).
#[cfg(target_arch = "wasm32")]
async fn request_dir_granted(dir: &web_sys::FileSystemDirectoryHandle) -> bool {
    let handle: &web_sys::FileSystemHandle = dir.unchecked_ref();
    let descriptor = web_sys::FileSystemHandlePermissionDescriptor::new();
    descriptor.set_mode(web_sys::FileSystemPermissionMode::Readwrite);
    let promise = handle.request_permission_with_descriptor(&descriptor);
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .ok()
        .map(String::from)
        .is_some_and(|state| state == "granted")
}

/// Системный пикер папки (жест): Some(dir) — выбрана И readwrite
/// обеспечен (после пикера — query, при необходимости request в том же
/// жесте). Отмена пикера — None (тихо).
#[cfg(target_arch = "wasm32")]
async fn pick_folder() -> Option<web_sys::FileSystemDirectoryHandle> {
    let window = web_sys::window()?;
    let promise = window.show_directory_picker().ok()?;
    let picked = wasm_bindgen_futures::JsFuture::from(promise).await.ok()?;
    let dir: web_sys::FileSystemDirectoryHandle = picked.into();
    if dir_query_granted(&dir).await {
        return Some(dir);
    }
    // Пикер — жест: request в его же активации допустим
    if request_dir_granted(&dir).await {
        return Some(dir);
    }
    tracing::warn!(target: "canvas_web", "папка выбрана без readwrite — режим папки не включается");
    None
}

/// Персистентный dir-хэндл (IndexedDB, ключ "workspace" — JS-глю
/// `dirHandleGet`).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn saved_dir_handle() -> Option<web_sys::FileSystemDirectoryHandle> {
    let Some(Ok(value)) = crate::js_glue::call("dirHandleGet", &[]).await else {
        return None;
    };
    value.dyn_into::<web_sys::FileSystemDirectoryHandle>().ok()
}

/// Запомнить dir-хэндл (fire-and-forget).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn store_dir_handle(handle: &web_sys::FileSystemDirectoryHandle) {
    let _ = crate::js_glue::call("dirHandlePut", &[handle.into()]).await;
}

// ============================================================================
// wasm: подключение режима папки (общая точка переключения)
// ============================================================================

/// Подключить granted-папку как рабочее пространство: web_state
/// (хэндл + ActiveKind::Folder), зеркало `FsAccessStore` из листинга,
/// персональное хранилище активного канваса (FsAccessStorage, паттерн
/// `open_from_disk`), недавние, база watch-снимка. Возвращает (имя, json)
/// активного канваса для `OpenScene`.
#[cfg(target_arch = "wasm32")]
async fn activate_folder(
    dir: web_sys::FileSystemDirectoryHandle,
    name: &str,
    text: Option<String>,
) -> (String, String, Arc<dyn canvas_core::CanvasStorage>) {
    let storage = Arc::new(crate::fs_access::FsAccessStorage::new());
    let json = match text {
        Some(json) => json,
        None => {
            // Файла в папке нет (или битый) — сеем пустой (нативная
            // семантика load_or_seed; битый НЕ перезаписываем — читаем как
            // есть и парсинг решает).
            let json = empty_canvas_text();
            if let Err(err) = dir_write_text(&dir, name, &json).await {
                tracing::warn!(target: "canvas_web", file = name, error = ?err, "папка: сеяние не записано");
            }
            json
        }
    };
    storage.seed_mirror(Path::new(name), &json);
    crate::web_state::set_folder_handle(dir.clone());
    crate::web_state::set_active(name.to_owned(), crate::web_state::ActiveKind::Folder);
    rebuild_folder_mirror(&dir).await;
    reset_watch_snapshot(&dir).await;
    (
        name.to_owned(),
        json,
        storage as Arc<dyn canvas_core::CanvasStorage>,
    )
}

/// Перестроить зеркало общего `FsAccessStore` из листинга папки.
#[cfg(target_arch = "wasm32")]
async fn rebuild_folder_mirror(dir: &web_sys::FileSystemDirectoryHandle) {
    let listing = dir_list(dir).await;
    crate::web_state::fs_store().seed_listing(&listing);
}

// ============================================================================
// wasm: старт в режиме папки (№41c — вызов из init_scene)
// ============================================================================

/// Имя стартового канваса (без OPFS): `?canvas=` → недавний → default.
#[cfg(target_arch = "wasm32")]
async fn choose_canvas_name(params: &crate::url_params::WebParams) -> String {
    if let Some(name) = &params.canvas {
        tracing::info!(target: "canvas_web", file = %name, "стартовый канвас из URL (?canvas=)");
        return name.clone();
    }
    if let Some(name) = crate::recent::recent_top().await {
        return name;
    }
    crate::opfs::DEFAULT_CANVAS.to_string()
}

/// Тихая попытка старта в режиме папки (№41c): решение принимает чистая
/// [`choose_start_mode`] (FS Access доступен + сохранённый dir-хэндл +
/// query-readwrite granted — БЕЗ запроса жеста). None — тихо OPFS
/// (браузерная матрица: Firefox/Safari всегда OPFS). Вызывается из
/// `init_scene` ДО OPFS-пути; правки там минимальны (FR-105).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn try_folder_start(
    params: &crate::url_params::WebParams,
) -> Option<crate::opfs::WebScene> {
    let fs_available = crate::fs_access::available();
    let saved = saved_dir_handle().await;
    let granted = match &saved {
        Some(dir) => dir_query_granted(dir).await,
        None => false,
    };
    match choose_start_mode(fs_available, saved.is_some(), granted) {
        StartMode::Folder => {
            let dir = saved.expect("режим Folder — хэндл сохранён");
            let name = choose_canvas_name(params).await;
            let text = dir_read_text(&dir, &name).await;
            tracing::info!(target: "canvas_web", file = %name, "старт в режиме папки (granted dir-хэндл)");
            let (name, json, storage) = activate_folder(dir, &name, text).await;
            // Недавние: имя папочного канваса — как у каждой точки открытия
            wasm_bindgen_futures::spawn_local({
                let name = name.clone();
                async move {
                    crate::recent::record_recent(&name).await;
                }
            });
            let canvas = Canvas::from_str(&json).unwrap_or_default();
            Some(crate::opfs::WebScene {
                path: std::path::PathBuf::from(&name),
                storage,
                opfs: None,
                canvas,
                // Битой ссылки в режиме папки не бывает: имя выбиралось из
                // существующих (?canvas= проверяется чтением файла ниже).
                broken_link: None,
            })
        }
        StartMode::Opfs => {
            if fs_available && saved.is_some() {
                tracing::info!(target: "canvas_web", "dir-хэндл без granted-разрешения — тихий OPFS-старт (№41c)");
            }
            None
        }
    }
}

// ============================================================================
// wasm: баннер потери доступа №44b — действия кнопок
// ============================================================================

/// «Переподключить» (жест клика): персистентный/живой dir-хэндл →
/// requestPermission (жест) → успех: режим папки жив (зеркало и база
/// watch обновлены, баннер снимет App по событию), отказ — false.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn reconnect_flow() -> bool {
    // Живой хэндл в приоритете; его нет — персистентный из IndexedDB (async).
    let dir = match crate::web_state::folder_handle() {
        Some(dir) => dir,
        None => match saved_dir_handle().await {
            Some(dir) => dir,
            None => return false,
        },
    };
    if !request_dir_granted(&dir).await {
        tracing::info!(target: "canvas_web", "переподключение: разрешение не выдано");
        return false;
    }
    crate::web_state::set_folder_handle(dir.clone());
    rebuild_folder_mirror(&dir).await;
    reset_watch_snapshot(&dir).await;
    tracing::info!(target: "canvas_web", "папка переподключена (режим папки жив)");
    true
}

/// «Переключиться в браузерное» (№44b): OPFS-режим. Текущий канвас
/// (json передал App — несохранённые правки не теряем) сеется в OPFS,
/// сцена переоткрывается с OPFS-хранилищем. Dir-хэндл остаётся в
/// IndexedDB (следующий старт тихо OPFS: permission после перезагрузки
/// не granted — №41c; «Переехать на диск…» снова предложит папку).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn switch_to_browser(name: &str, json: &str) {
    let Ok(root) = crate::opfs::opfs_root().await else {
        tracing::warn!(target: "canvas_web", "OPFS недоступен — переключение только в памяти сессии");
        return;
    };
    if let Err(err) = crate::opfs::write_opfs_text(&root, name, json).await {
        tracing::warn!(target: "canvas_web", file = name, error = ?err, "OPFS: сеяние при переключении не удалось");
    }
    if let Some(storage) = crate::web_state::opfs_storage() {
        storage.seed_mirror(Path::new(name), json);
    }
    crate::web_state::set_active(name.to_owned(), crate::web_state::ActiveKind::Opfs);
    crate::web_state::clear_folder_handle();
    tracing::info!(target: "canvas_web", file = name, "переключение в браузерное хранилище (OPFS)");
}

// ============================================================================
// wasm: миграция OPFS → папка (№42a/№52a)
// ============================================================================

/// Листинг OPFS `[(имя, ts)]` — собственная итерация корня (values() →
/// AsyncIterator; js-глю opfsList — территория волны C1, не дублируем).
#[cfg(target_arch = "wasm32")]
async fn opfs_listing() -> Vec<(String, u64)> {
    let Ok(root) = crate::opfs::opfs_root().await else {
        return Vec::new();
    };
    let iter = root.values();
    let mut out = Vec::new();
    loop {
        let Ok(promise) = iter.next() else {
            break;
        };
        let Ok(result) = wasm_bindgen_futures::JsFuture::from(promise).await else {
            break;
        };
        if js_sys::Reflect::get(&result, &"done".into())
            .ok()
            .and_then(|done| done.as_bool())
            .unwrap_or(true)
        {
            break;
        }
        let Ok(value) = js_sys::Reflect::get(&result, &"value".into()) else {
            continue;
        };
        let handle: web_sys::FileSystemHandle = match value.dyn_into() {
            Ok(handle) => handle,
            Err(_) => continue,
        };
        if handle.kind() != web_sys::FileSystemHandleKind::File {
            continue;
        }
        let name = handle.name();
        let ts = match wasm_bindgen_futures::JsFuture::from(root.get_file_handle(&name)).await {
            Ok(file_handle) => {
                let file_handle: web_sys::FileSystemFileHandle = file_handle.into();
                wasm_bindgen_futures::JsFuture::from(file_handle.get_file())
                    .await
                    .ok()
                    .and_then(|f| f.dyn_into::<web_sys::File>().ok())
                    .map(|file| file.last_modified().max(0.0) as u64)
                    .unwrap_or(0)
            }
            Err(_) => 0,
        };
        out.push((name, ts));
    }
    out
}

/// Листинг OPFS как `CanvasEntry` (источник плана миграции; обработчик
/// `WebRequest::MigrateList` → `AppEvent::MigrateOpfsList`).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn opfs_entries() -> Vec<CanvasEntry> {
    opfs_listing()
        .await
        .into_iter()
        .filter(|(name, _)| name.ends_with(".canvas"))
        .map(|(name, ts)| CanvasEntry {
            name,
            ts,
            kind: EntryKind::Opfs,
            repo: None,
        })
        .collect()
}

/// Удалить файл из OPFS (removeEntry; NotFound — уже нет, Ok).
#[cfg(target_arch = "wasm32")]
async fn opfs_remove(root: &web_sys::FileSystemDirectoryHandle, name: &str) -> bool {
    match wasm_bindgen_futures::JsFuture::from(root.remove_entry(name)).await {
        Ok(_) => true,
        Err(err) => {
            let not_found = err
                .dyn_ref::<js_sys::Error>()
                .and_then(|e| e.name().as_string())
                .is_some_and(|kind| kind == "NotFoundError");
            not_found
        }
    }
}

/// Пикер папки + исполнение миграции выбранных канвасов (№42a/№52a).
/// Вызывается обработчиком WebRequest::MigrateRun из App ПО ЖЕСТУ клика
/// кнопки «Переехать» диалога (пикер требует жеста). `selected` —
/// выбранные имена файлов; активный канвас обязателен (договор-место
/// диалога), здесь — страховочный дедуп + добавка.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn pick_folder_and_migrate(
    proxy: winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>,
    mut selected: Vec<String>,
) {
    // 0) Страховка: активный канвас переезжает всегда (переключение
    // режима не может оставить живую сцену в OPFS).
    if let Some(active) = crate::web_state::active_name() {
        if !selected
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&active))
        {
            selected.push(active);
        }
    }
    // 1) Пикер папки (жест).
    let Some(dir) = pick_folder().await else {
        tracing::info!(target: "canvas_web", "миграция: пикер папки отменён");
        return;
    };
    store_dir_handle(&dir).await;
    // 2) План (C0): источник OPFS × выбранное × уже лежащее в папке.
    let source = opfs_entries().await;
    let target: Vec<CanvasEntry> = dir_list(&dir)
        .await
        .into_iter()
        .filter(|(name, _)| name.ends_with(".canvas"))
        .map(|(name, ts)| CanvasEntry {
            name,
            ts,
            kind: EntryKind::Folder,
            repo: None,
        })
        .collect();
    let plan = canvas_core::workspace::migration_plan(&source, &selected, &target);
    // 3) Исполнитель: та же машина состояний, что в нативных тестах
    //    (копирование → проверка → удаление; №52a — частичный сбой
    //    не теряет данные: оригинал удаляется только после Verify).
    let Ok(root) = crate::opfs::opfs_root().await else {
        send_migrate_failed(proxy, 0, "OPFS недоступен").await;
        return;
    };
    let mut driver = crate::workspace::MigrationDriver::new(&plan);
    loop {
        use crate::workspace::MigrationStep;
        match driver.step() {
            MigrationStep::Read { src, .. } => {
                match crate::opfs::read_opfs_text(&root, &src).await {
                    Ok(Some(text)) => driver.read_complete(text),
                    Ok(None) => driver.copy_fail(crate::workspace::WorkspaceError::NotFound(src)),
                    Err(err) => {
                        driver.copy_fail(crate::workspace::WorkspaceError::Io(format!("{err:?}")))
                    }
                }
            }
            MigrationStep::Write { dst, text, .. } => {
                if let Err(err) = dir_write_text(&dir, &dst, &text).await {
                    driver.copy_fail(crate::workspace::WorkspaceError::Io(format!("{err:?}")));
                } else {
                    driver.write_complete();
                }
            }
            MigrationStep::Verify { dst, .. } => {
                let has = wasm_bindgen_futures::JsFuture::from(dir.get_file_handle(&dst))
                    .await
                    .is_ok();
                driver.verify_complete(has);
            }
            MigrationStep::Remove { src, .. } => {
                if opfs_remove(&root, &src).await {
                    driver.remove_complete();
                } else {
                    driver.remove_fail();
                }
            }
            MigrationStep::Done => break,
        }
    }
    let report = driver.report();
    let moved = report.moved.clone();
    let failures = report.failed.len() + report.kept.len();
    tracing::info!(
        target: "canvas_web",
        moved = moved.len(),
        failed = report.failed.len(),
        kept = report.kept.len(),
        missing = report.missing.len(),
        "миграция OPFS→папка исполнена"
    );
    // 4) Чистка OPFS: .bak-близнецы переехавших канвасов тоже уходят
    //    (№52a «чистый переезд»; лучший-effорт — отказ не блокирует).
    for (src, _) in &moved {
        let _ = opfs_remove(&root, &format!("{src}.bak")).await;
    }
    // 5) Переключение в режим папки: только если активный канвас переехал.
    let active = crate::web_state::active_name();
    let active_moved = active
        .as_ref()
        .is_some_and(|name| moved.iter().any(|(src, _)| src.eq_ignore_ascii_case(name)));
    if failures > 0 || !active_moved {
        send_migrate_failed(proxy, moved.len(), "часть канвасов не переехала").await;
        return;
    }
    let active_dst = moved
        .iter()
        .find(|(src, _)| active.as_ref().is_some_and(|a| src.eq_ignore_ascii_case(a)))
        .map(|(_, dst)| dst.clone())
        .unwrap_or_else(|| active.clone().unwrap_or_default());
    let json = dir_read_text(&dir, &active_dst)
        .await
        .unwrap_or_else(empty_canvas_text);
    let (_, json, storage) = activate_folder(dir, &active_dst, Some(json)).await;
    crate::toolbar::set_recent_label(&active_dst);
    let _ = proxy.send_event(canvas_app::app::AppEvent::OpenScene {
        path: std::path::PathBuf::from(&active_dst),
        json,
        storage: Some(storage),
    });
    let _ = proxy.send_event(canvas_app::app::AppEvent::MigrateDone { moved: moved.len() });
}

/// Тост-отказ миграции (App закрывает диалог, показывает причину).
#[cfg(target_arch = "wasm32")]
async fn send_migrate_failed(
    proxy: winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>,
    moved: usize,
    reason: &str,
) {
    tracing::warn!(target: "canvas_web", moved, reason, "миграция не завершена целиком");
    let _ = proxy.send_event(canvas_app::app::AppEvent::MigrateFailed { moved });
}

// ============================================================================
// wasm: обработчики WebRequest-ов хранилища (конвейер FR-104, web_requests)
// ============================================================================
// Действия баннера №44b / диалога миграции №42a / тоста №45b приезжают из
// App обратным каналом (WebRequest) и исполняются здесь (см.
// `web_requests::handle`): ответы — AppEvent-ами через прокси — тот же
// конвейер, что у конвейера канвасов C1.

// ============================================================================
// wasm: watch внешних изменений (№45b/№53b) — poll lastModified
// ============================================================================

thread_local! {
    /// База сравнения watch: последний известный снимок папки (только
    /// `.canvas`; `.bak` не участвуют). `None` — снимка ещё не было
    /// (первый poll становится базой, молча).
    static WATCH_SNAPSHOT: std::cell::RefCell<Option<canvas_core::workspace::WatchSnapshot>> =
        const { std::cell::RefCell::new(None) };
}

/// Сбросить базу watch свежим снимком (после старта/миграции/записей).
#[cfg(target_arch = "wasm32")]
async fn reset_watch_snapshot(dir: &web_sys::FileSystemDirectoryHandle) {
    let snap = folder_watch_snapshot(dir).await;
    WATCH_SNAPSHOT.with(|cell| *cell.borrow_mut() = Some(snap));
}

/// Отложить сброс базы watch (после собственных записей — иначе poll на
/// focus счёл бы их внешними изменениями, №45b).
#[cfg(target_arch = "wasm32")]
pub(crate) fn schedule_watch_refresh() {
    wasm_bindgen_futures::spawn_local(async move {
        if let Some(dir) = crate::web_state::folder_handle() {
            reset_watch_snapshot(&dir).await;
        }
    });
}

/// Установить DOM-листенеры focus/visibilitychange (после построения
/// event loop). В режиме папки каждый триггер — poll `lastModified`
/// через листинг, diff против базы → событие «файл изменился снаружи»
/// для активного канваса (№45b/№53b: тост ВСЕГДА, даже без локальных
/// правок; спрашивает перезагрузку).
#[cfg(target_arch = "wasm32")]
pub(crate) fn install_watch(proxy: winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>) {
    use wasm_bindgen::prelude::Closure;
    let Some(window) = web_sys::window() else {
        return;
    };
    let target = window.unchecked_ref::<web_sys::EventTarget>();
    let focus_proxy = proxy.clone();
    let on_focus = Closure::wrap(Box::new(move || {
        let proxy = focus_proxy.clone();
        wasm_bindgen_futures::spawn_local(async move {
            poll_external_changes(proxy).await;
        });
    }) as Box<dyn FnMut()>);
    if let Err(err) = target.add_event_listener_with_callback(
        "focus",
        on_focus.as_ref().unchecked_ref::<js_sys::Function>(),
    ) {
        tracing::warn!(target: "canvas_web", ?err, "watch: focus-листенер не установлен");
        return;
    }
    on_focus.forget(); // singleton: живёт до выгрузки страницы

    let vis_proxy = proxy;
    let on_visible = Closure::wrap(Box::new(move || {
        let document = web_sys::window().and_then(|w| w.document());
        let visible = document
            .as_ref()
            .map(|d| d.visibility_state() == web_sys::VisibilityState::Visible)
            .unwrap_or(false);
        if !visible {
            return;
        }
        let proxy = vis_proxy.clone();
        wasm_bindgen_futures::spawn_local(async move {
            poll_external_changes(proxy).await;
        });
    }) as Box<dyn FnMut()>);
    if let Some(document) = window.document() {
        if let Err(err) = document.add_event_listener_with_callback(
            "visibilitychange",
            on_visible.as_ref().unchecked_ref::<js_sys::Function>(),
        ) {
            tracing::warn!(target: "canvas_web", ?err, "watch: visibility-листенер не установлен");
            return;
        }
    }
    on_visible.forget();
    tracing::info!(target: "canvas_web", "watch внешних изменений подключён (focus/visibilitychange)");
}

/// Один poll: разрешение → снимок → diff базы → событие для активного.
/// Недоступная папка (query ≠ granted) — баннер №44b. Первый poll —
/// только база (молча). Изменение АКТИВНОГО канваса → тост №45b/№53b
/// (всегда, даже без локальных правок).
#[cfg(target_arch = "wasm32")]
async fn poll_external_changes(
    proxy: winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>,
) {
    if crate::web_state::active_kind() != Some(crate::web_state::ActiveKind::Folder) {
        return;
    }
    let Some(dir) = crate::web_state::folder_handle() else {
        return;
    };
    if !dir_query_granted(&dir).await {
        let _ = proxy.send_event(canvas_app::app::AppEvent::StorageAccessLost {
            detail: "permission".to_owned(),
        });
        return;
    }
    let after = folder_watch_snapshot(&dir).await;
    let changed: Vec<String> = WATCH_SNAPSHOT.with(|cell| {
        let mut slot = cell.borrow_mut();
        let changed = match slot.as_ref() {
            Some(before) => canvas_core::workspace::snapshot_changed(before, &after),
            None => Vec::new(), // первый poll — только база
        };
        *slot = Some(after);
        changed
    });
    if let (Some(active), true) = (crate::web_state::active_name(), !changed.is_empty()) {
        if changed
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&active))
        {
            tracing::info!(target: "canvas_web", file = %active, "внешнее изменение активного канваса (№45b)");
            let _ = proxy.send_event(canvas_app::app::AppEvent::ExtFileChanged { name: active });
        }
    }
}

/// Перезагрузка активного канваса после внешнего изменения (кнопка
/// «Перезагрузить» тоста №45b): локальная версия (если были правки —
/// App передал json) сперва в `.bak` («правки — в .bak»), затем сцена
/// переоткрывается текстом из папки (OpenScene + папочное хранилище).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn reload_after_external(
    proxy: winit::event_loop::EventLoopProxy<canvas_app::app::AppEvent>,
    name: &str,
    local_json: Option<&str>,
) {
    let Some(dir) = crate::web_state::folder_handle() else {
        return;
    };
    if let Some(json) = local_json {
        if let Err(err) = dir_write_text(&dir, &format!("{name}.bak"), json).await {
            tracing::warn!(target: "canvas_web", file = name, error = ?err, "перезагрузка: локальная версия не ушла в .bak");
        }
    }
    let json = dir_read_text(&dir, name)
        .await
        .unwrap_or_else(empty_canvas_text);
    let storage = Arc::new(crate::fs_access::FsAccessStorage::new());
    storage.seed_mirror(Path::new(name), &json);
    crate::web_state::set_active(name.to_owned(), crate::web_state::ActiveKind::Folder);
    reset_watch_snapshot(&dir).await;
    let _ = proxy.send_event(canvas_app::app::AppEvent::OpenScene {
        path: std::path::PathBuf::from(name),
        json,
        storage: Some(storage as Arc<dyn canvas_core::CanvasStorage>),
    });
}

// ============================================================================
// Нативные тесты (зеркало FsAccessStore = эталон MemWorkspaceStore)
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::workspace::WorkspaceStore;

    fn names(store: &FsAccessStore) -> Vec<String> {
        store.list().into_iter().map(|e| e.name).collect()
    }

    // --- выбор режима старта (№41c) ----------------------------------------

    #[test]
    fn start_mode_folder_only_with_granted_saved_handle() {
        use StartMode::*;
        // Полный комплект → папка
        assert_eq!(choose_start_mode(true, true, true), Folder);
        // FS Access есть, но хэндла нет / разрешение не granted → OPFS
        assert_eq!(choose_start_mode(true, false, true), Opfs);
        assert_eq!(choose_start_mode(true, true, false), Opfs);
        assert_eq!(choose_start_mode(true, false, false), Opfs);
        // Браузерная матрица: Firefox/Safari (FS Access нет) — всегда OPFS,
        // даже если что-то сохранилось
        assert_eq!(choose_start_mode(false, true, true), Opfs);
        assert_eq!(choose_start_mode(false, false, false), Opfs);
    }

    // --- контракт зеркала (эталон — MemWorkspaceStore) ---------------------

    #[test]
    fn create_list_exists_roundtrip() {
        let store = FsAccessStore::new();
        store.create("first.canvas").expect("создание");
        store.create("второй.canvas").expect("создание");
        assert!(store.exists("first.canvas"));
        assert!(!store.exists("ghost.canvas"));
        let mut list = names(&store);
        list.sort();
        assert_eq!(list, ["first.canvas", "второй.canvas"]);
        assert_eq!(store.queued(), 0, "очередь слита (натив — лог)");
        // kind листинга — Folder (источник записей)
        assert!(store.list().iter().all(|e| e.kind == EntryKind::Folder));
    }

    #[test]
    fn create_rejects_collisions_invalid_names_and_limit() {
        let store = FsAccessStore::new();
        store.create("x.canvas").expect("создание");
        assert_eq!(
            store.create("x.canvas"),
            Err(crate::workspace::WorkspaceError::NameTaken(
                "x.canvas".into()
            ))
        );
        assert_eq!(
            store.create("без-расширения"),
            Err(crate::workspace::WorkspaceError::NameInvalid(
                "без-расширения".into()
            ))
        );
        assert_eq!(
            store.create(""),
            Err(crate::workspace::WorkspaceError::NameInvalid("".into()))
        );
        assert_eq!(
            store.create("X.canvas"),
            Err(crate::workspace::WorkspaceError::NameTaken(
                "X.canvas".into()
            )),
            "регистронезависимая коллизия (Windows-папки)"
        );
        // лимит MAX_CANVASES (№16)
        let full = FsAccessStore::new();
        for i in 0..MAX_CANVASES {
            full.create(&format!("c{i:05}.canvas"))
                .expect("в пределах лимита");
        }
        assert_eq!(
            full.create("overflow.canvas"),
            Err(crate::workspace::WorkspaceError::LimitReached)
        );
    }

    #[test]
    fn rename_moves_file_and_bak_twin() {
        let store = FsAccessStore::new();
        store.create("a.canvas").expect("создание");
        // сеем .bak-близнеца напрямую в зеркало (листинг папки так и делает)
        store.seed_listing(&[("a.canvas".to_owned(), 1), ("a.canvas.bak".to_owned(), 2)]);
        store.rename("a.canvas", "b.canvas").expect("ренейм");
        assert!(!store.exists("a.canvas"));
        assert!(store.exists("b.canvas"));
        assert!(
            store.exists("b.canvas.bak"),
            ".bak-близнец переехал вместе с файлом (R-T6)"
        );
        assert!(!store.exists("a.canvas.bak"));
        // ренейм в занятое / несуществующее / идемпотентный no-op
        store.create("c.canvas").expect("создание");
        assert_eq!(
            store.rename("b.canvas", "c.canvas"),
            Err(crate::workspace::WorkspaceError::NameTaken(
                "c.canvas".into()
            ))
        );
        assert_eq!(
            store.rename("ghost.canvas", "d.canvas"),
            Err(crate::workspace::WorkspaceError::NotFound(
                "ghost.canvas".into()
            ))
        );
        assert_eq!(store.rename("b.canvas", "b.canvas"), Ok(()));
    }

    #[test]
    fn delete_is_soft_and_hidden_from_listing() {
        let store = FsAccessStore::new();
        store.seed_listing(&[("gone.canvas".to_owned(), 7)]);
        store.delete("gone.canvas").expect("удаление");
        assert!(!store.exists("gone.canvas"));
        assert!(store.exists("gone.canvas.bak"), "страховка в .bak (№15a)");
        assert!(
            store.list().is_empty(),
            ".bak скрыт из листинга белым списком .canvas"
        );
        // повторное удаление уже удалённого
        assert_eq!(
            store.delete("gone.canvas"),
            Err(crate::workspace::WorkspaceError::NotFound(
                "gone.canvas".into()
            ))
        );
        // повторное создание того же имени допустимо (бак не мешает)
        store
            .create("gone.canvas")
            .expect("создание после удаления");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn seed_listing_rebuilds_mirror() {
        let store = FsAccessStore::new();
        store.seed_listing(&[
            ("a.canvas".to_owned(), 10),
            ("a.canvas.bak".to_owned(), 5),
            ("b.canvas".to_owned(), 20),
        ]);
        let list = names(&store);
        assert_eq!(list, ["a.canvas", "b.canvas"], "только .canvas");
        assert!(store.exists("a.canvas.bak"));
        // ts листинга — из листинга папки
        let a = store.list().into_iter().find(|e| e.name == "a.canvas");
        assert_eq!(a.map(|e| e.ts), Some(10));
    }
}
