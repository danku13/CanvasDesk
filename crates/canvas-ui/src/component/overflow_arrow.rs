//! FR-UI-OVERFLOW-ARROW: 4-arrow overflow indicator (scroll hint).
//!
//! Extracts pattern from `explain.rs` (overflow scroll arrows — `app/explain.rs`
//! строки ~1371-1408): 4 стрелки у краёв тела окна, по одной на каждое
//! направление переполнения. Канон геометрии из explain.rs:
//! - `ARROW = 22` — сторона квадратной стрелки (по умолчанию `OVERFLOW_ARROW`
//!   = 22 — то же значение, что в explain.rs:1373);
//! - `EDGE = 6` — отступ стрелки от края тела (по умолчанию
//!   `OVERFLOW_ARROW_EDGE` = 6 — то же значение, что в explain.rs:1374);
//! - 4 позиции: left=center-left, right=center-right, top=top-center,
//!   bottom=bottom-center.
//!
//! ## Отрисовка
//!
//! `paint_overflow_arrow` отдаёт 2 PaintItem (паритет explain.rs:1395-1407):
//! 1. Filled `PaintItem::Rect` (заливка = `tint`, радиус = 7 — паритет
//!    explain.rs:1399, квадрат со скруглением);
//! 2. `PaintItem::Text` со стрелочным глифом «←»/«→»/«↑»/«↓» (паритет
//!    explain.rs:1401-1407) — глиф центрирован в `rect`, цвет — `tint`
//!    (на акцент-заливке текст должен быть контрастным; потребитель передаёт
//!    `tint = palette.text_on_accent` ИЛИ передаёт «контур-стрелку»: tint =
//!    прозрачный fill, border = accent — альтернатива через `paint_outline_arrow`).
//!
//! Альтернатива: 3-квадовый треугольник (3 PaintItem::Rect — decreasing-width
//! rows, forming a triangle). НЕ ИСПОЛЬЗУЕТСЯ — глиф-стрелка читаемее при
//! малом размере (8-22 px), и паритет 1:1 с explain.rs (I-1: ноль визуального
//! скачка при миграции).

use crate::geometry::UiRect;
use crate::paint::{PaintAlign, PaintItem};

/// Сторона квадратной стрелки по умолчанию (паритет explain.rs:1373 `ARROW`).
pub const OVERFLOW_ARROW: f32 = 22.0;

/// Отступ стрелки от края тела (паритет explain.rs:1374 `EDGE`).
pub const OVERFLOW_ARROW_EDGE: f32 = 6.0;

/// Кегль глифа-стрелки (паритет explain.rs:1405 — 12 px).
pub const OVERFLOW_ARROW_GLYPH_SIZE: f32 = 12.0;

/// Радиус скругления квадрата-стрелки (паритет explain.rs:1399 — 7.0).
pub const OVERFLOW_ARROW_RADIUS: f32 = 7.0;

/// Вертикальное смещение глифа внутри квадрата (паритет explain.rs:1402 —
/// +3 px: компенсирует baseline глифа, чтобы он был визуально по центру).
pub const OVERFLOW_ARROW_GLYPH_DY: f32 = 3.0;

/// Высота строки глифа (паритет explain.rs:1402 — 15 px).
pub const OVERFLOW_ARROW_GLYPH_H: f32 = 15.0;

/// Направление стрелки переполнения.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowDir {
    Up,
    Down,
    Left,
    Right,
}

impl OverflowDir {
    /// Глиф-стрелка для направления (← → ↑ ↓ — U+2190..U+2193).
    pub fn glyph(self) -> &'static str {
        match self {
            OverflowDir::Up => "↑",
            OverflowDir::Down => "↓",
            OverflowDir::Left => "←",
            OverflowDir::Right => "→",
        }
    }
}

/// 4 направления переполнения (true = есть скрытый контент за этим краем —
/// показать стрелку).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OverflowDirs {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
}

impl OverflowDirs {
    /// Нет стрелок — `OverflowDirs::default()` (все false).
    pub fn none() -> Self {
        Self::default()
    }

    /// Все 4 направления (для тестов и деградаций).
    pub fn all() -> Self {
        Self {
            up: true,
            down: true,
            left: true,
            right: true,
        }
    }

    /// Хоть одна стрелка показана.
    pub fn any(self) -> bool {
        self.up || self.down || self.left || self.right
    }
}

/// Layout 4 arrow indicators at the edges of `body` rect.
///
/// Each arrow is a small square (`arrow_size × arrow_size`) placed at:
/// - up: top-center, inset `edge` from top;
/// - down: bottom-center, inset `edge` from bottom;
/// - left: left-center, inset `edge` from left;
/// - right: right-center, inset `edge` from right.
///
/// Arrows where `dirs.*` is `false` are NOT included in the output.
///
/// Returns `Vec<(rect, direction)>` — consumer draws via
/// `paint_overflow_arrow(rect, dir, tint)`.
pub fn overflow_arrows(
    body: UiRect,
    dirs: OverflowDirs,
    arrow_size: f32,
) -> Vec<(UiRect, OverflowDir)> {
    let mut out: Vec<(UiRect, OverflowDir)> = Vec::new();
    if body.is_empty() {
        return out;
    }
    let cy = body.y + body.h / 2.0 - arrow_size / 2.0;
    let cx = body.x + body.w / 2.0 - arrow_size / 2.0;
    let edge = OVERFLOW_ARROW_EDGE;
    if dirs.left {
        out.push((
            UiRect::new(body.x + edge, cy, arrow_size, arrow_size),
            OverflowDir::Left,
        ));
    }
    if dirs.right {
        out.push((
            UiRect::new(body.right() - arrow_size - edge, cy, arrow_size, arrow_size),
            OverflowDir::Right,
        ));
    }
    if dirs.up {
        out.push((
            UiRect::new(cx, body.y + edge, arrow_size, arrow_size),
            OverflowDir::Up,
        ));
    }
    if dirs.down {
        out.push((
            UiRect::new(
                cx,
                body.bottom() - arrow_size - edge,
                arrow_size,
                arrow_size,
            ),
            OverflowDir::Down,
        ));
    }
    out
}

/// Paint arrow as 2 `PaintItem`s: filled square (with radius) + arrow glyph
/// centered (паритет explain.rs:1395-1407). `tint` — fill color of the
/// square AND color of the glyph (на акцент-заливке потребитель передаёт
/// контрастный `text_on_accent` для both, ИЛИ передаёт `[0;4]` fill и
/// `border = accent` для outline-варианта через свой собственный paint —
/// кит отдаёт канонический «filled accent square + dark glyph»).
///
/// Glyph centered via `PaintAlign::Center`; vertical offset =
/// `OVERFLOW_ARROW_GLYPH_DY` (компенсация baseline глифа, паритет explain).
pub fn paint_overflow_arrow(rect: UiRect, dir: OverflowDir, tint: [f32; 4]) -> Vec<PaintItem> {
    let glyph = dir.glyph();
    vec![
        // 1. Filled accent square (radius = OVERFLOW_ARROW_RADIUS).
        PaintItem::Rect {
            rect,
            fill: tint,
            border: [0.0; 4],
            radius: OVERFLOW_ARROW_RADIUS,
        },
        // 2. Arrow glyph centered (паритет explain.rs:1401-1407).
        PaintItem::Text {
            area: UiRect::new(
                rect.x,
                rect.y + OVERFLOW_ARROW_GLYPH_DY,
                rect.w,
                OVERFLOW_ARROW_GLYPH_H,
            ),
            text: glyph.to_owned(),
            color: tint,
            size: OVERFLOW_ARROW_GLYPH_SIZE,
            align: PaintAlign::Center,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Все 4 направления true → 4 стрелки на правильных краях: left=center-left,
    /// right=center-right, top=top-center, bottom=bottom-center.
    #[test]
    fn all_four_dirs_place_arrows_at_correct_edges() {
        let body = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let arrows = overflow_arrows(body, OverflowDirs::all(), OVERFLOW_ARROW);
        assert_eq!(arrows.len(), 4, "4 стрелки для all-true dirs");
        let size = OVERFLOW_ARROW;
        let edge = OVERFLOW_ARROW_EDGE;
        let cy = body.y + body.h / 2.0 - size / 2.0;
        let cx = body.x + body.w / 2.0 - size / 2.0;
        // Найти каждую стрелку по направлению.
        let left = arrows
            .iter()
            .find(|(_, d)| *d == OverflowDir::Left)
            .expect("left arrow exists");
        let right = arrows
            .iter()
            .find(|(_, d)| *d == OverflowDir::Right)
            .expect("right arrow exists");
        let up = arrows
            .iter()
            .find(|(_, d)| *d == OverflowDir::Up)
            .expect("up arrow exists");
        let down = arrows
            .iter()
            .find(|(_, d)| *d == OverflowDir::Down)
            .expect("down arrow exists");
        // Left: x = body.x + edge, y = center.
        assert!((left.0.x - (body.x + edge)).abs() < 0.01);
        assert!((left.0.y - cy).abs() < 0.01);
        assert!((left.0.w - size).abs() < 0.01 && (left.0.h - size).abs() < 0.01);
        // Right: x = body.right - size - edge, y = center.
        assert!((right.0.x - (body.right() - size - edge)).abs() < 0.01);
        assert!((right.0.y - cy).abs() < 0.01);
        // Up: x = center, y = body.y + edge.
        assert!((up.0.x - cx).abs() < 0.01);
        assert!((up.0.y - (body.y + edge)).abs() < 0.01);
        // Down: x = center, y = body.bottom - size - edge.
        assert!((down.0.x - cx).abs() < 0.01);
        assert!((down.0.y - (body.bottom() - size - edge)).abs() < 0.01);
    }

    /// Только 2 направления (left + right) → 2 стрелки по бокам, top/bottom
    /// отсутствуют.
    #[test]
    fn only_horizontal_dirs_skip_vertical() {
        let body = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let dirs = OverflowDirs {
            up: false,
            down: false,
            left: true,
            right: true,
        };
        let arrows = overflow_arrows(body, dirs, OVERFLOW_ARROW);
        assert_eq!(arrows.len(), 2);
        let dirs_present: Vec<OverflowDir> = arrows.iter().map(|(_, d)| *d).collect();
        assert!(dirs_present.contains(&OverflowDir::Left));
        assert!(dirs_present.contains(&OverflowDir::Right));
        assert!(!dirs_present.contains(&OverflowDir::Up));
        assert!(!dirs_present.contains(&OverflowDir::Down));
    }

    /// Все false → пустой Vec.
    #[test]
    fn no_dirs_returns_empty() {
        let body = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let arrows = overflow_arrows(body, OverflowDirs::none(), OVERFLOW_ARROW);
        assert!(arrows.is_empty());
    }

    /// Пустой body → пустой Vec (no-op).
    #[test]
    fn empty_body_returns_empty() {
        let body = UiRect::new(0.0, 0.0, 0.0, 0.0);
        let arrows = overflow_arrows(body, OverflowDirs::all(), OVERFLOW_ARROW);
        assert!(arrows.is_empty());
    }

    /// `paint_overflow_arrow` возвращает 2 PaintItem: Rect (filled) + Text
    /// (glyph). Glyph matches direction.
    #[test]
    fn paint_arrow_returns_filled_rect_and_glyph() {
        let rect = UiRect::new(10.0, 10.0, 22.0, 22.0);
        let tint = [0.94, 0.94, 0.94, 1.0];
        for dir in [
            OverflowDir::Up,
            OverflowDir::Down,
            OverflowDir::Left,
            OverflowDir::Right,
        ] {
            let items = paint_overflow_arrow(rect, dir, tint);
            assert_eq!(items.len(), 2, "2 PaintItem для dir={dir:?}");
            // Первый — Rect (filled = tint, radius = OVERFLOW_ARROW_RADIUS).
            match &items[0] {
                PaintItem::Rect {
                    fill,
                    radius,
                    border,
                    ..
                } => {
                    assert_eq!(*fill, tint, "fill = tint");
                    assert!((radius - OVERFLOW_ARROW_RADIUS).abs() < 0.01);
                    assert_eq!(*border, [0.0; 4]);
                }
                other => panic!("первый item должен быть Rect, got {other:?}"),
            }
            // Второй — Text с правильным glyph.
            match &items[1] {
                PaintItem::Text { text, color, .. } => {
                    assert_eq!(text, dir.glyph(), "glyph matches direction");
                    assert_eq!(*color, tint, "text color = tint");
                }
                other => panic!("второй item должен быть Text, got {other:?}"),
            }
        }
    }

    /// `OverflowDirs::any()` — true если хоть одно направление активно.
    #[test]
    fn dirs_any_helper() {
        assert!(!OverflowDirs::none().any());
        assert!(OverflowDirs::all().any());
        assert!(OverflowDirs {
            up: false,
            down: false,
            left: false,
            right: true
        }
        .any());
    }

    /// Glyph mapping для каждого направления.
    #[test]
    fn dir_glyph_mapping() {
        assert_eq!(OverflowDir::Up.glyph(), "↑");
        assert_eq!(OverflowDir::Down.glyph(), "↓");
        assert_eq!(OverflowDir::Left.glyph(), "←");
        assert_eq!(OverflowDir::Right.glyph(), "→");
    }
}
