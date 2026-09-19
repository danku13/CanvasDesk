//! M8/W6 (wasm-port §4.1/§4.2): OPFS-хранилище канвасов — фолбэк и дефолт
//! браузерной сборки (работает без диска и без разрешений).
//!
//! Контракт `CanvasStorage` — синхронный (нативная семантика W3), браузер
//! же даёт только async API. Мост — двухуровневый:
//!
//! 1. **[`MirrorStore`]** (чистая, тестируется нативно): синхронное зеркало
//!    «путь → текст» для `load()` трейта + очередь записей. `save()`
//!    сериализует канвас и ставит в очередь ПАРУ записей — прежняя версия
//!    как `<имя>.canvas.bak` (семантика нативного `save_with_backup`,
//!    SPEC §9) и новая версия.
//! 2. **OPFS-сброс** (wasm): очередь сливается в OPFS фоновым
//!    `spawn_local`-таском (fire-and-forget, ошибки — в консоль-лог);
//!    `load()` читает из зеркала мгновенно.
//!
//! Инициализация ([`init_scene`]) — до построения App (async-точка между
//! `start()` и `spawn_desk`): выбрать имя (`?canvas=` → недавний из
//! IndexedDB → `default.canvas`), прочитать из OPFS (или сеять), наполить
//! зеркало. Отказ OPFS целиком — деградация в `MemStorage` (страница
//! обязана открыться, план §7).

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use canvas_core::MemStorage;
use canvas_core::{Canvas, CanvasStorage, CoreError};
#[cfg(target_arch = "wasm32")]
use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use crate::url_params::WebParams;

/// Имя канваса по умолчанию (аналог `default.canvas` нативного старта).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // потребители — async-инициализация (wasm); натив: только тесты
pub(crate) const DEFAULT_CANVAS: &str = "default.canvas";

// ============================================================================
// Чистая часть (нативные тесты гейтов каркаса)
// ============================================================================

/// Ключ хранилища → имя файла OPFS: берётся только имя файла (OPFS —
/// плоская ФС); путь без имени (`/`, `.`) — None.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. DEFAULT_CANVAS
pub(crate) fn opfs_name(path: &Path) -> Option<String> {
    let name = path.file_name()?;
    name.to_str().map(str::to_string)
}

/// Имя версии-назад: конкатенация `.bak` (натив `save_with_backup` для
/// `x.canvas` даёт `x.canvas.bak` через `with_extension` — та же строка).
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. DEFAULT_CANVAS
pub(crate) fn bak_name(name: &str) -> String {
    format!("{name}.bak")
}

/// Синхронное состояние web-хранилища: зеркало «путь → текст» + очередь
/// асинхронных записей. `Send + Sync` — только `String`/`PathBuf` внутри,
/// поэтому трейт-объект [`OpfsStorage`] законен на любом таргете.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. DEFAULT_CANVAS
#[derive(Default)]
pub(crate) struct MirrorStore {
    /// Последний известный текст по пути (источник синхронного `load`).
    pub(crate) mirror: BTreeMap<PathBuf, String>,
    /// Записи на диск: (имя OPFS, текст) — пара bak+текущий на каждый save.
    pub(crate) queue: VecDeque<(PathBuf, String)>,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. DEFAULT_CANVAS
impl MirrorStore {
    /// Сохранить текст канваса: прежняя версия уходит в очередь как
    /// `.bak`, новая — в зеркало и в очередь. Ошибка сериализации
    /// обнаружена ДО вызова (см. `OpfsStorage::save`) — здесь отказа нет.
    pub(crate) fn save_text(&mut self, path: &Path, text: &str) {
        if let (Some(prev), Some(name)) = (self.mirror.get(path).cloned(), opfs_name(path)) {
            self.queue.push_back((PathBuf::from(bak_name(&name)), prev));
        }
        self.mirror.insert(path.to_path_buf(), text.to_string());
        if let Some(name) = opfs_name(path) {
            self.queue
                .push_back((PathBuf::from(name), text.to_string()));
        }
    }

    /// Положить текст в зеркало без очереди (инициализация: содержимое
    /// уже в OPFS — очередь должна остаться пустой).
    pub(crate) fn seed_text(&mut self, path: &Path, text: &str) {
        self.mirror.insert(path.to_path_buf(), text.to_string());
    }

    /// Синхронная загрузка из зеркала: Missing → NotFound (как `MemStorage`).
    pub(crate) fn load_text(&self, path: &Path) -> Result<String, CoreError> {
        self.mirror.get(path).cloned().ok_or_else(|| {
            CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("web-хранилище: файл не найден: {}", path.display()),
            ))
        })
    }
}

// ============================================================================
// Хранилище за трейтом CanvasStorage
// ============================================================================

/// OPFS-хранилище: синхронное зеркало + фоновая запись в OPFS (wasm).
/// На нативе (rlib-тесты) очередь сливается в debug-лог — поведение
/// зеркала идентично, I/O web-only.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. DEFAULT_CANVAS
pub(crate) struct OpfsStorage {
    inner: Mutex<MirrorStore>,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. DEFAULT_CANVAS
impl OpfsStorage {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(MirrorStore::default()),
        }
    }

    /// Отравленный Mutex не роняет хранилище (паттерн MemStorage).
    fn lock(&self) -> std::sync::MutexGuard<'_, MirrorStore> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }

    /// Наполнить зеркало при инициализации (содержимое уже в OPFS).
    pub(crate) fn seed_mirror(&self, path: &Path, text: &str) {
        self.lock().seed_text(path, text);
    }

    /// Слить очередь записей: wasm — фоновый таск в OPFS; натив — лог.
    fn drain(&self) {
        let batch: Vec<_> = {
            let mut inner = self.lock();
            inner.queue.drain(..).collect()
        };
        if batch.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        spawn_opfs_flush(batch);
        #[cfg(not(target_arch = "wasm32"))]
        tracing::debug!(
            target: "canvas_web",
            count = batch.len(),
            "native: OPFS-запись пропущена (web-only путь)"
        );
    }

    /// Число файлов в очереди (нативные тесты).
    #[cfg(test)]
    fn queued(&self) -> usize {
        self.lock().queue.len()
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // см. DEFAULT_CANVAS
impl CanvasStorage for OpfsStorage {
    fn load(&self, path: &Path) -> Result<Canvas, CoreError> {
        Canvas::from_str(&self.lock().load_text(path)?)
    }

    fn save(&self, canvas: &Canvas, path: &Path) -> Result<(), CoreError> {
        // Сериализация синхронно: ошибка формата не должна теряться тихо
        let text = canvas.to_json()?;
        self.lock().save_text(path, &text);
        self.drain();
        Ok(())
    }
}

/// Фоновый сброс очереди в OPFS: одна запись = один createWritable-цикл;
/// ошибки не паникуют (автосейв повторится при следующей правке), но
/// видны в консоли (?log=debug — приёмка W7).
#[cfg(target_arch = "wasm32")]
fn spawn_opfs_flush(batch: Vec<(PathBuf, String)>) {
    wasm_bindgen_futures::spawn_local(async move {
        let root = match opfs_root().await {
            Ok(root) => root,
            Err(err) => {
                tracing::error!(
                    target: "canvas_web",
                    error = ?err,
                    count = batch.len(),
                    "OPFS: корень недоступен, записи пропущены"
                );
                return;
            }
        };
        for (path, text) in batch {
            let Some(name) = opfs_name(&path) else {
                continue;
            };
            match write_opfs_text(&root, &name, &text).await {
                Ok(()) => tracing::debug!(
                    target: "canvas_web",
                    file = %name,
                    bytes = text.len(),
                    "OPFS: записано"
                ),
                Err(err) => tracing::error!(
                    target: "canvas_web",
                    file = %name,
                    error = ?err,
                    "OPFS: ошибка записи"
                ),
            }
        }
    });
}

// ============================================================================
// Async-примитивы OPFS (только wasm: window/StorageManager)
// ============================================================================

/// Корень OPFS origin'а (`navigator.storage.getDirectory()`).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn opfs_root() -> Result<web_sys::FileSystemDirectoryHandle, wasm_bindgen::JsValue>
{
    use wasm_bindgen::JsCast;
    let window = web_sys::window()
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("нет window (OPFS недоступен)"))?;
    let storage = window.navigator().storage();
    let root = wasm_bindgen_futures::JsFuture::from(storage.get_directory()).await?;
    root.dyn_into::<web_sys::FileSystemDirectoryHandle>()
        .map_err(|err| {
            wasm_bindgen::JsValue::from_str(&format!("getDirectory вернул не каталог: {err:?}"))
        })
}

/// Прочитать текст файла OPFS; файла нет → `Ok(None)` (NotFoundError
/// отделяется от прочих отказов — инициализация сеет только при «пусто»).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn read_opfs_text(
    root: &web_sys::FileSystemDirectoryHandle,
    name: &str,
) -> Result<Option<String>, wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    let handle_value = match wasm_bindgen_futures::JsFuture::from(root.get_file_handle(name)).await
    {
        Ok(value) => value,
        Err(err) => {
            let is_not_found = err
                .dyn_ref::<js_sys::Error>()
                .and_then(|e| e.name().as_string())
                .is_some_and(|name| name == "NotFoundError");
            if is_not_found {
                return Ok(None);
            }
            return Err(err);
        }
    };
    let handle: web_sys::FileSystemFileHandle = handle_value
        .dyn_into()
        .map_err(|err| wasm_bindgen::JsValue::from(format!("хэндл не файл: {err:?}")))?;
    let file: web_sys::File = wasm_bindgen_futures::JsFuture::from(handle.get_file())
        .await?
        .into();
    let text = wasm_bindgen_futures::JsFuture::from(file.text()).await?;
    Ok(text.as_string())
}

/// Записать текст в файл OPFS (createWritable → write → close).
#[cfg(target_arch = "wasm32")]
pub(crate) async fn write_opfs_text(
    root: &web_sys::FileSystemDirectoryHandle,
    name: &str,
    text: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;
    // create: true — файла может не быть (сеяние/первая запись); без опции
    // get_file_handle бросил бы NotFoundError
    let options = web_sys::FileSystemGetFileOptions::new();
    options.set_create(true);
    let handle: web_sys::FileSystemFileHandle =
        wasm_bindgen_futures::JsFuture::from(root.get_file_handle_with_options(name, &options))
            .await?
            .into();
    let writable: web_sys::FileSystemWritableFileStream =
        wasm_bindgen_futures::JsFuture::from(handle.create_writable())
            .await?
            .into();
    wasm_bindgen_futures::JsFuture::from(writable.write_with_str(text)?).await?;
    // close() объявлен на родителе WritableStream (FileSystemWritableFileStream
    // его расширяет; биндинга close на наследнике web-sys не генерирует)
    let stream = writable.unchecked_ref::<web_sys::WritableStream>();
    wasm_bindgen_futures::JsFuture::from(stream.close()).await?;
    Ok(())
}

// ============================================================================
// Инициализация сцены (до построения App)
// ============================================================================

/// Результат инициализации: путь/хранилище/модель для `SceneState`.
/// `opfs` — Some, когда хранилище — OPFS (переживёт перезагрузку): только
/// оно регистрируется в web_state как общее (DOM-drop/reopen подставляют
/// его в OpenScene); `?stress`/отказ OPFS — None (MemStorage, без сейва).
#[cfg(target_arch = "wasm32")]
pub(crate) struct WebScene {
    pub path: PathBuf,
    pub storage: Arc<dyn CanvasStorage>,
    pub opfs: Option<Arc<OpfsStorage>>,
    pub canvas: Canvas,
}

/// Выбор и загрузка стартового канваса (план §4.2, UX-поток):
/// `?canvas=` → недавний (IndexedDB) → `default.canvas`; файла нет — сеем.
/// Отказ OPFS целиком — `MemStorage` (страница открывается всегда).
/// `?stress` — сразу в память: нагрузочная сцена не пишет OPFS/recent.
#[cfg(target_arch = "wasm32")]
pub(crate) async fn init_scene(params: &WebParams) -> WebScene {
    if params.stress.is_some() {
        return WebScene {
            path: PathBuf::from("stress.canvas"),
            storage: Arc::new(MemStorage::new()),
            opfs: None,
            canvas: Canvas::default(), // реальная сцена строится spawn_desk'ом
        };
    }
    let (root, name) = match choose_canvas(params).await {
        Ok(pair) => pair,
        Err(err) => {
            tracing::warn!(
                target: "canvas_web",
                %err,
                "OPFS недоступен — сцена в памяти (без сохранения)"
            );
            return WebScene {
                path: PathBuf::from(DEFAULT_CANVAS),
                storage: Arc::new(MemStorage::new()),
                opfs: None,
                canvas: Canvas::default(),
            };
        }
    };
    let storage = Arc::new(OpfsStorage::new());
    match read_opfs_text(&root, &name).await {
        Ok(Some(text)) => match Canvas::from_str(&text) {
            Ok(canvas) => {
                storage.seed_mirror(Path::new(&name), &text);
                tracing::info!(target: "canvas_web", file = %name, "канвас загружен из OPFS");
                record(name, storage, canvas)
            }
            // Битый файл — как натив load_or_seed: сеем поверх (лог + warn)
            Err(err) => {
                seed_over(storage, &root, &name, format!("битый канвас в OPFS: {err}")).await
            }
        },
        Ok(None) => seed_over(storage, &root, &name, "файла нет — сеется новый".into()).await,
        Err(err) => {
            // Читаемая, но не открываемая OPFS — сеять опасно (перезапись
            // может погубить данные при временной ошибке): честная деградация
            // в память, файл не трогаем.
            tracing::warn!(target: "canvas_web", file = %name, error = ?err, "OPFS: чтение не удалось — сцена в памяти");
            WebScene {
                path: PathBuf::from(&name),
                storage: Arc::new(MemStorage::new()),
                opfs: None,
                canvas: Canvas::default(),
            }
        }
    }
}

/// Собрать результат: имя → активный (web_state), запись в недавние.
#[cfg(target_arch = "wasm32")]
fn record(name: String, storage: Arc<OpfsStorage>, canvas: Canvas) -> WebScene {
    crate::web_state::set_active(name.clone(), crate::web_state::ActiveKind::Opfs);
    // IndexedDB-запись асинхронная и не влияет на старт (ошибки — в лог);
    // копия имени — иммутабельный ключ, гонок с DOM нет.
    wasm_bindgen_futures::spawn_local({
        let name = name.clone();
        async move {
            crate::recent::record_recent(&name).await;
        }
    });
    WebScene {
        path: PathBuf::from(&name),
        storage: Arc::clone(&storage) as Arc<dyn CanvasStorage>,
        opfs: Some(storage),
        canvas,
    }
}

/// Сеять канвас поверх отсутствующего/битого файла (нативная семантика
/// `load_or_seed`): сериализованный дефолт уходит в OPFS через очередь.
#[cfg(target_arch = "wasm32")]
async fn seed_over(
    storage: Arc<OpfsStorage>,
    root: &web_sys::FileSystemDirectoryHandle,
    name: &str,
    reason: String,
) -> WebScene {
    tracing::info!(target: "canvas_web", file = %name, reason, "сеется новый канвас");
    let canvas = Canvas::default();
    let text = canvas.to_json().unwrap_or_else(|_| "{}".to_string());
    // Прямая запись (до App): гарантированный старт с файлом на месте;
    // ошибка — в лог, зеркало всё равно наполнено (автосейв повторит).
    if let Err(err) = write_opfs_text(root, name, &text).await {
        tracing::warn!(target: "canvas_web", file = %name, error = ?err, "OPFS: сеяние не записалось");
    }
    storage.seed_mirror(Path::new(name), &text);
    record(name.to_string(), storage, canvas)
}

/// Выбрать имя стартового канваса и получить корень OPFS.
#[cfg(target_arch = "wasm32")]
async fn choose_canvas(
    params: &WebParams,
) -> Result<(web_sys::FileSystemDirectoryHandle, String), String> {
    let root = opfs_root()
        .await
        .map_err(|err| format!("OPFS root: {err:?}"))?;
    if let Some(name) = &params.canvas {
        tracing::info!(target: "canvas_web", file = %name, "стартовый канвас из URL (?canvas=)");
        return Ok((root, name.clone()));
    }
    if let Some(name) = crate::recent::recent_top().await {
        tracing::info!(target: "canvas_web", file = %name, "стартовый канвас из недавних");
        return Ok((root, name));
    }
    Ok((root, DEFAULT_CANVAS.to_string()))
}

// ============================================================================
// Нативные тесты: контракт зеркала = синхронная часть трейта
// ============================================================================

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use canvas_core::CanvasStorage as _;

    fn canvas_with(name: &str) -> Canvas {
        let mut canvas = Canvas::default();
        canvas.extra.insert("name".into(), serde_json::json!(name));
        canvas
    }

    /// opfs_name: только имя файла; bak_name: конкатенация .bak.
    #[test]
    fn name_helpers() {
        assert_eq!(
            opfs_name(Path::new("dir/проект.canvas")).as_deref(),
            Some("проект.canvas")
        );
        assert_eq!(opfs_name(Path::new("/")), None);
        assert_eq!(opfs_name(Path::new(".")), None);
        assert_eq!(bak_name("x.canvas"), "x.canvas.bak");
    }

    /// save_text: новая версия в зеркало + пара (bak, новая) в очередь;
    /// seed_text: зеркало без очереди (содержимое уже в OPFS).
    #[test]
    fn mirror_save_queues_bak_and_file() {
        let mut store = MirrorStore::default();
        let path = Path::new("a.canvas");
        store.seed_text(path, "первый");
        assert!(store.queue.is_empty(), "сеяние не пишет очередь");
        assert_eq!(store.load_text(path).as_deref().ok(), Some("первый"));

        store.save_text(path, "второй");
        assert_eq!(store.load_text(path).as_deref().ok(), Some("второй"));
        assert_eq!(store.queue.len(), 2, "bak + новая версия");
        assert_eq!(
            store.queue[0],
            (PathBuf::from("a.canvas.bak"), "первый".to_string())
        );
        assert_eq!(
            store.queue[1],
            (PathBuf::from("a.canvas"), "второй".to_string())
        );

        // Первый save после сеяния: bak = сеянный текст (версия-назад с диска)
        let mut store = MirrorStore::default();
        store.seed_text(path, "сеянный");
        store.save_text(path, "правка");
        assert_eq!(
            store.queue[0],
            (PathBuf::from("a.canvas.bak"), "сеянный".to_string())
        );
    }

    /// load из пустого зеркала — NotFound (тот же код ошибки, что у
    /// MemStorage — контракт трейта единый).
    #[test]
    fn mirror_missing_is_not_found() {
        let store = MirrorStore::default();
        let err = store
            .load_text(Path::new("нет.canvas"))
            .expect_err("нет файла — ошибка");
        assert!(matches!(err, CoreError::Io(ref e) if e.kind() == std::io::ErrorKind::NotFound));
    }

    /// OpfsStorage как CanvasStorage (натив-путь): save обновляет зеркало,
    /// load читает обратно; очередь сливается сразу (натив — debug-лог,
    /// wasm — фоновая запись), поэтому после save она пуста — содержимое
    /// пары (bak + файл) проверено в mirror_save_queues_bak_and_file.
    #[test]
    fn opfs_storage_trait_contract() {
        let storage = OpfsStorage::new();
        let path = Path::new("трейт.canvas");
        assert!(storage.load(path).is_err(), "пустое хранилище — ошибка");

        storage
            .save(&canvas_with("первая"), path)
            .expect("первый save");
        assert_eq!(storage.queued(), 0, "очередь слита в drain()");

        let loaded = storage.load(path).expect("roundtrip");
        assert_eq!(
            loaded.extra.get("name").and_then(|v| v.as_str()),
            Some("первая")
        );
        // .bak-версия в зеркале? Нет: bak живёт в очереди OPFS-записей —
        // зеркало держит только актуальный текст (контракт load).
        assert_eq!(
            storage.lock().mirror.get(Path::new("трейт.canvas.bak")),
            None
        );
    }
}
