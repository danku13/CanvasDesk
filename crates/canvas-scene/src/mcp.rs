//! MCP-инструменты канваса (FR-037/ADR-0012, перенос из
//! canvas-app/main.rs): `mcp_dispatch` — чистая функция над SceneState
//! (28 инструментов), хелперы параметров, FR-033 graph_apply (атомарная
//! батч-композиция). Протокольный мост (stdio/JSON-RPC) — крейт
//! canvas-mcp; инструменты не знают о транспорте.

use std::collections::{BTreeMap, HashMap};

use canvas_core::analyze::{self, AnalysisConfig};
use canvas_core::expr;
use canvas_core::flow::{self, FlowKind};
use canvas_core::{Canvas, Edge, Node, Side, SpatialIndex};

use crate::measure::fit_template_node_height;
use crate::scene::{
    next_free_id, spill_edge_value, split_formula_lines, whatif_delta_str, SceneState, MAX_ZOOM,
    MIN_ZOOM,
};

/// Дефолт размеров файловой ноды MCP `node_create_file` (синхронно с
/// canvas_app::ui::DROP_CARD_W/H — паритет-тест в canvas-app).
pub const DEFAULT_FILE_CARD_W: f32 = 320.0;
pub const DEFAULT_FILE_CARD_H: f32 = 220.0;

/// Распаковка MCP-конверта, пришедшего по pipe: `tools/call` несёт имя
/// инструмента и аргументы внутри params (`name`/`arguments`) — посредник
/// форвардит конверт как есть; прочие методы проходят без изменений.
/// Чистая функция — тестируется без pipe.
pub fn mcp_unwrap_call(method: &str, params: &serde_json::Value) -> (String, serde_json::Value) {
    if method == "tools/call" {
        let name = params
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .unwrap_or_default();
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        (name, args)
    } else {
        (method.to_owned(), params.clone())
    }
}

/// Обязательный строковый параметр MCP-инструмента.
fn mcp_req_str<'v>(params: &'v serde_json::Value, name: &str) -> Result<&'v str, String> {
    params
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("отсутствует параметр '{name}'"))
}

/// MCP-текст ноды (node_create_note / node_update_text / node_edit):
/// нормализация literal-эскейпов — ИИ-агенты передают многострочный текст
/// последовательностями `\n` (два символа), принимаем их как реальные
/// переводы строк (контракт `canvas_core::mcp_text::normalize_escapes`).
fn mcp_node_text(params: &serde_json::Value, name: &str) -> Option<String> {
    params
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(canvas_core::mcp_text::normalize_escapes)
}

/// Обязательный числовой параметр MCP-инструмента.
fn mcp_req_f32(params: &serde_json::Value, name: &str) -> Result<f32, String> {
    params
        .get(name)
        .and_then(serde_json::Value::as_f64)
        .map(|value| value as f32)
        .ok_or_else(|| format!("отсутствует числовой параметр '{name}'"))
}

/// Опциональный числовой параметр MCP-инструмента.
fn mcp_opt_f32(params: &serde_json::Value, name: &str) -> Option<f32> {
    params
        .get(name)
        .and_then(serde_json::Value::as_f64)
        .map(|value| value as f32)
}

/// Индекс ноды по строковому id (MCP-инструменты адресуют ноды id).
fn mcp_node_index(canvas: &Canvas, id: &str) -> Result<usize, String> {
    canvas
        .nodes
        .iter()
        .position(|node| node.id == id)
        .ok_or_else(|| format!("нода не найдена: {id}"))
}

/// Сводка ноды для списков; поле text — только по запросу (can be большим).
fn mcp_node_summary(node: &Node, with_text: bool) -> serde_json::Value {
    let mut value = serde_json::json!({
        "id": node.id,
        "type": node.node_type,
        "x": node.x,
        "y": node.y,
        "width": node.width,
        "height": node.height,
        "label": node.label,
        "file": node.file,
        "color": node.color,
        // FR-013: Numi-формула (null — calc-режим выключен)
        "expr": node.expr(),
    });
    if with_text {
        value["text"] = serde_json::Value::from(node.text.clone());
    }
    value
}

/// Сторона связи MCP: "any"/отсутствие → None (автовывод из геометрии).
fn mcp_side(params: &serde_json::Value, name: &str) -> Result<Option<Side>, String> {
    match params.get(name).and_then(serde_json::Value::as_str) {
        None | Some("any") => Ok(None),
        Some(text) => serde_json::from_value(serde_json::Value::String(text.to_owned()))
            .map(Some)
            .map_err(|_| format!("неверная сторона '{name}': {text}")),
    }
}

/// FR-032: каноническая схема ребра для `edges_list`/`edge_get` — агент
/// восстанавливает топологию графа (CR-013 G4). FR-029 (CP1): при влитии
/// полей `toParam`/`fromOutput` добавить их сюда же (опциональные, как
/// fromLine) — обе ветки читения используют только эту функцию.
fn mcp_edge_json(edge: &Edge) -> serde_json::Value {
    let mut value = serde_json::json!({
        "id": edge.id,
        "from": edge.from_node,
        "to": edge.to_node,
        // FR-014: тип потока ("value"/"control") — ключ топологии для агента
        "kind": edge.flow_kind().as_str(),
        "fromSide": edge.from_side,
        "toSide": edge.to_side,
    });
    if let Some(line) = edge.from_line {
        value["fromLine"] = serde_json::json!(line);
    }
    // FR-029: адресация портов — именованный исток и проливание в параметр
    if let Some(name) = &edge.from_output {
        value["fromOutput"] = serde_json::json!(name);
    }
    if let Some(name) = &edge.to_param {
        value["toParam"] = serde_json::json!(name);
    }
    value
}

/// FR-017: одна изменившаяся точка графа (подменённая строка или итог
/// ноды, пересчитанный каскадом).
#[derive(Debug, Clone)]
pub(crate) struct WhatIfDeltaRow {
    node: String,
    /// None — узловое значение ноды; Some(i) — строка i текста.
    line: Option<usize>,
    base: Option<String>,
    whatif: Option<String>,
    delta: Option<String>,
}

impl WhatIfDeltaRow {
    fn to_json(&self) -> serde_json::Value {
        let mut entry = serde_json::json!({ "node": self.node });
        if let Some(line) = self.line {
            entry["line"] = serde_json::json!(line);
        }
        if let Some(base) = &self.base {
            entry["base"] = serde_json::json!(base);
        }
        if let Some(whatif) = &self.whatif {
            entry["whatif"] = serde_json::json!(whatif);
        }
        if let Some(delta) = &self.delta {
            entry["delta"] = serde_json::json!(delta);
        }
        entry
    }
}

/// FR-017: дельты активного сценария против базы — те же пары «было →
/// стало», что видит пользователь на канвасе (инвариант 6: MCP-видимость
/// эквивалентна UI).
pub(crate) fn whatif_delta_rows(scene: &SceneState) -> Vec<WhatIfDeltaRow> {
    let mut rows = Vec::new();
    let Some(index) = scene.active_scenario else {
        return rows;
    };
    let Some(scenario) = scene.scenarios.get(index) else {
        return rows;
    };
    // Подменённые строки: сравнение построчных значений base vs active
    let mut keys: Vec<&(String, usize)> = scenario.line_exprs.keys().collect();
    keys.sort();
    for (node, line) in keys {
        let base = scene.flow_baseline.lines.get(&(node.clone(), *line));
        let whatif = scene.flow_active.lines.get(&(node.clone(), *line));
        if let (Some(base), Some(whatif)) = (base, whatif) {
            rows.push(WhatIfDeltaRow {
                node: node.clone(),
                line: Some(*line),
                base: Some(base.to_string()),
                whatif: Some(whatif.to_string()),
                delta: whatif_delta_str(base, whatif),
            });
        }
    }
    // Итоги нод, пересчитанные каскадом (downstream по value-рёбрам)
    let mut node_ids: Vec<&String> = scene.flow_active.outputs.keys().collect();
    node_ids.sort();
    for id in node_ids {
        let base = scene
            .flow_baseline
            .outputs
            .get(id)
            .and_then(|r| r.as_ref().ok());
        let whatif = scene
            .flow_active
            .outputs
            .get(id)
            .and_then(|r| r.as_ref().ok());
        if let (Some(base), Some(whatif)) = (base, whatif) {
            if whatif_delta_str(base, whatif).is_some() {
                rows.push(WhatIfDeltaRow {
                    node: id.clone(),
                    line: None,
                    base: Some(base.to_string()),
                    whatif: Some(whatif.to_string()),
                    delta: whatif_delta_str(base, whatif),
                });
            }
        }
    }
    rows
}

/// Род узла lineage-дерева — стабильные строки MCP-контракта (PRD-0007 §6.6,
/// окно проверки показывает те же роды).
fn lineage_kind_str(kind: canvas_core::LineageNodeKind) -> &'static str {
    use canvas_core::LineageNodeKind as K;
    match kind {
        K::Calc => "calc",
        K::Leaf => "leaf",
        K::Cycle => "cycle",
        K::Unmapped => "unmapped",
        K::Unlinked => "unlinked",
        K::Truncated => "truncated",
    }
}

/// Выполнить MCP-инструмент над сценой: 25 инструментов канваса
/// (tools/list — в canvas-mcp). Чистая функция над SceneState
/// (viewport_get/set — viewport-зеркало в SceneState, ADR-0012; кламп
/// зума — как у Camera) — тестируется без окна и pipe; каждая мутирующая
/// ветка обновляет spatial index и помечает канвас грязным (автосейв).
/// Ошибки — строки, посредник заворачивает их в isError.
pub fn mcp_dispatch(
    scene: &mut SceneState,
    templates: &canvas_core::templates::TemplateRegistry,
    method: &str,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match method {
        "canvas_info" => Ok(serde_json::json!({
            "path": scene.path.to_string_lossy(),
            "nodes": scene.canvas.nodes.len(),
            "edges": scene.canvas.edges.len(),
        })),
        "nodes_list" => {
            let with_text = params
                .get("text")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            let nodes = scene
                .canvas
                .nodes
                .iter()
                .map(|node| mcp_node_summary(node, with_text))
                .collect();
            Ok(serde_json::Value::Array(nodes))
        }
        "node_get" => {
            let id = mcp_req_str(params, "id")?;
            let node = scene
                .canvas
                .node(id)
                .ok_or_else(|| format!("нода не найдена: {id}"))?;
            serde_json::to_value(node).map_err(|err| err.to_string())
        }
        "nodes_search" => {
            let query = mcp_req_str(params, "query")?.to_lowercase();
            let nodes = scene
                .canvas
                .nodes
                .iter()
                .filter(|node| {
                    [
                        node.text.as_deref(),
                        node.label.as_deref(),
                        node.file.as_deref(),
                    ]
                    .into_iter()
                    .any(|field| field.is_some_and(|text| text.to_lowercase().contains(&query)))
                })
                .map(|node| mcp_node_summary(node, true))
                .collect();
            Ok(serde_json::Value::Array(nodes))
        }
        "node_create_note" => {
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            let text = mcp_node_text(params, "text").unwrap_or_default();
            let mut node = Node::text(next_free_id(&scene.canvas, "note"), &text, x, y);
            if let Some(width) = mcp_opt_f32(params, "width") {
                node.width = width;
            }
            if let Some(height) = mcp_opt_f32(params, "height") {
                node.height = height;
            }
            // FR-013: строки «= …» в тексте — формула
            node.set_expr(split_formula_lines(&text));
            let index = scene.canvas.nodes.len();
            // FR-006: MCP-мутация — undo-шаг (после валидации, до вставки)
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes.push(node);
            scene.spatial.insert(index, &scene.canvas.nodes[index]);
            scene.mark_dirty();
            // FR-014: живой пересчёт потока после создания expr-ноды
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": scene.canvas.nodes[index].id }))
        }
        "node_create_file" => {
            let path = mcp_req_str(params, "path")?;
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            // Файл на диске НЕ создаём — только карточка в модели
            let node = Node::file(
                next_free_id(&scene.canvas, "file"),
                path,
                x,
                y,
                mcp_opt_f32(params, "width").unwrap_or(DEFAULT_FILE_CARD_W),
                mcp_opt_f32(params, "height").unwrap_or(DEFAULT_FILE_CARD_H),
            );
            let index = scene.canvas.nodes.len();
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes.push(node);
            scene.spatial.insert(index, &scene.canvas.nodes[index]);
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": scene.canvas.nodes[index].id }))
        }
        "node_update_text" => {
            let id = mcp_req_str(params, "id")?;
            let text = mcp_node_text(params, "text")
                .ok_or_else(|| "отсутствует параметр 'text'".to_owned())?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes[index].text = Some(text.clone());
            // FR-013: строки «= …» в тексте — формула (единая семантика
            // с редактором); формула нет — сброс
            scene.canvas.nodes[index].set_expr(split_formula_lines(&text));
            scene.mark_dirty();
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": id }))
        }
        // FR-005: редактирование ноды одним вызовом — обновляются ТОЛЬКО
        // переданные поля; label/color/expr = null — сброс; геометрия — с
        // обновлением spatial index; ответ — сводка с текстом
        "node_edit" => {
            let id = mcp_req_str(params, "id")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-013: expr валидируется ПЕРЕД undo-шагом — при ошибке
            // парсинга нода не меняется вовсе (isError с диагностикой)
            let expr_update = match params.get("expr") {
                None => None,
                Some(serde_json::Value::Null) => Some(None),
                Some(serde_json::Value::String(formula)) => match expr::parse(formula) {
                    Ok(_) => Some(Some(formula.clone())),
                    Err(err) => return Err(format!("expr: {err}")),
                },
                Some(other) => return Err(format!("expr должен быть строкой или null: {other}")),
            };
            let mut geometry = false;
            // FR-006: MCP-мутация — undo-шаг. Пушим до мутаций: валидация
            // отдельных полей переплетена с применением остальных (частичные
            // применения при Err тоже должны быть отменяемы)
            scene.push_undo(scene.canvas.clone());
            if let Some(text) = mcp_node_text(params, "text") {
                scene.canvas.nodes[index].text = Some(text);
                // CR-012: ленивый резерв футера под переносы нового текста
                // (двухуровневый refit: оценка-ворота → измерение; formula_lines
                // ensure_reserve_at берёт из текущих expr_line_results).
                // expr здесь не пересчитывается — правило футера по текущим
                // expr_results/expr_line_results; полный пересчёт — в ветке
                // expr ниже (recompute_flow поднимет резерв сам).
                scene.ensure_reserve_at(index);
            }
            match params.get("label") {
                None => {}
                Some(serde_json::Value::Null) => scene.canvas.nodes[index].label = None,
                Some(serde_json::Value::String(label)) => {
                    scene.canvas.nodes[index].label = Some(label.clone());
                }
                Some(other) => return Err(format!("label должен быть строкой или null: {other}")),
            }
            if params.get("color").is_some() {
                let color = match params
                    .get("color")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null)
                {
                    serde_json::Value::Null => None,
                    serde_json::Value::String(preset)
                        if matches!(preset.as_str(), "1" | "2" | "3" | "4" | "5" | "6") =>
                    {
                        Some(preset)
                    }
                    other => {
                        return Err(format!(
                            "color должен быть пресетом \"1\"..\"6\" или null, получено {other}"
                        ));
                    }
                };
                scene.canvas.nodes[index].color = color;
            }
            if let Some(x) = mcp_opt_f32(params, "x") {
                scene.canvas.nodes[index].x = x;
                geometry = true;
            }
            if let Some(y) = mcp_opt_f32(params, "y") {
                scene.canvas.nodes[index].y = y;
                geometry = true;
            }
            for (name, field) in [("width", 0), ("height", 1)] {
                if let Some(value) = mcp_opt_f32(params, name) {
                    if value <= 0.0 {
                        return Err(format!("{name} должен быть > 0, получено {value}"));
                    }
                    if field == 0 {
                        scene.canvas.nodes[index].width = value;
                    } else {
                        scene.canvas.nodes[index].height = value;
                    }
                    geometry = true;
                }
            }
            if geometry {
                let node = &scene.canvas.nodes[index];
                scene.spatial.update(index, node);
            }
            // FR-013: формула уже провалидирована — применяем и пересчитываем;
            // FR-014: живой пересчёт downstream
            if let Some(new_expr) = expr_update {
                scene.canvas.nodes[index].set_expr(new_expr);
                scene.mark_dirty();
                scene.recompute_flow();
            }
            scene.mark_dirty();
            let node = &scene.canvas.nodes[index];
            Ok(mcp_node_summary(node, true))
        }
        "node_move" => {
            let id = mcp_req_str(params, "id")?;
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            scene.move_node(index, x, y);
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": id }))
        }
        "node_resize" => {
            let id = mcp_req_str(params, "id")?;
            let width = mcp_req_f32(params, "width")?;
            let height = mcp_req_f32(params, "height")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            let node = &mut scene.canvas.nodes[index];
            node.width = width;
            node.height = height;
            scene.spatial.update(index, node);
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": id }))
        }
        "node_delete" => {
            let id = mcp_req_str(params, "id")?;
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг
            scene.push_undo(scene.canvas.clone());
            let removed = scene
                .canvas
                .remove_node(index)
                .ok_or_else(|| format!("нода не найдена: {id}"))?;
            // Индексы сдвинулись — spatial перестраивается (паттерн delete_selected).
            // UI-выделение (selected/selected_nodes/dragging) чистит App в
            // on_mcp_wake после успешного node_delete (ADR-0012: UI-поля — в App)
            scene.spatial = SpatialIndex::build(&scene.canvas);
            scene.mark_dirty();
            // FR-014: downstream удалённой ноды — «вход отсутствует»
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": removed.id }))
        }
        "node_set_color" => {
            let id = mcp_req_str(params, "id")?;
            let color = match params
                .get("color")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
            {
                serde_json::Value::Null => None,
                serde_json::Value::String(preset)
                    if matches!(preset.as_str(), "1" | "2" | "3" | "4" | "5" | "6") =>
                {
                    Some(preset)
                }
                other => {
                    return Err(format!(
                        "color должен быть пресетом \"1\"..\"6\" или null, получено {other}"
                    ));
                }
            };
            let index = mcp_node_index(&scene.canvas, id)?;
            // FR-006: MCP-мутация — undo-шаг (валидация цвета прошла выше)
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes[index].color = color;
            scene.mark_dirty();
            Ok(serde_json::json!({ "id": id }))
        }
        "edge_create" => {
            let from = mcp_req_str(params, "from")?.to_owned();
            let to = mcp_req_str(params, "to")?.to_owned();
            let from_index = mcp_node_index(&scene.canvas, &from)?;
            let to_index = mcp_node_index(&scene.canvas, &to)?;
            // FR-029 v2: адресация портов. kind — "value" включает поток
            // значений сразу (раньше требовался flow_set_kind); дефолт
            // "control" — визуальная связь (обратная совместимость).
            let kind = match params.get("kind").and_then(serde_json::Value::as_str) {
                None | Some("control") => FlowKind::Control,
                Some("value") => FlowKind::Value,
                Some(other) => {
                    return Err(format!(
                        "kind должен быть \"value\" или \"control\", получено {other:?}"
                    ))
                }
            };
            // Взаимное исключение fromLine/fromOutput — ошибка схемы
            let from_line = match params.get("fromLine") {
                None => None,
                Some(value) => {
                    let line = value.as_u64().ok_or_else(|| {
                        "fromLine должен быть целым ≥ 0 (индексом строки)".to_owned()
                    })?;
                    Some(line as usize)
                }
            };
            let from_output = params
                .get("fromOutput")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            if from_line.is_some() && from_output.is_some() {
                return Err(
                    "fromLine и fromOutput взаимно исключаются: адресуй строку ИЛИ именованный выход"
                        .to_owned(),
                );
            }
            let to_param = params
                .get("toParam")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            if to_param.is_some() && kind != FlowKind::Value {
                return Err(
                    "toParam адресует параметр шаблонной ноды: требуется kind = \"value\""
                        .to_owned(),
                );
            }
            // Валидация имён по снапшотам (FR-029): неизвестное имя — ошибка
            // вызова, агент не молчит. Исток: шаблонная нода — секция
            // outputs снапшота; текстовая — переменные Numi-листа
            // (присваивания `имя = …`, чтение по списку params_from_text).
            if let Some(name) = &from_output {
                let source = &scene.canvas.nodes[from_index];
                let known = match source.template() {
                    Some(tpl) => tpl.outputs.iter().any(|spec| &spec.name == name),
                    None => canvas_core::templates::params_from_text(
                        &source.text.clone().unwrap_or_default(),
                    )
                    .contains_key(name),
                };
                if !known {
                    return Err(format!(
                        "нода {from} не имеет выхода {name:?}: у шаблонной — секция outputs, у текстовой — переменная Numi-листа"
                    ));
                }
            }
            // Приёмник: toParam — только параметр шаблонной ноды
            if let Some(name) = &to_param {
                let target = &scene.canvas.nodes[to_index];
                match target.template() {
                    Some(tpl) if tpl.params.contains_key(name) => {}
                    Some(_) => {
                        return Err(format!(
                            "нода {to} не имеет параметра {name:?}: доступные — {}",
                            target
                                .template()
                                .map(|tpl| tpl
                                    .params
                                    .keys()
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", "))
                                .unwrap_or_default()
                        ))
                    }
                    None => {
                        return Err(format!(
                            "toParam адресует параметры шаблонных нод: нода {to} — текстовая (позиционные слоты $1..$N без адресации)"
                        ))
                    }
                }
            }
            // FR-050 Н4 (fail-fast дубль-входов): второе value-ребро в тот же
            // `toParam` отклоняется немедленно — не ждём `graph_validate`
            // (замена источника — явная пара edge_delete + edge_create; UI —
            // диалог «Заменить источник?», этап C). Легаси-файлы с дублями
            // грузятся с warning «последний побеждает» (как сегодня).
            if let Some(param) = &to_param {
                if let Some(existing) = scene.canvas.edges.iter().find(|edge| {
                    edge.flow_kind() == FlowKind::Value
                        && edge.to_node == to
                        && edge.to_param.as_deref() == Some(param.as_str())
                }) {
                    return Err(format!(
                        "E-DOUBLE-INPUT: параметр {param:?} ноды {to} уже запитан value-ребром {} — замена: edge_delete {} + edge_create (или один graph_apply)",
                        existing.id, existing.id
                    ));
                }
            }
            let mut edge = Edge::new(
                scene.canvas.next_edge_id(),
                &from,
                mcp_side(params, "fromSide")?,
                &to,
                mcp_side(params, "toSide")?,
            );
            edge.set_flow_kind(kind);
            edge.from_line = from_line;
            edge.from_output = from_output;
            edge.to_param = to_param;
            // FR-014: value-ребро не замыкает цикл (DAG-инвариант)
            if kind == FlowKind::Value
                && canvas_core::creates_value_cycle(&scene.canvas, &from, &to)
            {
                let participants = canvas_core::value_path(&scene.canvas, &to, &from)
                    .unwrap_or_default()
                    .join(" → ");
                return Err(format!("цикл потока значений: {participants}"));
            }
            // FR-006: MCP-мутация — undo-шаг (все валидации прошли)
            scene.push_undo(scene.canvas.clone());
            let id = edge.id.clone();
            scene.canvas.add_edge(edge);
            scene.mark_dirty();
            // FR-014: топология изменилась — живой пересчёт потока
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": id }))
        }
        "edge_delete" => {
            let id = mcp_req_str(params, "id")?;
            // FR-006: MCP-мутация — undo-шаг (проверка существования связи
            // идёт в remove — неудача шага не оставит: снапшот не изменится,
            // а лишний пуш свернётся сравнением ниже)
            let snapshot = scene.canvas.clone();
            if !scene.canvas.remove_edge(id) {
                return Err(format!("связь не найдена: {id}"));
            }
            if scene.canvas != snapshot {
                scene.push_undo(snapshot);
            }
            scene.mark_dirty();
            // FR-014: топология изменилась — живой пересчёт потока
            scene.recompute_flow();
            Ok(serde_json::json!({ "id": id }))
        }
        // FR-014: тогл типа потока. Тогл в Value, замыкающий цикл value-рёбер,
        // — isError с участниками (пользователю UI показывает диалог, агенту
        // MCP — явную ошибку)
        "flow_set_kind" => {
            let id = mcp_req_str(params, "id")?;
            let kind = match params.get("kind").and_then(serde_json::Value::as_str) {
                Some("value") => FlowKind::Value,
                Some("control") => FlowKind::Control,
                other => {
                    return Err(format!(
                        "kind должен быть \"value\" или \"control\", получено {other:?}"
                    ));
                }
            };
            let edge_index = scene
                .canvas
                .edges
                .iter()
                .position(|edge| edge.id == id)
                .ok_or_else(|| format!("связь не найдена: {id}"))?;
            let (from, to) = {
                let edge = &scene.canvas.edges[edge_index];
                (edge.from_node.clone(), edge.to_node.clone())
            };
            if kind == FlowKind::Value
                && scene.canvas.edges[edge_index].flow_kind() != FlowKind::Value
                && canvas_core::creates_value_cycle(&scene.canvas, &from, &to)
            {
                let participants = canvas_core::value_path(&scene.canvas, &to, &from)
                    .unwrap_or_default()
                    .join(" → ");
                return Err(format!("цикл потока значений: {participants}"));
            }
            let snapshot = scene.canvas.clone();
            scene.canvas.edges[edge_index].set_flow_kind(kind);
            if scene.canvas != snapshot {
                scene.push_undo(snapshot);
            }
            scene.mark_dirty();
            scene.recompute_flow();
            Ok(serde_json::json!({
                "id": id,
                "kind": scene.canvas.edges[edge_index].flow_kind().as_str(),
            }))
        }
        // FR-029 v2: карта значений потока для агента — узловое значение +
        // именованные выходы + построчные значения + warnings/spilled.
        // FR-050/MCP-parity: значения АКТИВНОГО what-if сценария — те же,
        // что видит пользователь на канвасе (инвариант «MCP-видимость
        // эквивалентна UI», CP6): подмены активного сценария учитываются
        // (каскад Р-1: what-if перекрывает проливание перекрывает локальные).
        // Пересчёт СВЕЖИЙ: ленивые мутации (node_edit с text, CR-012) не
        // поднимают ревал — агенту нужен актуальный снимок; чтение —
        // revision не двигает (PRD-0007 AC-3.3). FR-050 Р-4: авто-строки
        // приёмников — из того же пересчёта.
        "flow_recalc" => mcp_flow_active_fresh(scene),
        // FR-014: проверка DAG-инварианта — [] или участники цикла
        "flow_cycle_check" => match flow::topo_sort(&scene.canvas) {
            Ok(_) => Ok(serde_json::json!([])),
            Err(cycle) => Ok(serde_json::json!(cycle.nodes)),
        },
        // PRD-0007 (X2, FR-048): дерево происхождения цифры — та же модель,
        // что окно проверки цепочки (инвариант F-5: один источник).
        // Активное состояние: значения what-if подмен видны агенту, как
        // пользователю; цикл потока — топология без значений (AC-2.4).
        "lineage" => {
            let node_id = mcp_req_str(params, "node_id")?;
            // null/нет поля — итог ноды (полоса D); иначе — индекс строки
            // Numi-листа (FR-025), как в LineageNodeId
            let line =
                match params.get("line") {
                    None | Some(serde_json::Value::Null) => None,
                    Some(value) => {
                        Some(value.as_i64().filter(|index| *index >= 0).ok_or(
                            "line: null (итог ноды) или целое ≥ 0 (индекс строки Numi-листа)",
                        )? as usize)
                    }
                };
            let root = match line {
                None => canvas_core::LineageNodeId::total(node_id),
                Some(index) => canvas_core::LineageNodeId::line(node_id, index),
            };
            // Паритет с окном проверки (app::build_lineage_snapshot):
            // Ready по активным значениям, Cycled — топология без значений.
            // Пересчёт СВЕЖИЙ — ленивые мутации не искажают дерево
            let (whatif, _stale) = scene.active_whatif_overrides();
            let tree = match flow::propagate_with_lines(&scene.canvas, &whatif) {
                Ok(solutions) => {
                    let data = canvas_core::DataSnapshots::new();
                    canvas_core::build_lineage(
                        &scene.canvas,
                        canvas_core::LineageFlow::Ready {
                            solutions: &solutions,
                            data: &data,
                        },
                        root,
                    )
                }
                Err(cycle) => canvas_core::build_lineage(
                    &scene.canvas,
                    canvas_core::LineageFlow::Cycled(&cycle),
                    root,
                ),
            }
            .map_err(|err| err.to_string())?;
            // DFS-порядок: родитель раньше ребёнка; children[] — индексы
            // в nodes (ромб разворачивается, дубликаты адресов возможны)
            let nodes: Vec<serde_json::Value> = tree
                .nodes
                .iter()
                .map(|node| {
                    let mut entry = serde_json::json!({
                        "node_id": node.node_id,
                        "line": node.line,
                        "kind": lineage_kind_str(node.kind),
                        "title": node.title,
                    });
                    match &node.value {
                        Some(Ok(value)) => {
                            entry["value"] = serde_json::json!(value.num);
                            entry["unit"] = serde_json::json!(value.unit.display());
                        }
                        Some(Err(text)) => {
                            entry["error"] = serde_json::json!(text);
                        }
                        None => {}
                    }
                    if let Some(formula) = &node.formula {
                        entry["formula"] = serde_json::json!(formula);
                    }
                    if let Some(label) = &node.label {
                        entry["label"] = serde_json::json!(label);
                    }
                    let children: Vec<serde_json::Value> = node
                        .children
                        .iter()
                        .map(|child| {
                            let mut item = serde_json::json!({ "child": child.child });
                            if let Some(via) = &child.via {
                                item["via"] = serde_json::json!({
                                    "edge_id": via.edge_id,
                                    "from_node": via.from_node,
                                    "to_node": via.to_node,
                                    "from_line": via.from_line,
                                    "from_output": via.from_output,
                                    "to_param": via.to_param,
                                });
                            }
                            item
                        })
                        .collect();
                    if !children.is_empty() {
                        entry["children"] = serde_json::Value::Array(children);
                    }
                    entry
                })
                .collect();
            Ok(serde_json::json!({
                "root": {
                    "node_id": tree.root.node_id,
                    "line": tree.root.line,
                },
                "nodes": nodes,
            }))
        }
        // PRD-0007 (X6, FR-048, F-9 must): объяснение цифры ТЕКСТОМ —
        // линейная развёртка того же дерева, что окно проверки и lineage
        // (инвариант F-5: один источник). Персона 4 §4: ИИ-агент получает
        // готовое объяснение «почему цифра такая» без разбора JSON.
        // Значения активного what-if сценария; цикл потока — топология
        // без значений (AC-2.4); бюджет 4096 — маркер truncated в тексте.
        "explain_number" => {
            let node_id = mcp_req_str(params, "node_id")?;
            let line =
                match params.get("line") {
                    None | Some(serde_json::Value::Null) => None,
                    Some(value) => {
                        Some(value.as_i64().filter(|index| *index >= 0).ok_or(
                            "line: null (итог ноды) или целое ≥ 0 (индекс строки Numi-листа)",
                        )? as usize)
                    }
                };
            let root = match line {
                None => canvas_core::LineageNodeId::total(node_id),
                Some(index) => canvas_core::LineageNodeId::line(node_id, index),
            };
            // Паритет с lineage и окном проверки (app::build_lineage_snapshot):
            // Ready по активным значениям, Cycled — топология без значений.
            // Пересчёт СВЕЖИЙ — ленивые мутации не искажают дерево.
            let (whatif, _stale) = scene.active_whatif_overrides();
            // Преамбула — по НАЛИЧИЮ активных подмен (после whatif_reset
            // режим остаётся включённым, но подмен нет — значения базовые).
            let has_overrides = !whatif.line_exprs.is_empty();
            let tree = match flow::propagate_with_lines(&scene.canvas, &whatif) {
                Ok(solutions) => {
                    let data = canvas_core::DataSnapshots::new();
                    canvas_core::build_lineage(
                        &scene.canvas,
                        canvas_core::LineageFlow::Ready {
                            solutions: &solutions,
                            data: &data,
                        },
                        root,
                    )
                }
                Err(cycle) => canvas_core::build_lineage(
                    &scene.canvas,
                    canvas_core::LineageFlow::Cycled(&cycle),
                    root,
                ),
            }
            .map_err(|err| err.to_string())?;
            let truncated = tree
                .nodes
                .iter()
                .any(|node| node.kind == canvas_core::LineageNodeKind::Truncated);
            let mut text = String::new();
            if has_overrides {
                text.push_str("Режим what-if: значения активного сценария.\n");
            }
            text.push_str(&canvas_core::explain_text(&tree));
            Ok(serde_json::json!({
                "render": "text",
                "text": text,
                "root": {
                    "node_id": tree.root.node_id,
                    "line": tree.root.line,
                },
                "nodes": tree.nodes.len(),
                "truncated": truncated,
            }))
        }
        // --- FR-017 (CP6): what-if сценарии ---
        // Построчная подмена активного сценария. Режим/сценарий
        // поднимаются автоматически (неявный «Сценарий MCP»). Подмена —
        // runtime: `.canvas` не мутируется (инвариант 2); expr нормализуется
        // (literal `\n` от ИИ-агентов → реальные переводы, mcp_text).
        "whatif_set_override" => {
            let node_id = mcp_req_str(params, "node_id")?.to_owned();
            let line = params
                .get("line")
                .and_then(serde_json::Value::as_u64)
                .ok_or("line: неотрицательное целое обязательно")? as usize;
            let expr = canvas_core::mcp_text::normalize_escapes(mcp_req_str(params, "expr")?);
            if scene.canvas.node(&node_id).is_none() {
                return Err(format!("нода не найдена: {node_id}"));
            }
            if !scene.whatif_active {
                scene.whatif_active = true;
            }
            if scene.active_scenario.is_none() {
                let snapshot = scene.canvas.clone();
                let index = scene.whatif_create_scenario("Сценарий MCP")?;
                scene.active_scenario = Some(index);
                canvas_core::whatif::scenarios_to_canvas(&mut scene.canvas, &scene.scenarios);
                if scene.canvas != snapshot {
                    scene.push_undo(snapshot);
                }
                scene.mark_dirty();
            }
            let index = scene.active_scenario.expect("сценарий активен");
            scene.scenarios[index]
                .line_exprs
                .insert((node_id.clone(), line), expr.clone());
            scene.recompute_flow();
            Ok(serde_json::json!({
                "scenario": scene.scenarios[index].name,
                "node": node_id,
                "line": line,
                "expr": expr,
            }))
        }
        // FR-017: sugar для шаблонных нод — адресация по имени параметра:
        // находит строку `param = …` в тексте ноды и строит подмену
        "whatif_set_param" => {
            let node_id = mcp_req_str(params, "node_id")?.to_owned();
            let param = mcp_req_str(params, "param")?.to_owned();
            let value = canvas_core::mcp_text::normalize_escapes(mcp_req_str(params, "value")?);
            let node = scene
                .canvas
                .node(&node_id)
                .ok_or_else(|| format!("нода не найдена: {node_id}"))?;
            let text = node.text.clone().unwrap_or_default();
            let line = {
                text.split('\n')
                    .position(|row| {
                        row.split_once('=')
                            .map(|(name, _)| name.trim() == param)
                            .unwrap_or(false)
                    })
                    .ok_or_else(|| format!("параметр {param} не найден в тексте ноды {node_id}"))?
            };
            let expr = format!("{param} = {value}");
            mcp_dispatch(
                scene,
                templates,
                "whatif_set_override",
                &serde_json::json!({ "node_id": node_id, "line": line, "expr": expr }),
            )
        }
        // FR-017: список сценариев с маркерами протухших подмен (Q5c)
        "whatif_scenario_list" => {
            let scenarios: Vec<serde_json::Value> = scene
                .scenarios
                .iter()
                .map(|scenario| {
                    let stale =
                        canvas_core::whatif::validate_scenario(&scene.canvas, scenario).len();
                    serde_json::json!({
                        "name": scenario.name,
                        "overrides": scenario.line_exprs.len(),
                        "stale": stale,
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "active": scene
                    .active_scenario
                    .and_then(|i| scene.scenarios.get(i))
                    .map(|scenario| scenario.name.clone()),
                "whatif_active": scene.whatif_active,
                "scenarios": scenarios,
            }))
        }
        // FR-017: создать именованный сценарий (freeze, Q5d) — мутация
        // `canvasdesk.whatif` одним undo-шагом
        "whatif_scenario_create" => {
            let name = params
                .get("name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let snapshot = scene.canvas.clone();
            let index = scene.whatif_create_scenario(name)?;
            // Активация нового сценария (паттерн UI create): без неё
            // set_override завёл бы параллельный «Сценарий MCP»
            scene.active_scenario = Some(index);
            if !scene.whatif_active {
                scene.whatif_active = true;
            }
            canvas_core::whatif::scenarios_to_canvas(&mut scene.canvas, &scene.scenarios);
            scene.push_undo(snapshot);
            scene.mark_dirty();
            scene.recompute_flow();
            Ok(serde_json::json!({
                "name": scene.scenarios[index].name,
                "index": index,
            }))
        }
        // FR-017: удалить сценарий (мутация `.canvas`, undo-шаг)
        "whatif_scenario_delete" => {
            let name = mcp_req_str(params, "name")?;
            let Some(index) = scene.scenarios.iter().position(|s| s.name == name) else {
                return Err(format!("сценарий не найден: {name}"));
            };
            let snapshot = scene.canvas.clone();
            scene.whatif_delete_scenario(index);
            canvas_core::whatif::scenarios_to_canvas(&mut scene.canvas, &scene.scenarios);
            scene.push_undo(snapshot);
            scene.mark_dirty();
            scene.recompute_flow();
            Ok(serde_json::json!({ "deleted": name }))
        }
        // FR-017: переключение — runtime-only, файл не трогается (инвариант 2)
        "whatif_scenario_activate" => {
            let name = mcp_req_str(params, "name")?;
            if !scene.whatif_active {
                scene.whatif_active = true;
            }
            let index = if name == "База" {
                None
            } else {
                Some(
                    scene
                        .scenarios
                        .iter()
                        .position(|s| s.name == name)
                        .ok_or_else(|| format!("сценарий не найден: {name}"))?,
                )
            };
            scene.whatif_activate(index);
            Ok(serde_json::json!({ "active": name }))
        }
        // FR-017: дельты активного сценария — пары «было → стало», те же,
        // что видны на канвасе (инвариант 6)
        "whatif_deltas" => {
            let rows = whatif_delta_rows(scene);
            let nodes: serde_json::Map<String, serde_json::Value> = rows
                .iter()
                .map(|row| {
                    (
                        format!(
                            "{}:{}",
                            row.node,
                            row.line
                                .map(|l| l.to_string())
                                .unwrap_or_else(|| "value".to_owned())
                        ),
                        row.to_json(),
                    )
                })
                .collect();
            Ok(serde_json::json!({
                "active": scene
                    .active_scenario
                    .and_then(|i| scene.scenarios.get(i))
                    .map(|scenario| scenario.name.clone()),
                "deltas": nodes,
            }))
        }
        // FR-017 (Q6a/Q6b): Apply активного сценария — записать подмены в
        // persisted-строки/params и удалить сценарий; один undo-шаг
        "whatif_apply" => {
            if scene.active_scenario.is_none() {
                return Err("активен сценарий «База» — нечего применять".to_owned());
            }
            let snapshot = scene.canvas.clone();
            let applied = scene.whatif_apply_active();
            canvas_core::whatif::scenarios_to_canvas(&mut scene.canvas, &scene.scenarios);
            if scene.canvas != snapshot {
                scene.push_undo(snapshot);
                scene.mark_dirty();
            }
            scene.recompute_flow();
            Ok(serde_json::json!({ "applied": applied }))
        }
        // FR-017: сброс overrides активного сценария (runtime, файл не трогается)
        "whatif_reset" => {
            let cleared = scene.whatif_override_count();
            if let Some(index) = scene.active_scenario {
                if let Some(scenario) = scene.scenarios.get_mut(index) {
                    scenario.line_exprs.clear();
                }
            }
            scene.recompute_flow();
            Ok(serde_json::json!({ "cleared": cleared }))
        }
        // CR-008: стороны подключения связи. "auto" — снять закрепления
        // (кратчайший путь); "from"/"to"/"both" — закрепить концы, фиксируя
        // текущие эффективные стороны (WYSIWYG, как в палитре)
        "edge_ports" => {
            let id = mcp_req_str(params, "id")?;
            let pin = params.get("pin").and_then(serde_json::Value::as_str);
            let edge_index = scene
                .canvas
                .edges
                .iter()
                .position(|edge| edge.id == id)
                .ok_or_else(|| format!("связь не найдена: {id}"))?;
            let snapshot = scene.canvas.clone();
            match pin {
                Some("auto") => {
                    scene.canvas.edges[edge_index].clear_port_pins();
                }
                Some("from" | "to" | "both") => {
                    let pin_from = pin != Some("to");
                    let pin_to = pin != Some("from");
                    for (end, do_pin) in [
                        (canvas_core::EdgeEnd::From, pin_from),
                        (canvas_core::EdgeEnd::To, pin_to),
                    ] {
                        if !do_pin {
                            continue;
                        }
                        // Текущая эффективная сторона конца (геометрия как на экране)
                        let Some((side, _)) =
                            canvas_core::edge_endpoint(&scene.canvas, edge_index, end)
                        else {
                            return Err("висячая связь (нода не найдена)".to_owned());
                        };
                        let edge = &mut scene.canvas.edges[edge_index];
                        match end {
                            canvas_core::EdgeEnd::From => edge.from_side = Some(side),
                            canvas_core::EdgeEnd::To => edge.to_side = Some(side),
                        }
                        edge.set_port_pin(end, true);
                    }
                }
                other => {
                    return Err(format!(
                        "pin должен быть \"auto\", \"from\", \"to\" или \"both\", получено {other:?}"
                    ));
                }
            }
            let edge = &scene.canvas.edges[edge_index];
            let (pin_from, pin_to) = edge.port_pins();
            if scene.canvas != snapshot {
                scene.push_undo(snapshot);
            }
            scene.mark_dirty();
            Ok(serde_json::json!({
                "id": id,
                "pins": {
                    "from": pin_from,
                    "to": pin_to,
                },
            }))
        }
        // FR-032: чтение связей — агент восстанавливает топологию (CR-013
        // G4: раньше рёбер не было видно вовсе, сборка шла вслепую)
        "edges_list" => {
            let edges: Vec<serde_json::Value> =
                scene.canvas.edges.iter().map(mcp_edge_json).collect();
            Ok(serde_json::Value::Array(edges))
        }
        // FR-032: одно ребро по id — полная каноническая схема
        "edge_get" => {
            let id = mcp_req_str(params, "id")?;
            let edge = scene
                .canvas
                .edges
                .iter()
                .find(|edge| edge.id == id)
                .ok_or_else(|| format!("связь не найдена: {id}"))?;
            Ok(mcp_edge_json(edge))
        }
        // FR-032: валидация модели — отчёт ядра (validate.rs) с кодами
        // E-*/W-*; чтение, не мутация: undo/автосейв не затрагиваются
        "graph_validate" => {
            let issues = canvas_core::validate::validate(&scene.canvas);
            let valid = !canvas_core::validate::has_errors(&issues);
            let issues: Vec<serde_json::Value> = issues
                .iter()
                .map(|issue| serde_json::to_value(issue).map_err(|err| err.to_string()))
                .collect::<Result<_, _>>()?;
            Ok(serde_json::json!({
                "valid": valid,
                "issues": issues,
            }))
        }
        // FR-016 (CP5) + MCP-parity: карта флагов АКТИВНОГО состояния
        // (what-if подмены учитываются) — те же severity/бейджи, что
        // рисует оверлей канваса Ctrl+B (инвариант 4 FR-016). Пересчёт
        // СВЕЖИЙ (как flow_recalc) — ленивые мутации (CR-012) не
        // искажают отчёт; чтение — не мутация. Цикл — честный error.
        "analyze_bottlenecks" => {
            let (whatif, _stale) = scene.active_whatif_overrides();
            let solutions = match flow::propagate_with_lines(&scene.canvas, &whatif) {
                Ok(solutions) => solutions,
                Err(cycle) => {
                    return Ok(serde_json::json!({
                        "error": format!("цикл потока значений: {cycle}"),
                    }));
                }
            };
            let config = AnalysisConfig::default();
            let state = analyze::analyze(&scene.canvas, &solutions, &config);
            let nodes: Vec<serde_json::Value> = scene
                .canvas
                .nodes
                .iter()
                .filter_map(|node| {
                    let flags = state.get(&node.id)?;
                    let mut entry = serde_json::Map::new();
                    entry.insert("id".into(), serde_json::json!(node.id));
                    if let Ok(serde_json::Value::Object(map)) = serde_json::to_value(flags) {
                        entry.extend(map);
                    }
                    // Бейдж — строка, которую видит пользователь на канвасе
                    entry.insert(
                        "badge".into(),
                        serde_json::json!(analyze::badge_text(flags)),
                    );
                    Some(serde_json::Value::Object(entry))
                })
                .collect();
            Ok(serde_json::json!({
                "nodes": nodes,
                "thresholds": serde_json::to_value(config).unwrap_or(serde_json::json!({})),
            }))
        }
        // FR-018: список шаблонов реестра — те же, что в палитре/wheel
        // (инвариант 4: MCP-видимость эквивалентна UI)
        "template_list" => {
            let templates: Vec<serde_json::Value> = templates
                .list()
                .iter()
                .map(|manifest| {
                    let params: serde_json::Map<String, serde_json::Value> = manifest
                        .params
                        .iter()
                        .map(|spec| {
                            let mut entry = serde_json::json!({
                                "type": spec.kind.as_str(),
                                "default": spec.default,
                            });
                            if let Some(unit) = &spec.unit {
                                entry["unit"] = serde_json::json!(unit);
                            }
                            if let Some(min) = spec.min {
                                entry["min"] = serde_json::json!(min);
                            }
                            if let Some(max) = spec.max {
                                entry["max"] = serde_json::json!(max);
                            }
                            (spec.name.clone(), entry)
                        })
                        .collect();
                    // FR-029: именованные выходы — агент адресует их
                    // рёбрами fromOutput
                    let outputs: Vec<serde_json::Value> = manifest
                        .outputs
                        .iter()
                        .map(|spec| {
                            let mut entry = serde_json::json!({ "name": spec.name });
                            if let Some(unit) = &spec.unit {
                                entry["unit"] = serde_json::json!(unit);
                            }
                            match &spec.source {
                                canvas_core::templates::OutputSource::Line(line) => {
                                    entry["line"] = serde_json::json!(line);
                                }
                                canvas_core::templates::OutputSource::Expr(expr) => {
                                    entry["expr"] = serde_json::json!(expr);
                                }
                            }
                            entry
                        })
                        .collect();
                    let mut entry = serde_json::json!({
                        "id": manifest.id,
                        "name_en": manifest.name,
                        "name_ru": manifest.name_ru,
                        "version": manifest.version,
                        "category": manifest.category,
                        "description": manifest.description,
                        "description_en": manifest.description_en,
                        "expr": manifest.expr,
                        "icon": manifest.icon,
                        "color": manifest.color,
                        "source": manifest.source.as_str(),
                        "params": params,
                    });
                    if !outputs.is_empty() {
                        entry["outputs"] = serde_json::Value::Array(outputs);
                    }
                    entry
                })
                .collect();
            Ok(serde_json::Value::Array(templates))
        }
        // FR-018: инстанциация шаблона (идентично wheel/палитре): text-нода
        // с Numi-листом параметров и снимком canvasdesk.template; undo-шаг
        "template_instantiate" => {
            let id = mcp_req_str(params, "id")?;
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            let manifest = templates
                .find(id)
                .ok_or_else(|| format!("шаблон не найден: {id}"))?
                .clone();
            let mut overrides: BTreeMap<String, canvas_core::templates::TemplateParam> =
                BTreeMap::new();
            if let Some(map) = params.get("params").and_then(serde_json::Value::as_object) {
                for (name, value) in map {
                    let param = match value {
                        serde_json::Value::Number(num) => {
                            let num = num
                                .as_f64()
                                .ok_or_else(|| format!("параметр {name}: число"))?;
                            // Число без единицы наследует единицу параметра
                            // манифеста ({"rps": 2000} = 2000 rps)
                            let unit = manifest
                                .params
                                .iter()
                                .find(|spec| &spec.name == name)
                                .and_then(|spec| spec.unit.clone());
                            canvas_core::templates::TemplateParam { num, unit }
                        }
                        serde_json::Value::Object(obj) => canvas_core::templates::TemplateParam {
                            num: obj
                                .get("num")
                                .and_then(serde_json::Value::as_f64)
                                .ok_or_else(|| format!("параметр {name}: num обязателен"))?,
                            unit: obj
                                .get("unit")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                        },
                        _ => {
                            return Err(format!("параметр {name}: число или {{num, unit}}"));
                        }
                    };
                    overrides.insert(name.clone(), param);
                }
            }
            let node_id = next_free_id(&scene.canvas, "tpl");
            let mut node =
                canvas_core::templates::instantiate(&manifest, &overrides, node_id, x, y)
                    .map_err(|err| err.to_string())?;
            // FR-023: авто-высота — единая с GUI-путём (см. комментарий)
            fit_template_node_height(&mut node);
            let index = scene.canvas.nodes.len();
            // FR-006: MCP-мутация — undo-шаг (после валидации, до вставки)
            scene.push_undo(scene.canvas.clone());
            scene.canvas.nodes.push(node);
            scene.spatial.insert(index, &scene.canvas.nodes[index]);
            scene.mark_dirty();
            // Формула шаблона с параметрами — в поток (FR-014)
            scene.recompute_flow();
            let summary = mcp_node_summary(&scene.canvas.nodes[index], true);
            Ok(serde_json::json!({
                "id": scene.canvas.nodes[index].id,
                "index": index,
                "node": summary,
            }))
        }
        // PRD-0008 (Q5 v2): список встроенных схем галереи — те же пакеты,
        // что видит пользователь в галерее (Ctrl+T): готовые канвасы с
        // расчётами, пучками и подсказками (инвариант «MCP-видимость
        // эквивалентна UI»)
        "schemes_list" => {
            let schemes: Vec<serde_json::Value> = canvas_core::schemes::SchemeRegistry::embedded()
                .list()
                .iter()
                .map(|manifest| {
                    serde_json::json!({
                        "id": manifest.id,
                        "name": manifest.name_ru,
                        "name_en": manifest.name_en,
                        "category": manifest.category,
                        "category_ru": manifest.category_ru,
                        "category_en": manifest.category_en,
                        "version": manifest.version,
                        "description": manifest.description_ru,
                        "description_en": manifest.description_en,
                        "nodes": manifest.content.nodes.len(),
                        "edges": manifest.content.edges.len(),
                    })
                })
                .collect();
            Ok(serde_json::Value::Array(schemes))
        }
        // PRD-0008 (Q5 v2): вставить схему в текущий канвас — как «Открыть»
        // в галерее: ремап id без коллизий (note-N/group-N/edge-N),
        // содержимое центрируется в точку (x, y) или центр viewport;
        // один undo-шаг, полный пересчёт. Ответ: созданные объекты, bbox
        // (для viewport_set/zoom-to-fit) и значения активного состояния
        "schemes_apply" => {
            let id = mcp_req_str(params, "id")?;
            let manifest = canvas_core::schemes::SchemeRegistry::embedded()
                .get(id)
                .ok_or_else(|| format!("схема не найдена: {id}"))?
                .clone();
            let x = mcp_opt_f32(params, "x").unwrap_or(scene.viewport.x);
            let y = mcp_opt_f32(params, "y").unwrap_or(scene.viewport.y);
            let instance =
                crate::scheme_apply::instantiate_scheme(&manifest, &scene.canvas, [x, y])
                    .map_err(|err| err.to_string())?;
            let nodes: Vec<String> = instance.nodes.iter().map(|node| node.id.clone()).collect();
            let edges: Vec<serde_json::Value> = instance.edges.iter().map(mcp_edge_json).collect();
            // FR-006: один undo-шаг на вставку (до мутации — как галерея)
            let snapshot = scene.canvas.clone();
            scene.push_undo(snapshot);
            for node in instance.nodes {
                let index = scene.canvas.nodes.len();
                scene.canvas.nodes.push(node);
                scene.spatial.insert(index, &scene.canvas.nodes[index]);
            }
            for edge in instance.edges {
                scene.canvas.add_edge(edge);
            }
            scene.mark_dirty();
            scene.recompute_flow();
            // Значения АКТИВНОГО состояния (вставка не трогает сценарии
            // what-if — подмены сохраняются) + авто-строки FR-050 Р-4;
            // цикл не роняет вставку — честный error в поле flow
            let flow = match mcp_flow_active_fresh(scene) {
                Ok(flow) => flow,
                Err(cycle) => serde_json::json!({ "error": cycle }),
            };
            Ok(serde_json::json!({
                "applied": manifest.id,
                "name": manifest.display_name(true),
                "nodes": nodes,
                "edges": edges,
                "bbox": instance.bbox,
                "flow": flow,
            }))
        }
        "viewport_get" => Ok(serde_json::json!({
            "x": scene.viewport.x,
            "y": scene.viewport.y,
            "zoom": scene.viewport.zoom,
        })),
        "viewport_set" => {
            let x = mcp_req_f32(params, "x")?;
            let y = mcp_req_f32(params, "y")?;
            scene.viewport.x = x;
            scene.viewport.y = y;
            // Зум клампится — как Camera::set_zoom (canvas-render); константы
            // синхронны, паритет — тестом в canvas-app
            if let Some(zoom) = mcp_opt_f32(params, "zoom") {
                scene.viewport.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
            }
            Ok(serde_json::json!({
                "x": scene.viewport.x,
                "y": scene.viewport.y,
                "zoom": scene.viewport.zoom,
            }))
        }
        // FR-033: атомарная батч-композиция — клон → apply → commit
        // (один undo-шаг) либо отброшенный клон при любой ошибке операции
        "graph_apply" => mcp_graph_apply(scene, templates, params),
        other => Err(format!("неизвестный инструмент: {other}")),
    }
}

// --- FR-033: graph_apply — атомарная батч-композиция графа ---

/// Лимиты батча (FR-033 п.1): ≤ 256 операций, ≤ 128 новых нод — защита
/// live-бюджета пересчёта (SPEC §6.3: ≤ 1000 нод < 10 мс).
const GRAPH_APPLY_MAX_OPS: usize = 256;
const GRAPH_APPLY_MAX_NODES: usize = 128;

/// Ошибка одной операции батча: стабильный код + сообщение (FR-033 п.2в —
/// код различим машиной, сообщение — человеку/агенту).
struct BatchOpError {
    code: &'static str,
    message: String,
}

impl BatchOpError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Запись отчёта об операции батча (FR-033 п.3).
struct BatchEntry {
    op_index: usize,
    op: &'static str,
    /// true — операция создала объект (created), false — адресовала
    /// существующий (param_set/node_move — только report, FR-033 п.3)
    created: bool,
    ref_name: Option<String>,
    node_id: Option<String>,
    edge_id: Option<String>,
}

impl BatchEntry {
    /// Идентификатор для report: созданный/адресованный объект операции.
    fn object_id(&self) -> Option<&String> {
        self.node_id.as_ref().or(self.edge_id.as_ref())
    }
}

/// Структурированный ответ об ошибке операции (канвас при этом НЕ меняется —
/// клон отброшен вызывающей стороной).
fn graph_apply_error(op_index: usize, err: &BatchOpError) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "op_index": op_index,
        "code": err.code,
        "message": err.message,
    })
}

/// Резолв адреса ноды в батче: ref (созданные в этом же батче) приоритетно,
/// иначе — существующий id ноды канваса.
fn batch_node_id(
    canvas: &Canvas,
    refs: &HashMap<String, String>,
    key: &str,
) -> Result<String, BatchOpError> {
    if let Some(id) = refs.get(key) {
        return Ok(id.clone());
    }
    if canvas.node(key).is_some() {
        return Ok(key.to_owned());
    }
    Err(BatchOpError::new(
        "E-NOT-FOUND",
        format!("нода не найдена (ни ref батча, ни id канваса): {key}"),
    ))
}

/// Опциональная строка из операции.
fn batch_opt_str<'v>(op: &'v serde_json::Value, name: &str) -> Option<&'v str> {
    op.get(name).and_then(serde_json::Value::as_str)
}

/// Обязательная строка из операции.
fn batch_req_str<'v>(op: &'v serde_json::Value, name: &str) -> Result<&'v str, BatchOpError> {
    op.get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| BatchOpError::new("E-BAD-OP", format!("отсутствует поле '{name}'")))
}

/// Зарегистрировать ref батча: дубликат — ошибка операции (ref должен
/// однозначно адресовать ноду сборки).
fn register_ref(
    refs: &mut HashMap<String, String>,
    name: &str,
    id: &str,
) -> Result<(), BatchOpError> {
    if refs.insert(name.to_owned(), id.to_owned()).is_some() {
        return Err(BatchOpError::new(
            "E-BAD-OP",
            format!("ref '{name}' уже использован в этом батче"),
        ));
    }
    Ok(())
}

/// Обязательное число из операции.
fn batch_req_f64(op: &serde_json::Value, name: &str) -> Result<f64, BatchOpError> {
    op.get(name)
        .and_then(serde_json::Value::as_f64)
        .ok_or_else(|| BatchOpError::new("E-BAD-OP", format!("отсутствует число '{name}'")))
}

/// Сторона связи из операции батча: "any"/отсутствие → None (автовывод
/// из геометрии) — семантика mcp_side.
fn batch_side(op: &serde_json::Value, name: &str) -> Result<Option<Side>, BatchOpError> {
    match op.get(name).and_then(serde_json::Value::as_str) {
        None | Some("any") => Ok(None),
        Some(text) => serde_json::from_value::<Side>(serde_json::Value::String(text.to_owned()))
            .map(Some)
            .map_err(|_| {
                BatchOpError::new("E-BAD-OP", format!("неверная сторона '{name}': {text}"))
            }),
    }
}

/// Применить одну операцию батча к клону канваса (FR-033 п.2б). Чистые
/// мутации Canvas: spatial/undo/пересчёт — на стороне коммита, не здесь.
fn batch_apply_op(
    canvas: &mut Canvas,
    refs: &mut HashMap<String, String>,
    templates: &canvas_core::templates::TemplateRegistry,
    op_index: usize,
    op: &serde_json::Value,
) -> Result<BatchEntry, BatchOpError> {
    let entry = |op: &'static str, created: bool, ref_name: Option<String>| BatchEntry {
        op_index,
        op,
        created,
        ref_name,
        node_id: None,
        edge_id: None,
    };
    let kind = batch_req_str(op, "op")?;
    match kind {
        "node_create_note" => {
            let x = batch_req_f64(op, "x")? as f32;
            let y = batch_req_f64(op, "y")? as f32;
            let text_owned = batch_opt_str(op, "text")
                .map(canvas_core::mcp_text::normalize_escapes)
                .unwrap_or_default();
            let text = text_owned.as_str();
            let ref_name = batch_opt_str(op, "ref").map(str::to_owned);
            let mut node = Node::text(next_free_id(canvas, "note"), text, x, y);
            if let Some(width) = op.get("width").and_then(serde_json::Value::as_f64) {
                node.width = width as f32;
            }
            if let Some(height) = op.get("height").and_then(serde_json::Value::as_f64) {
                node.height = height as f32;
            }
            // FR-013: строки «= …» в тексте — формула (единая семантика)
            node.set_expr(split_formula_lines(text));
            let id = node.id.clone();
            canvas.nodes.push(node);
            if let Some(name) = &ref_name {
                register_ref(refs, name, &id)?;
            }
            let mut e = entry("node_create_note", true, ref_name);
            e.node_id = Some(id);
            Ok(e)
        }
        "node_create_file" => {
            let path = batch_req_str(op, "path")?;
            let x = batch_req_f64(op, "x")? as f32;
            let y = batch_req_f64(op, "y")? as f32;
            let ref_name = batch_opt_str(op, "ref").map(str::to_owned);
            // Файл на диске НЕ создаём — только карточка в модели
            let node = Node::file(
                next_free_id(canvas, "file"),
                path,
                x,
                y,
                op.get("width")
                    .and_then(serde_json::Value::as_f64)
                    .map(|v| v as f32)
                    .unwrap_or(DEFAULT_FILE_CARD_W),
                op.get("height")
                    .and_then(serde_json::Value::as_f64)
                    .map(|v| v as f32)
                    .unwrap_or(DEFAULT_FILE_CARD_H),
            );
            let id = node.id.clone();
            canvas.nodes.push(node);
            if let Some(name) = &ref_name {
                register_ref(refs, name, &id)?;
            }
            let mut e = entry("node_create_file", true, ref_name);
            e.node_id = Some(id);
            Ok(e)
        }
        "template_instantiate" => {
            let template_id = batch_req_str(op, "template")?;
            let x = batch_req_f64(op, "x")? as f32;
            let y = batch_req_f64(op, "y")? as f32;
            let ref_name = batch_opt_str(op, "ref").map(str::to_owned);
            let manifest = templates.find(template_id).ok_or_else(|| {
                BatchOpError::new("E-NOT-FOUND", format!("шаблон не найден: {template_id}"))
            })?;
            let mut overrides: BTreeMap<String, canvas_core::templates::TemplateParam> =
                BTreeMap::new();
            if let Some(map) = op.get("params").and_then(serde_json::Value::as_object) {
                for (name, value) in map {
                    let param = match value {
                        serde_json::Value::Number(num) => {
                            let num = num.as_f64().ok_or_else(|| {
                                BatchOpError::new("E-BAD-OP", format!("параметр {name}: число"))
                            })?;
                            // Число без единицы наследует единицу параметра
                            let unit = manifest
                                .params
                                .iter()
                                .find(|spec| &spec.name == name)
                                .and_then(|spec| spec.unit.clone());
                            canvas_core::templates::TemplateParam { num, unit }
                        }
                        serde_json::Value::Object(obj) => canvas_core::templates::TemplateParam {
                            num: obj.get("num").and_then(serde_json::Value::as_f64).ok_or_else(
                                || {
                                    BatchOpError::new(
                                        "E-BAD-OP",
                                        format!("параметр {name}: num обязателен"),
                                    )
                                },
                            )?,
                            unit: obj
                                .get("unit")
                                .and_then(serde_json::Value::as_str)
                                .map(str::to_owned),
                        },
                        _ => {
                            return Err(BatchOpError::new(
                                "E-BAD-OP",
                                format!("параметр {name}: число или {{num, unit}}"),
                            ))
                        }
                    };
                    overrides.insert(name.clone(), param);
                }
            }
            let node_id = next_free_id(canvas, "tpl");
            let mut node = canvas_core::templates::instantiate(
                manifest,
                &overrides,
                node_id,
                x,
                y,
            )
            .map_err(|err| match err {
                canvas_core::templates::InstantiateError::UnknownParam(name) => {
                    BatchOpError::new("E-PORT-UNKNOWN", format!("неизвестный параметр шаблона: {name}"))
                }
                canvas_core::templates::InstantiateError::ParamOutOfRange { name, value } => {
                    BatchOpError::new("E-RANGE", format!("параметр {name} вне диапазона: {value}"))
                }
                other => BatchOpError::new("E-BAD-OP", other.to_string()),
            })?;
            // FR-023: авто-высота — единая с GUI-путём и template_instantiate
            fit_template_node_height(&mut node);
            let id = node.id.clone();
            canvas.nodes.push(node);
            if let Some(name) = &ref_name {
                register_ref(refs, name, &id)?;
            }
            let mut e = entry("template_instantiate", true, ref_name);
            e.node_id = Some(id);
            Ok(e)
        }
        "edge_create" => {
            let from_key = batch_opt_str(op, "fromRef")
                .or_else(|| batch_opt_str(op, "from"))
                .ok_or_else(|| BatchOpError::new("E-BAD-OP", "отсутствует поле 'fromRef'/'from'"))?;
            let to_key = batch_opt_str(op, "toRef")
                .or_else(|| batch_opt_str(op, "to"))
                .ok_or_else(|| BatchOpError::new("E-BAD-OP", "отсутствует поле 'toRef'/'to'"))?;
            let from = batch_node_id(canvas, refs, from_key)?;
            let to = batch_node_id(canvas, refs, to_key)?;
            let from_side = batch_side(op, "fromSide")?;
            let to_side = batch_side(op, "toSide")?;
            // kind: value|control, дефолт control (FR-029 edge_create v2)
            let flow_kind = match batch_opt_str(op, "kind") {
                None => flow::FlowKind::Control,
                Some("value") => flow::FlowKind::Value,
                Some("control") => flow::FlowKind::Control,
                Some(other) => {
                    return Err(BatchOpError::new(
                        "E-BAD-OP",
                        format!("kind должен быть \"value\" или \"control\", получено {other:?}"),
                    ))
                }
            };
            // Адресация портов (FR-029): fromLine (индекс) взаимно
            // исключителен с fromOutput (имя)
            let from_line = match op.get("fromLine") {
                None => None,
                Some(value) => {
                    if value.as_u64().is_none() {
                        return Err(BatchOpError::new(
                            "E-BAD-OP",
                            "fromLine должен быть целым ≥ 0",
                        ));
                    }
                    if op.get("fromOutput").is_some() {
                        return Err(BatchOpError::new(
                            "E-BAD-OP",
                            "fromLine и fromOutput взаимно исключительны",
                        ));
                    }
                    value.as_u64().map(|v| v as usize)
                }
            };
            let from_output = batch_opt_str(op, "fromOutput").map(str::to_owned);
            let to_param = batch_opt_str(op, "toParam").map(str::to_owned);
            if flow_kind == flow::FlowKind::Value
                && canvas_core::creates_value_cycle(canvas, &from, &to)
            {
                let participants = canvas_core::value_path(canvas, &to, &from)
                    .unwrap_or_default()
                    .join(" → ");
                return Err(BatchOpError::new(
                    "E-CYCLE",
                    format!("цикл потока значений: {participants}"),
                ));
            }
            // Валидация имён портов по снапшотам шаблонов (FR-029 п.5)
            if let Some(param) = &to_param {
                let known = canvas
                    .node(&to)
                    .and_then(|node| node.template())
                    .map(|tpl| tpl.params.contains_key(param))
                    .unwrap_or(false);
                if !known {
                    return Err(BatchOpError::new(
                        "E-PORT-UNKNOWN",
                        format!("у ноды {to} нет параметра '{param}' (приёмник не шаблон либо параметр не объявлен)"),
                    ));
                }
                // FR-050 Н4 (fail-fast дубль-входов): симметрично прямому
                // edge_create — второе value-ребро в занятый `toParam`
                // отклоняется операцией (батч атомарно откатывается,
                // инвариант FR-033); замена источника — явная пара
                // edge_delete + edge_create в том же батче.
                if let Some(existing) = canvas.edges.iter().find(|edge| {
                    edge.flow_kind() == flow::FlowKind::Value
                        && edge.to_node == to
                        && edge.to_param.as_deref() == Some(param.as_str())
                }) {
                    return Err(BatchOpError::new(
                        "E-DOUBLE-INPUT",
                        format!(
                            "параметр '{param}' ноды {to} уже запитан value-ребром {} — замена: edge_delete + edge_create в одном батче",
                            existing.id
                        ),
                    ));
                }
            }
            if let Some(output) = &from_output {
                let known = canvas
                    .node(&from)
                    .and_then(|node| node.template())
                    .map(|tpl| tpl.outputs.iter().any(|spec| &spec.name == output))
                    .unwrap_or(false);
                if !known {
                    return Err(BatchOpError::new(
                        "E-PORT-UNKNOWN",
                        format!("у ноды {from} нет выхода '{output}' (источник не шаблон либо выход не объявлен)"),
                    ));
                }
            }
            let mut edge = Edge::new(canvas.next_edge_id(), from, from_side, to, to_side);
            edge.set_flow_kind(flow_kind);
            edge.from_line = from_line;
            edge.from_output = from_output;
            edge.to_param = to_param;
            let id = edge.id.clone();
            canvas.add_edge(edge);
            // FR-050 Н4: ref ребра регистрируется в карте батча — пара
            // «edge_delete + edge_create» (замена занятого toParam) адресует
            // созданное ребро по ref в том же батче (уникальность имени —
            // как у нод, дубликат — ошибка операции).
            if let Some(name) = batch_opt_str(op, "ref") {
                register_ref(refs, name, &id)?;
            }
            let ref_name = batch_opt_str(op, "ref").map(str::to_owned);
            let mut e = entry("edge_create", true, ref_name);
            e.edge_id = Some(id);
            Ok(e)
        }
        "edge_delete" => {
            // FR-050 Н4: удаление ребра в батче — вторая половина пары
            // «delete + create» замены источника (замена занятого toParam —
            // edge_create падает E-DOUBLE-INPUT, замена = явная пара в ОДНОМ
            // батче: атомарно, один undo-шаг на весь батч). id — существующее
            // ребро канваса или ref рёбра, созданного ранее в этом же батче.
            let key = batch_opt_str(op, "id")
                .or_else(|| batch_opt_str(op, "ref"))
                .ok_or_else(|| {
                    BatchOpError::new("E-BAD-OP", "отсутствует поле 'id'/'ref'")
                })?;
            let id = refs.get(key).cloned().unwrap_or_else(|| key.to_owned());
            let index = canvas
                .edges
                .iter()
                .position(|edge| edge.id == id)
                .ok_or_else(|| {
                    BatchOpError::new("E-NOT-FOUND", format!("связь не найдена: {id}"))
                })?;
            canvas.edges.remove(index);
            let mut e = entry("edge_delete", false, None);
            e.edge_id = Some(id);
            Ok(e)
        }
        "param_set" => {
            let key = batch_opt_str(op, "ref")
                .or_else(|| batch_opt_str(op, "id"))
                .ok_or_else(|| BatchOpError::new("E-BAD-OP", "отсутствует поле 'ref'/'id'"))?;
            let param = batch_req_str(op, "param")?;
            let num = batch_req_f64(op, "value")?;
            let unit_op = batch_opt_str(op, "unit").map(str::to_owned);
            let node_id = batch_node_id(canvas, refs, key)?;
            let index = canvas
                .nodes
                .iter()
                .position(|node| node.id == node_id)
                .ok_or_else(|| BatchOpError::new("E-NOT-FOUND", format!("нода не найдена: {node_id}")))?;
            let node = &mut canvas.nodes[index];
            let text = node
                .text
                .clone()
                .ok_or_else(|| BatchOpError::new("E-PARAM-UNKNOWN", format!("у ноды {node_id} нет текста — параметра '{param}' нет")))?;
            // Единица: явная из операции → снапшот шаблона → токен из строки
            let unit_fallback = || -> Option<String> {
                let line = text.split('\n').find(|line| {
                    line.split_once('=')
                        .map(|(name, _)| name.trim() == param)
                        .unwrap_or(false)
                })?;
                let rhs = line.split_once('=')?.1.trim();
                rhs.split_whitespace().nth(1).map(str::to_owned)
            };
            let unit = unit_op.or_else(|| {
                node.template().and_then(|tpl| {
                    tpl.params
                        .get(param)
                        .and_then(|spec| spec.unit.clone())
                })
            }).or_else(unit_fallback);
            // Правка ровно одной строки «param = value unit», остальные —
            // без изменений (FR-033 п.4); параметра нет — ошибка (без append)
            let mut lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
            let mut replaced = false;
            for line in &mut lines {
                let matches = line
                    .split_once('=')
                    .map(|(name, _)| name.trim() == param)
                    .unwrap_or(false);
                if matches && !replaced {
                    let value =
                        canvas_core::expr::unit_value(num, unit.as_deref()).to_string();
                    *line = format!("{param} = {value}");
                    replaced = true;
                }
            }
            if !replaced {
                return Err(BatchOpError::new(
                    "E-PARAM-UNKNOWN",
                    format!("в тексте ноды {node_id} нет строки параметра '{param}'"),
                ));
            }
            let new_text = lines.join("\n");
            node.text = Some(new_text);
            // Синхронизация снапшота шаблона: формула читает params снапшота,
            // не текст (FR-018) — иначе правка не подействует на расчёт
            if node.template().map(|tpl| tpl.params.contains_key(param)).unwrap_or(false) {
                let mut tpl = node.template().expect("проверено выше");
                tpl.params.insert(
                    param.to_owned(),
                    canvas_core::templates::TemplateParam {
                        num,
                        unit: unit.clone(),
                    },
                );
                node.set_template(Some(tpl));
            }
            let mut e = entry("param_set", false, None);
            e.node_id = Some(node_id);
            Ok(e)
        }
        "node_move" => {
            let key = batch_opt_str(op, "ref")
                .or_else(|| batch_opt_str(op, "id"))
                .ok_or_else(|| BatchOpError::new("E-BAD-OP", "отсутствует поле 'ref'/'id'"))?;
            let x = batch_req_f64(op, "x")? as f32;
            let y = batch_req_f64(op, "y")? as f32;
            let node_id = batch_node_id(canvas, refs, key)?;
            let index = canvas
                .nodes
                .iter()
                .position(|node| node.id == node_id)
                .ok_or_else(|| BatchOpError::new("E-NOT-FOUND", format!("нода не найдена: {node_id}")))?;
            canvas.nodes[index].x = x;
            canvas.nodes[index].y = y;
            let mut e = entry("node_move", false, None);
            e.node_id = Some(node_id);
            Ok(e)
        }
        other => Err(BatchOpError::new(
            "E-BAD-OP",
            format!(
                "неизвестная операция '{other}' (ожидались node_create_note/node_create_file/template_instantiate/edge_create/edge_delete/param_set/node_move)"
            ),
        )),
    }
}

/// Полная карта значений потока (контракт flow_recalc v2, FR-029 п.4):
/// `{node_id: {value, unit, outputs, lines, warnings?, spilled?, error?}}`.
/// Ноды вне потока (проза/файлы) не включаются; ноды с ошибкой — `{error}`.
/// Чистая функция над ДАННЫМИ решениями: базовыми (легаси-обходчик
/// [`mcp_flow_v2`] для тестов) или активным what-if состоянием сцены
/// ([`mcp_flow_state`] — то, что видит пользователь). Детерминизм:
/// порядок нод канваса, отсортированные имена выходов и индексы строк.
fn mcp_flow_map(canvas: &Canvas, solutions: &flow::FlowSolutions) -> serde_json::Value {
    // Построчные значения и именованные выходы, сгруппированные по нодам
    // (BTreeMap — порядок индексов; имена выходов отсортированы —
    // HashMap-порядок недетерминирован)
    let mut lines_by_node: HashMap<String, BTreeMap<usize, canvas_core::expr::Value>> =
        HashMap::new();
    for ((node_id, line), value) in &solutions.lines {
        lines_by_node
            .entry(node_id.clone())
            .or_default()
            .insert(*line, value.clone());
    }
    let mut named_by_node: HashMap<String, BTreeMap<String, canvas_core::expr::Value>> =
        HashMap::new();
    for ((node_id, name), value) in &solutions.named {
        named_by_node
            .entry(node_id.clone())
            .or_default()
            .insert(name.clone(), value.clone());
    }
    let mut nodes = serde_json::Map::new();
    for node in &canvas.nodes {
        let mut entry = match solutions.outputs.get(&node.id) {
            Some(Ok(value)) => serde_json::json!({
                "value": value.num,
                "unit": value.unit.display(),
            }),
            Some(Err(err)) => serde_json::json!({ "error": err.to_string() }),
            None => serde_json::json!({}),
        };
        // Именованные выходы ноды (FR-029): шаблонные — секция outputs,
        // текстовые — переменные Numi-листа
        if let Some(named) = named_by_node.get(&node.id) {
            let outputs: serde_json::Map<String, serde_json::Value> = named
                .iter()
                .map(|(name, value)| {
                    (
                        name.clone(),
                        serde_json::json!({
                            "value": value.num,
                            "unit": value.unit.display(),
                        }),
                    )
                })
                .collect();
            entry["outputs"] = serde_json::Value::Object(outputs);
        }
        // Построчные значения (FR-025/FR-029)
        if let Some(lines) = lines_by_node.get(&node.id) {
            let lines: Vec<serde_json::Value> = lines
                .iter()
                .map(|(index, value)| {
                    serde_json::json!({
                        "index": index,
                        "value": value.num,
                        "unit": value.unit.display(),
                    })
                })
                .collect();
            entry["lines"] = serde_json::Value::Array(lines);
        }
        if let Some(warnings) = solutions.warnings.get(&node.id) {
            entry["warnings"] = serde_json::json!(warnings);
        }
        if entry.as_object().is_some_and(|map| !map.is_empty()) {
            nodes.insert(node.id.clone(), entry);
        }
    }
    let mut result = serde_json::Value::Object(nodes);
    // FR-029: проливание в параметры — агент видит, откуда пришло
    // значение каждого запитанного параметра (источник + адресация
    // порта + эффективное значение строки из пересчёта).
    for node in &canvas.nodes {
        let spills = flow::param_spills(canvas, &node.id);
        if spills.is_empty() {
            continue;
        }
        let spilled: serde_json::Map<String, serde_json::Value> = spills
            .into_iter()
            .map(|spill| {
                let mut item = serde_json::json!({ "from": spill.from_node });
                if let Some(output) = &spill.from_output {
                    item["fromOutput"] = serde_json::json!(output);
                }
                if let Some(line) = spill.from_line {
                    item["fromLine"] = serde_json::json!(line);
                }
                if let Some(value) = spill_edge_value(solutions, &spill) {
                    item["value"] = serde_json::json!(value.num);
                    item["unit"] = serde_json::json!(value.unit.display());
                }
                (spill.param, item)
            })
            .collect();
        let entry = result
            .as_object_mut()
            .expect("карта нод")
            .entry(node.id.clone())
            .or_insert_with(|| serde_json::json!({}));
        entry["spilled"] = serde_json::Value::Object(spilled);
    }
    result
}

/// Значения АКТИВНОГО состояния — СВЕЖИМ пересчётом с подменами
/// активного сценария (тот же источник подмен, что у recompute_flow:
/// active_whatif_overrides). Не читает кэш сцены: ленивые мутации
/// (node_edit с text — CR-012) не поднимают пересчёт, агенту нужен
/// актуальный снимок; кэши сцены и revision не затрагиваются (чистая
/// функция). Цикл потока — Err (значений нет). + авто-строки FR-050 Р-4
/// из того же пересчёта (порядок строк — canvas.edges, инвариант 2).
fn mcp_flow_active_fresh(scene: &SceneState) -> Result<serde_json::Value, String> {
    let (whatif, _stale) = scene.active_whatif_overrides();
    let solutions =
        flow::propagate_with_lines(&scene.canvas, &whatif).map_err(|cycle| cycle.to_string())?;
    let mut result = mcp_flow_map(&scene.canvas, &solutions);
    for node in &scene.canvas.nodes {
        let rows = flow::auto_rows(&scene.canvas, &node.id, &solutions);
        if rows.is_empty() {
            continue;
        }
        let entry = result
            .as_object_mut()
            .expect("карта нод")
            .entry(node.id.clone())
            .or_insert_with(|| serde_json::json!({}));
        entry["autoRows"] = serde_json::Value::Array(mcp_auto_rows_json(&rows));
    }
    Ok(result)
}

/// FR-050 Р-4: авто-строка приёмника для агента — слот, ребро-источник
/// истины, путь «Объект.Поле»; value|unit или unmapped («не
/// подставлено» — агент видит проблему, как пользователь в тултипе).
fn mcp_auto_rows_json(rows: &[flow::AutoRow]) -> Vec<serde_json::Value> {
    rows.iter()
        .map(|row| {
            let mut item = serde_json::json!({
                "slot": row.slot,
                "edge": row.edge_id,
                "path": row.path,
                "field": row.field,
            });
            match &row.value {
                Some(value) => {
                    item["value"] = serde_json::json!(value.num);
                    item["unit"] = serde_json::json!(value.unit.display());
                }
                None => {
                    item["unmapped"] = serde_json::json!(true);
                }
            }
            item
        })
        .collect()
}

/// Легаси-обходчик: БАЗОВЫЙ пересчёт без what-if подмен (сравнение базы,
/// тесты). Активное состояние агента — mcp_dispatch "flow_recalc"
/// (см. mcp_flow_active_fresh).
pub fn mcp_flow_v2(canvas: &Canvas) -> serde_json::Value {
    let solutions = match flow::propagate_with_lines(canvas, &flow::WhatIfOverrides::default()) {
        Ok(solutions) => solutions,
        Err(cycle) => {
            return serde_json::json!({ "error": format!("цикл потока значений: {cycle}") })
        }
    };
    mcp_flow_map(canvas, &solutions)
}

/// FR-033 п.2: транзакционное применение батча. Ошибки схемы/лимитов —
/// Err (isError); ошибка ОПЕРАЦИИ — структурированный ответ {ok:false} при
/// нетронутом канвасе; успех — один undo-шаг, spatial, dirty (автосейв),
/// полный пересчёт потока и карта значений в ответе.
fn mcp_graph_apply(
    scene: &mut SceneState,
    templates: &canvas_core::templates::TemplateRegistry,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let Some(ops) = params
        .get("operations")
        .and_then(serde_json::Value::as_array)
    else {
        return Err("отсутствует параметр 'operations' (массив операций)".to_owned());
    };
    if ops.is_empty() {
        return Err("operations: минимум 1 операция".to_owned());
    }
    if ops.len() > GRAPH_APPLY_MAX_OPS {
        return Err(format!(
            "operations: не более {GRAPH_APPLY_MAX_OPS} операций, получено {}",
            ops.len()
        ));
    }
    let new_nodes = ops
        .iter()
        .filter(|op| {
            matches!(
                op.get("op").and_then(serde_json::Value::as_str),
                Some("node_create_note" | "node_create_file" | "template_instantiate")
            )
        })
        .count();
    if new_nodes > GRAPH_APPLY_MAX_NODES {
        return Err(format!(
            "батч создаёт {new_nodes} нод — лимит {GRAPH_APPLY_MAX_NODES}"
        ));
    }

    // Транзакция: операции применяются к клону; любая ошибка — клон
    // отбрасывается, оригинал байт-в-байт прежний (инвариант атомарности)
    let mut canvas = scene.canvas.clone();
    let mut refs: HashMap<String, String> = HashMap::new();
    let mut entries: Vec<BatchEntry> = Vec::new();
    for (op_index, op) in ops.iter().enumerate() {
        match batch_apply_op(&mut canvas, &mut refs, templates, op_index, op) {
            Ok(entry) => entries.push(entry),
            Err(err) => return Ok(graph_apply_error(op_index, &err)),
        }
    }

    // Успех: ровно ОДИН undo-шаг на весь батч (FR-033 п.2г)
    let before = std::mem::replace(&mut scene.canvas, canvas);
    scene.push_undo(before);
    // Индексы изменились — spatial перестраивается (паттерн node_delete)
    scene.spatial = SpatialIndex::build(&scene.canvas);
    scene.mark_dirty();
    scene.recompute_flow();

    let created: Vec<serde_json::Value> = entries
        .iter()
        .filter(|e| e.created)
        .map(|e| {
            let mut item = serde_json::json!({ "op_index": e.op_index });
            let map = item.as_object_mut().expect("json object");
            if let Some(ref_name) = &e.ref_name {
                map.insert("ref".into(), serde_json::json!(ref_name));
            }
            if let Some(node_id) = &e.node_id {
                map.insert("node_id".into(), serde_json::json!(node_id));
            }
            if let Some(edge_id) = &e.edge_id {
                map.insert("edge_id".into(), serde_json::json!(edge_id));
            }
            item
        })
        .collect();
    let report: Vec<serde_json::Value> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "op_index": e.op_index,
                "op": e.op,
                "id": e.object_id(),
            })
        })
        .collect();
    // FR-050/MCP-parity: flow — СВЕЖИЙ пересчёт АКТИВНОГО состояния
    // (what-if подмены, авто-строки) — те же значения, что видит
    // пользователь; цикл потока не роняет успешный батч — честный error
    // в поле flow (как раньше)
    let flow = match mcp_flow_active_fresh(scene) {
        Ok(flow) => flow,
        Err(cycle) => serde_json::json!({ "error": cycle }),
    };
    Ok(serde_json::json!({
        "ok": true,
        "created": created,
        "report": report,
        "flow": flow,
    }))
}
