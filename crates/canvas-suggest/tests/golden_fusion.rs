//! Golden-гейт гибрида: fusion(lex v1, mm; α=0.85, min-max) на замороженных
//! mm-пробах волны 1 — воспроизведение главной находки волны 2 (§15.1):
//! p@1 test 0.468 против 0.319 у чистой лексики.
//!
//! CI-регресс FR-079 §Verification: fusion ≥ 0.46 test-old.

mod common;

use canvas_suggest::fusion::fuse;
use canvas_suggest::lex::LexEngine;
use common::{main_filter, Hits};

#[test]
fn fusion_reproduces_frozen_wave2_numbers() {
    let fxs = common::load_evalset();
    let mm = common::load_mm();
    let main: Vec<&common::Fixture> = fxs.iter().filter(|f| main_filter(f)).collect();
    assert_eq!(main.len(), 139);

    let engine = LexEngine::default();
    let alpha = 0.85; // dev-fit волны 2–3
    let mut hits = Hits::new();
    let mut n_none_wins: u32 = 0;
    for f in &main {
        let opts: Vec<canvas_suggest::types::OptionDesc> = f
            .options
            .iter()
            .map(|o| canvas_suggest::types::OptionDesc::new(&o.id, &o.desc))
            .collect();
        let lex = engine.scores(&f.context, &opts);
        let probs = mm[f.id.as_str()]
            .probs
            .as_ref()
            .unwrap_or_else(|| panic!("mm-проба без probs для main-фикстуры {}", f.id));
        let mm_scores: Vec<(String, f64)> = probs.iter().map(|(k, v)| (k.clone(), *v)).collect();
        let fused = fuse(&lex, &mm_scores, alpha);
        let top = fused.first().expect("fusion непуста");
        if top.id == "none" {
            n_none_wins += 1;
        }
        let hit = top.id == *f.golden.choice.as_deref().unwrap_or("");
        hits.add(&f.split, hit);
    }
    // Замороженные числа волны 2: test 0.4681 / dev 0.4239 / pool 0.4388
    assert_eq!(
        (hits.test, hits.dev, hits.pool),
        (22, 39, 61),
        "fusion α=0.85: хиты test/dev/pool — побитовое воспроизведение"
    );
    // CI-гейт FR-079: p@1 test ≥ 0.46
    assert!(f64::from(hits.test) / 47.0 >= 0.46, "гейт: p@1 test ≥ 0.46");
    // none-зона участвует в ранжировании (кандидат из mm), но не доминирует
    assert!(
        n_none_wins < 10,
        "none не должен вытеснять каталог: побед {n_none_wins}"
    );
}
