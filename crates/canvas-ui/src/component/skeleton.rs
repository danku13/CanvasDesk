//! Wave C §5.3.8: Skeleton — заглушка при загрузке (pulse animation).
//!
//! Анатомия: `rects` (серые блоки) + `phase` (pulse 0.04..0.12).
//! Pattern: TextLines, Card, Table.

use crate::geometry::UiRect;

/// Шаблон skeleton-заглушки (Wave C §5.3.8).
#[derive(Debug, Clone, PartialEq)]
pub enum SkeletonPattern {
    /// N строк текста (row_h + gap).
    TextLines { count: usize, row_h: f32 },
    /// Карточка: header + body lines + footer.
    Card { w: f32, h: f32 },
    /// Таблица: rows × cols.
    Table { rows: usize, cols: usize },
}

/// Раскладка skeleton (Wave C §5.3.8).
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletonLayout {
    pub rects: Vec<UiRect>,
    /// Pulse phase (0..1) из anim.rs.
    pub phase: f32,
}

/// Вёрстка skeleton по шаблону.
pub fn skeleton_layout(slot: UiRect, pattern: &SkeletonPattern, gap: f32) -> SkeletonLayout {
    let mut rects = Vec::new();
    match pattern {
        SkeletonPattern::TextLines { count, row_h } => {
            for i in 0..*count {
                let y = slot.y + i as f32 * (row_h + gap);
                // Последняя строка короче (70%).
                let w = if i == count - 1 { slot.w * 0.7 } else { slot.w };
                rects.push(UiRect::new(slot.x, y, w, *row_h));
            }
        }
        SkeletonPattern::Card { w, h } => {
            let header_h = h * 0.2;
            rects.push(UiRect::new(slot.x, slot.y, *w, header_h));
            for i in 0..3 {
                let y = slot.y + header_h + gap + i as f32 * (header_h * 0.6 + gap);
                let line_w = if i == 2 { w * 0.6 } else { *w };
                rects.push(UiRect::new(slot.x, y, line_w, header_h * 0.6));
            }
            let footer_y = slot.y + h - header_h * 0.5;
            rects.push(UiRect::new(slot.x, footer_y, w * 0.4, header_h * 0.5));
        }
        SkeletonPattern::Table { rows, cols } => {
            let cell_w = (slot.w - gap * (cols - 1) as f32) / *cols as f32;
            let cell_h = (slot.h - gap * (rows - 1) as f32) / *rows as f32;
            for r in 0..*rows {
                for c in 0..*cols {
                    let x = slot.x + c as f32 * (cell_w + gap);
                    let y = slot.y + r as f32 * (cell_h + gap);
                    rects.push(UiRect::new(x, y, cell_w, cell_h));
                }
            }
        }
    }
    SkeletonLayout { rects, phase: 0.0 }
}

/// Pulse-анимация (M3 shimmer): alpha 0.04 ↔ 0.12, period 1500ms.
/// `time_ms` — время от anim.rs.
pub fn skeleton_alpha(time_ms: u64) -> f32 {
    let period = 1500.0_f64;
    let phase = (time_ms as f64 % period) / period;
    // sine wave: 0.04 + 0.08 * (sin(phase * 2π) * 0.5 + 0.5)
    let sine = (phase * std::f64::consts::TAU).sin() * 0.5 + 0.5;
    (0.04 + 0.08 * sine) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_text_lines() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 100.0);
        let pat = SkeletonPattern::TextLines { count: 3, row_h: 16.0 };
        let lay = skeleton_layout(slot, &pat, 8.0);
        assert_eq!(lay.rects.len(), 3);
        // Первая строка — полная ширина
        assert!((lay.rects[0].w - 200.0).abs() < 0.01);
        // Последняя — 70%
        assert!((lay.rects[2].w - 140.0).abs() < 0.01);
        // Вторая строка: y = 16 + 8 = 24
        assert!((lay.rects[1].y - 24.0).abs() < 0.01);
    }

    #[test]
    fn skeleton_table() {
        let slot = UiRect::new(0.0, 0.0, 300.0, 200.0);
        let pat = SkeletonPattern::Table { rows: 2, cols: 3 };
        let lay = skeleton_layout(slot, &pat, 8.0);
        assert_eq!(lay.rects.len(), 6);
        // cell_w = (300 - 8*2) / 3 = 284/3 ≈ 94.67
        let cell_w = (300.0 - 8.0 * 2.0) / 3.0;
        assert!((lay.rects[0].w - cell_w).abs() < 0.01);
    }

    #[test]
    fn skeleton_card() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 300.0);
        let pat = SkeletonPattern::Card { w: 200.0, h: 100.0 };
        let lay = skeleton_layout(slot, &pat, 8.0);
        // header + 3 body lines + footer = 5 rects
        assert_eq!(lay.rects.len(), 5);
    }

    #[test]
    fn skeleton_alpha_in_range() {
        for t in [0, 375, 750, 1125, 1500, 1875] {
            let a = skeleton_alpha(t);
            assert!(a >= 0.04 - 0.001 && a <= 0.12 + 0.001, "alpha {} at t={} out of range", a, t);
        }
    }

    #[test]
    fn skeleton_alpha_midpoint() {
        // At t=750 (half period), sine = sin(π) = 0 → alpha = 0.04
        let a = skeleton_alpha(750);
        assert!((a - 0.04).abs() < 0.01 || (a - 0.12).abs() < 0.01); // sin(π)=0 или sin(0)=0
    }
}
