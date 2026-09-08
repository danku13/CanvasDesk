//! Геометрия связей (T8): порты нод, кубическая Безье между портами,
//! тесселяция в полилинию и hit-test «точка → кривая».
//!
//! Всё — чистые функции в world-координатах (SPEC §5.1); рендер использует
//! тесселяцию, ввод — hit-test. Зависимостей от ОС и GPU нет.

use crate::model::{Canvas, Edge, Node, Side};

/// Допуск hit-test'а линии связи в world-единицах (TASKS T8: < 6px).
pub const EDGE_HIT_TOLERANCE: f32 = 6.0;

/// Допуск попадания курсора в порт ноды — в экранных пикселях
/// (переводится в world-единицы делением на zoom).
pub const PORT_HIT_PX: f32 = 10.0;

/// Число сегментов полилинии при тесселяции кривой.
pub const TESSELLATION_SEGMENTS: usize = 24;

/// Минимальное смещение контрольных точек от портов (world-единицы).
const MIN_CONTROL_OFFSET: f32 = 40.0;

/// Кубическая кривая Безье: концы в портах, контрольные точки — по нормалям.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    pub p0: [f32; 2],
    pub c0: [f32; 2],
    pub c1: [f32; 2],
    pub p1: [f32; 2],
}

/// Точка порта — центр соответствующей стороны ноды.
pub fn port_point(node: &Node, side: Side) -> [f32; 2] {
    let cx = node.x + node.width / 2.0;
    let cy = node.y + node.height / 2.0;
    match side {
        Side::Top => [cx, node.y],
        Side::Right => [node.x + node.width, cy],
        Side::Bottom => [cx, node.y + node.height],
        Side::Left => [node.x, cy],
    }
}

/// Внешняя нормаль стороны (направление «от ноды»).
pub fn side_normal(side: Side) -> [f32; 2] {
    match side {
        Side::Top => [0.0, -1.0],
        Side::Right => [1.0, 0.0],
        Side::Bottom => [0.0, 1.0],
        Side::Left => [-1.0, 0.0],
    }
}

/// Сторона ноды, ближайшая к точке (доминирующая ось с учётом пропорций).
/// Используется при drop резиновой линии и для None-сторон из файла.
pub fn nearest_side(node: &Node, point: [f32; 2]) -> Side {
    let dx = point[0] - (node.x + node.width / 2.0);
    let dy = point[1] - (node.y + node.height / 2.0);
    let nx = dx.abs() / (node.width / 2.0).max(1.0);
    let ny = dy.abs() / (node.height / 2.0).max(1.0);
    if nx >= ny {
        if dx >= 0.0 {
            Side::Right
        } else {
            Side::Left
        }
    } else if dy >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}

/// Кривая между портами двух нод. Контрольные точки смещены от портов
/// по нормалям сторон на max(MIN_CONTROL_OFFSET, 40% расстояния).
pub fn bezier_between(a: &Node, sa: Side, b: &Node, sb: Side) -> CubicBezier {
    let p0 = port_point(a, sa);
    let p1 = port_point(b, sb);
    let dist = ((p1[0] - p0[0]).powi(2) + (p1[1] - p0[1]).powi(2)).sqrt();
    let offset = (dist * 0.4).max(MIN_CONTROL_OFFSET);
    let n0 = side_normal(sa);
    let n1 = side_normal(sb);
    CubicBezier {
        p0,
        c0: [p0[0] + n0[0] * offset, p0[1] + n0[1] * offset],
        c1: [p1[0] + n1[0] * offset, p1[1] + n1[1] * offset],
        p1,
    }
}

/// Точка на кривой по параметру t ∈ [0, 1].
pub fn curve_point(curve: &CubicBezier, t: f32) -> [f32; 2] {
    let mt = 1.0 - t;
    let (b0, b1, b2, b3) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
    [
        b0 * curve.p0[0] + b1 * curve.c0[0] + b2 * curve.c1[0] + b3 * curve.p1[0],
        b0 * curve.p0[1] + b1 * curve.c0[1] + b2 * curve.c1[1] + b3 * curve.p1[1],
    ]
}

/// Нормированная касательная к кривой в точке t (для стрелки на конце).
pub fn curve_tangent(curve: &CubicBezier, t: f32) -> [f32; 2] {
    let mt = 1.0 - t;
    let dx = 3.0 * mt * mt * (curve.c0[0] - curve.p0[0])
        + 6.0 * mt * t * (curve.c1[0] - curve.c0[0])
        + 3.0 * t * t * (curve.p1[0] - curve.c1[0]);
    let dy = 3.0 * mt * mt * (curve.c0[1] - curve.p0[1])
        + 6.0 * mt * t * (curve.c1[1] - curve.c0[1])
        + 3.0 * t * t * (curve.p1[1] - curve.c1[1]);
    let len = (dx * dx + dy * dy).sqrt();
    if len < f32::EPSILON {
        [1.0, 0.0]
    } else {
        [dx / len, dy / len]
    }
}

/// Тесселяция кривой в полилинию из `segments` отрезков (segments+1 точек).
pub fn tessellate(curve: &CubicBezier, segments: usize) -> Vec<[f32; 2]> {
    let segments = segments.max(1);
    (0..=segments)
        .map(|i| curve_point(curve, i as f32 / segments as f32))
        .collect()
}

/// Расстояние от точки до отрезка.
fn distance_point_to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let ab = [b[0] - a[0], b[1] - a[1]];
    let ap = [p[0] - a[0], p[1] - a[1]];
    let len2 = ab[0] * ab[0] + ab[1] * ab[1];
    let t = if len2 < f32::EPSILON {
        0.0
    } else {
        ((ap[0] * ab[0] + ap[1] * ab[1]) / len2).clamp(0.0, 1.0)
    };
    let closest = [a[0] + ab[0] * t, a[1] + ab[1] * t];
    ((p[0] - closest[0]).powi(2) + (p[1] - closest[1]).powi(2)).sqrt()
}

/// Расстояние от точки до полилинии (минимум по сегментам).
pub fn distance_point_to_polyline(point: [f32; 2], points: &[[f32; 2]]) -> f32 {
    points
        .windows(2)
        .map(|seg| distance_point_to_segment(point, seg[0], seg[1]))
        .fold(f32::INFINITY, f32::min)
}

/// Кривая связи: резолвит ноды по id, None-стороны выводит из взаимного
/// положения центров. None, если хотя бы одна нода не найдена (висячая связь).
pub fn edge_curve(canvas: &Canvas, edge: &Edge) -> Option<CubicBezier> {
    let from = canvas.node(&edge.from_node)?;
    let to = canvas.node(&edge.to_node)?;
    let from_side = edge
        .from_side
        .unwrap_or_else(|| nearest_side(from, [to.x + to.width / 2.0, to.y + to.height / 2.0]));
    let to_side = edge.to_side.unwrap_or_else(|| {
        nearest_side(to, [from.x + from.width / 2.0, from.y + from.height / 2.0])
    });
    Some(bezier_between(from, from_side, to, to_side))
}

/// Расстояние от world-точки до кривой связи; None для висячей связи.
/// Hit-test: результат < EDGE_HIT_TOLERANCE — попадание.
pub fn distance_to_edge(canvas: &Canvas, edge: &Edge, point: [f32; 2]) -> Option<f32> {
    let curve = edge_curve(canvas, edge)?;
    let points = tessellate(&curve, TESSELLATION_SEGMENTS);
    Some(distance_point_to_polyline(point, &points))
}

/// Порт ноды под курсором: сторона, чья точка порта ближе всего к `point`
/// в пределах допуска PORT_HIT_PX экранных пикселей (zoom — camera.zoom()).
pub fn port_at(node: &Node, point: [f32; 2], zoom: f32) -> Option<Side> {
    let tolerance = PORT_HIT_PX / zoom.max(1e-3);
    [Side::Top, Side::Right, Side::Bottom, Side::Left]
        .into_iter()
        .map(|side| {
            let port = port_point(node, side);
            let dist = ((point[0] - port[0]).powi(2) + (point[1] - port[1]).powi(2)).sqrt();
            (dist, side)
        })
        .filter(|(dist, _)| *dist <= tolerance)
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, side)| side)
}

/// Ближайшая к точке связь в допуске EDGE_HIT_TOLERANCE.
/// Возвращает индекс в `canvas.edges`; None — промах (или все связи висячие).
pub fn edge_at(canvas: &Canvas, point: [f32; 2]) -> Option<usize> {
    canvas
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| distance_to_edge(canvas, edge, point).map(|d| (index, d)))
        .filter(|(_, dist)| *dist < EDGE_HIT_TOLERANCE)
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(index, _)| index)
}

/// Кривая «резиновой линии» при drag новой связи: от точки порта к курсору.
/// Контрольная точка у порта смещена по нормали стороны, у курсора —
/// совпадает с ним (линия «прилипает» к указателю без выраженного изгиба).
pub fn draft_curve(port: [f32; 2], side: Side, to: [f32; 2]) -> CubicBezier {
    let dist = ((to[0] - port[0]).powi(2) + (to[1] - port[1]).powi(2)).sqrt();
    let offset = (dist * 0.4).max(MIN_CONTROL_OFFSET);
    let normal = side_normal(side);
    CubicBezier {
        p0: port,
        c0: [port[0] + normal[0] * offset, port[1] + normal[1] * offset],
        c1: to,
        p1: to,
    }
}
