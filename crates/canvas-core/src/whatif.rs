//! FR-017 (CP6, волна B2): what-if сценарии — именованные наборы
//! построчных подмен (expr-подмена исходников строк, гипотезы Q1/Q2).
//!
//! Сценарии персистентны в `.canvas`: canvas-level extra
//! `canvasdesk.whatif: { scenarios: [{ name, overrides: [{node, line, expr}] }] }`
//! (гипотеза Q11 — сценарии едут с канвасом, round-trip с Obsidian
//! сохраняется через `Canvas.extra` flatten). Runtime-подмены активного
//! сценария применяются через [`crate::flow::WhatIfOverrides`] — модуль
//! только хранение, сериализация и деградация протухших подмен
//! (инвариант 5 FR-017).

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::model::Canvas;

/// Именованный what-if сценарий (FR-017 Фаза B): набор построчных подмен.
/// Ключи совпадают по схеме с [`crate::flow::WhatIfOverrides::line_exprs`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scenario {
    pub name: String,
    /// Построчные подмены: (id ноды, индекс строки ТЕКСТА) → новый исходник.
    pub line_exprs: HashMap<(String, usize), String>,
}

/// Протухшая подмена сценария (инвариант 5, гипотеза Q5c): нода удалена,
/// строка удалена или стала прозой. Деградация — тихий пропуск слота
/// в пересчёте + видимый маркер в списке overrides панели.
#[derive(Debug, Clone, PartialEq)]
pub struct StaleOverride {
    pub node: String,
    pub line: usize,
    pub reason: String,
}

/// Чтение сценариев из `canvas.extra["canvasdesk"]["whatif"]`. Толерантно:
/// мусор/отсутствие поля — пустой список (старые `.canvas` работают).
pub fn scenarios_from_canvas(canvas: &Canvas) -> Vec<Scenario> {
    let Some(list) = canvas
        .extra
        .get("canvasdesk")
        .and_then(|ext| ext.get("whatif"))
        .and_then(|whatif| whatif.get("scenarios"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?.to_owned();
            let mut line_exprs = HashMap::new();
            if let Some(overrides) = entry.get("overrides").and_then(Value::as_array) {
                for item in overrides {
                    let node = item.get("node").and_then(Value::as_str)?.to_owned();
                    let line = item.get("line").and_then(Value::as_u64)? as usize;
                    let expr = item.get("expr").and_then(Value::as_str)?.to_owned();
                    line_exprs.insert((node, line), expr);
                }
            }
            Some(Scenario { name, line_exprs })
        })
        .collect()
}

/// Запись сценариев в `canvas.extra["canvasdesk"]["whatif"]`. Чужие поля
/// `canvasdesk` сохраняются; пустой список УДАЛЯЕТ поле (и пустой
/// контейнер) — файл без сценариев байт-в-байт как раньше (round-trip
/// чистый, инвариант 5).
pub fn scenarios_to_canvas(canvas: &mut Canvas, scenarios: &[Scenario]) {
    let Some(ext) = canvas
        .extra
        .get_mut("canvasdesk")
        .and_then(Value::as_object_mut)
    else {
        if !scenarios.is_empty() {
            let mut ext = Map::new();
            ext.insert("whatif".to_owned(), whatif_value(scenarios));
            canvas
                .extra
                .insert("canvasdesk".to_owned(), Value::Object(ext));
        }
        return;
    };
    if scenarios.is_empty() {
        ext.remove("whatif");
        if ext.is_empty() {
            canvas.extra.remove("canvasdesk");
        }
        return;
    }
    ext.insert("whatif".to_owned(), whatif_value(scenarios));
}

/// JSON-представление блока `whatif`.
fn whatif_value(scenarios: &[Scenario]) -> Value {
    let list: Vec<Value> = scenarios
        .iter()
        .map(|scenario| {
            let mut overrides: Vec<(&usize, &String, &String)> = scenario
                .line_exprs
                .iter()
                .map(|((node, line), expr)| (line, node, expr))
                .collect();
            overrides.sort();
            serde_json::json!({
                "name": scenario.name,
                "overrides": overrides.iter().map(|(line, node, expr)| serde_json::json!({
                    "node": node, "line": line, "expr": expr,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::json!({ "scenarios": list })
}

/// Валидация сценария против канваса: протухшие подмены (нода удалена,
/// строка вне текста, строка стала прозой). Валидные подмены не возвращаются.
pub fn validate_scenario(canvas: &Canvas, scenario: &Scenario) -> Vec<StaleOverride> {
    let mut stale = Vec::new();
    let mut entries: Vec<(&(String, usize), &String)> = scenario.line_exprs.iter().collect();
    entries.sort_by_key(|((_, line), _)| *line);
    for ((node, line), _) in entries {
        let Some(target) = canvas.node(node) else {
            stale.push(StaleOverride {
                node: node.clone(),
                line: *line,
                reason: "нода удалена".to_owned(),
            });
            continue;
        };
        let text = target.text.as_deref().unwrap_or_default();
        let Some(current) = text.split('\n').nth(*line) else {
            stale.push(StaleOverride {
                node: node.clone(),
                line: *line,
                reason: "строка удалена".to_owned(),
            });
            continue;
        };
        let trimmed = current.trim();
        let looks_formula = trimmed.starts_with('=') || trimmed.contains('=');
        // PRD-0007 X3 (AC-4.3): листом дерева может быть и числовая/
        // выраженческая константа без «=» («620», «1 240», «-5 %») —
        // строка жива, если движок даёт ей результат (тот же детектор
        // рода строки, что у формул: проза и код-фенсы не считаются).
        let evaluates = crate::expr::eval_lines(current)
            .into_iter()
            .next()
            .flatten()
            .is_some();
        if trimmed.is_empty() || (!looks_formula && !evaluates) {
            stale.push(StaleOverride {
                node: node.clone(),
                line: *line,
                reason: "строка стала прозой".to_owned(),
            });
        }
    }
    stale
}

/// Активные (непротухшие) подмены сценария — то, что передаётся
/// в [`crate::flow::WhatIfOverrides`] (протухшие тихо пропускаются,
/// симметрия политики FR-025).
pub fn active_line_exprs(canvas: &Canvas, scenario: &Scenario) -> HashMap<(String, usize), String> {
    let stale = validate_scenario(canvas, scenario);
    scenario
        .line_exprs
        .iter()
        .filter(|((node, line), _)| {
            !stale
                .iter()
                .any(|entry| &entry.node == node && entry.line == *line)
        })
        .map(|(key, expr)| (key.clone(), expr.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Node;

    fn scene() -> Canvas {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("a", "rps = 1000", 0.0, 0.0));
        canvas.nodes.push(Node::text("b", "lat = 50 ms", 1.0, 0.0));
        canvas
            .nodes
            .push(Node::text("c", "просто текст без формул", 2.0, 0.0));
        canvas
    }

    fn sample() -> Vec<Scenario> {
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("a".to_owned(), 0), "rps = 2000".to_owned());
        vec![
            Scenario {
                name: "Рост ×2".to_owned(),
                line_exprs: line_exprs.clone(),
            },
            Scenario {
                name: "Пустой".to_owned(),
                line_exprs: HashMap::new(),
            },
        ]
    }

    /// Инвариант 5: без сценариев — поле не появляется (round-trip чистый).
    #[test]
    fn empty_scenarios_leave_canvas_untouched() {
        let mut canvas = scene();
        scenarios_to_canvas(&mut canvas, &[]);
        assert!(canvas.extra.is_empty(), "поле whatif не создаётся");
        scenarios_to_canvas(&mut canvas, &sample());
        assert!(!canvas.extra.is_empty());
        scenarios_to_canvas(&mut canvas, &[]);
        assert!(canvas.extra.is_empty(), "пустой список удаляет поле");
    }

    /// Инвариант 5: сценарии переживают serialize → deserialize.
    #[test]
    fn scenarios_round_trip() {
        let mut canvas = scene();
        scenarios_to_canvas(&mut canvas, &sample());
        let json = serde_json::to_string(&canvas).expect("сериализация");
        let restored: Canvas = serde_json::from_str(&json).expect("десериализация");
        let scenarios = scenarios_from_canvas(&restored);
        assert_eq!(scenarios, sample(), "сценарии восстановлены");
    }

    /// Чужие поля `canvasdesk` верхнего уровня не теряются.
    #[test]
    fn foreign_canvasdesk_fields_survive() {
        let mut canvas = scene();
        canvas
            .extra
            .insert("canvasdesk".to_owned(), serde_json::json!({ "foreign": 1 }));
        scenarios_to_canvas(&mut canvas, &sample());
        assert_eq!(
            canvas.extra["canvasdesk"]["foreign"],
            serde_json::json!(1),
            "чужое поле сохранено"
        );
        scenarios_to_canvas(&mut canvas, &[]);
        assert_eq!(
            canvas.extra["canvasdesk"]["foreign"],
            serde_json::json!(1),
            "контейнер не удалён при живом чужом поле"
        );
    }

    /// Старый файл без `canvasdesk.whatif` — пустой список сценариев.
    #[test]
    fn missing_whatif_field_reads_empty() {
        let canvas = scene();
        assert!(scenarios_from_canvas(&canvas).is_empty());
    }

    /// validate_scenario: протухшие подмены диагностируются по каждому
    /// сценарию деградации; валидные молчат.
    #[test]
    fn validate_marks_stale_overrides() {
        let canvas = scene();
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("a".to_owned(), 0), "rps = 2000".to_owned());
        line_exprs.insert(("b".to_owned(), 5), "lat = 10 ms".to_owned());
        line_exprs.insert(("ghost".to_owned(), 0), "x = 1".to_owned());
        line_exprs.insert(("c".to_owned(), 0), "c = 1".to_owned());
        let scenario = Scenario {
            name: "S".to_owned(),
            line_exprs,
        };
        let stale = validate_scenario(&canvas, &scenario);
        assert_eq!(stale.len(), 3, "три протухшие подмены: {stale:?}");
        assert!(stale.iter().any(|s| s.node == "b" && s.line == 5));
        assert!(stale.iter().any(|s| s.node == "ghost"));
        assert!(stale
            .iter()
            .any(|s| s.node == "c" && s.line == 0 && s.reason.contains("прозой")));
        // active_line_exprs отфильтровывает протухшие, валидная жива
        let active = active_line_exprs(&canvas, &scenario);
        assert_eq!(active.len(), 1);
        assert!(active.contains_key(&("a".to_owned(), 0)));
    }

    /// PRD-0007 X3 (AC-4.3): листом дерева бывает константа без «=»
    /// («620», «1 240 ₽») — подмена такой строки НЕ протухает; проза
    /// и пустые строки по-прежнему протухают (инвариант 5 FR-017).
    #[test]
    fn numeric_constant_lines_stay_valid() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text(
            "sheet",
            "Аренда = 620\n620\n-5 %\nпросто проза\n```js\ncode();\n```",
            0.0,
            0.0,
        ));
        let mut line_exprs = HashMap::new();
        // Строка 1 — «620» (константа без «=»), строка 2 — с единицей.
        for (line, expr) in [(1usize, "700"), (2usize, "-8 %"), (3usize, "проза")] {
            line_exprs.insert(("sheet".to_owned(), line), expr.to_owned());
        }
        let scenario = Scenario {
            name: "S".to_owned(),
            line_exprs,
        };
        let stale = validate_scenario(&canvas, &scenario);
        assert_eq!(
            stale.iter().filter(|s| s.line == 3).count(),
            1,
            "проза протухает: {stale:?}"
        );
        assert!(
            stale.iter().all(|s| s.line != 1 && s.line != 2),
            "числовые константы живы: {stale:?}"
        );
        let active = active_line_exprs(&canvas, &scenario);
        assert_eq!(active.len(), 2, "живые: строки 1 и 2");
    }
}
