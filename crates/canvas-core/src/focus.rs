//! Окрестность фокуса (T23, brainstorm-focus): чистый расчёт 1-hop
//! окрестности ноды или связи — кто подсвечивается при включённом
//! режиме фокуса. Владелец данных — приложение (пересчёт на кадр);
//! рендер получает только индексы (см. `canvas_render::FocusView`).
//!
//! Всё — чистые функции над `Canvas`, без ОС/GPU; тестируется юнитами.

use crate::model::Canvas;

/// Семя фокуса: нода (hover/выделение) или связь (выделение линии).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusSeed {
    /// Индекс ноды в `canvas.nodes`.
    Node(usize),
    /// Индекс связи в `canvas.edges`.
    Edge(usize),
}

/// Результат расчёта: подсвечиваемые ноды (семя + соседи) и связи
/// (инцидентные семени). Индексы отсортированы — детерминизм и
/// предсказуемые тесты.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FocusSet {
    pub nodes: Vec<usize>,
    pub edges: Vec<usize>,
}

impl FocusSet {
    /// Пустой набор (нет семени / невалидное) — ничего не подсвечивать.
    pub const fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Нода в наборе подсвеченных?
    pub fn contains_node(&self, index: usize) -> bool {
        self.nodes.binary_search(&index).is_ok()
    }

    /// Связь в наборе подсвеченных?
    pub fn contains_edge(&self, index: usize) -> bool {
        self.edges.binary_search(&index).is_ok()
    }
}

/// 1-hop окрестность семени:
///
/// - `FocusSeed::Node(i)` — сама нода, все связи, у которых она `from`
///   или `to`, и противоположные концы этих связей. Висячее ребро (нет
///   противоположной ноды) подсвечивается, но ноды не добавляет.
/// - `FocusSeed::Edge(i)` — связь и обе её ноды.
///
/// Невалидный индекс — пустой набор, без паники. Сортировка на выходе.
pub fn focus_set(canvas: &Canvas, seed: FocusSeed) -> FocusSet {
    let mut set = FocusSet::empty();
    match seed {
        FocusSeed::Node(index) => {
            let Some(node) = canvas.nodes.get(index) else {
                return set;
            };
            set.nodes.push(index);
            let id = &node.id;
            for (edge_index, edge) in canvas.edges.iter().enumerate() {
                // Инцидентность в любом направлении
                let other = if edge.from_node == *id {
                    &edge.to_node
                } else if edge.to_node == *id {
                    &edge.from_node
                } else {
                    continue;
                };
                set.edges.push(edge_index);
                // Противоположный конец — по id (висячие рёбра молча
                // пропускаются: подсвечиваем только существующее)
                if let Some(pos) = canvas.nodes.iter().position(|n| &n.id == other) {
                    set.nodes.push(pos);
                }
            }
        }
        FocusSeed::Edge(index) => {
            let Some(edge) = canvas.edges.get(index) else {
                return set;
            };
            set.edges.push(index);
            for id in [&edge.from_node, &edge.to_node] {
                if let Some(pos) = canvas.nodes.iter().position(|n| &n.id == id) {
                    set.nodes.push(pos);
                }
            }
        }
    }
    sort_dedup(&mut set.nodes);
    sort_dedup(&mut set.edges);
    set
}

/// Сортировка + дедуп (кратные связи одной пары дают дубли соседей).
fn sort_dedup(values: &mut Vec<usize>) {
    values.sort_unstable();
    values.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};

    /// Сцена: хаб h (рёбра к a, b, цепочка h→c→d), изолированная нода solo,
    /// висячее ребро h→ghost (ghost-ноды нет), дубль пары a↔h.
    fn scene() -> Canvas {
        let mut canvas = Canvas::default();
        for (id, x) in [
            ("h", 0.0),
            ("a", 100.0),
            ("b", 200.0),
            ("c", 300.0),
            ("d", 400.0),
        ] {
            canvas.nodes.push(Node::text(id, id, x, 0.0));
        }
        canvas.nodes.push(Node::text("solo", "solo", 500.0, 0.0));
        let e = |id: &str, from: &str, to: &str| Edge::new(id, from, None, to, None);
        canvas.add_edge(e("e-ha", "h", "a"));
        canvas.add_edge(e("e-hb", "h", "b"));
        canvas.add_edge(e("e-cd", "c", "d"));
        canvas.add_edge(e("e-hc", "h", "c"));
        // Дубль пары h↔a
        canvas.add_edge(e("e-ah", "a", "h"));
        // Висячее: ghost-ноды нет
        canvas.add_edge(e("e-hghost", "h", "ghost"));
        canvas
    }

    /// Хаб: сама нода + соседи a, b, c; все инцидентные рёбра, включая
    /// дубль и висячее; d (2-hop) и solo не входят.
    #[test]
    fn node_seed_hub_one_hop() {
        let canvas = scene();
        let set = focus_set(&canvas, FocusSeed::Node(0));
        assert_eq!(set.nodes, vec![0, 1, 2, 3], "h + a + b + c");
        assert_eq!(
            set.edges,
            vec![0, 1, 3, 4, 5],
            "ha, hb, hc, дубль ah, висячее"
        );
        assert!(!set.contains_node(4), "d — 2-hop, не входит");
        assert!(!set.contains_node(5), "solo не входит");
    }

    /// Концевая нода a: соседи h (двумя рёбрами — дедуп) и никто больше.
    #[test]
    fn node_seed_dedup_neighbors() {
        let canvas = scene();
        let set = focus_set(&canvas, FocusSeed::Node(1));
        assert_eq!(set.nodes, vec![0, 1], "a + h (дедуп дубля)");
        assert_eq!(set.edges, vec![0, 4], "e-ha и e-ah");
    }

    /// Изолированная нода: только она, рёбер нет.
    #[test]
    fn node_seed_isolated() {
        let canvas = scene();
        let set = focus_set(&canvas, FocusSeed::Node(5));
        assert_eq!(set.nodes, vec![5]);
        assert!(set.edges.is_empty());
    }

    /// Незнакомая связь c→d: ребро + оба конца; прочее не подсвечено.
    #[test]
    fn edge_seed_two_ends() {
        let canvas = scene();
        let set = focus_set(&canvas, FocusSeed::Edge(2));
        assert_eq!(set.nodes, vec![3, 4], "c + d");
        assert_eq!(set.edges, vec![2]);
    }

    /// Висячее ребро h→ghost: ребро подсвечено, нода h входит,
    /// несуществующий конец — нет.
    #[test]
    fn edge_seed_dangling_keeps_existing_end() {
        let canvas = scene();
        let set = focus_set(&canvas, FocusSeed::Edge(5));
        assert_eq!(set.nodes, vec![0], "только существующий h");
        assert_eq!(set.edges, vec![5]);
    }

    /// Невалидные индексы — пустой набор без паники.
    #[test]
    fn invalid_seed_is_empty() {
        let canvas = scene();
        assert_eq!(focus_set(&canvas, FocusSeed::Node(99)), FocusSet::empty());
        assert_eq!(focus_set(&canvas, FocusSeed::Edge(99)), FocusSet::empty());
    }

    /// Пустой канвас — пустой набор.
    #[test]
    fn empty_canvas_is_empty() {
        let canvas = Canvas::default();
        assert_eq!(focus_set(&canvas, FocusSeed::Node(0)), FocusSet::empty());
    }
}
