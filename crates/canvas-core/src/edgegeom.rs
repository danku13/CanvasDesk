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

/// CR-008: штраф стоимости пары портов, когда порт «спиной» к другой ноде
/// (нормаль смотрит против направления на соседа) — в долях расстояния.
const BACK_FACING_PENALTY: f32 = 1.5;

/// CR-008: штраф за одинаковые стороны истока и стока (петлеобразная
/// кривая) — в долях расстояния.
const SAME_SIDE_PENALTY: f32 = 1.0;

/// Порог «спины» по скалярному произведению нормали и направления на
/// соседа: |dot| ниже порога — боковой порт, штрафа нет.
const FACING_DOT_THRESHOLD: f32 = 0.25;

/// CR-008: пара сторон с кратчайшим путём между нодами. Перебор всех 4×4
/// пар портов; стоимость = расстояние между портами + штрафы за «спинные»
/// порты (нормаль смотрит от соседа) и за одинаковые стороны. Детерминирован:
/// при равной стоимости остаётся первая пара в порядке перебора (Top, Right,
/// Bottom, Left по истоку; внутренний цикл — по стоку).
pub fn best_sides(from: &Node, to: &Node) -> (Side, Side) {
    let sides = [Side::Top, Side::Right, Side::Bottom, Side::Left];
    let mut best = (Side::Top, Side::Top);
    let mut best_cost = f32::INFINITY;
    for &sa in &sides {
        let pa = port_point(from, sa);
        let na = side_normal(sa);
        for &sb in &sides {
            let pb = port_point(to, sb);
            let dx = pb[0] - pa[0];
            let dy = pb[1] - pa[1];
            let dist = dx.hypot(dy).max(1e-3);
            let ux = dx / dist;
            let uy = dy / dist;
            let mut cost = dist;
            // порт истока «спиной» к получателю
            if na[0] * ux + na[1] * uy < -FACING_DOT_THRESHOLD {
                cost += dist * BACK_FACING_PENALTY;
            }
            // порт получателя «спиной» к истоку (нормаль по направлению связи)
            let nb = side_normal(sb);
            if nb[0] * ux + nb[1] * uy > FACING_DOT_THRESHOLD {
                cost += dist * BACK_FACING_PENALTY;
            }
            if sa == sb {
                cost += dist * SAME_SIDE_PENALTY;
            }
            if cost < best_cost - f32::EPSILON {
                best_cost = cost;
                best = (sa, sb);
            }
        }
    }
    best
}

/// CR-008: эффективные стороны связи — склейка закреплений и геометрии.
/// Закреплённый конец (`canvasdesk.pin_ports`) берёт сохранённую сторону
/// (None — `best_sides` как фолбэк), свободный — сторону кратчайшего пути.
pub fn effective_sides(edge: &Edge, from: &Node, to: &Node) -> (Side, Side) {
    let (from_pinned, to_pinned) = edge.port_pins();
    let (best_from, best_to) = best_sides(from, to);
    let from_side = if from_pinned {
        edge.from_side.unwrap_or(best_from)
    } else {
        best_from
    };
    let to_side = if to_pinned {
        edge.to_side.unwrap_or(best_to)
    } else {
        best_to
    };
    (from_side, to_side)
}

/// Кривая связи: резолвит ноды по id; стороны — эффективные (CR-008):
/// закреплённый конец — сохранённая сторона, свободный — кратчайший путь
/// по взаимному положению нод (пересчёт на каждом кадре: drag, автораскладка
/// и загрузка идут через один путь). None, если хотя бы одна нода не найдена
/// (висячая связь).
pub fn edge_curve(canvas: &Canvas, edge: &Edge) -> Option<CubicBezier> {
    let from = canvas.node(&edge.from_node)?;
    let to = canvas.node(&edge.to_node)?;
    let (from_side, to_side) = effective_sides(edge, from, to);
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
    // CR-008: сторона хэндла — та же эффективная сторона, по которой
    // рисуется линия (пин учитывается, авто следует геометрии)
    let (eff_from, eff_to) = effective_sides(edge, from, to);
    let (node, side) = match end {
        EdgeEnd::From => (from, eff_from),
        EdgeEnd::To => (to, eff_to),
    };
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
            // FR-025: перепривязка ИСТОКА на другую ноду сбрасывает
            // построчный исток (индекс строки мог не существовать у новой
            // ноды; v1 — деградация до узлового значения)
            edge.from_line = None;
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
/// с огибанием посторонних нод. Не препятствия: концевые ноды и ВСЕ их
/// группы-предки (транзитивно) — линк к ноде внутри группы свободно
/// проходит её границу, линк к группе в целом — её конец. Чужие группы
/// (не содержащие концы) огибаются как обычные ноды.
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
    let mut excluded: Vec<usize> = Vec::new();
    for id in [&edge.from_node, &edge.to_node] {
        if let Some(i) = canvas.nodes.iter().position(|node| node.id == *id) {
            excluded.push(i);
            excluded.extend(crate::enclosing_group_indices(canvas, i));
        }
    }
    excluded.sort_unstable();
    excluded.dedup();
    let obstacles: Vec<[f32; 4]> = canvas
        .nodes
        .iter()
        .enumerate()
        .filter(|(index, _)| !excluded.contains(index))
        .map(|(_, node)| [node.x, node.y, node.width, node.height])
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

/// FR-025: построчная точка выхода — порт формульной строки Numi-листа на
/// правом краю ноды (вертикаль — ряд результата строки, как у бейджа FR-013).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinePort {
    /// Индекс формульной строки текста ноды. `None` — узловое значение
    /// (футер шаблонной ноды FR-023: формула и есть финальное значение).
    pub line: Option<usize>,
    /// World-точка порта (`node.x + node.width`, Y ряда результата).
    pub point: [f32; 2],
    /// Финальная строка (значение ноды) — рендер отличает заполненным
    /// кружком от промежуточных.
    pub is_final: bool,
}

/// FR-025: построчный порт под курсором — ближайший в пределах допуска
/// `tolerance_px` экранных пикселей (тот же допуск, что у сторонных
/// портов, CR-003). Приоритет построчного порта над сторонным решает
/// вызывающий (проверка `line_port_at` ДО `port_at`).
pub fn line_port_at(
    ports: &[LinePort],
    point: [f32; 2],
    zoom: f32,
    tolerance_px: f32,
) -> Option<LinePort> {
    let tolerance = tolerance_px / zoom.max(1e-3);
    ports
        .iter()
        .copied()
        .map(|port| {
            let dist =
                ((point[0] - port.point[0]).powi(2) + (point[1] - port.point[1]).powi(2)).sqrt();
            (dist, port)
        })
        .filter(|(dist, _)| *dist <= tolerance)
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(_, port)| port)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Canvas;

    fn node_a() -> Node {
        Node::text("a", "a", 0.0, 0.0)
    }

    /// FR-025: hit-test построчных портов — попадание в зоне допуска,
    /// промах вне, из двух близких портов берётся ближайший.
    #[test]
    fn line_port_at_hit_miss_nearest() {
        let ports = [
            LinePort {
                line: Some(0),
                point: [200.0, 30.0],
                is_final: false,
            },
            LinePort {
                line: Some(1),
                point: [200.0, 50.0],
                is_final: true,
            },
        ];
        // В зоне (tolerance 10 screen px при zoom 1 → 10 world)
        let hit = line_port_at(&ports, [206.0, 50.0], 1.0, 10.0).expect("попадание");
        assert_eq!(hit.line, Some(1));
        assert!(hit.is_final);
        // Промах: дальше допуска по вертикали
        assert!(line_port_at(&ports, [206.0, 90.0], 1.0, 10.0).is_none());
        // Промах: далеко правее края
        assert!(line_port_at(&ports, [400.0, 50.0], 1.0, 10.0).is_none());
        // Между двумя портами — ближайший
        let hit = line_port_at(&ports, [200.0, 41.0], 1.0, 10.0).expect("попадание");
        assert_eq!(hit.line, Some(1), "41 ближе к 50, чем к 30");
        // zoom > 1 ужесточает world-допуск: 10 screen px при zoom 2 —
        // 5 world px — точка в 6 px от порта уже мимо
        assert!(line_port_at(&ports, [206.0, 50.0], 2.0, 10.0).is_none());
    }

    /// FR-025: перепривязка ИСТОКА сбрасывает построчный исток (v1),
    /// перепривязка СТОКА — сохраняет.
    #[test]
    fn retarget_resets_from_line_only_on_from_end() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(node_a());
        canvas.nodes.push(Node::text("b", "b", 400.0, 0.0));
        canvas.nodes.push(Node::text("c", "c", 800.0, 0.0));
        canvas.nodes.push(Node::text("d", "d", 1200.0, 0.0));
        let mut edge = Edge::new("e1", "a", Some(Side::Right), "b", Some(Side::Left));
        edge.from_line = Some(1);
        canvas.add_edge(edge);

        // Перепривязка СТОКА (b → c): from_line сохраняется
        assert!(retarget_edge(&mut canvas, 0, EdgeEnd::To, "c", Side::Left));
        assert_eq!(canvas.edges[0].to_node, "c");
        assert_eq!(canvas.edges[0].from_line, Some(1));
        // Перепривязка ИСТОКА (a → d, цель не противоположный конец):
        // from_line сбрасывается
        assert!(retarget_edge(
            &mut canvas,
            0,
            EdgeEnd::From,
            "d",
            Side::Right
        ));
        assert_eq!(canvas.edges[0].from_node, "d");
        assert_eq!(
            canvas.edges[0].from_line, None,
            "v1: сброс построчного истока"
        );
    }
}
