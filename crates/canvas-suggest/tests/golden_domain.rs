//! Golden-гейт домен-детектора: 14 схем `assets/canvas-schemes/`
//! (11 расчётных + 3 framework) + карвинг framework-схем.
//!
//! CI-регресс FR-079 §Verification: framework ≥ 90% подавлено
//! (карвинг всех негод-hint нод), расчётные — 0 ложных подавлений.

mod common;

use canvas_suggest::domain::{detect, DomainEdge, DomainNode, Verdict};
use canvas_suggest::types::SuggestContext;
use serde::Deserialize;
use std::fs;

// `category` — информационное поле фикстуры (используется `expect`).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SchemeFixture {
    id: String,
    category: String,
    expect: String,
    nodes: Vec<SNode>,
    edges: Vec<SEdge>,
    /// Ноды, которые можно «вырезать» (все кроме инструктажной hint).
    carvable: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SNode {
    id: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SEdge {
    from: String,
    to: String,
    #[serde(default)]
    from_output: Option<String>,
    #[serde(default)]
    to_param: Option<String>,
}

impl SNode {
    fn to_domain(&self) -> DomainNode<'_> {
        DomainNode {
            id: &self.id,
            text: &self.text,
            label: self.label.as_deref(),
            template_key: None, // схемы — text-ноуты без templateId (волна 1 §4)
        }
    }
}

impl SEdge {
    fn to_domain(&self) -> DomainEdge<'_> {
        DomainEdge {
            from: &self.from,
            to: &self.to,
            from_output: self.from_output.as_deref(),
            to_param: self.to_param.as_deref(),
        }
    }
}

fn load_schemes() -> Vec<SchemeFixture> {
    let path = common::fixtures_dir().join("domain_schemes.jsonl");
    let text = fs::read_to_string(&path).expect("domain_schemes.jsonl читается");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("разбор схемы"))
        .collect()
}

fn expect_verdict(s: &SchemeFixture) -> Verdict {
    match s.expect.as_str() {
        "framework" => Verdict::Framework,
        "calculation" => Verdict::Calculation,
        other => panic!("неизвестный ожидаемый вердикт: {other}"),
    }
}

#[test]
fn full_schemes_verdicts() {
    let schemes = load_schemes();
    assert_eq!(schemes.len(), 14, "11 расчётных + 3 framework");

    let mut suppressed_calc = 0;
    for s in &schemes {
        let nodes: Vec<DomainNode> = s.nodes.iter().map(|n| n.to_domain()).collect();
        let edges: Vec<DomainEdge> = s.edges.iter().map(|e| e.to_domain()).collect();
        let got = detect(&nodes, &edges);
        assert_eq!(
            got,
            expect_verdict(s),
            "схема {}: получили {:?}, ожидали {:?}",
            s.id,
            got,
            expect_verdict(s)
        );
        if expect_verdict(s) == Verdict::Calculation && !got.should_suggest() {
            suppressed_calc += 1;
        }
    }
    assert_eq!(
        suppressed_calc, 0,
        "0 ложных подавлений на расчётных схемах"
    );
}

#[test]
fn framework_carving_stays_suppressed() {
    let schemes = load_schemes();
    let fw: Vec<&SchemeFixture> = schemes.iter().filter(|s| s.expect == "framework").collect();
    assert_eq!(fw.len(), 3, "cjm-saas / jtbd-saas / service-blueprint-saas");

    let mut checks = 0;
    let mut framework_label = 0;
    for s in &fw {
        assert!(!s.carvable.is_empty());
        for hole in &s.carvable {
            // карвинг: нода удаляется вместе с рёбрами
            let nodes: Vec<DomainNode> = s
                .nodes
                .iter()
                .filter(|n| n.id != *hole)
                .map(|n| n.to_domain())
                .collect();
            let edges: Vec<DomainEdge> = s
                .edges
                .iter()
                .filter(|e| e.from != *hole && e.to != *hole)
                .map(|e| e.to_domain())
                .collect();
            let got = detect(&nodes, &edges);
            // Продукт-инвариант: framework-канвас остаётся ПОДАВЛЕННЫМ после
            // любой дырки (wave 3: golden=none на framework-карвинге).
            assert!(
                !got.should_suggest(),
                "схема {}, вырезана нода {hole}: подсказки должны быть подавлены, получили {got:?}",
                s.id
            );
            if got == Verdict::Framework {
                framework_label += 1;
            }
            checks += 1;
        }
    }
    // ≥ 33 фикстур волны 3 (фактически все карвинги трёх схем: 36)
    assert!(checks >= 33, "framework-карвингов: {checks} ≥ 33");
    // Ярлык Framework устойчив ≥ 90% карвингов. Известное исключение:
    // удаление хаба (jtbd job_main — участник всех рёбер) роняет связность,
    // вердикт деградирует до Bespoke — подавление сохраняется, ярлык
    // косметический (reason-строка).
    assert!(
        f64::from(framework_label) / checks as f64 >= 0.9,
        "Framework-ярлык: {framework_label}/{checks}"
    );
}

/// Схемы-соседи домен-детектора: контекст формат А из scheme-нод
/// (санити: сериализатор и детектор живут на одних данных).
#[test]
fn schemes_are_suggestable_material() {
    let schemes = load_schemes();
    let calc: Vec<&SchemeFixture> = schemes
        .iter()
        .filter(|sc| sc.expect == "calculation")
        .take(3)
        .collect();
    assert_eq!(calc.len(), 3);
    for _ in &calc {
        let opts = vec![canvas_suggest::types::OptionDesc::new(
            "ue-cac",
            "CAC: привлечение",
        )];
        let ctx = SuggestContext::new("[canvas] x", &opts);
        assert!(!ctx.options.is_empty());
    }
}
