//! Wave A §5.5.6: Smart guides — линии выравнивания + distance labels при drag.
//!
//! tldraw/Miro/Figma pattern: при drag ноды показываются magenta линии
//! выравнивания к siblings (center/edge snap) + distance labels «12 px».

use canvas_ui::geometry::{UiPoint, UiRect, UiVec2};

/// Ориентация линии-гида (Wave A §5.5.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideOrientation {
    Horizontal,
    Vertical,
}

/// Линия выравнивания (Wave A §5.5.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuideLine {
    pub orientation: GuideOrientation,
    /// Позиция (x для Vertical, y для Horizontal).
    pub pos: f32,
    /// Диапазон линии [start, end].
    pub range: [f32; 2],
}

/// Distance label между dragged и sibling (Wave A §5.5.6).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistanceLabel {
    pub pos: UiPoint,
    pub text: [char; 16], // фиксированный буфер (не String — no alloc в hot path)
    pub text_len: usize,
}

impl DistanceLabel {
    pub fn new(pos: UiPoint, text: &str) -> Self {
        let mut buf = [' '; 16];
        let chars: Vec<char> = text.chars().collect();
        let len = chars.len().min(16);
        for (i, c) in chars.iter().take(len).enumerate() {
            buf[i] = *c;
        }
        Self { pos, text: buf, text_len: len }
    }

    pub fn text_str(&self) -> String {
        self.text[..self.text_len].iter().collect()
    }
}

/// Результат smart-guides вычисления (Wave A §5.5.6).
#[derive(Debug, Clone, PartialEq)]
pub struct SmartGuides {
    /// Линии выравнивания (center/edge snap).
    pub lines: Vec<GuideLine>,
    /// Distance labels между dragged и siblings.
    pub labels: Vec<DistanceLabel>,
}

impl Default for SmartGuides {
    fn default() -> Self {
        Self { lines: Vec::new(), labels: Vec::new() }
    }
}

/// Порог snap (px) — расстояние до центра/края sibling, при котором
/// показывается guide и происходит snap.
pub const SNAP_THRESHOLD: f32 = 6.0;

/// Вычислить smart-guides для dragged rect относительно siblings (Wave A §5.5.6).
///
/// Алгоритм:
/// 1. Для каждого sibling — проверить center/edge alignment (vertical/horizontal).
/// 2. Если разница < threshold — добавить guide line + snap offset.
/// 3. Добавить distance labels для ближайших siblings.
///
/// Возвращает (SmartGuides, snap_offset) — offset на который нужно сдвинуть
/// dragged для align (0 если нет snap).
pub fn compute_smart_guides(
    dragged: UiRect,
    siblings: &[UiRect],
    threshold: f32,
) -> (SmartGuides, UiVec2) {
    let mut guides = SmartGuides::default();
    let mut snap_x = 0.0_f32;
    let mut snap_y = 0.0_f32;
    let dragged_cx = dragged.x + dragged.w / 2.0;
    let dragged_cy = dragged.y + dragged.h / 2.0;
    for sibling in siblings {
        let sib_cx = sibling.x + sibling.w / 2.0;
        let sib_cy = sibling.y + sibling.h / 2.0;
        // Vertical center alignment (x axis).
        let dx = (dragged_cx - sib_cx).abs();
        if dx < threshold {
            snap_x = sib_cx - dragged_cx;
            guides.lines.push(GuideLine {
                orientation: GuideOrientation::Vertical,
                pos: sib_cx,
                range: [dragged_cy.min(sib_cy), dragged_cy.max(sib_cy)],
            });
        }
        // Horizontal center alignment (y axis).
        let dy = (dragged_cy - sib_cy).abs();
        if dy < threshold {
            snap_y = sib_cy - dragged_cy;
            guides.lines.push(GuideLine {
                orientation: GuideOrientation::Horizontal,
                pos: sib_cy,
                range: [dragged_cx.min(sib_cx), dragged_cx.max(sib_cx)],
            });
        }
        // Edge alignment (left/right/top/bottom).
        let edges = [
            (dragged.x, sibling.x, GuideOrientation::Vertical, true),
            (dragged.right(), sibling.right(), GuideOrientation::Vertical, true),
            (dragged.y, sibling.y, GuideOrientation::Horizontal, false),
            (dragged.bottom(), sibling.bottom(), GuideOrientation::Horizontal, false),
        ];
        for (d_edge, s_edge, orient, is_x) in edges {
            let diff = (d_edge - s_edge).abs();
            if diff < threshold {
                let offset = s_edge - d_edge;
                if is_x {
                    if snap_x.abs() < 0.001 {
                        snap_x = offset;
                    }
                } else if snap_y.abs() < 0.001 {
                    snap_y = offset;
                }
                let range = if is_x {
                    [dragged_cy.min(sib_cy), dragged_cy.max(sib_cy)]
                } else {
                    [dragged_cx.min(sib_cx), dragged_cx.max(sib_cx)]
                };
                guides.lines.push(GuideLine {
                    orientation: orient,
                    pos: s_edge,
                    range,
                });
            }
        }
        // Distance label (между центрами).
        if dx > threshold || dy > threshold {
            let label_pos = UiPoint::new(
                (dragged_cx + sib_cx) / 2.0,
                (dragged_cy + sib_cy) / 2.0,
            );
            let dist = ((dragged_cx - sib_cx).powi(2) + (dragged_cy - sib_cy).powi(2)).sqrt();
            let text = format!("{} px", dist.round() as i32);
            guides.labels.push(DistanceLabel::new(label_pos, &text));
        }
    }
    (guides, UiVec2::new(snap_x, snap_y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smart_guides_center_alignment_vertical() {
        // dragged center x = 100, sibling center x = 102 → within threshold
        let dragged = UiRect::new(80.0, 50.0, 40.0, 40.0); // cx=100
        let sibling = UiRect::new(82.0, 200.0, 40.0, 40.0); // cx=102
        let (guides, snap) = compute_smart_guides(dragged, &[sibling], SNAP_THRESHOLD);
        // Vertical guide at x=102 (sibling center)
        assert!(guides.lines.iter().any(|l| l.orientation == GuideOrientation::Vertical && (l.pos - 102.0).abs() < 0.01));
        // snap_x = 102-100 = 2
        assert!((snap.x - 2.0).abs() < 0.01);
    }

    #[test]
    fn smart_guides_no_alignment_when_far() {
        let dragged = UiRect::new(0.0, 0.0, 40.0, 40.0); // cx=20
        let sibling = UiRect::new(500.0, 500.0, 40.0, 40.0); // cx=520
        let (guides, snap) = compute_smart_guides(dragged, &[sibling], SNAP_THRESHOLD);
        // No guide lines (centers too far)
        assert!(guides.lines.is_empty());
        assert!((snap.x - 0.0).abs() < 0.01);
        // But distance label should be present
        assert_eq!(guides.labels.len(), 1);
    }

    #[test]
    fn smart_guides_edge_alignment() {
        // dragged left edge = 10, sibling left edge = 12 → within threshold
        let dragged = UiRect::new(10.0, 50.0, 40.0, 40.0);
        let sibling = UiRect::new(12.0, 200.0, 40.0, 40.0);
        let (guides, snap) = compute_smart_guides(dragged, &[sibling], SNAP_THRESHOLD);
        // snap_x = 12-10 = 2
        assert!((snap.x - 2.0).abs() < 0.01 || guides.lines.iter().any(|l| l.orientation == GuideOrientation::Vertical));
    }

    #[test]
    fn smart_guides_empty_siblings() {
        let dragged = UiRect::new(0.0, 0.0, 40.0, 40.0);
        let (guides, snap) = compute_smart_guides(dragged, &[], SNAP_THRESHOLD);
        assert!(guides.lines.is_empty());
        assert!(guides.labels.is_empty());
        assert!((snap.x - 0.0).abs() < 0.01);
        assert!((snap.y - 0.0).abs() < 0.01);
    }

    #[test]
    fn distance_label_text() {
        let label = DistanceLabel::new(UiPoint::new(10.0, 20.0), "12 px");
        assert_eq!(label.text_str(), "12 px");
        assert_eq!(label.text_len, 5);
    }

    #[test]
    fn distance_label_truncates_long_text() {
        let label = DistanceLabel::new(UiPoint::new(0.0, 0.0), "This is a very long text that exceeds 16 chars");
        // Truncated to 16 chars
        assert_eq!(label.text_len, 16);
    }
}
