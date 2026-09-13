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
/// Дефолт зоны; пользователь настраивает величину в настройках приложения
/// (CR-003, `Settings::port_zone_px`) — она пробрасывается в `port_at`.
pub const PORT_HIT_PX: f32 = 10.0;

/// Число сегментов полилинии при тесселяции кривой.
pub const TESSELLATION_SEGMENTS: usize = 24;

/// Минимальное смещение контрольных точек от портов (world-единицы).
const MIN_CONTROL_OFFSET: f32 = 40.0;

/// Зазор связи при обходе нод: препятствия инфлируются на эту величину
/// (world-px), чтобы линия не липла к границам нод.
pub const AVOID_MARGIN: f32 = 12.0;

/// Предел числа огибаний на одну связь: страховка от зацикливания роутинга;
/// при превышении возвращаем исходную полилинию (не хуже старого поведения).
pub const MAX_DETOURS: usize = 8;

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

/// Конец связи для перепривязки (CR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeEnd {
    /// Исток (`fromNode`).
    From,
    /// Сток (`toNode`).
    To,
}

/// Разрешённый конец связи: сторона и точка порта. None-сторона выводится
/// из взаимного положения нод (резолв как в `edge_curve`) — хэндл перепривязки
/// рисуется/ловится там же, где линия фактически начинается. None — связь
/// висячая (ноды нет) или индекс невалиден.
pub fn edge_endpoint(canvas: &Canvas, edge_index: usize, end: EdgeEnd) -> Option<(Side, [f32; 2])> {
    let edge = canvas.edges.get(edge_index)?;
    let from = canvas.node(&edge.from_node)?;
    let to = canvas.node(&edge.to_node)?;
    let (node, opposite, side) = match end {
        EdgeEnd::From => (
            from,
            [to.x + to.width / 2.0, to.y + to.height / 2.0],
            edge.from_side,
        ),
        EdgeEnd::To => (
            to,
            [from.x + from.width / 2.0, from.y + from.height / 2.0],
            edge.to_side,
        ),
    };
    let side = side.unwrap_or_else(|| nearest_side(node, opposite));
    Some((side, port_point(node, side)))
}

/// Перепривязать конец связи к ноде `target_node_id` со стороной `side`
/// (CR-002). Чистая функция над моделью: id и настройки связи (лейбл,
/// цвет, стиль, толщина) сохраняются. Отказ (false):
///
/// - связь/индекс невалидны (висячая);
/// - целевая нода не найдена;
/// - цель — противоположный конец (петля или слияние концов).
///
/// Успех — true (поля обновлены на месте).
pub fn retarget_edge(
    canvas: &mut Canvas,
    edge_index: usize,
    end: EdgeEnd,
    target_node_id: &str,
    side: Side,
) -> bool {
    // Проверки ДО mutable-заимствования: цель существует и не противоположный
    // конец (immutable-чтения не конфликтуют друг с другом)
    let opposite = {
        let Some(edge) = canvas.edges.get(edge_index) else {
            return false;
        };
        match end {
            EdgeEnd::From => edge.to_node.clone(),
            EdgeEnd::To => edge.from_node.clone(),
        }
    };
    if opposite == target_node_id {
        return false;
    }
    if canvas.node(target_node_id).is_none() {
        return false;
    }
    let Some(edge) = canvas.edges.get_mut(edge_index) else {
        return false;
    };
    let target = target_node_id.to_owned();
    match end {
        EdgeEnd::From => {
            edge.from_node = target;
            edge.from_side = Some(side);
        }
        EdgeEnd::To => {
            edge.to_node = target;
            edge.to_side = Some(side);
        }
    }
    true
}

/// Расстояние от world-точки до кривой связи; None для висячей связи.
/// Hit-test: результат < EDGE_HIT_TOLERANCE — попадание. `avoid` — обход
/// посторонних нод (глобальная настройка): рендер и hit-test ходят по одной
/// и той же полилинии, чтобы кликабельная область совпадала с нарисованным.
pub fn distance_to_edge(canvas: &Canvas, edge: &Edge, point: [f32; 2], avoid: bool) -> Option<f32> {
    let points = edge_polyline(canvas, edge, avoid, TESSELLATION_SEGMENTS)?;
    Some(distance_point_to_polyline(point, &points))
}

/// Полилиния связи для рендера/hit-test'а: тесселяция Безье; при avoid —
/// с огибанием посторонних нод (концевые ноды не препятствия).
pub fn edge_polyline(
    canvas: &Canvas,
    edge: &Edge,
    avoid: bool,
    segments: usize,
) -> Option<Vec<[f32; 2]>> {
    let curve = edge_curve(canvas, edge)?;
    let points = tessellate(&curve, segments.max(1));
    if !avoid {
        return Some(points);
    }
    let obstacles: Vec<[f32; 4]> = canvas
        .nodes
        .iter()
        .filter(|node| node.id != edge.from_node && node.id != edge.to_node)
        .map(|node| [node.x, node.y, node.width, node.height])
        .collect();
    Some(route_polyline(
        &points,
        &obstacles,
        AVOID_MARGIN,
        MAX_DETOURS,
    ))
}

/// Середина связи по длине дуги (для лейбла и бокса редактирования):
/// при avoid совпадает с видимой огибающей линией.
pub fn edge_midpoint(canvas: &Canvas, edge: &Edge, avoid: bool) -> Option<[f32; 2]> {
    let points = edge_polyline(canvas, edge, avoid, TESSELLATION_SEGMENTS)?;
    let total: f32 = points
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum();
    let half = total / 2.0;
    let mut acc = 0.0;
    for w in points.windows(2) {
        let seg = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        if acc + seg >= half {
            let t = ((half - acc) / seg.max(f32::EPSILON)).clamp(0.0, 1.0);
            return Some([
                w[0][0] + (w[1][0] - w[0][0]) * t,
                w[0][1] + (w[1][1] - w[0][1]) * t,
            ]);
        }
        acc += seg;
    }
    points.last().copied().or_else(|| points.first().copied())
}

// --- Огибание препятствий (обход нод) ---

/// Rect [x, y, w, h], инфлированный на margin.
fn inflate_rect(rect: [f32; 4], margin: f32) -> [f32; 4] {
    [
        rect[0] - margin,
        rect[1] - margin,
        rect[2] + margin * 2.0,
        rect[3] + margin * 2.0,
    ]
}

/// Точка внутри rect (граница считается внутренностью).
fn point_in_rect(p: [f32; 2], rect: [f32; 4]) -> bool {
    p[0] >= rect[0] && p[0] <= rect[0] + rect[2] && p[1] >= rect[1] && p[1] <= rect[1] + rect[3]
}

/// Пересекается ли отрезок a–b с прямоугольником (slab-метод).
fn segment_hits_rect(a: [f32; 2], b: [f32; 2], rect: [f32; 4]) -> bool {
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut t_enter = 0.0f32;
    let mut t_exit = 1.0f32;
    for axis in 0..2 {
        let (start, dir, lo, hi) = if axis == 0 {
            (a[0], d[0], rect[0], rect[0] + rect[2])
        } else {
            (a[1], d[1], rect[1], rect[1] + rect[3])
        };
        if dir.abs() < f32::EPSILON {
            if start < lo || start > hi {
                return false;
            }
        } else {
            let (t0, t1) = ((lo - start) / dir, (hi - start) / dir);
            let (t0, t1) = (t0.min(t1), t0.max(t1));
            t_enter = t_enter.max(t0);
            t_exit = t_exit.min(t1);
            if t_enter > t_exit {
                return false;
            }
        }
    }
    true
}

/// Точка входа отрезка a–b в rect (a снаружи): пересечение с ближней гранью.
fn rect_entry(a: [f32; 2], b: [f32; 2], rect: [f32; 4]) -> Option<[f32; 2]> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut t_enter = 0.0f32;
    for axis in 0..2 {
        let (start, dir, lo, hi) = if axis == 0 {
            (a[0], d[0], rect[0], rect[0] + rect[2])
        } else {
            (a[1], d[1], rect[1], rect[1] + rect[3])
        };
        if dir.abs() < f32::EPSILON {
            continue;
        }
        let (t0, t1) = ((lo - start) / dir, (hi - start) / dir);
        t_enter = t_enter.max(t0.min(t1));
    }
    (t_enter > 0.0 && t_enter <= 1.0).then(|| [a[0] + d[0] * t_enter, a[1] + d[1] * t_enter])
}

/// Параметр точки на границе rect вдоль периметра (по часовой от левого
/// верхнего угла): верх → право → низ → лево. Углы: TL=0, TR=w, BR=w+h,
/// BL=2w+h; периметр L = 2(w+h).
fn boundary_param(p: [f32; 2], rect: [f32; 4]) -> f32 {
    let (w, h) = (rect[2], rect[3]);
    let eps = 1e-3;
    if (p[1] - rect[1]).abs() <= eps {
        (p[0] - rect[0]).clamp(0.0, w)
    } else if (p[0] - (rect[0] + w)).abs() <= eps {
        w + (p[1] - rect[1]).clamp(0.0, h)
    } else if (p[1] - (rect[1] + h)).abs() <= eps {
        w + h + (rect[0] + w - p[0]).clamp(0.0, w)
    } else {
        2.0 * w + h + (rect[1] + h - p[1]).clamp(0.0, h)
    }
}

/// Ближайшая к точке точка границы rect.
fn nearest_boundary_point(p: [f32; 2], rect: [f32; 4]) -> [f32; 2] {
    let (l, t, w, h) = (rect[0], rect[1], rect[2], rect[3]);
    let cx = p[0].clamp(l, l + w);
    let cy = p[1].clamp(t, t + h);
    if cx != p[0] || cy != p[1] {
        return [cx, cy];
    }
    // Внутри: ближайшая грань.
    let candidates = [
        ([l, cy], cx - l),
        ([l + w, cy], l + w - cx),
        ([cx, t], cy - t),
        ([cx, t + h], t + h - cy),
    ];
    candidates
        .into_iter()
        .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(point, _)| point)
        .unwrap_or([cx, cy])
}

/// Угловые waypoints вдоль более короткого пути по периметру rect от p до q
/// (обе на границе). Порядок — от p к q, q НЕ включена.
fn detour_corners(rect: [f32; 4], p: [f32; 2], q: [f32; 2]) -> Vec<[f32; 2]> {
    let (l, t, w, h) = (rect[0], rect[1], rect[2], rect[3]);
    let perimeter = 2.0 * (w + h);
    if perimeter <= f32::EPSILON {
        return Vec::new();
    }
    let s_p = boundary_param(p, rect);
    let s_q = boundary_param(q, rect);
    let dist_cw = (s_q - s_p).rem_euclid(perimeter);
    let dist_ccw = perimeter - dist_cw;
    let corners = [
        (0.0f32, [l, t]),
        (w, [l + w, t]),
        (w + h, [l + w, t + h]),
        (2.0 * w + h, [l, t + h]),
    ];
    let mut waypoints: Vec<(f32, [f32; 2])> = corners
        .into_iter()
        .filter_map(|(s, point)| {
            if dist_cw <= dist_ccw {
                let off = (s - s_p).rem_euclid(perimeter);
                (off > 1e-3 && off < dist_cw - 1e-3).then_some((off, point))
            } else {
                let off = (s_p - s).rem_euclid(perimeter);
                (off > 1e-3 && off < dist_ccw - 1e-3).then_some((off, point))
            }
        })
        .collect();
    waypoints.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    waypoints.into_iter().map(|(_, point)| point).collect()
}

/// Огибание препятствий: полилиния обходит инфлированные прямоугольники
/// нод короткой стороной. Детерминировано; при превышении `max_detours`
/// возвращается исходная полилиния (деградация к прямой Безье, без петель).
pub fn route_polyline(
    points: &[[f32; 2]],
    obstacles: &[[f32; 4]],
    margin: f32,
    max_detours: usize,
) -> Vec<[f32; 2]> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let rects: Vec<[f32; 4]> = obstacles
        .iter()
        .map(|rect| inflate_rect(*rect, margin))
        .filter(|rect| rect[2] > 0.0 && rect[3] > 0.0)
        .collect();
    if rects.is_empty() {
        return points.to_vec();
    }
    let mut path = points.to_vec();
    for _ in 0..max_detours {
        // Первое пересечение любого сегмента с любым препятствием.
        let mut hit: Option<(usize, usize, [f32; 2], usize)> = None;
        'scan: for i in 0..path.len() - 1 {
            for (ri, rect) in rects.iter().enumerate() {
                if !segment_hits_rect(path[i], path[i + 1], *rect) {
                    continue;
                }
                let entry = if point_in_rect(path[i], *rect) {
                    path[i]
                } else {
                    rect_entry(path[i], path[i + 1], *rect).unwrap_or(path[i])
                };
                // Индекс первой точки полилинии за пределами rect.
                let mut k = i + 1;
                while k < path.len() && point_in_rect(path[k], *rect) {
                    k += 1;
                }
                hit = Some((i, ri, entry, k));
                break 'scan;
            }
        }
        let Some((i, ri, entry, k)) = hit else {
            break;
        };
        let rect = rects[ri];
        let target = path.get(k).copied().or_else(|| path.last().copied());
        let Some(target) = target else { break };
        let exit = nearest_boundary_point(target, rect);
        let corners = detour_corners(rect, entry, exit);
        if (exit[0] - entry[0]).abs() < f32::EPSILON
            && (exit[1] - entry[1]).abs() < f32::EPSILON
            && corners.is_empty()
        {
            // Касание без огибания — дальше прогресса не будет.
            break;
        }
        // Склейка: путь до входа, угловые waypoints, выход, остаток от k.
        let mut rerouted = Vec::with_capacity(path.len() + corners.len() + 2);
        rerouted.extend_from_slice(&path[..=i]);
        if (entry[0] - path[i][0]).abs() > f32::EPSILON
            || (entry[1] - path[i][1]).abs() > f32::EPSILON
        {
            rerouted.push(entry);
        }
        rerouted.extend_from_slice(&corners);
        rerouted.push(exit);
        rerouted.extend_from_slice(&path[k.min(path.len())..]);
        path = rerouted;
    }
    path
}

/// Порт ноды под курсором: сторона, чья точка порта ближе всего к `point`
/// в пределах допуска `tolerance_px` экранных пикселей (zoom — camera.zoom();
/// CR-003: допуск — параметр, дефолт — `PORT_HIT_PX`).
pub fn port_at(node: &Node, point: [f32; 2], zoom: f32, tolerance_px: f32) -> Option<Side> {
    let tolerance = tolerance_px / zoom.max(1e-3);
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
/// `avoid` — обход посторонних нод (см. `edge_polyline`).
pub fn edge_at(canvas: &Canvas, point: [f32; 2], avoid: bool) -> Option<usize> {
    canvas
        .edges
        .iter()
        .enumerate()
        .filter_map(|(index, edge)| {
            distance_to_edge(canvas, edge, point, avoid).map(|d| (index, d))
        })
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
