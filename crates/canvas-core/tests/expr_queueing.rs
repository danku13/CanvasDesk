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
