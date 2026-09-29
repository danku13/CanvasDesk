//! Golden-контракт сериализатора контекста: побайтовое воспроизведение
//! эталонного Python `serialize.py: build_context` (волна 1) на парах
//! вход→выход, снятых с реальных схем репозитория.
//!
//! CI-регресс FR-079 §Verification: «строки побайтово, 0 расхождений».

mod common;

use canvas_suggest::context::{build_context, RawEdge, RawNode};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Deserialize)]
struct Pair {
    id: String,
    nodes: Vec<PNode>,
    edges: Vec<PEdge>,
    hole_id: String,
    #[serde(default)]
    editing: String,
    #[serde(default)]
    hole_title: String,
    categories: HashMap<String, String>,
    expected: String,
}

#[derive(Debug, Deserialize)]
struct PNode {
    id: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PEdge {
    from: String,
    to: String,
    #[serde(default)]
    from_output: Option<String>,
    #[serde(default)]
    to_param: Option<String>,
}

#[test]
fn context_builder_byte_exact() {
    let path = common::fixtures_dir().join("context_pairs.jsonl");
    let text = fs::read_to_string(&path).expect("context_pairs.jsonl читается");
    let pairs: Vec<Pair> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("разбор пары"))
        .collect();
    assert!(pairs.len() >= 8, "пар снято с эталона: {}", pairs.len());

    for p in &pairs {
        let nodes: Vec<RawNode> = p
            .nodes
            .iter()
            .map(|n| RawNode {
                id: &n.id,
                text: &n.text,
                label: n.label.as_deref(),
            })
            .collect();
        let edges: Vec<RawEdge> = p
            .edges
            .iter()
            .map(|e| RawEdge {
                from: &e.from,
                to: &e.to,
                from_output: e.from_output.as_deref(),
                to_param: e.to_param.as_deref(),
            })
            .collect();
        let got = build_context(
            &nodes,
            &edges,
            &p.hole_id,
            &p.editing,
            &p.hole_title,
            &p.categories,
        );
        assert_eq!(
            got, p.expected,
            "пара {}: расхождение с эталоном Python\n--- got ---\n{got}\n--- expected ---\n{}",
            p.id, p.expected
        );
    }
}
