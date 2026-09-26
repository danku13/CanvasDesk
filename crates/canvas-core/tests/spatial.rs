//! T5: spatial index (R-tree) над AABB нод — запросы по rect, инкрементальные
//! обновления, ускоренный hit-test (SPEC §4, §6.3).

use canvas_core::{Canvas, Node, SpatialIndex};

fn three_nodes() -> Canvas {
    let mut canvas = Canvas::default();
    // a: [0,0]-[100,100]
    canvas
        .nodes
        .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
    // b: [50,50]-[150,150] — перекрывает a
    canvas
        .nodes
        .push(Node::file("b", "C:/b.png", 50.0, 50.0, 100.0, 100.0));
    // c: [500,500]-[600,600] — далеко
    canvas.nodes.push(Node::text("c", "далеко", 500.0, 500.0));
    canvas
}

/// query_rect возвращает только пересекающиеся ноды, индексы отсортированы.
#[test]
fn query_rect_returns_intersecting_sorted() {
    let canvas = three_nodes();
    let index = SpatialIndex::build(&canvas);

    // Rect накрывает a и b, но не c
    let hits = index.query_rect([0.0, 0.0, 200.0, 200.0]);
    assert_eq!(hits, vec![0, 1]);

    // Частичное пересечение углом — тоже попадание
    let hits = index.query_rect([140.0, 140.0, 510.0, 510.0]);
    assert_eq!(hits, vec![1, 2]);

    // Полный промах
    assert!(index.query_rect([900.0, 900.0, 1000.0, 1000.0]).is_empty());
}

/// update перемещает ноду инкрементально: старый rect её не находит, новый — находит.
#[test]
fn update_moves_node() {
    let canvas = three_nodes();
    let mut index = SpatialIndex::build(&canvas);

    let mut moved = canvas.nodes[2].clone();
    moved.x = 10.0;
    moved.y = 10.0;
    index.update(2, &moved);

    assert_eq!(index.query_rect([0.0, 0.0, 300.0, 300.0]), vec![0, 1, 2]);
    assert!(index.query_rect([490.0, 490.0, 700.0, 700.0]).is_empty());
}

/// insert добавляет ноду; remove убирает по индексу.
#[test]
fn insert_and_remove() {
    let canvas = three_nodes();
    let mut index = SpatialIndex::build(&canvas);

    let extra = Node::file("d", "C:/d.png", 700.0, 700.0, 50.0, 50.0);
    index.insert(3, &extra);
    assert_eq!(index.query_rect([650.0, 650.0, 800.0, 800.0]), vec![3]);

    index.remove(3);
    assert!(index.query_rect([650.0, 650.0, 800.0, 800.0]).is_empty());

    // Остальные ноды на месте
    assert_eq!(index.query_rect([0.0, 0.0, 200.0, 200.0]), vec![0, 1]);
}

/// hit_test: промах, попадание, при перекрытии — верхняя (больший индекс) нода.
#[test]
fn hit_test_matches_linear_semantics() {
    let canvas = three_nodes();
    let index = SpatialIndex::build(&canvas);

    assert_eq!(index.hit_test([500.0, 500.0]), Some(2));
    assert_eq!(index.hit_test([10.0, 10.0]), Some(0));
    assert_eq!(index.hit_test([75.0, 75.0]), Some(1)); // перекрытие — верхняя b
    assert_eq!(index.hit_test([900.0, 900.0]), None);
    // Границы включительно, как у Canvas::hit_test
    assert_eq!(index.hit_test([100.0, 100.0]), Some(1));
}

/// Раздельные запросы нод и групп (FR-012 v2): query_rect_nodes не
/// возвращает группы, query_rect_groups — только группы; query_rect —
/// всё вместе (совместимость).
#[test]
fn split_queries_nodes_vs_groups() {
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::file("a", "C:/a.png", 10.0, 10.0, 100.0, 80.0));
    canvas.nodes.push(Node::group("g", 0.0, 0.0, 400.0, 300.0));
    canvas
        .nodes
        .push(Node::file("b", "C:/b.png", 200.0, 50.0, 60.0, 40.0));
    let index = SpatialIndex::build(&canvas);

    let rect = [50.0, 40.0, 120.0, 60.0]; // накрывает a, задевает g, мимо b
    assert_eq!(index.query_rect(rect), vec![0, 1], "всё вместе");
    assert_eq!(index.query_rect_nodes(rect), vec![0], "только ноды");
    assert_eq!(index.query_rect_groups(rect), vec![1], "только группы");

    // Точка внутри группы и внутри ноды-ребёнка: nodes — ребёнок, groups —
    // группа (без фильтров вручную)
    let point = [220.0, 60.0];
    let pt = [point[0], point[1], point[0], point[1]];
    assert_eq!(index.query_rect_nodes(pt), vec![2]);
    assert_eq!(index.query_rect_groups(pt), vec![1]);

    // insert после build сохраняет признак группы
    let mut index2 = SpatialIndex::build(&Canvas::default());
    index2.insert(0, &Node::group("g2", 0.0, 0.0, 50.0, 50.0));
    index2.insert(1, &Node::text("t", "x", 10.0, 10.0));
    let pt = [10.0, 10.0, 10.0, 10.0];
    assert_eq!(index2.query_rect_groups(pt), vec![0]);
    assert_eq!(index2.query_rect_nodes(pt), vec![1]);
}

/// Property-style: hit_test индекса совпадает с линейным Canvas::hit_test
/// на случайных точках (детерминированный PRNG).
#[test]
fn hit_test_matches_linear_on_random_points() {
    let mut canvas = Canvas::default();
    let mut state = 0x1234_5678u32;
    let mut next = move || {
        // xorshift32 — детерминированный PRNG без зависимостей
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        state
    };
    for i in 0..50 {
        let x = (next() % 800) as f32;
        let y = (next() % 800) as f32;
        let w = 20.0 + (next() % 180) as f32;
        let h = 20.0 + (next() % 180) as f32;
        canvas
            .nodes
            .push(Node::file(format!("n{i}"), "C:/f.png", x, y, w, h));
    }
    let index = SpatialIndex::build(&canvas);
    for _ in 0..500 {
        let point = [(next() % 1000) as f32, (next() % 1000) as f32];
        assert_eq!(
            index.hit_test(point),
            canvas.hit_test(point),
            "расхождение в точке {point:?}"
        );
    }
}

/// Пустой канвас и нода с нулевыми размерами не паникуют.
#[test]
fn degenerate_cases_do_not_panic() {
    let empty = Canvas::default();
    let index = SpatialIndex::build(&empty);
    assert!(index.query_rect([0.0, 0.0, 100.0, 100.0]).is_empty());
    assert_eq!(index.hit_test([0.0, 0.0]), None);

    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::file("z", "C:/z.png", 5.0, 5.0, 0.0, 0.0));
    let index = SpatialIndex::build(&canvas);
    assert_eq!(index.hit_test([5.0, 5.0]), Some(0));
    assert_eq!(index.hit_test([5.1, 5.0]), None);
}
