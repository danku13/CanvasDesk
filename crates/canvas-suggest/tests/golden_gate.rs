//! Golden-гейт show-гейта: правило длины/соседей на show-фикстурах волны 1
//! (33 позитивов источника А + негативы синтетики = 39).
//!
//! CI-регресс FR-079 §Verification: acc ≥ 0.90 (фактически 37/39 = 0.949).

mod common;

use canvas_suggest::gate::{should_show, DEFAULT_MIN_CTX_LEN};

#[test]
fn show_gate_reproduces_frozen_accuracy() {
    let fxs = common::load_evalset();
    let shows: Vec<&common::Fixture> = fxs.iter().filter(|f| f.golden.show.is_some()).collect();
    assert_eq!(shows.len(), 39, "show-фикстур волны 1");

    let mut ok = 0u32;
    let mut pos = 0u32;
    for f in &shows {
        let expected = f.golden.show.unwrap();
        if expected {
            pos += 1;
        }
        if should_show(&f.context, DEFAULT_MIN_CTX_LEN) == expected {
            ok += 1;
        }
    }
    assert_eq!(
        pos, 34,
        "позитивов (волнa 1: 33 A + 1 synth… сверено с данными)"
    );
    assert_eq!(
        ok, 37,
        "show-гейт: точность 37/39 = 0.949 (заморозка волны 1)"
    );
    assert!(
        f64::from(ok) / shows.len() as f64 >= 0.90,
        "гейт: acc ≥ 0.90"
    );
}
