//! FR-UI-TREE: 2D tidy-tree layout for hierarchical visualization.
//!
//! Extracts pattern from `explain_ui::layout_tree` — простая tidy-раскладка
//! иерархического дерева (без Reingold-Tilford contour-collision, без Bezier
//! curves; это базовая раскладка «листья внизу, родители по центру детей» —
//! минимальный kit-компонент для деревьев глубиной ≤ 4 уровня с малым числом
//! узлов). Полную раскладку explain_ui (с `LineageTree`/`Visibility`/
//! Beziers/table-cards) НЕ переносим — это специализированная логика explain
//! поверхности; kit отдаёт каноническую раскладку для новых потребителей
//! (дерево настроек, дерево тегов, диаграмма зависимостей).
//!
//! ## Алгоритм
//!
//! 1. **Глубина (BFS от корня, root_idx = 0):** depth[root] = 0,
//!    depth[child] = depth[parent] + 1.
//! 2. **Y-позиция:** `y = slot.y + node_r + depth · level_h` — корень сверху,
//!    листья снизу (паритет explain_ui: корень на уровне 0, листья — глубже).
//! 3. **X-позиция:** листья — слева направо, равномерно в `slot.w`
//!    (`x = slot.x + node_r + i · (slot.w - 2·node_r) / max(1, n_leaves-1)`;
//!    один лист — по центру). Родители — средний X детей (рекурсивно снизу
//!    вверх — дети выходят раньше родителя в post-order DFS).
//! 4. **Рёбра:** для каждого parent-child — `[(parent_x, parent_y),
//!    (child_x, child_y)]` (для отрисовки коннекторов потребителем).
//!
//! Контракт F-8 (PRD-0009 §8): kit НЕ рисует — только геометрия. Узлы
//! потребителем рисуются как круги (радиус `node_r`) через `Painter::rect`
//! с radius = node_r, рёбра — `Painter::rect` полосами или линиями.

use crate::geometry::UiRect;

/// Tree node for layout: `id`, parent index (`None` = root), children indices.
///
/// Инвариант: индексы в `Vec<TreeNode>` — это позиции в массиве `nodes`,
/// переданном в `tree_layout`. `id` — семантический идентификатор (для
/// потребителя, kit его не использует — только индексы).
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: usize,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
}

impl TreeNode {
    /// Convenience: лист — узел без детей.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// Tree layout result: position per node (center x, y) + edge polylines.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TreeLayout {
    /// `(x, y)` центр каждого узла (по индексу в `nodes`).
    pub positions: Vec<(f32, f32)>,
    /// `[(from, to)]` — пара точек для каждого parent-child ребра
    /// (порядок: parent → child; для рисования коннектора).
    pub edges: Vec<[(f32, f32); 2]>,
}

/// Layout a tree in `slot` using a simple tidy-tree algorithm:
/// leaves at bottom row, parents centered above children.
///
/// `nodes` — tree structure (index 0 = root by convention; для пустого
/// массива возвращается пустой `TreeLayout`).
/// `slot` — bounding rect (центр каждого узла — внутри `slot` с отступом
/// `node_r`).
/// `node_r` — радиус узла (для spacing — узлы не ближе `2·node_r` по X).
/// `level_h` — vertical gap between levels (depth·level_h + node_r).
///
/// Returns `TreeLayout` with positions (center per node) + edges.
///
/// ## Алгоритм (см. модуль-документацию)
///
/// 1. BFS от корня → depth[] каждого узла (depth[root]=0).
/// 2. y[node] = slot.y + node_r + depth[node] · level_h.
/// 3. leaves: x = slot.x + node_r + leaf_index · spacing (равномерно в slot.w).
/// 4. parents (post-order DFS): x = средний x детей.
/// 5. edges: [(parent_pos, child_pos)] для каждого parent-child.
pub fn tree_layout(nodes: &[TreeNode], slot: UiRect, node_r: f32, level_h: f32) -> TreeLayout {
    let n = nodes.len();
    let mut layout = TreeLayout {
        positions: vec![(0.0, 0.0); n],
        edges: Vec::with_capacity(n),
    };
    if n == 0 {
        return layout;
    }

    // 1. BFS от корня → depth[] каждого узла.
    let mut depth = vec![usize::MAX; n];
    depth[0] = 0;
    let mut queue: Vec<usize> = vec![0];
    let mut head = 0;
    while head < queue.len() {
        let i = queue[head];
        head += 1;
        for &c in &nodes[i].children {
            if c < n && depth[c] == usize::MAX {
                depth[c] = depth[i].saturating_add(1);
                queue.push(c);
            }
        }
    }

    // 2. Листья в порядке DFS (left-to-right) — им назначаем X равномерно.
    let mut leaves: Vec<usize> = Vec::new();
    dfs_leaves(nodes, 0, &mut leaves);
    let n_leaves = leaves.len();
    let avail_w = (slot.w - 2.0 * node_r).max(0.0);
    let spacing = if n_leaves > 1 {
        avail_w / (n_leaves - 1) as f32
    } else {
        0.0
    };
    for (i, &leaf_idx) in leaves.iter().enumerate() {
        let x = if n_leaves > 1 {
            slot.x + node_r + i as f32 * spacing
        } else {
            // Один лист — по центру слота.
            slot.x + slot.w / 2.0
        };
        let y = slot.y + node_r + depth[leaf_idx] as f32 * level_h;
        layout.positions[leaf_idx] = (x, y);
    }

    // 3. Parents (post-order DFS): x = средний x детей. Дети выходят раньше
    // родителя (post-order), поэтому их позиции уже посчитаны.
    let mut post_order: Vec<usize> = Vec::with_capacity(n);
    dfs_post_order(nodes, 0, &mut post_order);
    for &i in &post_order {
        if nodes[i].is_leaf() {
            continue; // лист — уже назначен в шаге 2
        }
        let children = &nodes[i].children;
        let xs: Vec<f32> = children.iter().map(|&c| layout.positions[c].0).collect();
        if xs.is_empty() {
            // Недостижимо: is_leaf() true для пустых children — но
            // на всякий: позиция = центр слота.
            layout.positions[i] = (slot.x + slot.w / 2.0, slot.y + node_r);
            continue;
        }
        let avg_x = xs.iter().sum::<f32>() / xs.len() as f32;
        let y = slot.y + node_r + depth[i] as f32 * level_h;
        layout.positions[i] = (avg_x, y);
    }

    // 4. Edges: для каждого parent-child — [(parent_pos, child_pos)].
    // Проходим в pre-order (родитель раньше ребёнка), порядок — стабильный.
    let mut pre_order: Vec<usize> = Vec::with_capacity(n);
    dfs_pre_order(nodes, 0, &mut pre_order);
    for &i in &pre_order {
        for &c in &nodes[i].children {
            let p_pos = layout.positions[i];
            let c_pos = layout.positions[c];
            layout.edges.push([p_pos, c_pos]);
        }
    }

    layout
}

/// DFS-оббор листьев в порядке left-to-right (pre-order).
fn dfs_leaves(nodes: &[TreeNode], root: usize, out: &mut Vec<usize>) {
    if root >= nodes.len() {
        return;
    }
    if nodes[root].is_leaf() {
        out.push(root);
        return;
    }
    for &c in &nodes[root].children {
        dfs_leaves(nodes, c, out);
    }
}

/// DFS post-order (дети раньше родителя) — для назначения позиций родителям
/// после детей.
fn dfs_post_order(nodes: &[TreeNode], root: usize, out: &mut Vec<usize>) {
    if root >= nodes.len() {
        return;
    }
    for &c in &nodes[root].children {
        dfs_post_order(nodes, c, out);
    }
    out.push(root);
}

/// DFS pre-order (родитель раньше детей) — для построения рёбер (стабильный
/// порядок: parent → children).
fn dfs_pre_order(nodes: &[TreeNode], root: usize, out: &mut Vec<usize>) {
    if root >= nodes.len() {
        return;
    }
    out.push(root);
    for &c in &nodes[root].children {
        dfs_pre_order(nodes, c, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3-уровневое дерево: root + 2 children + 4 grandchildren → 7 позиций,
    /// 6 рёбер (2 от root + 4 от 2 children).
    ///
    /// ```text
    ///           root (0)
    ///          /        \
    ///       ch1 (1)    ch2 (2)
    ///      /    \      /    \
    ///   gc1(3) gc2(4) gc3(5) gc4(6)
    /// ```
    #[test]
    fn three_level_tree_7_positions_6_edges() {
        let nodes = vec![
            TreeNode {
                id: 0,
                parent: None,
                children: vec![1, 2],
            },
            TreeNode {
                id: 1,
                parent: Some(0),
                children: vec![3, 4],
            },
            TreeNode {
                id: 2,
                parent: Some(0),
                children: vec![5, 6],
            },
            TreeNode {
                id: 3,
                parent: Some(1),
                children: vec![],
            },
            TreeNode {
                id: 4,
                parent: Some(1),
                children: vec![],
            },
            TreeNode {
                id: 5,
                parent: Some(2),
                children: vec![],
            },
            TreeNode {
                id: 6,
                parent: Some(2),
                children: vec![],
            },
        ];
        let slot = UiRect::new(0.0, 0.0, 800.0, 600.0);
        let layout = tree_layout(&nodes, slot, 20.0, 100.0);

        // 7 позиций — по одной на каждый узел.
        assert_eq!(layout.positions.len(), 7);
        // 6 рёбер: 2 от root + 2 от ch1 + 2 от ch2.
        assert_eq!(layout.edges.len(), 6);

        // Y-координаты по уровням: root = node_r = 20, ch = 120, gc = 220.
        assert!(
            (layout.positions[0].1 - 20.0).abs() < 0.01,
            "root y = node_r"
        );
        assert!(
            (layout.positions[1].1 - 120.0).abs() < 0.01,
            "children y = node_r + level_h"
        );
        assert!(
            (layout.positions[3].1 - 220.0).abs() < 0.01,
            "grandchildren y = node_r + 2·level_h"
        );

        // Leaves: 4 листа, равномерно в slot.w = 800, spacing = 800/3 ≈ 266.67.
        let leaf_xs: Vec<f32> = [3, 4, 5, 6]
            .iter()
            .map(|&i| layout.positions[i].0)
            .collect();
        // spacing = (800 - 2·20) / 3 ≈ 253.33; первый x = 20, последний = 780.
        assert!(
            (leaf_xs[0] - 20.0).abs() < 0.5,
            "первый лист слева: {}",
            leaf_xs[0]
        );
        assert!(
            (leaf_xs[3] - 780.0).abs() < 0.5,
            "последний лист справа: {}",
            leaf_xs[3]
        );
        // Равномерный шаг между листьями.
        let step = leaf_xs[1] - leaf_xs[0];
        for w in leaf_xs.windows(2) {
            assert!(
                (w[1] - w[0] - step).abs() < 0.5,
                "равномерный шаг между листьями"
            );
        }

        // Родитель ch1 (idx=1) — средний x детей (idx 3, 4).
        let avg_ch1 = (layout.positions[3].0 + layout.positions[4].0) / 2.0;
        assert!(
            (layout.positions[1].0 - avg_ch1).abs() < 0.01,
            "ch1 x = средний x его детей"
        );
        // Root — средний x ch1 и ch2.
        let avg_root = (layout.positions[1].0 + layout.positions[2].0) / 2.0;
        assert!(
            (layout.positions[0].0 - avg_root).abs() < 0.01,
            "root x = средний x детей"
        );

        // Все позиции внутри slot (с отступом node_r).
        for &(x, y) in &layout.positions {
            assert!(x >= slot.x - 0.5 && x <= slot.right() + 0.5);
            assert!(y >= slot.y - 0.5 && y <= slot.bottom() + 0.5);
        }
    }

    /// Один корень без детей — 1 позиция (центр слота по X, верх по Y),
    /// 0 рёбер.
    #[test]
    fn single_root_one_position_no_edges() {
        let nodes = vec![TreeNode {
            id: 0,
            parent: None,
            children: vec![],
        }];
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let layout = tree_layout(&nodes, slot, 15.0, 80.0);
        assert_eq!(layout.positions.len(), 1);
        assert_eq!(layout.edges.len(), 0);
        // X — центр слота (один лист = центр).
        assert!((layout.positions[0].0 - 200.0).abs() < 0.01);
        // Y = node_r = 15 (depth 0).
        assert!((layout.positions[0].1 - 15.0).abs() < 0.01);
    }

    /// Пустой массив узлов → пустой TreeLayout.
    #[test]
    fn empty_nodes_returns_empty_layout() {
        let nodes: Vec<TreeNode> = vec![];
        let slot = UiRect::new(0.0, 0.0, 400.0, 300.0);
        let layout = tree_layout(&nodes, slot, 15.0, 80.0);
        assert!(layout.positions.is_empty());
        assert!(layout.edges.is_empty());
    }
}
