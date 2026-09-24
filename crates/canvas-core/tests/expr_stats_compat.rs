//! FR-063: обратная совместимость БЕЗ фичи `stats` — зеркальный гейт
//! `expr_stats.rs` (там `#![cfg(feature = "stats")]`).
//!
//! Контракт FR-063 «Контракты на стыках»: без фичи stats-имена НЕ
//! маршрутизируются — `eval` даёт `UnknownFunction` (graceful-деградация:
//! `.canvas` с формулой `normal_quantile(...)` открывается без фичи, нода
//! помечается ошибкой, остальной поток значений считается штатно — no
//! panic, no abort). Zero-dep инвариант: без фичи deps (statrs/rand/…)
//! в граф не попадают.
#![cfg(not(feature = "stats"))]

use canvas_core::expr::{eval, parse, Env, EvalError};

/// Каждое stats-имя без фичи — UnknownFunction (не паника, не BadCall).
#[test]
fn stats_functions_degrade_to_unknown_function_without_feature() {
    for source in [
        "normal_quantile(0.975, 0, 1)",
        "normal_cdf(1.96, 0, 1)",
        "lognormal_quantile(0.5, 0, 1)",
        "exp_quantile(0.632, 1)",
        "poisson_pmf(3, 2)",
        "triangular_quantile(0.5, 0, 1, 0.5)",
        "triangular(0.5, 0, 1, 0.5)",
        "ci_mean(100, 15, 100, 0.95)",
        "normal_sample(0, 1, 100, 42)",
        "lognormal_sample(0, 0.5, 100, 42)",
    ] {
        let expr = parse(source).expect("грамматика не зависит от фичи");
        match eval(&expr, &Env::empty()) {
            Err(EvalError::UnknownFunction(name)) => {
                let expected = source.split('(').next().unwrap_or(source);
                assert_eq!(name, expected, "{source}: имя ошибки не совпало");
            }
            other => panic!("{source}: ожидалась UnknownFunction, получено {other:?}"),
        }
    }
}

/// Остальной поток не задет: встроенные и queueing-функции считаются
/// штатно в той же сборке (fallback-arm не тронут).
#[test]
fn builtin_and_queueing_still_work_without_stats_feature() {
    let expr = parse("sum(1, 2)").unwrap();
    assert_eq!(eval(&expr, &Env::empty()).unwrap().num, 3.0);
    let expr = parse("mm1(1000 rps, 1200 rps)").unwrap();
    let value = eval(&expr, &Env::empty()).unwrap();
    assert!((value.num - 0.005).abs() < 1e-9, "W = 5 ms");
    let expr = parse("npv(0.1, -100, 110)").unwrap();
    assert!(eval(&expr, &Env::empty()).unwrap().num.abs() < 1e-9);
}
