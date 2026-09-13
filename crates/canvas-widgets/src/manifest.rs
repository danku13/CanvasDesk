//! Манифест `widget.json` (SPEC §7.6, план M5 §4.1): строгая схема,
//! неизвестные поля — warn, безопасные id/entry/размеры.

use crate::permissions::Permission;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::path::Path;

/// Ошибка валидации манифеста. Установка пакета с невалидным манифестом
/// отвергается целиком (T21-B), remote-виджеты невозможны архитектурно.
#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("манифест не является JSON-объектом")]
    NotAnObject,
    #[error("поле `{0}` отсутствует или пусто")]
    MissingField(&'static str),
    #[error("id `{0}` невалиден: 3..64 символа из [a-z0-9.-], без «..» и лидирующих точек")]
    InvalidId(String),
    #[error("entry `{0}` небезопасен: ожидается относительный путь внутри пакета без «..», схем и дисков")]
    UnsafeEntry(String),
    #[error("defaultSize [{0}x{1}] вне диапазона 160..2000")]
    BadSize(f32, f32),
    #[error("версия `{0}` невалидна: 1..32 символа")]
    BadVersion(String),
    #[error("некорректный JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Манифест виджета — строгая схема из SPEC §7.6.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    /// Относительный путь точки входа внутри пакета (например, `index.html`).
    pub entry: String,
    /// Размер ноды при создании, мировые px (клампится 160..2000).
    pub default_size: [f32; 2],
    pub permissions: Vec<Permission>,
    /// Неизвестные поля манифеста — не ошибка (forward compatibility),
    /// передаются в warn-лог установки (SPEC: «неизвестные поля — warn»).
    pub unknown_fields: Vec<String>,
}

/// Промежуточная serde-структура: поля строго типизированы, неизвестное —
/// во flatten-карту для warn-листа.
#[derive(Debug, Deserialize)]
struct RawManifest {
    id: String,
    name: String,
    version: String,
    entry: String,
    #[serde(rename = "defaultSize", default = "default_size_fallback")]
    default_size: [f32; 2],
    #[serde(default)]
    permissions: Vec<Permission>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

fn default_size_fallback() -> [f32; 2] {
    [320.0, 200.0]
}

impl WidgetManifest {
    /// Парсинг и валидация манифеста из JSON-текста.
    pub fn parse(json: &str) -> Result<Self, ManifestError> {
        let raw: RawManifest = serde_json::from_str(json)?;
        Self::validate(raw)
    }

    /// Парсинг манифеста `widget.json` из папки пакета.
    pub fn from_dir(dir: &Path) -> Result<Self, ManifestError> {
        let path = dir.join("widget.json");
        let json = std::fs::read_to_string(&path)
            .map_err(|_| ManifestError::MissingField("widget.json (файл не читается)"))?;
        Self::parse(&json)
    }

    fn validate(raw: RawManifest) -> Result<Self, ManifestError> {
        let unknown_fields: Vec<String> = raw.extra.keys().cloned().collect();

        if raw.id.trim().is_empty() {
            return Err(ManifestError::MissingField("id"));
        }
        if !valid_id(&raw.id) {
            return Err(ManifestError::InvalidId(raw.id));
        }
        if raw.name.trim().is_empty() {
            return Err(ManifestError::MissingField("name"));
        }
        if raw.version.trim().is_empty() || raw.version.len() > 32 {
            return Err(ManifestError::BadVersion(raw.version));
        }
        if !safe_entry(&raw.entry) {
            return Err(ManifestError::UnsafeEntry(raw.entry));
        }
        let [w, h] = raw.default_size;
        if !(160.0..=2000.0).contains(&w) || !(160.0..=2000.0).contains(&h) {
            return Err(ManifestError::BadSize(w, h));
        }

        Ok(Self {
            id: raw.id,
            name: raw.name,
            version: raw.version,
            entry: raw.entry,
            default_size: raw.default_size,
            permissions: raw.permissions,
            unknown_fields,
        })
    }

    /// Путь точки входа, безопасно присоединённый к корню пакета
    /// (`join` после валидации `safe_entry` — traversal исключён).
    pub fn entry_path(&self, package_dir: &Path) -> std::path::PathBuf {
        package_dir.join(self.entry.replace('\\', "/"))
    }

    /// Виртуальный hostname для origin пакета (SPEC §7.6,
    /// `SetVirtualHostNameToFolderMapping`): id без точек — легальный host.
    pub fn virtual_host(&self) -> String {
        self.id.replace('.', "-")
    }

    /// Версия в виде тройки чисел (semver-lite) для сравнения при обновлении;
    /// нераспарсиваемые суффиксы игнорируются, совсем неизвестное — [0,0,0].
    pub fn version_tuple(&self) -> [u32; 3] {
        let mut out = [0, 0, 0];
        for (slot, part) in self.version.split('.').take(3).enumerate() {
            let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
            out[slot] = digits.parse().unwrap_or(0);
        }
        out
    }
}

/// id: 3..64 символа `[a-z0-9.-]`, без `..`, без лидирующей/хвостовой точки.
fn valid_id(id: &str) -> bool {
    let len = id.len();
    if !(3..64).contains(&len) {
        return false;
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
    {
        return false;
    }
    if id.contains("..") || id.starts_with('.') || id.starts_with('-') || id.ends_with('.') {
        return false;
    }
    true
}

/// entry: относительный путь без схем, дисков, `..` и абсолютных приставок.
/// URL (remote-виджет) отвергается архитектурно (SPEC §7.6 «Безопасность»).
fn safe_entry(entry: &str) -> bool {
    if entry.is_empty() || entry.len() > 200 {
        return false;
    }
    let lower = entry.to_ascii_lowercase();
    if lower.contains("://") || lower.starts_with("//") {
        return false; // http://, https://, file://, ...
    }
    if lower.starts_with('/') || lower.starts_with('\\') {
        return false; // абсолютный путь
    }
    if entry.len() >= 2 && entry.as_bytes()[1] == b':' {
        return false; // диск C: (латиница любой буквы)
    }
    // Компоненты: без «..», без пустых (двойной слеш) — нормализация не нужна
    entry
        .replace('\\', "/")
        .split('/')
        .all(|part| !part.is_empty() && part != "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{
        "id": "com.example.clock", "name": "Clock", "version": "1.2.0",
        "entry": "index.html", "defaultSize": [320, 200],
        "permissions": ["fs:read", "network"]
    }"#;

    #[test]
    fn parse_good_manifest() {
        let m = WidgetManifest::parse(GOOD).expect("валидный манифест");
        assert_eq!(m.id, "com.example.clock");
        assert_eq!(m.name, "Clock");
        assert_eq!(m.version, "1.2.0");
        assert_eq!(m.entry, "index.html");
        assert_eq!(m.default_size, [320.0, 200.0]);
        assert_eq!(m.permissions, vec![Permission::FsRead, Permission::Network]);
        assert!(m.unknown_fields.is_empty());
        assert_eq!(m.virtual_host(), "com-example-clock");
        assert_eq!(m.version_tuple(), [1, 2, 0]);
    }

    #[test]
    fn defaults_without_size_and_permissions() {
        let m = WidgetManifest::parse(
            r#"{"id": "a.clock", "name": "C", "version": "1.0.0", "entry": "index.html"}"#,
        )
        .expect("defaultSize/permissions опциональны");
        assert_eq!(m.default_size, [320.0, 200.0]);
        assert!(m.permissions.is_empty());
    }

    #[test]
    fn unknown_fields_collected_not_rejected() {
        let m = WidgetManifest::parse(
            r#"{"id": "a.clock", "name": "C", "version": "1.0.0", "entry": "i.html",
                "futureFeature": true, "another": 1}"#,
        )
        .expect("неизвестные поля — warn, не ошибка");
        assert_eq!(
            m.unknown_fields,
            vec!["another".to_owned(), "futureFeature".to_owned()]
        );
    }

    #[test]
    fn rejects_remote_url_entry() {
        for entry in [
            "https://evil.example/index.html",
            "http://x.js",
            "file:///C:/x.html",
            "//host/x.html",
        ] {
            let json = format!(
                r#"{{"id": "a.clock", "name": "C", "version": "1.0.0", "entry": "{entry}"}}"#
            );
            let err = WidgetManifest::parse(&json).expect_err("URL отвергается");
            assert!(
                matches!(err, ManifestError::UnsafeEntry(_)),
                "{entry}: {err}"
            );
        }
    }

    #[test]
    fn rejects_traversal_and_absolute_entries() {
        for entry in [
            "../outside.html",
            "sub/../../x.html",
            "/abs.html",
            "\\abs.html",
            "C:/x.html",
            "",
        ] {
            let json = format!(
                r#"{{"id": "a.clock", "name": "C", "version": "1.0.0", "entry": "{entry}"}}"#
            );
            assert!(
                WidgetManifest::parse(&json).is_err(),
                "entry «{entry}» должен отвергаться"
            );
        }
        // Легальные варианты с подпапками и обратным слешем (JSON \\ = слеш)
        for entry in ["sub/index.html", "sub\\\\index.html", "index.html"] {
            let json = format!(
                r#"{{"id": "a.clock", "name": "C", "version": "1.0.0", "entry": "{entry}"}}"#
            );
            assert!(
                WidgetManifest::parse(&json).is_ok(),
                "entry «{entry}» легален"
            );
        }
    }

    #[test]
    fn rejects_bad_ids() {
        let long_id = format!("a.{}", "x".repeat(70));
        let ids: [&str; 7] = ["ab", "", "UPPER.CASE", "a..b", ".hidden", "-lead", &long_id];
        for id in ids {
            let json = format!(
                r#"{{"id": "{id}", "name": "C", "version": "1.0.0", "entry": "index.html"}}"#
            );
            assert!(
                WidgetManifest::parse(&json).is_err(),
                "id «{id}» должен отвергаться"
            );
        }
    }

    #[test]
    fn rejects_bad_sizes_and_versions() {
        let base = r#"{"id": "a.clock", "name": "C", "entry": "index.html""#;
        for (size, ok) in [
            ("[100, 200]", false),
            ("[160, 200]", true),
            ("[320, 9999]", false),
            ("[320, 0]", false),
        ] {
            let json = format!("{base}, \"version\": \"1.0.0\", \"defaultSize\": {size}}}");
            assert_eq!(WidgetManifest::parse(&json).is_ok(), ok, "size {size}");
        }
        for (version, ok) in [
            ("\"1.0.0\"", true),
            ("\"\"", false),
            ("\"0123456789012345678901234567890123\"", false),
        ] {
            let json = format!("{base}, \"version\": {version}, \"defaultSize\": [320, 200]}}");
            assert_eq!(
                WidgetManifest::parse(&json).is_ok(),
                ok,
                "version {version}"
            );
        }
    }

    #[test]
    fn missing_required_fields() {
        let err =
            WidgetManifest::parse(r#"{"id": "a.clock"}"#).expect_err("нет name/version/entry");
        assert!(matches!(err, ManifestError::Json(_)));
    }

    #[test]
    fn version_tuple_tolerates_suffixes() {
        let m = WidgetManifest::parse(
            r#"{"id": "a.clock", "name": "C", "version": "2.5.1-beta.3", "entry": "i.html"}"#,
        )
        .expect("суффикс версии не ломает парсинг");
        assert_eq!(m.version_tuple(), [2, 5, 1]);
        let m2 = WidgetManifest::parse(
            r#"{"id": "a.clock", "name": "C", "version": "weird", "entry": "i.html"}"#,
        )
        .expect("строка-версия допускается");
        assert_eq!(m2.version_tuple(), [0, 0, 0]);
    }

    #[test]
    fn entry_path_joins_normalized() {
        // JSON «sub\\index.html» = значение «sub\index.html» (обратный слеш)
        let m = WidgetManifest::parse(
            r#"{"id": "a.clock", "name": "C", "version": "1", "entry": "sub\\index.html"}"#,
        )
        .expect("легальный entry");
        assert_eq!(m.entry, "sub\\index.html");
        let joined = m.entry_path(Path::new("/widgets/a.clock"));
        assert_eq!(joined, Path::new("/widgets/a.clock/sub/index.html"));
    }
}
