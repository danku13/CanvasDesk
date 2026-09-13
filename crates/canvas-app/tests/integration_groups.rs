//! Интеграционные тесты для групп нод (v1).
//!
//! Группа — нода `node_type = "group"`, подпись в `label`. Дети определяются
//! геометрией (центр внутри rect группы). Тестируют полный цикл по образцу
//! integration_edges.rs: создание через меню («Сгруппировать» / «Создать
//! группу»), drag группы со сдвигом детей, выборочный hit-test (ребёнок
//! раньше группы), редактирование подписи двойным кликом, удаление группы
//! без каскада по детям.

use std::str::FromStr;

use canvas_app::ui::{
    menu_item_at_for, plan_group_around, plan_group_at, select_node_hit, CanvasMenuItem,
    NodeMenuItem, CANVAS_MENU_ITEMS, GROUP_HEIGHT, GROUP_PADDING, GROUP_WIDTH, MENU_ITEM_HEIGHT,
    MENU_PADDING, NODE_MENU_ITEMS,
};
use canvas_app::{Camera, EditTarget, EditingSession, KeyCommand, Selection};
use canvas_core::{group_children, Canvas, Node, NodeKind, SpatialIndex};
use cosmic_text::FontSystem;

/// Сцена: группа с двумя детьми и одной нодой снаружи.
fn group_scene() -> (Canvas, SpatialIndex) {
    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::group("g", 0.0, 0.0, 400.0, 300.0));
    canvas
        .nodes
        .push(Node::file("child-1", "C:/a.png", 50.0, 50.0, 100.0, 80.0));
    canvas.nodes.push(Node::file(
        "child-2", "C:/b.png", 200.0, 150.0, 120.0, 100.0,
    ));
    canvas
        .nodes
        .push(Node::file("outside", "C:/c.png", 600.0, 0.0, 100.0, 100.0));
    let spatial = SpatialIndex::build(&canvas);
    (canvas, spatial)
}

/// Тест 1: «Сгруппировать» из меню ноды — группа с bbox = нода + padding 40
#[test]
fn test_group_via_node_menu() {
    let (mut canvas, mut spatial) = group_scene();

    // Меню ноды: пункт «Сгруппировать» — последний (после разделителя)
    assert_eq!(NODE_MENU_ITEMS.len(), 9);
    assert_eq!(NODE_MENU_ITEMS[7], NodeMenuItem::Separator);
    assert_eq!(NODE_MENU_ITEMS[8], NodeMenuItem::Group);

    // Клик по пункту меню (симуляция: hit-test пункта → Group)
    let origin = [100.0, 50.0];
    let group_item_y = 50.0 + MENU_PADDING + 8.0 * MENU_ITEM_HEIGHT + 3.0;
    assert_eq!(
        menu_item_at_for(origin, [110.0, group_item_y], NODE_MENU_ITEMS.len()),
        Some(8),
        "пункт «Сгруппировать» — индекс 8"
    );

    // План группы вокруг выбранной ноды (child-1, индекс 1)
    let group = plan_group_around(&canvas, 1, GROUP_PADDING).expect("нода есть");
    assert_eq!(group.kind(), NodeKind::Group);
    assert_eq!(group.id, "group-1");
    assert_eq!(group.label.as_deref(), Some("Группа"));
    // bbox = rect ноды + padding 40 по всем сторонам
    assert_eq!(
        (group.x, group.y, group.width, group.height),
        (50.0 - 40.0, 50.0 - 40.0, 100.0 + 80.0, 80.0 + 80.0)
    );

    // Вставка (паттерн App::insert_group): модель + spatial + выделение
    canvas.nodes.push(group);
    let index = canvas.nodes.len() - 1;
    spatial.insert(index, &canvas.nodes[index]);
    let selected = Some(Selection::Node(index));
    assert_eq!(selected, Some(Selection::Node(4)), "выбрана новая группа");
    // Новая группа (10..190 × 10..190) лежит внутри исходной g — дети g:
    assert_eq!(
        group_children(&canvas, 0),
        vec![1, 2, 4],
        "новая группа внутри g"
    );
}

/// Тест 2: «Создать группу» из меню пустого канваса — 400×300 в центре viewport
#[test]
fn test_create_group_at_viewport_center() {
    let (canvas, _spatial) = group_scene();

    // Меню пустого канваса: «Создать группу» + «Фокус на связях» (T23)
    assert_eq!(CANVAS_MENU_ITEMS.len(), 2);
    assert_eq!(CANVAS_MENU_ITEMS[0], CanvasMenuItem::NewGroup);
    assert_eq!(CANVAS_MENU_ITEMS[1], CanvasMenuItem::FocusMode);

    // Центр viewport в мировых координатах (паттерн App::viewport_center_world)
    let viewport = [1600.0, 900.0];
    let mut camera = Camera::default();
    camera.pan([120.0, -60.0]);
    let rect = camera.visible_world_rect(viewport);
    let center = [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0];

    let group = plan_group_at(&canvas, center);
    assert_eq!(group.kind(), NodeKind::Group);
    assert_eq!(group.width, GROUP_WIDTH);
    assert_eq!(group.height, GROUP_HEIGHT);
    assert_eq!(group.label.as_deref(), Some("Группа"));
    // Группа отцентрирована в центре viewport
    assert!(
        (group.x + group.width / 2.0 - center[0]).abs() < 1e-3,
        "центр по x: {}",
        group.x + group.width / 2.0
    );
    assert!(
        (group.y + group.height / 2.0 - center[1]).abs() < 1e-3,
        "центр по y: {}",
        group.y + group.height / 2.0
    );
    // Центр viewport — это позиция камеры после панорамирования
    let expected = camera.position();
    assert!((center[0] - expected[0]).abs() < 1e-3);
    assert!((center[1] - expected[1]).abs() < 1e-3);
}

/// Тест 3: Drag группы двигает группу и детей на один дельта-вектор
#[test]
fn test_drag_group_moves_children() {
    let (mut canvas, mut spatial) = group_scene();
    // Исходные позиции
    assert_eq!(group_children(&canvas, 0), vec![1, 2]);

    // Drag: сдвиг на (30, -20) (паттерн App::on_cursor_moved для группы —
    // дельта от позиции группы, translate_group + spatial.update)
    let (old_x, old_y) = (canvas.nodes[0].x, canvas.nodes[0].y);
    let moved = canvas.translate_group(0, 30.0, -20.0);
    for index in &moved {
        spatial.update(*index, &canvas.nodes[*index]);
    }
    assert_eq!(moved, vec![0, 1, 2], "группа + оба ребёнка");
    assert_eq!(
        (canvas.nodes[0].x - old_x, canvas.nodes[0].y - old_y),
        (30.0, -20.0)
    );
    for index in 1..=2 {
        assert_eq!(
            (canvas.nodes[index].x, canvas.nodes[index].y),
            match index {
                1 => (80.0, 30.0),
                _ => (230.0, 130.0),
            },
            "ребёнок {index} сдвинут на тот же вектор"
        );
    }
    // Снаружи — на месте
    assert_eq!((canvas.nodes[3].x, canvas.nodes[3].y), (600.0, 0.0));

    // Hit-test после drag: дети следуют за группой (spatial обновлён)
    let candidates = spatial.query_rect([80.0, 30.0, 80.0, 30.0]);
    assert!(candidates.contains(&0) && candidates.contains(&1));
    assert_eq!(
        select_node_hit(&canvas, &candidates),
        Some(1),
        "по свободному месту ребёнка — ребёнок, не группа"
    );
}

/// Тест 4: Выборочный hit-test — клик по ребёнку выбирает ребёнка,
/// по свободной части группы — группу
#[test]
fn test_selective_hit_child_before_group() {
    let (canvas, spatial) = group_scene();

    // Точка внутри child-1: под ней и группа, и ребёнок
    let point = [100.0, 90.0];
    let candidates = spatial.query_rect([point[0], point[1], point[0], point[1]]);
    assert!(candidates.contains(&0) && candidates.contains(&1));
    assert_eq!(
        select_node_hit(&canvas, &candidates),
        Some(1),
        "ребёнок выбирается раньше группы"
    );

    // Свободная часть группы (вне детей): выбор — группа
    let point = [350.0, 280.0];
    let candidates = spatial.query_rect([point[0], point[1], point[0], point[1]]);
    assert_eq!(candidates, vec![0]);
    assert_eq!(select_node_hit(&canvas, &candidates), Some(0));

    // Вне группы: outside-нода
    let point = [650.0, 50.0];
    let candidates = spatial.query_rect([point[0], point[1], point[0], point[1]]);
    assert_eq!(select_node_hit(&canvas, &candidates), Some(3));
}

/// Тест 5: Редактирование подписи группы (двойной клик) — commit пишет label,
/// пустой текст сбрасывает в None
#[test]
fn test_group_label_edit_commit() {
    let (mut canvas, _spatial) = group_scene();
    canvas.nodes[0].label = Some("Спринт 1".to_owned());

    // Двойной клик по группе: сессия редактирования с текстом = label
    // (паттерн App::begin_editing для группы)
    let mut font_system = FontSystem::new();
    let label = canvas.nodes[0].label.clone().unwrap_or_default();
    let mut session = EditingSession::new(
        &mut font_system,
        EditTarget::Node(0),
        &label,
        300.0,
        200.0,
        1.0,
    );
    assert_eq!(session.target(), EditTarget::Node(0));
    assert_eq!(session.text(), "Спринт 1");

    // Правим подпись и коммитим (паттерн App::finish_editing для группы:
    // label = trim(text), пусто → None)
    session.apply(&mut font_system, KeyCommand::SelectAll);
    session.insert_text(&mut font_system, "Спринт 2");
    let text = session.text();
    let text = text.trim();
    canvas.nodes[0].label = if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    };
    assert_eq!(canvas.nodes[0].label, Some("Спринт 2".to_owned()));

    // Пустой лейбл → None (заголовок станет дефолтным «Группа»)
    let mut session = EditingSession::new(
        &mut font_system,
        EditTarget::Node(0),
        "Спринт 2",
        300.0,
        200.0,
        1.0,
    );
    session.apply(&mut font_system, KeyCommand::SelectAll);
    session.apply(
        &mut font_system,
        KeyCommand::Action(cosmic_text::Action::Backspace),
    );
    assert_eq!(session.text(), "");
    let text = session.text();
    let text = text.trim();
    canvas.nodes[0].label = if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    };
    assert_eq!(canvas.nodes[0].label, None);
}

/// Тест 6: Удаление группы не удаляет детей (как Obsidian)
#[test]
fn test_delete_group_keeps_children() {
    let (mut canvas, _spatial) = group_scene();
    assert_eq!(group_children(&canvas, 0), vec![1, 2]);

    // Выбрана группа (индекс 0) — Del: remove_node каскадит только связи
    let selected = Selection::Node(0);
    match selected {
        Selection::Node(index) => {
            canvas.remove_node(index);
        }
        _ => panic!("ожидалось Selection::Node"),
    }
    assert_eq!(canvas.nodes.len(), 3);
    assert!(canvas
        .nodes
        .iter()
        .all(|node| node.kind() != NodeKind::Group));
    assert!(canvas.nodes.iter().any(|node| node.id == "child-1"));
    assert!(canvas.nodes.iter().any(|node| node.id == "child-2"));
}

/// Тест 7: Round-trip `.canvas` с группой — подпись и геометрия доезжают
#[test]
fn test_group_round_trip() {
    let (mut canvas, _spatial) = group_scene();
    canvas.nodes[0].label = Some("Архив".to_owned());

    let json = canvas.to_json().expect("сериализация");
    assert!(json.contains("\"type\": \"group\""), "{json}");
    assert!(json.contains("\"label\": \"Архив\""), "{json}");

    let parsed = Canvas::from_str(&json).expect("парсинг");
    assert_eq!(parsed.nodes[0].kind(), NodeKind::Group);
    assert_eq!(parsed.nodes[0].label.as_deref(), Some("Архив"));
    assert_eq!(
        (
            parsed.nodes[0].x,
            parsed.nodes[0].y,
            parsed.nodes[0].width,
            parsed.nodes[0].height
        ),
        (0.0, 0.0, 400.0, 300.0)
    );
}
