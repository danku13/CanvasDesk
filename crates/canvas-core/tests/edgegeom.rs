//! T8: геометрия связей (порты, кубическая Безье, hit-test) и операции модели
//! с edges — чистые функции, тестируются без GPU.

use canvas_core::{
    bezier_between, curve_point, curve_tangent, distance_point_to_polyline, distance_to_edge,
    draft_curve, edge_at, edge_curve, nearest_side, port_at, port_point, side_normal, tessellate,
    Canvas, Edge, Node, Side, EDGE_HIT_TOLERANCE, PORT_HIT_PX,
};

fn node(id: &str, x: f32, y: f32, w: f32, h: f32) -> Node {
    Node::file(id, "C:/f.png", x, y, w, h)
}

fn approx(a: [f32; 2], b: [f32; 2]) {
    assert!(
        (a[0] - b[0]).abs() < 1e-4 && (a[1] - b[1]).abs() < 1e-4,
        "ожидалось {b:?}, получено {a:?}"
    );
}

/// Порт — центр соответствующей стороны ноды.
#[test]
fn port_points_are_side_centers() {
    let n = node("a", 100.0, 200.0, 300.0, 120.0);
    approx(port_point(&n, Side::Top), [250.0, 200.0]);
    approx(port_point(&n, Side::Right), [400.0, 260.0]);
    approx(port_point(&n, Side::Bottom), [250.0, 320.0]);
    approx(port_point(&n, Side::Left), [100.0, 260.0]);
}

/// Концы кривой — точно в портах, контрольные точки смещены по нормалям сторон.
#[test]
fn bezier_endpoints_and_control_directions() {
    let a = node("a", 0.0, 0.0, 100.0, 100.0);
    let b = node("b", 400.0, 0.0, 100.0, 100.0);
    let curve = bezier_between(&a, Side::Right, &b, Side::Left);
    approx(curve.p0, [100.0, 50.0]);
    approx(curve.p1, [400.0, 50.0]);
    // Контрольная точка от right-порта смещена вправо, от left-порта — влево.
    assert!(curve.c0[0] > curve.p0[0], "c0 вправо от p0: {:?}", curve);
    assert!(curve.c1[0] < curve.p1[0], "c1 влево от p1: {:?}", curve);
    assert!((curve.c0[1] - curve.p0[1]).abs() < 1e-4);
    assert!((curve.c1[1] - curve.p1[1]).abs() < 1e-4);
}

/// Тесселяция: segments+1 точек, концы совпадают с портами.
#[test]
fn tessellation_covers_endpoints() {
    let a = node("a", 0.0, 0.0, 100.0, 100.0);
    let b = node("b", 0.0, 400.0, 100.0, 100.0);
    let curve = bezier_between(&a, Side::Bottom, &b, Side::Top);
    let points = tessellate(&curve, 24);
    assert_eq!(points.len(), 25);
    approx(points[0], curve.p0);
    approx(points[24], curve.p1);
}

/// Точка на кривой t=0.5 — середина для симметричной пары портов.
#[test]
fn curve_midpoint_is_symmetric() {
    let a = node("a", 0.0, 0.0, 100.0, 100.0);
    let b = node("b", 500.0, 0.0, 100.0, 100.0);
    let curve = bezier_between(&a, Side::Right, &b, Side::Left);
    let mid = curve_point(&curve, 0.5);
    approx(mid, [300.0, 50.0]);
    // Касательная в середине горизонтальна и направлена к b.
    let tangent = curve_tangent(&curve, 0.5);
    assert!(tangent[0] > 0.99 && tangent[1].abs() < 1e-4);
}

/// Расстояние точки до полилинии: на линии — 0, сбоку — перпендикуляр.
#[test]
fn distance_to_polyline() {
    let line = vec![[0.0, 0.0], [100.0, 0.0]];
    assert_eq!(distance_point_to_polyline([50.0, 0.0], &line), 0.0);
    assert_eq!(distance_point_to_polyline([50.0, 6.0], &line), 6.0);
    assert_eq!(distance_point_to_polyline([150.0, 0.0], &line), 50.0);
}

/// edge_curve резолвит ноды по id; None-стороны выводятся из взаимного положения.
#[test]
fn edge_curve_resolves_missing_sides() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(node("a", 0.0, 0.0, 100.0, 100.0));
    canvas.nodes.push(node("b", 400.0, 20.0, 100.0, 100.0));
    let edge = Edge::new("e1", "a", None, "b", None);
    let curve = edge_curve(&canvas, &edge).expect("ноды существуют");
    // b правее a → from right, to left.
    approx(curve.p0, port_point(canvas.node("a").unwrap(), Side::Right));
    approx(curve.p1, port_point(canvas.node("b").unwrap(), Side::Left));
}

/// edge_curve для висячей связи (нода удалена) — None.
#[test]
fn edge_curve_dangling_returns_none() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(node("a", 0.0, 0.0, 100.0, 100.0));
    let edge = Edge::new("e1", "a", None, "missing", None);
    assert!(edge_curve(&canvas, &edge).is_none());
}

/// Hit-test линии: точка рядом с кривой (< 6px) попадает, дальше — нет.
#[test]
fn edge_hit_test_tolerance() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(node("a", 0.0, 0.0, 100.0, 100.0));
    canvas.nodes.push(node("b", 500.0, 0.0, 100.0, 100.0));
    let edge = Edge::new("e1", "a", Some(Side::Right), "b", Some(Side::Left));
    // Середина кривой — y=50.
    let near = distance_to_edge(&canvas, &edge, [250.0, 54.0]).unwrap();
    assert!(near < EDGE_HIT_TOLERANCE);
    let far = distance_to_edge(&canvas, &edge, [250.0, 120.0]).unwrap();
    assert!(far > EDGE_HIT_TOLERANCE);
}

/// nearest_side: ближайшая сторона ноды к точке (для выбора to_side при drop).
#[test]
fn nearest_side_picks_dominant_direction() {
    let n = node("a", 0.0, 0.0, 200.0, 100.0);
    assert_eq!(nearest_side(&n, [300.0, 50.0]), Side::Right);
    assert_eq!(nearest_side(&n, [-50.0, 50.0]), Side::Left);
    assert_eq!(nearest_side(&n, [100.0, -50.0]), Side::Top);
    assert_eq!(nearest_side(&n, [100.0, 200.0]), Side::Bottom);
}

/// add/remove/edges_of/next_edge_id в модели.
#[test]
fn canvas_edge_ops() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(node("a", 0.0, 0.0, 10.0, 10.0));
    canvas.nodes.push(node("b", 100.0, 0.0, 10.0, 10.0));
    assert_eq!(canvas.next_edge_id(), "edge-1");
    canvas.add_edge(Edge::new(
        "edge-1",
        "a",
        Some(Side::Right),
        "b",
        Some(Side::Left),
    ));
    canvas.add_edge(Edge::new("edge-2", "b", None, "a", None));
    assert_eq!(canvas.next_edge_id(), "edge-3");
    assert_eq!(canvas.edges_of("a"), vec![0, 1]);
    assert_eq!(canvas.edges_of("b"), vec![0, 1]);
    assert!(canvas.remove_edge("edge-1"));
    assert!(!canvas.remove_edge("edge-1"));
    assert_eq!(canvas.edges.len(), 1);
    assert_eq!(canvas.edges_of("a"), vec![0]);
    // После удаления edge-1 id не переиспользуется (уникальность по суффиксу).
    assert_eq!(canvas.next_edge_id(), "edge-3");
}

/// Удаление ноды каскадно убирает все её связи.
#[test]
fn remove_node_cascades_edges() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(node("a", 0.0, 0.0, 10.0, 10.0));
    canvas.nodes.push(node("b", 100.0, 0.0, 10.0, 10.0));
    canvas.nodes.push(node("c", 200.0, 0.0, 10.0, 10.0));
    canvas.add_edge(Edge::new("e1", "a", None, "b", None));
    canvas.add_edge(Edge::new("e2", "b", None, "c", None));
    canvas.add_edge(Edge::new("e3", "a", None, "c", None));
    let removed = canvas.remove_node(1).expect("нода b существует");
    assert_eq!(removed.id, "b");
    assert_eq!(canvas.nodes.len(), 2);
    assert_eq!(canvas.edges.len(), 1);
    assert_eq!(canvas.edges[0].id, "e3");
}

/// Round-trip: созданные нами edges сериализуются с fromSide/toSide как в Obsidian.
#[test]
fn edge_serialization_matches_spec() {
    let mut canvas = Canvas::default();
    canvas.add_edge(Edge::new(
        "e1",
        "a",
        Some(Side::Right),
        "b",
        Some(Side::Top),
    ));
    let json = canvas.to_json().unwrap();
    assert!(json.contains("\"fromNode\": \"a\""));
    assert!(json.contains("\"fromSide\": \"right\""));
    assert!(json.contains("\"toNode\": \"b\""));
    assert!(json.contains("\"toSide\": \"top\""));
}

/// port_at: курсор у центра стороны попадает в её порт, в стороне — промах;
/// допуск — экранные px, с ростом zoom world-допуск уменьшается.
#[test]
fn port_at_hits_side_centers() {
    let n = node("a", 100.0, 200.0, 300.0, 120.0);
    // Точно в портах при zoom 1.0
    assert_eq!(port_at(&n, [250.0, 200.0], 1.0), Some(Side::Top));
    assert_eq!(port_at(&n, [400.0, 260.0], 1.0), Some(Side::Right));
    assert_eq!(port_at(&n, [250.0, 320.0], 1.0), Some(Side::Bottom));
    assert_eq!(port_at(&n, [100.0, 260.0], 1.0), Some(Side::Left));
    // В допуске PORT_HIT_PX экранных px
    assert_eq!(port_at(&n, [250.0, 209.0], 1.0), Some(Side::Top));
    // За допуском — промах
    assert_eq!(port_at(&n, [250.0, 211.0], 1.0), None);
    // Центр ноды — не порт
    assert_eq!(port_at(&n, [250.0, 260.0], 1.0), None);
    // zoom 2.0: world-допуск = PORT_HIT_PX / 2
    let half = PORT_HIT_PX / 2.0;
    assert_eq!(
        port_at(&n, [250.0, 200.0 + half - 0.5], 2.0),
        Some(Side::Top)
    );
    assert_eq!(port_at(&n, [250.0, 200.0 + half + 0.5], 2.0), None);
}

/// edge_at: ближайшая связь в допуске, промах — None; висячие связи не мешают.
#[test]
fn edge_at_picks_nearest_within_tolerance() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(node("a", 0.0, 0.0, 100.0, 100.0));
    canvas.nodes.push(node("b", 500.0, 0.0, 100.0, 100.0));
    canvas.nodes.push(node("c", 0.0, 400.0, 100.0, 100.0));
    canvas.add_edge(Edge::new(
        "e1",
        "a",
        Some(Side::Right),
        "b",
        Some(Side::Left),
    ));
    canvas.add_edge(Edge::new(
        "e2",
        "a",
        Some(Side::Bottom),
        "c",
        Some(Side::Top),
    ));
    // Висячая связь — пропускается без паники
    canvas.add_edge(Edge::new("e3", "a", None, "missing", None));
    // Рядом с горизонтальной кривой e1 (y=50)
    assert_eq!(edge_at(&canvas, [250.0, 53.0]), Some(0));
    // Рядом с вертикальной кривой e2 (x=50)
    assert_eq!(edge_at(&canvas, [47.0, 250.0]), Some(1));
    // Промах
    assert_eq!(edge_at(&canvas, [250.0, 250.0]), None);
    assert_eq!(edge_at(&canvas, [250.0, 120.0]), None);
}

/// Резиновая линия: концы — порт и курсор, контрольная точка у порта по нормали.
#[test]
fn draft_curve_endpoints() {
    let port = [100.0, 50.0];
    let cursor = [400.0, 200.0];
    let curve = draft_curve(port, Side::Right, cursor);
    approx(curve.p0, port);
    approx(curve.p1, cursor);
    // c0 смещена по нормали Right (вправо)
    assert!(curve.c0[0] > port[0]);
    assert!((curve.c0[1] - port[1]).abs() < 1e-4);
    // c1 = курсору (без изгиба у конца)
    approx(curve.c1, cursor);
    // Тесселяция замкнута на концах
    let points = tessellate(&curve, 8);
    approx(points[0], port);
    approx(points[8], cursor);
}

/// Тест: Самопетля (edge от ноды к себе) — корректная геометрия
#[test]
fn self_loop_geometry() {
    let mut canvas = Canvas::default();
    let node = node("a", 100.0, 100.0, 200.0, 150.0);
    canvas.nodes.push(node);

    // Edge от ноды к самой себе
    let edge = Edge::new("self", "a", Some(Side::Right), "a", Some(Side::Left));
    canvas.add_edge(edge);

    // Геометрия должна корректно строиться
    let curve = edge_curve(&canvas, &canvas.edges[0]).expect("самопетля должна резолвиться");

    // Порты: правый и левый одной и той же ноды
    let p_right = port_point(&canvas.nodes[0], Side::Right);
    let p_left = port_point(&canvas.nodes[0], Side::Left);

    approx(curve.p0, p_right);
    approx(curve.p1, p_left);

    // Кривая должна иметь разумную форму (контрольные точки смещены)
    assert!(curve.c0[0] > curve.p0[0], "c0 вправо от правого порта");
    assert!(curve.c1[0] < curve.p1[0], "c1 влево от левого порта");

    // Hit-test работает
    let hit = edge_at(&canvas, [200.0, 175.0]); // примерно середина
    assert_eq!(hit, Some(0), "самопетля должна быть кликабельна");
}

/// Тест: Все 16 комбинаций fromSide → toSide
#[test]
fn all_16_side_combinations() {
    let mut canvas = Canvas::default();
    canvas.nodes.push(node("a", 0.0, 0.0, 100.0, 100.0));
    canvas.nodes.push(node("b", 400.0, 300.0, 100.0, 100.0));

    let sides = [Side::Top, Side::Right, Side::Bottom, Side::Left];
    let mut combinations = 0;

    for from_side in sides {
        for to_side in sides {
            let mut test_canvas = canvas.clone();
            let edge_id = format!("e-{from_side:?}-{to_side:?}");
            test_canvas.add_edge(Edge::new(edge_id, "a", Some(from_side), "b", Some(to_side)));

            let curve = edge_curve(&test_canvas, &test_canvas.edges[0]).expect(&format!(
                "комбинация {from_side:?} -> {to_side:?} должна резолвиться"
            ));

            // Концы в правильных портах
            assert_eq!(curve.p0, port_point(&test_canvas.nodes[0], from_side));
            assert_eq!(curve.p1, port_point(&test_canvas.nodes[1], to_side));

            // Контрольные точки смещены по нормалям
            let n0 = side_normal(from_side);
            let n1 = side_normal(to_side);
            // Минимум 40px смещения (MIN_CONTROL_OFFSET)
            assert!(
                (curve.c0[0] - (curve.p0[0] + n0[0] * 40.0)).abs() < 1.0
                    || (curve.c0[1] - (curve.p0[1] + n0[1] * 40.0)).abs() < 1.0,
                "c0 смещена по нормали from_side для {from_side:?}->{to_side:?}"
            );
            assert!(
                (curve.c1[0] - (curve.p1[0] + n1[0] * 40.0)).abs() < 1.0
                    || (curve.c1[1] - (curve.p1[1] + n1[1] * 40.0)).abs() < 1.0,
                "c1 смещена по нормали to_side для {from_side:?}->{to_side:?}"
            );

            combinations += 1;
        }
    }

    assert_eq!(combinations, 16, "должно быть 16 комбинаций");
}

/// Тест: Порт хит-зона при экстремальных зумах
#[test]
fn port_hitzone_at_extreme_zooms() {
    let node = node("a", 100.0, 200.0, 300.0, 120.0);

    // Zoom 0.05 (минимальный) — world-допуск = PORT_HIT_PX / 0.05 = 200px
    let port = port_at(&node, [250.0, 200.0], 0.05); // точно в top порту
    assert_eq!(
        port,
        Some(Side::Top),
        "при zoom 0.05 порт top должен находиться"
    );

    // В допуске 200px — ближайший порт (bottom на y=320, расстояние 30px)
    let port = port_at(&node, [250.0, 350.0], 0.05); // 150px ниже top, 30px ниже bottom
    assert_eq!(port, Some(Side::Bottom), "при zoom 0.05 ближайший — bottom");

    // За допуском
    let port = port_at(&node, [250.0, 550.0], 0.05); // 350px ниже top, 230px ниже bottom
    assert_eq!(port, None, "за допуском 200px — промах");

    // Zoom 4.0 (максимальный) — world-допуск = PORT_HIT_PX / 4.0 = 2.5px
    let port = port_at(&node, [250.0, 200.0], 4.0);
    assert_eq!(
        port,
        Some(Side::Top),
        "при zoom 4.0 порт top должен находиться"
    );

    let port = port_at(&node, [250.0, 202.0], 4.0); // 2px ниже
    assert_eq!(port, Some(Side::Top), "в допуске 2.5px");

    let port = port_at(&node, [250.0, 203.0], 4.0); // 3px ниже
    assert_eq!(port, None, "за допуском 2.5px — промах");

    // Zoom 1.0 — стандартный допуск 10px
    let port = port_at(&node, [250.0, 209.0], 1.0);
    assert_eq!(port, Some(Side::Top), "в допуске 10px");

    let port = port_at(&node, [250.0, 211.0], 1.0);
    assert_eq!(port, None, "за допуском 10px — промах");
}
