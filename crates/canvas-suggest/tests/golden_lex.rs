//! Golden-гейт lex-движков: побитовое воспроизведение замороженных чисел
//! волны 1 (lex v1) и волны 3 H9 (lex v2) на eval-сете PoC.
//!
//! CI-регресс FR-079 §Verification: lex v1 p@1 ≥ 0.38 pool / ≥ 0.319 test.

mod common;

use canvas_suggest::lex::LexEngine;
use canvas_suggest::lex_v2::{name_words, LexEngineV2};
use canvas_suggest::types::{OptionDesc, SuggestContext, SuggestEngine};
use common::{main_filter, Hits};
use std::collections::{BTreeSet, HashMap};

fn to_options(f: &common::Fixture) -> Vec<OptionDesc> {
    f.options
        .iter()
        .map(|o| OptionDesc::new(&o.id, &o.desc))
        .collect()
}

#[test]
fn lex_v1_reproduces_frozen_wave1_numbers() {
    let fxs = common::load_evalset();
    let main: Vec<&common::Fixture> = fxs.iter().filter(|f| main_filter(f)).collect();
    assert_eq!(main.len(), 139, "main-фильтр: пул (test 47 / dev 92)");

    let engine = LexEngine::default();
    let mut hits = Hits::new();
    for f in &main {
        let opts = to_options(f);
        let ranked = engine.rank(&SuggestContext::new(&f.context, &opts));
        let hit = ranked.first().map(|s| s.id.as_str()) == f.golden.choice.as_deref();
        hits.add(&f.split, hit);
    }
    // Замороженные числа волны 1 (отчёт §5.1): 0.3191 / 0.4130 / 0.3813
    assert_eq!(
        (hits.test, hits.dev, hits.pool),
        (15, 38, 53),
        "lex v1: хиты test/dev/pool — побитовое воспроизведение порта"
    );
    // CI-гейт FR-079
    assert!(
        f64::from(hits.pool) / main.len() as f64 >= 0.38,
        "гейт: p@1 pool ≥ 0.38"
    );
    assert!(
        f64::from(hits.test) / 47.0 >= 0.319,
        "гейт: p@1 test ≥ 0.319"
    );
}

#[test]
fn lex_v2_reproduces_frozen_wave3_test() {
    let fxs = common::load_evalset();
    let main: Vec<&common::Fixture> = fxs.iter().filter(|f| main_filter(f)).collect();

    // каталог имён (источник: корпус шаблонов волны 1, 62 манифеста)
    let path = common::fixtures_dir().join("catalog_names.json");
    let catalog: Vec<CatalogName> =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("catalog_names.json читается"))
            .expect("разбор catalog_names.json");
    assert_eq!(catalog.len(), 62);
    let names: HashMap<String, BTreeSet<String>> = catalog
        .iter()
        .map(|c| (c.key.clone(), name_words(&c.name_ru, &c.name_en, &c.key)))
        .collect();

    let engine = LexEngineV2::new(names);
    let mut hits = Hits::new();
    for f in &main {
        let opts = to_options(f);
        let ranked = engine.rank(&SuggestContext::new(&f.context, &opts));
        let hit = ranked.first().map(|s| s.id.as_str()) == f.golden.choice.as_deref();
        hits.add(&f.split, hit);
    }
    // Замороженное число волны 3 H9: test 0.4043 (+8.5 п.п. к v1)
    assert_eq!(hits.test, 19, "lex v2: хиты test — реконструкция H9");
    // dev/pool: формула восстановлена по test-совпадению; ожидание ≤ отчёта
    // (в отчёте ablation-dev 0.4239 / pool 0.4173 — код волны 3 утерян)
    assert_eq!(
        (hits.dev, hits.pool),
        (37, 56),
        "lex v2: реконструированные dev/pool"
    );
}

#[derive(serde::Deserialize)]
struct CatalogName {
    key: String,
    name_ru: String,
    name_en: String,
}
