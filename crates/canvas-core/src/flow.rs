//! FR-014: поток значений по рёбрам — DAG-движок и live-ревал.
//!
//! Чистый Rust, без I/O и глобального состояния (инвариант тестируемости
//! FR-014): [`topo_sort`] — `(&Canvas) -> Result<Vec<usize>, CycleError>`,
//! [`propagate`] — `(&Canvas, &HashMap<String, Value>) ->
//! Result<FlowOutputs, CycleError>`, 0 side-эффектов.
//!
//! Семантика:
//! - поток значений идёт только по рёбрам с `flow.kind = "value"`
//!   ([`FlowKind::Value`]); контрольные рёбра — визуальные связи, в граф
//!   не входят;
//! - граф value-рёбер обязан быть DAG — цикл это [`CycleError`] со списком
//!   участников (SCC-поиск поверх алгоритма Кана);
//! - в пересчёте участвуют формульные ноды: явная формула `canvasdesk.expr`
//!   (FR-013, MCP) ИЛИ текст заметки в Numi-стиле — значение text-ноды =
//!   последняя формульная строка (семантика итога FR-013); проза значения
//!   не даёт — вход для downstream «отсутствует»;
//! - значение ноды считается в окружении со входами (`$in`, `$1..$N`);
//! - входящие value-рёбра ноды нумеруются в порядке `canvas.edges`:
//!   `$1` — первое, `$N` — N-е; `$in` требует ровно одно входящее ребро;
//! - `overrides` (what-if, FR-017): подменённое значение ноды используется
//!   вместо её формулы — сама нода и весь downstream видят override;
//! - результат (`FlowOutputs`) — runtime-данные: НЕ сериализуется в
//!   `.canvas`, источник истины — формулы + топология рёбер.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::expr::{self, Env, EvalError, ExprOutcome, Value};
use crate::model::Canvas;
use crate::templates::OutputSource;

/// Тип потока ребра (FR-014): контрольная связь (по умолчанию — обратная
/// совместимость со старыми `.canvas`) или поток значений.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    /// Визуальная связь без потока значений (существующее поведение рёбер).
    Control,
    /// Ребро переносит значение источника в `$in`/`$1..$N` приёмника.
    Value,
}

impl FlowKind {
    /// Разбор значения `canvasdesk.flow.kind`; неизвестное — Control.
    pub fn from_kind_str(kind: &str) -> Self {
        match kind {
            "value" => Self::Value,
            _ => Self::Control,
        }
    }

    /// Строковое значение для `canvasdesk.flow.kind`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Value => "value",
        }
    }
}

/// Ошибка топологии: цикл из value-рёбер. `nodes` — участники цикла
/// (id нод, отсортированы) — пользователю показывается для отладки.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("цикл потока значений: {}", nodes.join(" → "))]
pub struct CycleError {
    pub nodes: Vec<String>,
}

/// Результат пересчёта графа: `node_id` — значение формулы или ошибка
/// вычисления (часть downstream может ошибаться без падения всего графа).
/// Ноды без формулы записи не получают.
pub type FlowOutputs = HashMap<String, Result<Value, EvalError>>;

/// Топологический порядок нод по value-рёбрам (алгоритм Кана). В граф
/// входят только рёбра [`FlowKind::Value`] с существующими концами; порядок
/// детерминирован (очередь по возрастанию индексов). Цикл —
/// `Err(CycleError)` с участниками (SCC размера > 1 и петли).
pub fn topo_sort(canvas: &Canvas) -> Result<Vec<usize>, CycleError> {
    let n = canvas.nodes.len();
    let mut index_of: HashMap<&str, usize> = HashMap::with_capacity(n);
    for (i, node) in canvas.nodes.iter().enumerate() {
        index_of.insert(node.id.as_str(), i);
    }
    // Граф value-рёбер: смежность + полустепени захода. Висячие рёбра
    // (конец не в canvas.nodes) пропускаются — они не создают циклов.
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indegree = vec![0usize; n];
    let mut self_loops: Vec<usize> = Vec::new();
    for edge in &canvas.edges {
        if edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let (Some(&from), Some(&to)) = (
            index_of.get(edge.from_node.as_str()),
            index_of.get(edge.to_node.as_str()),
        ) else {
            continue;
        };
        if from == to {
            // Петля — цикл из одного участника; нода блокируется в Кане
            // (полустепень захода никогда не станет 0)
            self_loops.push(from);
            indegree[from] += 1;
            continue;
        }
        adj[from].push(to);
        indegree[to] += 1;
    }
    // Кан: очередь заполняется по возрастанию индексов — детерминизм
    let mut queue: VecDeque<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(i) = queue.pop_front() {
        order.push(i);
        for &j in &adj[i] {
            indegree[j] -= 1;
            if indegree[j] == 0 {
                queue.push_back(j);
            }
        }
    }
    if order.len() == n {
        return Ok(order);
    }
    // Остаток — циклы и их окрестности: участники — SCC размера > 1 и петли
    Err(CycleError {
        nodes: cycle_participants(canvas, n, &adj, &self_loops),
    })
}

/// Участники циклов value-графа: ноды SCC размера > 1 (Тарьян) + петли.
/// id сортируются — детерминизм и читаемый список.
fn cycle_participants(
    canvas: &Canvas,
    n: usize,
    adj: &[Vec<usize>],
    self_loops: &[usize],
) -> Vec<String> {
    // Тарьян (итеративный — глубина графа не ограничена стеком вызовов)
    let mut indices: Vec<Option<usize>> = vec![None; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut counter = 0usize;
    let mut participants: Vec<String> = Vec::new();

    // Кадр: (нода, позиция обхода смежности)
    let mut frames: Vec<(usize, usize)> = Vec::new();
    for root in 0..n {
        if indices[root].is_some() {
            continue;
        }
        frames.push((root, 0));
        while let Some(&(v, it)) = frames.last() {
            if it == 0 {
                indices[v] = Some(counter);
                low[v] = counter;
                counter += 1;
                stack.push(v);
                on_stack[v] = true;
            }
            if it < adj[v].len() {
                let w = adj[v][it];
                if let Some(frame) = frames.last_mut() {
                    frame.1 += 1;
                }
                match indices[w] {
                    None => frames.push((w, 0)),
                    Some(iw) if on_stack[w] => low[v] = low[v].min(iw),
                    Some(_) => {}
                }
            } else {
                frames.pop();
                if let Some(&(parent, _)) = frames.last() {
                    low[parent] = low[parent].min(low[v]);
                }
                if low[v] == indices[v].unwrap_or(0) {
                    // Корень SCC: компонента — верх стека до v включительно
                    let mut size = 0usize;
                    let mut component: Vec<usize> = Vec::new();
                    while let Some(w) = stack.pop() {
                        on_stack[w] = false;
                        component.push(w);
                        size += 1;
                        if w == v {
                            break;
                        }
                    }
                    if size > 1 {
                        component.sort_unstable();
                        participants.extend(
                            component
                                .iter()
                                .filter_map(|&i| canvas.nodes.get(i).map(|node| node.id.clone())),
                        );
                    }
                }
            }
        }
    }
    // Петли — участники из одного элемента
    for &i in self_loops {
        if let Some(node) = canvas.nodes.get(i) {
            participants.push(node.id.clone());
        }
    }
    participants.sort();
    participants.dedup();
    participants
}

/// Живой пересчёт всего графа (чистая функция, live-ревал FR-014).
///
/// Для каждой ноды в топологическом порядке: собрать значения входящих
/// value-рёбер (в порядке `canvas.edges`), вычислить `canvasdesk.expr` в
/// окружении со входами. Нода без формулы значения не даёт; ошибка формулы
/// не прерывает пересчёт — downstream этой ноды получает «вход
/// отсутствует». Возвращает карту результатов всех формульных нод.
///
/// `overrides` — подмена значений нод по id (what-if, FR-017): нода
/// получает override как своё значение, downstream видит его же.
pub fn propagate(
    canvas: &Canvas,
    overrides: &HashMap<String, Value>,
) -> Result<FlowOutputs, CycleError> {
    propagate_with_lines(canvas, overrides).map(|solutions| solutions.outputs)
}

/// FR-025: построчные выходы Numi-листов — значение каждой формульной
/// строки: `(id ноды, индекс строки) → Value`. Заполняется в
/// [`propagate_with_lines`] (тот же обход и то же окружение, что у
/// значения ноды — строки видят входы value-рёбер). Ошибки строк и проза
/// значений не дают (слот деградирует до `None`).
pub type LineOutputs = HashMap<(String, usize), Value>;

/// FR-025: полный результат пересчёта — значения нод ([`FlowOutputs`])
/// плюс построчные выходы Numi-листов ([`LineOutputs`]).
#[derive(Debug, Clone, Default)]
pub struct FlowSolutions {
    /// Значения нод (как в [`propagate`]).
    pub outputs: FlowOutputs,
    /// Значения формульных строк текстовых нод (FR-025).
    pub lines: LineOutputs,
    /// FR-029 (минимальный контур R1): значения именованных выходов
    /// шаблонных нод — `(id ноды, имя выхода) → Value` (по снапшоту
    /// `canvasdesk.template.outputs`; выход с ошибкой/невычислимый
    /// отсутствует — тихая деградация, как у строк FR-025).
    pub named: HashMap<(String, String), Value>,
}

/// [`propagate`] с построчными выходами (FR-025): для текстовых Numi-листов
/// дополнительно собирает значение каждой формульной строки. Нумерация
/// строк — индекс строки ТЕКСТА ноды (тот же, что в `ExprLineResults`
/// FR-013 и в бейджах результатов рендера). FR-025 (правка 2, по проверке
/// владельца): шаблонные ноды тоже дают построчные выходы — их текст (лист
/// параметров FR-018) вычисляется наравне с заметками, узловое значение
/// остаётся у формулы шаблона (футер, `line = None`).
pub fn propagate_with_lines(
    canvas: &Canvas,
    overrides: &HashMap<String, Value>,
) -> Result<FlowSolutions, CycleError> {
    let order = topo_sort(canvas)?;
    let mut solutions = FlowSolutions::default();
    for index in order {
        let node = &canvas.nodes[index];
        let id = &node.id;
        // What-if: подменённое значение заменяет формулу целиком
        if let Some(value) = overrides.get(id) {
            solutions.outputs.insert(id.clone(), Ok(value.clone()));
            continue;
        }
        let slots = inbound_slots_with_lines(
            canvas,
            id,
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        let env = if slots.is_empty() {
            Env::empty()
        } else {
            Env::with_inbound(slots)
        };
        // FR-018: у шаблонной ноды параметры (`canvasdesk.template.params`)
        // входят в окружение как `$имя`; формула — снимок из template-ссылки
        // (приоритет над `canvasdesk.expr` — шаблон определяет расчёт).
        let template = node.template();
        let mut env = match &template {
            Some(tpl) => env.with_param_map(tpl.param_values()),
            None => env,
        };
        // FR-029 (минимальный контур R1): проливание value-рёбер с
        // `to_param` — значение ребра подставляется как `$<параметр>`
        // приёмника ПОВЕРХ локальных параметров («проливание сильнее
        // дефолта»). Несколько рёбер в один `to_param` — побеждает
        // последнее по порядку `canvas.edges` (детерминизм, FR-029);
        // строгая диагностика дубля — graph_validate (FR-032).
        for edge in canvas.edges.iter() {
            let Some(param) = edge.to_param.as_deref() else {
                continue;
            };
            if edge.to_node != *id || edge.flow_kind() != FlowKind::Value {
                continue;
            }
            if let Some(value) = edge_carry_value(edge, &solutions) {
                env = env.set_param(param.to_owned(), value);
            }
        }
        // FR-025 (правка 2): значение КАЖДОЙ формульной строки текста —
        // кандидат построчной точки выхода, теперь и у шаблонных нод
        // (лист параметров — присваивания со значениями). Ошибки строк
        // и проза значений не дают.
        let line_outcomes = expr::eval_lines_in(&node.text.clone().unwrap_or_default(), &env);
        for (line_index, line_outcome) in line_outcomes.iter().enumerate() {
            if let Some(ExprOutcome::Ok(value)) = line_outcome {
                solutions
                    .lines
                    .insert((id.clone(), line_index), value.clone());
            }
        }
        // Значение ноды: шаблонная формула (FR-018), явная формула
        // `canvasdesk.expr` (MCP) или — для обычных заметок — последняя
        // формульная строка Numi-листа (FR-013: «итог заметки — последняя
        // формульная строка»; живой UI-путь: пользователь пишет
        // «1200 + 480» в заметке и тянет value-ребро). Проза/пустой текст
        // значения не дают — нода не участвует в потоке.
        let outcome = match &template {
            Some(tpl) => expr::parse(&tpl.expr)
                .map_err(|err| EvalError::BadFormula(err.to_string()))
                .and_then(|parsed| expr::eval(&parsed, &env)),
            None => match node.expr() {
                Some(formula) => expr::parse(formula)
                    .map_err(|err| EvalError::BadFormula(err.to_string()))
                    .and_then(|parsed| expr::eval(&parsed, &env)),
                None => {
                    let last = line_outcomes.into_iter().flatten().last();
                    match last {
                        Some(ExprOutcome::Ok(value)) => Ok(value),
                        Some(ExprOutcome::Err(msg)) => Err(EvalError::BadFormula(msg)),
                        None => continue,
                    }
                }
            },
        };
        solutions.outputs.insert(id.clone(), outcome);
        // FR-029 (минимальный контур R1): именованные выходы шаблонной
        // ноды — по снапшоту outputs: `Line(i)` — уже посчитанная строка;
        // `Expr` — подвыражение в финальном окружении ноды (включая
        // проливание). Ошибка вычисления — выход отсутствует (тихая
        // деградация, как у строк FR-025).
        if let Some(tpl) = &template {
            for spec in &tpl.outputs {
                let value = match &spec.source {
                    OutputSource::Line(line) => solutions.lines.get(&(id.clone(), *line)).cloned(),
                    OutputSource::Expr(formula) => expr::parse(formula)
                        .ok()
                        .and_then(|parsed| expr::eval(&parsed, &env).ok()),
                };
                if let Some(value) = value {
                    solutions
                        .named
                        .insert((id.clone(), spec.name.clone()), value);
                }
            }
        }
    }
    Ok(solutions)
}

/// FR-029: значение, которое value-ребро уносит в приёмник. Приоритет
/// адресации: `from_line` (индекс строки, FR-025) → `from_output`
/// (именованный выход, FR-029 — по снапшоту outputs истока) → узловое
/// значение. Неразрешимая адресация/ошибка источника — `None` (слот
/// «значения нет», тихая деградация — согласована с FR-025).
fn edge_carry_value(edge: &crate::model::Edge, solutions: &FlowSolutions) -> Option<Value> {
    if let Some(line) = edge.from_line {
        return solutions
            .lines
            .get(&(edge.from_node.clone(), line))
            .cloned();
    }
    if let Some(output) = edge.from_output.as_deref() {
        return solutions
            .named
            .get(&(edge.from_node.clone(), output.to_owned()))
            .cloned();
    }
    solutions
        .outputs
        .get(&edge.from_node)
        .and_then(|result| result.as_ref().ok())
        .cloned()
}

/// Значения входящих value-рёбер ноды (в порядке `canvas.edges`) по карте
/// результатов: `Some(Some(v))` — значение источника; `Some(None)` — ребро
/// есть, значения нет (источник без формулы, с ошибкой или висячее ребро).
/// Слоты НЕ схлопываются — индексы `$1..$N` стабильны.
pub fn inbound_slots(canvas: &Canvas, node_id: &str, outputs: &FlowOutputs) -> Vec<Option<Value>> {
    inbound_slots_with_lines(
        canvas,
        node_id,
        outputs,
        &LineOutputs::new(),
        &HashMap::new(),
    )
}

/// FR-025: [`inbound_slots`] с построчными выходами: ребро с
/// `from_line = Some(i)` уносит значение строки `i` источника; строка
/// удалена/стала прозой/ошибка — слот `Some(None)` (тихая деградация,
/// согласована с принципом тишины прозы Numi). Ребро без `from_line` —
/// значение ноды целиком (текущее поведение, инвариант флага FR-025).
/// FR-029: ребро с `from_output` уносит значение именованного выхода
/// истока (после `from_line` в приоритете адресации).
pub fn inbound_slots_with_lines(
    canvas: &Canvas,
    node_id: &str,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
    named: &HashMap<(String, String), Value>,
) -> Vec<Option<Value>> {
    canvas
        .edges
        .iter()
        .filter(|edge| edge.to_node == node_id && edge.flow_kind() == FlowKind::Value)
        .map(|edge| {
            if let Some(line) = edge.from_line {
                return lines.get(&(edge.from_node.clone(), line)).cloned();
            }
            if let Some(output) = edge.from_output.as_deref() {
                return named
                    .get(&(edge.from_node.clone(), output.to_owned()))
                    .cloned();
            }
            outputs
                .get(&edge.from_node)
                .and_then(|result| result.as_ref().ok())
                .cloned()
        })
        .collect()
}

/// Путь по value-рёбрам от `start` до `goal` (включая концы) — участники
/// цикла при добавлении ребра `goal → start`. `None` — пути нет, ребро
/// цикл не замыкает. Поиск в ширину; путь детерминирован (первый найденный
/// в порядке `canvas.edges`).
pub fn value_path(canvas: &Canvas, start: &str, goal: &str) -> Option<Vec<String>> {
    if canvas.node(start).is_none() || canvas.node(goal).is_none() {
        return None;
    }
    // parents: нода → откуда пришли
    let mut parents: HashMap<String, String> = HashMap::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();
    queue.push_back(start.to_owned());
    visited.insert(start.to_owned());
    while let Some(current) = queue.pop_front() {
        if current == goal {
            // Восстановить путь start → … → goal
            let mut path = vec![goal.to_owned()];
            let mut node = goal;
            while node != start {
                let parent = parents.get(node)?;
                path.push(parent.clone());
                node = parent;
            }
            path.reverse();
            return Some(path);
        }
        for edge in &canvas.edges {
            if edge.flow_kind() != FlowKind::Value || edge.from_node != current {
                continue;
            }
            let next = &edge.to_node;
            if visited.insert(next.clone()) {
                parents.insert(next.clone(), current.clone());
                queue.push_back(next.clone());
            }
        }
    }
    None
}

/// Замкнёт ли новое value-ребро `from → to` цикл (быстрый предикат для UI
/// и MCP поверх [`value_path`]).
pub fn creates_value_cycle(canvas: &Canvas, from: &str, to: &str) -> bool {
    value_path(canvas, to, from).is_some()
}

/// Сериализация результатов для отображения/MCP (FR-014): id ноды →
/// отображаемое значение (`Ok`) или текст ошибки (`Err`).
pub fn outputs_display(outputs: &FlowOutputs) -> HashMap<String, Result<String, String>> {
    outputs
        .iter()
        .map(|(id, result)| {
            let mapped = match result {
                Ok(value) => Ok(value.to_string()),
                Err(err) => Err(err.to_string()),
            };
            (id.clone(), mapped)
        })
        .collect()
}

// --- Тесты (верификационный список FR-014 + регрессии топологии) ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Edge, Node};
    use std::str::FromStr;

    /// Сцена: ноды с формулами + value-ребро.
    fn node_with_expr(canvas: &mut Canvas, id: &str, formula: &str, x: f32) {
        let mut node = Node::text(id, id, x, 0.0);
        node.set_expr(Some(formula.to_owned()));
        canvas.nodes.push(node);
    }

    fn value_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        canvas.add_edge(edge);
    }

    fn control_edge(canvas: &mut Canvas, id: &str, from: &str, to: &str) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Control);
        canvas.add_edge(edge);
    }

    /// Верификация FR-014: пустой граф — пустой порядок.
    #[test]
    fn topo_sort_empty_canvas() {
        let canvas = Canvas::default();
        assert_eq!(topo_sort(&canvas), Ok(Vec::new()));
    }

    /// Изолированные ноды без рёбер — порядок по индексам (детерминизм Кана).
    #[test]
    fn topo_sort_no_edges() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "b", "2", 0.0);
        node_with_expr(&mut canvas, "a", "1", 1.0);
        assert_eq!(topo_sort(&canvas), Ok(vec![0, 1]));
    }

    /// Верификация FR-014: цепочка A→B→C (value) — [A, B, C].
    #[test]
    fn topo_sort_chain() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        assert_eq!(topo_sort(&canvas), Ok(vec![0, 1, 2]));
    }

    /// Верификация FR-014: цикл A→B→A — Err с участниками ["A", "B"].
    #[test]
    fn topo_sort_cycle_two_nodes() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "A");
        let err = topo_sort(&canvas).expect_err("цикл");
        assert_eq!(err.nodes, vec!["A".to_owned(), "B".to_owned()]);
    }

    /// Цикл из трёх участников + downstream-нода за циклом — в участники
    /// downstream НЕ входит (она не на цикле).
    #[test]
    fn topo_sort_cycle_three_with_downstream() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        node_with_expr(&mut canvas, "C", "3", 2.0);
        node_with_expr(&mut canvas, "D", "4", 3.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        value_edge(&mut canvas, "e3", "C", "A");
        value_edge(&mut canvas, "e4", "C", "D");
        let err = topo_sort(&canvas).expect_err("цикл A→B→C→A");
        assert_eq!(
            err.nodes,
            vec!["A".to_owned(), "B".to_owned(), "C".to_owned()]
        );
    }

    /// Петля A→A — цикл с единственным участником.
    #[test]
    fn topo_sort_self_loop() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        value_edge(&mut canvas, "e1", "A", "A");
        value_edge(&mut canvas, "e2", "A", "B");
        let err = topo_sort(&canvas).expect_err("петля");
        assert_eq!(err.nodes, vec!["A".to_owned()]);
    }

    /// Контрольные рёбра НЕ создают циклов (визуальные связи свободны).
    #[test]
    fn topo_sort_control_edges_ignore_cycles() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        control_edge(&mut canvas, "e1", "A", "B");
        control_edge(&mut canvas, "e2", "B", "A");
        let order = topo_sort(&canvas).expect("контрольные рёбра не создают циклов");
        assert_eq!(order.len(), 2);
    }

    /// Верификация FR-014: ромб A→B, A→C, B→D, C→D — D последняя.
    #[test]
    fn topo_sort_diamond() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        node_with_expr(&mut canvas, "C", "3", 2.0);
        node_with_expr(&mut canvas, "D", "4", 3.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "A", "C");
        value_edge(&mut canvas, "e3", "B", "D");
        value_edge(&mut canvas, "e4", "C", "D");
        let order = topo_sort(&canvas).expect("DAG");
        assert_eq!(order.last(), Some(&3), "D — последняя");
        assert_eq!(order.len(), 4);
    }

    /// Висячее value-ребро (конец не существует) не входит в граф.
    #[test]
    fn topo_sort_dangling_value_edge_ignored() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        value_edge(&mut canvas, "e1", "A", "ghost");
        assert_eq!(topo_sort(&canvas), Ok(vec![0]));
    }

    /// Верификация FR-014: цепочка A=5, B=$in × 2, C=$in + 1 → {5, 10, 11}.
    #[test]
    fn propagate_chain() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(5.0));
        assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(10.0));
        assert_eq!(outputs["C"].as_ref().unwrap(), &Value::scalar(11.0));
    }

    /// Слияние потоков: C = $1 + $2 по двум входам (порядок — canvas.edges).
    #[test]
    fn propagate_merge_slots() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "3", 0.0);
        node_with_expr(&mut canvas, "B", "7", 1.0);
        node_with_expr(&mut canvas, "C", "$1 + $2", 2.0);
        value_edge(&mut canvas, "e1", "A", "C");
        value_edge(&mut canvas, "e2", "B", "C");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["C"].as_ref().unwrap(), &Value::scalar(10.0));
    }

    /// Контрольное ребро не переносит значение — вход отсутствует.
    #[test]
    fn propagate_control_edge_carries_nothing() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        control_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(5.0));
        assert!(matches!(
            outputs["B"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
    }

    /// Верификация FR-014: вход отсутствует — ошибка downstream, не падение
    /// (A удалена; висячее ребро даёт пустой слот).
    #[test]
    fn propagate_missing_inbound_is_per_node_error() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        node_with_expr(&mut canvas, "C", "$in + 1", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        // Удаляем A (каскадно уходит и ребро e1)
        canvas.remove_node(0);
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert!(matches!(
            outputs["B"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
        assert!(matches!(
            outputs["C"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
    }

    /// Источник БЕЗ формулы (проза) — значение не даёт, downstream
    /// получает MissingInbound; сам источник в результатах отсутствует.
    #[test]
    fn propagate_prose_source_has_no_value() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("A", "Встреча в 15:00", 0.0, 0.0));
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert!(!outputs.contains_key("A"), "проза не участвует");
        assert!(matches!(
            outputs["B"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
    }

    /// Живой UI-путь FR-014: заметки с Numi-формулами в тексте (без
    /// `canvasdesk.expr`) участвуют в потоке — итог = последняя строка.
    #[test]
    fn propagate_text_source_line_formula() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("A", "1200 + 480", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "$in / 3", 0.0, 1.0));
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(1680.0));
        assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(560.0));
    }

    /// Многострочный текст: проза не считается, итог — последняя строка.
    #[test]
    fn propagate_text_last_formula_line_is_value() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("A", "смета:\n100\n200 × 2", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "$in - 30", 0.0, 1.0));
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(400.0));
        assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(370.0));
    }

    /// Ошибка построчной формулы источника — downstream видит «вход
    /// отсутствует» (ошибочный слот), а не значение. Построчные ошибки
    /// обоих нод приходят обёрнутыми в BadFormula (единый вариант графа).
    #[test]
    fn propagate_text_error_breaks_downstream() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("A", "1 / 0", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "$in × 2", 0.0, 1.0));
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert!(matches!(
            outputs["A"],
            Err(EvalError::BadFormula(ref msg)) if msg.contains("деление на ноль")
        ));
        assert!(matches!(
            outputs["B"],
            Err(EvalError::BadFormula(ref msg)) if msg.contains("вход отсутствует")
        ));
    }

    /// Явная формула `canvasdesk.expr` приоритетнее текста (совместимость).
    #[test]
    fn propagate_explicit_expr_beats_text() {
        let mut canvas = Canvas::default();
        let mut a = Node::text("A", "1200 + 480", 0.0, 0.0);
        a.set_expr(Some("7".to_owned()));
        canvas.nodes.push(a);
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(7.0));
    }

    /// Ошибка формулы источника → downstream «вход отсутствует», остальной
    /// граф считается.
    #[test]
    fn propagate_source_error_does_not_stop_graph() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5 ms + 3 rps", 0.0); // UnitMismatch
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        node_with_expr(&mut canvas, "solo", "7", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert!(outputs["A"].is_err(), "источник с ошибкой");
        assert!(matches!(
            outputs["B"],
            Err(EvalError::MissingInbound { index: 0 })
        ));
        assert_eq!(outputs["solo"].as_ref().unwrap(), &Value::scalar(7.0));
    }

    /// Цикл value-рёбер — propagate возвращает ту же CycleError.
    #[test]
    fn propagate_cycle_is_error() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "A");
        let err = propagate(&canvas, &HashMap::new()).expect_err("цикл");
        assert_eq!(err.nodes, vec!["A".to_owned(), "B".to_owned()]);
    }

    /// What-if (FR-017): override значения ноды меняет её и downstream;
    /// формула ноды с override не вычисляется.
    #[test]
    fn propagate_overrides_flow_downstream() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let mut overrides = HashMap::new();
        overrides.insert("A".to_owned(), Value::scalar(100.0));
        let outputs = propagate(&canvas, &overrides).expect("DAG");
        assert_eq!(outputs["A"].as_ref().unwrap(), &Value::scalar(100.0));
        assert_eq!(outputs["B"].as_ref().unwrap(), &Value::scalar(200.0));
    }

    /// Unit-значения текут по рёбрам: 1000 rps → $in / 4 → 250 rps.
    #[test]
    fn propagate_units_flow() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1000 rps", 0.0);
        node_with_expr(&mut canvas, "B", "$in / 4", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let outputs = propagate(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(outputs["B"].as_ref().unwrap().to_string(), "250 rps");
    }

    /// Верификация FR-014: round-trip flow.kind + Control по умолчанию.
    #[test]
    fn edge_flow_kind_round_trip() {
        let mut edge = Edge::new("e1", "A", None, "B", None);
        assert_eq!(edge.flow_kind(), FlowKind::Control, "дефолт — control");
        edge.set_flow_kind(FlowKind::Value);
        assert_eq!(edge.flow_kind(), FlowKind::Value);
        // Serialize → deserialize
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("A", "A", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "B", 1.0, 0.0));
        canvas.add_edge(edge);
        let json = canvas.to_json().expect("сериализация");
        let restored = Canvas::from_str(&json).expect("парсинг");
        assert_eq!(restored.edges[0].flow_kind(), FlowKind::Value);
        // Сброс в Control убирает поле целиком (чистый round-trip)
        let mut restored_edge = restored.edges[0].clone();
        restored_edge.set_flow_kind(FlowKind::Control);
        assert_eq!(restored_edge.flow_kind(), FlowKind::Control);
        assert!(
            restored_edge.extra.get("canvasdesk").is_none(),
            "пустое расширение не хранится"
        );
    }

    /// Соседние неизвестные поля `canvasdesk` ребра не теряются при тогле.
    #[test]
    fn edge_flow_kind_preserves_siblings() {
        let mut edge = Edge::new("e1", "A", None, "B", None);
        edge.extra.insert(
            "canvasdesk".to_owned(),
            serde_json::json!({ "note": "моё", "flow": { "kind": "control" } }),
        );
        edge.set_flow_kind(FlowKind::Value);
        assert_eq!(edge.flow_kind(), FlowKind::Value);
        let note = edge
            .extra
            .get("canvasdesk")
            .and_then(|ext| ext.get("note"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(note, Some("моё"), "чужие поля canvasdesk сохранены");
        edge.set_flow_kind(FlowKind::Control);
        let note = edge
            .extra
            .get("canvasdesk")
            .and_then(|ext| ext.get("note"))
            .and_then(serde_json::Value::as_str);
        assert_eq!(note, Some("моё"), "и после сброса flow");
    }

    /// value_path: цикл A→B→C + попытка C→A — путь A→B→C.
    #[test]
    fn value_path_detects_cycle_participants() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "2", 1.0);
        node_with_expr(&mut canvas, "C", "3", 2.0);
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        assert_eq!(
            value_path(&canvas, "A", "C"),
            Some(vec!["A".to_owned(), "B".to_owned(), "C".to_owned()])
        );
        assert!(creates_value_cycle(&canvas, "C", "A"), "C→A замкнёт цикл");
        assert!(
            !creates_value_cycle(&canvas, "A", "C"),
            "A→C — расширение существующего пути, не цикл"
        );
        // Контрольные рёбра не участвуют в путях: D→E только control —
        // value-пути E→…→D нет
        node_with_expr(&mut canvas, "D", "4", 3.0);
        node_with_expr(&mut canvas, "E", "5", 4.0);
        control_edge(&mut canvas, "e3", "D", "E");
        assert!(!creates_value_cycle(&canvas, "E", "D"));
    }

    /// Display CycleError — цепочка участников через « → ».
    #[test]
    fn cycle_error_display() {
        let err = CycleError {
            nodes: vec!["A".to_owned(), "B".to_owned()],
        };
        assert_eq!(err.to_string(), "цикл потока значений: A → B");
    }

    /// outputs_display: значения и ошибки в строках (для MCP/рендера).
    #[test]
    fn outputs_display_maps_to_strings() {
        let mut outputs: FlowOutputs = HashMap::new();
        outputs.insert("A".to_owned(), Ok(Value::scalar(5.0)));
        outputs.insert("B".to_owned(), Err(EvalError::MissingInbound { index: 0 }));
        let display = outputs_display(&outputs);
        assert_eq!(display["A"].as_ref().unwrap(), "5");
        assert!(display["B"].as_ref().unwrap_err().contains("вход"));
    }

    // --- FR-025: построчные точки выхода (значение строки в потоке) ---

    /// FR-025: propagate_with_lines собирает значение КАЖДОЙ формульной
    /// строки Numi-листа (присваивания и выражения); проза и ошибки — нет.
    #[test]
    fn propagate_collects_line_outputs() {
        let mut canvas = Canvas::default();
        let mut node = Node::text("A", "", 0.0, 0.0);
        node.text = Some("встреча в 15:00\nrps = 1000\nlatency = 50 ms\nrps × latency".to_owned());
        canvas.nodes.push(node);
        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        // Формульные строки 1, 2, 3 — значения есть (индексы ТЕКСТА;
        // числа сверяем по .num — юниты строк сохраняются: ms у latency)
        assert_eq!(
            solutions.lines[&("A".to_owned(), 1)].num,
            1000.0,
            "присваивание rps"
        );
        assert_eq!(
            solutions.lines[&("A".to_owned(), 2)].num,
            50.0,
            "присваивание latency (юнит ms сохранён)"
        );
        assert!(
            solutions.lines.contains_key(&("A".to_owned(), 3)),
            "выражение — строка 3"
        );
        // Проза (строка 0) значения не даёт
        assert!(!solutions.lines.contains_key(&("A".to_owned(), 0)));
        // Значение ноды = последняя формульная строка (инвариант FR-013)
        assert_eq!(
            solutions.outputs.get("A"),
            Some(&Ok(solutions.lines[&("A".to_owned(), 3)].clone()))
        );
    }

    /// FR-025 (правка 2, по проверке владельца): шаблонная нода даёт
    /// построчные выходы листа параметров (присваивания текста), узловое
    /// значение — формула шаблона (НЕ последняя строка листа).
    #[test]
    fn propagate_template_line_outputs() {
        use crate::templates::{TemplateParam, TemplateRef};
        let mut canvas = Canvas::default();
        let mut node = Node::text("T", "", 0.0, 0.0);
        node.text = Some("rps = 1000\nservers = 2".to_owned());
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "rps".to_owned(),
            TemplateParam {
                num: 1000.0,
                unit: None,
            },
        );
        params.insert(
            "servers".to_owned(),
            TemplateParam {
                num: 2.0,
                unit: None,
            },
        );
        node.set_template(Some(TemplateRef {
            id: "com.canvasdesk.test".to_owned(),
            version: "1.0.0".to_owned(),
            expr: "$rps × $servers".to_owned(),
            params,
            icon: String::new(),
            color: String::new(),
            name: None,
            outputs: Vec::new(),
        }));
        canvas.nodes.push(node);

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(
            solutions.lines[&("T".to_owned(), 0)].num,
            1000.0,
            "строка 0 листа параметров (rps)"
        );
        assert_eq!(
            solutions.lines[&("T".to_owned(), 1)].num,
            2.0,
            "строка 1 листа параметров (servers)"
        );
        // Узловое значение — формула шаблона (2000), не последняя строка (2)
        assert_eq!(
            solutions.outputs.get("T"),
            Some(&Ok(Value::scalar(2000.0))),
            "значение шаблонной ноды — формула"
        );
    }

    /// FR-025 (правка 2): drag от строки листа параметров шаблонной ноды —
    /// downstream получает значение ИМЕННО этой строки (регрессия проверки
    /// владельца: передавалось только узловое значение — результат формулы).
    #[test]
    fn inbound_slot_from_template_line_carries_param_value() {
        use crate::templates::{TemplateParam, TemplateRef};
        let mut canvas = Canvas::default();
        let mut node = Node::text("T", "", 0.0, 0.0);
        node.text = Some("rps = 1000\nservers = 2".to_owned());
        let mut params = std::collections::BTreeMap::new();
        params.insert(
            "rps".to_owned(),
            TemplateParam {
                num: 1000.0,
                unit: None,
            },
        );
        params.insert(
            "servers".to_owned(),
            TemplateParam {
                num: 2.0,
                unit: None,
            },
        );
        node.set_template(Some(TemplateRef {
            id: "com.canvasdesk.test".to_owned(),
            version: "1.0.0".to_owned(),
            expr: "$rps × $servers".to_owned(),
            params,
            icon: String::new(),
            color: String::new(),
            name: None,
            outputs: Vec::new(),
        }));
        canvas.nodes.push(node);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        // Ребро от строки 0 шаблонной ноды (rps = 1000)
        let mut line_edge = Edge::new("e1", "T", None, "B", None);
        line_edge.set_flow_kind(FlowKind::Value);
        line_edge.from_line = Some(0);
        canvas.add_edge(line_edge);

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        let slots = inbound_slots_with_lines(
            &canvas,
            "B",
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        assert_eq!(
            slots[0].as_ref().map(|value| value.num),
            Some(1000.0),
            "слот — значение строки 0 (rps), а не результат формулы (2000)"
        );
        // Downstream: 1000 × 2 (значение строки умножается в приёмнике)
        assert_eq!(
            solutions.outputs.get("B"),
            Some(&Ok(Value::scalar(2000.0))),
            "$in приёмника — значение строки шаблонной ноды"
        );
    }

    /// FR-025: слот value-ребра с `from_line` == значению строки-истока;
    /// ребро без `from_line` — значение ноды (инвариант флага).
    #[test]
    fn inbound_slots_from_line_carries_line_value() {
        let mut canvas = Canvas::default();
        let mut sheet = Node::text("A", "", 0.0, 0.0);
        sheet.text = Some("rps = 1000\ncpu = 4\nrps × cpu".to_owned());
        canvas.nodes.push(sheet);
        node_with_expr(&mut canvas, "B", "$in × 2", 1.0);
        // Ребро 1: значение ноды целиком (последняя строка)
        value_edge(&mut canvas, "e1", "A", "B");
        // Ребро 2: построчный исток — строка 0 (rps)
        let mut line_edge = Edge::new("e2", "A", None, "B", None);
        line_edge.set_flow_kind(FlowKind::Value);
        line_edge.from_line = Some(0);
        canvas.add_edge(line_edge);

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        let slots = inbound_slots_with_lines(
            &canvas,
            "B",
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        assert_eq!(slots.len(), 2, "оба value-ребра");
        assert_eq!(
            slots[0],
            Some(solutions.lines[&("A".to_owned(), 2)].clone()),
            "без from_line — значение ноды (последняя строка)"
        );
        assert_eq!(
            slots[1].as_ref().map(|value| value.num),
            Some(1000.0),
            "from_line 0 — rps"
        );
    }

    /// FR-025: тихая деградация слота — строка удалена/ошибка/проза →
    /// `Some(None)` (как «источник без значения»), индексы `$N` стабильны.
    #[test]
    fn inbound_slots_missing_or_error_line_degrade_to_none() {
        let mut canvas = Canvas::default();
        let mut sheet = Node::text("A", "", 0.0, 0.0);
        // Строка 0 — ошибка (деление на ноль), строка 1 — проза
        sheet.text = Some("x = 1 / 0\nпросто текст".to_owned());
        canvas.nodes.push(sheet);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        let mut edge0 = Edge::new("e0", "A", None, "B", None);
        edge0.set_flow_kind(FlowKind::Value);
        edge0.from_line = Some(0);
        canvas.add_edge(edge0);
        let mut edge1 = Edge::new("e1", "A", None, "B", None);
        edge1.set_flow_kind(FlowKind::Value);
        edge1.from_line = Some(1);
        canvas.add_edge(edge1);
        // Строка 9 не существует вовсе
        let mut edge9 = Edge::new("e9", "A", None, "B", None);
        edge9.set_flow_kind(FlowKind::Value);
        edge9.from_line = Some(9);
        canvas.add_edge(edge9);

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        let slots = inbound_slots_with_lines(
            &canvas,
            "B",
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        assert_eq!(
            slots,
            vec![None, None, None],
            "ошибка/проза/нет строки — Some(None)"
        );
    }

    /// Инвариант флага FR-025: для рёбер БЕЗ `from_line` построчная
    /// механика ничего не меняет — propagate даёт прежние значения.
    #[test]
    fn propagate_without_from_line_matches_legacy() {
        let mut canvas = Canvas::default();
        let mut sheet = Node::text("A", "", 0.0, 0.0);
        sheet.text = Some("rps = 1000\nrps × 2".to_owned());
        canvas.nodes.push(sheet);
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");

        let legacy = propagate(&canvas, &HashMap::new()).expect("DAG");
        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(legacy, solutions.outputs, "значения нод совпадают");
        assert_eq!(
            legacy.get("A").and_then(|r| r.as_ref().ok()),
            solutions.lines.get(&("A".to_owned(), 1)),
            "значение ноды == последняя формульная строка"
        );
    }

    // --- FR-029 (минимальный контур R1): проливание и именованные выходы ---

    /// Шаблонная нода с параметрами/expr/outputs (тестовый помощник FR-029).
    fn template_node(
        canvas: &mut Canvas,
        id: &str,
        text: &str,
        params: &[(&str, f64)],
        expr: &str,
        outputs: Vec<crate::templates::OutputSpec>,
    ) {
        use crate::templates::{TemplateParam, TemplateRef};
        let mut node = Node::text(id, "", 0.0, 0.0);
        node.text = Some(text.to_owned());
        let mut map = std::collections::BTreeMap::new();
        for (name, num) in params {
            map.insert(
                (*name).to_owned(),
                TemplateParam {
                    num: *num,
                    unit: None,
                },
            );
        }
        node.set_template(Some(TemplateRef {
            id: format!("com.canvasdesk.{id}"),
            version: "1.0.0".to_owned(),
            expr: expr.to_owned(),
            params: map,
            icon: String::new(),
            color: String::new(),
            name: None,
            outputs,
        }));
        canvas.nodes.push(node);
    }

    fn value_edge_with_ports(
        canvas: &mut Canvas,
        id: &str,
        from: &str,
        to: &str,
        from_output: Option<&str>,
        to_param: Option<&str>,
    ) {
        let mut edge = Edge::new(id, from, None, to, None);
        edge.set_flow_kind(FlowKind::Value);
        edge.from_output = from_output.map(str::to_owned);
        edge.to_param = to_param.map(str::to_owned);
        canvas.add_edge(edge);
    }

    /// FR-029: проливание — значение ребра с `to_param` перекрывает
    /// локальный параметр шаблона («проливание сильнее дефолта»), формула
    /// шаблона НЕ правится.
    #[test]
    fn pour_overrides_template_param() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        template_node(
            &mut canvas,
            "T",
            "rps = 100",
            &[("rps", 100.0)],
            "$rps × 2",
            Vec::new(),
        );
        value_edge_with_ports(&mut canvas, "e1", "A", "T", None, Some("rps"));

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(
            solutions.outputs.get("T"),
            Some(&Ok(Value::scalar(10.0))),
            "пролитое 5 перекрыло локальные 100 → 5 × 2 = 10"
        );
    }

    /// FR-029: ребро без `to_param` — прежнее позиционное поведение
    /// (значение в `$in`/`$1..$N`, локальные параметры не трогаются).
    #[test]
    fn edge_without_to_param_keeps_legacy_semantics() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        template_node(
            &mut canvas,
            "T",
            "rps = 100",
            &[("rps", 100.0)],
            "$rps + $in",
            Vec::new(),
        );
        value_edge(&mut canvas, "e1", "A", "T");

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(
            solutions.outputs.get("T"),
            Some(&Ok(Value::scalar(105.0))),
            "$rps = 100 (локальный), $in = 5 (позиционный)"
        );
    }

    /// FR-029: несколько value-рёбер в один `to_param` — побеждает
    /// последнее по порядку `canvas.edges` (детерминизм v1).
    #[test]
    fn pour_multiple_edges_last_wins() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "7", 1.0);
        template_node(
            &mut canvas,
            "T",
            "rps = 100",
            &[("rps", 100.0)],
            "$rps × 2",
            Vec::new(),
        );
        value_edge_with_ports(&mut canvas, "e1", "A", "T", None, Some("rps"));
        value_edge_with_ports(&mut canvas, "e2", "B", "T", None, Some("rps"));

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(
            solutions.outputs.get("T"),
            Some(&Ok(Value::scalar(14.0))),
            "последнее ребро (B = 7) перекрыло первое"
        );
    }

    /// FR-029: резолв `from_output` — ребро уносит значение ИМЕНОВАННОГО
    /// выхода истока (Expr-источник в окружении с проливанием), а не
    /// узловое значение.
    #[test]
    fn from_output_resolves_named_output() {
        let mut canvas = Canvas::default();
        template_node(
            &mut canvas,
            "S",
            "rps = 100",
            &[("rps", 100.0)],
            "$rps × 3",
            vec![crate::templates::OutputSpec {
                name: "origin".to_owned(),
                unit: None,
                source: OutputSource::Expr("$rps × 0.5".to_owned()),
            }],
        );
        node_with_expr(&mut canvas, "B", "$in × 4", 1.0);
        value_edge_with_ports(&mut canvas, "e1", "S", "B", Some("origin"), None);

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(
            solutions.named.get(&("S".to_owned(), "origin".to_owned())),
            Some(&Value::scalar(50.0)),
            "именованный выход: 100 × 0.5"
        );
        assert_eq!(
            solutions.outputs.get("B"),
            Some(&Ok(Value::scalar(200.0))),
            "$in приёмника = выход origin (50), не узловое значение (300)"
        );
    }

    /// FR-029: выход с источником `Line(i)` — значение строки Numi-листа
    /// инстанса (тот же механизм построчных выходов FR-025).
    #[test]
    fn from_output_line_source_resolves_line_value() {
        let mut canvas = Canvas::default();
        template_node(
            &mut canvas,
            "S",
            "load = 10\npeak = 25",
            &[("load", 10.0)],
            "$load",
            vec![crate::templates::OutputSpec {
                name: "peak".to_owned(),
                unit: None,
                source: OutputSource::Line(1),
            }],
        );
        node_with_expr(&mut canvas, "B", "$in", 1.0);
        value_edge_with_ports(&mut canvas, "e1", "S", "B", Some("peak"), None);

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        assert_eq!(
            solutions.outputs.get("B"),
            Some(&Ok(Value::scalar(25.0))),
            "выход peak = строка 1 листа (25)"
        );
    }

    /// FR-029: неизвестное имя выхода / выход с ошибкой — тихая деградация
    /// слота в `None` (чужой `.canvas` не ломает пересчёт).
    #[test]
    fn from_output_unknown_degrades_to_none() {
        let mut canvas = Canvas::default();
        template_node(
            &mut canvas,
            "S",
            "rps = 100",
            &[("rps", 100.0)],
            "$rps",
            Vec::new(),
        );
        node_with_expr(&mut canvas, "B", "$in + 1", 1.0);
        value_edge_with_ports(&mut canvas, "e1", "S", "B", Some("nope"), None);

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        let slots = inbound_slots_with_lines(
            &canvas,
            "B",
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        assert_eq!(slots, vec![None], "выхода nope нет — слот пуст");
    }

    /// FR-029: проливание учитывается в именованных выходах приёмника —
    /// Expr-выход считается в финальном окружении (с уже пролитым $param).
    #[test]
    fn pour_feeds_named_outputs_of_target() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "200", 0.0);
        template_node(
            &mut canvas,
            "CDN",
            "rps = 50\nhit = 0.9",
            &[("rps", 50.0), ("hit", 0.9)],
            "$rps × (1 - $hit)",
            vec![crate::templates::OutputSpec {
                name: "origin".to_owned(),
                unit: None,
                source: OutputSource::Expr("$rps × (1 - $hit)".to_owned()),
            }],
        );
        value_edge_with_ports(&mut canvas, "e1", "A", "CDN", None, Some("rps"));

        let solutions = propagate_with_lines(&canvas, &HashMap::new()).expect("DAG");
        let approx = |value: Option<&Value>, expected: f64, what: &str| {
            let actual = value.expect(what).num;
            assert!(
                (actual - expected).abs() < 1e-9,
                "{what}: {actual} != {expected}"
            );
        };
        approx(
            solutions
                .named
                .get(&("CDN".to_owned(), "origin".to_owned())),
            20.0,
            "origin посчитан от пролитого rps = 200: 200 × 0.1",
        );
        approx(
            solutions.outputs.get("CDN").and_then(|r| r.as_ref().ok()),
            20.0,
            "узловое значение — та же формула шаблона",
        );
    }
}
