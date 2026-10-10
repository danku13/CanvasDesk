//! Wave C §5.3.7: Progress — индикатор прогресса (linear/circular/spinner).
//!
//! Анатомия: `track` + `fill` (для Linear) или `circle` (для Circular).
//! Indeterminate — анимация (phase из anim.rs).

use super::{ControlSize, KitPalette, KitState, Shape};
use crate::geometry::UiRect;

/// Тип прогресс-индикатора (Wave C §5.3.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProgressKind {
    /// Linear determinate (value 0..1).
    Linear { value: f32 },
    /// Linear indeterminate (анимация).
    LinearIndeterminate,
    /// Circular determinate (value 0..1, radius в px).
    Circular { value: f32, radius: f32 },
    /// Circular indeterminate (spinner, radius в px).
    Spinner { radius: f32 },
}

/// Раскладка progress-индикатора (Wave C §5.3.7).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressLayout {
    /// Rect трека (для Linear) или круга (для Circular).
    pub track: UiRect,
    /// Rect заполненной части (для Linear). Для Circular — тот же track.
    pub fill: UiRect,
    /// Тип индикатора.
    pub kind: ProgressKind,
}

/// Вёрстка progress в слоте.
pub fn progress_layout(slot: UiRect, kind: ProgressKind, size: ControlSize) -> ProgressLayout {
    match kind {
        ProgressKind::Linear { value } => {
            let track_h = (size.button_h() * 0.2).max(2.0); // ~20% высоты кнопки
            let track_y = slot.y + (slot.h - track_h).max(0.0) / 2.0;
            let track = UiRect::new(slot.x, track_y, slot.w, track_h);
            let fill_w = slot.w * value.clamp(0.0, 1.0);
            let fill = UiRect::new(slot.x, track_y, fill_w, track_h);
            ProgressLayout { track, fill, kind }
        }
        ProgressKind::LinearIndeterminate => {
            let track_h = (size.button_h() * 0.2).max(2.0);
            let track_y = slot.y + (slot.h - track_h).max(0.0) / 2.0;
            let track = UiRect::new(slot.x, track_y, slot.w, track_h);
            // Indeterminate: fill = 40% ширины, позиция = phase * 60%
            let fill_w = slot.w * 0.4;
            let fill = UiRect::new(slot.x, track_y, fill_w, track_h);
            ProgressLayout { track, fill, kind }
        }
        ProgressKind::Circular { value, radius } | ProgressKind::Spinner { radius } => {
            let diameter = radius * 2.0;
            let cx = slot.x + slot.w / 2.0 - radius;
            let cy = slot.y + slot.h / 2.0 - radius;
            let track = UiRect::new(cx, cy, diameter, diameter);
            ProgressLayout { track, fill: track, kind }
        }
    }
}

/// Стиль progress: track — control_fill, fill — primary.
pub fn progress_style(state: KitState, p: &KitPalette) -> (super::ControlStyle, [f32; 4]) {
    let track_fill = match state {
        KitState::Disabled => p.control_fill,
        _ => p.control_fill,
    };
    let track_style = super::ControlStyle {
        fill: track_fill,
        border: [0.0; 4],
        text: [0.0; 4],
        radius: Shape::Full.px(),
        elevation: super::Elevation::None,
    };
    let fill_color = match state {
        KitState::Disabled => p.disabled_text,
        _ => p.control_primary,
    };
    (track_style, fill_color)
}

/// Animation phase для indeterminate (0..1, loop).
/// `time_ms` — время от anim.rs или consumer.
/// Linear: phase = (time_ms / 1500) % 1.0; позиция fill = phase * 60%.
/// Circular: phase = (time_ms / 1000) % 1.0; угол = phase * 360°.
pub fn progress_phase(time_ms: u64, kind: &ProgressKind) -> f32 {
    let period_ms = match kind {
        ProgressKind::LinearIndeterminate => 1500,
        ProgressKind::Spinner { .. } => 1000,
        _ => return 0.0,
    };
    (time_ms % period_ms) as f32 / period_ms as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_linear_determinate() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 30.0);
        let lay = progress_layout(slot, ProgressKind::Linear { value: 0.5 }, ControlSize::Sm);
        // value=0.5 → fill_w = 100
        assert!((lay.fill.w - 100.0).abs() < 0.01);
        assert!((lay.track.w - 200.0).abs() < 0.01);
    }

    #[test]
    fn progress_linear_value_clamped() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 30.0);
        let lay = progress_layout(slot, ProgressKind::Linear { value: 1.5 }, ControlSize::Sm);
        assert!((lay.fill.w - 200.0).abs() < 0.01);
        let lay = progress_layout(slot, ProgressKind::Linear { value: -0.5 }, ControlSize::Sm);
        assert!((lay.fill.w - 0.0).abs() < 0.01);
    }

    #[test]
    fn progress_linear_indeterminate_fill_40_percent() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 30.0);
        let lay = progress_layout(slot, ProgressKind::LinearIndeterminate, ControlSize::Sm);
        // fill = 40% ширины
        assert!((lay.fill.w - 80.0).abs() < 0.01);
    }

    #[test]
    fn progress_circular_centered() {
        let slot = UiRect::new(0.0, 0.0, 100.0, 100.0);
        let lay = progress_layout(slot, ProgressKind::Circular { value: 0.5, radius: 20.0 }, ControlSize::Sm);
        // diameter = 40, centered
        assert!((lay.track.w - 40.0).abs() < 0.01);
        assert!((lay.track.x - 30.0).abs() < 0.01); // (100-40)/2
        assert!((lay.track.y - 30.0).abs() < 0.01);
    }

    #[test]
    fn progress_phase_loops() {
        let kind = ProgressKind::LinearIndeterminate;
        // period=1500, phase at 750 = 0.5
        let phase = progress_phase(750, &kind);
        assert!((phase - 0.5).abs() < 0.01);
        // phase at 1500 = 0 (wrap)
        let phase = progress_phase(1500, &kind);
        assert!(phase < 0.01);
    }

    #[test]
    fn progress_phase_zero_for_determinate() {
        let kind = ProgressKind::Linear { value: 0.5 };
        assert_eq!(progress_phase(100, &kind), 0.0);
    }
}
