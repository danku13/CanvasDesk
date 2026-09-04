//! T4: hit-test нод и сохранение с бэкапом (.bak, SPEC §9).

use canvas_core::{Canvas, Node};

fn two_overlapping_nodes() -> Canvas {
    let mut canvas = Canvas::default();
    canvas
        .nodes
        .push(Node::file("a", "C:/a.png", 0.0, 0.0, 100.0, 100.0));
    canvas
        .nodes
        .push(Node::file("b", "C:/b.png", 50.0, 50.0, 100.0, 100.0));
    canvas
}

/// Пустой канвас и точка вне нод — промах.
#[test]
fn hit_test_miss() {
    assert_eq!(Canvas::default().hit_test([10.0, 10.0]), None);
    let canvas = two_overlapping_nodes();
    assert_eq!(canvas.hit_test([500.0, 500.0]), None);
}

/// Перекрытие: попадает верхняя (поздняя в массиве) нода.
#[test]
fn hit_test_topmost_wins() {
    let canvas = two_overlapping_nodes();
    assert_eq!(canvas.hit_test([75.0, 75.0]), Some(1)); // пересечение — верхняя "b"
    assert_eq!(canvas.hit_test([10.0, 10.0]), Some(0)); // только "a"
}

/// Граница ноды включительно.
#[test]
fn hit_test_edges_inclusive() {
    let canvas = two_overlapping_nodes();
    assert_eq!(canvas.hit_test([0.0, 0.0]), Some(0));
    assert_eq!(canvas.hit_test([100.0, 100.0]), Some(1));
}

/// Первый сейв — без .bak; второй — .bak содержит предыдущую версию.
#[test]
fn save_with_backup_creates_bak() {
    let dir = std::env::temp_dir().join(format!("canvasdesk-t4-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tempdir");
    let path = dir.join("scene.canvas");
    let bak = dir.join("scene.canvas.bak");

    let mut canvas = Canvas::default();
    canvas.nodes.push(Node::text("n1", "первая", 0.0, 0.0));
    canvas.save_with_backup(&path).expect("первый сейв");
    assert!(path.exists());
    assert!(!bak.exists(), "при первом сейве .bak не нужен");

    canvas.nodes.push(Node::text("n2", "вторая", 300.0, 0.0));
    canvas.save_with_backup(&path).expect("второй сейв");
    let backup = Canvas::load(&bak).expect("bak парсится");
    assert_eq!(backup.nodes.len(), 1, ".bak — предыдущая версия");
    assert_eq!(backup.nodes[0].id, "n1");

    let current = Canvas::load(&path).expect("текущая версия парсится");
    assert_eq!(current.nodes.len(), 2);

    std::fs::remove_dir_all(&dir).ok();
}
