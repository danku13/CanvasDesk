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

/// Тема интерфейса: тёмная (по умолчанию) или светлая (панель настроек).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Тёмная (базовая, SPEC T1: фон #1e1e22).
    #[default]
    Dark,
    /// Светлая.
    Light,
}

impl Theme {
    /// Переключение темы (строка панели настроек).
    pub fn next(self) -> Self {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }

    /// Подпись темы в панели настроек.
    pub fn label(self) -> &'static str {
        match self {
            Theme::Dark => "тёмная",
            Theme::Light => "светлая",
        }
    }
}

/// Вид сетки канваса: линии или точки (панель настроек).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GridStyle {
    /// Линии (классическая сетка).
    #[default]
    Lines,
    /// Точки в узлах мелкой сетки.
    Dots,
}

impl GridStyle {
    /// Переключение вида (строка панели настроек).
    pub fn next(self) -> Self {
        match self {
            GridStyle::Lines => GridStyle::Dots,
            GridStyle::Dots => GridStyle::Lines,
        }
    }

    /// Подпись вида в панели настроек.
    pub fn label(self) -> &'static str {
        match self {
            GridStyle::Lines => "линии",
            GridStyle::Dots => "точки",
        }
    }
}

/// Плотность сетки: шаг линий/точек относительно базового (20/100 world-px).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GridDensity {
    /// Частая: шаги 10/50 world-px.
    Dense,
    /// Средняя: шаги 20/100 world-px (базовая, SPEC T2).
    #[default]
    Medium,
    /// Редкая: шаги 40/200 world-px.
    Sparse,
}

impl GridDensity {
    /// Шаги (мелкий, крупный) в world-пикселях.
    pub fn steps(self) -> (f32, f32) {
        match self {
            GridDensity::Dense => (10.0, 50.0),
            GridDensity::Medium => (20.0, 100.0),
            GridDensity::Sparse => (40.0, 200.0),
        }
    }

    /// Переключение плотности по циклу (строка панели настроек).
    pub fn next(self) -> Self {
        match self {
            GridDensity::Dense => GridDensity::Medium,
            GridDensity::Medium => GridDensity::Sparse,
            GridDensity::Sparse => GridDensity::Dense,
        }
    }

    /// Подпись плотности в панели настроек.
    pub fn label(self) -> &'static str {
        match self {
            GridDensity::Dense => "частая",
            GridDensity::Medium => "средняя",
            GridDensity::Sparse => "редкая",
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
    /// Вид сетки: линии или точки.
    pub grid_style: GridStyle,
    /// Плотность сетки (шаг линий/точек).
    pub grid_density: GridDensity,
    /// Тема интерфейса.
    pub theme: Theme,
    /// HUD (fps/p95, F3) включён сразу при старте.
    pub hud_on_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            button_corner: Corner::TopRight,
            grid_visible: true,
            grid_style: GridStyle::Lines,
            grid_density: GridDensity::Medium,
            theme: Theme::Dark,
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
            grid_style: GridStyle::Dots,
            grid_density: GridDensity::Sparse,
            theme: Theme::Light,
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
        assert_eq!(settings.grid_style, GridStyle::Lines);
        assert_eq!(settings.grid_density, GridDensity::Medium);
        assert_eq!(settings.theme, Theme::Dark);
        assert!(!settings.hud_on_start);
        assert!(warn.is_none());
    }

    /// Тема: переключение замкнуто, подписи непустые, дефолт — тёмная.
    #[test]
    fn theme_cycle_and_labels() {
        assert_eq!(Theme::Dark.next(), Theme::Light);
        assert_eq!(Theme::Light.next(), Theme::Dark);
        assert_eq!(Settings::default().theme, Theme::Dark);
        for theme in [Theme::Dark, Theme::Light] {
            assert!(!theme.label().is_empty());
        }
    }

    /// Вид сетки: переключение замкнуто, подписи непустые.
    #[test]
    fn grid_style_cycle_and_labels() {
        assert_eq!(GridStyle::Lines.next(), GridStyle::Dots);
        assert_eq!(GridStyle::Dots.next(), GridStyle::Lines);
        for style in [GridStyle::Lines, GridStyle::Dots] {
            assert!(!style.label().is_empty());
        }
    }

    /// Плотность сетки: шаги мелкой/крупной линий, замкнутый цикл, подписи.
    #[test]
    fn grid_density_steps_cycle() {
        // Базовая (SPEC T2) — средняя
        assert_eq!(GridDensity::Medium.steps(), (20.0, 100.0));
        assert_eq!(GridDensity::Dense.steps(), (10.0, 50.0));
        assert_eq!(GridDensity::Sparse.steps(), (40.0, 200.0));
        // Цикл из 3 без повторов
        let start = GridDensity::Dense;
        let mut density = start;
        let mut seen = vec![density];
        for _ in 0..2 {
            density = density.next();
            assert!(!seen.contains(&density));
            seen.push(density);
        }
        assert_eq!(density.next(), start);
        for density in seen {
            assert!(!density.label().is_empty());
        }
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
        assert!(text.contains("grid_style"), "{text}");
        assert!(text.contains("grid_density"), "{text}");
        assert!(text.contains("theme"), "{text}");
        assert!(text.contains("hud_on_start"), "{text}");
    }
}
