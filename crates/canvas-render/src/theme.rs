//! Палитра темы интерфейса (тёмная/светлая): все тема-зависимые цвета
//! рендера в одной структуре. Значения — в sRGB 0..1 (так же, как
//! прежние константы в cards.rs/text.rs: GPU-пайплайны пишут их как есть
//! в sRGB-surface). Clear-color конвертируется в linear (`clear_color`).

use crate::config::srgb_to_linear;
use crate::Color;

/// Палитра темы: фон, сетка, карточки, текст, UI-оверлеи.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeColors {
    /// Фон канваса, sRGB-байты (#1e1e22 тёмный / #f5f5f7 светлый).
    pub background: [u8; 3],
    /// Мелкая сетка, sRGB 0..1.
    pub grid_minor: [f32; 3],
    /// Крупная сетка, sRGB 0..1.
    pub grid_major: [f32; 3],
    /// Заливка карточки по умолчанию.
    pub card_fill: [f32; 4],
    /// Подложка бокса редактирования лейбла связи.
    pub edge_edit_fill: [f32; 4],
    /// Подложка лейбла связи на кривой.
    pub edge_label_fill: [f32; 4],
    /// Фон меню и панелей.
    pub menu_fill: [f32; 4],
    /// Поле ввода панели поиска.
    pub search_input_fill: [f32; 4],
    /// Невыделенная строка результатов поиска (полупрозрачная подложка).
    pub search_row_fill: [f32; 4],
    /// Цвет заголовка карточки.
    pub title: Color,
    /// Цвет иконки в заголовке.
    pub icon: Color,
    /// Цвет тела заметки.
    pub body: Color,
    /// Цвет лейбла связи.
    pub edge_label: Color,
}

impl ThemeColors {
    /// Тёмная тема (базовая, SPEC T1) — прежние константы рендера.
    pub fn dark() -> Self {
        Self {
            background: [0x1e, 0x1e, 0x22],
            grid_minor: [0.141, 0.141, 0.161],
            grid_major: [0.169, 0.169, 0.190],
            card_fill: [0.149, 0.149, 0.173, 1.0],
            edge_edit_fill: [0.13, 0.13, 0.16, 0.95],
            edge_label_fill: [0.11, 0.11, 0.13, 0.85],
            menu_fill: [0.11, 0.11, 0.13, 0.97],
            search_input_fill: [0.16, 0.17, 0.20, 1.0],
            search_row_fill: [0.13, 0.14, 0.17, 0.55],
            title: Color::rgb(0xe6, 0xe6, 0xe6),
            icon: Color::rgb(0x9a, 0xaa, 0xbf),
            body: Color::rgb(0xd4, 0xd4, 0xd4),
            edge_label: Color::rgb(0xcf, 0xd8, 0xe3),
        }
    }

    /// Светлая тема: тёмный текст на белых карточках, сетка светло-серая.
    pub fn light() -> Self {
        Self {
            background: [0xf5, 0xf5, 0xf7],
            // Контраст сетки ~50% к фону (симметрично тёмной теме)
            grid_minor: [0.851, 0.851, 0.878],
            grid_major: [0.769, 0.769, 0.812],
            card_fill: [0.984, 0.984, 0.992, 1.0],
            edge_edit_fill: [0.97, 0.97, 0.98, 0.95],
            edge_label_fill: [0.95, 0.95, 0.97, 0.85],
            menu_fill: [0.98, 0.98, 0.99, 0.97],
            search_input_fill: [0.90, 0.90, 0.93, 1.0],
            search_row_fill: [0.88, 0.88, 0.92, 0.55],
            title: Color::rgb(0x20, 0x20, 0x24),
            icon: Color::rgb(0x6a, 0x7a, 0x8f),
            body: Color::rgb(0x38, 0x38, 0x3e),
            edge_label: Color::rgb(0x2a, 0x35, 0x42),
        }
    }

    /// Палитра по enum темы (canvas-core).
    pub fn from_theme(theme: canvas_core::Theme) -> Self {
        match theme {
            canvas_core::Theme::Dark => Self::dark(),
            canvas_core::Theme::Light => Self::light(),
        }
    }

    /// Clear-color фона канваса (linear space для wgpu).
    pub fn clear_color(&self) -> wgpu::Color {
        let channel = |i: usize| srgb_to_linear(self.background[i] as f64 / 255.0);
        wgpu::Color {
            r: channel(0),
            g: channel(1),
            b: channel(2),
            a: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тёмная палитра — прежние константы рендера (регрессия не меняет вид).
    #[test]
    fn dark_palette_matches_legacy_constants() {
        let dark = ThemeColors::dark();
        assert_eq!(dark.background, [0x1e, 0x1e, 0x22]);
        assert_eq!(dark.card_fill, [0.149, 0.149, 0.173, 1.0]);
        assert_eq!(dark.menu_fill, [0.11, 0.11, 0.13, 0.97]);
    }

    /// Светлая палитра отличается от тёмной по всем ключевым цветам.
    #[test]
    fn light_palette_differs_from_dark() {
        let dark = ThemeColors::dark();
        let light = ThemeColors::light();
        assert_ne!(dark.background, light.background);
        assert_ne!(dark.card_fill, light.card_fill);
        assert_ne!(dark.title, light.title);
        assert_ne!(dark.body, light.body);
        // Светлый фон ярче тёмного по каждому каналу
        for i in 0..3 {
            assert!(light.background[i] > dark.background[i]);
        }
    }

    /// from_theme: enum ↔ палитра.
    #[test]
    fn from_theme_maps_both() {
        assert_eq!(
            ThemeColors::from_theme(canvas_core::Theme::Dark),
            ThemeColors::dark()
        );
        assert_eq!(
            ThemeColors::from_theme(canvas_core::Theme::Light),
            ThemeColors::light()
        );
    }

    /// Clear-color: sRGB-байты → linear (0x1e ≈ 0.01174 в linear).
    #[test]
    fn clear_color_is_linear() {
        let dark = ThemeColors::dark();
        let clear = dark.clear_color();
        assert!((clear.r - srgb_to_linear(0x1e as f64 / 255.0)).abs() < 1e-9);
        assert!((clear.b - srgb_to_linear(0x22 as f64 / 255.0)).abs() < 1e-9);
        assert_eq!(clear.a, 1.0);
        // Светлый фон в linear ярче тёмного
        let light = ThemeColors::light().clear_color();
        assert!(light.r > clear.r);
    }
}
