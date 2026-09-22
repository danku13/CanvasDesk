//! Регресс-тесты CJM-инвариантов схем PRD-0008 (аудит 2026-09-22: «что
//! проходит формальный чек, но ломает CJM»). Проверяют то, что не видно
//! инвариантам контента в `scheme_apply`: финальную геометрию после
//! авто-роста высот (`ensure_result_reserve` с реальным шейпингом —
//! как ставит приложение: наложения нод, вместимость групп) и живые
//! пользовательские пути D5/D6 — правка входного числа и сценарий
//! what-if с подменой строки (как делает `begin_whatif_override`).
//! Запуск: cargo test -p canvas-app scheme_cjm_tests

#![cfg(test)]

use canvas_core::schemes::SchemeRegistry;
use canvas_scene::measure::install_measured_reserve;
use canvas_scene::scheme_apply::instantiate_scheme;
use canvas_scene::SceneState;
use std::path::PathBuf;

fn rect(n: &canvas_core::Node) -> [f32; 4] {
    [n.x, n.y, n.width, n.height]
}

fn overlaps(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

fn contains(outer: [f32; 4], inner: [f32; 4], pad: f32) -> bool {
    inner[0] >= outer[0] + pad
        && inner[1] >= outer[1] + pad
        && inner[0] + inner[2] <= outer[0] + outer[2] - pad
        && inner[1] + inner[3] <= outer[1] + outer[3] - pad
}

#[test]
fn geometry_clean_after_autogrow() {
    install_measured_reserve(crate::app::measured_result_reserve_height);

    for manifest in SchemeRegistry::embedded().list() {
        let canvas = canvas_core::Canvas::default();
        let instance = instantiate_scheme(manifest, &canvas, [0.0, 0.0]).expect("инстанс");
        let canvas = canvas_core::Canvas {
            nodes: instance.nodes,
            edges: instance.edges,
            ..canvas_core::Canvas::default()
        };
        let mut scene = SceneState::new(canvas.clone(), PathBuf::from("audit.canvas"));
        scene.recompute_flow();
        // После recomputeFlow ноды могли вырасти; забираем финальные размеры.
        let final_nodes: Vec<canvas_core::Node> = scene.canvas.nodes.clone();
        let mut issues: Vec<String> = Vec::new();

        // 1. Авто-рост допустим (grow-only), но наложения после него — нет.
        let texts: Vec<&canvas_core::Node> = final_nodes
            .iter()
            .filter(|n| n.kind() == canvas_core::NodeKind::Text)
            .collect();
        for i in 0..texts.len() {
            for j in i + 1..texts.len() {
                if overlaps(rect(texts[i]), rect(texts[j])) {
                    issues.push(format!(
                        "OVERLAP text/text: {} × {}",
                        texts[i].id, texts[j].id
                    ));
                }
            }
        }

        // 2. Вместимость групп: ребёнок — внутри своей группы (авто-рост
        // не должен выталкивать ноду за контейнер).
        let groups: Vec<&canvas_core::Node> = final_nodes
            .iter()
            .filter(|n| n.kind() == canvas_core::NodeKind::Group)
            .collect();
        for g in &groups {
            let children: Option<&Vec<String>> = g.children.as_ref();
            for n in &texts {
                let own = children.is_some_and(|c| c.contains(&n.id));
                if own && !contains(rect(g), rect(n), 4.0) {
                    issues.push(format!("GROUP-ESCAPE: {} из {}", n.id, g.id));
                }
                if !own && overlaps(rect(g), rect(n)) {
                    issues.push(format!("OVERLAP node/foreign-group: {} × {}", n.id, g.id));
                }
            }
        }

        assert!(
            issues.is_empty(),
            "{}: геометрия после авто-роста:\n  {}",
            manifest.id,
            issues.join("\n  ")
        );
    }
}

/// D6-путь intro-whatif «как в приложении»: сценарий → подмена строки
/// маркетинга (индекс 3, как в begin_whatif_override) → дельты → база
/// не мутирована → возврат на «Базу» возвращает исходные значения.
#[test]
fn whatif_journey_intro_whatif() {
    install_measured_reserve(crate::app::measured_result_reserve_height);
    let manifest = SchemeRegistry::embedded()
        .get("com.canvasdesk.scheme.intro-whatif")
        .unwrap();
    let instance =
        instantiate_scheme(manifest, &canvas_core::Canvas::default(), [0.0, 0.0]).unwrap();
    let canvas = canvas_core::Canvas {
        nodes: instance.nodes,
        edges: instance.edges,
        ..canvas_core::Canvas::default()
    };
    let node_id = |scene: &SceneState, manifest_id: &str| -> String {
        let pos = manifest
            .content
            .nodes
            .iter()
            .position(|n| n.id == manifest_id)
            .unwrap();
        scene.canvas.nodes[pos].id.clone()
    };
    let value_of = |scene: &SceneState, id: &str| -> f64 {
        scene
            .expr_results
            .get(id)
            .and_then(|o| match o {
                canvas_core::expr::ExprOutcome::Ok(v) => Some(v.num),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{id}: нет Ok-итога"))
    };

    let mut scene = SceneState::new(canvas.clone(), PathBuf::from("audit.canvas"));
    scene.recompute_flow();
    let (total_id, annual_id, costs_id) = (
        node_id(&scene, "total"),
        node_id(&scene, "annual"),
        node_id(&scene, "costs"),
    );
    assert_eq!(value_of(&scene, &total_id), 1500.0, "база: итог месяца");
    assert_eq!(value_of(&scene, &annual_id), 18000.0, "база: годовой");

    // Шаг приложения: войти в режим (enter_whatif_mode) и создать сценарий.
    scene.whatif_active = true;
    let index = scene.whatif_create_scenario("Рост маркетинга").unwrap();
    scene.whatif_activate(Some(index));
    // Подмена «поднимите маркетинг до полутора тысяч»: строка 3 текста
    // costs («marketing = 1200 $») — как делает begin_whatif_override.
    scene.scenarios[index]
        .line_exprs
        .insert((costs_id.clone(), 3), "marketing = 1500 $".into());
    scene.recompute_flow();
    assert_eq!(
        value_of(&scene, &total_id),
        1800.0,
        "сценарий: итог = 1500+300"
    );
    assert_eq!(
        value_of(&scene, &annual_id),
        21600.0,
        "сценарий: годовой = 1800×12"
    );
    // База не мутирована (инвариант 2: файл не меняется).
    let costs_node = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.id == costs_id)
        .unwrap();
    assert!(
        costs_node
            .text
            .as_deref()
            .unwrap()
            .contains("marketing = 1200 $"),
        "persisted-текст не мутирован"
    );

    // Возврат на «Базу» возвращает исходные значения.
    scene.whatif_activate(None);
    assert_eq!(value_of(&scene, &total_id), 1500.0);
    assert_eq!(value_of(&scene, &annual_id), 18000.0);
    println!("D6 intro-whatif: 1500→1800, 18000→21600, база цела, возврат ок");
}

/// D5-путь capacity-service: правка спроса в базе (пользователь «меняет
/// число») — значения едут, перегрузка даёт читаемую диагностику, паник нет.
#[test]
fn edit_journey_capacity_service() {
    install_measured_reserve(crate::app::measured_result_reserve_height);
    let manifest = SchemeRegistry::embedded()
        .get("com.canvasdesk.scheme.capacity-service")
        .unwrap();
    let instance =
        instantiate_scheme(manifest, &canvas_core::Canvas::default(), [0.0, 0.0]).unwrap();
    let canvas = canvas_core::Canvas {
        nodes: instance.nodes,
        edges: instance.edges,
        ..canvas_core::Canvas::default()
    };
    let mut scene = SceneState::new(canvas, PathBuf::from("audit.canvas"));
    scene.recompute_flow();

    // Пользователь правит «requests = 5000 req» → 7000 (строка 3 demand).
    let demand = scene
        .canvas
        .nodes
        .iter_mut()
        .find(|n| {
            n.text
                .as_deref()
                .is_some_and(|t| t.contains("requests = 5000"))
        })
        .expect("нода спроса");
    let text = demand.text.clone().unwrap();
    demand.text = Some(text.replace("requests = 5000", "requests = 7000"));
    scene.recompute_flow();

    let util_id = scene
        .canvas
        .nodes
        .iter()
        .find(|n| {
            n.text
                .as_deref()
                .is_some_and(|t| t.contains("util = utilization"))
        })
        .map(|n| n.id.clone())
        .unwrap();
    let wait_id = scene
        .canvas
        .nodes
        .iter()
        .find(|n| n.text.as_deref().is_some_and(|t| t.contains("wait = mm1")))
        .map(|n| n.id.clone())
        .unwrap();
    // util = 7000/30/200 ≈ 1.17 — Percent, валиден; wait — перегрузка
    // (7000/30 = 233 > 200): ОШИБКА с человекочитаемой диагностикой.
    match scene.expr_results.get(&wait_id) {
        Some(canvas_core::expr::ExprOutcome::Err(e)) => {
            let msg = e.to_string();
            assert!(msg.contains("перегрузка"), "диагностика: {msg}");
            println!("D5 capacity: wait → «{msg}»");
        }
        other => panic!("ожидалась перегрузка wait, получено {other:?}"),
    }
    match scene.expr_results.get(&util_id) {
        Some(canvas_core::expr::ExprOutcome::Ok(v)) => {
            println!("D5 capacity: util = {:.3}", v.num);
        }
        other => panic!("util сломался: {other:?}"),
    }
}
