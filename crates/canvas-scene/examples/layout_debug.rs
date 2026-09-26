//! Диагностика FR-071 grid-раскладки (Task 8): инстанс схемы → позиции нод,
//! колонки/ряды сетки, детали пересечений «ребро × нода» и «ребро × ребро».
//!
//! Запуск: `cargo run --example layout_debug -p canvas-scene -- [scheme-id]`
//! Без аргумента — все built-in схемы. Полезно при правках scheme_layout.rs:
//! 0 пересечений обоих типов — oracle-инварианты `scheme_apply.rs`.
use canvas_core::schemes::SchemeRegistry;
use canvas_scene::scheme_apply::instantiate_scheme_with_language;

fn main() {
    let filter = std::env::args().nth(1);
    let registry = SchemeRegistry::embedded();
    let empty = canvas_core::Canvas::default();
    for scheme in registry.list() {
        if let Some(f) = &filter {
            if !scheme.id.contains(f) {
                continue;
            }
        }
        let instance =
            instantiate_scheme_with_language(scheme, &empty, [0.0, 0.0], canvas_core::Language::Ru)
                .expect("инстанс");
        let canvas = canvas_core::Canvas {
            nodes: instance.nodes.clone(),
            edges: instance.edges.clone(),
            ..Default::default()
        };
        println!("scheme: {}", scheme.id);
        for n in &canvas.nodes {
            println!(
                "  {:<14} x={:>7.1} y={:>7.1} w={:>5.1} h={:>5.1} kind={:?}",
                n.id,
                n.x,
                n.y,
                n.width,
                n.height,
                n.kind()
            );
        }
        // детали пересечений: ребро × нода (тот же критерий, что в oracle)
        let index_of: std::collections::HashMap<&str, usize> = canvas
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let mut segs: Vec<(String, String, [f32; 2], [f32; 2])> = Vec::new();
        for edge in &canvas.edges {
            let (Some(&u), Some(&v)) = (
                index_of.get(edge.from_node.as_str()),
                index_of.get(edge.to_node.as_str()),
            ) else {
                continue;
            };
            let (a, b) = (&canvas.nodes[u], &canvas.nodes[v]);
            let p0 = [a.x + a.width, a.y + a.height / 2.0];
            let p1 = [b.x, b.y + b.height / 2.0];
            segs.push((a.id.clone(), b.id.clone(), p0, p1));
            for (i, n) in canvas.nodes.iter().enumerate() {
                if n.kind() == canvas_core::NodeKind::Group || i == u || i == v {
                    continue;
                }
                let r = [n.x, n.y, n.width, n.height];
                if canvas_core::scheme_layout::debug_seg_intersects(p0, p1, r, 12.0) {
                    println!(
                        "  CROSS-NODE: {} -> {}  ×  {}  (p0={:?}, p1={:?}, rect={:?})",
                        a.id, b.id, n.id, p0, p1, r
                    );
                }
            }
        }
        // детали пересечений: ребро × ребро (v3 — proper-crossing порт→порт)
        for i in 0..segs.len() {
            for j in (i + 1)..segs.len() {
                let (a0, a1) = (segs[i].2, segs[i].3);
                let (b0, b1) = (segs[j].2, segs[j].3);
                if canvas_core::scheme_layout::debug_seg_seg_cross(a0, a1, b0, b1) {
                    println!(
                        "  CROSS-EDGE: {}->{} × {}->{}",
                        segs[i].0, segs[i].1, segs[j].0, segs[j].1
                    );
                }
            }
        }
        println!(
            "  total edge×node crossings: {}",
            canvas_core::scheme_layout::count_edge_node_crossings(&canvas)
        );
        println!(
            "  total edge×edge crossings: {}",
            canvas_core::scheme_layout::count_edge_edge_crossings(&canvas)
        );
    }
}
