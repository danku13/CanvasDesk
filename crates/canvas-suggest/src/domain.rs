//! Домен-детектор C4: «канвас вне каталога» → не предлагаем (FR-079 §2.3).
//!
//! Правило, не ML: волна 3 показала, что сигнал «канвас вне каталога»
//! отсутствует в признаках zero-shot mm (framework-домены 0.036–0.071,
//! H4 побитово) — его дешевле привнести правилом до L1.
//!
//! Признаки считаются по разобранным нодам ([`crate::context::parse_node`]):
//! - `template_nodes` — ноды, инстанцированные из шаблонов;
//! - `assigns` — всего Numi-присваиваний (строк-формул);
//! - `value_edges` — рёбра, чей `from_output` определён формулой источника
//!   (у framework-схем fromOutput ссылается на неопределённые «порты»).
//!
//! Вердикты:
//! - [`Verdict::Unknown`] — канвас < 5 нод: cold start, предлагаем
//!   (не душим нового пользователя);
//! - [`Verdict::Calculation`] — есть шаблонные ноды ≥ 1, ИЛИ присваиваний
//!   ≥ 2, ИЛИ value-рёбра ≥ 1: расчётная модель, предлагаем (C1/C3);
//! - [`Verdict::Framework`] — 0 формул и ≥ 5 edge-связанных прозаических
//!   нод: структурированная карта (CJM/JTBD/Service Blueprint) — не предлагаем;
//! - [`Verdict::Bespoke`] — остальное (слабый расчётный сигнал, проза):
//!   не предлагаем.
//!
//! Пороги откалиброваны на 14 схемах `assets/canvas-schemes/`
//! (см. `tests/golden_domain.rs`): 11 расчётных — 0 ложных подавлений,
//! 3 framework (cjm-saas/jtbd-saas/service-blueprint-saas) — 100%
//! подавлений, включая вырезанные дырки.

use crate::context::parse_node;
use std::collections::{HashMap, HashSet};

/// Вердикт домена.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Расчётная модель — предлагаем шаблоны (C1/C3).
    Calculation,
    /// Framework-канвас (CJM/JTBD/SB) — не предлагаем.
    Framework,
    /// Bespoke-проза (заметки без формул) — не предлагаем.
    Bespoke,
    /// Малый/пустой канвас (< 5 нод) — предлагаем (cold start).
    Unknown,
}

impl Verdict {
    /// Предлагать ли подсказки для этого домена.
    pub fn should_suggest(&self) -> bool {
        matches!(self, Verdict::Calculation | Verdict::Unknown)
    }

    /// Короткое имя для логов/HUD.
    pub fn as_str(&self) -> &'static str {
        match self {
            Verdict::Calculation => "calculation",
            Verdict::Framework => "framework",
            Verdict::Bespoke => "bespoke",
            Verdict::Unknown => "unknown",
        }
    }
}

/// Нода канваса для домен-детектора (лёгкий вход, развязан от canvas-core;
/// адаптер приложения — S3).
#[derive(Debug, Clone, Copy)]
pub struct DomainNode<'a> {
    pub id: &'a str,
    pub text: &'a str,
    pub label: Option<&'a str>,
    /// Ключ шаблона, если нода — инстанс (structural templateId).
    pub template_key: Option<&'a str>,
}

/// Ребро канваса для домен-детектора.
#[derive(Debug, Clone, Copy)]
pub struct DomainEdge<'a> {
    pub from: &'a str,
    pub to: &'a str,
    pub from_output: Option<&'a str>,
    pub to_param: Option<&'a str>,
}

/// Минимум нод для вердикта (не Unknown).
const MIN_NODES: usize = 5;
/// Минимум edge-связанных прозаических нод для Framework.
const MIN_LINKED_PROSE: usize = 5;

/// Определить домен канваса.
pub fn detect(nodes: &[DomainNode], edges: &[DomainEdge]) -> Verdict {
    if nodes.len() < MIN_NODES {
        return Verdict::Unknown;
    }

    let by_id: HashMap<&str, &DomainNode> = nodes.iter().map(|n| (n.id, n)).collect();
    // определяет ли нода-источник свой from_output формулой
    let node_defines: HashMap<&str, HashSet<String>> = nodes
        .iter()
        .map(|n| {
            let p = parse_node(n.text, n.label);
            (n.id, p.defines.iter().cloned().collect::<HashSet<String>>())
        })
        .collect();

    let template_nodes = nodes
        .iter()
        .filter(|n| n.template_key.is_some_and(|k| !k.is_empty()))
        .count();
    let assigns: usize = nodes
        .iter()
        .map(|n| parse_node(n.text, n.label).formulas.len())
        .sum();
    let value_edges = edges
        .iter()
        .filter(|e| {
            e.from_output.is_some_and(|out| {
                !out.is_empty()
                    && by_id
                        .get(e.from)
                        .and_then(|src| node_defines.get(src.id))
                        .is_some_and(|d| d.contains(out))
            })
        })
        .count();

    if template_nodes >= 1 || assigns >= 2 || value_edges >= 1 {
        return Verdict::Calculation;
    }

    // нет расчётного сигнала: проза. Связанная карта → framework, иначе bespoke
    let formula_nodes: HashSet<&str> = nodes
        .iter()
        .filter(|n| !parse_node(n.text, n.label).formulas.is_empty())
        .map(|n| n.id)
        .collect();
    let connected: HashSet<&str> = edges.iter().flat_map(|e| [e.from, e.to]).collect();
    let linked_prose = nodes
        .iter()
        .filter(|n| !formula_nodes.contains(n.id) && connected.contains(n.id))
        .count();
    if formula_nodes.is_empty() && linked_prose >= MIN_LINKED_PROSE {
        Verdict::Framework
    } else {
        Verdict::Bespoke
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prose_node<'a>(id: &'a str, text: &'a str) -> DomainNode<'a> {
        DomainNode {
            id,
            text,
            label: None,
            template_key: None,
        }
    }
    fn edge<'a>(from: &'a str, to: &'a str) -> DomainEdge<'a> {
        DomainEdge {
            from,
            to,
            from_output: None,
            to_param: None,
        }
    }

    #[test]
    fn small_canvas_is_unknown() {
        let nodes = vec![prose_node("a", "заметка"), prose_node("b", "ещё")];
        assert_eq!(detect(&nodes, &[]), Verdict::Unknown);
        assert!(detect(&nodes, &[]).should_suggest());
    }

    #[test]
    fn formulas_mean_calculation() {
        let nodes = vec![
            prose_node("a", "Расход\nburn = 120000"),
            prose_node("b", "Кэш\ncash = 2_000_000"),
            prose_node("c", "Рунвей"),
            prose_node("d", "Комментарий"),
            prose_node("e", "Ещё"),
        ];
        assert_eq!(detect(&nodes, &[]), Verdict::Calculation);
    }

    #[test]
    fn template_node_means_calculation() {
        let mut n = prose_node("a", "LTV");
        n.template_key = Some("ue-ltv");
        let nodes = vec![
            n,
            prose_node("b", "заметка"),
            prose_node("c", "заметка"),
            prose_node("d", "заметка"),
            prose_node("e", "заметка"),
        ];
        assert_eq!(detect(&nodes, &[]), Verdict::Calculation);
    }

    #[test]
    fn linked_prose_map_is_framework() {
        let ids: Vec<String> = (0..6).map(|i| format!("n{i}")).collect();
        let nodes: Vec<DomainNode> = ids
            .iter()
            .map(|id| prose_node(id, "Стадия пути пользователя"))
            .collect();
        let edges = vec![
            edge("n0", "n1"),
            edge("n1", "n2"),
            edge("n2", "n3"),
            edge("n3", "n4"),
            edge("n4", "n5"),
        ];
        assert_eq!(detect(&nodes, &edges), Verdict::Framework);
        assert!(!detect(&nodes, &edges).should_suggest());
    }

    #[test]
    fn loose_prose_is_bespoke() {
        let ids: Vec<String> = (0..5).map(|i| format!("n{i}")).collect();
        let nodes: Vec<DomainNode> = ids
            .iter()
            .map(|id| prose_node(id, "Идея для ремонта кухни"))
            .collect();
        assert_eq!(detect(&nodes, &[]), Verdict::Bespoke);
        assert!(!detect(&nodes, &[]).should_suggest());
    }

    #[test]
    fn framework_edge_outputs_undefined() {
        // fromOutput ссылается на «порт», не определённый формулой → не value-edge;
        // все 5 прозаических нод связаны рёбрами → структурированная карта
        let nodes = vec![
            prose_node("stage", "Осознание\nпотребность есть"),
            prose_node("tp", "Реклама"),
            prose_node("em", "Интерес"),
            prose_node("pain", "Дорого"),
            prose_node("next", "Рассмотрение"),
        ];
        let edges = vec![
            DomainEdge {
                from: "stage",
                to: "tp",
                from_output: Some("stage"),
                to_param: None,
            },
            DomainEdge {
                from: "stage",
                to: "em",
                from_output: Some("emotion"),
                to_param: None,
            },
            DomainEdge {
                from: "tp",
                to: "pain",
                from_output: Some("touchpoint"),
                to_param: None,
            },
            DomainEdge {
                from: "pain",
                to: "next",
                from_output: Some("pain"),
                to_param: None,
            },
        ];
        assert_eq!(detect(&nodes, &edges), Verdict::Framework);
    }

    #[test]
    fn defined_output_means_value_edge() {
        let nodes = vec![
            prose_node("unit", "Единица\nprice = 10"),
            prose_node("margin", "Маржа"),
            prose_node("c", "заметка"),
            prose_node("d", "заметка"),
            prose_node("e", "заметка"),
        ];
        let edges = vec![DomainEdge {
            from: "unit",
            to: "margin",
            from_output: Some("price"),
            to_param: None,
        }];
        // 1 присваивание < 2, но value-ребро определено → Calculation
        assert_eq!(detect(&nodes, &edges), Verdict::Calculation);
    }
}
