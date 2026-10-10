//! FR-103 (мультиканвас C0): контракт хранилища рабочего стола канвасов
//! — `WorkspaceStore` (план v2.1 §3.1). Реализации вольнах C1
//! (`OpfsStore`: JS-глю OPFS) и C2 (`FsAccessStore`: granted-папка FS
//! Access) — C0 замораживает сигнатуры и семантику, чтобы воркстримы
//! работали параллельно.
//!
//! ## Паттерн «зеркало + конвейер событий»
//!
//! Сигнатуры синхронные (как у `CanvasStorage` в ядре — W3 wasm-port):
//! браузерные async-оперы живут ВНУТРИ реализаций — зеркало «имя → запись»
//! наполняется при инициализации и после каждой мутации фоновым таском
//! (прецедент `MirrorStore` в `opfs.rs`), запросы UI идут через AppEvent
//! (`RequestCanvasList` → `CanvasList`, план §3.2): событие просит
//! обновление зеркала, свежий список приезжает ответом.
//!
//! ## Замороженная семантика операций
//!
//! - `list` — только белый список `.canvas` (`.bak`-близнецы скрыты,
//!   инвариант §5.2 плана); порядок — порядок хранилища, сортировка —
//!   сторона UI (`workspace::sorted_entries`);
//! - `create` — сеет пустой `Canvas::default()` под данным именем; отказы:
//!   коллизия/лимит [`MAX_CANVASES`]/невалидное имя (расширение `.canvas`
//!   обязательно — иначе имя невидимо для листинга);
//! - `rename` — атомарно переносит файл И `.bak`-близнец; ключ камеры
//!   (`workspace::camera_key_for`, №12/№30b) переносит вызывающий слой;
//! - `delete` — мягкое удаление (№15a): файл → `<name>.bak` (замещает
//!   прежний `.bak`), из листинга исчезает; undo восстанавливает из `.bak`;
//! - `exists` — по зеркалу, без I/O.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::{Mutex, MutexGuard};

use canvas_core::workspace::{CanvasEntry, EntryKind, MAX_CANVASES};
use canvas_core::{Canvas, CanvasStorage, CoreError};

/// Отказ операции хранилища рабочего стола (контракт C0; реализации C1/C2
/// отображают платформенные ошибки в эти варианты — человекочитаемая
/// диагностика F-14 PRD-0005 ложится поверх `Display`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceError {
    /// Операция над несуществующим файлом (удалён из другой вкладки).
    NotFound(String),
    /// Имя уже занято (регистронезависимо — семантика Windows-папок).
    NameTaken(String),
    /// Имя не проходит контракт: без расширения `.canvas` или пустое.
    NameInvalid(String),
    /// Достигнут санити-лимит [`MAX_CANVASES`] (№16).
    LimitReached,
    /// Granted-папка недоступна (потеря доступа, №44b — баннер).
    AccessLost(String),
    /// Платформенная ошибка I/O (OPFS/FS Access).
    Io(String),
}

impl fmt::Display for WorkspaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(name) => write!(f, "канвас не найден: {name}"),
            Self::NameTaken(name) => write!(f, "имя уже занято: {name}"),
            Self::NameInvalid(name) => write!(f, "недопустимое имя: {name}"),
            Self::LimitReached => write!(f, "достигнут лимит канвасов ({MAX_CANVASES})"),
            Self::AccessLost(detail) => write!(f, "папка недоступна: {detail}"),
            Self::Io(detail) => write!(f, "ошибка хранилища: {detail}"),
        }
    }
}

impl std::error::Error for WorkspaceError {}

/// Хранилище рабочего стола канвасов (план v2.1 §3.1). Реализации:
/// `OpfsStore` (C1), `FsAccessStore` (C2); контрактные тесты — на
/// [`MemWorkspaceStore`] (эталон семантики, ниже).
pub trait WorkspaceStore {
    /// Список записей (зеркало; `.bak` скрыты белым списком `.canvas`).
    fn list(&self) -> Vec<CanvasEntry>;
    /// Создать пустой канвас с данным именем файла (`x.canvas`).
    fn create(&self, name: &str) -> Result<(), WorkspaceError>;
    /// Переименовать: файл + `.bak`-близнец; ключ камеры — сторона вызова.
    fn rename(&self, old: &str, new: &str) -> Result<(), WorkspaceError>;
    /// Мягкое удаление (№15a): файл → `<name>.bak`, из листинга исчезает.
    fn delete(&self, name: &str) -> Result<(), WorkspaceError>;
    /// Есть ли файл (по зеркалу, без I/O).
    fn exists(&self, name: &str) -> bool;
}

// ============================================================================
// Тест-двойник: память как хранилище (эталон семантики для C1/C2)
// ============================================================================

/// Запись хранилища: текст + время последнего изменения.
#[derive(Debug, Clone)]
struct MemFile {
    text: String,
    ts: u64,
}

/// Внутреннее состояние (Mutex: трейт берёт `&self` — как `OpfsStorage`;
/// отравленный лок не роняет хранилище — паттерн ядра).
#[derive(Debug, Default)]
struct MemInner {
    files: BTreeMap<String, MemFile>,
    /// Монотонный источник ts (детерминизм тестов).
    clock: u64,
}

/// Тест-двойник `WorkspaceStore` (C0): BTreeMap в памяти. Платформенно
/// нейтрален — нативные контрактные тесты без браузера; для C1/C2 служит
/// эталоном семантики (их тесты зеркалят эти кейсы). Играет роль OPFS.
#[derive(Debug, Default)]
pub struct MemWorkspaceStore {
    inner: Mutex<MemInner>,
}

impl MemWorkspaceStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Текст файла (проверки миграции/копирования в тестах C1/C2).
    pub fn text(&self, name: &str) -> Option<String> {
        self.lock().files.get(name).map(|file| file.text.clone())
    }

    /// Записать файл напрямую (сеяние тестовой обстановки).
    pub fn seed(&self, name: &str, text: &str) {
        let mut inner = self.lock();
        inner.clock += 1;
        let ts = inner.clock;
        inner.files.insert(
            name.to_owned(),
            MemFile {
                text: text.to_owned(),
                ts,
            },
        );
    }

    /// Лок зеркала; отравление (паника внутри) не роняет хранилище.
    fn lock(&self) -> MutexGuard<'_, MemInner> {
        self.inner.lock().unwrap_or_else(|err| err.into_inner())
    }
}

impl WorkspaceStore for MemWorkspaceStore {
    fn list(&self) -> Vec<CanvasEntry> {
        self.lock()
            .files
            .iter()
            .filter(|(name, _)| name.ends_with(".canvas"))
            .map(|(name, file)| CanvasEntry {
                name: name.clone(),
                ts: file.ts,
                kind: EntryKind::Opfs,
                repo: None,
            })
            .collect()
    }

    fn create(&self, name: &str) -> Result<(), WorkspaceError> {
        let mut inner = self.lock();
        if name.is_empty() || !name.ends_with(".canvas") {
            return Err(WorkspaceError::NameInvalid(name.to_owned()));
        }
        let lower = name.to_lowercase();
        if inner
            .files
            .keys()
            .any(|existing| existing.to_lowercase() == lower)
        {
            return Err(WorkspaceError::NameTaken(name.to_owned()));
        }
        if inner.files.len() >= MAX_CANVASES {
            return Err(WorkspaceError::LimitReached);
        }
        inner.clock += 1;
        let ts = inner.clock;
        inner.files.insert(
            name.to_owned(),
            MemFile {
                text: empty_canvas_text(),
                ts,
            },
        );
        Ok(())
    }

    fn rename(&self, old: &str, new: &str) -> Result<(), WorkspaceError> {
        if new.is_empty() || !new.ends_with(".canvas") {
            return Err(WorkspaceError::NameInvalid(new.to_owned()));
        }
        let mut inner = self.lock();
        if old == new && inner.files.contains_key(old) {
            return Ok(()); // идемпотентный no-op
        }
        let lower = new.to_lowercase();
        if inner
            .files
            .keys()
            .any(|existing| existing.to_lowercase() == lower)
        {
            return Err(WorkspaceError::NameTaken(new.to_owned()));
        }
        let file = inner
            .files
            .remove(old)
            .ok_or_else(|| WorkspaceError::NotFound(old.to_owned()))?;
        inner.clock += 1;
        let ts = inner.clock;
        inner.files.insert(
            new.to_owned(),
            MemFile {
                text: file.text,
                ts,
            },
        );
        // .bak-близнец переезжает вместе с файлом (контракт rename).
        if let Some(bak) = inner.files.remove(&format!("{old}.bak")) {
            inner.files.insert(format!("{new}.bak"), bak);
        }
        Ok(())
    }

    fn delete(&self, name: &str) -> Result<(), WorkspaceError> {
        let mut inner = self.lock();
        let file = inner
            .files
            .remove(name)
            .ok_or_else(|| WorkspaceError::NotFound(name.to_owned()))?;
        // Мягкое удаление (№15a): содержимое — в .bak (замещает прежний).
        inner.files.insert(
            format!("{name}.bak"),
            MemFile {
                text: file.text,
                ts: file.ts,
            },
        );
        Ok(())
    }

    fn exists(&self, name: &str) -> bool {
        self.lock().files.contains_key(name)
    }
}

/// Пустой канвас как текст (сеяние `create`): сериализация `Canvas::default`.
/// FR-104 (C1): `pub(crate)` — сеяние свежих канвасов переиспользует
/// OpfsStore (`create`) и фолбэк Web Locks (`web_requests`).
pub(crate) fn empty_canvas_text() -> String {
    Canvas::default()
        .to_json()
        .unwrap_or_else(|_| "{}".to_owned())
}

/// Хранилище как `CanvasStorage` — автосейв активного канваса в тестах
/// C1/C2 (трейт ядра работает с путями; берётся только имя файла).
impl CanvasStorage for MemWorkspaceStore {
    fn load(&self, path: &Path) -> Result<Canvas, CoreError> {
        let name = file_name_of(path)?;
        let text = self
            .lock()
            .files
            .get(name)
            .map(|file| file.text.clone())
            .ok_or_else(|| {
                CoreError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("файл не найден: {name}"),
                ))
            })?;
        Canvas::from_str(&text)
    }

    fn save(&self, canvas: &Canvas, path: &Path) -> Result<(), CoreError> {
        let name = file_name_of(path)?;
        let text = canvas.to_json()?;
        let mut inner = self.lock();
        // Обновление записи без .bak-очереди: это живой автосейв канваса
        // (мягкое удаление — отдельная операция delete).
        inner.clock += 1;
        let ts = inner.clock;
        inner.files.insert(name.to_owned(), MemFile { text, ts });
        Ok(())
    }
}

/// Имя файла из пути (хранилище плоское — как OPFS, см. `opfs_name`).
fn file_name_of(path: &Path) -> Result<&str, CoreError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| CoreError::Io(std::io::Error::other("нет имени файла")))
}

// ============================================================================
// Контрактные тесты трейта (нативные — эталон для OpfsStore/FsAccessStore)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_list_exists_roundtrip() {
        let store = MemWorkspaceStore::new();
        store.create("first.canvas").expect("создание");
        store.create("второй.canvas").expect("создание");
        assert!(store.exists("first.canvas"));
        assert!(!store.exists("ghost.canvas"));

        let mut names: Vec<_> = store.list().into_iter().map(|e| e.name).collect();
        names.sort();
        assert_eq!(names, ["first.canvas", "второй.canvas"]);
        // create сеет пустой канвас: текст — валидный .canvas
        let canvas = Canvas::from_str(&store.text("first.canvas").unwrap()).expect("валидный JSON");
        assert_eq!(canvas, Canvas::default());
    }

    #[test]
    fn create_rejects_collisions_invalid_names_and_limit() {
        let store = MemWorkspaceStore::new();
        store.create("x.canvas").expect("создание");
        assert_eq!(
            store.create("x.canvas"),
            Err(WorkspaceError::NameTaken("x.canvas".into())),
            "коллизия"
        );
        assert_eq!(
            store.create("без-расширения"),
            Err(WorkspaceError::NameInvalid("без-расширения".into()))
        );
        assert_eq!(
            store.create(""),
            Err(WorkspaceError::NameInvalid("".into()))
        );
        // расширение строго `.canvas`: иной регистр расширения невидим
        // для белого списка листинга — так же NameInvalid
        assert_eq!(
            store.create("X.CANVAS"),
            Err(WorkspaceError::NameInvalid("X.CANVAS".into()))
        );
        // но имя в верхнем регистре с точным расширением — коллизия
        assert_eq!(
            store.create("X.canvas"),
            Err(WorkspaceError::NameTaken("X.canvas".into())),
            "регистронезависимая коллизия имён"
        );
        // лимит: MAX_CANVASES записей — следующая отказывает (№16)
        let full = MemWorkspaceStore::new();
        for i in 0..MAX_CANVASES {
            full.create(&format!("c{i:05}.canvas"))
                .expect("в пределах лимита");
        }
        assert_eq!(
            full.create("overflow.canvas"),
            Err(WorkspaceError::LimitReached)
        );
    }

    #[test]
    fn rename_moves_file_and_bak_twin() {
        let store = MemWorkspaceStore::new();
        store.create("a.canvas").expect("создание");
        store.seed("a.canvas.bak", "старая версия");
        store.rename("a.canvas", "b.canvas").expect("ренейм");
        assert!(!store.exists("a.canvas"));
        assert!(store.exists("b.canvas"));
        assert_eq!(
            store.text("b.canvas.bak").as_deref(),
            Some("старая версия"),
            ".bak-близнец переехал вместе с файлом"
        );
        assert!(!store.exists("a.canvas.bak"));
        // ренейм в занятое / несуществующее
        store.create("c.canvas").expect("создание");
        assert_eq!(
            store.rename("b.canvas", "c.canvas"),
            Err(WorkspaceError::NameTaken("c.canvas".into()))
        );
        assert_eq!(
            store.rename("ghost.canvas", "d.canvas"),
            Err(WorkspaceError::NotFound("ghost.canvas".into()))
        );
        // идемпотентный no-op тем же именем
        assert_eq!(store.rename("b.canvas", "b.canvas"), Ok(()));
    }

    #[test]
    fn delete_is_soft_and_hidden_from_listing() {
        let store = MemWorkspaceStore::new();
        store.create("gone.canvas").expect("создание");
        store.seed("gone.canvas", "содержимое до удаления");
        store.delete("gone.canvas").expect("удаление");
        assert!(!store.exists("gone.canvas"));
        assert_eq!(
            store.text("gone.canvas.bak").as_deref(),
            Some("содержимое до удаления"),
            "мягкое удаление: содержимое в .bak (№15a)"
        );
        // .bak скрыт из листинга белым списком .canvas
        assert!(store.list().is_empty());
        // повторное удаление уже удалённого
        assert_eq!(
            store.delete("gone.canvas"),
            Err(WorkspaceError::NotFound("gone.canvas".into()))
        );
        // повторное создание того же имени допустимо (бак не мешает)
        store
            .create("gone.canvas")
            .expect("создание после удаления");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn canvas_storage_save_updates_mirror() {
        let store = MemWorkspaceStore::new();
        store.create("live.canvas").expect("создание");
        let mut canvas = Canvas::default();
        canvas.extra.insert("marker".into(), serde_json::json!(42));
        store
            .save(&canvas, Path::new("live.canvas"))
            .expect("автосейв");
        let loaded = store
            .load(Path::new("live.canvas"))
            .expect("чтение после сейва");
        assert_eq!(loaded.extra.get("marker"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn error_display_is_human_readable() {
        assert_eq!(
            WorkspaceError::NameTaken("x.canvas".into()).to_string(),
            "имя уже занято: x.canvas"
        );
        assert_eq!(
            WorkspaceError::LimitReached.to_string(),
            format!("достигнут лимит канвасов ({MAX_CANVASES})")
        );
    }
}
