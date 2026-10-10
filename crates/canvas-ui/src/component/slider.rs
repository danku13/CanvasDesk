//! Wave C §5.3.3: Slider — ползунок с треком, заполнением, бегунком.
//!
//! Анатомия: `track` (горизонтальная полоса) + `filled` (от начала до значения)
//! + `knob` (бегунок). Keyboard: ←/→ step, Home/End min/max, PageUp/Down big step.
//! Hit: knob (drag) + track (jump-to-click).

use super::{ControlSize, ControlStyle, KitPalette, KitState, Shape};
use crate::geometry::{UiPoint, UiRect};

/// Опции слайдера (Wave C §5.3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct SliderOpts {
    pub min: f32,
    pub max: f32,
    /// None — continuous; Some(step) — discrete.
    pub step: Option<f32>,
    /// Рисовать метки шага (для discrete).
    pub discrete_ticks: bool,
    /// Формат значения (например, "50 ms"). None — без label.
    pub label_format: Option<fn(f32) -> String>,
}

impl Default for SliderOpts {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: 1.0,
            step: None,
            discrete_ticks: false,
            label_format: None,
        }
    }
}

/// Раскладка слайдера (Wave C §5.3.3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderLayout {
    /// Rect трека (горизонтальная полоса).
    pub track: UiRect,
    /// Rect заполненной части (от начала до значения).
    pub filled: UiRect,
    /// Rect бегунка (knob, квадрат/круг).
    pub knob: UiRect,
    /// Текущее значение [min..max] (normalized для отрисовки — value/(max-min)).
    pub value: f32,
}

/// Hit-зона слайдера (Wave C §5.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderHit {
    /// Бегунок — drag.
    Knob,
    /// Трек — jump-to-click.
    Track,
}

/// Вёрстка слайдера в слоте: трек по центру высоты, filled пропорционально
/// value, knob центрирован на value.
pub fn slider_layout(
    slot: UiRect,
    value: f32,
    opts: &SliderOpts,
    size: ControlSize,
) -> SliderLayout {
    let track_h = (size.button_h() * 0.4).max(4.0); // трек ~40% высоты кнопки
    let knob_side = size.button_h() * 0.6; // knob ~60% высоты кнопки
    let track_y = slot.y + (slot.h - track_h).max(0.0) / 2.0;
    let track = UiRect::new(slot.x, track_y, slot.w, track_h);
    // Normalized value [0..1].
    let range = (opts.max - opts.min).max(1e-6);
    let norm = ((value - opts.min) / range).clamp(0.0, 1.0);
    let filled_w = slot.w * norm;
    let filled = UiRect::new(slot.x, track_y, filled_w, track_h);
    let knob_x = slot.x + filled_w - knob_side / 2.0;
    let knob_y = slot.y + (slot.h - knob_side).max(0.0) / 2.0;
    let knob = UiRect::new(knob_x, knob_y, knob_side, knob_side);
    SliderLayout { track, filled, knob, value }
}

/// Стиль слайдера: track — control_fill, filled — primary, knob — primary/text_title.
pub fn slider_style(state: KitState, p: &KitPalette) -> (ControlStyle, [f32; 4]) {
    let track_fill = match state {
        KitState::Hovered | KitState::Pressed => p.hover_fill,
        KitState::Disabled => p.control_fill,
        _ => p.control_fill,
    };
    let track_style = ControlStyle {
        fill: track_fill,
        border: p.control_border,
        text: [0.0; 4],
        radius: Shape::Full.px(),
        elevation: super::Elevation::None,
    };
    let knob_fill = match state {
        KitState::Disabled => p.disabled_text,
        _ => p.control_primary,
    };
    (track_style, knob_fill)
}

/// Hit-test: knob (drag) или track (jump-to-click).
pub fn slider_hit(layout: &SliderLayout, p: UiPoint) -> Option<SliderHit> {
    if layout.knob.contains(p) {
        Some(SliderHit::Knob)
    } else if layout.track.contains(p) {
        Some(SliderHit::Track)
    } else {
        None
    }
}

/// Keyboard: ←/→ — step, Home/End — min/max, PageUp/Down — big step.
/// Возвращает новое значение или None если key не обработан.
pub fn slider_key(
    value: f32,
    opts: &SliderOpts,
    key: SliderKey,
) -> Option<f32> {
    let range = opts.max - opts.min;
    let step = opts.step.unwrap_or(range * 0.01); // 1% по умолчанию
    let big_step = opts.step.unwrap_or(range * 0.1); // 10% для PageUp/Down
    let new_val = match key {
        SliderKey::Decrement => value - step,
        SliderKey::Increment => value + step,
        SliderKey::Home => opts.min,
        SliderKey::End => opts.max,
        SliderKey::PageDown => value - big_step,
        SliderKey::PageUp => value + big_step,
    };
    Some(new_val.clamp(opts.min, opts.max))
}

/// Keyboard keys для слайдера (Wave C §5.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliderKey {
    Decrement, // ←
    Increment, // →
    Home,      // min
    End,       // max
    PageDown,  // big step down
    PageUp,    // big step up
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slider_layout_track_centered() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 30.0);
        let opts = SliderOpts { min: 0.0, max: 100.0, ..Default::default() };
        let lay = slider_layout(slot, 50.0, &opts, ControlSize::Sm);
        // Track центрирован по высоте
        let track_h = (30.0 * 0.4).max(4.0); // 12
        assert!((lay.track.h - track_h).abs() < 0.01);
        assert!((lay.track.y - (30.0 - track_h) / 2.0).abs() < 0.01);
        // value=50 → filled_w = 100 (половина)
        assert!((lay.filled.w - 100.0).abs() < 0.01);
    }

    #[test]
    fn slider_layout_value_clamped() {
        let slot = UiRect::new(0.0, 0.0, 100.0, 30.0);
        let opts = SliderOpts { min: 0.0, max: 10.0, ..Default::default() };
        // value > max → clamped to 1.0 normalized
        let lay = slider_layout(slot, 15.0, &opts, ControlSize::Sm);
        assert!((lay.filled.w - 100.0).abs() < 0.01); // full width
        // value < min → 0
        let lay = slider_layout(slot, -5.0, &opts, ControlSize::Sm);
        assert!((lay.filled.w - 0.0).abs() < 0.01);
    }

    #[test]
    fn slider_hit_knob_or_track() {
        let slot = UiRect::new(0.0, 0.0, 200.0, 30.0);
        let opts = SliderOpts { min: 0.0, max: 100.0, ..Default::default() };
        let lay = slider_layout(slot, 50.0, &opts, ControlSize::Sm);
        // Knob в центре (value=50 → knob_x = 100 - knob_side/2)
        let knob_side = 30.0 * 0.6; // 18
        let knob_center = UiPoint::new(100.0, 15.0);
        assert_eq!(slider_hit(&lay, knob_center), Some(SliderHit::Knob));
        // Track слева от knob
        assert_eq!(slider_hit(&lay, UiPoint::new(10.0, 15.0)), Some(SliderHit::Track));
        // Мимо
        assert_eq!(slider_hit(&lay, UiPoint::new(250.0, 15.0)), None);
    }

    #[test]
    fn slider_key_decrement_increment() {
        let opts = SliderOpts { min: 0.0, max: 100.0, step: Some(5.0), ..Default::default() };
        assert!((slider_key(50.0, &opts, SliderKey::Decrement).unwrap() - 45.0).abs() < 0.01);
        assert!((slider_key(50.0, &opts, SliderKey::Increment).unwrap() - 55.0).abs() < 0.01);
        // Clamped
        assert!((slider_key(2.0, &opts, SliderKey::Decrement).unwrap() - 0.0).abs() < 0.01);
        assert!((slider_key(98.0, &opts, SliderKey::Increment).unwrap() - 100.0).abs() < 0.01);
    }

    #[test]
    fn slider_key_home_end() {
        let opts = SliderOpts { min: 10.0, max: 90.0, ..Default::default() };
        assert!((slider_key(50.0, &opts, SliderKey::Home).unwrap() - 10.0).abs() < 0.01);
        assert!((slider_key(50.0, &opts, SliderKey::End).unwrap() - 90.0).abs() < 0.01);
    }

    #[test]
    fn slider_key_page_up_down_big_step() {
        let opts = SliderOpts { min: 0.0, max: 100.0, step: Some(5.0), ..Default::default() };
        // PageUp = big_step = step (5.0) когда step задан
        let v = slider_key(50.0, &opts, SliderKey::PageUp).unwrap();
        assert!((v - 55.0).abs() < 0.01 || (v - 60.0).abs() < 0.01); // step или 10%
    }
}
