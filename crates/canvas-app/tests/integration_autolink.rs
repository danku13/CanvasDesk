//! Интеграционный тест PRD-0007 X4 (F-7, AC-5.1–AC-5.5): автосвязь по
//! именам на уровне модели.
//!
//! Сценарий: две текстовые ноды-«таблицы» (A — присваивания, B — ссылки
//! `$имя`) → детектор [`canvas_core::find_proposals`] находит точные
//! совпадения → приложение (паттерн `create_autolink_edges`) создаёт
//! рёбра адресованного проливания ОДНИМ undo-батом → живой пересчёт
//! закрывает потребности приёмника. Инварианты:
//! 1. детектор предлагает только корректные пары (без циклов/дубликатов —
//!    AC-5.4, юнит-тесты ядра; здесь — сквозной прогон);
//! 2. после создания пачки повторный скан пуст (AC-5.4 — дубликаты не
//!    предлагаются, потребности закрыты);
//! 3. undo-бат: `set_undo_tag("autolink_batch")` + один снапшот «до» —
//!    откат одним шагом возвращает модель (AC-5.3), повторный скан снова
//!    находит те же предложения (AC-5.5 — перепроверка);
//! 4. рёбра пачки — value-рёбра с адресацией `fromOutput`/`toParam`
//!    (FR-029) — значения переносятся (итог приёмника считается).

use std::path::PathBuf;

use canvas_app::{Canvas, Edge, Node};
use canvas_core::{find_proposals, FlowKind};
use canvas_scene::SceneState;

/// Заметка-calc: текст — Numi-лист.
fn calc_node(canvas: &mut Canvas, id: &str, text: &str, x: f32) {
    canvas.nodes.push(Node::text(id, id, x, 0.0));
    canvas.nodes.last_mut().expect("нода").text = Some(text.to_owned());
}

/// Ландшафт: A («Риск NPL»: npl_annual, recovery_rate) и B («Платёжная
/// сетка»: ссылки $npl_annual/$recovery_rate) — 2 предложения.
fn two_tables_canvas() -> Canvas {
    let mut canvas = Canvas::default();
    calc_node(
        &mut canvas,
        "A",
        "Риск NPL\nnpl_annual = 12 %\nrecovery_rate = 70 %",
        0.0,
    );
    calc_node(
        &mut canvas,
        "B",
        "Платёжная сетка\ndefault_m = $npl_annual × 2\nrecovery = $recovery_rate × 100",
        300.0,
    );
    canvas
}

/// Создать рёбра пачки (зеркало `create_autolink_edges` из app.rs):
/// один undo-бат с тегом, рёбра — fromOutput=toParam=имя.
fn create_batch(scene: &mut SceneState, accepted: &[canvas_core::AutolinkProposal]) {
    let snapshot = scene.canvas.clone();
    scene.set_undo_tag("autolink_batch");
    scene.push_undo(snapshot);
    for proposal in accepted {
        let id = scene.canvas.next_edge_id();
        let mut edge = Edge::new(
            id,
            proposal.from_node.as_str(),
            None,
            proposal.to_node.as_str(),
            None,
        );
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = Some(proposal.param.clone());
        edge.to_param = Some(proposal.param.clone());
        scene.canvas.add_edge(edge);
    }
    scene.recompute_flow();
}

/// AC-5.1 + AC-5.3 + AC-5.5: пачка создаётся одним undo-шагом,
/// откат возвращает модель, перепроверка снова находит предложения.
#[test]
fn autolink_batch_create_and_undo() {
    let mut scene = SceneState::new(two_tables_canvas(), PathBuf::from("target/tmp/x4.canvas"));
    let rev_before = scene.revision;

    // Детектор: обе переменные A предлагаются для B
    let proposals = find_proposals(&scene.canvas);
    assert_eq!(proposals.len(), 2, "npl_annual + recovery_rate");
    assert!(proposals
        .iter()
        .all(|p| p.from_node == "A" && p.to_node == "B"));
    assert!(proposals.iter().all(|p| p.percent == 100));

    // Создание пачки: один push_undo с тегом
    create_batch(&mut scene, &proposals);
    assert_eq!(scene.canvas.edges.len(), 2);
    assert_eq!(scene.undo_stack.len(), 1, "один снапшот «до» на пачку");
    assert_eq!(scene.peek_undo_tag(), Some("autolink_batch"));
    assert!(scene.revision > rev_before, "живой пересчёт (ревизия)");

    // Повторный скан: потребности закрыты — предложений нет (AC-5.4)
    assert!(
        find_proposals(&scene.canvas).is_empty(),
        "после создания пачки предложений нет"
    );

    // Значения переносятся: итог B считается без ошибок (обе ссылки живы)
    let b_out = scene.flow_active.outputs.get("B").expect("итог B");
    assert!(
        b_out.is_ok(),
        "ссылки $npl_annual/$recovery_rate закрыты: {b_out:?}"
    );

    // Откат пачки — ОДИН undo-шаг (AC-5.3): модель вернулась
    let canvas_after_create = scene.canvas.clone();
    let before = scene.take_undo().expect("undo-снапшот пачки");
    scene.canvas = before;
    scene.recompute_flow();
    assert!(scene.canvas.edges.is_empty(), "рёбра пачки откачены");
    assert_ne!(scene.canvas, canvas_after_create);

    // Перепроверка (AC-5.5): те же предложения вернулись
    let proposals_again = find_proposals(&scene.canvas);
    assert_eq!(proposals_again.len(), 2, "перепроверка после отката");
    let mut names: Vec<&str> = proposals_again.iter().map(|p| p.param.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["npl_annual", "recovery_rate"]);
}

/// AC-5.4: цикл и занятый параметр не предлагаются на сквозном прогоне.
#[test]
fn autolink_filters_on_live_model() {
    let mut canvas = two_tables_canvas();
    // Цикл: A→B уже питается от B (позиционный), B ссылается $npl_annual:
    // реверсивное предложение A→B не должно создаваться при обратной связи
    let mut feed = Edge::new("e-feed", "A", None, "B", None);
    feed.set_flow_kind(FlowKind::Value);
    canvas.add_edge(feed.clone());
    // Позиционное ребро A→B НЕ глушит проливание (другая адресация):
    // предложения остаются, но после проливания тем же источником — нет
    let proposals = find_proposals(&canvas);
    assert_eq!(proposals.len(), 2, "позиционное ребро — не дубликат спилла");
    feed.to_param = Some("npl_annual".to_owned());
    feed.from_output = Some("npl_annual".to_owned());
    canvas.edges[0] = feed;
    let proposals = find_proposals(&canvas);
    assert_eq!(proposals.len(), 1, "npl_annual занят проливанием");
    assert_eq!(proposals[0].param, "recovery_rate");
}

/// Round-trip (G6): рёбра автосвязи сериализуются в `.canvas` без потерь —
/// адресация `fromOutput`/`toParam` переживает сохранение/загрузку.
#[test]
fn autolink_edges_round_trip() {
    let mut canvas = two_tables_canvas();
    let proposals = find_proposals(&canvas);
    assert_eq!(proposals.len(), 2);
    for proposal in &proposals {
        let id = canvas.next_edge_id();
        let mut edge = Edge::new(id, "A", None, "B", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = Some(proposal.param.clone());
        edge.to_param = Some(proposal.param.clone());
        canvas.add_edge(edge);
    }
    let json = serde_json::to_string(&canvas).expect("сериализация");
    let back: Canvas = serde_json::from_str(&json).expect("разбор");
    assert_eq!(back, canvas, "round-trip без потерь");
    assert!(
        find_proposals(&back).is_empty(),
        "после загрузки предложений нет — связи живы"
    );
}
