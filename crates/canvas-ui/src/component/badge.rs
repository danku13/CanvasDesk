//! Wave C §5.3.9: Badge — счётчик/статус/индикатор.
//!
//! Анатомия: `rect` (маленький квад/pill) + glyph/text.
//! Tone: Default/Primary/Success/Warning/Danger/Info.

use super::{KitPalette, KitState, Shape};
use crate::geometry::{UiPoint, UiRect};
use crate::measure::TextMeasurer;

/// Тип бейджа (Wave C §5.3.9).
#[derive(Debug, Clone, PartialEq)]
pub enum BadgeKind {
    /// Числовой счётчик (уведомления, выделенные элементы).
    Count(usize),
    /// Текстовый бейдж (статус, категория).
    Text(String),
    /// Точка-индикатор (онлайн, изменено).
    Dot,
}

/// Цветовой тон бейджа (Wave C §5.3.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeTone {
    #[default]
    Default,  // secondary
    Primary,  // accent
    Success,  // green
    Warning,  // amber
    Danger,   // red
    Info,     // blue
}

/// Раскладка бейджа (Wave C §5.3.9).
#[derive(Debug, Clone, PartialEq)]
pub struct BadgeLayout {
    /// Rect бейджа (позиционируется relative к якорю — top-right corner).
    pub rect: UiRect,
    /// Тип бейджа.
    pub kind: BadgeKind,
    /// Тон.
    pub tone: BadgeTone,
}

/// Вёрстка бейджа: позиционируется relative к `anchor` (top-right corner).
/// Dot — круг 8px; Count — pill с числом; Text — pill с текстом.
pub fn badge_layout(
    anchor: UiRect,
    kind: BadgeKind,
    tone: BadgeTone,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> BadgeLayout {
    let (w, h) = match &kind {
        BadgeKind::Dot => (8.0, 8.0),
        BadgeKind::Count(n) => {
            let text = n.to_string();
            let text_w = m.width_of(fs, &text, family, font_size);
            let w = text_w + 8.0; // pad 4 each side
            (w.max(16.0), 16.0) // минимум 16px
        }
        BadgeKind::Text(t) => {
            let text_w = m.width_of(fs, t, family, font_size);
            let w = text_w + 8.0;
            (w.max(16.0), 16.0)
        }
    };
    // Top-right corner of anchor.
    let x = anchor.right() - w / 2.0;
    let y = anchor.y - h / 2.0;
    let rect = UiRect::new(x, y, w, h);
    BadgeLayout { rect, kind, tone }
}

/// Стиль бейджа по тону.
pub fn badge_style(tone: BadgeTone, p: &KitPalette) -> super::ControlStyle {
    let fill = match tone {
        BadgeTone::Default => p.control_fill,
        BadgeTone::Primary => p.control_primary,
        BadgeTone::Success => p.control_success,
        BadgeTone::Warning => p.control_warning,
        BadgeTone::Danger => p.control_danger,
        BadgeTone::Info => p.accent,
    };
    let text = match tone {
        BadgeTone::Default => p.text,
        _ => p.text_title,
    };
    super::ControlStyle {
        fill,
        border: [0.0; 4],
        text,
        radius: Shape::Full.px(),
        elevation: super::Elevation::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_dot_layout() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let anchor = UiRect::new(10.0, 10.0, 30.0, 30.0);
        let lay = badge_layout(anchor, BadgeKind::Dot, BadgeTone::Danger, &mut m, &mut fs, "sans", 10.0);
        // Dot: 8×8, top-right corner
        assert!((lay.rect.w - 8.0).abs() < 0.01);
        assert!((lay.rect.h - 8.0).abs() < 0.01);
        // x = anchor.right - w/2 = 40 - 4 = 36
        assert!((lay.rect.x - 36.0).abs() < 0.01);
        // y = anchor.y - h/2 = 10 - 4 = 6
        assert!((lay.rect.y - 6.0).abs() < 0.01);
    }

    #[test]
    fn badge_count_layout() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let anchor = UiRect::new(10.0, 10.0, 30.0, 30.0);
        let lay = badge_layout(anchor, BadgeKind::Count(3), BadgeTone::Danger, &mut m, &mut fs, "sans", 10.0);
        // Count: pill, минимум 16px высота
        assert!(lay.rect.h >= 16.0);
        assert!(lay.rect.w >= 16.0);
    }

    #[test]
    fn badge_style_by_tone() {
        let p = test_palette();
        let s = badge_style(BadgeTone::Danger, &p);
        assert_eq!(s.fill, p.control_danger);
        let s = badge_style(BadgeTone::Success, &p);
        assert_eq!(s.fill, p.control_success);
        let s = badge_style(BadgeTone::Default, &p);
        assert_eq!(s.fill, p.control_fill);
    }

    fn test_palette() -> KitPalette {
        KitPalette {
            panel_fill: [0.0; 4], panel_border: [0.0; 4],
            control_fill: [0.3, 0.3, 0.3, 1.0], control_border: [0.0; 4],
            control_primary: [0.1, 0.5, 0.9, 1.0], control_danger: [0.9, 0.1, 0.1, 1.0],
            hover_fill: [0.0; 4], primary_hover_fill: [0.0; 4], selected_fill: [0.0; 4],
            text: [0.9, 0.9, 0.9, 1.0], text_title: [1.0, 1.0, 1.0, 1.0],
            text_muted: [0.6, 0.6, 0.6, 1.0], disabled_text: [0.5, 0.5, 0.5, 1.0],
            accent: [0.0, 0.5, 1.0, 1.0],
            control_success: [0.1, 0.8, 0.3, 1.0], control_warning: [0.9, 0.7, 0.1, 1.0],
            stage_dim: [0.0; 4], scrollbar_thumb: [0.0; 4], rule_color: [0.0; 4],
        }
    }
}
