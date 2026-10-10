//! Wave C §5.3.5: Tabs — панель вкладок с активным индикатором.
//!
//! Анатомия: `bar` (полоса) + `tab`(N) + `indicator` (активный) + `content`.
//! Keyboard: ←/→ navigate tabs, Tab — enter content.

use super::{KitPalette, KitState, Shape};
use crate::geometry::{UiPoint, UiRect};
use crate::measure::TextMeasurer;

/// Стиль табов (Wave C §5.3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabStyle {
    /// Подчёркивание активного (Material 3).
    #[default]
    Underline,
    /// Заливка активного (Carbon).
    Fill,
    /// Pill-активный (Figma UI3).
    Pill,
}

/// Раскладка одного таба (Wave C §5.3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct TabLayout {
    /// Rect таба.
    pub rect: UiRect,
    /// Подпись после ellipsis.
    pub label: String,
    /// Rect индикатора (подчёркивание/заливка/pill).
    pub indicator: UiRect,
    /// Активен ли этот таб.
    pub active: bool,
}

/// Раскладка панели табов (Wave C §5.3.5).
#[derive(Debug, Clone, PartialEq)]
pub struct TabsLayout {
    /// Rect полосы табов.
    pub bar: UiRect,
    /// Сами табы.
    pub tabs: Vec<TabLayout>,
    /// Rect контент-области под табами.
    pub content: UiRect,
}

/// Вёрстка табов: равные ширины (если sum < slot.w) или SqueezeTail (Wave C §5.3.5).
#[allow(clippy::too_many_arguments)] // прецедент kit: layout-функции
pub fn tabs_layout(
    slot: UiRect,
    labels: &[String],
    active: usize,
    style: TabStyle,
    bar_h: f32,
    gap: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> TabsLayout {
    let bar = UiRect::new(slot.x, slot.y, slot.w, bar_h);
    let content = UiRect::new(slot.x, slot.y + bar_h, slot.w, (slot.h - bar_h).max(0.0));
    if labels.is_empty() {
        return TabsLayout {
            bar,
            tabs: Vec::new(),
            content,
        };
    }
    let n = labels.len();
    // Равные ширины: slot.w / n (с учётом gap).
    let gap_total = gap * (n - 1) as f32;
    let tab_w = ((slot.w - gap_total) / n as f32).max(0.0);
    let tabs = labels
        .iter()
        .enumerate()
        .map(|(i, label)| {
            let x = slot.x + i as f32 * (tab_w + gap);
            let rect = UiRect::new(x, slot.y, tab_w, bar_h);
            let shown = m.ellipsis(fs, label, family, font_size, tab_w);
            let indicator = match style {
                TabStyle::Underline => {
                    // Подчёркивание внизу таба, ширина = tab_w * 0.6, центрировано.
                    let ind_w = tab_w * 0.6;
                    let ind_x = x + (tab_w - ind_w) / 2.0;
                    UiRect::new(ind_x, slot.y + bar_h - 2.0, ind_w, 2.0)
                }
                TabStyle::Fill => rect,
                TabStyle::Pill => {
                    // Pill с отступом.
                    let pad = 4.0;
                    UiRect::new(x + pad, slot.y + pad, tab_w - 2.0 * pad, bar_h - 2.0 * pad)
                }
            };
            TabLayout {
                rect,
                label: shown,
                indicator,
                active: i == active,
            }
        })
        .collect();
    TabsLayout { bar, tabs, content }
}

/// Стиль таба: active → accent indicator; hovered → hover_fill.
pub fn tab_style(
    active: bool,
    state: KitState,
    tab_style: TabStyle,
    p: &KitPalette,
) -> TabStyleResult {
    let fill = match state {
        KitState::Hovered | KitState::Pressed => p.hover_fill,
        _ => [0.0; 4],
    };
    let indicator_fill = if active { p.accent } else { [0.0; 4] };
    let text = match state {
        KitState::Disabled => p.disabled_text,
        _ if active => p.text_title,
        _ => p.text,
    };
    let shape = match tab_style {
        TabStyle::Underline | TabStyle::Fill => Shape::None.px(),
        TabStyle::Pill => Shape::L.px(),
    };
    TabStyleResult {
        fill,
        indicator_fill,
        text,
        radius: shape,
    }
}

/// Результат стиля таба (Wave C §5.3.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TabStyleResult {
    pub fill: [f32; 4],
    pub indicator_fill: [f32; 4],
    pub text: [f32; 4],
    pub radius: f32,
}

/// Hit-test таба: возвращает индекс таба или None.
pub fn tabs_hit(layout: &TabsLayout, p: UiPoint) -> Option<usize> {
    layout.tabs.iter().position(|t| t.rect.contains(p))
}

/// Arrow-key навигация: ←/→ по табам. Возвращает новый active или None.
pub fn tabs_key(active: usize, count: usize, key: TabKey) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let new = match key {
        TabKey::Prev => {
            if active == 0 {
                count - 1
            } else {
                active - 1
            }
        }
        TabKey::Next => (active + 1) % count,
    };
    if new != active {
        Some(new)
    } else {
        None
    }
}

/// Keyboard keys для табов (Wave C §5.3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabKey {
    Prev, // ←
    Next, // →
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tabs_layout_equal_widths() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 400.0);
        let labels = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let lay = tabs_layout(
            slot,
            &labels,
            0,
            TabStyle::Underline,
            36.0,
            0.0,
            &mut m,
            &mut fs,
            "sans",
            13.0,
        );
        assert_eq!(lay.tabs.len(), 3);
        // Равные ширины: 300/3 = 100
        assert!((lay.tabs[0].rect.w - 100.0).abs() < 0.01);
        assert!((lay.tabs[1].rect.w - 100.0).abs() < 0.01);
        // Content под баром
        assert!((lay.content.y - 36.0).abs() < 0.01);
        assert!((lay.content.h - 364.0).abs() < 0.01);
        // First is active
        assert!(lay.tabs[0].active);
        assert!(!lay.tabs[1].active);
    }

    #[test]
    fn tabs_layout_empty_labels() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 400.0);
        let lay = tabs_layout(
            slot,
            &[],
            0,
            TabStyle::Underline,
            36.0,
            0.0,
            &mut m,
            &mut fs,
            "sans",
            13.0,
        );
        assert!(lay.tabs.is_empty());
    }

    #[test]
    fn tabs_hit_returns_index() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let slot = UiRect::new(0.0, 0.0, 300.0, 400.0);
        let labels = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let lay = tabs_layout(
            slot,
            &labels,
            0,
            TabStyle::Underline,
            36.0,
            0.0,
            &mut m,
            &mut fs,
            "sans",
            13.0,
        );
        // Click on first tab
        assert_eq!(tabs_hit(&lay, UiPoint::new(10.0, 10.0)), Some(0));
        // Click on second tab (x=100..200)
        assert_eq!(tabs_hit(&lay, UiPoint::new(150.0, 10.0)), Some(1));
        // Click on third tab
        assert_eq!(tabs_hit(&lay, UiPoint::new(250.0, 10.0)), Some(2));
        // Click below bar (content area)
        assert_eq!(tabs_hit(&lay, UiPoint::new(10.0, 100.0)), None);
    }

    #[test]
    fn tabs_key_next_prev() {
        assert_eq!(tabs_key(0, 3, TabKey::Next), Some(1));
        assert_eq!(tabs_key(1, 3, TabKey::Next), Some(2));
        // Wrap
        assert_eq!(tabs_key(2, 3, TabKey::Next), Some(0));
        // Prev
        assert_eq!(tabs_key(1, 3, TabKey::Prev), Some(0));
        assert_eq!(tabs_key(0, 3, TabKey::Prev), Some(2)); // wrap
                                                           // Same → None
        assert_eq!(tabs_key(0, 1, TabKey::Next), None); // only 1 tab
    }
}
