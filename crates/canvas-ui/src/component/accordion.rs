//! Wave C §5.3.6: Accordion — сворачиваемая секция.
//!
//! Анатомия: `header` (chevron + label) + `content` (expanded/collapsed).
//! Click on header toggles expanded.

use super::{KitPalette, KitState, Shape};
use crate::geometry::{UiPoint, UiRect};
use crate::measure::TextMeasurer;

/// Раскладка accordion-секции (Wave C §5.3.6).
#[derive(Debug, Clone, PartialEq)]
pub struct AccordionLayout {
    /// Rect заголовка (chevron + label).
    pub header: UiRect,
    /// Rect контента (0-height если свёрнут).
    pub content: UiRect,
    /// Rect шеврона (▶/▼).
    pub chevron: UiRect,
    /// Раскрыт ли.
    pub expanded: bool,
}

/// Вёрстка accordion: header сверху, content снизу (если expanded).
pub fn accordion_layout(
    slot: UiRect,
    label: &str,
    expanded: bool,
    header_h: f32,
    content_h: f32,
    chevron_w: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> AccordionLayout {
    let header = UiRect::new(slot.x, slot.y, slot.w, header_h);
    let chevron = UiRect::new(slot.x, slot.y, chevron_w, header_h);
    let content = if expanded {
        UiRect::new(slot.x, slot.y + header_h, slot.w, content_h)
    } else {
        UiRect::new(slot.x, slot.y + header_h, slot.w, 0.0)
    };
    AccordionLayout { header, content, chevron, expanded }
}

/// Стиль accordion: header — control_fill; expanded → accent chevron.
pub fn accordion_style(expanded: bool, state: KitState, p: &KitPalette) -> (super::ControlStyle, [f32; 4]) {
    let fill = match state {
        KitState::Hovered | KitState::Pressed => p.hover_fill,
        _ => p.control_fill,
    };
    let chevron_color = if expanded { p.accent } else { p.text_muted };
    let style = super::ControlStyle {
        fill,
        border: p.control_border,
        text: p.text,
        radius: Shape::S.px(),
        elevation: super::Elevation::None,
    };
    (style, chevron_color)
}

/// Hit-test: клик по header (включая chevron) toggles.
pub fn accordion_hit(layout: &AccordionLayout, p: UiPoint) -> bool {
    layout.header.contains(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accordion_layout_expanded() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 500.0);
        let lay = accordion_layout(slot, "Section", true, 32.0, 100.0, 24.0, &mut m, &mut fs, "sans", 13.0);
        assert!(lay.expanded);
        assert!((lay.header.h - 32.0).abs() < 0.01);
        assert!((lay.content.h - 100.0).abs() < 0.01);
        assert!((lay.content.y - 32.0).abs() < 0.01);
        assert!((lay.chevron.w - 24.0).abs() < 0.01);
    }

    #[test]
    fn accordion_layout_collapsed() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 500.0);
        let lay = accordion_layout(slot, "Section", false, 32.0, 100.0, 24.0, &mut m, &mut fs, "sans", 13.0);
        assert!(!lay.expanded);
        assert!((lay.content.h - 0.0).abs() < 0.01);
    }

    #[test]
    fn accordion_hit_header() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 500.0);
        let lay = accordion_layout(slot, "Section", true, 32.0, 100.0, 24.0, &mut m, &mut fs, "sans", 13.0);
        // Click on header
        assert!(accordion_hit(&lay, UiPoint::new(10.0, 10.0)));
        // Click on chevron (inside header)
        assert!(accordion_hit(&lay, UiPoint::new(5.0, 10.0)));
        // Click on content (not header)
        assert!(!accordion_hit(&lay, UiPoint::new(10.0, 50.0)));
    }
}
