//! FR-025 (правка 2): headless smoke-тест построчных точек выхода.
//! Обычная заметка — порт у каждой формульной строки (финальная —
//! `is_final`); шаблонная нода — порты листа параметров ПЛЮС порт футера
//! (`line = None`, узловое значение). Если GPU-адаптер недоступен —
//! тест пропускается.

use std::collections::BTreeMap;

use canvas_core::expr::{self, Env as ExprEnv, ExprLineResults, ExprOutcome, ExprResults, Value};
use canvas_core::templates::{TemplateParam, TemplateRef};
use canvas_core::{Canvas, Node};
use canvas_render::gpu::GpuContext;
use canvas_render::text::{result_footer_y, TextSystem, TitleFrame};
use canvas_render::zorder;
use canvas_render::Camera;

/// FR-025 (правка 2): у шаблонной ноды порты на каждой строке листа
/// параметров + порт футера; у обычной заметки — построчные порты с
/// финальной строкой последней.
#[test]
fn line_ports_template_and_note() {
    let Some(gpu) = pollster::block_on(GpuContext::headless()) else {
        eprintln!("GPU-адаптер недоступен, headless-тест пропущен");
        return;
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mut text = TextSystem::new(&gpu.device, &gpu.queue, format);

    let mut canvas = Canvas::default();

    // Обычная заметка: две формульные строки
    let mut note = Node::text("note-1", "Расчёт", -190.0, -140.0);
    note.width = 380.0;
    note.height = 260.0;
    note.text = Some("1200 + 480\n1680 × 2".to_owned());
    canvas.nodes.push(note);

    // Шаблонная нода: лист параметров + формула (как instantiate FR-018)
    let mut template = Node::text("tpl-1", "Load Balancer", 250.0, -140.0);
    template.width = 300.0;
    template.height = 220.0;
    template.text = Some("rps = 1000\nservers = 2".to_owned());
    let mut params: BTreeMap<String, TemplateParam> = BTreeMap::new();
    params.insert(
        "rps".to_owned(),
        TemplateParam {
            num: 1000.0,
            unit: None,
        },
    );
    params.insert(
        "servers".to_owned(),
        TemplateParam {
            num: 2.0,
            unit: None,
        },
    );
    template.set_template(Some(TemplateRef {
        id: "com.canvasdesk.test".to_owned(),
        version: "1.0.0".to_owned(),
        expr: "$rps × $servers".to_owned(),
        params,
        icon: String::new(),
        color: String::new(),
        name: Some("Load Balancer".to_owned()),
        outputs: Vec::new(),
    }));
    canvas.nodes.push(template);

    // Runtime-кэши как в приложении (recompute_flow): футер шаблона —
    // программный итог формулы; построчные результаты — eval_lines текста
    let mut results: ExprResults = ExprResults::new();
    results.insert("tpl-1".to_owned(), ExprOutcome::Ok(Value::scalar(2000.0)));
    let mut line_results: ExprLineResults = ExprLineResults::new();
    for node in &canvas.nodes {
        let outcomes =
            expr::eval_lines_in(&node.text.clone().unwrap_or_default(), &ExprEnv::empty());
        if outcomes.iter().any(Option::is_some) {
            line_results.insert(node.id.clone(), outcomes);
        }
    }

    let camera = Camera::default();
    let zplan = zorder::plan_z_order(
        &[[-190.0, -140.0, 190.0, 120.0], [250.0, -140.0, 550.0, 80.0]],
        &[true, true],
        &[false, false],
        16,
    );
    text.prepare_titles(
        &gpu.device,
        &gpu.queue,
        &TitleFrame {
            camera: &camera,
            viewport_physical: [900, 400],
            scale_factor: 1.0,
            canvas: &canvas,
            indices: &[0, 1],
            hud: None,
            editing: None,
            editing_buffer: None,
            overlay_texts: &[],
            screen_bands: &[],
            zplan: &zplan,
            edge_labels: &[],
            focus: canvas_render::cards::FocusView::EMPTY,
            widget_title_reveal: &[],
            collapsed_counts: &[],
            expr_results: &results,
            expr_line_results: &line_results,
            editing_line_results: None,
            param_spills: &Default::default(),
            auto_rows: &Default::default(),
            whatif_nodes: &Default::default(),
            analysis_badges: &[],
            stage_texts: &[],
        },
    )
    .expect("prepare текста");

    // --- Обычная заметка: порт у каждой строки, финальная — последняя ---
    let note_ports = text.line_ports(0, &canvas.nodes[0]);
    assert_eq!(note_ports.len(), 2, "порт у каждой формульной строки");
    assert_eq!(note_ports[0].line, Some(0));
    assert_eq!(note_ports[1].line, Some(1));
    assert!(!note_ports[0].is_final, "первая строка — промежуточная");
    assert!(note_ports[1].is_final, "последняя строка — финальная");
    let note_right = canvas.nodes[0].x + canvas.nodes[0].width;
    for port in &note_ports {
        assert!((port.point[0] - note_right).abs() < 1e-3, "правый край");
    }

    // --- Шаблонная нода: строки параметров + футер (line = None) ---
    let tpl_ports = text.line_ports(1, &canvas.nodes[1]);
    assert_eq!(
        tpl_ports.len(),
        3,
        "две строки листа параметров + порт футера"
    );
    assert_eq!(tpl_ports[0].line, Some(0));
    assert_eq!(tpl_ports[1].line, Some(1));
    assert!(!tpl_ports[0].is_final, "строки листа — промежуточные");
    assert!(!tpl_ports[1].is_final, "строки листа — промежуточные");
    assert_eq!(tpl_ports[2].line, None, "футер — узловое значение");
    assert!(tpl_ports[2].is_final, "футер — финальное значение");
    let tpl = &canvas.nodes[1];
    let tpl_right = tpl.x + tpl.width;
    for port in &tpl_ports {
        assert!((port.point[0] - tpl_right).abs() < 1e-3, "правый край");
    }
    // Инвариант вертикали: построчные порты — ряды бейджей, футер — центр
    // футера результата (FR-023)
    assert!(
        (tpl_ports[2].point[1] - result_footer_y(tpl)).abs() < 1e-3,
        "футер-порт — вертикаль футера результата"
    );
    for port in tpl_ports.iter().take(2) {
        assert!((port.point[0] - tpl_right).abs() < 1e-3, "правый край");
        assert!(
            port.point[1] < tpl_ports[2].point[1],
            "строки листа выше футера"
        );
    }
}
