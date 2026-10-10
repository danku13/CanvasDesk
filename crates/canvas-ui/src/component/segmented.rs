//! Wave C §5.3.12: SegmentedControl — переключатель режимов (Apple HIG style).
//!
//! Анатомия: `container` (pill-фон) + `segments` (N) + `active_rect` (заливка).
//! Click on segment activates. Arrow-key navigation.

use super::{KitPalette, KitState, Shape};
use crate::geometry::{UiPoint, UiRect};
use crate::measure::TextMeasurer;

/// Раскладка segmented control (Wave C §5.3.12).
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentedLayout {
    /// Rect контейнера (pill-фон).
    pub container: UiRect,
    /// Сегменты.
    pub segments: Vec<UiRect>,
    /// Активный сегмент (заливка).
    pub active_rect: UiRect,
    /// Индекс активного сегмента.
    pub active: usize,
}

/// Вёрстка segmented control: равные ширины сегментов, pill-контейнер.
pub fn segmented_layout(
    slot: UiRect,
    labels: &[String],
    active: usize,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> SegmentedLayout {
    let container = slot;
    if labels.is_empty() {
        return SegmentedLayout { container, segments: Vec::new(), active_rect: UiRect::default(), active };
    }
    let n = labels.len();
    let seg_w = slot.w / n as f32;
    let segments = (0..n)
        .map(|i| UiRect::new(slot.x + i as f32 * seg_w, slot.y, seg_w, slot.h))
        .collect::<Vec<_>>();
    let active_rect = segments.get(active).copied().unwrap_or_default();
    SegmentedLayout { container, segments, active_rect, active }
}

/// Стиль segmented: container — control_fill; active — control_primary + on_primary.
pub fn segmented_style(active: bool, state: KitState, p: &KitPalette) -> (super::ControlStyle, super::ControlStyle) {
    let container_fill = match state {
        KitState::Hovered | KitState::Pressed => p.hover_fill,
        _ => p.control_fill,
    };
    let container = super::ControlStyle {
        fill: container_fill,
        border: [0.0; 4],
        text: p.text,
        radius: Shape::L.px(),
        elevation: super::Elevation::None,
    };
    let active_style = super::ControlStyle {
        fill: p.control_primary,
        border: [0.0; 4],
        text: p.text_title,
        radius: Shape::L.px(),
        elevation: super::Elevation::None,
    };
    (container, if active { active_style } else { container })
}

/// Hit-test: возвращает индекс сегмента или None.
pub fn segmented_hit(layout: &SegmentedLayout, p: UiPoint) -> Option<usize> {
    layout.segments.iter().position(|s| s.contains(p))
}

/// Arrow-key навигация: ←/→ по сегментам.
pub fn segmented_key(active: usize, count: usize, key: SegmentedKey) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let new = match key {
        SegmentedKey::Prev => if active == 0 { count - 1 } else { active - 1 },
        SegmentedKey::Next => (active + 1) % count,
    };
    if new != active { Some(new) } else { None }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentedKey {
    Prev,
    Next,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segmented_equal_widths() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 32.0);
        let labels = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let lay = segmented_layout(slot, &labels, 0, &mut m, &mut fs, "sans", 13.0);
        assert_eq!(lay.segments.len(), 3);
        // Равные ширины: 100 each
        assert!((lay.segments[0].w - 100.0).abs() < 0.01);
        assert!((lay.segments[1].x - 100.0).abs() < 0.01);
        assert_eq!(lay.active, 0);
    }

    #[test]
    fn segmented_hit() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 32.0);
        let labels = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let lay = segmented_layout(slot, &labels, 0, &mut m, &mut fs, "sans", 13.0);
        assert_eq!(segmented_hit(&lay, UiPoint::new(10.0, 10.0)), Some(0));
        assert_eq!(segmented_hit(&lay, UiPoint::new(150.0, 10.0)), Some(1));
        assert_eq!(segmented_hit(&lay, UiPoint::new(250.0, 10.0)), Some(2));
        assert_eq!(segmented_hit(&lay, UiPoint::new(310.0, 10.0)), None);
    }

    #[test]
    fn segmented_key_wrap() {
        assert_eq!(segmented_key(0, 3, SegmentedKey::Next), Some(1));
        assert_eq!(segmented_key(2, 3, SegmentedKey::Next), Some(0)); // wrap
        assert_eq!(segmented_key(0, 3, SegmentedKey::Prev), Some(2)); // wrap
    }
}
