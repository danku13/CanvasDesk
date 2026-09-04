//! Ввод-вывод `.canvas` файлов (JSON Canvas 1.0, SPEC §5.1).
//!
//! Автосейв с debounce и `.bak` — задача T4; здесь — простые parse/serialize/load/save.

use std::path::Path;
use std::str::FromStr;

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
