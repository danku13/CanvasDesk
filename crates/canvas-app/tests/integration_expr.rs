//! Интеграционные тесты FR-013: Numi-формулы text-нод (`canvasdesk.expr`).
//!
//! Инварианты FR-013 на уровне библиотеки:
//! 1. чистый парсер `expr::parse` — без I/O и глобального состояния;
//! 2. чистый eval `expr::eval` — `(Expr, &Env) -> Result<Value, EvalError>`;
//! 3. сериализация `canvasdesk.expr` через `extra` — round-trip без потерь
//!    (полная проверка в `json_canvas_io.rs`, здесь — сквозной сценарий);
//! 4. runtime-результат не в `.canvas` — модель хранит только формулу.
//!
//! Сценарий владельца: ландшафт сервиса — gateway/queue/replica — параметры
//! «человеческим языком», результат виден на карточке. Сценарии с окном
//! (MCP `node_edit` + `expr_results` + undo) — в `main.rs` mod tests
//! (mcp_dispatch — приватная функция бинарного крейта).

use std::str::FromStr;

use canvas_core::expr::{self, Env, ExprOutcome};
use canvas_core::{Canvas, Node, NodeKind};

/// Сценарий верификации FR-013: параметры ноды в стиле Numi —
/// переменные + единицы, итог в единице первого операнда.
#[test]
fn service_landscape_formula_evaluates() {
    // Gateway: rps = 1000, latency = 50 ms, cpu = latency × rps / replicas
    let program = "rps = 1000\nlatency = 50 ms\nreplicas = 3\ncpu = latency × rps / replicas";
    let parsed = expr::parse(program).expect("программа разбирается");
    let value = expr::eval(&parsed, &Env::empty()).expect("вычисляется");
    // Результат программы — значение последнего утверждения (cpu);
    // replicas — скалярная переменная, итог — в ms
    assert_eq!(value.to_string(), "16\u{a0}666.7 ms");
    assert!((value.num - 16666.666666666668).abs() < 1e-6);
}

/// Перцентиля и агрегаты (v1-функции) на значениях одной размерности.
#[test]
fn latency_percentiles() {
    let parsed = expr::parse(
        "p50 = percentile(50, 10 ms, 20 ms, 30 ms, 40 ms)\np90 = percentile(90, 10 ms, 20 ms, 30 ms, 40 ms)\navg(10 ms, 20 ms, 30 ms)",
    )
    .expect("разбирается");
    let value = expr::eval(&parsed, &Env::empty()).expect("считается");
    assert_eq!(value.to_string(), "20 ms");
    // Проверка p90 отдельно (интерполяция 30 + 0.7×10)
    let p90 = expr::eval(
        &expr::parse("percentile(90, 10 ms, 20 ms, 30 ms, 40 ms)").unwrap(),
        &Env::empty(),
    )
    .unwrap();
    assert!((p90.num - 37.0).abs() < 1e-9, "p90 = {}", p90);
}

/// Инвариант 4 (сквозной): формула в `.canvas`, результата в модели нет —
/// результат вычисляется приложением и живёт только в runtime-карте.
#[test]
fn formula_serializes_result_does_not() {
    let mut canvas = Canvas::default();
    let mut gateway = Node::text("gateway", "Gateway\n= 1k rps", 0.0, 0.0);
    gateway.width = 320.0;
    canvas.nodes.push(gateway);
    // Формула выведена из «=»-строки текста (семантика смешанного редактора)
    let text = canvas.nodes[0].text.clone().unwrap();
    let formula: String = text
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix('='))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    canvas.nodes[0].set_expr(Some(formula));

    // Round-trip: формула в файле, вычисленного значения нет
    let json = canvas.to_json().expect("сериализация");
    let restored = Canvas::from_str(&json).expect("парсинг");
    assert_eq!(restored.nodes[0].kind(), NodeKind::Text);
    assert_eq!(restored.nodes[0].expr(), Some("1k rps"));
    assert!(
        !json.contains("1000 rps"),
        "вычисленный результат не должен попадать в .canvas"
    );

    // Приложение вычисляет при загрузке (SceneState::new → recompute_all_expr);
    // здесь — тот же вызов expr::eval над сохранённой формулой
    let parsed = expr::parse(restored.nodes[0].expr().expect("формула есть")).expect("парсинг");
    let outcome = match expr::eval(&parsed, &Env::empty()) {
        Ok(value) => ExprOutcome::Ok(value),
        Err(err) => ExprOutcome::Err(err.to_string()),
    };
    match outcome {
        ExprOutcome::Ok(value) => assert_eq!(value.to_string(), "1\u{a0}000 rps"),
        other => panic!("ожидалось значение: {other:?}"),
    }
}

/// Env: предзаполненные переменные видны формуле — точка расширения
/// FR-014 (поток значений по рёбрам: `$in` придёт через Env).
#[test]
fn env_seeding_is_the_fr014_hook() {
    let env = Env::empty().set("in", expr::Value::scalar(500.0));
    let value = expr::eval(&expr::parse("in × 2 ms").unwrap(), &env).unwrap();
    assert_eq!(value.to_string(), "1\u{a0}000 ms");
    assert!((value.num - 1000.0).abs() < 1e-9);
}
