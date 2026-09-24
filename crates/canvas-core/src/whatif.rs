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
//!
//! FR-064 P2 (FR-017 v2): freeze/сравнение — заморозка активного сценария
//! в `Arc<FlowSolutions>`-снимок ([`FrozenScenario`], [`freeze_scenario`])
//! и диф снимков по `outputs`/`lines` ([`compare_scenarios`], таблица
//! «переменная | База | С1 | С2»). Имена замороженных персистентны рядом
//! со сценариями: `canvasdesk.whatif.frozen` (тот же паттерн хранения;
//! сами снимки — runtime, восстанавливаются пересчётом персистентного
//! канваса при загрузке). MCP `whatif_*` не расширяются (FR-064).

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::expr;
use crate::flow::{self, CycleError, FlowSolutions, WhatIfOverrides};
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

/// Запись сценариев в `canvas.extra["canvasdesk"]["whatif"]` (ключ
/// `scenarios`; FR-064 P2: соседний ключ `frozen` сохраняется). Чужие поля
/// `canvasdesk` сохраняются; пустой список УДАЛЯЕТ ключ (и пустые
/// контейнеры) — файл без сценариев байт-в-байт как раньше (round-trip
/// чистый, инвариант 5).
pub fn scenarios_to_canvas(canvas: &mut Canvas, scenarios: &[Scenario]) {
    let value = if scenarios.is_empty() {
        None
    } else {
        Some(whatif_value(scenarios))
    };
    set_whatif_key(canvas, "scenarios", value);
}

/// FR-064 P2: имена замороженных сценариев из
/// `canvas.extra["canvasdesk"]["whatif"]["frozen"]` (порядок заморозки).
/// Толерантно: мусор/отсутствие поля — пустой список.
pub fn frozen_from_canvas(canvas: &Canvas) -> Vec<String> {
    canvas
        .extra
        .get("canvasdesk")
        .and_then(|ext| ext.get("whatif"))
        .and_then(|whatif| whatif.get("frozen"))
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// FR-064 P2: запись имён замороженных (ключ `frozen`; соседний
/// `scenarios` сохраняется). Пустой список удаляет ключ — файл без
/// заморозок байт-в-байт как раньше.
pub fn frozen_to_canvas(canvas: &mut Canvas, names: &[String]) {
    let value = if names.is_empty() {
        None
    } else {
        Some(Value::Array(
            names
                .iter()
                .map(|name| Value::String(name.clone()))
                .collect(),
        ))
    };
    set_whatif_key(canvas, "frozen", value);
}

/// FR-064 P2: аккуратно обновить один ключ объекта
/// `canvasdesk.whatif` (scenarios/frozen — соседи не затираются;
/// чужие поля `canvasdesk` сохраняются). `value: None` — удалить ключ;
/// опустевший `whatif`/`canvasdesk` удаляется целиком (round-trip чистый).
fn set_whatif_key(canvas: &mut Canvas, key: &str, value: Option<Value>) {
    // Читаем ТЕКУЩИЙ объект whatif (или создаём при записи значения).
    let existing = canvas
        .extra
        .get("canvasdesk")
        .and_then(Value::as_object)
        .and_then(|ext| ext.get("whatif"))
        .and_then(Value::as_object)
        .cloned();
    let Some(value) = value else {
        // Удаление: только если объект существует.
        let Some(mut whatif) = existing else {
            return;
        };
        whatif.remove(key);
        if whatif.is_empty() {
            // whatif опустел — удалить его (и пустой canvasdesk).
            remove_whatif_object(canvas);
        } else {
            // Записать обратно с удалённым ключом (соседи сохраняются).
            if let Some(Value::Object(ext)) = canvas.extra.get_mut("canvasdesk") {
                ext.insert("whatif".to_owned(), Value::Object(whatif));
            }
        }
        return;
    };
    let mut whatif = existing.unwrap_or_default();
    whatif.insert(key.to_owned(), value);
    // canvasdesk-объект: существующий (чужие поля сохраняются) или новый.
    let mut ext = canvas
        .extra
        .get("canvasdesk")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    ext.insert("whatif".to_owned(), Value::Object(whatif));
    canvas
        .extra
        .insert("canvasdesk".to_owned(), Value::Object(ext));
}

/// FR-064 P2: удалить опустевший объект `whatif` (и `canvasdesk`, если
/// он тоже опустел — инвариант 5: пустых контейнеров не остаётся).
fn remove_whatif_object(canvas: &mut Canvas) {
    let Some(Value::Object(ext)) = canvas.extra.get_mut("canvasdesk") else {
        return;
    };
    ext.remove("whatif");
    if ext.is_empty() {
        canvas.extra.remove("canvasdesk");
    }
}

/// JSON-представление массива сценариев (ключ `scenarios` объекта
/// `whatif`); сортировка подмен — детерминизм.
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
    Value::Array(list)
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

// --- FR-064 P2 (FR-017 v2): freeze/сравнение -------------------------------

/// Замороженный снимок сценария: pinned значения потока на момент
/// заморозки (правки канваса их не двигают — снимок за Arc). Имя — имя
/// сценария (или «База»); персистентно только ИМЯ (`canvasdesk.whatif.frozen`),
/// сами значения — runtime (восстанавливаются пересчётом при загрузке).
#[derive(Debug, Clone)]
pub struct FrozenScenario {
    pub name: String,
    pub solutions: Arc<FlowSolutions>,
}

/// Одна строка таблицы сравнения сценариев «переменная | База | С1 | С2».
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonRow {
    /// Переменная: id ноды.
    pub node: String,
    /// `Some(i)` — построчный выход (строка i); `None` — узловой итог.
    pub line: Option<usize>,
    /// Значения по колонкам [База, С1, С2, …]; `None` — нет значения
    /// (ошибка/отсутствие — ячейка «—»).
    pub values: Vec<Option<expr::Value>>,
    /// Дельты против базы (формат [`expr::whatif_delta_str`]), индексы
    /// совпадают с `values`; у колонки «База» — всегда `None`.
    pub deltas: Vec<Option<String>>,
}

/// Результат сравнения ([`compare_scenarios`]): колонки сценариев
/// (без «Базы») и строки — union построчных переменных + изменившиеся
/// узловые итоги (дельты downstream).
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioComparison {
    /// Имена колонок-сценариев в порядке заморозки/передачи.
    pub columns: Vec<String>,
    /// Построчные переменные (union ключей), затем узловые итоги,
    /// изменившиеся хотя бы в одной колонке против базы; обе группы —
    /// в отсортированном порядке (детерминизм).
    pub rows: Vec<ComparisonRow>,
}

/// FR-064 P2: заморозить сценарий — пересчёт канваса с его активными
/// (непротухшими) подменами → снимок. Детерминирован: тот же канвас и
/// сценарий дают побитово одинаковый снимок (инвариант 2 FR-050).
pub fn freeze_scenario(canvas: &Canvas, scenario: &Scenario) -> Result<FrozenScenario, CycleError> {
    let overrides = WhatIfOverrides {
        line_exprs: active_line_exprs(canvas, scenario),
        node_values: HashMap::new(),
    };
    let solutions = Arc::new(flow::propagate_with_lines(canvas, &overrides)?);
    Ok(FrozenScenario {
        name: scenario.name.clone(),
        solutions,
    })
}

/// FR-064 P2: сравнить снимки двух и более сценариев с базой — диф
/// `lines` и `outputs`. `line_keys` — union построчных переменных
/// (обычно union активных подмен — задаёт вызывающий); узловые итоги
/// добавляются автоматически, если хотя бы одна колонка отличается от
/// базы (дельты downstream). Формат дельт — [`expr::whatif_delta_str`].
pub fn compare_scenarios(
    base: &FlowSolutions,
    frozen: &[FrozenScenario],
    line_keys: &[(String, usize)],
) -> ScenarioComparison {
    let columns: Vec<String> = frozen.iter().map(|f| f.name.clone()).collect();
    let mut rows: Vec<ComparisonRow> = Vec::new();
    // 1) Построчные переменные (union override-ключей, отсортированы).
    let mut keys: Vec<(String, usize)> = line_keys.to_vec();
    keys.sort();
    keys.dedup();
    for (node, line) in keys {
        let key = (node.clone(), line);
        let base_value = base.lines.get(&key);
        let mut values = vec![base_value.cloned()];
        let mut deltas = vec![None];
        for snapshot in frozen {
            let value = snapshot.solutions.lines.get(&key);
            let delta = match (base_value, value) {
                (Some(base), Some(value)) => expr::whatif_delta_str(base, value),
                _ => None,
            };
            values.push(value.cloned());
            deltas.push(delta);
        }
        rows.push(ComparisonRow {
            node,
            line: Some(line),
            values,
            deltas,
        });
    }
    // 2) Узловые итоги, изменившиеся хотя бы в одной колонке против базы
    // (дельты downstream — эталон ADR-0006 №2: смена rps видна ниже).
    // Сравнение — по Ok-значениям (ошибка в колонке против значения базы —
    // тоже изменение).
    let mut nodes: Vec<String> = frozen
        .iter()
        .flat_map(|snapshot| snapshot.solutions.outputs.keys().cloned())
        .chain(base.outputs.keys().cloned())
        .collect();
    nodes.sort();
    nodes.dedup();
    for node in nodes {
        let base_out = base.outputs.get(&node).and_then(|r| r.as_ref().ok());
        let changed = frozen.iter().any(|snapshot| {
            let col = snapshot
                .solutions
                .outputs
                .get(&node)
                .and_then(|r| r.as_ref().ok());
            match (base_out, col) {
                (Some(base), Some(col)) => base != col,
                (None, None) => false,
                _ => true,
            }
        });
        if !changed {
            continue;
        }
        let mut values = vec![base_out.cloned()];
        let mut deltas = vec![None];
        for snapshot in frozen {
            let col = snapshot
                .solutions
                .outputs
                .get(&node)
                .and_then(|r| r.as_ref().ok());
            let delta = match (base_out, col) {
                (Some(base), Some(col)) => expr::whatif_delta_str(base, col),
                _ => None,
            };
            values.push(col.cloned());
            deltas.push(delta);
        }
        rows.push(ComparisonRow {
            node,
            line: None,
            values,
            deltas,
        });
    }
    ScenarioComparison { columns, rows }
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
