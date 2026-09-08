//! Интеграционные тесты для T8 (Связи / Edges).
//!
//! Тестируют полный цикл: создание связи через drag порта, редактирование
//   лейбла двойным кликом, удаление связи Del, каскадное удаление при удалении ноды.

use canvas_app::{
    body_area, edge_edit_area, map_key, session_area,
    test_helpers::{
        in_resize_corner, menu_item_at, menu_label, menu_rect, next_note_id, point_in_rect,
    },
    EditTarget, EditingSession, KeyCommand, Marker, Selection, BODY_LINE_HEIGHT, EDGE_EDIT_HEIGHT,
    EDGE_EDIT_WIDTH,
};
use canvas_core::{
    edge_at, edge_curve, nearest_side, port_at, port_point, Canvas, Edge, Node, Side, SpatialIndex,
};
use cosmic_text::FontSystem;
use winit::keyboard::{Key, NamedKey};

// Вспомогательная функция для создания тестовой сцены с двумя нодами
fn test_scene() -> (Canvas, SpatialIndex) {
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
    canvas
        .nodes
        .push(Node::file("b", "C:/b.png", 400.0, 0.0, 100.0, 100.0));
    let spatial = SpatialIndex::build(&canvas);
    (canvas, spatial)
}

/// Тест 1: Создание связи через drag от порта к другой ноде
#[test]
fn test_create_edge_via_port_drag() {
    let (mut canvas, mut spatial) = test_scene();

    // 1. Hover на ноду "a" → должны быть видны порты
    let zoom = 1.0;
    let port = port_at(&canvas.nodes[0], [50.0, 0.0], zoom); // top port
    assert_eq!(
        port,
        Some(Side::Top),
        "порт top должен быть найден при hover"
    );

    // 2. Начинаем drag от правого порта ноды "a"
    let from_node = canvas.nodes[0].id.clone();
    let from_side = Side::Right;
    let port_pos = port_point(&canvas.nodes[0], from_side);
    assert_eq!(port_pos, [100.0, 50.0], "позиция правого порта");

    // 3. Drag к курсору (резиновая линия)
    let cursor = [300.0, 50.0];
    let draft = canvas_core::draft_curve(port_pos, from_side, cursor);
    assert_eq!(draft.p0, port_pos);
    assert_eq!(draft.p1, cursor);
    // Контрольная точка у порта смещена по нормали (вправо для Right)
    assert!(draft.c0[0] > port_pos[0]);

    // 4. Drop на ноду "b" → создаётся edge
    let to_node = &canvas.nodes[1];
    let to_side = nearest_side(to_node, cursor);
    assert_eq!(to_side, Side::Left, "ближайшая сторона к курсору — левая");

    let edge_id = canvas.next_edge_id();
    let edge = Edge::new(
        edge_id,
        from_node.clone(),
        Some(from_side),
        to_node.id.clone(),
        Some(to_side),
    );
    canvas.add_edge(edge);

    // 5. Проверяем, что edge добавился
    assert_eq!(canvas.edges.len(), 1);
    assert_eq!(canvas.edges[0].from_node, "a");
    assert_eq!(canvas.edges[0].to_node, "b");
    assert_eq!(canvas.edges[0].from_side, Some(Side::Right));
    assert_eq!(canvas.edges[0].to_side, Some(Side::Left));

    // 6. Spatial index обновился (индексы нод не изменились)
    spatial.update(0, &canvas.nodes[0]);
    spatial.update(1, &canvas.nodes[1]);

    // 7. Геометрия связи корректна
    let curve = edge_curve(&canvas, &canvas.edges[0]).expect("связь должна резолвиться");
    assert_eq!(curve.p0, [100.0, 50.0]); // правый порт a
    assert_eq!(curve.p1, [400.0, 50.0]); // левый порт b

    // 8. Hit-test связи работает
    let hit = edge_at(&canvas, [250.0, 53.0]); // рядом с кривой
    assert_eq!(hit, Some(0), "клик рядом с линией должен попадать в edge");

    let miss = edge_at(&canvas, [250.0, 200.0]); // далеко
    assert_eq!(miss, None, "клик далеко от линии не должен попадать");
}

/// Тест 2: Редактирование лейбла связи двойным кликом
#[test]
fn test_edit_edge_label_double_click() {
    let (mut canvas, _spatial) = test_scene();

    // Создаём связь с лейблом
    let edge = Edge::new("e1", "a", Some(Side::Right), "b", Some(Side::Left));
    canvas.add_edge(edge);
    assert_eq!(canvas.edges.len(), 1);

    // Позиция бокса редактирования — по центру кривой
    let edit_area = edge_edit_area(&canvas, 0).expect("область редактирования должна существовать");
    let (origin, width, height) = edit_area;

    // Проверяем размеры бокса
    assert_eq!(width, EDGE_EDIT_WIDTH);
    assert_eq!(height, EDGE_EDIT_HEIGHT);

    // Центр бокса = середина кривой (t=0.5)
    let curve = edge_curve(&canvas, &canvas.edges[0]).expect("кривая существует");
    let mid = canvas_core::curve_point(&curve, 0.5);
    let expected_origin = [
        mid[0] - EDGE_EDIT_WIDTH / 2.0,
        mid[1] - EDGE_EDIT_HEIGHT / 2.0,
    ];
    assert!((origin[0] - expected_origin[0]).abs() < 1e-3);
    assert!((origin[1] - expected_origin[1]).abs() < 1e-3);

    // Начинаем редактирование (симуляция двойного клика)
    let mut font_system = FontSystem::new();
    let mut session = EditingSession::new(
        &mut font_system,
        EditTarget::Edge(0),
        "блокирует", // исходный текст лейбла
        width * 1.0, // zoom_px = 1.0
        height * 1.0,
        1.0,
    );

    // Проверяем цель редактирования
    assert_eq!(session.target(), EditTarget::Edge(0));
    assert_eq!(session.node_index(), None);
    assert_eq!(session.text(), "блокирует");

    // Вводим новый текст
    session.insert_text(&mut font_system, " зависит от");
    assert_eq!(session.text(), "блокирует зависит от");

    // Commit (Enter) — текст сохраняется в модель
    let new_label = session.text().trim().to_owned();
    canvas.edges[0].label = Some(new_label.clone());

    assert_eq!(
        canvas.edges[0].label,
        Some("блокирует зависит от".to_owned())
    );

    // Cancel (Esc) — откат к исходному
    let mut session2 = EditingSession::new(
        &mut font_system,
        EditTarget::Edge(0),
        "новый лейбл",
        width * 1.0,
        height * 1.0,
        1.0,
    );
    session2.insert_text(&mut font_system, " изменение");
    assert_eq!(session2.text(), "новый лейбл изменение");

    // При cancel модель не меняется
    assert_ne!(session2.text(), session2.original());
}

/// Тест 3: Удаление связи клавишей Del
#[test]
fn test_delete_edge_via_del_key() {
    let (mut canvas, _spatial) = test_scene();

    // Создаём несколько связей
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
        "b",
        Some(Side::Top),
    ));
    assert_eq!(canvas.edges.len(), 2);

    // Выбираем первую связь (индекс 0)
    let selected = Selection::Edge(0);

    // Удаляем выбранную связь (как в App::delete_selected)
    match selected {
        Selection::Edge(index) => {
            if let Some(edge) = canvas.edges.get(index) {
                let id = edge.id.clone();
                let removed = canvas.remove_edge(&id);
                assert!(removed, "связь должна быть удалена");
            }
        }
        _ => panic!("ожидалось Selection::Edge"),
    }

    assert_eq!(canvas.edges.len(), 1);
    assert_eq!(canvas.edges[0].id, "e2");

    // Удаляем вторую связь
    canvas.remove_edge("e2");
    assert_eq!(canvas.edges.len(), 0);
    assert!(
        !canvas.remove_edge("e2"),
        "повторное удаление возвращает false"
    );

    // Удаление несуществующей связи
    assert!(!canvas.remove_edge("nonexistent"));
}

/// Тест 4: Каскадное удаление связей при удалении ноды
#[test]
fn test_cascade_delete_node_removes_edges() {
    let mut canvas = Canvas::default();

    // Три ноды в линии
    canvas
        .nodes
        .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
    canvas
        .nodes
        .push(Node::file("b", "C:/b.png", 300.0, 0.0, 100.0, 100.0));
    canvas
        .nodes
        .push(Node::file("c", "C:/c.png", 600.0, 0.0, 100.0, 100.0));

    // Связи: a→b, b→c, a→c
    canvas.add_edge(Edge::new("e1", "a", None, "b", None));
    canvas.add_edge(Edge::new("e2", "b", None, "c", None));
    canvas.add_edge(Edge::new("e3", "a", None, "c", None));
    assert_eq!(canvas.edges.len(), 3);

    // Удаляем среднюю ноду "b" (индекс 1)
    let removed = canvas.remove_node(1).expect("нода b должна существовать");
    assert_eq!(removed.id, "b");

    // Ноды: a (индекс 0), c (индекс 1 теперь)
    assert_eq!(canvas.nodes.len(), 2);
    assert_eq!(canvas.nodes[0].id, "a");
    assert_eq!(canvas.nodes[1].id, "c");

    // Связи: только e3 (a→c) выжила, e1 и e2 удалены каскадно
    assert_eq!(canvas.edges.len(), 1);
    assert_eq!(canvas.edges[0].id, "e3");
    assert_eq!(canvas.edges[0].from_node, "a");
    assert_eq!(canvas.edges[0].to_node, "c");

    // edges_of для оставшихся нод
    assert_eq!(canvas.edges_of("a"), vec![0]);
    assert_eq!(canvas.edges_of("c"), vec![0]);
}

/// Тест 5: Самопетля (edge от ноды к себе)
#[test]
fn test_self_loop_geometry() {
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::file("a", "C:/a.png", 100.0, 100.0, 200.0, 150.0));

    // Edge от ноды к самой себе
    let edge = Edge::new("self", "a", Some(Side::Right), "a", Some(Side::Left));
    canvas.add_edge(edge);

    // Геометрия должна корректно строиться
    let curve = edge_curve(&canvas, &canvas.edges[0]).expect("самопетля должна резолвиться");

    // Порты: правый и левый одной и той же ноды
    let p_right = port_point(&canvas.nodes[0], Side::Right);
    let p_left = port_point(&canvas.nodes[0], Side::Left);

    assert_eq!(curve.p0, p_right);
    assert_eq!(curve.p1, p_left);

    // Кривая должна иметь разумную форму (контрольные точки смещены)
    assert!(curve.c0[0] > curve.p0[0], "c0 вправо от правого порта");
    assert!(curve.c1[0] < curve.p1[0], "c1 влево от левого порта");

    // Hit-test работает
    let hit = edge_at(&canvas, [200.0, 175.0]); // примерно середина
    assert_eq!(hit, Some(0), "самопетля должна быть кликабельна");
}

/// Тест 6: Все 16 комбинаций fromSide → toSide
#[test]
fn test_all_16_side_combinations() {
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
    canvas
        .nodes
        .push(Node::file("b", "C:/b.png", 400.0, 300.0, 100.0, 100.0));

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
            let n0 = canvas_core::side_normal(from_side);
            let n1 = canvas_core::side_normal(to_side);
            assert!(
                (curve.c0[0] - (curve.p0[0] + n0[0] * 40.0)).abs() < 1.0
                    || (curve.c0[1] - (curve.p0[1] + n0[1] * 40.0)).abs() < 1.0,
                "c0 смещена по нормали from_side"
            );
            assert!(
                (curve.c1[0] - (curve.p1[0] + n1[0] * 40.0)).abs() < 1.0
                    || (curve.c1[1] - (curve.p1[1] + n1[1] * 40.0)).abs() < 1.0,
                "c1 смещена по нормали to_side"
            );

            combinations += 1;
        }
    }

    assert_eq!(combinations, 16, "должно быть 16 комбинаций");
}

/// Тест 7: Порт хит-зона при экстремальных зумах
#[test]
fn test_port_hitzone_at_extreme_zooms() {
    let node = Node::file("a", "C:/a.png", 100.0, 200.0, 300.0, 120.0);

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

    // За допуском (210px от top, 90px от bottom — но bottom тоже в допуске 200)
    // Нужно идти дальше: 200+120+100 = 420 от верха ноды = 620 абсолютно
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

/// Тест 8: Сессия редактирования лейбла связи — зум
#[test]
fn test_edge_edit_session_zoom() {
    let (mut canvas, _spatial) = test_scene();
    canvas.add_edge(Edge::new(
        "e1",
        "a",
        Some(Side::Right),
        "b",
        Some(Side::Left),
    ));

    let (_origin, width, height) = edge_edit_area(&canvas, 0).expect("area exists");

    let mut font_system = FontSystem::new();
    let mut session = EditingSession::new(
        &mut font_system,
        EditTarget::Edge(0),
        "test label",
        width * 1.0,
        height * 1.0,
        1.0,
    );

    // Изменяем зум — буфер должен перешейпиться
    session.set_layout(&mut font_system, width * 2.0, height * 2.0, 2.0);

    assert_eq!(session.text(), "test label");
    // layout — приватное поле, проверяем через результат
    let caret = session.caret_rect(&mut font_system).expect("caret exists");
    assert_eq!(caret[3], BODY_LINE_HEIGHT * 2.0);
}

/// Тест 9: Маппинг клавиш для редактирования связи
#[test]
fn test_key_mapping_for_edge_editing() {
    // Enter без Shift — Commit
    assert_eq!(
        map_key(&Key::Named(NamedKey::Enter), false, false),
        Some(KeyCommand::Commit)
    );

    // Shift+Enter — новая строка (Action::Enter)
    assert_eq!(
        map_key(&Key::Named(NamedKey::Enter), false, true),
        Some(KeyCommand::Action(cosmic_text::Action::Enter))
    );

    // Esc — Cancel
    assert_eq!(
        map_key(&Key::Named(NamedKey::Escape), false, false),
        Some(KeyCommand::Cancel)
    );

    // Ctrl+Enter — тоже Commit
    assert_eq!(
        map_key(&Key::Named(NamedKey::Enter), true, false),
        Some(KeyCommand::Commit)
    );

    // Backspace/Delete
    assert_eq!(
        map_key(&Key::Named(NamedKey::Backspace), false, false),
        Some(KeyCommand::Action(cosmic_text::Action::Backspace))
    );

    // Навигация
    assert_eq!(
        map_key(&Key::Named(NamedKey::ArrowLeft), true, false),
        Some(KeyCommand::Motion(cosmic_text::Motion::LeftWord, false))
    );

    // Форматирование (Ctrl+B/I/H)
    assert_eq!(
        map_key(&Key::Character("b".into()), true, false),
        Some(KeyCommand::ToggleMarker(Marker::Bold))
    );
}

/// Тест 10: Удаление ноды с ребилдом spatial index
#[test]
fn test_node_deletion_rebuilds_spatial() {
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
    canvas
        .nodes
        .push(Node::file("b", "C:/b.png", 300.0, 0.0, 100.0, 100.0));
    canvas
        .nodes
        .push(Node::file("c", "C:/c.png", 600.0, 0.0, 100.0, 100.0));

    canvas.add_edge(Edge::new("e1", "a", None, "b", None));
    canvas.add_edge(Edge::new("e2", "b", None, "c", None));

    let mut spatial = SpatialIndex::build(&canvas);
    assert_eq!(
        spatial.query_rect([-1000.0, -1000.0, 2000.0, 2000.0]).len(),
        3
    );

    // Удаляем среднюю ноду (как в App::delete_selected)
    canvas.remove_node(1);
    spatial = SpatialIndex::build(&canvas); // полный ребилд

    assert_eq!(
        spatial.query_rect([-1000.0, -1000.0, 2000.0, 2000.0]).len(),
        2
    );

    // Hit-test работает с новыми индексами
    let hit = spatial.hit_test([50.0, 50.0]); // нода a
    assert_eq!(hit, Some(0));

    let hit = spatial.hit_test([650.0, 50.0]); // нода c (была индекс 2, стала 1)
    assert_eq!(hit, Some(1));
}
