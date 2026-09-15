//! FR-010: авто-раскладка связанных карточек — чистые функции над моделью
//! канваса (без GPU/ОС, тестируется везде).
//!
//! Обход — BFS от ноды-семени по рёбрам с ЗНАКОВОЙ глубиной (по `id`,
//! образец `focus::focus_set`): исходящие рёбра ведут на уровни «от семени»
//! (+1, +2, …), входящие — «к семени» (−1, −2, …) — топология
//! лево/право и верх/низ сохраняется (пример проверки FR-010: цепочка
//! a–b–c при семени b → a слева, c справа). Защита от циклов (`visited`),
//! детерминизм: соседи сортируются по `id` (`BTreeSet`), план сортируется
//! по индексу ноды. Схемы: tree-горизонтально (родители слева, дети
//! справа), tree-вертикально (родители сверху, дети снизу), radial
//! (корень в центре, радиус по |уровню|).
//!
//! Семя остаётся на месте (якорь), уровни центрируются на его центре.
//! Группы в плане едут вместе с геометрическими детьми (инвариант
//! `translate_group`: каждая нода сдвигается ровно один раз).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use crate::model::{group_children, Canvas, NodeKind};

/// Схема раскладки связанных карточек (уточнение владельца FR-010).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    /// Уровни — колонками: родители слева от семени, дети справа.
    TreeHorizontal,
    /// Уровни — строками: родители сверху, дети снизу.
    TreeVertical,
    /// Корень в центре, уровни — по кругу (радиус по модулю уровня).
    Radial,
}

/// Зазор между колонками/строками уровней (world px).
pub const LEVEL_GAP: f32 = 80.0;
/// Зазор между соседями одного уровня (world px).
pub const SIBLING_GAP: f32 = 32.0;
/// Шаг радиуса за уровень глубины в radial-режиме (world px).
pub const RADIAL_RING_STEP: f32 = 260.0;

/// План раскладки: целевые top-left позиции нод (индекс → позиция).
/// Семя в план не входит (остаётся на месте — якорь).
pub type LayoutPlan = Vec<(usize, [f32; 2])>;

/// Построить план авто-раскладки связанной окрестности `seed`.
/// Ноды без связи с семенем не затрагиваются; невалидный seed/изолированная
/// нода — пустой план. Детерминирован: обход и порядок внутри уровней
/// фиксированы (сортировка соседей по `id`).
pub fn plan_related_layout(canvas: &Canvas, seed: usize, mode: LayoutMode) -> LayoutPlan {
    let Some(seed_node) = canvas.nodes.get(seed) else {
        return Vec::new();
    };

    // id → индекс (для рёбер; висячие концы игнорируются)
    let index_of: HashMap<&str, usize> = canvas
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Направленные списки смежности: out — исходящие рёбра (дети),
    // in — входящие (родители). Сортировка по id — детерминизм.
    let mut outgoing: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut incoming: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for edge in &canvas.edges {
        let (Some(from), Some(to)) = (
            index_of.get(edge.from_node.as_str()),
            index_of.get(edge.to_node.as_str()),
        ) else {
            continue;
        };
        if from == to {
            continue;
        }
        outgoing.entry(*from).or_default().insert(*to);
        incoming.entry(*to).or_default().insert(*from);
    }

    // BFS от семени со ЗНАКОВОЙ глубиной: дети (+1, +2, …), родители
    // (−1, −2, …). Порядок обхода — детерминированный (сортировка по id).
    let mut level: HashMap<usize, i32> = HashMap::new();
    let mut order: Vec<usize> = Vec::new();
    let mut queue: VecDeque<usize> = VecDeque::new();
    level.insert(seed, 0);
    order.push(seed);
    queue.push_back(seed);
    while let Some(node) = queue.pop_front() {
        let l = level[&node];
        // Дети — на уровень дальше от семени, родители — ближе
        let children = outgoing.get(&node).into_iter().flatten();
        let parents = incoming.get(&node).into_iter().flatten();
        for (next, next_level) in children
            .map(|&n| (n, l + 1))
            .chain(parents.map(|&n| (n, l - 1)))
        {
            if level.contains_key(&next) {
                continue;
            }
            level.insert(next, next_level);
            order.push(next);
            queue.push_back(next);
        }
    }
    // Изолированное семя (без связей) — раскладывать нечего
    if order.len() == 1 {
        return Vec::new();
    }

    let (seed_cx, seed_cy) = (
        seed_node.x + seed_node.width / 2.0,
        seed_node.y + seed_node.height / 2.0,
    );

    // Целевые позиции достигнутых нод (семя — якорь, без перемещения)
    let mut target: HashMap<usize, [f32; 2]> = HashMap::new();
    match mode {
        LayoutMode::TreeHorizontal => {
            // Колонка = уровень (знаковый); уровни правее семени — x после
            // максимума ширины предыдущего, левее — перед ним; y — стек по
            // порядку обхода, колонка центрирована на seed_cy
            let mut columns: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
            for &node in &order {
                columns.entry(level[&node]).or_default().push(node);
            }
            // Смещения уровней: x(L) для L>0 — кумулятивно вправо,
            // для L<0 — кумулятивно влево; уровень 0 (семя) — на месте
            let max_width = |lv: i32| -> f32 {
                columns
                    .get(&lv)
                    .map(|col| {
                        col.iter()
                            .map(|&n| canvas.nodes[n].width)
                            .fold(0.0f32, f32::max)
                    })
                    .unwrap_or(0.0)
            };
            let mut offsets: HashMap<i32, f32> = HashMap::new();
            let mut x_right = seed_node.x + seed_node.width + LEVEL_GAP;
            for lv in 1..=*columns.keys().max().unwrap_or(&0) {
                offsets.insert(lv, x_right);
                x_right += max_width(lv) + LEVEL_GAP;
            }
            let mut x_left = seed_node.x - LEVEL_GAP;
            for lv in (*columns.keys().min().unwrap_or(&0)..=-1).rev() {
                x_left -= max_width(lv);
                offsets.insert(lv, x_left);
                x_left -= LEVEL_GAP;
            }
            for (lv, column) in columns {
                if lv == 0 {
                    continue;
                }
                let total: f32 = column
                    .iter()
                    .map(|&n| canvas.nodes[n].height + SIBLING_GAP)
                    .sum::<f32>()
                    - SIBLING_GAP;
                let mut y = seed_cy - total / 2.0;
                let x = offsets[&lv];
                for &n in &column {
                    target.insert(n, [x, y]);
                    y += canvas.nodes[n].height + SIBLING_GAP;
                }
            }
        }
        LayoutMode::TreeVertical => {
            // Строка = уровень (знаковый); дети ниже семени, родители выше;
            // x — стек по порядку обхода, строка центрирована на seed_cx
            let mut rows: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
            for &node in &order {
                rows.entry(level[&node]).or_default().push(node);
            }
            let max_height = |lv: i32| -> f32 {
                rows.get(&lv)
                    .map(|row| {
                        row.iter()
                            .map(|&n| canvas.nodes[n].height)
                            .fold(0.0f32, f32::max)
                    })
                    .unwrap_or(0.0)
            };
            let mut offsets: HashMap<i32, f32> = HashMap::new();
            let mut y_down = seed_node.y + seed_node.height + LEVEL_GAP;
            for lv in 1..=*rows.keys().max().unwrap_or(&0) {
                offsets.insert(lv, y_down);
                y_down += max_height(lv) + LEVEL_GAP;
            }
            let mut y_up = seed_node.y - LEVEL_GAP;
            for lv in (*rows.keys().min().unwrap_or(&0)..=-1).rev() {
                y_up -= max_height(lv);
                offsets.insert(lv, y_up);
                y_up -= LEVEL_GAP;
            }
            for (lv, row) in rows {
                if lv == 0 {
                    continue;
                }
                let total: f32 = row
                    .iter()
                    .map(|&n| canvas.nodes[n].width + SIBLING_GAP)
                    .sum::<f32>()
                    - SIBLING_GAP;
                let mut x = seed_cx - total / 2.0;
                let y = offsets[&lv];
                for &n in &row {
                    target.insert(n, [x, y]);
                    x += canvas.nodes[n].width + SIBLING_GAP;
                }
            }
        }
        LayoutMode::Radial => {
            // Кольцо = |уровень|; радиус растёт с модулем уровня, узлы
            // распределяются по кругу в порядке обхода (первый — сверху).
            // Родители (−N) и дети (+N) делят одно кольцо — иначе кольца
            // с одинаковым радиусом рисовали бы узлы в одних точках
            let mut rings: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
            for &node in &order {
                let lv = level[&node];
                if lv != 0 {
                    rings.entry(lv.unsigned_abs()).or_default().push(node);
                }
            }
            for (ring_depth, ring) in rings {
                let radius = ring_depth as f32 * RADIAL_RING_STEP;
                let count = ring.len() as f32;
                let step = std::f32::consts::TAU / count;
                for (k, &n) in ring.iter().enumerate() {
                    let angle = -std::f32::consts::FRAC_PI_2 + k as f32 * step;
                    let cx = seed_cx + angle.cos() * radius;
                    let cy = seed_cy + angle.sin() * radius;
                    let node = &canvas.nodes[n];
                    target.insert(n, [cx - node.width / 2.0, cy - node.height / 2.0]);
                }
            }
        }
    }

    // Группы в плане едут вместе с геометрическими детьми (translate_group
    // без мутации): дельта группы распространяется на детей, не вошедших
    // в план по рёбрам. Вложенные группы обрабатываются той же очередью —
    // каждая нода получает дельту ровно один раз.
    let mut queue: VecDeque<usize> = target.keys().copied().collect();
    while let Some(node) = queue.pop_front() {
        if canvas.nodes.get(node).map(|n| n.kind()) != Some(NodeKind::Group) {
            continue;
        }
        let (Some(old), Some(&new)) = (
            canvas.nodes.get(node).map(|n| [n.x, n.y]),
            target.get(&node),
        ) else {
            continue;
        };
        let delta = [new[0] - old[0], new[1] - old[1]];
        for child in group_children(canvas, node) {
            if target.contains_key(&child) || child == seed {
                continue;
            }
            let Some(c) = canvas.nodes.get(child) else {
                continue;
            };
            target.insert(child, [c.x + delta[0], c.y + delta[1]]);
            queue.push_back(child);
        }
    }

    // Стабильный порядок применения — по индексу ноды
    let mut plan: LayoutPlan = target.into_iter().collect();
    plan.sort_by_key(|(index, _)| *index);
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};

    fn chain_canvas() -> Canvas {
        // a — b — c, ветвь b — d; e — без связей (не должен двигаться)
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 300.0, 300.0));
        canvas.nodes.push(Node::text("c", "c", 900.0, 900.0));
        canvas.nodes.push(Node::text("d", "d", 1200.0, 0.0));
        canvas.nodes.push(Node::text("e", "e", 1500.0, 1500.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        canvas.edges.push(Edge::new("e2", "b", None, "c", None));
        // Ветвление b–d (по примеру проверки FR-010: d — справа/снизу)
        canvas.edges.push(Edge::new("e3", "b", None, "d", None));
        canvas
    }

    fn overlaps(canvas: &Canvas, plan: &LayoutPlan) -> bool {
        let rects: Vec<[f32; 4]> = plan
            .iter()
            .filter_map(|(i, [x, y])| canvas.nodes.get(*i).map(|n| [*x, *y, n.width, n.height]))
            .collect();
        for i in 0..rects.len() {
            for j in (i + 1)..rects.len() {
                let [ax, ay, aw, ah] = rects[i];
                let [bx, by, bw, bh] = rects[j];
                if ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn tree_horizontal_chain() {
        let canvas = chain_canvas();
        let plan = plan_related_layout(&canvas, 1, LayoutMode::TreeHorizontal);
        // e не связан с b — в плане нет; семя b не двигается
        assert_eq!(plan.len(), 3, "{plan:?}");
        assert!(plan.iter().all(|(i, _)| *i != 1 && *i != 4));
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        let b = &canvas.nodes[1];
        let (a, c, d) = (by_id[&0], by_id[&2], by_id[&3]);
        // a — колонка слева от семени, c и d — справа
        assert!(a[0] < b.x, "a левее b");
        assert!(c[0] > b.x + b.width, "c правее b");
        assert!(d[0] > b.x + b.width, "d правее b");
        // c и d в одной колонке
        assert!((c[0] - d[0]).abs() < f32::EPSILON);
        // Без пересечений bbox
        assert!(!overlaps(&canvas, &plan), "{plan:?}");
    }

    #[test]
    fn tree_vertical_chain() {
        let canvas = chain_canvas();
        let plan = plan_related_layout(&canvas, 1, LayoutMode::TreeVertical);
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        let b = &canvas.nodes[1];
        let (a, c, d) = (by_id[&0], by_id[&2], by_id[&3]);
        assert!(a[1] < b.y, "a выше b");
        assert!(c[1] > b.y + b.height, "c ниже b");
        assert!((c[1] - d[1]).abs() < f32::EPSILON, "c и d в одной строке");
        assert!(!overlaps(&canvas, &plan));
    }

    #[test]
    fn radial_rings() {
        let canvas = chain_canvas();
        let plan = plan_related_layout(&canvas, 1, LayoutMode::Radial);
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        let b = &canvas.nodes[1];
        let b_c = [b.x + b.width / 2.0, b.y + b.height / 2.0];
        // Все соседи на первом кольце: одинаковое расстояние центров от семени
        let center_dist = |i: usize| -> f32 {
            let [x, y] = by_id[&i];
            let n = &canvas.nodes[i];
            let cx = x + n.width / 2.0;
            let cy = y + n.height / 2.0;
            ((cx - b_c[0]).powi(2) + (cy - b_c[1]).powi(2)).sqrt()
        };
        let (da, dc, dd) = (center_dist(0), center_dist(2), center_dist(3));
        assert!(
            (da - RADIAL_RING_STEP).abs() < 1.0,
            "a на первом кольце: {da}"
        );
        assert!(
            (dc - da).abs() < 1.0 && (dd - da).abs() < 1.0,
            "одно кольцо"
        );
        assert!(!overlaps(&canvas, &plan));
    }

    #[test]
    fn cycles_terminate() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "b", 300.0, 300.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        canvas.edges.push(Edge::new("e2", "b", None, "a", None));
        let plan = plan_related_layout(&canvas, 0, LayoutMode::TreeHorizontal);
        assert_eq!(plan.len(), 1, "цикл a↔b не зависает, b в плане");
    }

    #[test]
    fn invalid_seed_and_isolated() {
        let canvas = chain_canvas();
        assert!(plan_related_layout(&canvas, 99, LayoutMode::Radial).is_empty());
        // Изолированная нода — пустой план
        assert!(plan_related_layout(&canvas, 4, LayoutMode::Radial).is_empty());
    }

    #[test]
    fn determinism() {
        let canvas = chain_canvas();
        let p1 = plan_related_layout(&canvas, 1, LayoutMode::TreeHorizontal);
        let p2 = plan_related_layout(&canvas, 1, LayoutMode::TreeHorizontal);
        assert_eq!(p1, p2);
    }

    #[test]
    fn group_moves_children_once() {
        // b — группа с геометрическим ребёнком kid; группа связана с a
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "a", 0.0, 0.0));
        canvas
            .nodes
            .push(Node::group("b", 300.0, 300.0, 400.0, 300.0));
        canvas.nodes.push(Node::text("kid", "kid", 350.0, 350.0));
        canvas.nodes.push(Node::text("far", "far", 2000.0, 2000.0));
        canvas.edges.push(Edge::new("e1", "a", None, "b", None));
        let plan = plan_related_layout(&canvas, 0, LayoutMode::TreeHorizontal);
        let by_id: HashMap<usize, [f32; 2]> = plan.iter().copied().collect();
        // kid в плане вместе с группой (тот же дельта-вектор)
        assert_eq!(
            by_id[&2][0] - 350.0,
            by_id[&1][0] - 300.0,
            "kid едет с группой"
        );
        assert_eq!(by_id[&2][1] - 350.0, by_id[&1][1] - 300.0);
        assert!(!by_id.contains_key(&3), "far не тронут");
    }
}
