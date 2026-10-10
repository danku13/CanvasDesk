//! Wave C §5.3.10: Avatar — иконка пользователя/агента (initials/icon/image).
//!
//! Анатомия: `rect` (квадрат) + source (Initials/Icon/Image).
//! Tone — цвет фона для initials/icon.

use super::{ControlSize, KitPalette, Shape};
use crate::geometry::UiRect;
use crate::kit::BadgeTone;

/// Источник аватара (Wave C §5.3.10).
#[derive(Debug, Clone, PartialEq)]
pub enum AvatarSource {
    /// Инициалы ("AB" — 2 буквы).
    Initials(String),
    /// Иконка-плейсхолдер.
    Icon(&'static str),
    /// Растровое изображение (texture_id потребителя).
    Image(u64),
}

/// Раскладка аватара (Wave C §5.3.10).
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarLayout {
    /// Rect аватара (квадрат, сторона = size.icon_btn()).
    pub rect: UiRect,
    /// Источник.
    pub source: AvatarSource,
    /// Тон фона (для initials/icon).
    pub tone: BadgeTone,
}

/// Вёрстка аватара: квадрат в слоте, центрирован.
pub fn avatar_layout(slot: UiRect, source: AvatarSource, size: ControlSize) -> AvatarLayout {
    let side = size.icon_btn();
    let x = slot.x + (slot.w - side).max(0.0) / 2.0;
    let y = slot.y + (slot.h - side).max(0.0) / 2.0;
    AvatarLayout {
        rect: UiRect::new(x, y, side, side),
        source,
        tone: BadgeTone::Default,
    }
}

/// Стиль аватара: tone → fill, text — on-fill.
pub fn avatar_style(tone: BadgeTone, p: &KitPalette) -> super::ControlStyle {
    let fill = match tone {
        BadgeTone::Default => p.control_fill,
        BadgeTone::Primary => p.control_primary,
        BadgeTone::Success => p.control_success,
        BadgeTone::Warning => p.control_warning,
        BadgeTone::Danger => p.control_danger,
        BadgeTone::Info => p.accent,
    };
    super::ControlStyle {
        fill,
        border: [0.0; 4],
        text: p.text_title,
        radius: Shape::Full.px(),
        elevation: super::Elevation::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avatar_layout_centered() {
        let slot = UiRect::new(0.0, 0.0, 100.0, 50.0);
        let lay = avatar_layout(slot, AvatarSource::Initials("AB".into()), ControlSize::Md);
        // side = 32 (Md icon_btn), centered
        assert!((lay.rect.w - 32.0).abs() < 0.01);
        assert!((lay.rect.x - 34.0).abs() < 0.01); // (100-32)/2
        assert!((lay.rect.y - 9.0).abs() < 0.01); // (50-32)/2
    }

    #[test]
    fn avatar_source_variants() {
        let slot = UiRect::new(0.0, 0.0, 50.0, 50.0);
        let _ = avatar_layout(slot, AvatarSource::Initials("JD".into()), ControlSize::Sm);
        let _ = avatar_layout(slot, AvatarSource::Icon("user"), ControlSize::Sm);
        let _ = avatar_layout(slot, AvatarSource::Image(42), ControlSize::Sm);
    }
}
