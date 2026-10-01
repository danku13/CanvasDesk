//! Golden-гейт каталога: порт `template_option` воспроизводит описания
//! опций всех 62 шаблонов побитово (оба языка) на фикстуре, снятой с
//! эталонного Python-harness (архив PoC, коммит e6e666e).
//!
//! CI-регресс FR-079 S3: адаптер каталога приложения строит
//! `CatalogTemplate` из манифестов реестра — если алгоритм глосса
//! поменяется, этот тест упадёт раньше продукта.

mod common;

use canvas_suggest::catalog::{option_desc, CatalogParam, CatalogTemplate, Lang};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Row {
    key: String,
    name_ru: String,
    name_en: String,
    description: String,
    description_en: String,
    category: String,
    params: Vec<ParamRow>,
    option_ru: String,
    option_en: String,
}

#[derive(Debug, Deserialize)]
struct ParamRow {
    name: String,
    unit: Option<String>,
    #[serde(rename = "type")]
    ptype: Option<String>,
}

fn load_rows() -> Vec<Row> {
    let path = common::fixtures_dir().join("catalog_options.json");
    let text = std::fs::read_to_string(&path).expect("catalog_options.json читается");
    serde_json::from_str(&text).expect("разбор catalog_options.json")
}

fn to_template(r: &Row) -> CatalogTemplate {
    CatalogTemplate {
        key: r.key.clone(),
        name_ru: if r.name_ru.is_empty() {
            None
        } else {
            Some(r.name_ru.clone())
        },
        name_en: r.name_en.clone(),
        description: r.description.clone(),
        description_en: if r.description_en.is_empty() {
            None
        } else {
            Some(r.description_en.clone())
        },
        category: r.category.clone(),
        params: r
            .params
            .iter()
            .map(|p| CatalogParam {
                name: p.name.clone(),
                unit: p.unit.clone(),
                ptype: p.ptype.clone(),
            })
            .collect(),
    }
}

#[test]
fn catalog_options_bitexact_both_langs() {
    let rows = load_rows();
    assert_eq!(rows.len(), 62, "62 шаблона каталога PoC");
    let mut mismatches = 0;
    for row in &rows {
        let t = to_template(row);
        let ru = option_desc(&t, Lang::Ru);
        let en = option_desc(&t, Lang::En);
        if ru.desc != row.option_ru {
            mismatches += 1;
            eprintln!(
                "ru расхождение [{}]:\n  py: {}\n  rs: {}",
                row.key, row.option_ru, ru.desc
            );
        }
        if en.desc != row.option_en {
            mismatches += 1;
            eprintln!(
                "en расхождение [{}]:\n  py: {}\n  rs: {}",
                row.key, row.option_en, en.desc
            );
        }
        assert_eq!(ru.id, row.key);
        assert_eq!(en.id, row.key);
    }
    assert_eq!(mismatches, 0, "побитовая совместимость template_option");
}

#[test]
fn catalog_ids_unique_and_sorted() {
    let rows = load_rows();
    let mut keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
    keys.sort_unstable();
    let n = keys.len();
    keys.dedup();
    assert_eq!(keys.len(), n, "ключи каталога уникальны");
}
