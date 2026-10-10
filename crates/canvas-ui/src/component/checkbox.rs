//! Wave C §5.3.1: Checkbox — двух/трёхсостояющий toggle с подписью.
//!
//! Анатомия: `box` (квадрат) + `check_glyph` (✓/−) + `label` (текст справа).
//! Состояния: 3 (Unchecked/Checked/Indeterminate) × 7 KitState.
//! Keyboard: Space toggles. Touch: hit ≥ 44px (box + label).

use super::{ControlSize, ControlStyle, KitPalette, KitState, Shape};
use crate::geometry::{UiPoint, UiRect, UiVec2};
use crate::layout::{stack, HAlign, VAlign};
use crate::measure::TextMeasurer;

/// Состояние чек-бокса (Wave C §5.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckboxState {
    #[default]
    Unchecked,
    Checked,
    /// Indeterminate — для "select all" когда выбраны не все.
    Indeterminate,
}

/// Глиф для состояния (для отрисовки потребителем).
pub fn checkbox_glyph(state: CheckboxState) -> &'static str {
    match state {
        CheckboxState::Unchecked => "",
        CheckboxState::Checked => "✓",
        CheckboxState::Indeterminate => "−",
    }
}

/// Раскладка чек-бокса: квадрат + подпись (Wave C §5.3.1).
#[derive(Debug, Clone, PartialEq)]
pub struct CheckboxLayout {
    /// Rect квадрата чек-бокса (сторона = size.chip_h() или ControlSize).
    pub box_rect: UiRect,
    /// Rect подписи (измеренная ширина).
    pub label_rect: UiRect,
    /// Подпись после ellipsis (если слот узкий).
    pub label: String,
    /// Текущее состояние.
    pub state: CheckboxState,
}

/// Вёрстка чек-бокса в слоте: квадрат слева, подпись справа с зазором.
///
/// `size` — определяет сторону квадрата (`size.chip_h()`). `label_gap` —
/// зазор между квадратом и подписью (обычно `Spacing::Sm` = 8px).
pub fn checkbox_layout(
    slot: UiRect,
    label: &str,
    state: CheckboxState,
    size: ControlSize,
    label_gap: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> CheckboxLayout {
    let box_side = size.chip_h();
    let box_rect = UiRect::new(slot.x, slot.y + (slot.h - box_side).max(0.0) / 2.0, box_side, box_side);
    let label_x = box_rect.right() + label_gap;
    let label_w = (slot.right() - label_x).max(0.0);
    let shown = if label.is_empty() {
        String::new()
    } else {
        m.ellipsis(fs, label, family, font_size, label_w).unwrap_or_else(|| label.to_owned())
    };
    let label_rect = UiRect::new(label_x, slot.y, label_w, slot.h);
    CheckboxLayout {
        box_rect,
        label_rect,
        label: shown,
        state,
    }
}

/// Стиль чек-бокса: Checked → primary fill + on_primary glyph;
/// Indeterminate → control_fill + accent glyph; Unchecked → control_fill border.
pub fn checkbox_style(
    state: CheckboxState,
    kit_state: KitState,
    p: &KitPalette,
) -> ControlStyle {
    let fill = match state {
        CheckboxState::Checked => p.control_primary,
        CheckboxState::Indeterminate => p.control_fill,
        CheckboxState::Unchecked => p.control_fill,
    };
    let fill = match kit_state {
        KitState::Hovered | KitState::Pressed => p.hover_fill,
        _ => fill,
    };
    let border = match (state, kit_state) {
        (CheckboxState::Unchecked, KitState::Hovered | KitState::Pressed) => p.accent,
        (CheckboxState::Unchecked, _) => p.control_border,
        _ => [0.0; 4],
    };
    let text = match kit_state {
        KitState::Disabled => p.disabled_text,
        _ => match state {
            CheckboxState::Checked => p.text_title,
            _ => p.text,
        },
    };
    ControlStyle {
        fill,
        border,
        text,
        radius: Shape::S.px(),
        elevation: super::Elevation::None,
    }
}

/// Hit-test чек-бокса: клик по квадрату ИЛИ подписи toggles.
pub fn checkbox_hit(layout: &CheckboxLayout, p: UiPoint) -> bool {
    layout.box_rect.contains(p) || layout.label_rect.contains(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkbox_state_default_is_unchecked() {
        assert_eq!(CheckboxState::default(), CheckboxState::Unchecked);
    }

    #[test]
    fn checkbox_glyph_for_each_state() {
        assert_eq!(checkbox_glyph(CheckboxState::Unchecked), "");
        assert_eq!(checkbox_glyph(CheckboxState::Checked), "✓");
        assert_eq!(checkbox_glyph(CheckboxState::Indeterminate), "−");
    }

    #[test]
    fn checkbox_layout_box_on_left_label_on_right() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(10.0, 20.0, 200.0, 30.0);
        let lay = checkbox_layout(slot, "Accept", CheckboxState::Unchecked, ControlSize::Sm, 8.0, &mut m, &mut fs, "sans", 13.0);
        // Box слева: x=10, сторона=24 (chip_h Sm), центрирован по высоте
        assert!((lay.box_rect.x - 10.0).abs() < 0.01);
        assert!((lay.box_rect.w - 24.0).abs() < 0.01);
        assert!((lay.box_rect.h - 24.0).abs() < 0.01);
        // Box центрирован по высоте: y = 20 + (30-24)/2 = 23
        assert!((lay.box_rect.y - 23.0).abs() < 0.01);
        // Label справа с gap=8: x = 10+24+8 = 42
        assert!((lay.label_rect.x - 42.0).abs() < 0.01);
        assert_eq!(lay.state, CheckboxState::Unchecked);
    }

    #[test]
    fn checkbox_hit_box_or_label() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 200.0, 30.0);
        let lay = checkbox_layout(slot, "Test", CheckboxState::Checked, ControlSize::Sm, 8.0, &mut m, &mut fs, "sans", 13.0);
        // Клик по квадрату
        assert!(checkbox_hit(&lay, UiPoint::new(5.0, 15.0)));
        // Клик по подписи
        assert!(checkbox_hit(&lay, UiPoint::new(50.0, 15.0)));
        // Клик мимо
        assert!(!checkbox_hit(&lay, UiPoint::new(250.0, 15.0)));
    }

    #[test]
    fn checkbox_style_checked_uses_primary() {
        let p = KitPalette {
            control_primary: [0.1, 0.2, 0.9, 1.0],
            control_fill: [0.3, 0.3, 0.3, 1.0],
            control_border: [0.5, 0.5, 0.5, 1.0],
            accent: [0.0, 0.5, 1.0, 1.0],
            text: [0.9, 0.9, 0.9, 1.0],
            text_title: [1.0, 1.0, 1.0, 1.0],
            disabled_text: [0.5, 0.5, 0.5, 1.0],
            hover_fill: [0.4, 0.4, 0.4, 1.0],
            ..default_palette()
        };
        let s = checkbox_style(CheckboxState::Checked, KitState::Normal, &p);
        assert_eq!(s.fill, p.control_primary);
        // Indeterminate — control_fill
        let s = checkbox_style(CheckboxState::Indeterminate, KitState::Normal, &p);
        assert_eq!(s.fill, p.control_fill);
    }

    fn default_palette() -> KitPalette {
        KitPalette {
            panel_fill: [0.0; 4],
            panel_border: [0.0; 4],
            control_fill: [0.0; 4],
            control_border: [0.0; 4],
            control_primary: [0.0; 4],
            control_danger: [0.0; 4],
            hover_fill: [0.0; 4],
            primary_hover_fill: [0.0; 4],
            selected_fill: [0.0; 4],
            text: [0.0; 4],
            text_title: [0.0; 4],
            text_muted: [0.0; 4],
            disabled_text: [0.0; 4],
            accent: [0.0; 4],
            control_success: [0.0; 4],
            control_warning: [0.0; 4],
            stage_dim: [0.0; 4],
            scrollbar_thumb: [0.0; 4],
            rule_color: [0.0; 4],
        }
    }
}
