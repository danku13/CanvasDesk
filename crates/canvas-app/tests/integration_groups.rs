//! Интеграционные тесты для групп нод (v1).
//!
//! Группа — нода `node_type = "group"`, подпись в `label`. Дети определяются
//! геометрией (центр внутри rect группы). Тестируют полный цикл по образцу
//! integration_edges.rs: создание через меню («Сгруппировать» / «Создать
//! группу»), drag группы со сдвигом детей, выборочный hit-test (ребёнок
//! раньше группы), редактирование подписи двойным кликом, удаление группы
//! без каскада по детям.

use std::str::FromStr;

use canvas_app::palette::{
    palette_bar_size, palette_groups, palette_hit, palette_layout, palette_origin,
    palette_trigger_at, PaletteAction, PaletteHit, PaletteTarget,
};
use canvas_app::ui::{
    plan_group_around, plan_group_at, select_node_hit, CanvasMenuItem, CANVAS_MENU_ITEMS,
    GROUP_HEIGHT, GROUP_PADDING, GROUP_WIDTH,
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

/// Тест 1: «Сгруппировать» из палитры выделения — группа с bbox = нода +
/// padding 40 (уточнение владельца: FR-009/FR-010 — палитра под выделением)
#[test]
fn test_group_via_node_menu() {
    let (mut canvas, mut spatial) = group_scene();

    // Палитра выделения для ноды child-1 (индекс 1): группа «Действия»
    // содержит действие NodeGroup (старое текстовое меню заменено палитрой)
    let groups = palette_groups(
        &canvas,
        &PaletteTarget::Nodes {
            primary: 1,
            selected: vec![1],
        },
    );
    let actions = groups
        .iter()
        .find(|g| g.label == "Действия")
        .expect("группа «Действия» в палитре");
    let group_entry = actions
        .entries
        .iter()
        .find(|e| e.label == "Сгруппировать")
        .expect("пункт «Сгруппировать»");
    assert!(matches!(group_entry.action, PaletteAction::NodeGroup(1)));

    // Клик по строке выпадашки группы «Действия» (симуляция: hover на
    // кнопке группы → открытая колонка → hit-test строки «Сгруппировать»)
    let viewport = [1600.0, 900.0];
    let anchor = [800.0, 400.0];
    let origin = palette_origin(anchor, palette_bar_size(&groups), viewport);
    let lay = palette_layout(origin, &groups, viewport);
    let ai = groups.iter().position(|g| g.label == "Действия").unwrap();
    // Hover на кнопке группы — триггер раскрытия колонки (раскрытие
    // срабатывает ТОЛЬКО от кнопки, не от пустой области колонки)
    let btn = lay.groups[ai].button;
    assert_eq!(
        palette_trigger_at(&lay, [btn[0] + 5.0, btn[1] + 5.0]),
        Some(ai)
    );
    // Строка «Сгруппировать» (третья: после Переименовать/Дублировать)
    // кликабельна при ОТКРЫТОЙ группе
    let ei = actions
        .entries
        .iter()
        .position(|e| e.label == "Сгруппировать")
        .expect("индекс строки");
    let row = lay.groups[ai].rows[ei];
    let hit = palette_hit(&lay, [row[0] + 5.0, row[1] + 5.0], Some(ai));
    assert_eq!(
        hit,
        Some(PaletteHit::Entry { group: ai, entry: ei }),
        "строка кликабельна"
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

    // Меню пустого канваса: «Создать группу» + «Фокус на связях» (T23) +
    // «Горячие клавиши (F1)» (FR-004.1) + «Виджеты ▸…» (M5 T20-F)
    assert_eq!(CANVAS_MENU_ITEMS.len(), 4);
    assert_eq!(CANVAS_MENU_ITEMS[0], CanvasMenuItem::NewGroup);
    assert_eq!(CANVAS_MENU_ITEMS[1], CanvasMenuItem::FocusMode);
    assert_eq!(CANVAS_MENU_ITEMS[2], CanvasMenuItem::Hotkeys);
    assert_eq!(CANVAS_MENU_ITEMS[3], CanvasMenuItem::Widgets);

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

// --- FR-012: явное членство групп («втягивание») ---

use canvas_core::{group_add_children, group_expand_to_children, group_remove_child, plan_push_out};

/// FR-012 (главный регресс): случайное перекрытие не подвязывает ноду к
/// группе с явным списком детей; «Сгруппировать» материализует список.
#[test]
fn test_random_overlap_does_not_attach() {
    let (mut canvas, _spatial) = group_scene();
    // Новые группы (plan_group_around) создаются с ЯВНЫМ списком:
    let group = plan_group_around(&canvas, 3, GROUP_PADDING).expect("нода есть");
    assert_eq!(
        group.children.as_deref(),
        Some(&["outside".to_owned()][..]),
        "оборачиваемая нода — единственный явный ребёнок"
    );
    canvas.nodes.push(group);
    let gi = canvas.nodes.len() - 1;
    // Нода, случайно лежащая поверх новой группы, ребёнком НЕ становится:
    // child-1 (центр внутри rect новой группы) не в списке
    assert_eq!(group_children(&canvas, gi), vec![3]);
}

/// FR-012: жест втягивания — вставка, авторасширение, вынос; undo одного
/// шага возвращает membership + rect группы.
#[test]
fn test_insert_expand_drag_out() {
    let (mut canvas, mut spatial) = group_scene();
    // Легаси-группа материализуется при первой вставке: дети = in + edge
    group_add_children(&mut canvas, 0, &["outside".to_owned()]);
    assert_eq!(group_children(&canvas, 0), vec![1, 2, 3], "все трое дети");

    // Авторасширение: rect = bbox(дети) + GROUP_PADDING по всем сторонам.
    // Дети: in (50..150 × 50..130), edge (200..320 × 150..250), out (600..700 × 0..100)
    assert!(group_expand_to_children(&mut canvas, 0, GROUP_PADDING));
    let g = &canvas.nodes[0];
    assert_eq!(
        (g.x, g.y, g.width, g.height),
        (
            50.0 - GROUP_PADDING,
            0.0 - GROUP_PADDING,
            700.0 - 50.0 + GROUP_PADDING * 2.0,
            250.0 - 0.0 + GROUP_PADDING * 2.0
        )
    );
    spatial.update(0, &canvas.nodes[0]);

    // Вынос: outside покидает группу — ребёнок больше не едет с ней
    assert!(group_remove_child(&mut canvas, 0, "outside"));
    assert_eq!(group_children(&canvas, 0), vec![1, 2]);
}

/// FR-012: мягкое раздвигание — план выталкивания детерминирован и пуст
/// для нод без пересечения.
#[test]
fn test_push_out_plan_deterministic() {
    let plan = plan_push_out(
        [0.0, 0.0, 400.0, 300.0],
        &[(1, [390.0, 10.0, 50.0, 50.0]), (2, [900.0, 900.0, 50.0, 50.0])],
    );
    assert_eq!(plan, vec![(1, [10.0, 0.0])], "только пересекающийся сосед");
}

// --- Ctrl+G: группировка мультивыделения ---

use canvas_app::ui::{plan_group_around_nodes, HOTKEYS};

/// Ctrl+G: мультивыделение (child-1 + outside) группируется одной группой —
/// bbox набора + GROUP_PADDING, дети — ЯВНЫЙ список id выделенных
/// (FR-012); хоткей задокументирован в панели F1 (HOTKEYS).
#[test]
fn test_group_selection_hotkey_ctrl_g() {
    // Хоткей виден в оверлее F1
    assert!(
        HOTKEYS.iter().any(|(key, _)| *key == "Ctrl+G"),
        "Ctrl+G в панели хоткеев"
    );

    let (mut canvas, mut spatial) = group_scene();
    // Мультивыделение CR-001: child-1 (индекс 1) и outside (индекс 3);
    // группа g (индекс 0) — вне выделения и в группу не попадает
    let selection = [1usize, 3];
    let group = plan_group_around_nodes(&canvas, &selection, GROUP_PADDING).expect("ноды есть");
    assert_eq!(group.kind(), NodeKind::Group);
    assert_eq!(group.id, "group-1");
    assert_eq!(group.label.as_deref(), Some("Группа"));
    // bbox набора: child-1 (50..150 × 50..130) ∪ outside (600..700 × 0..100)
    // = [50,0..700,130] + padding 40 по всем сторонам
    assert_eq!(
        (group.x, group.y, group.width, group.height),
        (
            50.0 - GROUP_PADDING,
            0.0 - GROUP_PADDING,
            700.0 - 50.0 + GROUP_PADDING * 2.0,
            130.0 - 0.0 + GROUP_PADDING * 2.0
        )
    );
    // FR-012: дети — ЯВНЫЙ список id выделенных, порядок = порядок индексов
    assert_eq!(
        group.children.as_deref(),
        Some(&["child-1".to_owned(), "outside".to_owned()][..])
    );

    // Вставка (паттерн App::insert_group): модель + spatial
    canvas.nodes.push(group);
    let gi = canvas.nodes.len() - 1;
    spatial.insert(gi, &canvas.nodes[gi]);
    // Случайно попавшие внутрь rect ноды не в явном списке (FR-012):
    // геометрически внутри и child-2 (центр внутри), но список — только выделенные
    let explicit = canvas.nodes[gi].children.clone().expect("список есть");
    assert_eq!(explicit, vec!["child-1".to_owned(), "outside".to_owned()]);
}
