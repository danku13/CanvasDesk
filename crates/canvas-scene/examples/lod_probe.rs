//! LOD-проба зон описания шаблонных нод (багфикс «строки обрезаются при
//! отдалении»): генерирует `.canvas` с ОДНОЙ шаблонной нодой
//! com.canvasdesk.infra-cost (длинное описание манифеста → зона описания
//! в клампе TABLE_DESC_CLAMP_LINES + экспандер) для скриншотов wasm-стенда
//! на двух зумах: контент зоны обязан быть идентичным на любом зуме.
//!
//! Запуск: `cargo run -p canvas-scene --example lod_probe -- <путь>`.

use canvas_scene::measure::fit_template_node_height;
use std::str::FromStr;

fn main() {
    // Точный шейпер — как в shell_showcase (идемпотентность высот).
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
        .unwrap_or_else(|| "target/lodprobe.canvas".to_owned());
    let registry = canvas_core::templates::TemplateRegistry::builtin();
    let manifest = registry
        .find("com.canvasdesk.infra-cost")
        .expect("встроенный шаблон infra-cost");
    let mut node = canvas_core::templates::instantiate_with_language(
        manifest,
        &std::collections::BTreeMap::new(),
        "tpl-infra".to_owned(),
        -170.0,
        -150.0,
        canvas_core::Language::Ru,
    )
    .expect("дефолты манифеста в границах");
    fit_template_node_height(&mut node);

    let mut canvas = canvas_core::Canvas::default();
    canvas.nodes.push(node);
    let json = canvas.to_json().expect("сериализация пробы");
    std::fs::write(&out, &json).expect("запись файла пробы");

    // Диагностика: зона описания в мере (zoom 1) — ровно кламп + экспандер.
    let desc = manifest
        .display_description(canvas_core::Language::Ru)
        .to_owned();
    let measure = canvas_render::text::measure_body_height(
        canvas.nodes[0].text.as_deref().unwrap_or_default(),
        canvas.nodes[0].width - 24.0,
        &[],
        &desc,
        false,
        "",
    );
    println!(
        "инстанс: x={} y={} w={} h={}; зона описания (кламп, world-px): {measure:.1}; записано: {out}",
        canvas.nodes[0].x,
        canvas.nodes[0].y,
        canvas.nodes[0].width,
        canvas.nodes[0].height
    );

    // Контроль I-2: высота узла вмещает стек (мера == шейпер).
    let loaded = canvas_core::Canvas::from_str(&json).expect("обратный разбор пробы");
    let mut scene = canvas_scene::SceneState::new(loaded, std::path::PathBuf::from(&out));
    scene.template_descs =
        std::collections::HashMap::from_iter(std::iter::once((manifest.id.clone(), desc.clone())));
    scene.refit_after_template_descs();
    println!(
        "после refit_after_template_descs: h={} (было {})",
        scene.canvas.nodes[0].height, canvas.nodes[0].height
    );
}
