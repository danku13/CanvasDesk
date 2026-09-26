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
///
/// FR-071: геометрия строится умной раскладкой (`canvas_core::scheme_layout`)
/// — семантические кластеры, слоистая DAG-раскладка, СЕТКА (v2: x = колонка·
/// CELL_W, y = ряд·CELL_H — ряды выровнены между колонками, зазоры
/// гарантированы), минимизация пересечений рёбер с нодами; координаты
/// пакета остаются только подсказками порядка.
///
/// FR-040 v2: `language` выбирает контент (`content_en` при En и наличии,
/// иначе `content` — RU). Дефолт — без указания языка = `Ru` (обратная
/// совместимость; MCP-путь и тесты без контекста языка — на дефолте).
pub fn instantiate_scheme(
    manifest: &SchemeManifest,
    canvas: &canvas_core::Canvas,
    origin: [f32; 2],
) -> Result<SchemeInstance, SchemeInstantiateError> {
    instantiate_scheme_with_language(manifest, canvas, origin, canvas_core::Language::Ru)
}

/// FR-040 v2: инстанс с явным языком — контент берётся из
/// [`SchemeManifest::display_content`] (En → `content_en`, Ru → `content`).
/// GUI-путь передаёт `settings.language`; MCP-путь остаётся на дефолте.
pub fn instantiate_scheme_with_language(
    manifest: &SchemeManifest,
    canvas: &canvas_core::Canvas,
    origin: [f32; 2],
    language: canvas_core::Language,
) -> Result<SchemeInstance, SchemeInstantiateError> {
    let content = manifest.display_content(language);
    if content.nodes.is_empty() {
        return Err(SchemeInstantiateError::Empty);
    }

    // Генератор свободных id (образец next_free_id: canvas + уже выданные).
    let mut used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut counters: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();

    // Ремап id нод (порядок пакета стабилен → карта полна до рёбер).
    let mut id_map: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    for node in &content.nodes {
        let prefix = if node.node_type == "group" {
            "group"
        } else {
            "note"
        };
        let new_id = alloc_node_id(prefix, canvas, &mut used, &mut counters);
        id_map.insert(node.id.as_str(), new_id);
    }

    let mut nodes = Vec::with_capacity(content.nodes.len());
    for node in &content.nodes {
        let new_id = &id_map[node.id.as_str()];
        // Сырые координаты пакета: раскладка FR-071 ниже переопределит их
        // (остаются семантическими подсказками порядка).
        let x = node.x;
        let y = node.y;
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
        // FR-049 v2: `label` (JSON Canvas) — заголовок группы и
        // фолбэк-заголовок text-ноды.
        built.label = node.label.clone();
        if let Some(children) = &node.children {
            let mapped: Vec<String> = children
                .iter()
                .map(|child| id_map.get(child.as_str()).cloned().unwrap_or_default())
                .collect();
            built.children = Some(mapped);
        }
        nodes.push(built);
    }

    // Рёбра: id `edge-N` без коллизий; value-рёбра → FlowKind::Value;
    // FR-049 v2: адресация (fromLine/fromOutput/toParam) копируется в
    // модель `Edge` — поля уже существуют в формате `.canvas` (SPEC §5.1),
    // вставка не расширяет формат документа.
    let mut edge_used: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut edge_counter = canvas
        .edges
        .iter()
        .filter_map(|edge| edge.id.strip_prefix("edge-"))
        .filter_map(|suffix| suffix.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let mut edges = Vec::with_capacity(content.edges.len());
    for edge in &content.edges {
        let mut candidate;
        loop {
            candidate = format!("edge-{edge_counter}");
            edge_counter += 1;
            if !canvas.edges.iter().any(|e| e.id == candidate) && !edge_used.contains(&candidate) {
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
        built.from_line = edge.from_line;
        built.from_output = edge.from_output.clone();
        built.to_param = edge.to_param.clone();
        edges.push(built);
    }

    // FR-071: умная раскладка — план позиций от семантики графа (кластеры
    // по смыслу, слои DAG, barycenter, минимизация пересечений рёбер с
    // нодами); рамки групп пересчитываются по bbox детей.
    let mut laid = canvas_core::Canvas {
        nodes,
        edges,
        ..canvas_core::Canvas::default()
    };
    let plan = canvas_core::scheme_layout::plan_scheme_layout(&laid);
    for (index, [x, y]) in plan.positions {
        if let Some(node) = laid.nodes.get_mut(index) {
            node.x = x;
            node.y = y;
        }
    }
    for (index, [width, height]) in plan.group_sizes {
        if let Some(node) = laid.nodes.get_mut(index) {
            node.width = width;
            node.height = height;
        }
    }

    // Сдвиг bbox раскладки к origin (контракт zoom-to-fit прежний).
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for node in &laid.nodes {
        min_x = min_x.min(node.x);
        min_y = min_y.min(node.y);
        max_x = max_x.max(node.x + node.width);
        max_y = max_y.max(node.y + node.height);
    }
    let dx = origin[0] - (min_x + max_x) / 2.0;
    let dy = origin[1] - (min_y + max_y) / 2.0;
    for node in &mut laid.nodes {
        node.x += dx;
        node.y += dy;
    }

    Ok(SchemeInstance {
        nodes: laid.nodes,
        edges: laid.edges,
        bbox: [min_x + dx, min_y + dy, max_x + dx, max_y + dy],
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
        assert_eq!(instance.nodes.len(), 6, "6 нод intro-схемы (PRD §7.2)");
        assert_eq!(instance.edges.len(), 4);
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
            canvas
                .nodes
                .push(Node::text(format!("note-{i}"), "занято", 0.0, 0.0));
        }
        canvas
            .edges
            .push(Edge::new("edge-1", "note-1", None, "note-2", None));
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
    fn groups_remap_children_and_labels() {
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
            assert!(
                group.label.as_deref().is_some_and(|l| !l.is_empty()),
                "группа {} имеет заголовок (label)",
                group.id
            );
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

    /// FR-049 v2: адресация рёбер (fromOutput/toParam/fromLine) переживает
    /// инстанцирование — без неё схемы не смогли бы демонстрировать
    /// именованные выходы, проливание и построчные истоки.
    #[test]
    fn addressing_survives_instantiation() {
        let instance = instance_of("com.canvasdesk.scheme.project-budget");
        let bundle = instance
            .edges
            .iter()
            .filter(|e| e.from_output.as_deref() == Some("team"))
            .count();
        assert_eq!(bundle, 2, "подытог team питает и резерв, и итог");
        let spill = instance
            .edges
            .iter()
            .any(|e| e.to_param.as_deref() == Some("team"));
        assert!(spill, "резерв получает подытог через toParam");
        let lines = instance
            .edges
            .iter()
            .filter(|e| e.from_line.is_some())
            .count();
        assert_eq!(lines, 2, "график платежей адресуется по строкам");
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
        let canvas = canvas_core::Canvas {
            nodes: instance.nodes,
            edges: instance.edges,
            ..canvas_core::Canvas::default()
        };
        let mut scene = crate::scene::SceneState::new(canvas, PathBuf::from("oracle.canvas"));
        scene.recompute_flow();
        (scene, map)
    }

    #[test]
    fn oracle_intro_calculations() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.intro-calculations");
        assert!((value_of(&scene, &map, "load") - 5000.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "share") - 0.625).abs() < 1e-6);
    }

    #[test]
    fn oracle_intro_whatif() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.intro-whatif");
        assert!((value_of(&scene, &map, "total") - 1500.0).abs() < 1e-6);
        // Проливание: итог и горизонт приходят в параметры $spend/$months.
        assert!((value_of(&scene, &map, "annual") - 18000.0).abs() < 1e-6);
    }

    #[test]
    fn oracle_capacity_service() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.capacity-service");
        let rps = value_of(&scene, &map, "intensity");
        assert!((rps - 5000.0 / 30.0).abs() < 1e-6, "rps = {rps}");
        let util = value_of(&scene, &map, "util");
        assert!(
            (util - (5000.0 / 30.0) / 200.0).abs() < 1e-6,
            "util = {util}"
        );
        // M/M/1: W = W_q + 1/μ = ρ/(μ−λ) + 1/μ при c=1.
        let wait = value_of(&scene, &map, "wait");
        let rho = (5000.0 / 30.0) / 200.0;
        let expected = rho / (200.0 - 5000.0 / 30.0) + 1.0 / 200.0;
        assert!((wait - expected).abs() < 1e-6, "wait = {wait}");
        // Проливание: $rps и $factor приходят в пиковый поток по имени.
        assert!((value_of(&scene, &map, "peak") - 500.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "peak_util") - 2.5).abs() < 1e-6);
    }

    #[test]
    fn oracle_project_budget() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.project-budget");
        assert!((value_of(&scene, &map, "subtotals") - 1400.0).abs() < 1e-6);
        // Проливание: $team = 13500 из именованного выхода, $share = 0.15.
        assert!((value_of(&scene, &map, "reserve") - 2025.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "total") - 16925.0).abs() < 1e-6);
        // Построчные истоки: доли 0.6/0.4 из строк графика платежей.
        assert!((value_of(&scene, &map, "cash") - 16925.0).abs() < 1e-6);
    }

    #[test]
    fn oracle_unit_economics() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.unit-economics");
        assert!((value_of(&scene, &map, "margin") - 6.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "ltv") - 216.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "ratio") - 1.8).abs() < 1e-6);
        // Проливание: $cac и $margin приходят в формулу окупаемости.
        assert!((value_of(&scene, &map, "payback") - 20.0).abs() < 1e-6);
    }

    #[test]
    fn oracle_renovation_estimate() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.renovation-estimate");
        assert!((value_of(&scene, &map, "space") - 30.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "cost-living") - 450.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "cost-bedroom") - 300.0).abs() < 1e-6);
        assert!((value_of(&scene, &map, "total") - 1450.0).abs() < 1e-6);
        // Проливание: $total и $discount приходят в итог со скидкой.
        assert!((value_of(&scene, &map, "final") - 1305.0).abs() < 1e-6);
    }

    /// Erlang-B/C рекуррентность (образец queueing.rs) — для оракула штата.
    fn erlang_c_probability(offered_load: f64, servers: usize) -> f64 {
        let mut b = 1.0f64;
        for n in 1..=servers {
            b = offered_load * b / (n as f64 + offered_load * b);
        }
        let rho = offered_load / servers as f64;
        b / (1.0 - rho * (1.0 - b))
    }

    #[test]
    fn oracle_support_staffing() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.support-staffing");
        // λ = 240 req / 1 h = 1/15 req/s; μ = 1/180 req/s.
        let lambda = value_of(&scene, &map, "flow");
        assert!((lambda - 240.0 / 3600.0).abs() < 1e-9, "lambda = {lambda}");
        let mu = value_of(&scene, &map, "speed");
        assert!((mu - 1.0 / 180.0).abs() < 1e-9, "mu = {mu}");
        // ρ = λ/(c·μ) = 0.8 ровно.
        let rho = value_of(&scene, &map, "occupancy");
        assert!((rho - 0.8).abs() < 1e-9, "rho = {rho}");
        // Эрланг C: доля ожидающих при a = λ/μ = 12, c = 15.
        let expected_p = erlang_c_probability(12.0, 15);
        let p_wait = value_of(&scene, &map, "wait_prob");
        assert!((p_wait - expected_p).abs() < 1e-9, "p_wait = {p_wait}");
        // M/M/c: W = C/(c·μ − λ) + 1/μ (без перегрузки: ρ = 0.8 < 1).
        let wait = value_of(&scene, &map, "response");
        let expected_w = expected_p / (15.0 * mu - lambda) + 1.0 / mu;
        assert!((wait - expected_w).abs() < 1e-9, "wait = {wait}");
        // Литтл: L = λ·W.
        let wip = value_of(&scene, &map, "wip");
        assert!((wip - lambda * expected_w).abs() < 1e-9, "wip = {wip}");
    }

    #[test]
    fn oracle_investment_case() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.investment-case");
        // PV: y1..y4 дисконтированы степенями 1.12 (нулевой год — 0 $).
        let flows = [150000.0, 220000.0, 280000.0, 320000.0];
        let expected_pv: f64 = flows
            .iter()
            .enumerate()
            .map(|(i, cf)| cf / 1.12f64.powi(i as i32 + 1))
            .sum();
        let pv = value_of(&scene, &map, "pv");
        assert!((pv - expected_pv).abs() < 1e-6, "pv = {pv}");
        // Ретрансляция вложений: последняя строка узла NPV — capex.
        let npv_last = value_of(&scene, &map, "npv");
        assert!((npv_last - 500000.0).abs() < 1e-9, "npv relay = {npv_last}");
        // Проливание: $npv и $capex приходят в индекс по имени;
        // pi = npv_sum/capex + 1 = PV/capex.
        let pi = value_of(&scene, &map, "pi");
        assert!((pi - (expected_pv / 500000.0)).abs() < 1e-6, "pi = {pi}");
        // IRR: корень между ставкой (0.12) и утроенной ставкой; NPV в
        // найденной точке ≈ 0 — бизнес-смысл «проект окупается быстрее».
        let irr = value_of(&scene, &map, "irr");
        assert!(
            irr > 0.12 && irr < 0.36 && (irr - 0.284).abs() < 0.01,
            "irr = {irr}"
        );
        // CAGR потоков: (y4/y1)^(1/3) − 1.
        let growth = value_of(&scene, &map, "growth");
        let expected_g = (320000.0f64 / 150000.0).powf(1.0 / 3.0) - 1.0;
        assert!((growth - expected_g).abs() < 1e-9, "growth = {growth}");
    }

    #[test]
    fn oracle_cohort_launch() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.cohort-launch");
        // Кривая удержания: контрольная точка движка (см. scratch-валидацию
        // и queueing::cohort_ltv — интеграл по дням с хвостом 0.35^(t/30)).
        let ltv = value_of(&scene, &map, "ltv");
        assert!((ltv - 7.476760194027841).abs() < 1e-9, "ltv = {ltv}");
        // Проливание: $users и $ltv приходят в волну по имени.
        let wave = value_of(&scene, &map, "wave");
        assert!((wave - 10000.0 * ltv).abs() < 1e-6, "wave = {wave}");
        let cac = value_of(&scene, &map, "cac");
        assert!((cac - 1.2).abs() < 1e-9, "cac = {cac}");
        let ratio = value_of(&scene, &map, "ratio");
        assert!((ratio - ltv / 1.2).abs() < 1e-9, "ratio = {ratio}");
        let net = value_of(&scene, &map, "net");
        assert!(
            (net - (10000.0 * ltv - 12000.0)).abs() < 1e-6,
            "net = {net}"
        );
    }

    #[test]
    fn oracle_runway() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.runway");
        // sum по четырём статьям.
        let burn = value_of(&scene, &map, "burn");
        assert!((burn - 63200.0).abs() < 1e-9, "burn = {burn}");
        // Проливание: $burn − $revenue = 63200 − 41000.
        let net = value_of(&scene, &map, "net");
        assert!((net - 22200.0).abs() < 1e-9, "net = {net}");
        let yearly = value_of(&scene, &map, "yearly");
        assert!((yearly - 266400.0).abs() < 1e-9, "yearly = {yearly}");
        // Проливание: $balance / $net = 1200000 / 22200.
        let runway = value_of(&scene, &map, "runway");
        assert!(
            (runway - 1200000.0 / 22200.0).abs() < 1e-9,
            "runway = {runway}"
        );
    }

    /// FR-016: capacity-service демонстрирует анализ узких мест: средняя
    /// утилизация — Warn (ρ ≈ 0.83 ≥ 0.7), пиковая — Overload (2.5 ≥ 1).
    #[test]
    fn capacity_service_triggers_bottleneck_analysis() {
        use canvas_core::analyze::Severity;
        let (scene, map) = scene_with("com.canvasdesk.scheme.capacity-service");
        let util = &scene.analysis[&map["util"]];
        assert_eq!(
            util.severity,
            Severity::Warn,
            "средняя занятость — жёлтая рамка"
        );
        let peak = &scene.analysis[&map["peak_util"]];
        assert_eq!(peak.severity, Severity::Overload, "пик — перегрузка");
        assert!(
            (peak.utilization.unwrap() - 2.5).abs() < 1e-6,
            "ρ пика = 2.5"
        );
    }

    /// Проза-заметки не порождают значений и ошибок (инвариант тишины прозы).
    #[test]
    fn prose_notes_stay_silent() {
        let (scene, map) = scene_with("com.canvasdesk.scheme.intro-calculations");
        let hint = scene.expr_line_results.get(&map["hint"]);
        // Нода-подсказка: все строки без результата (проза молчит).
        if let Some(lines) = hint {
            assert!(lines.iter().all(|line| line.is_none()), "проза-нода молчит");
        }
    }

    /// Схема вставляется в канвас с существующими id без ошибок ремапа
    /// и пересчитывается (US-3 AC-3.1).
    #[test]
    fn instantiate_into_occupied_canvas_recomputes() {
        let mut canvas = canvas_core::Canvas::default();
        canvas
            .nodes
            .push(Node::text("note-1", "существующая нода", 0.0, 0.0));
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

    // --- Инварианты контента v2 (PRD-0008 §8 F-5, запрос владельца) ---
    // Требования: описания в каждой ноде, пучки для main stage, ноды с
    // множественными связями, адресация/проливание, глубина цепочки под
    // объяснение происхождения цифр, отсутствие красных строк.

    use canvas_core::expr::{line_kind, NumiLineKind};
    use canvas_core::schemes::{SchemeEdge, SchemeManifest, SchemeNode};

    fn all_schemes() -> Vec<&'static SchemeManifest> {
        SchemeRegistry::embedded().list().iter().collect()
    }

    /// Требование владельца: «краткие описания в каждую ноду, что там за
    /// значение» — первая строка каждой текстовой ноды обязана быть прозой
    /// (заголовок-объект для адресов «Объект.Поле»), минимум две
    /// прозаические строки (заголовок + пояснение смысла значений).
    #[test]
    fn every_text_node_is_documented() {
        for scheme in all_schemes() {
            for node in &scheme.content.nodes {
                if node.node_type != "text" {
                    continue;
                }
                let text = node.text.as_deref().unwrap_or_default();
                let lines: Vec<&str> = text.split('\n').collect();
                let first = lines[0].trim();
                assert!(
                    !first.is_empty(),
                    "{}: нода {} без заголовка",
                    scheme.id,
                    node.id
                );
                assert_eq!(
                    line_kind(first),
                    NumiLineKind::Prose,
                    "{}: первая строка {} должна быть прозой (заголовок), не формулой",
                    scheme.id,
                    node.id
                );
                let prose = lines
                    .iter()
                    .filter(|l| !l.trim().is_empty() && line_kind(l.trim()) == NumiLineKind::Prose)
                    .count();
                assert!(
                    prose >= 2,
                    "{}: нода {} — нужно заголовок и пояснение (не менее 2 проза-строк), проза-строк: {prose}",
                    scheme.id,
                    node.id
                );
            }
        }
    }

    /// Main stage открывается по пучку из ≥2 рёбер одной упорядоченной пары
    /// (FR-042): каждая схема обязана содержать хотя бы один такой пучок.
    #[test]
    fn every_scheme_opens_main_stage() {
        for scheme in all_schemes() {
            let mut pairs: std::collections::HashMap<(String, String), usize> =
                std::collections::HashMap::new();
            for edge in &scheme.content.edges {
                *pairs
                    .entry((edge.from_node.clone(), edge.to_node.clone()))
                    .or_insert(0) += 1;
            }
            let bundles: Vec<_> = pairs.iter().filter(|(_, n)| **n >= 2).collect();
            assert!(
                !bundles.is_empty(),
                "{}: нет пучка для main stage (нужно ≥2 рёбер одной пары)",
                scheme.id
            );
            for (pair, weight) in bundles {
                // Якоря веера: fromOutput (именованный выход) ИЛИ fromLine
                // (построчный исток) — оба заякоривают порты на строках.
                let addressed = scheme.content.edges.iter().any(|e| {
                    e.from_node == pair.0 && (e.from_output.is_some() || e.from_line.is_some())
                });
                assert!(
                    addressed,
                    "{}: пучок {:?} без адресации истока (веер без якорей)",
                    scheme.id, pair
                );
                let _ = weight;
            }
        }
    }

    /// «Ноды с множественными связями»: в каждой схеме есть нода с ≥3
    /// инцидентными value-рёбрами (fan-in/fan-out), обычно несколько.
    #[test]
    fn every_scheme_has_multi_connected_nodes() {
        for scheme in all_schemes() {
            let mut degree: std::collections::HashMap<&str, usize> =
                std::collections::HashMap::new();
            for edge in &scheme.content.edges {
                *degree.entry(edge.from_node.as_str()).or_insert(0) += 1;
                *degree.entry(edge.to_node.as_str()).or_insert(0) += 1;
            }
            let hubs = degree.values().filter(|d| **d >= 3).count();
            assert!(
                hubs >= 1,
                "{}: нет ноды с ≥3 value-связями (множественные связи)",
                scheme.id
            );
        }
    }

    /// Трассируемость фич: позиционные слоты и именованные выходы — во всех
    /// схемах; проливание toParam — во всех, кроме вводной (она учит по
    /// одному механизму за раз); построчные истоки fromLine — в бюджете.
    #[test]
    fn schemes_cover_addressing_features() {
        for scheme in all_schemes() {
            let from_output = scheme.content.edges.iter().any(|e| e.from_output.is_some());
            assert!(
                from_output,
                "{}: нет fromOutput (именованные выходы не демонстрируются)",
                scheme.id
            );
            if scheme.id != "com.canvasdesk.scheme.intro-calculations" {
                let spill = scheme.content.edges.iter().any(|e| e.to_param.is_some());
                assert!(
                    spill,
                    "{}: нет toParam (проливание значений не демонстрируется)",
                    scheme.id
                );
            }
        }
        let budget = SchemeRegistry::embedded()
            .get("com.canvasdesk.scheme.project-budget")
            .unwrap();
        assert!(
            budget.content.edges.iter().any(|e| e.from_line.is_some()),
            "бюджет: нет fromLine (построчные истоки не демонстрируются)"
        );
    }

    /// Глубина цепочки значений (готовность к объяснению происхождения
    /// цифр, PRD-0007): самый длинный путь по value-рёбрам от листа.
    /// Вводные схемы — ≥2 хопа, прочие — ≥3.
    #[test]
    fn schemes_have_deep_value_chains() {
        for scheme in all_schemes() {
            let mut best = 0usize;
            let ids: Vec<&str> = scheme.content.nodes.iter().map(|n| n.id.as_str()).collect();
            for start in &ids {
                let mut stack: Vec<(&str, usize)> = vec![(start, 0)];
                while let Some((node, depth)) = stack.pop() {
                    best = best.max(depth);
                    for edge in &scheme.content.edges {
                        if edge.from_node == node {
                            stack.push((edge.to_node.as_str(), depth + 1));
                        }
                    }
                }
            }
            let floor = if scheme.category == "onboarding" {
                2
            } else {
                3
            };
            assert!(
                best >= floor,
                "{}: цепочка значений {best} хопов, нужно ≥{floor} (дерево происхождения)",
                scheme.id
            );
        }
    }

    /// Живость без красного: ни одна строка и ни один узловой итог схем не
    /// дают ошибок — вставленная схема не выглядит «сломанной».
    #[test]
    fn schemes_never_show_red_lines() {
        for scheme in all_schemes() {
            let (scene, map) = scene_with(&scheme.id);
            for (node_id, outcome) in &scene.expr_results {
                assert!(
                    !matches!(outcome, ExprOutcome::Err(_)),
                    "{}: нода {node_id} в ошибке: {outcome:?}",
                    scheme.id
                );
            }
            for (node_id, lines) in &scene.expr_line_results {
                for (i, line) in lines.iter().enumerate() {
                    assert!(
                        !matches!(line, Some(ExprOutcome::Err(_))),
                        "{}: нода {node_id} строка {i} красная: {line:?}",
                        scheme.id
                    );
                }
            }
            // Формульные ноды живы: у каждой есть Ok-итог.
            for node in &scheme.content.nodes {
                if node.node_type != "text" {
                    continue;
                }
                let text = node.text.as_deref().unwrap_or_default();
                let has_formula = text
                    .split('\n')
                    .any(|l| line_kind(l.trim()) != NumiLineKind::Prose && !l.trim().is_empty());
                if has_formula {
                    let id = &map[&node.id];
                    assert!(
                        scene
                            .expr_results
                            .get(id)
                            .is_some_and(|o| !matches!(o, ExprOutcome::Err(_))),
                        "{}: формульная нода {} без живого итога",
                        scheme.id,
                        node.id
                    );
                }
            }
        }
    }

    /// D5 CJM: подсказка-приглашение «поменяйте число — цепочка
    /// пересчитается» живёт в каждой схеме (нода-проза hint).
    #[test]
    fn every_scheme_invites_to_edit() {
        for scheme in all_schemes() {
            let hint = scheme
                .content
                .nodes
                .iter()
                .find(|n| n.id == "hint")
                .unwrap_or_else(|| panic!("{}: нет ноды-подсказки hint", scheme.id));
            let text = hint.text.as_deref().unwrap_or_default();
            assert!(
                text.contains("пересчит"),
                "{}: hint не приглашает к правке (D5)",
                scheme.id
            );
            assert!(
                text.split('\n')
                    .all(|l| { l.trim().is_empty() || line_kind(l.trim()) == NumiLineKind::Prose }),
                "{}: hint обязан быть чистой прозой (тишина)",
                scheme.id
            );
        }
    }

    /// Усиление тишины (аудит схем 2026-09-25): подсказки и вердикты —
    /// пресет «4» (hint/verdict/try) — обязаны молчать ЦЕЛИКОМ во всех
    /// схемах: проза с числами («83 процента», «итог — 16925») не имеет
    /// права вычисляться (иначе вердикт превращается в «живую» карточку).
    #[test]
    fn hint_and_verdict_notes_stay_silent() {
        for scheme in all_schemes() {
            let (scene, map) = scene_with(&scheme.id);
            for node in &scheme.content.nodes {
                if node.color.as_deref() != Some("4") || node.node_type != "text" {
                    continue;
                }
                let id = &map[&node.id];
                let lines = scene.expr_line_results.get(id);
                if let Some(lines) = lines {
                    for (i, line) in lines.iter().enumerate() {
                        assert!(
                            line.is_none(),
                            "{}: проза-нода {} строка {i} вычислилась: {line:?}",
                            scheme.id,
                            node.id
                        );
                    }
                }
            }
        }
    }

    /// Цветовая семантика схем (аудит CJM 2026-09-22): красный пресет
    /// «1» в заливках конфликтует с красной рамкой перегрузки FR-016 —
    /// в схемах он запрещён (тревога — только анализ). Входы — «5» (циан),
    /// расчёты — «2», итоги — «6», подсказки/вердикты — «4», группы — «3».
    #[test]
    fn schemes_avoid_red_node_fills() {
        for scheme in all_schemes() {
            for node in &scheme.content.nodes {
                assert_ne!(
                    node.color.as_deref(),
                    Some("1"),
                    "{}: нода {} покрашена красным (пресет тревоги FR-016)",
                    scheme.id,
                    node.id
                );
            }
        }
    }

    /// Манифестная ссылка для инвариантов формата (компиляция полей
    /// адресации в serde-слое не деградирует).
    #[test]
    fn manifest_addressing_fields_parse() {
        let json = r#"{
            "id": "e", "fromNode": "a", "toNode": "b",
            "flowKind": "value", "fromLine": 3
        }"#;
        let edge: SchemeEdge = serde_json::from_str(json).unwrap();
        assert_eq!(edge.from_line, Some(3));
        let node: SchemeNode =
            serde_json::from_str(r#"{"id": "g", "type": "group", "x": 0, "y": 0, "width": 10, "height": 10, "label": "Команда"}"#)
                .unwrap();
        assert_eq!(node.label.as_deref(), Some("Команда"));
    }

    // --- FR-071: oracle-инварианты умной раскладки -----------------------

    /// Канвас инстанса схемы (нативно и под wasip1 — чистые функции ядра).
    fn laid_canvas(id: &str) -> (canvas_core::Canvas, SchemeInstance) {
        let instance = instance_of(id);
        let canvas = canvas_core::Canvas {
            nodes: instance.nodes.clone(),
            edges: instance.edges.clone(),
            ..Default::default()
        };
        (canvas, instance)
    }

    /// Oracle G-раскладка: во всех built-in схемах после умной раскладки
    /// 0 пересечений «ребро × нода» (прямые отрезки порт→порт, bbox
    /// инфлирован на CROSSING_MARGIN — метрика `count_edge_node_crossings`).
    #[test]
    fn smart_layout_zero_edge_node_crossings() {
        for scheme in SchemeRegistry::embedded().list() {
            let (canvas, _) = laid_canvas(&scheme.id);
            let crossings = canvas_core::scheme_layout::count_edge_node_crossings(&canvas);
            assert_eq!(
                crossings, 0,
                "{}: {crossings} пересечений рёбер с нодами после раскладки",
                scheme.id
            );
        }
    }

    /// Oracle G-раскладка: bbox не-групповых нод не пересекаются.
    #[test]
    fn smart_layout_no_bbox_overlaps() {
        for scheme in SchemeRegistry::embedded().list() {
            let (canvas, _) = laid_canvas(&scheme.id);
            for (i, a) in canvas.nodes.iter().enumerate() {
                if a.kind() == canvas_core::NodeKind::Group {
                    continue;
                }
                for b in canvas.nodes.iter().skip(i + 1) {
                    if b.kind() == canvas_core::NodeKind::Group {
                        continue;
                    }
                    assert!(
                        !(a.x < b.x + b.width
                            && b.x < a.x + a.width
                            && a.y < b.y + b.height
                            && b.y < a.y + a.height),
                        "{}: ноды {} и {} пересекаются",
                        scheme.id,
                        a.id,
                        b.id
                    );
                }
            }
        }
    }

    /// Oracle G-раскладка: дети каждой явной группы геометрически внутри
    /// рамки (инвариант FR-012; рамка = bbox детей + GROUP_PAD).
    #[test]
    fn smart_layout_groups_contain_children() {
        for scheme in SchemeRegistry::embedded().list() {
            let (canvas, _) = laid_canvas(&scheme.id);
            for (gi, group) in canvas.nodes.iter().enumerate() {
                if group.kind() != canvas_core::NodeKind::Group {
                    continue;
                }
                let Some(children) = &group.children else {
                    continue;
                };
                assert!(
                    !children.is_empty(),
                    "{}: группа {} без детей",
                    scheme.id,
                    group.id
                );
                for child_id in children {
                    let Some(child) = canvas.node(child_id) else {
                        panic!(
                            "{}: группа {}: висячий ребёнок {child_id}",
                            scheme.id, group.id
                        );
                    };
                    assert!(
                        child.x >= group.x - f32::EPSILON
                            && child.y >= group.y - f32::EPSILON
                            && child.x + child.width <= group.x + group.width + f32::EPSILON
                            && child.y + child.height <= group.y + group.height + f32::EPSILON,
                        "{}: ребёнок {child_id} вне рамки группы {}",
                        scheme.id,
                        group.id
                    );
                }
                let _ = gi;
            }
        }
    }

    /// Oracle G-раскладка: раскладка детерминирована — два инстанса одной
    /// схемы в пустой канвас дают идентичные позиции (после нормировки
    /// сдвигом к origin — совпадают побитово).
    #[test]
    fn smart_layout_is_deterministic() {
        for scheme in SchemeRegistry::embedded().list() {
            let (a, _) = laid_canvas(&scheme.id);
            let (b, _) = laid_canvas(&scheme.id);
            for (na, nb) in a.nodes.iter().zip(b.nodes.iter()) {
                assert_eq!(na.id, nb.id, "{}: порядок нод стабилен", scheme.id);
                assert_eq!((na.x, na.y), (nb.x, nb.y), "{}: нода {}", scheme.id, na.id);
            }
        }
    }
}
