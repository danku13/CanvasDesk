//! Wave A §5.5.4: NavRail — Figma-style navigation rail.
//!
//! Slim left-most vertical rail (~48px) of tabs + dynamic adjacent sidebar
//! (~280px). Decouples navigation from content — 19 поверхностей CanvasDesk
//! за unified rail.

use crate::component::{BadgeKind, BadgeTone, KitPalette, KitState, Shape};
use crate::geometry::{UiPoint, UiRect};

/// Элемент nav-rail (Wave A §5.5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct NavRailItem {
    pub id: String,
    pub icon: &'static str,
    pub label: String,
    pub badge: Option<BadgeKind>,
}

impl NavRailItem {
    pub fn new(id: impl Into<String>, icon: &'static str, label: impl Into<String>) -> Self {
        Self { id: id.into(), icon, label: label.into(), badge: None }
    }

    pub fn badge(mut self, badge: BadgeKind) -> Self {
        self.badge = Some(badge);
        self
    }
}

/// NavRail state (Wave A §5.5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct NavRail {
    pub items: Vec<NavRailItem>,
    pub active: usize,
}

impl NavRail {
    pub fn new(items: Vec<NavRailItem>) -> Self {
        Self { items, active: 0 }
    }
}

/// Раскладка nav-rail (Wave A §5.5.4).
#[derive(Debug, Clone, PartialEq)]
pub struct NavRailLayout {
    /// Rect rail (вертикальная полоса слева, ~48px).
    pub rail: UiRect,
    /// Rect кнопок в rail.
    pub buttons: Vec<UiRect>,
    /// Rect sidebar (dynamic content, ~280px).
    pub sidebar: UiRect,
    /// Индекс активного элемента.
    pub active: usize,
}

/// Вёрстка nav-rail: rail слева (~48px), sidebar справа от rail (~sidebar_w).
pub fn nav_rail_layout(
    viewport: UiRect,
    rail: &NavRail,
    sidebar_w: f32,
    rail_w: f32,
    button_h: f32,
    gap: f32,
) -> NavRailLayout {
    let rail_rect = UiRect::new(viewport.x, viewport.y, rail_w, viewport.h);
    let sidebar = UiRect::new(viewport.x + rail_w, viewport.y, sidebar_w, viewport.h);
    let buttons: Vec<UiRect> = (0..rail.items.len())
        .map(|i| {
            let y = viewport.y + gap + i as f32 * (button_h + gap);
            UiRect::new(viewport.x + 4.0, y, rail_w - 8.0, button_h)
        })
        .collect();
    NavRailLayout { rail: rail_rect, buttons, sidebar, active: rail.active }
}

/// Стиль кнопки nav-rail: active → accent fill; hovered → hover_fill.
pub fn nav_rail_button_style(active: bool, state: KitState, p: &KitPalette) -> crate::component::ControlStyle {
    let fill = if active {
        p.accent
    } else {
        match state {
            KitState::Hovered | KitState::Pressed => p.hover_fill,
            _ => [0.0; 4],
        }
    };
    let text = if active {
        p.text_title
    } else {
        match state {
            KitState::Disabled => p.disabled_text,
            _ => p.text_muted,
        }
    };
    crate::component::ControlStyle {
        fill,
        border: [0.0; 4],
        text,
        radius: Shape::S.px(),
        elevation: crate::component::Elevation::None,
    }
}

/// Hit-test nav-rail: возвращает индекс кнопки или None.
pub fn nav_rail_hit(layout: &NavRailLayout, p: UiPoint) -> Option<usize> {
    layout.buttons.iter().position(|b| b.contains(p))
}

/// Активировать элемент по индексу. Возвращает true если active изменился.
pub fn nav_rail_activate(rail: &mut NavRail, index: usize) -> bool {
    if index < rail.items.len() && index != rail.active {
        rail.active = index;
        true
    } else {
        false
    }
}

/// Активировать элемент по id. Возвращает true если найден и изменён.
pub fn nav_rail_activate_by_id(rail: &mut NavRail, id: &str) -> bool {
    if let Some(i) = rail.items.iter().position(|item| item.id == id) {
        nav_rail_activate(rail, i)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_rail() -> NavRail {
        NavRail::new(vec![
            NavRailItem::new("templates", "▤", "Шаблоны"),
            NavRailItem::new("calc", "∑", "Расчёт"),
            NavRailItem::new("explain", "?", "Объяснение"),
            NavRailItem::new("settings", "⚙", "Настройки"),
        ])
    }

    #[test]
    fn nav_rail_layout_geometry() {
        let viewport = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let rail = seed_rail();
        let lay = nav_rail_layout(viewport, &rail, 280.0, 48.0, 40.0, 8.0);
        // Rail: 48px wide, full height
        assert!((lay.rail.w - 48.0).abs() < 0.01);
        assert!((lay.rail.h - 800.0).abs() < 0.01);
        // Sidebar: 280px, right of rail
        assert!((lay.sidebar.x - 48.0).abs() < 0.01);
        assert!((lay.sidebar.w - 280.0).abs() < 0.01);
        // 4 buttons
        assert_eq!(lay.buttons.len(), 4);
        // First button: y = 8 (gap), h=40, w=40 (rail_w-8)
        assert!((lay.buttons[0].y - 8.0).abs() < 0.01);
        assert!((lay.buttons[0].h - 40.0).abs() < 0.01);
        assert!((lay.buttons[0].w - 40.0).abs() < 0.01);
        // Second button: y = 8 + 40 + 8 = 56
        assert!((lay.buttons[1].y - 56.0).abs() < 0.01);
        assert_eq!(lay.active, 0);
    }

    #[test]
    fn nav_rail_hit() {
        let viewport = UiRect::new(0.0, 0.0, 1280.0, 800.0);
        let rail = seed_rail();
        let lay = nav_rail_layout(viewport, &rail, 280.0, 48.0, 40.0, 8.0);
        // Click first button
        assert_eq!(nav_rail_hit(&lay, UiPoint::new(20.0, 20.0)), Some(0));
        // Click second button (y=56..96)
        assert_eq!(nav_rail_hit(&lay, UiPoint::new(20.0, 70.0)), Some(1));
        // Click outside rail
        assert_eq!(nav_rail_hit(&lay, UiPoint::new(100.0, 20.0)), None);
    }

    #[test]
    fn nav_rail_activate() {
        let mut rail = seed_rail();
        assert!(nav_rail_activate(&mut rail, 1));
        assert_eq!(rail.active, 1);
        // Same index → no change
        assert!(!nav_rail_activate(&mut rail, 1));
        // Out of bounds → no change
        assert!(!nav_rail_activate(&mut rail, 99));
    }

    #[test]
    fn nav_rail_activate_by_id() {
        let mut rail = seed_rail();
        assert!(nav_rail_activate_by_id(&mut rail, "settings"));
        assert_eq!(rail.active, 3);
        assert!(!nav_rail_activate_by_id(&mut rail, "nonexistent"));
    }

    #[test]
    fn nav_rail_item_with_badge() {
        let item = NavRailItem::new("notifications", "🔔", "Уведомления")
            .badge(BadgeKind::Count(3));
        assert!(item.badge.is_some());
    }
}
