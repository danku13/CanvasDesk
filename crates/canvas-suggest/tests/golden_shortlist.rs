//! Golden-гейт шортлиста: порт `shortlist.py` воспроизводит picked-наборы
//! всех 226 контекстов evalset (golden="" — продуктовый режим без golden,
//! категория схемы — по правилу synth.py).
//!
//! CI-регресс FR-079 S3: порядок префильтра каталога не дрейфует.

mod common;

use canvas_suggest::catalog::{CatalogParam, CatalogTemplate};
use canvas_suggest::shortlist::shortlist_keys;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Row {
    id: String,
    category: String,
    context: String,
    picked: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct TemplateRow {
    key: String,
    name_ru: String,
    name_en: String,
    category: String,
    #[serde(default)]
    params: Vec<ParamRow>,
}

#[derive(Debug, Deserialize)]
struct ParamRow {
    name: String,
}

fn load_rows() -> Vec<Row> {
    let path = common::fixtures_dir().join("shortlist_picked.json");
    let text = std::fs::read_to_string(&path).expect("shortlist_picked.json читается");
    serde_json::from_str(&text).expect("разбор shortlist_picked.json")
}

fn load_templates() -> Vec<CatalogTemplate> {
    let path = common::fixtures_dir().join("catalog_options.json");
    let text = std::fs::read_to_string(&path).expect("catalog_options.json читается");
    let rows: Vec<TemplateRow> = serde_json::from_str(&text).expect("разбор каталога");
    rows.into_iter()
        .map(|r| CatalogTemplate {
            key: r.key,
            name_ru: if r.name_ru.is_empty() {
                None
            } else {
                Some(r.name_ru)
            },
            name_en: r.name_en,
            description: String::new(),
            description_en: None,
            category: r.category,
            params: r
                .params
                .into_iter()
                .map(|p| CatalogParam {
                    name: p.name,
                    unit: None,
                    ptype: None,
                })
                .collect(),
        })
        .collect()
}

#[test]
fn shortlist_reproduces_frozen_picked() {
    let rows = load_rows();
    assert_eq!(rows.len(), 226, "226 контекстов evalset");
    let templates = load_templates();
    assert_eq!(templates.len(), 62);
    let mut mismatches = 0;
    for row in &rows {
        let picked = shortlist_keys(&row.context, &row.category, &templates, 20);
        if picked != row.picked {
            mismatches += 1;
            if mismatches <= 3 {
                eprintln!(
                    "[{}] расхождение:\n  py: {:?}\n  rs: {:?}",
                    row.id, row.picked, picked
                );
            }
        }
    }
    assert_eq!(mismatches, 0, "побитовая совместимость shortlist");
}
