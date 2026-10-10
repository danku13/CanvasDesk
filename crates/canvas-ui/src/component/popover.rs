//! Wave C §5.3.11: Popover — всплывающее окно с указателем (arrow).
//!
//! Анатомия: `content` + `arrow` (указатель на якорь).
//! Close-on-outside-click через SurfaceRegistry (Block capture).

use crate::geometry::UiRect;
use crate::component::dropdown::AnchoredSide;

/// Раскладка popover (Wave C §5.3.11).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopoverLayout {
    /// Rect контента.
    pub content: UiRect,
    /// Rect стрелки-указателя (если show_arrow=true).
    pub arrow: UiRect,
    /// Сторона якоря, на которой popover.
    pub arrow_side: AnchoredSide,
    /// Flipped (из-за нехватки места на preferred_side).
    pub flipped: bool,
}

/// Вёрстка popover: под/над якорем, при нехватке места — flip.
/// `preferred_side` — где желательно показать. `show_arrow` — рисовать ли указатель.
pub fn popover(
    anchor: UiRect,
    viewport: UiRect,
    content_size: crate::geometry::UiVec2,
    preferred_side: AnchoredSide,
    show_arrow: bool,
) -> PopoverLayout {
    let arrow_size = if show_arrow { 8.0 } else { 0.0 };
    let (content, arrow, side, flipped) = match preferred_side {
        AnchoredSide::Below => {
            let below_y = anchor.bottom() + arrow_size + 4.0;
            if below_y + content_size.y <= viewport.bottom() {
                // Below — помещается.
                let content = UiRect::new(anchor.x, below_y, content_size.x, content_size.y);
                let arrow = UiRect::new(anchor.x + anchor.w / 2.0 - 4.0, anchor.bottom() + 2.0, 8.0, 6.0);
                (content, arrow, AnchoredSide::Below, false)
            } else {
                // Flip above.
                let above_y = anchor.y - arrow_size - 4.0 - content_size.y;
                if above_y >= viewport.y {
                    let content = UiRect::new(anchor.x, above_y, content_size.x, content_size.y);
                    let arrow = UiRect::new(anchor.x + anchor.w / 2.0 - 4.0, anchor.y - 8.0, 8.0, 6.0);
                    (content, arrow, AnchoredSide::Above, true)
                } else {
                    // Не помещается нигде — прижать к низу вьюпорта.
                    let content = UiRect::new(anchor.x, viewport.bottom() - content_size.y, content_size.x, content_size.y);
                    (content, UiRect::default(), AnchoredSide::Below, true)
                }
            }
        }
        AnchoredSide::Above => {
            let above_y = anchor.y - arrow_size - 4.0 - content_size.y;
            if above_y >= viewport.y {
                let content = UiRect::new(anchor.x, above_y, content_size.x, content_size.y);
                let arrow = UiRect::new(anchor.x + anchor.w / 2.0 - 4.0, anchor.y - 8.0, 8.0, 6.0);
                (content, arrow, AnchoredSide::Above, false)
            } else {
                // Flip below.
                let below_y = anchor.bottom() + arrow_size + 4.0;
                let content = UiRect::new(anchor.x, below_y, content_size.x, content_size.y);
                let arrow = UiRect::new(anchor.x + anchor.w / 2.0 - 4.0, anchor.bottom() + 2.0, 8.0, 6.0);
                (content, arrow, AnchoredSide::Below, true)
            }
        }
        _ => {
            // Right/Left — упрощённо (как Below).
            let content = UiRect::new(anchor.right() + 4.0, anchor.y, content_size.x, content_size.y);
            (content, UiRect::default(), preferred_side, false)
        }
    };
    PopoverLayout { content, arrow, arrow_side: side, flipped }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{UiRect, UiVec2};

    #[test]
    fn popover_below_when_fits() {
        let anchor = UiRect::new(50.0, 50.0, 100.0, 30.0);
        let viewport = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let lay = popover(anchor, viewport, UiVec2::new(200.0, 100.0), AnchoredSide::Below, true);
        assert!(!lay.flipped);
        assert!(lay.content.y >= anchor.bottom());
        assert!(lay.arrow.w > 0.0); // arrow виден
    }

    #[test]
    fn popover_flips_above_when_no_space_below() {
        let anchor = UiRect::new(50.0, 550.0, 100.0, 30.0); // у нижнего края
        let viewport = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let lay = popover(anchor, viewport, UiVec2::new(200.0, 100.0), AnchoredSide::Below, true);
        assert!(lay.flipped);
        assert!(lay.content.y < anchor.y); // выше якоря
    }

    #[test]
    fn popover_no_arrow_when_show_arrow_false() {
        let anchor = UiRect::new(50.0, 50.0, 100.0, 30.0);
        let viewport = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let lay = popover(anchor, viewport, UiVec2::new(200.0, 100.0), AnchoredSide::Below, false);
        assert_eq!(lay.arrow.w, 0.0); // нет arrow
    }
}
