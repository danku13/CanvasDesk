//! Ввод-вывод `.canvas` файлов (JSON Canvas 1.0, SPEC §5.1).
//!
//! Автосейв с debounce и `.bak` — задача T4; здесь — простые parse/serialize/load/save.
//! M8/W3 (wasm-port §6): трейт [`CanvasStorage`] — хранилище как сервис
//! (нативно — [`FsCanvasStorage`] = сегодняшнее поведение load + save с
//! `.bak`; web — FS Access/OPFS, W6; тесты — [`MemStorage`]).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;

use crate::error::CoreError;
use crate::model::Canvas;

impl FromStr for Canvas {
    type Err = CoreError;

    /// Распарсить содержимое `.canvas`-файла.
    /// Ошибка содержит позицию (строка/колонка) и причину.
    fn from_str(source: &str) -> Result<Self, CoreError> {
        serde_json::from_str(source).map_err(CoreError::from_parse_error)
    }
}

impl Canvas {
    /// Сериализовать в JSON (pretty, 2 пробела).
    pub fn to_json(&self) -> Result<String, CoreError> {
        let pretty = serde_json::ser::PrettyFormatter::with_indent(b"  ");
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, pretty);
        serde::Serialize::serialize(self, &mut serializer).map_err(CoreError::from_parse_error)?;
        String::from_utf8(buffer).map_err(|err| CoreError::Parse {
            line: 0,
            column: 0,
            message: format!("невалидный UTF-8 при сериализации: {err}"),
        })
    }

    /// Загрузить канвас из файла.
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        let source = std::fs::read_to_string(path)?;
        Self::from_str(&source)
    }

    /// Сохранить канвас в файл (простая запись; автосейв/.bak — T4).
    pub fn save(&self, path: &Path) -> Result<(), CoreError> {
        std::fs::write(path, self.to_json()?)?;
        Ok(())
    }

    /// Сохранить с бэкапом: прежняя версия переименовывается в `<name>.canvas.bak` (SPEC §9).
    pub fn save_with_backup(&self, path: &Path) -> Result<(), CoreError> {
        let json = self.to_json()?;
        if path.exists() {
            let backup = path.with_extension("canvas.bak");
            std::fs::rename(path, &backup)?;
        }
        std::fs::write(path, json)?;
        Ok(())
    }
}

/// Хранилище `.canvas`-файлов как сервис (M8/W3, wasm-port §3.2/§6):
/// нативно — файлы на диске с `.bak` (сегодняшнее поведение), web —
/// FS Access API/OPFS (W6). Контракт `save` включает бэкап прежней
/// версии (нативная семантика SPEC §9); реализации без диска решают
/// сами, чем её заменить.
pub trait CanvasStorage: Send + Sync {
    /// Загрузить канвас по пути (ключу хранилища).
    fn load(&self, path: &Path) -> Result<Canvas, CoreError>;
    /// Сохранить канвас (нативно — с `.bak` прежней версии).
    fn save(&self, canvas: &Canvas, path: &Path) -> Result<(), CoreError>;
}

/// Файловое хранилище — нативная реализация «как сегодня»: `Canvas::load`
/// + `save_with_backup` (тот же код io.rs, обёрнутый в трейт).
pub struct FsCanvasStorage;

impl CanvasStorage for FsCanvasStorage {
    fn load(&self, path: &Path) -> Result<Canvas, CoreError> {
        Canvas::load(path)
    }

    fn save(&self, canvas: &Canvas, path: &Path) -> Result<(), CoreError> {
        canvas.save_with_backup(path)
    }
}

/// Хранилище в памяти — заглушка для тестов (паттерн NoopThumbnailProvider):
/// ключ пути → JSON-текст канваса. Без `.bak` (контракт бэкапа —
/// дисковая семантика; заглушка проверяет только roundtrip).
pub struct MemStorage {
    files: Mutex<BTreeMap<PathBuf, String>>,
}

impl MemStorage {
    pub fn new() -> Self {
        Self {
            files: Mutex::new(BTreeMap::new()),
        }
    }

    /// Число сохранённых файлов (тесты).
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Пусто ли хранилище (тесты).
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// Отравленный Mutex не роняет хранилище: данные восстанавливаем.
    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<PathBuf, String>> {
        self.files.lock().unwrap_or_else(|err| err.into_inner())
    }
}

impl Default for MemStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasStorage for MemStorage {
    fn load(&self, path: &Path) -> Result<Canvas, CoreError> {
        let text = self.lock().get(path).cloned().ok_or_else(|| {
            CoreError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("MemStorage: файл не найден: {}", path.display()),
            ))
        })?;
        Canvas::from_str(&text)
    }

    fn save(&self, canvas: &Canvas, path: &Path) -> Result<(), CoreError> {
        let json = canvas.to_json()?;
        self.lock().insert(path.to_path_buf(), json);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FsCanvasStorage = сегодняшний натив: save пишет файл + `.bak`
    /// прежней версии, load читает обратно. Натив-only: wasip1-гейт не
    /// даёт записываемый temp_dir (FS-хранилище под wasm не используется —
    /// W6 подставит FS Access/OPFS).
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn fs_storage_roundtrip_and_bak() {
        let dir = std::env::temp_dir().join(format!("canvasdesk-w3-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let path = dir.join("roundtrip.canvas");

        let storage = FsCanvasStorage;
        let mut first = Canvas::default();
        first
            .extra
            .insert("name".into(), serde_json::json!("первая версия"));
        storage.save(&first, &path).expect("первая запись");

        let mut second = Canvas::default();
        second
            .extra
            .insert("name".into(), serde_json::json!("вторая версия"));
        storage.save(&second, &path).expect("вторая запись");

        let bak = path.with_extension("canvas.bak");
        assert!(bak.exists(), ".bak прежней версии создан");
        assert_eq!(
            Canvas::load(&bak)
                .expect("bak читается")
                .extra
                .get("name")
                .and_then(|v| v.as_str()),
            Some("первая версия")
        );
        assert_eq!(
            storage
                .load(&path)
                .expect("roundtrip")
                .extra
                .get("name")
                .and_then(|v| v.as_str()),
            Some("вторая версия")
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// MemStorage: roundtrip load/save, отсутствие файла — NotFound.
    #[test]
    fn mem_storage_roundtrip_and_missing() {
        let storage = MemStorage::new();
        let path = PathBuf::from("mem://test.canvas");
        assert!(storage.is_empty());
        assert!(storage.load(&path).is_err(), "нет файла — ошибка");

        let canvas = Canvas::default();
        storage.save(&canvas, &path).expect("save");
        assert_eq!(storage.len(), 1);
        let loaded = storage.load(&path).expect("load");
        assert_eq!(
            loaded.to_json().expect("json"),
            canvas.to_json().expect("json")
        );
    }
}
