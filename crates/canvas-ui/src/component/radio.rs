//! Wave C §5.3.4: RadioGroup — exclusive-selection radio buttons.
//!
//! Анатомия: `circle` (круг) + `label` (текст). Группа: exclusive selection.
//! Keyboard: ←/→/↑/↓ navigation (FocusZone pattern).

use super::{ControlSize, ControlStyle, KitPalette, KitState, Shape};
use crate::geometry::{UiPoint, UiRect, UiVec2};
use crate::layout::{stack, HAlign, VAlign};
use crate::measure::TextMeasurer;

/// Ориентация группы radio (Wave C §5.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RadioOrientation {
    #[default]
    Vertical,
    Horizontal,
}

/// Группа radio: управляет exclusive-selection (Wave C §5.3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct RadioGroup {
    pub options: Vec<String>,
    pub selected: usize,
}

impl RadioGroup {
    pub fn new(options: Vec<String>) -> Self {
        Self { options, selected: 0 }
    }
}

/// Раскладка одного radio (Wave C §5.3.4).
#[derive(Debug, Clone, PartialEq)]
pub struct RadioLayout {
    /// Rect круга (radio button).
    pub circle: UiRect,
    /// Rect подписи (измеренная).
    pub label_rect: UiRect,
    /// Подпись после ellipsis.
    pub label: String,
    /// Выбран ли этот option.
    pub selected: bool,
}

/// Вёрстка группы radio: вертикально или горизонтально (Wave C §5.3.4).
pub fn radio_group_layout(
    slot: UiRect,
    group: &RadioGroup,
    orientation: RadioOrientation,
    size: ControlSize,
    gap: f32,
    label_gap: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> Vec<RadioLayout> {
    let circle_d = size.chip_h();
    let mut out = Vec::with_capacity(group.options.len());
    for (i, opt) in group.options.iter().enumerate() {
        let (item_slot, _row_h) = match orientation {
            RadioOrientation::Vertical => {
                let y = slot.y + i as f32 * (circle_d + gap);
                (UiRect::new(slot.x, y, slot.w, circle_d), circle_d)
            }
            RadioOrientation::Horizontal => {
                let label_w = m.width_of(fs, opt, family, font_size) + label_gap + circle_d;
                let x = slot.x + i as f32 * (label_w + gap);
                (UiRect::new(x, slot.y, label_w, circle_d), circle_d)
            }
        };
        let circle = UiRect::new(item_slot.x, item_slot.y, circle_d, circle_d);
        let label_x = circle.right() + label_gap;
        let label_w = (item_slot.right() - label_x).max(0.0);
        let shown = m
            .ellipsis(fs, opt, family, font_size, label_w)
            .unwrap_or_else(|| opt.clone());
        let label_rect = UiRect::new(label_x, item_slot.y, label_w, circle_d);
        out.push(RadioLayout {
            circle,
            label_rect,
            label: shown,
            selected: i == group.selected,
        });
    }
    out
}

/// Стиль radio: selected → primary circle + accent inner dot; unselected → control_fill border.
pub fn radio_style(selected: bool, state: KitState, p: &KitPalette) -> ControlStyle {
    let fill = if selected {
        p.control_primary
    } else {
        [0.0; 4] // прозрачный (только border)
    };
    let fill = match state {
        KitState::Hovered | KitState::Pressed => p.hover_fill,
        _ => fill,
    };
    let border = if selected {
        p.control_primary
    } else {
        p.control_border
    };
    let text = match state {
        KitState::Disabled => p.disabled_text,
        _ => p.text,
    };
    ControlStyle {
        fill,
        border,
        text,
        radius: Shape::Full.px(),
        elevation: super::Elevation::None,
    }
}

/// Hit-test radio: клик по кругу ИЛИ подписи.
pub fn radio_hit(layout: &RadioLayout, p: UiPoint) -> bool {
    layout.circle.contains(p) || layout.label_rect.contains(p)
}

/// Arrow-key навигация по группе (FocusZone pattern, Wave C §5.3.4).
/// Возвращает true если selected изменился.
pub fn radio_group_key(
    group: &mut RadioGroup,
    key: RadioKey,
    orientation: RadioOrientation,
) -> bool {
    let n = group.options.len();
    if n == 0 {
        return false;
    }
    let new_sel = match (key, orientation) {
        (RadioKey::Next, RadioOrientation::Vertical)
        | (RadioKey::Next, RadioOrientation::Horizontal) => {
            Some((group.selected + 1) % n)
        }
        (RadioKey::Prev, RadioOrientation::Vertical)
        | (RadioKey::Prev, RadioOrientation::Horizontal) => {
            Some(if group.selected == 0 { n - 1 } else { group.selected - 1 })
        }
        _ => None,
    };
    if let Some(ns) = new_sel {
        if ns != group.selected {
            group.selected = ns;
            return true;
        }
    }
    false
}

/// Keyboard keys для radio group (Wave C §5.3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadioKey {
    Prev, // ←/↑
    Next, // →/↓
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radio_group_new_default_selected_0() {
        let g = RadioGroup::new(vec!["A".into(), "B".into(), "C".into()]);
        assert_eq!(g.selected, 0);
        assert_eq!(g.options.len(), 3);
    }

    #[test]
    fn radio_group_layout_vertical_stack() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let group = RadioGroup::new(vec!["Local".into(), "Laya".into(), "Ollama".into()]);
        let slot = UiRect::new(0.0, 0.0, 200.0, 300.0);
        let layouts = radio_group_layout(slot, &group, RadioOrientation::Vertical, ControlSize::Sm, 8.0, 8.0, &mut m, &mut fs, "sans", 13.0);
        assert_eq!(layouts.len(), 3);
        // Vertical: y increments by circle_d + gap = 24 + 8 = 32
        assert!((layouts[0].circle.y - 0.0).abs() < 0.01);
        assert!((layouts[1].circle.y - 32.0).abs() < 0.01);
        assert!((layouts[2].circle.y - 64.0).abs() < 0.01);
        // First is selected
        assert!(layouts[0].selected);
        assert!(!layouts[1].selected);
    }

    #[test]
    fn radio_group_key_next_prev() {
        let mut g = RadioGroup::new(vec!["A".into(), "B".into(), "C".into()]);
        assert!(radio_group_key(&mut g, RadioKey::Next, RadioOrientation::Vertical));
        assert_eq!(g.selected, 1);
        assert!(radio_group_key(&mut g, RadioKey::Next, RadioOrientation::Vertical));
        assert_eq!(g.selected, 2);
        // Wrap around
        assert!(radio_group_key(&mut g, RadioKey::Next, RadioOrientation::Vertical));
        assert_eq!(g.selected, 0);
        // Prev from 0 → wrap to last
        assert!(radio_group_key(&mut g, RadioKey::Prev, RadioOrientation::Vertical));
        assert_eq!(g.selected, 2);
    }

    #[test]
    fn radio_hit_circle_or_label() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let group = RadioGroup::new(vec!["Test".into()]);
        let slot = UiRect::new(0.0, 0.0, 200.0, 30.0);
        let layouts = radio_group_layout(slot, &group, RadioOrientation::Vertical, ControlSize::Sm, 8.0, 8.0, &mut m, &mut fs, "sans", 13.0);
        let lay = &layouts[0];
        // Клик по кругу
        assert!(radio_hit(lay, UiPoint::new(5.0, 12.0)));
        // Клик по подписи
        assert!(radio_hit(lay, UiPoint::new(40.0, 12.0)));
        // Мимо
        assert!(!radio_hit(lay, UiPoint::new(250.0, 12.0)));
    }
}
