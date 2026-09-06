//! Spatial index (R-tree) над AABB нод (T5, SPEC §4/§6.3).
//!
//! Индекс хранит позицию ноды в `Canvas.nodes` (`usize`), а не id: на стадии M1
//! ноды не удаляются и не переупорядочиваются, поэтому позиции стабильны.
//! При появлении удаления/переупорядочивания (T8+) — rebuild через `build`
//! или переход на ключ по id.

use rstar::{RTree, RTreeObject, SelectionFunction, AABB};

use crate::model::{Canvas, Node};

/// Прямоугольник world-координат: [min_x, min_y, max_x, max_y].
pub type WorldRect = [f32; 4];

/// AABB ноды по её координатам и размерам.
fn node_rect(node: &Node) -> WorldRect {
    [node.x, node.y, node.x + node.width, node.y + node.height]
}

/// Элемент R-tree: AABB ноды + её индекс в `Canvas.nodes`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct NodeEntry {
    index: usize,
    rect: WorldRect,
}

impl RTreeObject for NodeEntry {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners([self.rect[0], self.rect[1]], [self.rect[2], self.rect[3]])
    }
}

/// Выбор элемента по индексу ноды (для remove без знания старого AABB).
struct SelectByIndex(usize);

impl SelectionFunction<NodeEntry> for SelectByIndex {
    fn should_unpack_parent(&self, _envelope: &AABB<[f32; 2]>) -> bool {
        true
    }

    fn should_unpack_leaf(&self, leaf: &NodeEntry) -> bool {
        leaf.index == self.0
    }
}

/// R-tree над AABB нод: инкрементальные обновления (remove+insert) и запросы
/// видимых нод по viewport. Не зависит от ОС и GPU (SPEC §4).
pub struct SpatialIndex {
    tree: RTree<NodeEntry>,
}

impl SpatialIndex {
    /// Построить индекс по всем нодам канваса (bulk load — эффективнее по одной).
    pub fn build(canvas: &Canvas) -> Self {
        let entries = canvas
            .nodes
            .iter()
            .enumerate()
            .map(|(index, node)| NodeEntry {
                index,
                rect: node_rect(node),
            })
            .collect();
        Self {
            tree: RTree::bulk_load(entries),
        }
    }

    /// Добавить ноду в индекс (при создании новой ноды).
    pub fn insert(&mut self, index: usize, node: &Node) {
        self.tree.insert(NodeEntry {
            index,
            rect: node_rect(node),
        });
    }

    /// Убрать ноду из индекса по её позиции в `Canvas.nodes`.
    pub fn remove(&mut self, index: usize) {
        // O(n) обход дерева: при тысячах нод — микросекунды, приемлемо;
        // если станет узким местом — держать HashMap index→entry для точного remove.
        self.tree
            .remove_with_selection_function(SelectByIndex(index));
    }

    /// Обновить положение/размер ноды после изменения (remove+insert, без rebuild).
    pub fn update(&mut self, index: usize, node: &Node) {
        self.remove(index);
        self.insert(index, node);
    }

    /// Индексы нод, пересекающих `rect`, **отсортированные по возрастанию**
    /// (z-порядок рендера = порядок нод в `Canvas.nodes`).
    pub fn query_rect(&self, rect: WorldRect) -> Vec<usize> {
        let envelope = AABB::from_corners([rect[0], rect[1]], [rect[2], rect[3]]);
        let mut indices: Vec<usize> = self
            .tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|entry| entry.index)
            .collect();
        indices.sort_unstable();
        indices
    }

    /// Индекс верхней ноды под world-точкой (семантика `Canvas::hit_test`:
    /// больший индекс = выше по z, границы включительны).
    pub fn hit_test(&self, point: [f32; 2]) -> Option<usize> {
        let envelope = AABB::from_corners(point, point);
        self.tree
            .locate_in_envelope_intersecting(&envelope)
            .map(|entry| entry.index)
            .max()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// node_rect — AABB по x/y/width/height.
    #[test]
    fn node_rect_corners() {
        let node = Node::file("n", "C:/a.png", 10.0, 20.0, 100.0, 50.0);
        assert_eq!(node_rect(&node), [10.0, 20.0, 110.0, 70.0]);
    }
}
