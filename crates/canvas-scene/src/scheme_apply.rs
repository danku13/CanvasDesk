//! FR-049 (PRD-0008 T2): чистый инстансер схем + oracle-тесты стартового
//! набора.
//!
//! [`instantiate_scheme`] — чистая функция над `(манифест, &Canvas, origin)`:
//! ремап id нод/рёбер без коллизий с текущим канвасом (генератор по образцу
//! `SceneState::next_free_id`), сдвиг bounding box к точке вставки,
//! value-рёбра (`flowKind: "value"` → `FlowKind::Value`, входы `$1..$N`).
//! Апплаер (undo-шаг, spatial, `recompute_flow`, zoom-to-fit) — в `App`
//! по образцу `instantiate_template_at` (FR-049 §Changes п.4).
//!
//! Оракулы (G2 PRD-0008): каждая built-in схема инстанцируется в пустой
//! канвас, `recompute_flow` даёт контрольные значения формул. Тесты
//! исполняются нативно и под wasm32-wasip1 (гейт FR-037) — без ФС и сети.

use canvas_core::flow::FlowKind;
use canvas_core::schemes::SchemeManifest;
use canvas_core::{Edge, Node, Side};

/// Инстанцированная схема: ноды/рёбра для вставки + bounding box (до
/// сдвига, в координатах схемы) для zoom-to-fit.
#[derive(Debug, Clone, PartialEq)]
pub struct SchemeInstance {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// `[min_x, min_y, max_x, max_y]` — bbox содержимого схемы в мировых
    /// координатах ПОСЛЕ сдвига к origin.
    pub bbox: [f32; 4],
}

/// Ошибка инстанцирования (контент валиден реестром; ошибка здесь —
/// только программная деградация формата).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SchemeInstantiateError {
    #[error("пакет схемы пуст (нет нод)")]
    Empty,
}

/// Первый свободный id вида `{prefix}-N` по образцу `next_free_id`
/// (`scene.rs`): занятые в канвасе и уже выданные пропускаются.
fn alloc_node_id(
    prefix: &'static str,
    canvas: &canvas_core::Canvas,
    used: &mut std::collections::HashSet<String>,
    counters: &mut std::collections::HashMap<&'static str, u32>,
) -> String {
    let counter = counters.entry(prefix).or_insert(1);
    loop {
        let candidate = format!("{prefix}-{counter}");
        *counter += 1;
        if canvas.node(&candidate).is_none() && !used.contains(&candidate) {
            used.insert(candidate.clone());
            return candidate;
        }
    }
}

/// Инстанцировать схему к точке `origin` (центр bbox → origin).
///
/// Ремап id: `note-N` (text) / `group-N` (group) — свободные в `canvas`;
/// id рёбер — `edge-N` (образец `Canvas::next_edge_id`). Стороны value-
/// рёбер — right→left (чтение слева направо). Дети групп ремапятся по той
/// же карте.
pub fn instantiate_scheme(
    manifest: &SchemeManifest,
    canvas: &canvas_core::Canvas,
    origin: [f32; 2],
) -> Result<SchemeInstance, SchemeInstantiateError> {
    if manifest.content.nodes.is_empty() {
        return Err(SchemeInstantiateError::Empty);
    }

    // Bounding box в координатах схемы.
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for node in &manifest.content.nodes {
        min_x = min_x.min(node.x);
        min_y = min_y.min(node.y);
        max_x = max_x.max(node.x + node.width);
        max_y = max_y.max(node.y + node.height);
    }
    let center_x = (min_x + max_x) / 2.0;
    let center_y = (min_y + max_y) / 2.0;
    let dx = origin[0] - center_x;
    let dy = origin[1] - center_y;

    // Генератор свободных id (образец next_free_id: canvas + уже выданные).
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut counters: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();

    // Ремап id нод (порядок пакета стабилен → карта полна до рёбер).
    let mut id_map: std::collections::HashMap<&str, String> =
        std::collections::HashMap::new();
    for node in &manifest.content.nodes {
        let prefix = if node.node_type == "group" {
            "group"
        } else {
            "note"
        };
        let new_id = alloc_node_id(prefix, canvas, &mut used, &mut counters);
        id_map.insert(node.id.as_str(), new_id);
    }

    let mut nodes = Vec::with_capacity(manifest.content.nodes.len());
    for node in &manifest.content.nodes {
        let new_id = &id_map[node.id.as_str()];
        let x = node.x + dx;
        let y = node.y + dy;
        let mut built = if node.node_type == "group" {
            Node::group(new_id.clone(), x, y, node.width, node.height)
        } else {
            let text = node.text.clone().unwrap_or_default();
            let mut n = Node::text(new_id.clone(), text, x, y);
            n.width = node.width;
            n.height = node.height;
            n
        };
        built.color = node.color.clone();
        if let Some(children) = &node.children {
            let mapped: Vec<String> = children
                .iter()
                .map(|child| id_map.get(child.as_str()).cloned().unwrap_or_default())
                .collect();
            built.children = Some(mapped);
        }
        nodes.push(built);
    }

    // Рёбра: id `edge-N` без коллизий; value-рёбра → FlowKind::Value.
    let mut edge_used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut edge_counter = canvas
        .edges
        .iter()
        .filter_map(|edge| edge.id.strip_prefix("edge-"))
        .filter_map(|suffix| suffix.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let mut edges = Vec::with_capacity(manifest.content.edges.len());
    for edge in &manifest.content.edges {
        let mut candidate;
        loop {
            candidate = format!("edge-{edge_counter}");
            edge_counter += 1;
            if !canvas.edges.iter().any(|e| e.id == candidate)
                && !edge_used.contains(&candidate)
            {
                break;
            }
        }
        edge_used.insert(candidate.clone());
        let from = id_map[edge.from_node.as_str()].clone();
        let to = id_map[edge.to_node.as_str()].clone();
        let mut built = Edge::new(candidate, from, Some(Side::Right), to, Some(Side::Left));
        if edge.flow_kind.as_deref() == Some("value") {
            built.set_flow_kind(FlowKind::Value);
        }
        edges.push(built);
    }

    Ok(SchemeInstance {
        nodes,
        edges,
        bbox: [
            min_x + dx,
            min_y + dy,
            max_x + dx,
            max_y + dy,
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use canvas_core::expr::ExprOutcome;
    use canvas_core::schemes::SchemeRegistry;
    use std::path::PathBuf;

    /// Инстанс схемы в пустой канвас (нативно и под wasip1).
    fn instance_of(id: &str) -> SchemeInstance {
        let manifest = SchemeRegistry::embedded()
            .get(id)
            .unwrap_or_else(|| panic!("схема {id} в реестре"));
        let canvas = canvas_core::Canvas::default();
        instantiate_scheme(manifest, &canvas, [0.0, 0.0]).expect("инстанс пустого канваса")
    }

    /// Значение формульной ноды после recompute_flow (`ExprOutcome::Ok`).
    /// Адресация — по id манифеста через карту ремапа (инстансер выдаёт
    /// свободные note-N, тест не полагается на конкретные значения).
    fn value_of(
        scene: &crate::scene::SceneState,
        map: &std::collections::HashMap<String, String>,
        manifest_id: &str,
    ) -> f64 {
        let node_id = &map[manifest_id];
        match scene.expr_results.get(node_id) {
            Some(ExprOutcome::Ok(value)) => value.num,
            other => panic!("нода {manifest_id}: ожидалось Ok, получено {other:?}"),
        }
    }

    #[test]
    fn instancer_remaps_ids_and_shifts_bbox() {
        let instance = instance_of("com.canvasdesk.scheme.intro-calculations");
        assert_eq!(instance.nodes.len(), 4);
        assert_eq!(instance.edges.len(), 2);
        // Все id свободны и префиксны.
        for node in &instance.nodes {
            assert!(
                node.id.starts_with("note-") || node.id.starts_with("group-"),
                "id {} префиксный",
                node.id
            );
        }
        for edge in &instance.edges {
            assert!(edge.id.starts_with("edge-"));
        }
        // bbox центрирован на origin.
        let cx = (instance.bbox[0] + instance.bbox[2]) / 2.0;
        let cy = (instance.bbox[1] + instance.bbox[3]) / 2.0;
        assert!((cx - 0.0).abs() < 0.01, "центр X = {cx}");
        assert!((cy - 0.0).abs() < 0.01, "центр Y = {cy}");
    }

    #[test]
    fn instancer_avoids_collisions_on_occupied_canvas() {
        let mut canvas = canvas_core::Canvas::default();
        for i in 1..=3 {
            canvas.nodes.push(Node::text(format!("note-{i}"), "занято", 0.0, 0.0));
        }
        canvas.edges.push(Edge::new("edge-1", "note-1", None, "note-2", None));
        let manifest = SchemeRegistry::embedded()
            .get("com.canvasdesk.scheme.intro-calculations")
            .unwrap();
        let instance = instantiate_scheme(manifest, &canvas, [100.0, 100.0])
            .expect("инстанс занятого канваса");
        for node in &instance.nodes {
            assert!(
                canvas.node(&node.id).is_none(),
                "id {} не должен коллидировать",
                node.id
            );
        }
        for edge in &instance.edges {
            assert!(
                !canvas.edges.iter().any(|e| e.id == edge.id),
                "id ребра {} не должен коллидировать",
                edge.id
            );
        }
    }

    #[test]
    fn groups_remap_children() {
        let instance = instance_of("com.canvasdesk.scheme.project-budget");
        let groups: Vec<&Node> = instance
            .nodes
            .iter()
            .filter(|n| n.kind() == canvas_core::NodeKind::Group)
            .collect();
        assert_eq!(groups.len(), 2, "две группы");
        let ids: Vec<&str> = instance.nodes.iter().map(|n| n.id.as_str()).collect();
        for group in groups {
            let children = group.children.as_ref().expect("дети группы");
            assert_eq!(children.len(), 2);
            for child in children {
                assert!(ids.contains(&child.as_str()), "ребёнок {child} ремапнут");
            }
        }
    }

    #[test]
    fn value_edges_carry_flow_kind() {
        let instance = instance_of("com.canvasdesk.scheme.intro-calculations");
        assert!(
            instance
                .edges
                .iter()
                .all(|e| e.flow_kind() == FlowKind::Value),
            "все рёбра схемы — value"
        );
    }

    // --- Оракулы стартового набора (G2 PRD-0008) ---

    fn scene_with(
        id: &str,
    ) -> (
        crate::scene::SceneState,
        std::collections::HashMap<String, String>,
    ) {
        let manifest = SchemeRegistry::embedded().get(id).unwrap();
        let instance = instantiate_scheme(manifest, &canvas_core::Canvas::default(), [0.0, 0.0])
            .expect("инстанс пустого канваса");
        // Порядок instance.nodes совпадает с манифестом → карта ремапа.
        let map: std::collections::HashMap<String, String> = manifest
            .content
            .nodes
            .iter()
            .map(|n| n.id.clone())
            .zip(instance.nodes.iter().map(|n| n.id.clone()))
            .collect();
        let mut canvas = canvas_core::Canvas::default();
        canvas.nodes = instance.nodes;
        canvas.edges = instance.edges;
        let mut scene = crate::scene::SceneState::new(canvas, PathBuf::from("oracle.canvas"));
        scene.recompute_flow();
        (scene, map)
    }

    #[test]
    fn oracle_intro_calculations() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.intro-calculations");
        assert!((value_of(&scene, &map, "load") - 5000.0).abs() < 1e-6);
    }

    #[test]
    fn oracle_intro_whatif() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.intro-whatif");
        assert!((value_of(&scene, &map, "total") - 1500.0).abs() < 1e-6);
    }

    #[test]
    fn oracle_capacity_service() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.capacity-service");
        let rps = value_of(&scene, &map, "rps");
        assert!((rps - 5000.0 / 30.0).abs() < 1e-6, "rps = {rps}");
        let util = value_of(&scene, &map, "util");
        assert!((util - (5000.0 / 30.0) / 200.0).abs() < 1e-6, "util = {util}");
    }

    #[test]
    fn oracle_project_budget() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.project-budget");
        assert!((value_of(&scene, &map, "team-total") - 13500.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "infra-total") - 1400.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "reserve") - 2025.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "total") - 16925.0).abs() < 1e-6);
    }

    #[test]
    fn oracle_unit_economics() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.unit-economics");
        assert!((value_of(&scene, &map, "margin") - 6.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "ltv") - 216.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "ratio") - 1.8).abs() < 1e-6);
    }

    #[test]
    fn oracle_renovation_estimate() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.renovation-estimate");
        assert!((value_of(&scene, &map, "cost-a") - 450.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "cost-b") - 300.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "total") - 1450.0).abs() < 1e-6);
    }

    /// Проза-заметки не порождают значений и ошибок (инвариант тишины прозы).
    #[test]
    fn prose_notes_stay_silent() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.intro-calculations");
        let hint = scene.expr_line_results.get(&map["hint"]);
        // Нода-подсказка: все строки без результата (проза молчит).
        if let Some(lines) = hint {
            assert!(
                lines.iter().all(|line| line.is_none()),
                "проза-нода молчит"
            );
        }
    }

    /// Схема вставляется в канвас с существующими id без ошибок ремапа
    /// и пересчитывается (US-3 AC-3.1).
    #[test]
    fn instantiate_into_occupied_canvas_recomputes() {
        let mut canvas = canvas_core::Canvas::default();
        canvas.nodes.push(Node::text("note-1", "существующая нода", 0.0, 0.0));
        let manifest = SchemeRegistry::embedded()
            .get("com.canvasdesk.scheme.intro-whatif")
            .unwrap();
        let instance = instantiate_scheme(manifest, &canvas, [500.0, 500.0]).unwrap();
        let map: std::collections::HashMap<String, String> = manifest
            .content
            .nodes
            .iter()
            .map(|n| n.id.clone())
            .zip(instance.nodes.iter().map(|n| n.id.clone()))
            .collect();
        canvas.nodes.extend(instance.nodes);
        canvas.edges.extend(instance.edges);
        let mut scene = crate::scene::SceneState::new(canvas, PathBuf::from("oracle.canvas"));
        scene.recompute_flow();
        // Итог схемы жив и не зависит от существующей ноды.
        assert!((value_of(&scene, &map, "total") - 1500.0).abs() < 1e-6);
    }
}
