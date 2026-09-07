//! Настройки приложения (пост-T7): `config.toml`, загружаемый при старте.
//!
//! Путь файла резолвит приложение (canvas-shell); здесь — только serde-схема
//! и чистые load/save. Расширение схемы — новые поля с `#[serde(default)]`:
//! старые конфиги не ломаются.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Угол экрана для летающей кнопки настроек.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Corner {
    TopLeft,
    #[default]
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    /// Следующий угол по циклу (переключение строкой в панели настроек).
    pub fn next(self) -> Self {
        match self {
            Corner::TopLeft => Corner::TopRight,
            Corner::TopRight => Corner::BottomRight,
            Corner::BottomRight => Corner::BottomLeft,
            Corner::BottomLeft => Corner::TopLeft,
        }
    }

    /// Подпись угла в панели настроек.
    pub fn label(self) -> &'static str {
        match self {
            Corner::TopLeft => "верхний левый",
            Corner::TopRight => "верхний правый",
            Corner::BottomLeft => "нижний левый",
            Corner::BottomRight => "нижний правый",
        }
    }
}

/// Настройки приложения. Дефолты — через `Default`, десериализация
/// подставляет их для отсутствующих полей (`#[serde(default)]` на struct).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Угол летающей кнопки настроек.
    pub button_corner: Corner,
    /// Рисовать сетку канваса.
    pub grid_visible: bool,
    /// HUD (fps/p95, F3) включён сразу при старте.
    pub hud_on_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            button_corner: Corner::TopRight,
            grid_visible: true,
            hud_on_start: false,
        }
    }
}

impl Settings {
    /// Загрузить настройки; отсутствующий или битый файл — дефолты
    /// (ошибка разбора возвращается для лога, приложение не падает).
    pub fn load(path: &Path) -> (Self, Option<String>) {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return (Self::default(), None);
            }
            Err(err) => {
                return (Self::default(), Some(format!("чтение конфига: {err}")));
            }
        };
        match toml::from_str(&text) {
            Ok(settings) => (settings, None),
            Err(err) => (
                Self::default(),
                Some(format!("разбор конфига (используются дефолты): {err}")),
            ),
        }
    }

    /// Сохранить настройки (создаёт родительские каталоги).
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        std::fs::write(path, text)
    }

    /// Разбор из строки (тесты; логика общая с load).
    #[cfg(test)]
    fn load_toml_str(text: &str) -> (Self, Option<String>) {
        match toml::from_str(text) {
            Ok(settings) => (settings, None),
            Err(err) => (Self::default(), Some(err.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip: сохранённый конфиг читается обратно без потерь.
    #[test]
    fn toml_round_trip() {
        let settings = Settings {
            button_corner: Corner::BottomLeft,
            grid_visible: false,
            hud_on_start: true,
        };
        let dir = std::env::temp_dir().join("canvasdesk-settings-test");
        let path = dir.join("config.toml");
        settings.save(&path).expect("сохранение");
        let (loaded, warn) = Settings::load(&path);
        assert_eq!(loaded, settings);
        assert!(warn.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Отсутствующий/битый файл — дефолты, без паники; битый — с предупреждением.
    #[test]
    fn broken_or_missing_gives_defaults() {
        let dir = std::env::temp_dir().join("canvasdesk-settings-broken");
        let _ = std::fs::create_dir_all(&dir);
        let missing = dir.join("nope.toml");
        let (settings, warn) = Settings::load(&missing);
        assert_eq!(settings, Settings::default());
        assert!(warn.is_none(), "отсутствие файла — не ошибка");

        let broken = dir.join("broken.toml");
        std::fs::write(&broken, "button_corner = [это не toml").expect("запись");
        let (settings, warn) = Settings::load(&broken);
        assert_eq!(settings, Settings::default());
        assert!(warn.is_some(), "битый файл — дефолты + предупреждение");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Частичный конфиг: отсутствующие поля — дефолтами (расширяемость схемы).
    #[test]
    fn partial_config_uses_defaults() {
        let (settings, warn) = Settings::load_toml_str("grid_visible = false\n");
        assert!(!settings.grid_visible);
        assert_eq!(settings.button_corner, Corner::TopRight);
        assert!(!settings.hud_on_start);
        assert!(warn.is_none());
    }

    /// Цикл углов замкнут: 4 переключения возвращают в исходный.
    #[test]
    fn corner_cycle_is_closed() {
        let start = Corner::TopLeft;
        let mut corner = start;
        let mut seen = vec![corner];
        for _ in 0..3 {
            corner = corner.next();
            assert!(
                !seen.contains(&corner),
                "цикл не должен повторяться раньше 4"
            );
            seen.push(corner);
        }
        assert_eq!(corner.next(), start);
        for corner in seen {
            assert!(!corner.label().is_empty());
        }
    }

    /// Имена полей в TOML — snake_case (стабильный формат файла).
    #[test]
    fn field_names_snake_case() {
        let text = toml::to_string_pretty(&Settings {
            button_corner: Corner::BottomRight,
            ..Settings::default()
        })
        .expect("сериализация");
        assert!(text.contains("button_corner = \"bottom_right\""), "{text}");
        assert!(text.contains("grid_visible"), "{text}");
        assert!(text.contains("hud_on_start"), "{text}");
    }
}
