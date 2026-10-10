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
// FR-105 (мультиканвас C2): исполнитель миграции OPFS → granted-папка (№42a/№52a)
// ============================================================================

/// IO-шов исполнителя миграции для синхронного окружения (нативные
/// тесты; план — из `canvas_core::workspace::migration_plan`, C0).
/// wasm-раннер водит ту же машину состояний ([`MigrationDriver`]) по
/// async-шагам — порядок фаз один и тот же (см. ниже).
pub trait MigrationIo {
    /// Прочитать текст исходного файла (OPFS). `NotFound` — источник пропал.
    fn read_source(&mut self, name: &str) -> Result<String, WorkspaceError>;
    /// Записать текст в целевой файл (папка), create/replace.
    fn write_target(&mut self, name: &str, text: &str) -> Result<(), WorkspaceError>;
    /// Проверить, что целевой файл существует (проверка после копирования).
    fn target_has(&mut self, name: &str) -> Result<bool, WorkspaceError>;
    /// Удалить исходный файл (OPFS) — только после успешной проверки копии.
    fn remove_source(&mut self, name: &str) -> Result<(), WorkspaceError>;
}

/// Итог миграции: отказоустойчивость №52a — оригинал удаляется ТОЛЬКО
/// после успешного копирования и проверки цели; любой частичный сбой
/// оставляет данные в OPFS (переезд можно повторить, потерь нет).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MigrationReport {
    /// Успешно перенесённые пары (источник → итоговое имя в папке).
    pub moved: Vec<(String, String)>,
    /// Отказы копирования: (источник, цель, ошибка) — оригинал не тронут.
    pub failed: Vec<(String, String, WorkspaceError)>,
    /// Скопированы, но НЕ удалены (отказ проверки/удаления) — дубль живёт
    /// в обеих сторонах, повторный прогон идемпотентен (коллизия в цели
    /// решится авто-суффиксом нового плана).
    pub kept: Vec<(String, String)>,
    /// Выбранные, но отсутствующие в источнике (из плана №42a).
    pub missing: Vec<String>,
}

/// Шаг исполнителя миграции: возвращается [`MigrationDriver::step`],
/// исполнитель (натив-двойник или wasm-раннер) выполняет его ровно один
/// раз и сообщает результат back-методами (`*_complete`/`*_fail`).
/// Порядок фаз — ПОРЯДОК ВАЖЕН (№52a): сначала ВСЕ копирования, потом
/// проверка и удаление — частичный сбой не теряет данные.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationStep {
    /// Прочитать текст источника (ответ: `read_complete`/`copy_fail`).
    Read { src: String, dst: String },
    /// Записать текст в цель (ответ: `write_complete`/`copy_fail`).
    Write {
        src: String,
        dst: String,
        text: String,
    },
    /// Проверить наличие цели (ответ: `verify_complete`).
    Verify { src: String, dst: String },
    /// Удалить источник — только после Verify=true (ответ: `remove_complete`
    /// / `remove_fail`).
    Remove { src: String, dst: String },
    /// Все шаги исчерпаны — забрать [`MigrationDriver::report`].
    Done,
}

#[derive(Debug, PartialEq, Eq)]
enum MigrationStage {
    /// Фаза 1: копирование (Read → Write по каждой паре плана).
    Copy,
    /// Фазы 2–3: проверка цели + удаление подтверждённых оригиналов.
    Remove,
    Done,
}

/// Управляемая машина миграции: чистая (std-only, нативные тесты гоняют
/// её через [`execute_migration`]; wasm-раннер — теми же шагами по
/// async-операциям). Инвариант: источник удаляется только после
/// успешных Write И Verify; ВСЕ копирования — до первого удаления
/// (порядок фаз, №52a).
#[derive(Debug)]
pub struct MigrationDriver {
    /// Пары плана, ожидающие копирования (порядок сохранён).
    pairs: std::collections::VecDeque<(String, String)>,
    /// Пара в полёте (Read выдан, Write ещё нет).
    inflight: Option<(String, String)>,
    /// Прочитанный текст + пара в полёте (ждут шага Write; живут до
    /// ответа — повторный `step` переигрывает Write тем же текстом;
    /// пара хранится вместе с текстом: «текст без пары» невозможен по
    /// построению, без `expect` в production-пути).
    inflight_write: Option<((String, String), String)>,
    /// Успешно скопированные пары (кандидаты на удаление).
    copied: Vec<(String, String)>,
    /// Очередь проверки+удаления (заполняется после фазы копий).
    to_remove: std::collections::VecDeque<(String, String)>,
    /// Пара в фазе Verify/Remove.
    removing: Option<(String, String)>,
    /// Verify пары подтвердил цель — следующий шаг её Remove (живёт до
    /// ответа: идемпотентность `next` между `remove_complete`/`remove_fail`).
    removing_ready: bool,
    stage: MigrationStage,
    report: MigrationReport,
}

impl MigrationDriver {
    /// Новая машина по плану (порядок `copies` сохраняется).
    pub fn new(plan: &canvas_core::workspace::MigrationPlan) -> Self {
        Self {
            pairs: plan.copies.iter().cloned().collect(),
            inflight: None,
            inflight_write: None,
            copied: Vec::new(),
            to_remove: std::collections::VecDeque::new(),
            removing: None,
            removing_ready: false,
            stage: MigrationStage::Copy,
            report: MigrationReport {
                missing: plan.missing.clone(),
                ..Default::default()
            },
        }
    }

    /// Следующий шаг (идемпотентен между ответами: повторный вызов без
    /// `*_complete`/`*_fail` возвращает тот же шаг).
    pub fn step(&mut self) -> MigrationStep {
        loop {
            match self.stage {
                MigrationStage::Copy => {
                    // Текст прочитан — Write (текст живёт до ответа Write)
                    if let Some(((src, dst), text)) = self.inflight_write.clone() {
                        return MigrationStep::Write { src, dst, text };
                    }
                    // Read в полёте без ответа — переигрываем его
                    if let Some((src, dst)) = self.inflight.clone() {
                        return MigrationStep::Read { src, dst };
                    }
                    if let Some((src, dst)) = self.pairs.pop_front() {
                        self.inflight = Some((src.clone(), dst.clone()));
                        return MigrationStep::Read { src, dst };
                    }
                    // Фаза копий исчерпана → проверка/удаление скопированных
                    self.to_remove = std::mem::take(&mut self.copied).into();
                    self.stage = MigrationStage::Remove;
                }
                MigrationStage::Remove => {
                    // Пара в полёте: Verify без ответа — переигрываем Verify;
                    // подтверждённая (Verify=true) — её Remove
                    if let Some((src, dst)) = self.removing.clone() {
                        return if self.removing_ready {
                            MigrationStep::Remove { src, dst }
                        } else {
                            MigrationStep::Verify { src, dst }
                        };
                    }
                    if let Some((src, dst)) = self.to_remove.pop_front() {
                        self.removing = Some((src.clone(), dst.clone()));
                        self.removing_ready = false;
                        return MigrationStep::Verify { src, dst };
                    }
                    self.stage = MigrationStage::Done;
                }
                MigrationStage::Done => return MigrationStep::Done,
            }
        }
    }

    /// Источник прочитан (ответ на `Read`): текст присоединяется к паре
    /// в полёте (нет пары — лишний ответ, игнорируем).
    pub fn read_complete(&mut self, text: String) {
        if let Some(pair) = self.inflight.clone() {
            self.inflight_write = Some((pair, text));
        }
    }

    /// Копия записана в цель (ответ на `Write`).
    pub fn write_complete(&mut self) {
        if let Some(pair) = self.inflight.take() {
            self.inflight_write = None;
            self.copied.push(pair);
        }
    }

    /// Отказ копирования (ответ на `Read`/`Write`): пара — в `failed`,
    /// оригинал не тронут.
    pub fn copy_fail(&mut self, err: WorkspaceError) {
        if let Some((src, dst)) = self.inflight.take() {
            self.inflight_write = None;
            self.report.failed.push((src, dst, err));
        }
    }

    /// Ответ на `Verify`: `has=false` — пара уходит в `kept` (без
    /// удаления); `has=true` — пара подтверждена, следующий шаг — её Remove.
    pub fn verify_complete(&mut self, has: bool) {
        self.removing_ready = has;
        if !has {
            if let Some(pair) = self.removing.take() {
                self.report.kept.push(pair);
            }
        }
    }

    /// Оригинал удалён (ответ на `Remove`).
    pub fn remove_complete(&mut self) {
        self.removing_ready = false;
        if let Some(pair) = self.removing.take() {
            self.report.moved.push(pair);
        }
    }

    /// Отказ удаления ПОСЛЕ успешной копии (ответ на `Remove`): обе
    /// стороны живы (kept) — данные не теряются.
    pub fn remove_fail(&mut self) {
        self.removing_ready = false;
        if let Some(pair) = self.removing.take() {
            self.report.kept.push(pair);
        }
    }

    /// Итог (после `MigrationStep::Done`).
    pub fn report(self) -> MigrationReport {
        self.report
    }
}

/// Синхронный прогон машины по IO-двойнику (нативные тесты; wasm
/// использует [`MigrationDriver`] напрямую — тот же порядок шагов).
pub fn execute_migration(
    plan: &canvas_core::workspace::MigrationPlan,
    io: &mut dyn MigrationIo,
) -> MigrationReport {
    let mut driver = MigrationDriver::new(plan);
    loop {
        match driver.step() {
            MigrationStep::Read { src, .. } => match io.read_source(&src) {
                Ok(text) => driver.read_complete(text),
                Err(err) => driver.copy_fail(err),
            },
            MigrationStep::Write { dst, text, .. } => match io.write_target(&dst, &text) {
                Ok(()) => driver.write_complete(),
                Err(err) => driver.copy_fail(err),
            },
            MigrationStep::Verify { dst, .. } => {
                driver.verify_complete(io.target_has(&dst).unwrap_or(false));
            }
            MigrationStep::Remove { src, .. } => {
                if io.remove_source(&src).is_ok() {
                    driver.remove_complete();
                } else {
                    driver.remove_fail();
                }
            }
            MigrationStep::Done => break,
        }
    }
    driver.report()
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

    // --- FR-105: исполнитель миграции (№42a/№52a) --------------------------

    use canvas_core::workspace::migration_plan;

    /// Запись OPFS-листинга для плана (kind — источник = OPFS).
    fn entry(name: &str, ts: u64) -> canvas_core::workspace::CanvasEntry {
        canvas_core::workspace::CanvasEntry {
            name: name.to_owned(),
            ts,
            kind: canvas_core::workspace::EntryKind::Opfs,
            repo: None,
        }
    }

    /// Двойник миграционного IO: OPFS-текст и папка в памяти + счётчики
    /// отказов (инъекция сбоя на N-й операции конкретного вида) + журнал
    /// операций (проверка порядка фаз: все записи — до первого удаления).
    struct MemIo {
        opfs: BTreeMap<String, String>,
        folder: BTreeMap<String, String>,
        fail_read: Option<String>,
        fail_write: Option<String>,
        fail_remove: Option<String>,
        /// Порядок фактических IO-операций ("read:a", "write:b", …).
        log: Vec<String>,
    }

    impl MemIo {
        fn new(opfs: &[(&str, &str)]) -> Self {
            Self {
                opfs: opfs
                    .iter()
                    .map(|(n, t)| (n.to_string(), t.to_string()))
                    .collect(),
                folder: BTreeMap::new(),
                fail_read: None,
                fail_write: None,
                fail_remove: None,
                log: Vec::new(),
            }
        }
    }

    impl MigrationIo for MemIo {
        fn read_source(&mut self, name: &str) -> Result<String, WorkspaceError> {
            self.log.push(format!("read:{name}"));
            if self.fail_read.as_deref() == Some(name) {
                return Err(WorkspaceError::Io("read fail".into()));
            }
            self.opfs
                .get(name)
                .cloned()
                .ok_or_else(|| WorkspaceError::NotFound(name.to_owned()))
        }
        fn write_target(&mut self, name: &str, text: &str) -> Result<(), WorkspaceError> {
            self.log.push(format!("write:{name}"));
            if self.fail_write.as_deref() == Some(name) {
                return Err(WorkspaceError::Io("write fail".into()));
            }
            self.folder.insert(name.to_owned(), text.to_owned());
            Ok(())
        }
        fn target_has(&mut self, name: &str) -> Result<bool, WorkspaceError> {
            Ok(self.folder.contains_key(name))
        }
        fn remove_source(&mut self, name: &str) -> Result<(), WorkspaceError> {
            self.log.push(format!("remove:{name}"));
            if self.fail_remove.as_deref() == Some(name) {
                return Err(WorkspaceError::Io("remove fail".into()));
            }
            self.opfs
                .remove(name)
                .map(|_| ())
                .ok_or_else(|| WorkspaceError::NotFound(name.to_owned()))
        }
    }

    fn plan_of(source: &[CanvasEntry], selected: &[&str]) -> canvas_core::workspace::MigrationPlan {
        let selected: Vec<String> = selected.iter().map(|s| s.to_string()).collect();
        migration_plan(source, &selected, &[])
    }

    /// Счастливый путь: копирование → проверка → удаление оригиналов (№52a),
    /// содержимое приезжает в папку, OPFS пуст.
    #[test]
    fn migration_moves_all_and_empties_opfs() {
        let source = [entry("a.canvas", 1), entry("b.canvas", 2)];
        let plan = plan_of(&source, &["a.canvas", "b.canvas"]);
        let mut io = MemIo::new(&[("a.canvas", "текст A"), ("b.canvas", "текст B")]);
        let report = execute_migration(&plan, &mut io);
        assert_eq!(report.moved.len(), 2);
        assert!(report.failed.is_empty() && report.kept.is_empty());
        assert!(io.opfs.is_empty(), "OPFS пуст (чистый переезд)");
        assert_eq!(io.folder["a.canvas"], "текст A");
        assert_eq!(io.folder["b.canvas"], "текст B");
    }

    /// Частичный сбой копирования: отказавший файл ОСТАЁТСЯ в OPFS,
    /// остальные переезжают (данные не теряются — порядок фаз).
    #[test]
    fn migration_partial_write_failure_keeps_source() {
        let source = [entry("a.canvas", 1), entry("b.canvas", 2)];
        let plan = plan_of(&source, &["a.canvas", "b.canvas"]);
        let mut io = MemIo::new(&[("a.canvas", "A"), ("b.canvas", "B")]);
        io.fail_write = Some("b.canvas".into());
        let report = execute_migration(&plan, &mut io);
        assert_eq!(report.moved, [("a.canvas".into(), "a.canvas".into())]);
        assert_eq!(report.failed.len(), 1, "b не скопирован");
        assert_eq!(
            io.opfs.keys().collect::<Vec<_>>(),
            [&"b.canvas".to_string()],
            "оригинал b не тронут — повторный прогон доедет"
        );
        assert_eq!(io.folder.len(), 1, "только a в папке");
    }

    /// Отказ удаления ПОСЛЕ успешной копии: файл остаётся в обеих сторонах
    /// (kept), данные не теряются; удаление не предшествует проверке.
    #[test]
    fn migration_remove_failure_keeps_both_sides() {
        let source = [entry("a.canvas", 1)];
        let plan = plan_of(&source, &["a.canvas"]);
        let mut io = MemIo::new(&[("a.canvas", "A")]);
        io.fail_remove = Some("a.canvas".into());
        let report = execute_migration(&plan, &mut io);
        assert!(report.moved.is_empty());
        assert_eq!(report.kept, [("a.canvas".into(), "a.canvas".into())]);
        assert!(io.folder.contains_key("a.canvas"), "копия в папке есть");
        assert!(io.opfs.contains_key("a.canvas"), "оригинал тоже жив");
    }

    /// Порядок фаз (№52a): ВСЕ записи — до ПЕРВОГО удаления (журнал
    /// операций двойника). Сбой одной копии не блокирует остальные —
    /// но удаления начинаются только после исчерпания фазы копий.
    #[test]
    fn migration_writes_all_before_first_remove() {
        let source = [entry("a.canvas", 1), entry("b.canvas", 2)];
        let plan = plan_of(&source, &["a.canvas", "b.canvas"]);
        let mut io = MemIo::new(&[("a.canvas", "A"), ("b.canvas", "B")]);
        // Сбой чтения первого файла: b всё равно переезжает целиком
        io.fail_read = Some("a.canvas".into());
        let report = execute_migration(&plan, &mut io);
        assert_eq!(
            report.moved,
            [("b.canvas".to_owned(), "b.canvas".to_owned())],
            "b доезжает: сбой a не блокирует остальные пары"
        );
        assert!(report.kept.is_empty());
        assert_eq!(report.failed.len(), 1, "a не прочитан");
        assert_eq!(io.folder.len(), 1, "b скопирован");
        assert!(io.opfs.contains_key("a.canvas"), "оригинал a не тронут");
        assert!(
            io.log.iter().any(|op| op.starts_with("remove")),
            "b удалён после успешной копии (частичный переезд честен)"
        );
        // Счастливый прогон: все write строго до первого remove
        let mut io = MemIo::new(&[("a.canvas", "A"), ("b.canvas", "B")]);
        let _ = execute_migration(&plan, &mut io);
        let last_write = io
            .log
            .iter()
            .rposition(|op| op.starts_with("write"))
            .expect("были записи");
        let first_remove = io
            .log
            .iter()
            .position(|op| op.starts_with("remove"))
            .expect("были удаления");
        assert!(
            last_write < first_remove,
            "порядок фаз: копии до удалений (журнал: {:?})",
            io.log
        );
    }

    /// Коллизия в цели: план даёт авто-суффикс, исполнитель пишет под ним;
    /// отсутствующие в источнике попадают в missing без блокировки остальных.
    #[test]
    fn migration_suffixes_and_reports_missing() {
        let source = [entry("x.canvas", 1)];
        let target = [entry("x.canvas", 9)];
        let plan = migration_plan(
            &source,
            &["x.canvas".to_string(), "ghost.canvas".to_string()],
            &target,
        );
        let mut io = MemIo::new(&[("x.canvas", "X")]);
        io.folder.insert("x.canvas".into(), "старое".into());
        let report = execute_migration(&plan, &mut io);
        assert_eq!(
            report.moved,
            [("x.canvas".into(), "x (1).canvas".into())],
            "коллизия в цели — авто-суффикс плана (№26b)"
        );
        assert_eq!(report.missing, ["ghost.canvas".to_owned()]);
        assert_eq!(io.folder["x (1).canvas"], "X");
        assert_eq!(io.folder["x.canvas"], "старое", "цель не перезаписана");
    }

    /// Машина состояний (wasm-путь): шаги качаются по одному — порядок
    /// фаз тот же (копирование ВСЕХ пар до первого удаления), ответ
    /// read_complete/write_complete двигает пару по конвейеру.
    #[test]
    fn migration_driver_pumps_steps_like_wasm_runner() {
        let source = [entry("a.canvas", 1), entry("b.canvas", 2)];
        let plan = plan_of(&source, &["a.canvas", "b.canvas"]);
        let mut driver = MigrationDriver::new(&plan);
        // Пара a: Read → Write
        assert_eq!(
            driver.step(),
            MigrationStep::Read {
                src: "a.canvas".into(),
                dst: "a.canvas".into()
            }
        );
        driver.read_complete("A".into());
        assert_eq!(
            driver.step(),
            MigrationStep::Write {
                src: "a.canvas".into(),
                dst: "a.canvas".into(),
                text: "A".into()
            }
        );
        driver.write_complete();
        // Пара b идёт СТРОГО после a — удаления ещё не было (№52a)
        assert_eq!(
            driver.step(),
            MigrationStep::Read {
                src: "b.canvas".into(),
                dst: "b.canvas".into()
            }
        );
        driver.read_complete("B".into());
        assert_eq!(
            driver.step(),
            MigrationStep::Write {
                src: "b.canvas".into(),
                dst: "b.canvas".into(),
                text: "B".into()
            }
        );
        driver.write_complete();
        // Только теперь — проверка и удаление
        assert_eq!(
            driver.step(),
            MigrationStep::Verify {
                src: "a.canvas".into(),
                dst: "a.canvas".into()
            }
        );
        driver.verify_complete(true);
        assert_eq!(
            driver.step(),
            MigrationStep::Remove {
                src: "a.canvas".into(),
                dst: "a.canvas".into()
            }
        );
        driver.remove_complete();
        // verify=false у b — удаления не будет, kept
        assert_eq!(
            driver.step(),
            MigrationStep::Verify {
                src: "b.canvas".into(),
                dst: "b.canvas".into()
            }
        );
        driver.verify_complete(false);
        assert_eq!(driver.step(), MigrationStep::Done);
        let report = driver.report();
        assert_eq!(report.moved, [("a.canvas".into(), "a.canvas".into())]);
        assert_eq!(report.kept, [("b.canvas".into(), "b.canvas".into())]);
    }

    /// Машина терпит посторонние/дублированные ответы (контракт
    /// идемпотентности): `read_complete` без пары в полёте — игнор,
    /// дубль `write_complete`/`verify_complete` — no-op, шаги
    /// воспроизводятся корректно, отчёт не искажается.
    #[test]
    fn migration_driver_tolerates_orphan_and_duplicate_answers() {
        let source = [entry("a.canvas", 1)];
        let plan = plan_of(&source, &["a.canvas"]);
        let mut driver = MigrationDriver::new(&plan);
        // посторонний ответ до первого шага — пары в полёте нет
        driver.read_complete("лишний".into());
        assert_eq!(
            driver.step(),
            MigrationStep::Read {
                src: "a.canvas".into(),
                dst: "a.canvas".into()
            }
        );
        driver.read_complete("A".into());
        assert_eq!(
            driver.step(),
            MigrationStep::Write {
                src: "a.canvas".into(),
                dst: "a.canvas".into(),
                text: "A".into()
            }
        );
        driver.write_complete();
        driver.write_complete(); // дубль ответа Write — no-op
        assert_eq!(
            driver.step(),
            MigrationStep::Verify {
                src: "a.canvas".into(),
                dst: "a.canvas".into()
            }
        );
        driver.verify_complete(true);
        driver.verify_complete(true); // дубль Verify — окно идемпотентности
        assert_eq!(
            driver.step(),
            MigrationStep::Remove {
                src: "a.canvas".into(),
                dst: "a.canvas".into()
            }
        );
        driver.remove_complete();
        driver.remove_complete(); // дубль Remove — no-op
        assert_eq!(driver.step(), MigrationStep::Done);
        let report = driver.report();
        assert_eq!(report.moved, [("a.canvas".into(), "a.canvas".into())]);
        assert!(report.failed.is_empty() && report.kept.is_empty());
    }
}
