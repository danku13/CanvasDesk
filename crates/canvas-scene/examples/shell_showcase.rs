//! FR-075 витрина шелла карточек: генерирует `.canvas` с вариантами
//! наполнения/вида карточки — для скриншота wasm-сборки (`?canvas=` в OPFS,
//! рецепт docs/WASM-TESTING.md).
//!
//! Варианты (10 видов): 6 шаблонных нод разных категорий/иконок/числа строк
//! (чип категории по цвету манифеста, иконка роли, таблица тела с зеброй,
//! полоса «ИТОГ», бирюзовые построчные порты), пара с перекрытием (порты
//! фоновой ноды не просвечивают сквозь карточку переднего плана — W1),
//! РАСЧЁТ (построчные формулы с единицами и группировкой разрядов),
//! ЗАМЕТКА, ФАЙЛ и сломанная формула (ошибка в футере + битая рамка).
//!
//! Запуск: `cargo run -p canvas-scene --example shell_showcase -- <путь>`.

use canvas_core::flow::FlowKind;
use canvas_core::templates::{self, TemplateManifest};
use canvas_core::{Canvas, Edge, Node};
use canvas_scene::measure::fit_template_node_height;
use std::str::FromStr;

/// Шаблонная нода с ЯВНОЙ высотой — целью точного шейпера (идемпотентно:
/// рефит приложения высоту не меняет). Значения — из диагностики ниже.
fn tpl(
    registry: &templates::TemplateRegistry,
    id: &str,
    node_id: &str,
    x: f32,
    y: f32,
    height: f32,
) -> Node {
    let manifest: &TemplateManifest = registry.find(id).expect("встроенный шаблон");
    let mut node = templates::instantiate_with_language(
        manifest,
        &std::collections::BTreeMap::new(),
        node_id.to_owned(),
        x,
        y,
        canvas_core::Language::Ru,
    )
    .expect("дефолты манифеста в границах");
    fit_template_node_height(&mut node);
    node.height = height;
    node
}

/// Value-ребро «именованный выход → параметр приёмника».
fn value_edge(id: &str, from: &str, output: &str, to: &str, param: &str) -> Edge {
    let mut edge = Edge {
        id: id.to_owned(),
        from_node: from.to_owned(),
        from_side: None,
        to_node: to.to_owned(),
        to_side: None,
        label: None,
        color: None,
        style: None,
        thickness: None,
        from_line: None,
        from_output: Some(output.to_owned()),
        to_param: Some(param.to_owned()),
        extra: Default::default(),
    };
    edge.set_flow_kind(FlowKind::Value);
    edge
}

fn main() {
    // Точный шейпер (зеркало canvas-app::support::measured_result_reserve_height):
    // пример обязан считать высоты ТАК ЖЕ, как приложение, — иначе файл
    // витрины не идемпотентен (рефит приложения сдвигает ряды сетки).
    canvas_scene::measure::install_measured_reserve(
        |text,
         node_width,
         formula_lines,
         desc,
         desc_expanded,
         footer_reserve,
         sigma_name,
         auto_rows| {
            use canvas_scene::measure::{BODY_PADDING, BODY_TOP_GAP, HEADER_HEIGHT};
            let body_width = (node_width - BODY_PADDING * 2.0).max(BODY_PADDING);
            let body = canvas_render::text::measure_body_height(
                text,
                body_width,
                formula_lines,
                desc,
                desc_expanded,
                sigma_name,
            );
            let auto_label = if auto_rows > 0 {
                canvas_scene::measure::ZONE_LABEL_LINE_HEIGHT
            } else {
                0.0
            };
            let footer = if footer_reserve {
                canvas_core::tokens::CARD_RESULT_STRIP_H
            } else {
                0.0
            };
            HEADER_HEIGHT + BODY_TOP_GAP + body + auto_label + BODY_PADDING + footer
        },
    );
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/showcase.canvas".to_owned());
    let registry = templates::TemplateRegistry::builtin();

    // Сетка 10 карточек под ТОЧНЫЕ высоты шейпера: 3 ряда, без пересечений,
    // в кадре 1280×800 при zoom 1 с учётом хрома (DOM-тулбар сверху-справа,
    // пилюля what-if снизу-центра world x −80..80, y 355..395).
    // Ряд 1: три шаблонных ноды (разные категории/цвета/иконки).
    let cdn = tpl(
        &registry,
        "com.canvasdesk.cdn",
        "tpl-cdn",
        -620.0,
        -364.0,
        220.0,
    );
    let gateway = tpl(
        &registry,
        "com.canvasdesk.api-gateway",
        "tpl-gateway",
        -280.0,
        -364.0,
        268.0,
    );
    let kafka = tpl(
        &registry,
        "com.canvasdesk.queue-kafka",
        "tpl-kafka",
        60.0,
        -364.0,
        294.0,
    );

    // Ряд 2: A/B-тест, РАСЧЁТ (формулы с единицами и разрядами), файл, заметка.
    // Лево ряда — с зазором от flyout свёрнутой палитры (world x < −440).
    let abtest = tpl(
        &registry,
        "com.canvasdesk.ab-test",
        "tpl-ab",
        -430.0,
        -30.0,
        266.0,
    );

    let mut calc = Node::text(
        "note-calc",
        "Пиковый трафик\ndau = 120000\nsess = 3.4\nreq = 8 req\n\
         avg_rps = dau × sess × req / 86400 sec\npeak_rps = avg_rps × 3.2",
        -30.0,
        -44.0,
    );
    calc.width = 320.0;
    calc.height = 204.0;
    calc.color = Some("2".to_owned());

    let file = Node::file("file-spec", "docs/fr-075.md", 300.0, -30.0, 170.0, 100.0);

    let mut note = Node::text(
        "note-small",
        "Заметка\nчип, зоны,\nзебра, порты.",
        490.0,
        -30.0,
    );
    note.width = 130.0;
    note.height = 108.0;
    note.color = Some("4".to_owned());

    // Ряд 3: сломанная формула, пара с перекрытием (W1: порты фоновой ноды
    // не рисуются поверх карточки переднего плана; lb — фон, http — план).
    let mut broken = Node::text("note-broken", "Итог недоступен", -620.0, 250.0);
    broken.width = 300.0;
    broken.height = 120.0;
    broken.set_expr(Some("итог = 1 + неизвестное".to_string()));

    let lb = tpl(&registry, "com.canvasdesk.lb", "tpl-lb", 76.0, 178.0, 220.0);
    let http = tpl(
        &registry,
        "com.canvasdesk.http-endpoint",
        "tpl-http",
        300.0,
        150.0,
        246.0,
    );

    let canvas = Canvas {
        // Порядок = z: перекрывающая пара — lb (фон), http (передний план).
        nodes: vec![
            cdn, gateway, kafka, abtest, calc, file, note, broken, lb, http,
        ],
        edges: vec![
            value_edge("edge-1", "tpl-cdn", "origin_rps", "tpl-gateway", "rps"),
            value_edge(
                "edge-2",
                "tpl-gateway",
                "upstream_rps",
                "tpl-kafka",
                "produce_rate",
            ),
        ],
        extra: Default::default(),
    };

    let json = canvas.to_json().expect("сериализация витрины");
    if let Some(parent) = std::path::Path::new(&out).parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&out, &json).expect("запись файла витрины");
    for node in &canvas.nodes {
        println!(
            "{:>12}  x={:>7.1}  y={:>7.1}  w={:>5.1}  h={:>5.1}",
            node.id, node.x, node.y, node.width, node.height
        );
    }
    println!("записано: {out}");

    // Диагностика загрузки: тот же файл → SceneState → recompute_flow →
    // высоты после growth-only резерва/рефита; затем — снимок описаний
    // манифестов (как App::sync_template_descs) и refit_after_template_descs
    // (фикс FR-075: высоты должны догнать зону описания).
    let loaded = canvas_core::Canvas::from_str(&json).expect("обратный разбор витрины");
    let mut scene = canvas_scene::SceneState::new(loaded, std::path::PathBuf::from(&out));
    scene.recompute_flow();
    println!("--- высоты после recompute_flow (без описаний) ---");
    for (index, node) in scene.canvas.nodes.iter().enumerate() {
        println!(
            "{:>12}  h={:>5.1}  (was {:>5.1})",
            node.id, node.height, canvas.nodes[index].height
        );
    }
    scene.template_descs = std::collections::HashMap::from_iter(registry.list().iter().map(|m| {
        (
            m.id.clone(),
            m.display_description(canvas_core::Language::Ru).to_owned(),
        )
    }));
    scene.refit_after_template_descs();
    println!("--- высоты после refit_after_template_descs (с описаниями) ---");
    for node in scene.canvas.nodes.iter() {
        println!(
            "{:>12}  h={:>5.1}  y+h = {:>6.1}",
            node.id,
            node.height,
            node.y + node.height
        );
    }
}
