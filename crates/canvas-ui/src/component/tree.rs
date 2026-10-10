//! Wave C §5.3.11: Tree — иерархический список (expand/collapse).
//!
//! Анатамия: `rows` (flatten visible nodes) с `indent`, `chevron`, `icon`, `label`.
//! Keyboard: ↑/↓ navigate, ←/→ expand/collapse, Enter activate.

use crate::geometry::{UiPoint, UiRect};
use crate::measure::TextMeasurer;

/// Узел дерева (Wave C §5.3.11). Рекурсивный.
#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub children: Vec<TreeNode>,
    pub expanded: bool,
    pub depth: usize,
    pub icon: Option<&'static str>,
}

impl TreeNode {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            children: Vec::new(),
            expanded: false,
            depth: 0,
            icon: None,
        }
    }

    pub fn child(mut self, node: TreeNode) -> Self {
        self.children.push(node);
        self
    }

    pub fn expanded(mut self, v: bool) -> Self {
        self.expanded = v;
        self
    }
}

/// Строка дерева в flatten-раскладке (Wave C §5.3.11).
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    pub rect: UiRect,
    /// Отступ слева (depth * indent_step).
    pub indent: f32,
    /// Rect шеврона (▶/▼) — если has_children.
    pub chevron: UiRect,
    /// Rect иконки (если есть).
    pub icon: Option<UiRect>,
    /// Rect подписи.
    pub label: UiRect,
    /// Подпись (после ellipsis).
    pub label_text: String,
    pub depth: usize,
    pub expanded: bool,
    pub has_children: bool,
    pub id: String,
}

/// Раскладка дерева: flatten visible nodes (Wave C §5.3.11).
pub fn tree_layout(
    slot: UiRect,
    root: &TreeNode,
    row_h: f32,
    indent_step: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    let mut y = slot.y;
    flatten(root, slot.x, &mut y, slot.w, row_h, indent_step, m, fs, family, font_size, &mut rows);
    rows
}

fn flatten(
    node: &TreeNode,
    x: f32,
    y: &mut f32,
    slot_w: f32,
    row_h: f32,
    indent_step: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
    out: &mut Vec<TreeRow>,
) {
    let indent = node.depth as f32 * indent_step;
    let chevron_w = if !node.children.is_empty() { 16.0 } else { 0.0 };
    let icon_w = if node.icon.is_some() { 20.0 } else { 0.0 };
    let chevron_x = x + indent;
    let icon_x = chevron_x + chevron_w + 4.0;
    let label_x = icon_x + icon_w + (if icon_w > 0.0 { 4.0 } else { 0.0 });
    let label_w = (slot_w - (label_x - x)).max(0.0);
    let label_text = m
        .ellipsis(fs, &node.label, family, font_size, label_w)
        .unwrap_or_else(|| node.label.clone());
    let chevron = if !node.children.is_empty() {
        UiRect::new(chevron_x, *y, chevron_w, row_h)
    } else {
        UiRect::default()
    };
    let icon = node.icon.map(|_| UiRect::new(icon_x, *y, icon_w, row_h));
    out.push(TreeRow {
        rect: UiRect::new(x, *y, slot_w, row_h),
        indent,
        chevron,
        icon,
        label: UiRect::new(label_x, *y, label_w, row_h),
        label_text,
        depth: node.depth,
        expanded: node.expanded,
        has_children: !node.children.is_empty(),
        id: node.id.clone(),
    });
    *y += row_h;
    if node.expanded {
        for child in &node.children {
            flatten(child, x, y, slot_w, row_h, indent_step, m, fs, family, font_size, out);
        }
    }
}

/// Action при keyboard-навигации (Wave C §5.3.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeAction {
    FocusNext,
    FocusPrev,
    Expand,
    Collapse,
    Activate,
    None,
}

/// Hit-test: возвращает id строки по точке.
pub fn tree_hit(rows: &[TreeRow], p: UiPoint) -> Option<&str> {
    rows.iter().find(|r| r.rect.contains(p)).map(|r| r.id.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_flatten_expanded() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let root = TreeNode::new("root", "Root")
            .expanded(true)
            .child(TreeNode::new("c1", "Child 1"))
            .child(TreeNode::new("c2", "Child 2").expanded(true)
                .child(TreeNode::new("g1", "Grandchild 1")));
        let slot = UiRect::new(0.0, 0.0, 300.0, 500.0);
        let rows = tree_layout(slot, &root, 26.0, 16.0, &mut m, &mut fs, "sans", 13.0);
        // root + c1 + c2 + g1 = 4 rows
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].id, "root");
        assert_eq!(rows[1].id, "c1");
        assert_eq!(rows[2].id, "c2");
        assert_eq!(rows[3].id, "g1");
        // Indent: root=0, c1=16, c2=16, g1=32
        assert!((rows[0].indent - 0.0).abs() < 0.01);
        assert!((rows[1].indent - 16.0).abs() < 0.01);
        assert!((rows[3].indent - 32.0).abs() < 0.01);
    }

    #[test]
    fn tree_flatten_collapsed() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let root = TreeNode::new("root", "Root")
            .expanded(false) // collapsed
            .child(TreeNode::new("c1", "Child 1"));
        let slot = UiRect::new(0.0, 0.0, 300.0, 500.0);
        let rows = tree_layout(slot, &root, 26.0, 16.0, &mut m, &mut fs, "sans", 13.0);
        // Only root (children скрыты)
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "root");
    }

    #[test]
    fn tree_hit_returns_id() {
        let mut m = TextMeasurer::new();
        let mut fs = cosmic_text::FontSystem::new();
        let root = TreeNode::new("root", "Root").expanded(true)
            .child(TreeNode::new("c1", "Child 1"));
        let slot = UiRect::new(0.0, 0.0, 300.0, 500.0);
        let rows = tree_layout(slot, &root, 26.0, 16.0, &mut m, &mut fs, "sans", 13.0);
        // Click on first row (y=0..26)
        assert_eq!(tree_hit(&rows, UiPoint::new(10.0, 10.0)), Some("root"));
        // Click on second row (y=26..52)
        assert_eq!(tree_hit(&rows, UiPoint::new(10.0, 30.0)), Some("c1"));
        // Click below
        assert_eq!(tree_hit(&rows, UiPoint::new(10.0, 100.0)), None);
    }
}
