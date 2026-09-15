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
    let order = topo_sort(canvas)?;
    let mut outputs: FlowOutputs = HashMap::new();
    for index in order {
        let node = &canvas.nodes[index];
        let id = &node.id;
        // What-if: подменённое значение заменяет формулу целиком
        if let Some(value) = overrides.get(id) {
            outputs.insert(id.clone(), Ok(value.clone()));
            continue;
        }
        let slots = inbound_slots(canvas, id, &outputs);
        let env = if slots.is_empty() {
            Env::empty()
        } else {
            Env::with_inbound(slots)
        };
        // FR-018: у шаблонной ноды параметры (`canvasdesk.template.params`)
        // входят в окружение как `$имя`; формула — снимок из template-ссылки
        // (приоритет над `canvasdesk.expr` — шаблон определяет расчёт).
        let template = node.template();
        let env = match &template {
            Some(tpl) => env.with_param_map(tpl.param_values()),
            None => env,
        };
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
                    let text = node.text.clone().unwrap_or_default();
                    let last = expr::eval_lines_in(&text, &env)
                        .into_iter()
                        .flatten()
                        .last();
                    match last {
                        Some(ExprOutcome::Ok(value)) => Ok(value),
                        Some(ExprOutcome::Err(msg)) => Err(EvalError::BadFormula(msg)),
                        None => continue,
                    }
                }
            },
        };
        outputs.insert(id.clone(), outcome);
    }
    Ok(outputs)
}

/// Значения входящих value-рёбер ноды (в порядке `canvas.edges`) по карте
/// результатов: `Some(Some(v))` — значение источника; `Some(None)` — ребро
/// есть, значения нет (источник без формулы, с ошибкой или висячее ребро).
/// Слоты НЕ схлопываются — индексы `$1..$N` стабильны.
pub fn inbound_slots(canvas: &Canvas, node_id: &str, outputs: &FlowOutputs) -> Vec<Option<Value>> {
    canvas
        .edges
        .iter()
        .filter(|edge| edge.to_node == node_id && edge.flow_kind() == FlowKind::Value)
        .map(|edge| {
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
}
