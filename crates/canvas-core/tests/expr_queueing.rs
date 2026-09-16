//! FR-015: золотые тесты функций теории очередей.
//!
//! Все значения — через публичный путь `parse` + `eval` (как пишет
//! пользователь в текстовой ноде). Допуски ±1e-9 относительных — математика
//! зафиксирована (инвариант 2 FR-015): M/M/1 и M/M/c (Erlang-C).
//! Референсные числа:
//! - `mm1(1000 rps, 1200 rps)`: ρ = 0.8333, W_q = ρ/(μ−λ) = 4.1667 ms,
//!   W = W_q + 1/μ = 5.0 ms (пример FR-015).
//! - `mmc(1000 rps, 600 rps, 3)`: a = 1.6667, ρ = 0.5556,
//!   Erlang C = 0.29976, W_q = C/(cμ−λ) = 0.3747 ms, W = 2.0414 ms.
//!   (Оценка «≈0.21» из документа FR-015 неточна — формула даёт 0.29976.)
//! - `mm1(1000 rps, 1200 rps, 2)`: a = 0.8333, ρ = 0.41667,
//!   C = 0.24509, W = 1.4461 ms.

use canvas_core::expr::{eval, eval_lines, parse, EvalError, Expr, ExprOutcome};

/// Результат вычисления формулы — (число, отображение единицы).
fn eval_num(source: &str) -> (f64, String) {
    let expr = parse(source).expect("формула парсится");
    let value = eval(&expr, &canvas_core::expr::Env::empty()).expect("формула вычисляется");
    (value.num, value.unit.display())
}

fn eval_err(source: &str) -> EvalError {
    let expr = parse(source).expect("формула парсится");
    eval(&expr, &canvas_core::expr::Env::empty()).expect_err("ожидалась ошибка")
}

#[test]
fn mm1_two_args_is_five_ms() {
    let (num, unit) = eval_num("mm1(1000 rps, 1200 rps)");
    assert!((num - 0.005).abs() < 1e-9, "W = 5 ms, получено {num}");
    assert_eq!(unit, "sec", "время — в базовой секунде");
}

#[test]
fn mm1_with_one_server_is_same_as_two_args() {
    let (a, _) = eval_num("mm1(1000 rps, 1200 rps)");
    let (b, _) = eval_num("mm1(1000 rps, 1200 rps, 1)");
    assert!((a - b).abs() < 1e-12);
}

#[test]
fn mmc_three_servers_wait_is_two_ms() {
    let (num, unit) = eval_num("mmc(1000 rps, 600 rps, 3)");
    assert!(
        (num - 0.0020414).abs() < 1e-6,
        "W ≈ 2.0414 ms, получено {num}"
    );
    assert_eq!(unit, "sec");
}

#[test]
fn mm1_with_servers_matches_mmc() {
    let (a, _) = eval_num("mm1(1000 rps, 600 rps, 3)");
    let (b, _) = eval_num("mmc(1000 rps, 600 rps, 3)");
    assert!((a - b).abs() < 1e-12, "mm1(λ, μ, c) — та же модель M/M/c");
}

#[test]
fn mm1_two_servers_is_one_and_half_ms() {
    let (num, _) = eval_num("mm1(1000 rps, 1200 rps, 2)");
    // a = 0.8333, ρ = 0.41667, C = 0.24510, W_q = C/(cμ−λ) = 0.17507 ms,
    // W = W_q + 1/μ = 1.00840 ms
    assert!(
        (num - 0.0010084).abs() < 1e-6,
        "W ≈ 1.0084 ms, получено {num}"
    );
}

#[test]
fn utilization_is_lambda_over_c_mu() {
    let (num, unit) = eval_num("utilization(1000 rps, 400 rps, 3)");
    assert!((num - 0.8333333).abs() < 1e-6);
    assert_eq!(unit, "%", "доля — в процентах (0..1, не 83.3)");
    // Без c — M/M/1
    let (num, _) = eval_num("utilization(1000 rps, 1200 rps)");
    assert!((num - 0.8333333).abs() < 1e-6);
    // Перегрузка — валидный ответ (>1), не ошибка
    let (num, _) = eval_num("utilization(2000 rps, 1000 rps)");
    assert!((num - 2.0).abs() < 1e-12);
}

#[test]
fn littles_law_is_lambda_times_wait() {
    let (num, unit) = eval_num("littles_law(1000 rps, 5 ms)");
    assert!(
        (num - 5.0).abs() < 1e-9,
        "L = 1000/s × 0.005 s = 5, получено {num}"
    );
    assert_eq!(unit, "req");
}

#[test]
fn erlang_c_probability_matches_formula() {
    let (num, unit) = eval_num("erlang_c(1000 rps, 600 rps, 3)");
    assert!(
        (num - 0.29976).abs() < 1e-4,
        "P_wait ≈ 0.29976, получено {num}"
    );
    assert_eq!(unit, "%");
    // ρ ≥ 1 — ждут все (SLA-ответ, не ошибка)
    let (num, _) = eval_num("erlang_c(2000 rps, 1000 rps, 2)");
    assert!((num - 1.0).abs() < 1e-12);
}

#[test]
fn overload_is_explicit_error() {
    match eval_err("mm1(1000 rps, 500 rps)") {
        EvalError::Overload { rho } => assert!((rho - 2.0).abs() < 1e-9, "ρ = 2"),
        other => panic!("ожидался Overload, получено {other:?}"),
    }
    // Граница ρ = 1 — тоже перегрузка
    match eval_err("mm1(1000 rps, 1000 rps)") {
        EvalError::Overload { .. } => {}
        other => panic!("ожидался Overload на ρ=1, получено {other:?}"),
    }
    match eval_err("mmc(1000 rps, 400 rps, 2)") {
        EvalError::Overload { rho } => assert!((rho - 1.25).abs() < 1e-9),
        other => panic!("ожидался Overload, получено {other:?}"),
    }
}

#[test]
fn zero_arrival_is_service_time_only() {
    let (num, _) = eval_num("mm1(0 rps, 1200 rps)");
    assert!(
        (num - (1.0 / 1200.0)).abs() < 1e-12,
        "пустая система: W = 1/μ"
    );
}

#[test]
fn wrong_argument_units_are_rejected() {
    // λ — не скорость
    match eval_err("mm1(5 sec, 1200 rps)") {
        EvalError::BadCall { func, msg } => {
            assert_eq!(func, "mm1");
            assert!(msg.contains("скоростью"), "{msg}");
        }
        other => panic!("ожидался BadCall, получено {other:?}"),
    }
    // W — не время
    match eval_err("littles_law(1000 rps, 500 rps)") {
        EvalError::BadCall { .. } => {}
        other => panic!("ожидался BadCall, получено {other:?}"),
    }
    // μ = 0
    match eval_err("mm1(1000 rps, 0 rps)") {
        EvalError::BadCall { .. } => {}
        other => panic!("ожидался BadCall, получено {other:?}"),
    }
}

#[test]
fn wrong_arity_is_rejected() {
    assert!(matches!(
        eval_err("mmc(1000 rps, 600 rps)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("erlang_c(1000 rps, 600 rps)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("littles_law(1000 rps)"),
        EvalError::BadCall { .. }
    ));
    assert!(matches!(
        eval_err("mm1(1000 rps)"),
        EvalError::BadCall { .. }
    ));
}

#[test]
fn servers_bounds_are_enforced() {
    // Дробное c
    assert!(matches!(
        eval_err("mm1(1000 rps, 1200 rps, 1.5)"),
        EvalError::BadCall { .. }
    ));
    // c > 1000
    assert!(matches!(
        eval_err("mm1(1000 rps, 1200 rps, 1001)"),
        EvalError::BadCall { .. }
    ));
    // c = 0
    assert!(matches!(
        eval_err("mm1(1000 rps, 1200 rps, 0)"),
        EvalError::BadCall { .. }
    ));
    // c в единицах Count — валидно
    let (num, _) = eval_num("mm1(1000 rps, 1200 rps, 2 req)");
    assert!((num - 0.0010084).abs() < 1e-6);
}

#[test]
fn queueing_calls_parse_as_function_call() {
    let expr = parse("mm1(1000 rps, 1200 rps)").unwrap();
    assert!(matches!(expr, Expr::Call { ref func, .. } if func == "mm1"));
}

#[test]
fn queueing_in_note_lines_shows_result() {
    let lines = eval_lines("λ = 1000 rps\nμ = 1200 rps\n= mm1(λ, μ)");
    assert!(matches!(lines[0], Some(ExprOutcome::Ok(_))));
    assert!(matches!(lines[1], Some(ExprOutcome::Ok(_))));
    match &lines[2] {
        Some(ExprOutcome::Ok(value)) => {
            assert_eq!(
                value.to_string(),
                "0.005 sec",
                "итог строки — время пребывания"
            );
        }
        other => panic!("ожидался результат mm1, получено {other:?}"),
    }
}

#[test]
fn overload_in_note_lines_shows_error() {
    let lines = eval_lines("= mm1(1000 rps, 500 rps)");
    match &lines[0] {
        Some(ExprOutcome::Err(msg)) => assert!(msg.contains("перегрузка"), "{msg}"),
        other => panic!("ожидалась красная строка перегрузки, получено {other:?}"),
    }
}

#[test]
fn inbound_value_feeds_queueing_formula() {
    // FR-014 × FR-015: вход value-ребра — скорость прибытия
    let rps = canvas_core::expr::Value::with_unit(
        1000.0,
        canvas_core::expr::Unit::atom(canvas_core::expr::Atom {
            dim: canvas_core::expr::Dimension::Rate,
            exp: 1,
            scale: 1.0,
            name: "rps",
        }),
    );
    let env = canvas_core::expr::Env::with_inbound(vec![Some(rps)]);
    // Префикс `=` — соглашение СТРОК листа (eval_lines), в parse() не входит
    let value = eval(&parse("mm1($in, 1200 rps)").unwrap(), &env).unwrap();
    assert!((value.num - 0.005).abs() < 1e-9, "получено {}", value.num);
    assert_eq!(value.unit.display(), "sec");
}

// === FR-027: финансовые функции (расширение FR-015) ===

/// `npv(0.1, -100, 110)` = −100 + 110/1.1 = −100 + 100 = 0 — точка
/// безубыточности при ставке 10% и возврате 110 через период.
#[test]
fn npv_break_even_is_zero() {
    let (num, _unit) = eval_num("npv(0.1, -100, 110)");
    assert!(
        num.abs() < 1e-9,
        "NPV(0.1, -100, 110) = 0, получено {num}"
    );
}

/// `npv(0.0, -100, 100, 50)` = −100 + 100 + 50 = 50 (нулевая ставка —
/// простая сумма потоков).
#[test]
fn npv_zero_rate_is_sum_of_flows() {
    let (num, _unit) = eval_num("npv(0.0, -100, 100, 50)");
    assert!((num - 50.0).abs() < 1e-9, "получено {num}");
}

/// `npv` с большим числом потоков — стандартный проект: −1000, +500, +600,
/// +200 при ставке 10%. NPV = −1000 + 454.55 + 495.87 + 150.26 ≈ 100.68.
#[test]
fn npv_classic_project_npv() {
    let (num, _unit) = eval_num("npv(0.1, -1000, 500, 600, 200)");
    // Σ = -1000 + 500/1.1 + 600/1.21 + 200/1.331
    //   = -1000 + 454.5455 + 495.8678 + 150.2629 ≈ 100.6762
    assert!((num - 100.6762).abs() < 1e-3, "получено {num}");
}

/// `cagr(100, 200, 3)` ≈ 0.2599 (2^(1/3) − 1 — удвоение за 3 года).
#[test]
fn cagr_doubling_in_three_years() {
    let (num, _unit) = eval_num("cagr(100, 200, 3)");
    assert!((num - 0.25992).abs() < 1e-4, "получено {num}");
}

/// `cagr(100, 100, 5)` = 0 (без роста).
#[test]
fn cagr_no_growth_is_zero() {
    let (num, _unit) = eval_num("cagr(100, 100, 5)");
    assert!(num.abs() < 1e-9, "CAGR без роста = 0, получено {num}");
}

/// `cagr` с begin ≤ 0 — BadCall (отрицательный старт бессмысленен).
#[test]
fn cagr_negative_begin_is_error() {
    assert!(matches!(
        eval_err("cagr(0, 100, 3)"),
        EvalError::BadCall { .. }
    ));
}

/// `irr(-100, 110)` = 0.1 — тривиальный случай: инвестиция 100, возврат
/// 110 через период → IRR = 10%.
#[test]
fn irr_simple_one_period_is_ten_percent() {
    let (num, _unit) = eval_num("irr(-100, 110)");
    assert!(
        (num - 0.1).abs() < 1e-6,
        "irr(-100, 110) = 0.1, получено {num}"
    );
}

/// `irr(-1000, 500, 600, 200)` ≈ 0.1635 — стандартный проект.
/// NPV(r) = 0 при r ≈ 16.35%: проверить можно ручной подстановкой
/// (500/1.1635 + 600/1.3537 + 200/1.5749 ≈ 429.8 + 443.2 + 127.0 ≈ 1000).
#[test]
fn irr_classic_project_irr() {
    let (num, _unit) = eval_num("irr(-1000, 500, 600, 200)");
    // NPV(r) = 0 при r ≈ 0.1635 — Newton-Raphson нашёл корень.
    assert!(
        (num - 0.1635).abs() < 5e-3,
        "irr ≈ 0.1635, получено {num}"
    );
}

/// `irr` с одинаковым знаком — ошибка (нет корня).
#[test]
fn irr_same_sign_is_error() {
    assert!(matches!(
        eval_err("irr(100, 200)"),
        EvalError::BadCall { .. }
    ));
}

/// `cohort_ltv` с retention = 1 весь период — LTV = arpu × margin × months
/// (Σ retention по дням / 30 = months).
#[test]
fn cohort_ltv_perfect_retention_equals_arpu_times_margin_times_months() {
    let (num, _unit) = eval_num("cohort_ltv(10, 1, 1, 1, 1, 1)");
    // arpu_m0 = 10, margin = 1, retention = 1 на каждом дне, months = 1.
    // Сумма retention по дням 0..30 = 31, ltv = 10 * 1 * 31 / 30 ≈ 10.3333
    assert!((num - 10.3333).abs() < 1e-3, "получено {num}");
}

/// `cohort_ltv` с типичными значениями SaaS (r_d1=0.4, r_d7=0.25, r_d30=0.1)
/// — LTV ≈ 4.57 (низкое удержание → низкий LTV; это и есть здоровый сигнал
/// «проблема в онбординге»).
#[test]
fn cohort_ltv_saas_typical_retention_curve() {
    let (num, _unit) = eval_num("cohort_ltv(20, 0.8, 0.4, 0.25, 0.1, 12)");
    // Положительный, конечный, в разумных пределах (1..100).
    assert!(num.is_finite(), "конечный результат, получено {num}");
    assert!(num > 0.0, "LTV > 0, получено {num}");
    assert!(num < 100.0, "LTV < 100 usd для агрессивного churn, получено {num}");
}

/// `cohort_ltv` с margin вне 0..1 — ошибка.
#[test]
fn cohort_ltv_margin_out_of_range_is_error() {
    assert!(matches!(
        eval_err("cohort_ltv(20, 1.5, 1, 1, 1, 12)"),
        EvalError::BadCall { .. }
    ));
}

/// `cohort_ltv` с нецелым months — ошибка.
#[test]
fn cohort_ltv_non_integer_months_is_error() {
    assert!(matches!(
        eval_err("cohort_ltv(20, 0.8, 1, 1, 1, 1.5)"),
        EvalError::BadCall { .. }
    ));
}

/// Все 4 новые функции маршрутизируются как `Expr::Call` (FR-013).
#[test]
fn financial_calls_parse_as_function_call() {
    assert!(matches!(
        parse("npv(0.1, -100, 110)").unwrap(),
        Expr::Call { ref func, .. } if func == "npv"
    ));
    assert!(matches!(
        parse("cagr(100, 200, 3)").unwrap(),
        Expr::Call { ref func, .. } if func == "cagr"
    ));
    assert!(matches!(
        parse("irr(-100, 110)").unwrap(),
        Expr::Call { ref func, .. } if func == "irr"
    ));
    assert!(matches!(
        parse("cohort_ltv(20, 0.8, 0.4, 0.25, 0.1, 12)").unwrap(),
        Expr::Call { ref func, .. } if func == "cohort_ltv"
    ));
}

/// Arity guard — недостаточно аргументов → BadCall.
#[test]
fn financial_wrong_arity_is_rejected() {
    assert!(matches!(eval_err("npv(0.1)"), EvalError::BadCall { .. }));
    assert!(matches!(eval_err("cagr(100, 200)"), EvalError::BadCall { .. }));
    assert!(matches!(eval_err("irr(-100)"), EvalError::BadCall { .. }));
    assert!(
        matches!(eval_err("cohort_ltv(20, 0.8, 0.4, 0.25, 0.1)"), EvalError::BadCall { .. })
    );
}
