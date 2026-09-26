//! Витрина FR-071 v2 (grid-раскладка, Task 8): инстансы built-in схем
//! → файлы `.canvas` для wasm-скриншотов. Каждая схема — отдельный файл
//! (`?canvas=<имя>`), bbox центрирован на origin (контракт инстансера).
//!
//! Запуск: `cargo run -p canvas-scene --example layout_showcase -- <каталог>`.
use canvas_scene::scheme_apply::instantiate_scheme_with_language;
use std::str::FromStr;

fn main() {
    // Точный шейпер — как в lod_probe/shell_showcase (идемпотентность высот).
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
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target".to_owned());
    let registry = canvas_core::schemes::SchemeRegistry::embedded();
    let empty = canvas_core::Canvas::default();
    for (id, file) in [
        ("com.canvasdesk.scheme.project-budget", "grid_budget.canvas"),
        ("com.canvasdesk.scheme.ab-testing", "grid_abtest.canvas"),
    ] {
        let manifest = registry.get(id).unwrap_or_else(|| panic!("{id} встроен"));
        let instance = instantiate_scheme_with_language(
            manifest,
            &empty,
            [0.0, 0.0],
            canvas_core::Language::Ru,
        )
        .unwrap_or_else(|e| panic!("{id}: {e:?}"));
        let mut canvas = canvas_core::Canvas::default();
        canvas.nodes = instance.nodes.clone();
        canvas.edges = instance.edges.clone();
        let json = canvas.to_json().expect("сериализация");
        let path = format!("{dir}/{file}");
        std::fs::write(&path, &json).expect("запись файла");
        // Диагностика: bbox + сетка (колонки/ряды должны быть кратны шагу).
        let back = canvas_core::Canvas::from_str(&json).expect("обратный разбор");
        let max_w = back
            .nodes
            .iter()
            .filter(|n| n.kind() != canvas_core::NodeKind::Group)
            .map(|n| n.width)
            .fold(0.0f32, f32::max);
        let max_h = back
            .nodes
            .iter()
            .filter(|n| n.kind() != canvas_core::NodeKind::Group)
            .map(|n| n.height)
            .fold(0.0f32, f32::max);
        println!(
            "{id}: bbox={:?} ({} нод, {} рёбер); шаг сетки {}×{}; пересечений: {}",
            instance.bbox,
            back.nodes.len(),
            back.edges.len(),
            max_w + canvas_core::scheme_layout::LAYER_GAP,
            max_h + canvas_core::scheme_layout::ROW_GAP,
            canvas_core::scheme_layout::count_edge_node_crossings(&back),
        );
    }
}
