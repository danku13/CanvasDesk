//! Wave C §5.3.12: Snackbar — пассивное уведомление с optional action.
//!
//! Анатомия: `message` + optional `action_button`. Bottom-center.
//! Auto-dismiss по ttl_ms. Queue — стакаются вертикально.

use super::{KitPalette, Shape};
use crate::geometry::UiRect;
use crate::kit::BadgeTone;
use crate::measure::TextMeasurer;

/// Snackbar (Wave C §5.3.12).
#[derive(Debug, Clone, PartialEq)]
pub struct Snackbar {
    pub message: String,
    pub action_label: Option<String>,
    pub ttl_ms: u64,
    pub tone: BadgeTone,
}

impl Default for Snackbar {
    fn default() -> Self {
        Self {
            message: String::new(),
            action_label: None,
            ttl_ms: 3000,
            tone: BadgeTone::Default,
        }
    }
}

/// Раскладка snackbar (Wave C §5.3.12).
#[derive(Debug, Clone, PartialEq)]
pub struct SnackbarLayout {
    /// Rect всего snackbar (bottom-center, pill).
    pub rect: UiRect,
    /// Rect сообщения.
    pub message_rect: UiRect,
    /// Rect action-кнопки (если есть).
    pub action_rect: Option<UiRect>,
    /// Сообщение (после ellipsis).
    pub label: String,
    /// Подпись action-кнопки.
    pub action_label: Option<String>,
}

/// Вёрстка snackbar: bottom-center, pill shape, message + optional action.
#[allow(clippy::too_many_arguments)] // прецедент kit: layout-функции
pub fn snackbar_layout(
    viewport: UiRect,
    snack: &Snackbar,
    bar_h: f32,
    pad_h: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> SnackbarLayout {
    let msg_w = m.width_of(fs, &snack.message, family, font_size);
    let action_w = snack
        .action_label
        .as_ref()
        .map(|l| m.width_of(fs, l, family, font_size) + 2.0 * pad_h)
        .unwrap_or(0.0);
    let gap = if snack.action_label.is_some() {
        pad_h
    } else {
        0.0
    };
    let total_w = msg_w + 2.0 * pad_h + gap + action_w;
    // Bottom-center, с отступом 24px от низа.
    let x = viewport.x + (viewport.w - total_w).max(0.0) / 2.0;
    let y = viewport.bottom() - bar_h - 24.0;
    let rect = UiRect::new(x, y, total_w, bar_h);
    let message_rect = UiRect::new(x + pad_h, y, msg_w, bar_h);
    let action_rect = if snack.action_label.is_some() {
        Some(UiRect::new(x + pad_h + msg_w + gap, y, action_w, bar_h))
    } else {
        None
    };
    SnackbarLayout {
        rect,
        message_rect,
        action_rect,
        label: snack.message.clone(),
        action_label: snack.action_label.clone(),
    }
}

/// Стиль snackbar: tone → fill, text — on-fill.
pub fn snackbar_style(tone: BadgeTone, p: &KitPalette) -> super::ControlStyle {
    let fill = match tone {
        BadgeTone::Default => p.panel_fill,
        BadgeTone::Primary => p.control_primary,
        BadgeTone::Success => p.control_success,
        BadgeTone::Warning => p.control_warning,
        BadgeTone::Danger => p.control_danger,
        BadgeTone::Info => p.accent,
    };
    super::ControlStyle {
        fill,
        border: [0.0; 4],
        text: p.text_title,
        radius: Shape::L.px(),
        elevation: super::Elevation::Md,
    }
}

/// Queue snackbar-ов: стакаются вертикально bottom-center.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SnackbarQueue {
    /// (snack, expire_time_ms) — expire = время когда убрать.
    pub items: Vec<(Snackbar, u64)>,
}

impl SnackbarQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Добавить snackbar. `now_ms` — текущее время.
    pub fn push(&mut self, snack: Snackbar, now_ms: u64) {
        let expire = now_ms + snack.ttl_ms;
        self.items.push((snack, expire));
    }

    /// Удалить истёкшие. `now_ms` — текущее время.
    pub fn tick(&mut self, now_ms: u64) {
        self.items.retain(|(_, expire)| *expire > now_ms);
    }

    /// Количество активных snackbar-ов.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snackbar_default() {
        let s = Snackbar::default();
        assert_eq!(s.ttl_ms, 3000);
        assert_eq!(s.tone, BadgeTone::Default);
        assert!(s.action_label.is_none());
    }

    #[test]
    fn snackbar_layout_bottom_center() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let viewport = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let snack = Snackbar {
            message: "Saved".into(),
            ..Default::default()
        };
        let lay = snackbar_layout(viewport, &snack, 36.0, 12.0, &mut m, &mut fs, "sans", 13.0);
        // Bottom: y = 600 - 36 - 24 = 540
        assert!((lay.rect.y - 540.0).abs() < 0.01);
        assert!(lay.action_rect.is_none());
    }

    #[test]
    fn snackbar_layout_with_action() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let viewport = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let snack = Snackbar {
            message: "Deleted".into(),
            action_label: Some("Undo".into()),
            ..Default::default()
        };
        let lay = snackbar_layout(viewport, &snack, 36.0, 12.0, &mut m, &mut fs, "sans", 13.0);
        assert!(lay.action_rect.is_some());
    }

    #[test]
    fn snackbar_queue_push_tick() {
        let mut q = SnackbarQueue::new();
        assert!(q.is_empty());
        let s = Snackbar {
            message: "Test".into(),
            ttl_ms: 1000,
            ..Default::default()
        };
        q.push(s, 0);
        assert_eq!(q.len(), 1);
        // Not expired at t=500
        q.tick(500);
        assert_eq!(q.len(), 1);
        // Expired at t=1001
        q.tick(1001);
        assert_eq!(q.len(), 0);
    }
}
