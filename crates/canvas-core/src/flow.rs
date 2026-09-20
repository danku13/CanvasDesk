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

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

use crate::expr::{self, Env, EvalError, ExprOutcome, Value};
use crate::model::{Canvas, Edge};
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
/// `overrides` — подмена значений нод по id (FR-014): нода
/// получает override как своё значение, downstream видит его же.
/// Сохраняет сигнатуру FR-014; построчные подмены (FR-017) — через
/// [`propagate_with_lines`] с [`WhatIfOverrides`].
pub fn propagate(
    canvas: &Canvas,
    overrides: &HashMap<String, Value>,
) -> Result<FlowOutputs, CycleError> {
    let whatif = WhatIfOverrides::from_node_values(overrides);
    propagate_with_lines(canvas, &whatif).map(|solutions| solutions.outputs)
}

/// FR-017 (CP6): what-if подмены для [`propagate_with_lines`]. Пустая
/// структура — поведение идентично FR-014/FR-025 (обратная совместимость,
/// инвариант 1 FR-017).
#[derive(Debug, Clone, Default)]
pub struct WhatIfOverrides {
    /// Построчные подмены исходников: (id ноды, индекс строки ТЕКСТА)
    /// → новый исходник строки. Индексация едина с `FlowSolutions.lines`
    /// и `Edge::from_line` (инвариант 4 FR-017). Индексы вне диапазона
    /// текста игнорируются (тихая деградация протухших подмен).
    pub line_exprs: HashMap<(String, usize), String>,
    /// Value-level подмена значения ноды целиком (наследие FR-014; путь
    /// MCP): формула не выполняется, downstream видит подменённое значение.
    pub node_values: HashMap<String, Value>,
}

impl WhatIfOverrides {
    /// Value-only слой из старой карты FR-014 (миграция вызовов).
    pub fn from_node_values(values: &HashMap<String, Value>) -> Self {
        Self {
            line_exprs: HashMap::new(),
            node_values: values.clone(),
        }
    }

    /// Подмены строк одной ноды, отсортированные по индексу строки
    /// (порядок важен: последовательная оценка RHS в виртуальной
    /// param-карте шаблонной ноды).
    pub fn line_overrides(&self, node_id: &str) -> Vec<(usize, &String)> {
        let mut list: Vec<(usize, &String)> = self
            .line_exprs
            .iter()
            .filter(|((id, _), _)| id == node_id)
            .map(|((_, line), expr)| (*line, expr))
            .collect();
        list.sort_by_key(|(line, _)| *line);
        list
    }
}

/// FR-017: виртуальный исходник текста — строки с индексами из подмен
/// заменены новыми исходниками, остальные — как в persisted-тексте. Индексы
/// вне диапазона игнорируются (тихая деградация протухших подмен,
/// симметрия политики FR-025). Публична для приложения: построчные
/// результаты рендера считаются по тому же виртуальному листу, что и
/// propagator (инвариант 4 FR-017).
pub fn whatif_virtual_text(text: &str, overrides: &[(usize, &String)]) -> String {
    if overrides.is_empty() {
        return text.to_owned();
    }
    let mut lines: Vec<&str> = text.split('\n').collect();
    for (index, expr) in overrides {
        if let Some(slot) = lines.get_mut(*index) {
            *slot = expr;
        }
    }
    lines.join("\n")
}

/// FR-017: разбор override-строки присваивания (`rps = 2000 rps`) в
/// (имя, значение) — RHS вычисляется в данном окружении. Не-присваивание
/// (проза, явная формула `= …`) — None (параметр не биндится).
fn override_assignment(expr: &str, env: &Env) -> Option<(String, Value)> {
    let (name, rhs) = expr.split_once('=')?;
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return None;
    }
    let parsed = expr::parse(rhs.trim()).ok()?;
    let value = expr::eval(&parsed, env).ok()?;
    Some((name.to_owned(), value))
}

/// FR-025: построчные выходы Numi-листов — значение каждой формульной
/// строки: `(id ноды, индекс строки) → Value`. Заполняется в
/// [`propagate_with_lines`] (тот же обход и то же окружение, что у
/// значения ноды — строки видят входы value-рёбер). Ошибки строк и проза
/// значений не дают (слот деградирует до `None`).
pub type LineOutputs = HashMap<(String, usize), Value>;

/// FR-029: именованные выходы — `(id ноды, имя выхода) → Value`.
/// Для шаблонных нод — секция `outputs` манифеста (снапшот
/// `canvasdesk.template.outputs`: строка листа или подвыражение от
/// параметров); для текстовых — переменные Numi-листа (последнее
/// определение). Источник адресуется value-рёбрами `fromOutput`.
pub type NamedOutputs = HashMap<(String, String), Value>;

/// FR-029: полный результат пересчёта — значения нод ([`FlowOutputs`]),
/// построчные выходы ([`LineOutputs`]), именованные выходы
/// ([`NamedOutputs`]) и предупреждения по нодам (конфликты проливания).
#[derive(Debug, Clone, Default)]
pub struct FlowSolutions {
    /// Значения нод (как в [`propagate`]).
    pub outputs: FlowOutputs,
    /// Значения формульных строк текстовых нод (FR-025).
    pub lines: LineOutputs,
    /// Именованные выходы нод (FR-029).
    pub named: NamedOutputs,
    /// FR-029: предупреждения по нодам — несколько value-рёбер в один
    /// `toParam` (побеждает последнее по порядку `canvas.edges`).
    /// Строгая диагностика — `graph_validate` (FR-032).
    pub warnings: HashMap<String, Vec<String>>,
}

/// FR-029: входящие value-рёбра ноды, разобранные по адресации.
/// Позиционные слоты `$1..$N` — рёбра БЕЗ `toParam` (порядок `canvas.edges`;
/// для канвасов без адресованных рёбер — то же множество, что и раньше,
/// обратная совместимость FR-014); карта проливания — рёбра С `toParam`
/// (последнее по `canvas.edges` побеждает, детерминизм).
#[derive(Debug, Default)]
struct InboundValues {
    /// Слоты `$1..$N` (значение ребра или `None` — источник без значения).
    slots: Vec<Option<Value>>,
    /// Проливание в параметры: имя параметра → значение (последнее
    /// по `canvas.edges` ребро с этим `toParam`).
    spill: BTreeMap<String, Option<Value>>,
    /// Имена параметров с конфликтом (≥ 2 рёбер в один `toParam`).
    conflicts: Vec<String>,
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
    whatif: &WhatIfOverrides,
) -> Result<FlowSolutions, CycleError> {
    let order = topo_sort(canvas)?;
    let mut solutions = FlowSolutions::default();
    for index in order {
        let node = &canvas.nodes[index];
        let id = &node.id;
        // FR-014/FR-017: value-level подмена заменяет формулу целиком
        if let Some(value) = whatif.node_values.get(id) {
            solutions.outputs.insert(id.clone(), Ok(value.clone()));
            continue;
        }
        // FR-017: построчные подмены → виртуальный исходник текста
        // (пустой список — persisted-текст как есть, нулевой оверхед).
        let line_overrides = whatif.line_overrides(id);
        let text = whatif_virtual_text(&node.text.clone().unwrap_or_default(), &line_overrides);
        // FR-029: входы по адресации — позиционные слоты (рёбра без
        // toParam) и карта проливания в параметры (рёбра с toParam)
        let inbound = inbound_values(
            canvas,
            id,
            &solutions.outputs,
            &solutions.lines,
            &solutions.named,
        );
        if !inbound.conflicts.is_empty() {
            let warnings = inbound
                .conflicts
                .iter()
                .map(|name| {
                    format!(
                        "несколько value-рёбер в параметр {name}: побеждает последнее по canvas.edges"
                    )
                })
                .collect();
            solutions.warnings.insert(id.clone(), warnings);
        }
        let env = if inbound.slots.is_empty() {
            Env::empty()
        } else {
            Env::with_inbound(inbound.slots.clone())
        };
        // FR-018: у шаблонной ноды параметры (`canvasdesk.template.params`)
        // входят в окружение как `$имя`; формула — снимок из template-ссылки
        // (приоритет над `canvasdesk.expr` — шаблон определяет расчёт).
        // FR-017: override строки-параметра подменяет значение в ВИРТУАЛЬНОЙ
        // param-карте (persisted-снапшот не трогается): RHS вычисляется
        // последовательно в окружении входов + уже подменённых параметров.
        let template = node.template();
        let env = match &template {
            Some(tpl) => {
                let mut params = tpl.param_values();
                if !line_overrides.is_empty() {
                    let mut env_params = env.clone();
                    for (_, expr) in &line_overrides {
                        if let Some((name, value)) = override_assignment(expr, &env_params) {
                            params.insert(name.clone(), value.clone());
                            env_params = env_params.with_param_map(params.clone());
                        }
                    }
                }
                env.with_param_map(params)
            }
            None => env,
        };
        // FR-029: ПРОЛИВАНИЕ — значения рёбер с `toParam` подставляются в
        // окружение как `$<имя параметра>` ПОСЛЕ локальных параметров:
        // ребро перекрывает локальное значение («проливание сильнее
        // дефолта»), без правки формулы шаблона. Ребро без значения —
        // параметр остаётся локальным (тихая деградация, как у слотов).
        let env = if inbound.spill.is_empty() {
            env
        } else {
            let resolved: BTreeMap<String, Value> = inbound
                .spill
                .into_iter()
                .filter_map(|(name, value)| value.map(|v| (name, v)))
                .collect();
            env.with_param_map(resolved)
        };
        // FR-025 (правка 2): значение КАЖДОЙ формульной строки текста —
        // кандидат построчной точки выхода, теперь и у шаблонных нод
        // (лист параметров — присваивания со значениями). Ошибки строк
        // и проза значений не дают. FR-029: финальное окружение листа
        // (переменные) сохраняется — именованные выходы текстовой ноды.
        // FR-017: вычисляется ВИРТУАЛЬНЫЙ исходник (подменённые строки).
        let (line_outcomes, sheet_env) = expr::eval_lines_with_env(&text, &env);
        for (line_index, line_outcome) in line_outcomes.iter().enumerate() {
            if let Some(ExprOutcome::Ok(value)) = line_outcome {
                solutions
                    .lines
                    .insert((id.clone(), line_index), value.clone());
            }
        }
        // FR-029: именованные выходы ноды — адресация `fromOutput`.
        // Шаблонная нода: снапшот outputs манифеста (Line(i) — из
        // построчных значений; Expr(s) — вычисление в окружении ноды —
        // входы + параметры + проливание). Текстовая нода: переменные
        // Numi-листа (последнее определение имени — «строка сдвинулась,
        // связь жива").
        match &template {
            Some(tpl) => {
                for spec in &tpl.outputs {
                    let value = match &spec.source {
                        OutputSource::Line(line) => {
                            solutions.lines.get(&(id.clone(), *line)).cloned()
                        }
                        OutputSource::Expr(source) => expr::parse(source)
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
            None => {
                for (name, value) in sheet_env.vars_iter() {
                    solutions
                        .named
                        .insert((id.clone(), name.clone()), value.clone());
                }
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
    }
    Ok(solutions)
}

/// Значения входящих value-рёбер ноды (в порядке `canvas.edges`) по карте
/// результатов: `Some(Some(v))` — значение источника; `Some(None)` — ребро
/// есть, значения нет (источник без формулы, с ошибкой или висячее ребро).
/// Слоты НЕ схлопываются — индексы `$1..$N` стабильны.
pub fn inbound_slots(canvas: &Canvas, node_id: &str, outputs: &FlowOutputs) -> Vec<Option<Value>> {
    inbound_slots_with_lines(canvas, node_id, outputs, &LineOutputs::new())
}

/// FR-025: [`inbound_slots`] с построчными выходами: ребро с
/// `from_line = Some(i)` уносит значение строки `i` источника; строка
/// удалена/стала прозой/ошибка — слот `Some(None)` (тихая деградация,
/// согласована с принципом тишины прозы Numi). Ребро без `from_line` —
/// значение ноды целиком (текущее поведение, инвариант флага FR-025).
///
/// FR-029 (семантика адресации рёбер, общая с [`inbound_values`]):
/// `fromOutput` — именованный выход источника (снапшот `outputs` шаблона
/// или переменная Numi-листа текстовой ноды), тихая деградация `None`;
/// приоритет при обоих полях — `fromLine`. Рёбра с `toParam` в слоты
/// НЕ входят (они «проливаются» в параметры — [`inbound_values`]); в
/// канвасах без адресованных рёбер поведение идентично прежнему.
pub fn inbound_slots_with_lines(
    canvas: &Canvas,
    node_id: &str,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
) -> Vec<Option<Value>> {
    canvas
        .edges
        .iter()
        .filter(|edge| {
            edge.to_node == node_id
                && edge.flow_kind() == FlowKind::Value
                && edge.to_param.is_none()
        })
        .map(|edge| edge_source_value(edge, outputs, lines, &NamedOutputs::new()))
        .collect()
}

/// FR-029: значение, которое несёт value-ребро (источник по адресации):
/// `fromLine` → строка листа; иначе `fromOutput` → именованный выход;
/// иначе значение ноды целиком. Сломанная адресация — `None` (тихая
/// деградация: проваленный исток не роняет пересчёт downstream).
pub(crate) fn edge_source_value(
    edge: &Edge,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
    named: &NamedOutputs,
) -> Option<Value> {
    if let Some(line) = edge.from_line {
        lines.get(&(edge.from_node.clone(), line)).cloned()
    } else if let Some(name) = &edge.from_output {
        named.get(&(edge.from_node.clone(), name.clone())).cloned()
    } else {
        outputs
            .get(&edge.from_node)
            .and_then(|result| result.as_ref().ok())
            .cloned()
    }
}

/// FR-029: входящие value-рёбра ноды по адресации — позиционные слоты
/// `$1..$N` (рёбра без `toParam`, порядок `canvas.edges`) и карта
/// проливания в параметры (рёбра с `toParam`; несколько рёбер в один
/// параметр — последнее по `canvas.edges` побеждает, имя попадает в
/// `conflicts` для предупреждения `flow_recalc`/`graph_validate`).
fn inbound_values(
    canvas: &Canvas,
    node_id: &str,
    outputs: &FlowOutputs,
    lines: &LineOutputs,
    named: &NamedOutputs,
) -> InboundValues {
    let mut result = InboundValues::default();
    // параметры, в которые уже приходили рёбра (для детекции конфликтов)
    let mut seen_params: BTreeMap<String, usize> = BTreeMap::new();
    for edge in &canvas.edges {
        if edge.to_node != node_id || edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let value = edge_source_value(edge, outputs, lines, named);
        match &edge.to_param {
            None => result.slots.push(value),
            Some(name) => {
                let count = seen_params.entry(name.clone()).or_insert(0);
                *count += 1;
                if *count == 2 {
                    result.conflicts.push(name.clone());
                }
                result.spill.insert(name.clone(), value);
            }
        }
    }
    result
}

/// FR-029 (визуализация проливания): параметр ноды, запитанный входящим
/// value-ребром с `toParam` — источник значения для отображения
/// («rps ← Traffic Profile · peak_rps»). Runtime-данные: не сериализуются.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParamSpill {
    /// Имя параметра (адрес `toParam`; совпадает с именем присваивания
    /// в Numi-листе ноды).
    pub param: String,
    /// id ноды-источника значения.
    pub from_node: String,
    /// Заголовок источника для отображения (снимок имени шаблона /
    /// первая строка текста / label / id как последний фолбэк).
    pub from_label: String,
    /// FR-025: построчная адресация истока (индекс строки листа источника).
    pub from_line: Option<usize>,
    /// FR-029: именованный выход истока (секция outputs шаблона /
    /// переменная Numi-листа текстовой ноды).
    pub from_output: Option<String>,
}

/// FR-029: параметры ноды, запитанные входящими value-рёбрами с `toParam`.
/// Последнее ребро по `canvas.edges` в параметр побеждает — зеркало
/// семантики проливания в [`propagate_with_lines`] (тихая деградация:
/// источник удалён/без значения — spill в списке остаётся, значение
/// подставляет приложение из результатов пересчёта).
pub fn param_spills(canvas: &Canvas, node_id: &str) -> Vec<ParamSpill> {
    let mut by_param: BTreeMap<String, ParamSpill> = BTreeMap::new();
    for edge in &canvas.edges {
        if edge.to_node != node_id || edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let Some(param) = &edge.to_param else {
            continue;
        };
        let from_label = canvas
            .node(&edge.from_node)
            .map(spill_source_title)
            .unwrap_or_else(|| edge.from_node.clone());
        by_param.insert(
            param.clone(),
            ParamSpill {
                param: param.clone(),
                from_node: edge.from_node.clone(),
                from_label,
                from_line: edge.from_line,
                from_output: edge.from_output.clone(),
            },
        );
    }
    by_param.into_values().collect()
}

/// FR-045 R-3: вход без пролитого значения — производное pending-состояние
/// «значение не подставлено». Определение (§Решения Р-3): value-ребро
/// подключено к приёмнику, но значения нет (источник pending/ошибка/
/// отсутствует, `fromLine` пуст или вне диапазона, именованный выход/
/// колонка отсутствует в снапшоте). Контрольные рёбра значения не несут —
/// в состояние не входят.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmappedInput {
    /// id ребра (пунктирная отрисовка unmapped-ребра, FR-045 Р-3).
    pub edge_id: String,
    /// Позиционный слот `$N` (0-based среди позиционных рёбер) — для
    /// слотов; `None` — проливание в параметр.
    pub slot: Option<usize>,
    /// Имя параметра (`toParam`); `None` — позиционный слот.
    pub param: Option<String>,
}

/// FR-045 R-3: множество unmapped-входов ноды — производное состояние,
/// **не сериализуется** (в `.canvas` не пишется); вычисляется на каждом
/// пересчёте из готовых результатов [`propagate_with_lines`]. Порядок —
/// `canvas.edges` (детерминирован). Подстановка значения снимает состояние
/// автоматически (следующий пересчёт; инвариант 4 FR-045).
pub fn unmapped_inputs(
    canvas: &Canvas,
    node_id: &str,
    solutions: &FlowSolutions,
) -> Vec<UnmappedInput> {
    let mut slot: usize = 0;
    let mut result = Vec::new();
    for edge in &canvas.edges {
        if edge.to_node != node_id || edge.flow_kind() != FlowKind::Value {
            continue;
        }
        let value = edge_source_value(edge, &solutions.outputs, &solutions.lines, &solutions.named);
        if value.is_none() {
            let (slot_no, param) = if edge.to_param.is_none() {
                let no = slot;
                (Some(no), None)
            } else {
                (None, edge.to_param.clone())
            };
            result.push(UnmappedInput {
                edge_id: edge.id.clone(),
                slot: slot_no,
                param,
            });
        }
        if edge.to_param.is_none() {
            slot += 1;
        }
    }
    result
}

/// Заголовок ноды-источника для подписи проливания: снимок имени шаблона
/// (FR-023) / первая непустая строка текста (ATX-маркеры не показываем) /
/// label / id. Легковесный аналог заголовка карточки рендера: file/группы
/// для источника значения не бывает (формульные ноды — text-ноды).
fn spill_source_title(node: &crate::model::Node) -> String {
    if let Some(name) = node
        .template()
        .and_then(|template| template.name)
        .filter(|name| !name.is_empty())
    {
        return name;
    }
    if let Some(line) = node
        .text
        .as_deref()
        .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
    {
        return line.trim_start_matches('#').trim().to_owned();
    }
    node.label
        .as_deref()
        .filter(|label| !label.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| node.id.clone())
}

/// FR-029 (визуализация проливания): заменить строки-присваивания
/// пролитых параметров на подпись источника — `name ← from_label[ · output]`
/// (как на карточке: вместо локального литерала показываем, ОТКУДА пришло
/// значение). Индексы строк сохраняются (меняется содержимое, не структура)
/// — построчные результаты и порты FR-025 остаются на своих рядах.
/// `spills` — кортежи `(параметр, заголовок источника, именованный выход)`.
/// Строка без `=`, с пустым/многословным именем или чужим параметром —
/// без изменений; список пуст или совпадений нет — исходный текст (копия).
pub fn substitute_spilled_lines<'a>(
    text: &str,
    spills: impl IntoIterator<Item = (&'a str, &'a str, Option<&'a str>)>,
) -> String {
    let spills: Vec<(&str, &str, Option<&str>)> = spills.into_iter().collect();
    if spills.is_empty() {
        return text.to_owned();
    }
    let mut changed = false;
    let lines: Vec<Cow<str>> = text
        .split('\n')
        .map(|line| {
            let Some((name, _)) = line.split_once('=') else {
                return Cow::Borrowed(line);
            };
            let name = name.trim();
            if name.is_empty() || name.chars().any(char::is_whitespace) {
                return Cow::Borrowed(line);
            }
            match spills.iter().find(|(param, _, _)| *param == name) {
                Some((_, label, output)) => {
                    changed = true;
                    let suffix = output.map(|out| format!(" · {out}")).unwrap_or_default();
                    Cow::Owned(format!("{name} ← {label}{suffix}"))
                }
                None => Cow::Borrowed(line),
            }
        })
        .collect();
    if !changed {
        return text.to_owned();
    }
    lines.join("\n")
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

    // --- FR-029: визуализация проливания (param_spills) ---

    /// value-ребро с toParam → spill с адресацией истока и заголовком
    /// (первая строка текста источника).
    #[test]
    fn param_spills_collects_to_param_edges() {
        let mut canvas = Canvas::default();
        canvas
            .nodes
            .push(Node::text("A", "Traffic Profile — ADR005 (MVP)", 0.0, 0.0));
        node_with_expr(&mut canvas, "B", "$rps × 2", 1.0);
        let mut edge = Edge::new("e1", "A", None, "B", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.to_param = Some("rps".to_owned());
        edge.from_output = Some("peak_rps".to_owned());
        canvas.add_edge(edge);
        let spills = param_spills(&canvas, "B");
        assert_eq!(spills.len(), 1);
        assert_eq!(spills[0].param, "rps");
        assert_eq!(spills[0].from_node, "A");
        assert_eq!(spills[0].from_label, "Traffic Profile — ADR005 (MVP)");
        assert_eq!(spills[0].from_output, Some("peak_rps".to_owned()));
        assert_eq!(spills[0].from_line, None);
        // У ноды без входящих toParam-рёбер — пусто.
        assert!(param_spills(&canvas, "A").is_empty());
    }

    /// Два ребра в один параметр — побеждает последнее по canvas.edges
    /// (зеркало семантики проливания propagate).
    #[test]
    fn param_spills_last_edge_wins() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "C", "2", 1.0);
        node_with_expr(&mut canvas, "B", "$rps", 2.0);
        for (id, from) in [("e1", "A"), ("e2", "C")] {
            let mut edge = Edge::new(id, from, None, "B", None);
            edge.set_flow_kind(FlowKind::Value);
            edge.to_param = Some("rps".to_owned());
            canvas.add_edge(edge);
        }
        let spills = param_spills(&canvas, "B");
        assert_eq!(spills.len(), 1, "один параметр — один spill");
        assert_eq!(spills[0].from_node, "C", "последнее ребро побеждает");
    }

    /// Control-рёбра и value-рёбра без toParam в список не попадают.
    #[test]
    fn param_spills_ignores_control_and_slot_edges() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "1", 0.0);
        node_with_expr(&mut canvas, "B", "$1 + $rps", 1.0);
        let mut control = Edge::new("e1", "A", None, "B", None);
        control.to_param = Some("rps".to_owned());
        canvas.add_edge(control);
        let mut slot = Edge::new("e2", "A", None, "B", None);
        slot.set_flow_kind(FlowKind::Value);
        canvas.add_edge(slot);
        assert!(
            param_spills(&canvas, "B").is_empty(),
            "control с toParam и value без toParam — не проливание"
        );
    }

    /// Заголовок источника-шаблона — снимок имени шаблона (FR-023),
    /// а не первая строка листа параметров.
    #[test]
    fn param_spills_template_snapshot_name_as_label() {
        let mut canvas = Canvas::default();
        let mut source = Node::text("A", "rps = 1000 rps", 0.0, 0.0);
        source.set_template(Some(crate::templates::TemplateRef {
            id: "t".to_owned(),
            version: "1".to_owned(),
            expr: "$rps".to_owned(),
            params: Default::default(),
            icon: "custom".to_owned(),
            color: "#9B9B9B".to_owned(),
            name: Some("Сервис аутентификации".to_owned()),
            outputs: Vec::new(),
        }));
        canvas.nodes.push(source);
        node_with_expr(&mut canvas, "B", "$token_verify × 2", 1.0);
        let mut edge = Edge::new("e1", "A", None, "B", None);
        edge.set_flow_kind(FlowKind::Value);
        edge.to_param = Some("token_verify".to_owned());
        edge.from_line = Some(1);
        canvas.add_edge(edge);
        let spills = param_spills(&canvas, "B");
        assert_eq!(spills[0].from_label, "Сервис аутентификации");
        assert_eq!(spills[0].from_line, Some(1));
    }

    /// Подмена строки-присваивания на подпись источника: индексы строк
    /// сохраняются, чужие строки и проза не тронуты.
    #[test]
    fn substitute_spilled_lines_rewrites_assignment() {
        let text = "rps = 1389 rps\ncache_hit = 0.6\nlatency = 200 ms";
        let out = substitute_spilled_lines(
            text,
            [("rps", "Traffic Profile — ADR005 (MVP)", Some("peak_rps"))],
        );
        assert_eq!(
            out,
            "rps ← Traffic Profile — ADR005 (MVP) · peak_rps\ncache_hit = 0.6\nlatency = 200 ms"
        );
        assert_eq!(out.lines().count(), 3, "структура строк сохранена");
    }

    /// Подмена без именованного выхода — подпись без суффикса «· output».
    #[test]
    fn substitute_spilled_lines_without_output() {
        let out = substitute_spilled_lines("load = 100", [("load", "CDN", None)]);
        assert_eq!(out, "load ← CDN");
    }

    /// Совпадений нет / список пуст — исходный текст без изменений.
    #[test]
    fn substitute_spilled_lines_no_match_is_identity() {
        let text = "a = 1\nпроза\nb = 2";
        assert_eq!(substitute_spilled_lines(text, []), text);
        assert_eq!(
            substitute_spilled_lines(text, [("c", "X", None)]),
            text,
            "чужой параметр — без изменений"
        );
        // Многословное имя до '=' (проза с равенством) не присваивание.
        assert_eq!(
            substitute_spilled_lines("it = quality = 5", [("it = quality", "X", None)]),
            "it = quality = 5"
        );
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
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
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

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
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

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let slots = inbound_slots_with_lines(&canvas, "B", &solutions.outputs, &solutions.lines);
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

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let slots = inbound_slots_with_lines(&canvas, "B", &solutions.outputs, &solutions.lines);
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

        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        let slots = inbound_slots_with_lines(&canvas, "B", &solutions.outputs, &solutions.lines);
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
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(legacy, solutions.outputs, "значения нод совпадают");
        assert_eq!(
            legacy.get("A").and_then(|r| r.as_ref().ok()),
            solutions.lines.get(&("A".to_owned(), 1)),
            "значение ноды == последняя формульная строка"
        );
    }

    // --- FR-029: именованные порты значений (fromOutput / toParam) ---

    /// Хелпер: шаблонная нода со снапшотом (параметры + outputs).
    /// Параметры — (имя, значение, юнит): Rate-параметры требуют "rps".
    fn template_node_with_outputs(
        canvas: &mut Canvas,
        id: &str,
        params: &[(&str, f64, Option<&str>)],
        expr: &str,
        outputs: &[(&str, &str)],
    ) {
        use crate::templates::{OutputSource, OutputSpec, TemplateParam, TemplateRef};
        let text = params
            .iter()
            .map(|(name, value, unit)| match unit {
                Some(unit) => format!("{name} = {value} {unit}"),
                None => format!("{name} = {value}"),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut node = Node::text(id, text, 0.0, 0.0);
        let mut param_map = std::collections::BTreeMap::new();
        for (name, value, unit) in params {
            param_map.insert(
                (*name).to_owned(),
                TemplateParam {
                    num: *value,
                    unit: unit.map(str::to_owned),
                },
            );
        }
        node.set_template(Some(TemplateRef {
            id: format!("mock.{id}"),
            version: "1.0.0".to_owned(),
            expr: expr.to_owned(),
            params: param_map,
            icon: "custom".to_owned(),
            color: "#9B9B9B".to_owned(),
            name: None,
            outputs: outputs
                .iter()
                .map(|(name, expr)| OutputSpec {
                    name: (*name).to_owned(),
                    unit: None,
                    source: OutputSource::Expr((*expr).to_owned()),
                })
                .collect(),
        }));
        canvas.nodes.push(node);
    }

    /// value-ребро с адресацией портов (fromOutput / toParam).
    fn ported_value_edge(
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

    /// FR-029: проливание входа в параметр шаблонной ноды — ребро с
    /// `toParam` подставляет значение как `$rps`, перекрывая локальное
    /// значение параметра (семантика «проливание сильнее дефолта»).
    /// Сценарий ADR-0005: `Трафик.peak_rps → gateway.rps`.
    #[test]
    fn spill_overrides_local_param() {
        let mut canvas = Canvas::default();
        // Текстовая нода «Трафик»: peak_rps = 2500
        let traffic = Node::text("traffic", "peak_rps = 2500 rps", 0.0, 0.0);
        canvas.nodes.push(traffic);
        // Шаблонная нода: utilization($rps, 1 req / 10 ms) — локальный
        // rps = 1000 дал бы ρ = 10; проливание 2500 даёт ρ = 25
        template_node_with_outputs(
            &mut canvas,
            "gateway",
            &[("rps", 1000.0, Some("rps"))],
            "utilization($rps, 1 req / 10 ms)",
            &[],
        );
        // value-ребро: значение traffic (2500) → параметр rps
        ported_value_edge(&mut canvas, "e1", "traffic", "gateway", None, Some("rps"));
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        // utilization = 2500 / 100 = 25 — проливание перекрыло локальный 1000
        let utilization = solutions
            .outputs
            .get("gateway")
            .and_then(|result| result.as_ref().ok())
            .expect("формула шаблона вычислилась");
        assert!(
            (utilization.num - 25.0).abs() < 1e-9,
            "проливание перекрывает локальный параметр: {utilization:?}"
        );
        // Проливание видно и в именованном… нет — в значениях строк листа:
        // текст gateway (`rps = 1000`) НЕ переписывается (инвариант «без
        // правки формулы») — перекрытие только в окружении вычисления.
        let gateway_text = canvas
            .node("gateway")
            .and_then(|node| node.text.clone())
            .unwrap_or_default();
        assert!(
            gateway_text.contains("1000"),
            "текст ноды не мутируется проливанием"
        );
    }

    /// FR-029: ребро БЕЗ `toParam` — прежнее поведение FR-014:
    /// позиционный слот `$1` (обратная совместимость).
    #[test]
    fn edge_without_to_param_keeps_positional_slots() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "7", 0.0);
        node_with_expr(&mut canvas, "B", "$1 × 3", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("B")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(21.0),
            "позиционные слоты работают как до FR-029"
        );
    }

    /// FR-029: `fromOutput` на шаблонной ноде — именованный выход (Expr
    /// от параметров) адресуется downstream-ребром.
    #[test]
    fn from_output_template_named_output() {
        let mut canvas = Canvas::default();
        // CDN: rps = 1000, hit = 60% → выход origin_rps = $rps × (1 - $hit)
        template_node_with_outputs(
            &mut canvas,
            "cdn",
            &[("rps", 1000.0, Some("rps")), ("hit", 0.6, None)],
            "mm1($rps × (1 - $hit), 1 req / 200 ms)",
            &[("origin_rps", "$rps × (1 - $hit)")],
        );
        // Шаблонная нода-приёмник: lb с параметром connections_per_sec
        template_node_with_outputs(
            &mut canvas,
            "lb",
            &[("connections_per_sec", 100.0, Some("rps"))],
            "$connections_per_sec",
            &[],
        );
        ported_value_edge(
            &mut canvas,
            "e1",
            "cdn",
            "lb",
            Some("origin_rps"),
            Some("connections_per_sec"),
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        // Именованный выход зарегистрирован
        assert_eq!(
            solutions
                .named
                .get(&("cdn".to_owned(), "origin_rps".to_owned()))
                .map(|value| value.num),
            Some(400.0),
            "origin_rps = 1000 × 0.4"
        );
        // Проливание: lb.connections_per_sec = 400 (не локальные 100)
        assert_eq!(
            solutions
                .outputs
                .get("lb")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(400.0),
            "значение lb = пролитый origin_rps"
        );
    }

    /// FR-029: `fromOutput` на ТЕКСТОВОЙ ноде — переменная Numi-листа;
    /// правка текста выше (сдвиг строк) НЕ рвёт связь (имя стабильнее
    /// индекса — мотивация именованных портов).
    #[test]
    fn from_output_text_variable_survives_line_shift() {
        let mut canvas = Canvas::default();
        // Текстовая нода: peak_rps объявлен в середине листа
        let traffic = Node::text(
            "traffic",
            "dau = 1000000\npeak_rps = 1389 rps\navg = dau / 86400 s",
            0.0,
            0.0,
        );
        canvas.nodes.push(traffic);
        template_node_with_outputs(
            &mut canvas,
            "gateway",
            &[("rps", 1.0, Some("rps"))],
            "utilization($rps, 1 req / 10 ms)",
            &[],
        );
        ported_value_edge(
            &mut canvas,
            "e1",
            "traffic",
            "gateway",
            Some("peak_rps"),
            Some("rps"),
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .named
                .get(&("traffic".to_owned(), "peak_rps".to_owned()))
                .map(|value| value.num),
            Some(1389.0),
            "переменная листа — именованный выход текстовой ноды"
        );
        assert_eq!(
            solutions
                .outputs
                .get("gateway")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(13.89),
            "проливание peak_rps в параметр rps: ρ = 1389/100"
        );
        // Сдвиг строк: комментарий добавлен ВЫШЕ peak_rps — индекс строки
        // изменился, имя живёт
        let shifted = canvas
            .nodes
            .iter_mut()
            .find(|node| node.id == "traffic")
            .expect("нода traffic");
        shifted.text = Some(
            "# примечание\nX = 1\ndau = 1000000\npeak_rps = 1389 rps\navg = dau / 86400 s"
                .to_owned(),
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("gateway")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(13.89),
            "адресация по имени живёт при сдвиге строк: ρ = 1389/100"
        );
    }

    /// FR-029: несколько value-рёбер в один `toParam` — побеждает
    /// последнее по порядку `canvas.edges` (детерминизм) + предупреждение.
    #[test]
    fn multiple_spill_edges_last_wins_with_warning() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "100", 0.0);
        node_with_expr(&mut canvas, "B", "2500", 1.0);
        template_node_with_outputs(&mut canvas, "gw", &[("rps", 1.0, None)], "$rps", &[]);
        // Порядок в canvas.edges: e1 (A) раньше e2 (B) — победит B
        ported_value_edge(&mut canvas, "e1", "A", "gw", None, Some("rps"));
        ported_value_edge(&mut canvas, "e2", "B", "gw", None, Some("rps"));
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("gw")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(2500.0),
            "последнее ребро по canvas.edges побеждает"
        );
        let warnings = solutions
            .warnings
            .get("gw")
            .expect("предупреждение записано");
        assert!(
            warnings.iter().any(|w| w.contains("rps")),
            "предупреждение называет параметр: {warnings:?}"
        );
        // Слоты $1..$N не занимаются адресованными рёбрами: позиционная
        // формула `$1` у gw не видит рёбер с toParam (их нет в слотах)
    }

    /// FR-029: несуществующий `fromOutput` / `toParam` у рукописного
    /// файла — тихая деградация: значение не приходит, локальный
    /// параметр остаётся, пересчёт не падает (симметрия FR-025).
    #[test]
    fn broken_port_addressing_degrades_silently() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "100", 0.0);
        template_node_with_outputs(&mut canvas, "gw", &[("rps", 500.0, None)], "$rps", &[]);
        // fromOutput указывает на несуществующее имя; toParam — на
        // несуществующий параметр: формула должна взять локальный rps
        ported_value_edge(
            &mut canvas,
            "e1",
            "A",
            "gw",
            Some("no_such"),
            Some("also_missing"),
        );
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("gw")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(500.0),
            "локальный параметр остаётся при провале адресации"
        );
        // А точный toParam с провалившимся значением (источник без формулы):
        // параметр тоже локальный — ребро не «обнуляет» его
        let mut node = Node::text("prose", "просто проза", 2.0, 0.0);
        node.set_expr(None);
        canvas.nodes.push(node);
        ported_value_edge(&mut canvas, "e2", "prose", "gw", None, Some("rps"));
        let solutions = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("gw")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(500.0),
            "ребро без значения не сбрасывает локальный параметр"
        );
    }

    // --- FR-017 (CP6): what-if построчные подмены ---

    /// Инвариант 1 FR-017: пустой WhatIfOverrides — результат байт-в-байт
    /// как прежний propagate без подмен (обратная совместимость FR-014).
    #[test]
    fn whatif_empty_overrides_matches_baseline() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "B", "$in * 2", 1.0);
        value_edge(&mut canvas, "e", "A", "B");
        let legacy = propagate(&canvas, &HashMap::new()).expect("DAG");
        let whatif = propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("DAG");
        assert_eq!(legacy, whatif.outputs, "baseline идентичен");
        // Value-слой — прежняя семантика FR-014 (тест
        // overrides_flow_downstream_what_if перенесён на WhatIfOverrides)
        let mut node_values = HashMap::new();
        node_values.insert("A".to_owned(), Value::scalar(20.0));
        let whatif = WhatIfOverrides {
            line_exprs: HashMap::new(),
            node_values,
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("B")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(40.0),
            "node_values подмена течёт в downstream"
        );
    }

    /// FR-017: подмена строки заметки пересчитывает зависимые строки ниже
    /// по листу; независимые строки не тронуты (инвариант 4).
    #[test]
    fn whatif_line_override_note_recomputes_dependents() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text(
            "n",
            "rps = 1000\nlat = 50 ms\ncpu = rps * 2",
            0.0,
            0.0,
        ));
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("n".to_owned(), 0), "rps = 2000".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        // Строка-подмена: lines видит новое значение (индекс един с
        // FlowSolutions.lines / from_line — тот же слот)
        assert_eq!(
            solutions.lines.get(&("n".to_owned(), 0)).map(|v| v.num),
            Some(2000.0),
            "подменённая строка пересчитана в lines"
        );
        // Зависимая строка ниже пересчиталась
        assert_eq!(
            solutions.lines.get(&("n".to_owned(), 2)).map(|v| v.num),
            Some(4000.0),
            "зависимая строка видит подменённую переменную"
        );
        // Независимая строка не тронута
        assert_eq!(
            solutions.lines.get(&("n".to_owned(), 1)).map(|v| v.num),
            Some(50.0),
            "независимая строка не изменилась"
        );
        // Итог ноды — последняя формульная строка виртуального листа
        assert_eq!(
            solutions
                .outputs
                .get("n")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(4000.0),
            "итог ноды = пересчитанная последняя строка"
        );
    }

    /// FR-017: подмена ПОСЛЕДНЕЙ строки заменяет итог ноды.
    #[test]
    fn whatif_line_override_last_line_becomes_node_value() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("n", "10 + 5", 0.0, 0.0));
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("n".to_owned(), 0), "7 * 3".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("n")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(21.0),
            "итог ноды = подменённая последняя строка"
        );
    }

    /// FR-017: override строки-параметра шаблонной ноды мержится в
    /// ВИРТУАЛЬНУЮ param-карту (снапшот не тронут): формула пересчитана,
    /// соседний параметр — нет.
    #[test]
    fn whatif_template_param_virtual_map() {
        let mut canvas = Canvas::default();
        template_node_with_outputs(
            &mut canvas,
            "svc",
            &[("rps", 100.0, None), ("capacity", 200.0, None)],
            "$rps / $capacity",
            &[],
        );
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("svc".to_owned(), 0), "rps = 300".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("svc")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(1.5),
            "формула пересчитана с виртуальной param-картой"
        );
        // Соседний параметр не подменён
        assert_eq!(
            solutions.lines.get(&("svc".to_owned(), 1)).map(|v| v.num),
            Some(200.0),
            "capacity остался локальным"
        );
    }

    /// FR-017: каскад — подмена в A пересчитывает B и C по value-рёбрам.
    #[test]
    fn whatif_line_override_cascades_downstream() {
        let mut canvas = Canvas::default();
        canvas.nodes.push(Node::text("A", "5", 0.0, 0.0));
        canvas.nodes.push(Node::text("B", "$in * 2", 1.0, 0.0));
        canvas.nodes.push(Node::text("C", "$in + 1", 2.0, 0.0));
        value_edge(&mut canvas, "e1", "A", "B");
        value_edge(&mut canvas, "e2", "B", "C");
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("A".to_owned(), 0), "20".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        let num = |id: &str| {
            solutions
                .outputs
                .get(id)
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num)
        };
        assert_eq!(num("A"), Some(20.0));
        assert_eq!(num("B"), Some(40.0), "B пересчитан");
        assert_eq!(num("C"), Some(41.0), "C пересчитан");
    }

    /// FR-017: индекс вне текста — тихая деградация (протухшая подмена
    /// не ломает пересчёт, симметрия политики FR-025).
    #[test]
    fn whatif_out_of_range_line_ignored() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "n", "5", 0.0);
        let mut line_exprs = HashMap::new();
        line_exprs.insert(("n".to_owned(), 7), "999".to_owned());
        let whatif = WhatIfOverrides {
            line_exprs,
            node_values: HashMap::new(),
        };
        let solutions = propagate_with_lines(&canvas, &whatif).expect("DAG");
        assert_eq!(
            solutions
                .outputs
                .get("n")
                .and_then(|result| result.as_ref().ok())
                .map(|value| value.num),
            Some(5.0),
            "протухшая подмена проигнорирована"
        );
    }
    /// FR-045 R-3 (инвариант 4): unmapped ставится — ребро есть, значения
    /// нет (источник без значения); подача значения снимает состояние.
    #[test]
    fn unmapped_inputs_set_and_unset() {
        let mut canvas = Canvas::default();
        // Источник — проза: значения не даёт → unmapped.
        node_with_expr(&mut canvas, "A", "заметка без формулы", 0.0);
        node_with_expr(&mut canvas, "B", "$in * 2", 1.0);
        value_edge(&mut canvas, "e1", "A", "B");
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        let unmapped = unmapped_inputs(&canvas, "B", &solutions);
        assert_eq!(unmapped.len(), 1);
        assert_eq!(unmapped[0].edge_id, "e1");
        assert_eq!(unmapped[0].slot, Some(0));
        assert_eq!(unmapped[0].param, None);
        // Значение появилось — состояние снято (следующий пересчёт).
        node_with_expr(&mut canvas, "A", "21", 0.0);
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        assert!(unmapped_inputs(&canvas, "B", &solutions).is_empty());
    }

    /// FR-045 R-3: mixed-кейс — часть слотов пролитая, часть unmapped;
    /// слот-индекс учитывает только позиционные рёбра.
    #[test]
    fn unmapped_inputs_mixed_slots_and_params() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "A", "5", 0.0);
        node_with_expr(&mut canvas, "P", "проза", 1.0);
        node_with_expr(&mut canvas, "T", "$1 * $2 + $Сезон", 2.0);
        value_edge(&mut canvas, "e1", "A", "T");
        value_edge(&mut canvas, "e2", "P", "T"); // позиционный слот 1 — без значения
        let mut e3 = Edge::new("e3", "A", None, "T", None);
        e3.set_flow_kind(FlowKind::Value);
        e3.to_param = Some("Сезон".to_owned());
        canvas.edges.push(e3); // проливание — значение есть
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        let unmapped = unmapped_inputs(&canvas, "T", &solutions);
        assert_eq!(unmapped.len(), 1, "unmapped только у e2");
        assert_eq!(unmapped[0].slot, Some(1));
        // Проливание без значения источника → unmapped-параметр.
        let mut canvas2 = Canvas::default();
        node_with_expr(&mut canvas2, "P", "проза", 0.0);
        node_with_expr(&mut canvas2, "T", "$Сезон", 1.0);
        let mut e = Edge::new("eP", "P", None, "T", None);
        e.set_flow_kind(FlowKind::Value);
        e.to_param = Some("Сезон".to_owned());
        canvas2.edges.push(e);
        let solutions =
            propagate_with_lines(&canvas2, &WhatIfOverrides::default()).expect("пересчёт");
        let unmapped = unmapped_inputs(&canvas2, "T", &solutions);
        assert_eq!(unmapped.len(), 1);
        assert_eq!(unmapped[0].slot, None);
        assert_eq!(unmapped[0].param.as_deref(), Some("Сезон"));
    }

    /// FR-045 R-3: контрольные рёбра и висячее ребро; control не входит
    /// в состояние, висячее (исток не существует) — unmapped.
    #[test]
    fn unmapped_inputs_control_and_dangling() {
        let mut canvas = Canvas::default();
        node_with_expr(&mut canvas, "B", "$in", 0.0);
        control_edge(&mut canvas, "ec", "ghost1", "B");
        value_edge(&mut canvas, "ev", "ghost2", "B");
        let solutions =
            propagate_with_lines(&canvas, &WhatIfOverrides::default()).expect("пересчёт");
        let unmapped = unmapped_inputs(&canvas, "B", &solutions);
        assert_eq!(unmapped.len(), 1, "control не в состоянии");
        assert_eq!(unmapped[0].edge_id, "ev");
    }

    /// FR-045 (инвариант 1): `canvasdesk.desc` не влияет на поток —
    /// расчёт ноды с описанием и без идентичен.
    #[test]
    fn desc_does_not_affect_flow() {
        let mut base = Canvas::default();
        node_with_expr(&mut base, "A", "3", 0.0);
        node_with_expr(&mut base, "B", "$in + 1", 1.0);
        value_edge(&mut base, "e1", "A", "B");
        let mut with_desc = base.clone();
        with_desc.nodes[1].set_desc(Some("Итоговый коэффициент".to_owned()));
        let out_base = propagate_with_lines(&base, &WhatIfOverrides::default()).expect("пересчёт");
        let out_desc =
            propagate_with_lines(&with_desc, &WhatIfOverrides::default()).expect("пересчёт");
        assert_eq!(out_base.outputs, out_desc.outputs);
        assert_eq!(out_base.lines, out_desc.lines);
    }
}
