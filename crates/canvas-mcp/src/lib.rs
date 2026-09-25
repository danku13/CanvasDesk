//! canvas-mcp — MCP-сервер-посредник: stdio (newline-delimited JSON-RPC 2.0,
//! БЕЗ Content-Length, как предписывает MCP spec) ↔ named pipe запущенного
//! canvas-app (`\\.\pipe\canvasdesk`, тот же line-framing).
//!
//! Вся логика — чистые функции в этом модуле (framing, JSON-RPC-огибающие,
//! tools/list, автомат handshake/tools-call), чтобы тестироваться без pipe.
//! Handshake не зависит от состояния приложения (ADR-0009/FR-034):
//! `initialize` успешен всегда; недоступность CanvasDesk — состояние, а не
//! краш — `tools/call` отвечает isError «не запущен», а мост перед каждым
//! пакетом короткой попыткой переподключается к pipe (приложение могло
//! подняться позже). Поддержаны batch-запросы (JSON-RPC массив) и протокол
//! 2025-06-18.
//!
//! Чистота stdout (ADR-0010/FR-035): stdout процесса — ТОЛЬКО newline-
//! delimited JSON-RPC. Собственный код моста молчалив по stdout, а
//! автоспавн поднимает ребёнка с `stdin/stdout/stderr = Stdio::null()` —
//! чужие логи (ANSI-tracing GUI) физически не могут попасть в протокол;
//! диагностика моста — в stderr.

use serde_json::{json, Value};
use std::time::Duration;

/// Pipe, который слушает canvas-app (SPEC: один канал на инстанс приложения).
pub const PIPE_NAME: &str = r"\\.\pipe\canvasdesk";
/// Протокольные версии MCP, которые мы заявляем (новейшая — первая).
/// FR-034/ADR-0009: добавлена 2025-06-18 — версия клиента эхом, иначе
/// строгие SDK (проверяют protocolVersion в ответе сервера) рвут соединение.
pub const SUPPORTED_PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];
/// Дефолтная версия протокола: отвечаем ею, если версия клиента неизвестна.
pub const DEFAULT_PROTOCOL: &str = "2024-11-05";
/// Таймаут ответа приложения на tools/call (SPEC задачи: 30 с → isError).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(30);

/// Разобранный JSON-RPC запрос. `id == None` — notification (ответ не нужен).
#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub id: Option<Value>,
    pub method: String,
    pub params: Value,
}

/// Ошибка разбора конверта: id (если удалось извлечь) + JSON-RPC код/текст.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub id: Option<Value>,
    pub code: i64,
    pub message: String,
}

/// Результат обработки одного элемента входного пакета stdio.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleOutcome {
    /// Ответить строкой на stdio (в batch — элемент массива ответов).
    Reply(String),
    /// Молчать (notification без результата).
    Silent,
}

/// Транспорт к canvas-app: pipe (Windows) или фейк в тестах.
pub trait AppTransport {
    /// Отправить строку-запрос приложению.
    fn send_line(&mut self, line: &str) -> Result<(), String>;
    /// Следующая строка-ответ (таймаут — ответственность реализации).
    fn recv_line(&mut self) -> Option<String>;
    /// Живо ли соединение.
    fn is_connected(&self) -> bool;
}

/// Вырезать завершённые `\n`-строки из буфера (хвост сохраняется до след.
/// чтения — сообщение может прийти разрезанным по середине). Пустые строки
/// пропускаются; завершающий `\r` срезается.
pub fn split_frames(buf: &mut Vec<u8>) -> Vec<String> {
    let mut lines = Vec::new();
    while let Some(pos) = buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = buf.drain(..=pos).collect();
        let line = &line[..line.len() - 1]; // без '\n'
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        lines.push(String::from_utf8_lossy(line).into_owned());
    }
    lines
}

/// Разобрать одну строку как JSON-RPC 2.0 запрос.
/// Мусор → -32700 (Parse error); невалидная структура → -32600 (Invalid Request).
pub fn parse_envelope(line: &str) -> Result<Request, ParseError> {
    let invalid = |message: &str| ParseError {
        id: None,
        code: -32600,
        message: message.to_owned(),
    };
    let value: Value = serde_json::from_str(line).map_err(|err| ParseError {
        id: None,
        code: -32700,
        message: format!("parse error: {err}"),
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| invalid("ожидался JSON-RPC объект"))?;
    let jsonrpc = object.get("jsonrpc").and_then(Value::as_str);
    if jsonrpc != Some("2.0") {
        return Err(invalid("поле jsonrpc != \"2.0\""));
    }
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("отсутствует method"))?;
    let id = object.get("id").cloned();
    let params = object.get("params").cloned().unwrap_or(Value::Null);
    Ok(Request {
        id,
        method: method.to_owned(),
        params,
    })
}

/// Сериализовать успешный JSON-RPC ответ в одну строку.
pub fn build_result(id: &Value, result: &Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

/// Сериализовать JSON-RPC ошибку в одну строку. `id: None` — null (id неизвестен).
pub fn build_error(id: Option<&Value>, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id.cloned().unwrap_or(Value::Null),
        "error": { "code": code, "message": message },
    })
    .to_string()
}

/// Ответ на `initialize`: версия протокола — версия клиента, если она в
/// списке поддерживаемых, иначе дефолт; заявляем только tools-возможность.
pub fn initialize_result(client_version: Option<&str>) -> Value {
    let protocol = client_version
        .filter(|v| SUPPORTED_PROTOCOLS.contains(v))
        .unwrap_or(DEFAULT_PROTOCOL);
    json!({
        "protocolVersion": protocol,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "canvasdesk", "version": env!("CARGO_PKG_VERSION") },
    })
}

/// Ответ tools/call: чистый JSON результата в text-контенте (машинный разбор
/// без вложенного парсинга) и structuredContent для объектных результатов
/// (spec 2025-06-18: структурированный вывод дублируется текстом для
/// обратной совместимости со старыми хостами).
pub fn build_call_result(id: &Value, result: &Value) -> String {
    // PRD-0007 X6 (F-9): text-first инструменты (`render: "text"`) отдают
    // готовый человекочитаемый текст как content text (не JSON-эхо всего
    // объекта) — LLM-клиенту не нужно парсить JSON ради поля text;
    // structuredContent сохраняет полный объект для структурированных
    // клиентов. Маркер узкий: только объекты с render == "text" и полем
    // text (никакой другой инструмент его не выставляет).
    let text_first = result.get("render").and_then(Value::as_str) == Some("text")
        && result.get("text").and_then(Value::as_str).is_some();
    if text_first {
        let mut envelope = json!({
            "content": [{ "type": "text", "text": result["text"].as_str().unwrap_or_default() }],
        });
        envelope["structuredContent"] = result.clone();
        return build_result(id, &envelope);
    }
    let mut envelope = json!({ "content": [{ "type": "text", "text": result.to_string() }] });
    if result.is_object() {
        envelope["structuredContent"] = result.clone();
    }
    build_result(id, &envelope)
}

/// Ошибка инструмента (MCP-идиома: isError внутри результата, НЕ JSON-RPC error).
pub fn build_call_error(id: &Value, message: &str) -> String {
    build_result(
        id,
        &json!({
            "isError": true,
            "content": [{ "type": "text", "text": message }],
        }),
    )
}

/// Спецификация одного инструмента для tools/list.
struct ToolSpec {
    name: &'static str,
    description: &'static str,
    /// Обязательные параметры (для inputSchema.required).
    required: &'static [&'static str],
    /// Пары (имя, JSON Schema фрагмент параметра).
    properties: &'static [(&'static str, &'static str)],
}

const STR: &str = r#"{"type":"string"}"#;
const NUM: &str = r#"{"type":"number"}"#;
const BOOL: &str = r#"{"type":"boolean"}"#;
const COLOR_PROP: &str = r#"{"type":["string","null"],"enum":["1","2","3","4","5","6",null]}"#;
const SIDE_PROP: &str =
    r#"{"type":"string","enum":["any","top","right","bottom","left"],"default":"any"}"#;

/// 40 инструментов канваса (FR-005 — node_edit; FR-025 построчные истоки;
/// FR-029 — адресация портов; FR-032 — edges_list/edge_get/graph_validate;
/// FR-033 — graph_apply; FR-016 — analyze_bottlenecks; FR-017/CP6 — 9 whatif_*;
/// PRD-0008 Q5 — schemes_*; PRD-0007 — lineage + explain_number F-9)
/// + 1 native-only FR-066 (monte_carlo_run — см. [`MC_TOOLS`]).
const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        name: "canvas_info",
        description: "Сводка по канвасу: число нод и связей, путь к файлу .canvas",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "nodes_list",
        description: "Список нод: id, тип, координаты, размеры, подпись, файл (поле text только при text=true)",
        required: &[],
        properties: &[("text", BOOL)],
    },
    ToolSpec {
        name: "node_get",
        description: "Одна нода по id со всеми полями (включая text)",
        required: &["id"],
        properties: &[("id", STR)],
    },
    ToolSpec {
        name: "nodes_search",
        description: "Поиск нод: подстрока без учёта регистра по text/title/label/file (title — явный заголовок canvasdesk.title, FR-072)",
        required: &["query"],
        properties: &[("query", STR)],
    },
    ToolSpec {
        name: "node_create_note",
        description: "Создать ноду-заметку; возвращает id. Размеры по умолчанию 260×120. Многострочный текст передавайте реальными переводами строк (в JSON LF экранируется как \\n); последовательность \\n как два символа тоже принимается как перевод строки; литеральный обратный слеш — \\\\ FR-072: title — явный заголовок карточки (без него заголовок — первая строка текста, legacy)",
        required: &["x", "y"],
        properties: &[("x", NUM), ("y", NUM), ("text", STR), ("title", STR), ("width", NUM), ("height", NUM)],
    },
    ToolSpec {
        name: "node_create_file",
        description: "Создать файловую ноду по пути (файл на диске НЕ создаётся); возвращает id",
        required: &["path", "x", "y"],
        properties: &[("path", STR), ("x", NUM), ("y", NUM), ("width", NUM), ("height", NUM)],
    },
    ToolSpec {
        name: "node_update_text",
        description: "Заменить текст ноды-заметки. Многострочный текст — реальными переводами строк (в JSON LF экранируется как \\n); последовательность \\n как два символа тоже принимается как перевод строки; литеральный обратный слеш — \\\\",
        required: &["id", "text"],
        properties: &[("id", STR), ("text", STR)],
    },
    ToolSpec {
        name: "node_edit",
        description: "Редактировать ноду одним вызовом: обновляет ТОЛЬКО переданные поля (text, title, label, color, expr, x, y, width, height); label/color/expr = null — сброс; title — явный заголовок (FR-072), title = null — сброс к фолбэку «первая строка текста»; expr — Numi-style формула, результат рендерится под текстом ноды (невалидная формула — ошибка, нода не меняется); возвращает обновлённую ноду. Многострочный text — реальными переводами строк (в JSON LF экранируется как \\n); последовательность \\n как два символа тоже принимается как перевод строки; литеральный обратный слеш — \\\\",
        required: &["id"],
        properties: &[
            ("id", STR),
            ("text", STR),
            ("title", r#"{"type":["string","null"]}"#),
            ("label", r#"{"type":["string","null"]}"#),
            ("color", COLOR_PROP),
            ("expr", r#"{"type":["string","null"]}"#),
            ("x", NUM),
            ("y", NUM),
            ("width", NUM),
            ("height", NUM),
        ],
    },
    ToolSpec {
        name: "node_move",
        description: "Переместить ноду в world-координаты (x, y)",
        required: &["id", "x", "y"],
        properties: &[("id", STR), ("x", NUM), ("y", NUM)],
    },
    ToolSpec {
        name: "node_resize",
        description: "Изменить размеры ноды (width, height). fit:true — подогнать высоту под контент (фон не сжимается меньше видимого)",
        required: &["id", "width", "height"],
        properties: &[("id", STR), ("width", NUM), ("height", NUM), ("fit", BOOL)],
    },
    ToolSpec {
        name: "node_delete",
        description: "Удалить ноду по id (связи удаляются каскадно; дети группы НЕ удаляются)",
        required: &["id"],
        properties: &[("id", STR)],
    },
    ToolSpec {
        name: "node_set_color",
        description: "Установить цвет ноды: пресет \"1\"..\"6\" или null для сброса",
        required: &["id", "color"],
        properties: &[("id", STR), ("color", COLOR_PROP)],
    },
    ToolSpec {
        name: "edge_create",
        description: "Создать связь между нодами; стороны any|top|right|bottom|left (дефолт any — авто); возвращает id. FR-029 v2 — адресация портов для value-связей: kind \"value\" включает поток значений (дефолт \"control\" — визуальная связь); fromLine (int ≥ 0) — построчный исток FR-025; fromOutput (string) — именованный выход истока (секция outputs шаблона или переменная Numi-листа текстовой ноды); toParam (string) — проливание значения в параметр $имя шаблонной ноды приёмника (перекрывает локальное). fromLine и fromOutput взаимно исключаются; toParam только при kind=value; неизвестные имена выходов/параметров (для шаблонных нод) — ошибка вызова",
        required: &["from", "to"],
        properties: &[
            ("from", STR),
            ("to", STR),
            ("fromSide", SIDE_PROP),
            ("toSide", SIDE_PROP),
            ("kind", r#"{"type":"string","enum":["value","control"]}"#),
            ("fromLine", NUM),
            ("fromOutput", STR),
            ("toParam", STR),
        ],
    },
    ToolSpec {
        name: "edge_delete",
        description: "Удалить связь по id",
        required: &["id"],
        properties: &[("id", STR)],
    },
    ToolSpec {
        name: "flow_set_kind",
        description: "FR-014: тип потока связи — \"value\" (переносит значение источника в $in/$1..$N формулы downstream) или \"control\" (визуальная связь, дефолт). Тогл в value, замыкающий цикл, — ошибка с участниками; пересчёт потока — сразу",
        required: &["id", "kind"],
        properties: &[
            ("id", STR),
            ("kind", r#"{"type":"string","enum":["value","control"]}"#),
        ],
    },
    ToolSpec {
        name: "flow_recalc",
        description: "FR-029 v2: карта значений потока — {node_id: {value, unit, outputs: {имя: {value, unit}} (именованные выходы, вкл. переменные Numi-листов), lines: [{index, value, unit}] (построчные), warnings? (конфликты), spilled? {param: {from, fromOutput?, fromLine?, value, unit}}, autoRows? [{slot, edge, path «Объект.Поле», field, value, unit | unmapped}] (FR-050 Р-4: производные строки приёмников)}}. Значения АКТИВНОГО what-if сценария — те же, что видит пользователь на канвасе (MCP-видимость = UI): подмены активного сценария учитываются, база — после whatif_scenario_activate «База». Цикл потока — ошибка вызова",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "lineage",
        description: "PRD-0007 (X2, FR-048): дерево происхождения цифры — те же данные, что окно проверки цепочки: node_id + line (индекс строки Numi-листа; null/без поля — итог ноды). Ответ {root, nodes[]}: DFS-порядок (родитель раньше ребёнка, ромб разворачивается), kind calc|leaf|cycle|unmapped|unlinked|truncated, value+unit|error, formula?, title, label? (терминальные: имя переменной/входа), children [{child (индекс в nodes), via? {edge_id, from_node, to_node, from_line?, from_output?, to_param?}}] — via = ребро для подсветки цепочки. Значения активного сценария; цикл потока — топология без значений (AC-2.4); бюджет 4096 узлов — свёртка truncated",
        required: &["node_id"],
        properties: &[
            ("node_id", STR),
            ("line", r#"{"type":["integer","null"],"minimum":0}"#),
        ],
    },
    ToolSpec {
        name: "flow_cycle_check",
        description: "FR-014: проверка DAG-инварианта value-рёбер: [] — циклов нет, иначе список id участников цикла",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "explain_number",
        description: "PRD-0007 (X6, FR-048, F-9 must): объяснение цифры ТЕКСТОМ — линейная развёртка дерева происхождения с адресами (node_id, строка Numi-листа, выход/параметр) и значениями каждого узла; тот же снапшот, что окно проверки и lineage (F-5). Ответ {render:\"text\", text, root, nodes, truncated}: text — готовое объяснение (мост отдаёт его как content text), truncated — достигнут бюджет 4096 узлов. Значения активного what-if сценария (преамбула в text); цикл потока — топология без значений (AC-2.4)",
        required: &["node_id"],
        properties: &[
            ("node_id", STR),
            ("line", r#"{"type":["integer","null"],"minimum":0}"#),
        ],
    },
    ToolSpec {
        name: "edge_ports",
        description: "CR-008: стороны подключения связи. По умолчанию — авто: кратчайший путь, пересчёт при перетаскивании нод и раскладке. pin: \"auto\" — снять закрепления; \"from\"/\"to\"/\"both\" — закрепить концы, фиксируя текущие эффективные стороны (WYSIWYG). Возвращает {id, pins:{from,to}}",
        required: &["id", "pin"],
        properties: &[
            ("id", STR),
            (
                "pin",
                r#"{"type":"string","enum":["auto","from","to","both"]}"#,
            ),
        ],
    },
    ToolSpec {
        name: "edges_list",
        description: "FR-032/FR-029: все связи канваса: {id, from, to, kind (\"value\"|\"control\"), fromLine?, fromOutput?, toParam?, fromSide, toSide} — агент восстанавливает топологию графа (CR-013 G4); fromLine — индекс строки-истока (FR-025), fromOutput/toParam — адресация портов значений (FR-029)",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "edge_get",
        description: "FR-032: одна связь по id — схема как у элемента edges_list",
        required: &["id"],
        properties: &[("id", STR)],
    },
    ToolSpec {
        name: "graph_validate",
        description: "FR-032: валидация модели — {valid, issues:[{severity, code, node_id, edge_id, message}]}. Коды (стабильный контракт, docs/change-requests/fr-032-graph-read-validate.md): E-CYCLE (цикл value-рёбер), E-OVERLOAD (ρ ≥ 1), W-AMBIGUOUS-SRC (многолинейный исток без fromLine), W-UNUSED-SLOT (вход $N не читается формулой); E-UNIT/E-PORT-UNKNOWN/E-DOUBLE-INPUT — реализованы поверх FR-029 (адресация портов). valid = нет issue с severity \"error\"",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "template_list",
        description: "FR-018: список шаблонов реестра — id, name, version, category, description, expr (Numi-формула с $param), icon, color, params ({type, default, unit?, min?, max?}), outputs (FR-029: именованные выходы {name, unit?, line?|expr?} — потребляются рёбрами fromOutput). Те же шаблоны, что видит пользователь в палитре (Ctrl+P) и wheel-меню",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "template_instantiate",
        description: "FR-018: создать text-ноду из шаблона: текст — Numi-лист параметров (rps = 1000 rps), canvasdesk.template — снимок {id, version, expr, params}; формула считает поток (FR-014). params — переопределения {имя: число (в единице параметра манифеста) или {num, unit}} (значение вне min/max — ошибка). Возвращает {id, index, node}",
        required: &["id", "x", "y"],
        properties: &[("id", STR), ("x", NUM), ("y", NUM), ("params", r#"{"type":"object"}"#)],
    },
    ToolSpec {
        name: "schemes_list",
        description: "PRD-0008 (Q5 v2): список встроенных схем галереи — те же пакеты, что видит пользователь в галерее (Ctrl+T): [{id, name, name_en, category, category_ru/en, version, description/description_en, nodes, edges}] — готовые канвасы с расчётами, пучками и подсказками; RU-первично, как в UI",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "schemes_apply",
        description: "PRD-0008 (Q5 v2): вставить схему в текущий канвас — как «Открыть» в галерее: id нод/рёбер ремапятся без коллизий (note-N/group-N/edge-N), содержимое центрируется в точку (x, y) или центр viewport (дефолт — видимое пользователю место); один undo-шаг (Ctrl+Z откатывает вставку целиком), полный пересчёт, what-if сценарии не трогаются. Ответ: {applied, name, nodes[] (созданные id), edges[] (схема как у edges_list — вкл. kind/fromLine/fromOutput/toParam), bbox [min_x, min_y, max_x, max_y] (для viewport_set/zoom-to-fit), flow} — flow = значения АКТИВНОГО сценария (как flow_recalc: значения/выходы/построчные/авто-строки)",
        required: &["id"],
        properties: &[("id", STR), ("x", NUM), ("y", NUM)],
    },
    ToolSpec {
        name: "viewport_get",
        description: "Центр viewport в world-координатах и зум",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "viewport_set",
        description: "Установить центр viewport (x, y) и опционально зум",
        required: &["x", "y"],
        properties: &[("x", NUM), ("y", NUM), ("zoom", NUM)],
    },
    ToolSpec {
        name: "graph_apply",
        description: "FR-033: атомарный батч операций над канвасом — «всё или ничего»: ошибка ЛЮБОЙ операции (в том числе в середине списка) откатывает весь батч, канвас остаётся прежним; успешный батч = один undo-шаг + полный пересчёт потока. Операции (поле op): node_create_note {ref?, x, y, text?, width?, height?}; node_create_file {ref?, x, y, path}; template_instantiate {ref?, template, params?, x, y}; edge_create {fromRef|from, toRef|to, kind? \"value\"|\"control\", fromLine?, fromOutput?, toParam?, fromSide?, toSide?} — адресация портов FR-029, ref-ы адресуют ноды, созданные ранее В ЭТОМ ЖЕ батче; edge_delete {id|ref} — удаление ребра (FR-050 Н4: замена занятого toParam = пара edge_delete + edge_create в одном батче — второе edge_create в занятый параметр падает E-DOUBLE-INPUT); param_set {ref|id, param, value, unit?} — правит одну строку «param = value unit», параметра нет — ошибка; node_move {ref|id, x, y}. Ответ: {ok, created[], report[], flow{node_id: {value, unit, outputs, lines, autoRows?, error?}}} — flow = значения АКТИВНОГО сценария после пересчёта (как flow_recalc; второй вызов не нужен); при ошибке операции — {ok: false, op_index, code, message}. Лимиты: ≤ 256 операций, ≤ 128 новых нод на батч",
        required: &["operations"],
        properties: &[(
            "operations",
            r#"{"type":"array","minItems":1,"maxItems":256,"items":{"type":"object","required":["op"],"properties":{"op":{"type":"string","enum":["node_create_note","node_create_file","template_instantiate","edge_create","edge_delete","param_set","node_move"]}}}}"#,
        )],
    },
    // --- FR-017 (CP6): what-if сценарии ---
    ToolSpec {
        name: "whatif_set_override",
        description: "FR-017: построчная what-if подмена активного сценария: исходник строки node_id:line замещается expr на время сценария (база не мутируется, дельты видны на канвасе и в whatif_deltas). Режим/сценарий поднимаются автоматически. Многострочный expr: перенос — настоящий \\n в JSON-строке (двухсимвольная эскапировка нормализуется толерантно)",
        required: &["node_id", "line", "expr"],
        properties: &[
            ("node_id", STR),
            ("line", r#"{"type":"integer","minimum":0}"#),
            ("expr", STR),
        ],
    },
    ToolSpec {
        name: "whatif_set_param",
        description: "FR-017: sugar поверх whatif_set_override для шаблонных нод — адресация по имени параметра: находит строку «param = …» в тексте ноды и подменяет её значением (строка «param = value»). Параметра нет в тексте — ошибка",
        required: &["node_id", "param", "value"],
        properties: &[(("node_id"), STR), (("param"), STR), (("value"), STR)],
    },
    ToolSpec {
        name: "whatif_scenario_list",
        description: "FR-017: список what-if сценариев: {active, whatif_active, scenarios: [{name, overrides, stale}]} — stale = число протухших подмен (нода/строка удалены, строка стала прозой; пропускаются пересчётом, маркируются здесь)",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "whatif_scenario_create",
        description: "FR-017: создать именованный what-if сценарий (freeze — сохраняется в canvasdesk.whatif внутри .canvas, один undo-шаг). Без name — имя по умолчанию. Лимит 3 сценария",
        required: &[],
        properties: &[("name", STR)],
    },
    ToolSpec {
        name: "whatif_scenario_delete",
        description: "FR-017: удалить what-if сценарий по имени (мутация .canvas, undo-шаг)",
        required: &["name"],
        properties: &[("name", STR)],
    },
    ToolSpec {
        name: "whatif_scenario_activate",
        description: "FR-017: переключить активный сценарий (имя или «База»). Runtime-only: файл не меняется; канвас пересчитывается с подменами сценария",
        required: &["name"],
        properties: &[("name", STR)],
    },
    ToolSpec {
        name: "whatif_deltas",
        description: "FR-017: дельты активного сценария против базы — {active, deltas: {\"node:line\"|\"node:value\": {node, line?, base, whatif, delta}}} — те же пары «было → стало (+Δ)», что видит пользователь (инвариант: MCP-видимость эквивалентна UI)",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "whatif_apply",
        description: "FR-017: применить активный сценарий — подмены записываются в persisted-строки/params канваса (один undo-шаг), сценарий удаляется. Переключает «Базу»",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "whatif_reset",
        description: "FR-017: сбросить подмены активного сценария (runtime, файл не трогается). Режим остаётся активным",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "analyze_bottlenecks",
        description: "FR-016 (CP5): анализ узких мест и риска очередей — те же флаги, что видит пользователь на канвасе (оверлей Ctrl+B) — АКТИВНОГО what-if состояния (подмены учитываются, MCP-видимость = UI). Ответ: {nodes:[{id, severity (\"none\"|\"warn\"|\"critical\"|\"overload\"), utilization? (ρ, доля 0..1, >1 при перегрузке), queue_length?, wait_sec? (W, базовые секунды), badge (строка бейджа канваса)}], thresholds}. Детекция: значение ноды = ошибка Overload (ρ ≥ 1) → severity \"overload\" (ρ из ошибки); utilization-выход шаблона / Percent-значение → пороги 0.7/0.9; Time-значение (W) → пороги 100 ms/1 s; queue_length-выход → 1/10. Чистая функция над пересчитанным потоком: не мутирует канвас",
        required: &[],
        properties: &[],
    },
];

/// FR-066 (M5/S3, §5.8): имя native-only MC/QMC-инструмента. Реестр
/// wasm-сборки его НЕ отдаёт (фича `qmc` не собирается на wasm), но
/// скиллы описывают весь (native) продукт — каноническое имя участвует
/// в тестах синхронности на обеих платформах. `allow(dead_code)` на
/// wasm: константа используется тестами и native-реестром.
#[allow(dead_code)]
const MC_TOOL_NAME: &str = "monte_carlo_run";

/// Native-only инструменты (FR-066 §5.8): monte_carlo_run — за фичей
/// `qmc` канвас-сцены (stats+parallel+sobol_burley, не собирается на
/// wasm32; диспетчер сцены без фичи отвечает внятной ошибкой).
#[cfg(not(target_arch = "wasm32"))]
const MC_TOOLS: &[ToolSpec] = &[ToolSpec {
    name: MC_TOOL_NAME,
    description: "FR-066: Monte Carlo/QMC-прогон модели — N прогонов расчётного графа с распределёнными параметрами и квантили P50/P90/P99 результатов (runway/LTV-риски: «с какой вероятностью уйдёт в ноль» — глубже ±20 %-сеток whatif). params: {\"node:param\": {\"dist\": \"normal\"|\"lognormal\"|\"exp\"|\"poisson\", …}} — normal/lognormal: {mean, sd} (натуральное пространство), exp/poisson: {lambda}; параметр — строка «param = …» Numi-листа ноды (подменяется на каждый прогон, паттерн whatif_set_param; единица придаётся формулой-потребителем). mode: \"qmc\" (дефолт — Owen-scrambled Sobol, меньшая дисперсия) | \"mc\" (ChaCha8); seed — u64 (дефолт 0; тот же seed → те же квантили — воспроизводимость first-class); quantiles — дефолт [0.5, 0.9, 0.99]. Ответ: {runs, failed_runs, mode, seed, stale, duration_ms, quantiles, outputs{node:{P50:{value,unit}…}}, lines{node:line:{…}}, named{node:out:{…}}, analysis{quantile, nodes (как analyze_bottlenecks), thresholds}, severity}. analysis — узкие места FR-016 на ХВОСТОВОМ квантиле (P90): severity none|warn|critical|overload. Лимиты: runs ≤ 10⁶; qmc ≤ 65536 (2¹⁶ — длина Sobol); poisson λ ≤ 1000. Цикл потока — ошибка вызова; параметры не из листа — ошибка ДО прогонов",
    required: &["runs", "params"],
    properties: &[
        (
            "runs",
            r#"{"type":"integer","minimum":1,"maximum":1000000}"#,
        ),
        (
            "params",
            r#"{"type":"object","additionalProperties":{"type":"object","required":["dist"],"properties":{"dist":{"type":"string","enum":["normal","lognormal","exp","poisson"]},"mean":{"type":"number"},"sd":{"type":"number"},"lambda":{"type":"number"}}}}"#,
        ),
        (
            "mode",
            r#"{"type":"string","enum":["qmc","mc"],"default":"qmc"}"#,
        ),
        ("seed", r#"{"type":"integer","minimum":0}"#),
        (
            "quantiles",
            r#"{"type":"array","items":{"type":"number","exclusiveMinimum":0,"exclusiveMaximum":1},"default":[0.5,0.9,0.99]}"#,
        ),
    ],
}];
#[cfg(target_arch = "wasm32")]
const MC_TOOLS: &[ToolSpec] = &[];

/// tools/list: массив дескрипторов с name/description/inputSchema.
/// Native: 41 (40 + monte_carlo_run FR-066); wasm: 40 (§5.8 — qmc не
/// собирается на wasm, реестр без native-only инструментов).
pub fn tools_list() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .chain(MC_TOOLS.iter())
        .map(|tool| {
            let properties: serde_json::Map<String, Value> = tool
                .properties
                .iter()
                .map(|(name, schema)| {
                    // Схемы — константы модуля, покрытые тестом tools_list;
                    // в проде паника недопустима — при опечатке параметр
                    // вырождается в пустой объект, а не роняет сервер.
                    let schema = serde_json::from_str(schema).unwrap_or_else(|err| {
                        debug_assert!(false, "невалидная схема параметра {name}: {err}");
                        json!({})
                    });
                    (name.to_string(), schema)
                })
                .collect();
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": {
                    "type": "object",
                    "properties": properties,
                    "required": tool.required,
                },
            })
        })
        .collect();
    json!({ "tools": tools })
}

/// Текст ошибки «приложение не запущено» (единый для tools/call и reconnect).
fn not_running_message() -> String {
    format!("CanvasDesk не запущен (pipe {PIPE_NAME} не найден)")
}

/// Разворот конверта приложения (FR-034/ADR-0009): по pipe CanvasDesk
/// отвечает JSON-RPC-конвертом (`build_result` в `on_mcp_wake`) — клиенту
/// нужен только `result` (или сообщение из `error`). Не-конвертный payload
/// (легаси-транспорт, тестовые заглушки) проходит как есть.
pub fn unwrap_app_payload(payload: &str) -> Result<Value, String> {
    let value: Value =
        serde_json::from_str(payload).map_err(|err| format!("ответ CanvasDesk не JSON: {err}"))?;
    if value.get("jsonrpc").and_then(Value::as_str) == Some("2.0") {
        if let Some(result) = value.get("result") {
            return Ok(result.clone());
        }
        if let Some(error) = value.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("ошибка приложения");
            return Err(message.to_owned());
        }
    }
    Ok(value)
}

/// Обработать один JSON-RPC элемент stdio в конечном автомате MCP
/// (для batch-массивов используйте `handle_input`).
///
/// - `initialize` → ответ с protocolVersion/capabilities/serverInfo —
///   ВСЕГДА успешный (ADR-0009: состояние приложения не влияет на handshake);
/// - `notifications/initialized`, `notifications/cancelled` → Silent;
/// - `ping` → `{}`;
/// - `tools/list` → 41 инструмент с inputSchema (40 + monte_carlo_run
///   FR-066; на wasm32 — 40, реестр без qmc);
/// - `tools/call` → форвард строки на pipe, конверт приложения разворачивается
///   в чистый результат (text-контент + structuredContent, FR-034);
///   isError-результат приложения проходит насквозь; pipe мёртв → isError
///   «не запущен», таймаут ответа → isError;
/// - `resources/list`, `prompts/list`, `resources/templates/list`,
///   `logging/setLevel` → толерантные пустые ответы (хосты зондируют их
///   безотносительно заявленных capabilities);
/// - прочее → JSON-RPC -32601.
pub fn handle_line<T: AppTransport>(line: &str, transport: &mut Option<T>) -> HandleOutcome {
    let request = match parse_envelope(line) {
        Ok(request) => request,
        Err(err) => {
            return HandleOutcome::Reply(build_error(err.id.as_ref(), err.code, &err.message))
        }
    };
    let id = request.id.clone().unwrap_or(Value::Null);
    match request.method.as_str() {
        "initialize" => {
            let client_version = request
                .params
                .get("protocolVersion")
                .and_then(Value::as_str);
            HandleOutcome::Reply(build_result(&id, &initialize_result(client_version)))
        }
        "notifications/initialized" | "notifications/cancelled" => HandleOutcome::Silent,
        "ping" => HandleOutcome::Reply(build_result(&id, &json!({}))),
        "tools/list" => HandleOutcome::Reply(build_result(&id, &tools_list())),
        "resources/list" => HandleOutcome::Reply(build_result(&id, &json!({ "resources": [] }))),
        "prompts/list" => HandleOutcome::Reply(build_result(&id, &json!({ "prompts": [] }))),
        "resources/templates/list" => {
            HandleOutcome::Reply(build_result(&id, &json!({ "resourceTemplates": [] })))
        }
        "logging/setLevel" => HandleOutcome::Reply(build_result(&id, &json!({}))),
        "tools/call" => {
            let Some(pipe) = transport.as_mut().filter(|t| t.is_connected()) else {
                return HandleOutcome::Reply(build_call_error(&id, &not_running_message()));
            };
            if let Err(err) = pipe.send_line(line) {
                return HandleOutcome::Reply(build_call_error(&id, &err));
            }
            match pipe.recv_line() {
                Some(payload) => match unwrap_app_payload(&payload) {
                    // isError из приложения — готовый MCP-результат ошибки:
                    // проходим насквозь, не заворачивая повторно
                    Ok(result) if result.get("isError").and_then(Value::as_bool) == Some(true) => {
                        HandleOutcome::Reply(build_result(&id, &result))
                    }
                    Ok(result) => HandleOutcome::Reply(build_call_result(&id, &result)),
                    Err(message) => HandleOutcome::Reply(build_call_error(&id, &message)),
                },
                None => HandleOutcome::Reply(build_call_error(
                    &id,
                    &format!("таймаут ответа CanvasDesk ({} с)", CALL_TIMEOUT.as_secs()),
                )),
            }
        }
        other => HandleOutcome::Reply(build_error(
            Some(&id),
            -32601,
            &format!("метод не поддержан: {other}"),
        )),
    }
}

/// Обработать входной пакет stdio (FR-034/ADR-0009): одиночный JSON-RPC
/// объект ИЛИ batch-массив (spec 2025-03-26). Элементы массива обрабатываются
/// по порядку, ответы собираются в вектор; уведомления ответов не порождают
/// (батч целиком из уведомлений → пустой вектор, stdout молчит). Пустой
/// массив — один ответ -32600 (невалидный батч по JSON-RPC 2.0).
pub fn handle_input<T: AppTransport>(input: &str, transport: &mut Option<T>) -> Vec<HandleOutcome> {
    let value: Value = match serde_json::from_str(input) {
        Ok(value) => value,
        Err(err) => {
            return vec![HandleOutcome::Reply(build_error(
                None,
                -32700,
                &format!("parse error: {err}"),
            ))]
        }
    };
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                return vec![HandleOutcome::Reply(build_error(
                    None,
                    -32600,
                    "пустой batch-запрос",
                ))];
            }
            let mut outcomes = Vec::new();
            for item in items {
                outcomes.push(handle_line(&item.to_string(), transport));
            }
            outcomes
        }
        _ => vec![handle_line(input, transport)],
    }
}

/// Заглушка транспорта для сборки вне Windows: pipe всегда недоступен —
/// мост работает offline (handshake успешен, вызовы — isError, ADR-0009).
#[cfg(not(windows))]
pub struct OfflineTransport;

#[cfg(not(windows))]
impl AppTransport for OfflineTransport {
    fn send_line(&mut self, _line: &str) -> Result<(), String> {
        Err("named pipe поддерживается только на Windows".to_owned())
    }
    fn recv_line(&mut self) -> Option<String> {
        None
    }
    fn is_connected(&self) -> bool {
        false
    }
}

// --- FR-008: один exe — режим MCP-посредника (подкоманда `mcp`) ---

/// Сколько секунд ждать pipe после автостарта сервиса (GUI + wgpu init).
#[cfg(windows)]
const SPAWN_WAIT_SECS: u32 = 15;
/// Ожидание pipe при уже запущенном сервисе (как у старого бинарника).
#[cfg(windows)]
const CONNECT_WAIT_MS: u32 = 2000;
/// Ожидание pipe при переподключении перед каждым пакетом (FR-034): pipe
/// отсутствует → WaitNamedPipeW откажет мгновенно; занят — ждём до
/// полсекунды. Автоспавна на reconnect нет — только на старте (FR-008).
#[cfg(windows)]
const RECONNECT_WAIT_MS: u32 = 500;

/// Запустить stdio-MCP-сервер (FR-008) — нативная обёртка: автоспавн сервиса
/// (FR-008/FR-035), offline-режим вне Windows (ADR-0009), хук reconnect
/// (FR-034). Семантика не меняется; сам stdio-цикл выделен в
/// платформенно-нейтральную `run_stdio_with_transport` (FR-037/MW2).
pub fn run_stdio(args: &[String]) -> anyhow::Result<()> {
    let no_spawn = args.iter().any(|arg| arg == "--no-spawn");
    #[cfg(windows)]
    let transport = connect_app(no_spawn);
    // Вне Windows pipe нет — мост работает offline: handshake успешен,
    // tools/call отвечает isError «не запущен» (FR-034/ADR-0009)
    #[cfg(not(windows))]
    let _ = no_spawn;
    #[cfg(not(windows))]
    let transport: Option<OfflineTransport> = None;
    // Хук reconnect (FR-034): на Windows — refresh_transport (fn item
    // реализует FnMut(&mut Option<PipeTransport>)), вне Windows — no-op.
    #[cfg(windows)]
    let reconnect = refresh_transport;
    #[cfg(not(windows))]
    let reconnect = |_t: &mut Option<OfflineTransport>| {};

    run_stdio_with_transport(transport, reconnect)
}

/// Платформенно-нейтральный stdio-цикл моста (выделение FR-037/MW2):
/// stdin → split_frames → handle_input → stdout. Контракт `reconnect` —
/// короткая попытка переподключения транспорта перед каждым пакетом
/// (FR-034; на Windows — refresh_transport к pipe, вне Windows — no-op).
/// Молчалив по stdout (там протокол MCP), диагностика — в stderr.
/// EOF stdin — штатный выход.
pub fn run_stdio_with_transport<T, R>(
    mut transport: Option<T>,
    mut reconnect: R,
) -> anyhow::Result<()>
where
    T: AppTransport,
    R: FnMut(&mut Option<T>),
{
    use std::io::{Read, Write};
    let stdin = std::io::stdin();
    let mut stdin = stdin.lock();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        // EOF stdin — клиент (MCP-хост) закрыл канал: штатный выход
        let read = stdin.read(&mut buf)?;
        if read == 0 {
            break;
        }
        pending.extend_from_slice(&buf[..read]);
        for line in split_frames(&mut pending) {
            // FR-034: приложение могло подняться после старта моста —
            // короткая попытка reconnect перед каждым пакетом
            reconnect(&mut transport);
            for outcome in handle_input(&line, &mut transport) {
                match outcome {
                    HandleOutcome::Reply(reply) => {
                        writeln!(stdout, "{reply}")?;
                        stdout.flush()?;
                    }
                    HandleOutcome::Silent => {}
                }
            }
        }
    }
    Ok(())
}

/// FR-034 (ADR-0009): переподключение перед обработкой пакета. Приложение
/// может подняться ПОСЛЕ старта моста (автоспавн не удался, GUI перезапущен,
/// pipe-сессия разорвалась) — короткая попытка соединения (без автоспавна)
/// подхватывает pipe без перезапуска MCP-сессии.
#[cfg(windows)]
fn refresh_transport(transport: &mut Option<PipeTransport>) {
    let dead = transport.as_ref().map(|t| t.is_connected()) != Some(true);
    if dead {
        if let Some(fresh) = try_connect(RECONNECT_WAIT_MS) {
            *transport = Some(fresh);
        }
    }
}

/// FR-035/ADR-0010: команда автоспавна с изолированным stdio. В Rust
/// `Command::spawn()` без явных хэндлов НАСЛЕДУЕТ stdin/stdout/stderr
/// родителя — спавненный GUI писал ANSI-tracing-логи прямо в JSON-RPC-канал
/// моста («Invalid JSON» у клиента). Null-хэндлы отсекают класс целиком:
/// чужой процесс физически не может писать в stdout протокола или красть
/// stdin. Поведенческая проверка — юнит-тестом (ребёнок репортит свои fd).
// FR-037/MW2 (контингенция плана §4 MW2-a п.4, прецедент FR-036): под
// wasm-таргеты хелперы исключены вовсе — их тесты загвардены, а компиляция
// в wasip1-test давала бы dead_code и вносила бы std::process в wasm-сборку.
#[cfg(any(windows, all(test, not(target_arch = "wasm32"))))]
fn spawn_service_command(exe: &std::path::Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(exe);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    cmd
}

/// FR-035/ADR-0010: ориентация автоспавна. Единый бинарь `canvasdesk`
/// без аргументов — GUI-режим того же exe (FR-008). Автономный мост
/// `canvasdesk-mcp` спавнить СЕБЯ не может (двойник конкурирует за stdin
/// и рекурсивно плодит процессы) — вместо него ищем GUI-бинарь
/// `canvasdesk.exe` рядом (один каталог дистрибутива); нет соседа —
/// спавн невозможен (None → offline-режим ADR-0009).
#[cfg(any(windows, all(test, not(target_arch = "wasm32"))))]
fn autosprawn_target(current_exe: &std::path::Path) -> Option<std::path::PathBuf> {
    let stem = current_exe
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())?;
    if stem == "canvasdesk-mcp" {
        let sibling = current_exe.with_file_name("canvasdesk.exe");
        if sibling.exists() {
            Some(sibling)
        } else {
            None
        }
    } else {
        Some(current_exe.to_path_buf())
    }
}

/// Подключение к приложению с автостартом (FR-008): сервис уже работает —
/// короткое ожидание; нет — спавним сервис (см. autosprawn_target) и ждём
/// подъёма pipe. Не удалось — None: мост продолжит работу offline
/// (ADR-0009/FR-034).
#[cfg(windows)]
fn connect_app(no_spawn: bool) -> Option<PipeTransport> {
    if let Some(transport) = try_connect(CONNECT_WAIT_MS) {
        return Some(transport);
    }
    if no_spawn {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let Some(target) = autosprawn_target(&exe) else {
        eprintln!(
            "автостарт недоступен: рядом с canvasdesk-mcp нет canvasdesk.exe — мост работает offline"
        );
        return None;
    };
    match spawn_service_command(&target).spawn() {
        Ok(child) => {
            // Child дропается: процесс сервиса живёт своей жизнью, зомби на
            // Windows не образуется, убийство при выходе посредника не нужно
            let _ = child;
            eprintln!("CanvasDesk не был запущен — сервис поднят автоматически, ждём pipe…");
        }
        Err(err) => {
            eprintln!("не удалось запустить сервис CanvasDesk: {err}");
            return None;
        }
    }
    try_connect(SPAWN_WAIT_SECS * 1000)
}

/// Одна попытка подключения: WaitNamedPipeW с таймаутом + CreateFileW.
#[cfg(windows)]
fn try_connect(wait_ms: u32) -> Option<PipeTransport> {
    use windows::core::PCWSTR;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_OVERLAPPED, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
    };
    use windows::Win32::System::Pipes::WaitNamedPipeW;

    let wide: Vec<u16> = PIPE_NAME.encode_utf16().chain(std::iter::once(0)).collect();
    let name = PCWSTR(wide.as_ptr());
    // WaitNamedPipeW возвращает BOOL (не Result): true — pipe появился.
    // SAFETY: wide NUL-терминирован и живёт до конца вызова.
    let waited = unsafe { WaitNamedPipeW(name, wait_ms) }.as_bool();
    if !waited {
        return None;
    }
    // SAFETY: name валиден; параметры — константы Win32.
    let handle = unsafe {
        CreateFileW(
            name,
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_FLAG_OVERLAPPED,
            None,
        )
    }
    .ok()?;
    PipeTransport::new(handle)
}

/// Клиент named pipe: дуплексный overlapped-хэндл (копия в reader-потоке),
/// ответы — строки в канале с таймаутом CALL_TIMEOUT (lib кодирует None
/// в isError). Overlapped обязателен: на блокирующем хэндле ядро
/// сериализовало бы операции, и запись из main-потока встала бы за
/// висящим чтением reader-потока (дедлок «запрос ждёт, ответ ждёт»).
#[cfg(windows)]
struct PipeTransport {
    handle: windows::Win32::Foundation::HANDLE,
    write_event: windows::Win32::Foundation::HANDLE,
    inbox: std::sync::mpsc::Receiver<String>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

#[cfg(windows)]
impl PipeTransport {
    fn new(handle: windows::Win32::Foundation::HANDLE) -> Option<Self> {
        use windows::Win32::System::Threading::CreateEventW;
        // Событие для записи из main-потока (reader живёт со своим).
        // SAFETY: параметры события — константы Win32.
        let write_event = unsafe { CreateEventW(None, true, false, None) }.ok()?;
        if write_event.is_invalid() {
            return None;
        }
        let (tx, inbox) = std::sync::mpsc::channel::<String>();
        let connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let connected_reader = std::sync::Arc::clone(&connected);
        // HANDLE в windows 0.62 не Send — в нить передаём usize и собираем
        // handle обратно (передача значения, не владения).
        let reader_handle = handle.0 as usize;
        let reader = std::thread::Builder::new()
            .name("mcp-pipe-reader".to_owned())
            .spawn(move || {
                let reader_handle =
                    windows::Win32::Foundation::HANDLE(reader_handle as *mut core::ffi::c_void);
                read_loop(reader_handle, tx, connected_reader);
            });
        // Reader не поднялся — транспорт бесполезен (деградация без паники)
        reader.ok()?;
        Some(Self {
            handle,
            write_event,
            inbox,
            connected,
        })
    }
}

/// Overlapped-цикл чтения строк: блокирующий по смыслу (ожидание события),
/// но оставляет хэндл открытым для конкурентной записи из main-потока.
#[cfg(windows)]
fn read_loop(
    handle: windows::Win32::Foundation::HANDLE,
    tx: std::sync::mpsc::Sender<String>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    use windows::Win32::Foundation::{CloseHandle, ERROR_IO_PENDING, WAIT_OBJECT_0};
    use windows::Win32::Storage::FileSystem::ReadFile;
    use windows::Win32::System::Threading::{
        CreateEventW, ResetEvent, WaitForSingleObject, INFINITE,
    };
    use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};

    let event = unsafe { CreateEventW(None, true, false, None) }.ok();
    let event = match event {
        Some(event) if !event.is_invalid() => event,
        _ => {
            connected.store(false, std::sync::atomic::Ordering::SeqCst);
            return;
        }
    };
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        // SAFETY: event валиден; buf и overlapped живут до GetOverlappedResult.
        let _ = unsafe { ResetEvent(event) };
        let mut overlapped = OVERLAPPED {
            hEvent: event,
            ..Default::default()
        };
        // Для overlapped ReadFile lpNumberOfBytesRead обязан быть NULL.
        let result = unsafe { ReadFile(handle, Some(&mut buf), None, Some(&mut overlapped)) };
        if let Err(err) = result {
            if err.code() != ERROR_IO_PENDING.to_hresult() {
                // Приложение закрыло pipe — ReadFile вернёт ошибку
                connected.store(false, std::sync::atomic::Ordering::SeqCst);
                break;
            }
            // SAFETY: event валиден; ждём бесконечно.
            let wait = unsafe { WaitForSingleObject(event, INFINITE) };
            if wait != WAIT_OBJECT_0 {
                break;
            }
        }
        let mut read = 0u32;
        // SAFETY: overlapped завершён; read — out-параметр.
        let completed = unsafe { GetOverlappedResult(handle, &overlapped, &mut read, false) };
        if completed.is_err() || read == 0 {
            connected.store(false, std::sync::atomic::Ordering::SeqCst);
            break;
        }
        pending.extend_from_slice(&buf[..read as usize]);
        for line in split_frames(&mut pending) {
            if tx.send(line).is_err() {
                // Транспорт уничтожен (main-поток вышел)
                break;
            }
        }
    }
    let _ = unsafe { CloseHandle(event) };
}

#[cfg(windows)]
impl AppTransport for PipeTransport {
    fn send_line(&mut self, line: &str) -> Result<(), String> {
        use windows::Win32::Foundation::{ERROR_IO_PENDING, WAIT_OBJECT_0};
        use windows::Win32::Storage::FileSystem::WriteFile;
        use windows::Win32::System::Threading::{ResetEvent, WaitForSingleObject, INFINITE};
        use windows::Win32::System::IO::{GetOverlappedResult, OVERLAPPED};

        if !self.is_connected() {
            return Err("соединение с CanvasDesk разорвано".to_owned());
        }
        let mut bytes = line.as_bytes().to_vec();
        bytes.push(b'\n');
        let mut rest: &[u8] = &bytes;
        while !rest.is_empty() {
            // SAFETY: write_event валиден; буферы живы до GetOverlappedResult.
            let _ = unsafe { ResetEvent(self.write_event) };
            let mut overlapped = OVERLAPPED {
                hEvent: self.write_event,
                ..Default::default()
            };
            let result = unsafe { WriteFile(self.handle, Some(rest), None, Some(&mut overlapped)) };
            if let Err(err) = result {
                if err.code() != ERROR_IO_PENDING.to_hresult() {
                    return Err(format!("WriteFile: {err}"));
                }
                let wait = unsafe { WaitForSingleObject(self.write_event, INFINITE) };
                if wait != WAIT_OBJECT_0 {
                    return Err("ожидание записи прервано".to_owned());
                }
            }
            let mut written = 0u32;
            // SAFETY: overlapped завершён; written — out-параметр.
            let completed =
                unsafe { GetOverlappedResult(self.handle, &overlapped, &mut written, false) };
            if completed.is_err() {
                return Err("WriteFile: соединение разорвано".to_owned());
            }
            if written == 0 {
                return Err("WriteFile: записано 0 байт".to_owned());
            }
            rest = &rest[written as usize..];
        }
        Ok(())
    }

    fn recv_line(&mut self) -> Option<String> {
        self.inbox.recv_timeout(CALL_TIMEOUT).ok()
    }

    fn is_connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Фейковый транспорт к приложению: исходящие строки запоминаются,
    /// входящие ответы отдаются из очереди.
    struct FakeTransport {
        sent: Vec<String>,
        inbox: VecDeque<String>,
        connected: bool,
    }

    impl FakeTransport {
        fn connected(inbox: &[&str]) -> Self {
            Self {
                sent: Vec::new(),
                inbox: inbox.iter().map(|s| s.to_string()).collect(),
                connected: true,
            }
        }
    }

    impl AppTransport for FakeTransport {
        fn send_line(&mut self, line: &str) -> Result<(), String> {
            self.sent.push(line.to_owned());
            Ok(())
        }
        fn recv_line(&mut self) -> Option<String> {
            self.inbox.pop_front()
        }
        fn is_connected(&self) -> bool {
            self.connected
        }
    }

    /// split_frames: два сообщения в одном chunk; разрез посередине строки;
    /// пустые строки пропускаются; '\r' срезается; хвост без '\n' остаётся.
    #[test]
    fn split_frames_boundaries_and_garbage() {
        let mut buf = b"{\"a\":1}\n\n{\"b\":2}\r\n{\"c".to_vec();
        let lines = split_frames(&mut buf);
        assert_eq!(lines, vec!["{\"a\":1}", "{\"b\":2}"]);
        assert_eq!(buf, b"{\"c", "хвост сохранён");

        buf.extend_from_slice(b"\":3}\n");
        let lines = split_frames(&mut buf);
        assert_eq!(lines, vec!["{\"c\":3}"]);
        assert!(buf.is_empty());
    }

    /// Разрез UTF-8 многобайтового символа по границе чанков не ломает
    /// сборку строки (байты храним, строку собираем только по '\n').
    #[test]
    fn split_frames_multibyte_split() {
        let text = "{\"text\":\"привет\"}";
        let bytes = text.as_bytes();
        let mut buf = bytes[..bytes.len() - 3].to_vec(); // разрезали UTF-8
        assert!(split_frames(&mut buf).is_empty());
        buf.extend_from_slice(&bytes[bytes.len() - 3..]);
        buf.push(b'\n');
        assert_eq!(split_frames(&mut buf), vec![text]);
    }

    /// parse_envelope: валидный запрос, notification без id, мусор → -32700,
    /// невалидная структура → -32600.
    #[test]
    fn parse_envelope_valid_and_invalid() {
        let request = parse_envelope(r#"{"jsonrpc":"2.0","id":7,"method":"ping","params":{}}"#)
            .expect("валидный запрос");
        assert_eq!(request.id, Some(json!(7)));
        assert_eq!(request.method, "ping");
        assert_eq!(request.params, json!({}));

        let notification =
            parse_envelope(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
                .expect("notification");
        assert_eq!(notification.id, None);

        let garbage = parse_envelope("not json at all").expect_err("мусор");
        assert_eq!(garbage.code, -32700);

        let no_method = parse_envelope(r#"{"jsonrpc":"2.0","id":1}"#).expect_err("нет method");
        assert_eq!(no_method.code, -32600);

        let wrong_version =
            parse_envelope(r#"{"jsonrpc":"1.0","id":1,"method":"ping"}"#).expect_err("версия");
        assert_eq!(wrong_version.code, -32600);
    }

    /// build_result/build_error парсятся обратно и несут код/сообщение.
    #[test]
    fn build_envelopes_round_trip() {
        let ok = build_result(&json!(3), &json!({"a": 1}));
        let parsed: Value = serde_json::from_str(&ok).expect("result парсится");
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 3);
        assert_eq!(parsed["result"], json!({"a": 1}));

        let err = build_error(None, -32601, "метод не поддержан: x");
        let parsed: Value = serde_json::from_str(&err).expect("error парсится");
        assert_eq!(parsed["id"], Value::Null);
        assert_eq!(parsed["error"]["code"], -32601);
        assert_eq!(parsed["error"]["message"], "метод не поддержан: x");
    }

    /// initialize: версия клиента проходит, если поддерживается, иначе дефолт.
    /// FR-034: 2025-06-18 эхом — строгие SDK не рвут соединение из-за даунгрейда.
    #[test]
    fn initialize_protocol_negotiation() {
        let latest = initialize_result(Some("2025-06-18"));
        assert_eq!(latest["protocolVersion"], "2025-06-18");

        let accepted = initialize_result(Some("2025-03-26"));
        assert_eq!(accepted["protocolVersion"], "2025-03-26");
        assert_eq!(accepted["capabilities"], json!({"tools": {}}));
        assert_eq!(accepted["serverInfo"]["name"], "canvasdesk");

        let fallback = initialize_result(Some("1999-01-01"));
        assert_eq!(fallback["protocolVersion"], DEFAULT_PROTOCOL);
        let none = initialize_result(None);
        assert_eq!(none["protocolVersion"], DEFAULT_PROTOCOL);
    }

    /// tools/list: 41 инструмент native (40 + monte_carlo_run FR-066;
    /// wasm: 40 — реестр без qmc, §5.8), у каждого inputSchema с required.
    #[test]
    fn tools_list_has_all_with_schemas() {
        let list = tools_list();
        let tools = list["tools"].as_array().expect("массив tools");
        // FR-066 §5.8: monte_carlo_run — native-only (фича qmc не
        // собирается на wasm32 — реестр wasm-сборки без него)
        #[cfg(not(target_arch = "wasm32"))]
        let expected_count = 41;
        #[cfg(target_arch = "wasm32")]
        let expected_count = 40;
        assert_eq!(
            tools.len(),
            expected_count,
            "26 (FR-032/FR-033) + analyze_bottlenecks (FR-016) + 9 whatif_* (FR-017, CP6) + 4 новых: schemes_list/schemes_apply (PRD-0008 Q5) + lineage (PRD-0007 X2) + explain_number (PRD-0007 X6/FR-048, F-9) + monte_carlo_run (FR-066, native)"
        );
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for expected in [
            "canvas_info",
            "nodes_list",
            "node_get",
            "nodes_search",
            "node_create_note",
            "node_create_file",
            "node_update_text",
            "node_edit",
            "node_move",
            "node_resize",
            "node_delete",
            "node_set_color",
            "edge_create",
            "edges_list",
            "edge_delete",
            "flow_set_kind",
            "flow_recalc",
            "flow_cycle_check",
            "edge_ports",
            "edges_list",
            "edge_get",
            "graph_validate",
            "template_list",
            "template_instantiate",
            "schemes_list",
            "schemes_apply",
            "lineage",
            "explain_number",
            "viewport_get",
            "viewport_set",
            "graph_apply",
            "analyze_bottlenecks",
            "whatif_set_override",
            "whatif_set_param",
            "whatif_scenario_list",
            "whatif_scenario_create",
            "whatif_scenario_delete",
            "whatif_scenario_activate",
            "whatif_deltas",
            "whatif_apply",
            "whatif_reset",
        ] {
            assert!(names.contains(&expected), "нет инструмента {expected}");
        }
        // FR-066: monte_carlo_run — только в native-реестре
        #[cfg(not(target_arch = "wasm32"))]
        assert!(
            names.contains(&MC_TOOL_NAME),
            "native-реестр обязан объявлять monte_carlo_run (FR-066)"
        );
        // required по сигнатурам
        let by_name = |name: &str| -> Value {
            tools
                .iter()
                .find(|t| t["name"] == name)
                .unwrap_or_else(|| panic!("инструмент {name}"))
                .clone()
        };
        assert_eq!(
            by_name("node_create_note")["inputSchema"]["required"],
            json!(["x", "y"])
        );
        assert_eq!(
            by_name("node_move")["inputSchema"]["required"],
            json!(["id", "x", "y"])
        );
        assert_eq!(
            by_name("edge_create")["inputSchema"]["required"],
            json!(["from", "to"])
        );
        assert_eq!(
            by_name("node_set_color")["inputSchema"]["properties"]["color"]["enum"],
            json!(["1", "2", "3", "4", "5", "6", null])
        );
        assert_eq!(
            by_name("node_edit")["inputSchema"]["required"],
            json!(["id"])
        );
        assert_eq!(
            by_name("node_edit")["inputSchema"]["properties"]["label"]["type"],
            json!(["string", "null"])
        );
        assert_eq!(
            by_name("edge_create")["inputSchema"]["properties"]["fromSide"]["enum"],
            json!(["any", "top", "right", "bottom", "left"])
        );
        // FR-032: новые инструменты чтения/валидации
        assert_eq!(
            by_name("edge_get")["inputSchema"]["required"],
            json!(["id"])
        );
        assert_eq!(by_name("edges_list")["inputSchema"]["required"], json!([]));
        assert_eq!(
            by_name("graph_validate")["inputSchema"]["required"],
            json!([])
        );
        // PRD-0008 Q5 / PRD-0007 X2/X6: схемы галереи + lineage + explain_number
        assert_eq!(
            by_name("schemes_list")["inputSchema"]["required"],
            json!([])
        );
        assert_eq!(
            by_name("explain_number")["inputSchema"]["required"],
            json!(["node_id"])
        );
        assert_eq!(
            by_name("explain_number")["inputSchema"]["properties"]["line"]["type"],
            json!(["integer", "null"])
        );
        assert_eq!(
            by_name("schemes_apply")["inputSchema"]["required"],
            json!(["id"])
        );
        assert_eq!(
            by_name("schemes_apply")["inputSchema"]["properties"]["x"]["type"],
            json!("number")
        );
        assert_eq!(
            by_name("lineage")["inputSchema"]["required"],
            json!(["node_id"])
        );
        assert_eq!(
            by_name("lineage")["inputSchema"]["properties"]["line"]["type"],
            json!(["integer", "null"])
        );
        // FR-033: схема graph_apply — операции с тегом op, лимиты массива
        let ops = &by_name("graph_apply")["inputSchema"]["properties"]["operations"];
        assert_eq!(ops["type"], "array");
        assert_eq!(ops["minItems"], 1);
        assert_eq!(ops["maxItems"], 256);
        assert_eq!(ops["items"]["required"], json!(["op"]));
        let tags: Vec<&str> = ops["items"]["properties"]["op"]["enum"]
            .as_array()
            .expect("enum op")
            .iter()
            .filter_map(Value::as_str)
            .collect();
        for expected in [
            "node_create_note",
            "node_create_file",
            "template_instantiate",
            "edge_create",
            "edge_delete",
            "param_set",
            "node_move",
        ] {
            assert!(tags.contains(&expected), "нет операции {expected}");
        }
    }

    // --- Пакет скиллов MCP (skills/): контракт синхронности с реестром ---
    // Правила проверок и протокол обновления пакета — skills/UPDATE-PROTOCOL.md.
    // Файлы встраиваются include_str! (компайл-тайм — работает и под wasm);
    // новый файл скилла добавляется в списки ниже осознанно.

    /// Канонические имена всех инструментов native-продукта (FR-066 §5.8:
    /// wasm-реестр — подмножество без monte_carlo_run, но скиллы описывают
    /// весь продукт — синхронность проверяется по native-виду на обеих
    /// платформах).
    fn canonical_tool_names() -> Vec<&'static str> {
        let mut names: Vec<&'static str> = TOOLS.iter().map(|tool| tool.name).collect();
        names.push(MC_TOOL_NAME);
        names
    }

    /// Все markdown-файлы пакета скиллов (для call-позиций).
    fn skills_package_text() -> String {
        [
            include_str!("../../../skills/README.md"),
            include_str!("../../../skills/UPDATE-PROTOCOL.md"),
            include_str!("../../../skills/CHANGELOG.md"),
            include_str!("../../../skills/canvasdesk-mcp/SKILL.md"),
            include_str!("../../../skills/canvasdesk-mcp/references/tools.md"),
            include_str!("../../../skills/canvasdesk-model-build/SKILL.md"),
            include_str!("../../../skills/canvasdesk-model-verify/SKILL.md"),
            include_str!("../../../skills/canvasdesk-whatif/SKILL.md"),
        ]
        .concat()
    }

    /// Тела четырёх SKILL.md (для проверки покрытия инструментами).
    fn skills_bodies_text() -> String {
        [
            include_str!("../../../skills/canvasdesk-mcp/SKILL.md"),
            include_str!("../../../skills/canvasdesk-model-build/SKILL.md"),
            include_str!("../../../skills/canvasdesk-model-verify/SKILL.md"),
            include_str!("../../../skills/canvasdesk-whatif/SKILL.md"),
        ]
        .concat()
    }

    /// Токены в бэктиках в «позиции вызова» — `` `имя` {…} `` в той же
    /// строке: такая форма в пакете означает вызов инструмента, поэтому
    /// имя обязано быть в реестре (или в CALL_POSITION_OPS ниже).
    fn call_position_tokens(text: &str) -> Vec<String> {
        let mut tokens = Vec::new();
        for line in text.lines() {
            let bytes = line.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] != b'`' {
                    i += 1;
                    continue;
                }
                let Some(close) = bytes[i + 1..].iter().position(|b| *b == b'`') else {
                    break; // незакрытый бэктик на строке — дальше строки
                };
                let j = i + 1 + close;
                let token = &line[i + 1..j];
                let mut k = j + 1;
                while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                    k += 1;
                }
                if k < bytes.len() && bytes[k] == b'{' {
                    tokens.push(token.to_owned());
                }
                i = j + 1;
            }
        }
        tokens
    }

    /// Операции graph_apply, легитимные в call-позиции скиллов, но не
    /// являющиеся MCP-инструментами. Расширять осознанно (UPDATE-PROTOCOL).
    const CALL_POSITION_OPS: &[&str] = &["param_set"];

    /// Правило 1 (полнота): каталог skills/ описывает каждый инструмент
    /// native-реестра (FR-066: wasm-сборка отдаёт подмножество — каталог
    /// описывает продукт целиком).
    #[test]
    fn skills_catalog_covers_every_tool() {
        let catalog = include_str!("../../../skills/canvasdesk-mcp/references/tools.md");
        for tool in canonical_tool_names() {
            assert!(
                catalog.contains(tool),
                "skills: каталог references/tools.md не описывает инструмент {tool} — \
                 обновите пакет (skills/UPDATE-PROTOCOL.md)",
            );
        }
    }

    /// Правило 2 (покрытие): каждый инструмент native-продукта упомянут
    /// хотя бы в одном SKILL.md — новый инструмент обязан получить зону
    /// ответственности (FR-066: monte_carlo_run — canvasdesk-model-verify).
    #[test]
    fn skills_bodies_mention_every_tool() {
        let bodies = skills_bodies_text();
        for tool in canonical_tool_names() {
            assert!(
                bodies.contains(tool),
                "skills: инструмент {tool} не упомянут ни в одном SKILL.md — \
                 отнесите его к зоне скилла (skills/UPDATE-PROTOCOL.md)",
            );
        }
    }

    /// Правило 3 (счётчик): README пакета несёт актуальное число
    /// инструментов native-продукта («N инструмент…» — с любым окончанием
    /// слова; FR-066: 41 — включая native-only monte_carlo_run).
    #[test]
    fn skills_readme_tool_counter_is_current() {
        let readme = include_str!("../../../skills/README.md");
        let count = canonical_tool_names().len();
        assert!(
            readme.contains(&format!("{} инструмент", count)),
            "skills/README.md не содержит актуальный счётчик «{} инструмент(ов…)» — \
             обновите пакет (skills/UPDATE-PROTOCOL.md)",
            count
        );
    }

    /// Правило 4 (call-позиции): форма `` `имя` {…} `` в пакете — вызов;
    /// имя обязано быть инструментом native-продукта или операцией батча.
    /// Ловит вызовы удалённых/переименованных инструментов.
    #[test]
    fn skills_call_positions_are_registered_tools() {
        let known_names = canonical_tool_names();
        for token in call_position_tokens(&skills_package_text()) {
            let looks_like_identifier =
                token.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                    && token
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !looks_like_identifier {
                continue; // проза в бэктиках перед '{' — не вызов
            }
            let known = known_names.contains(&token.as_str())
                || CALL_POSITION_OPS.contains(&token.as_str());
            assert!(
                known,
                "skills: call-позиция `{token} {{…}}` не является инструментом \
                 реестра TOOLS ни операцией батча — переименованный/удалённый \
                 инструмент? (skills/UPDATE-PROTOCOL.md)"
            );
        }
    }

    /// Автомат: initialize → initialized → tools/list → tools/call форвардит
    /// строку и разворачивает конверт приложения в text + structuredContent
    /// (FR-034; FakeTransport теперь возвращает конверт, как прод-`on_mcp_wake`).
    #[test]
    fn handshake_and_call_with_connected_pipe() {
        let mut transport = Some(FakeTransport::connected(&[
            r#"{"jsonrpc":"2.0","id":3,"result":{"nodes":3}}"#,
        ]));
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test"}}}"#;
        let HandleOutcome::Reply(reply) = handle_line(line, &mut transport) else {
            panic!("initialize должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("initialize ответ");
        assert_eq!(parsed["result"]["protocolVersion"], "2025-06-18");

        assert_eq!(
            handle_line(
                r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                &mut transport
            ),
            HandleOutcome::Silent
        );

        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            &mut transport,
        ) else {
            panic!("tools/list должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("tools/list ответ");
        // FR-066 §5.8: monte_carlo_run — native-only (wasm: 40 без qmc)
        #[cfg(not(target_arch = "wasm32"))]
        let expected_count = 41;
        #[cfg(target_arch = "wasm32")]
        let expected_count = 40;
        assert_eq!(
            parsed["result"]["tools"].as_array().expect("tools").len(),
            expected_count,
            "26 (FR-032/FR-033) + analyze_bottlenecks (FR-016) + 9 whatif_* (FR-017, CP6) + schemes_list/schemes_apply + lineage + explain_number (PRD-0007 X6, F-9) + monte_carlo_run (FR-066, native)"
        );

        let call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"canvas_info","arguments":{}}}"#;
        let HandleOutcome::Reply(reply) = handle_line(call, &mut transport) else {
            panic!("tools/call должен ответить");
        };
        let transport = transport.as_ref().expect("транспорт");
        assert_eq!(transport.sent, vec![call], "строка форвардится как есть");
        let parsed: Value = serde_json::from_str(&reply).expect("call ответ");
        assert_eq!(parsed["id"], 3);
        // FR-034: конверт приложения развёрнут — клиент видит чистый результат
        assert_eq!(
            parsed["result"]["content"],
            json!([{ "type": "text", "text": r#"{"nodes":3}"# }])
        );
        assert_eq!(parsed["result"]["structuredContent"], json!({"nodes":3}));
        assert!(!parsed["result"]
            .as_object()
            .unwrap()
            .contains_key("isError"));
    }

    /// PRD-0007 X6 (F-9): text-first результат (render:"text") — content text
    /// несёт готовый текст объяснения (не JSON-эхо), structuredContent —
    /// полный объект; обычные результаты не затронуты (JSON-эхо как раньше).
    #[test]
    fn call_result_text_first_marker() {
        let id = json!(7);
        let result = json!({
            "render": "text",
            "text": "Цепочка расчёта: Итог [b] = 10\nВсего узлов: 2 (листьев: 1)",
            "root": {"node_id": "b", "line": null},
            "nodes": 2,
            "truncated": false,
        });
        let parsed: Value =
            serde_json::from_str(&build_call_result(&id, &result)).expect("конверт");
        assert_eq!(
            parsed["result"]["content"],
            json!([{
                "type": "text",
                "text": "Цепочка расчёта: Итог [b] = 10\nВсего узлов: 2 (листьев: 1)",
            }]),
            "content text — готовое объяснение, не JSON"
        );
        assert_eq!(parsed["result"]["structuredContent"], result);

        // Инструмент без маркера — прежнее поведение (JSON-эхо), даже
        // если в объекте есть поле text (например node_get с текстом ноды).
        let plain = json!({"id": "note-1", "text": "A\n= 5"});
        let parsed: Value = serde_json::from_str(&build_call_result(&id, &plain)).expect("конверт");
        assert_eq!(
            parsed["result"]["content"],
            json!([{ "type": "text", "text": plain.to_string() }])
        );
    }

    /// FR-034: разворот конверта приложения — error-конверт → isError;
    /// isError-результат проходит насквозь (без двойной упаковки);
    /// легаси не-конвертный payload — как есть, без structuredContent.
    #[test]
    fn call_result_unwrapping() {
        let mut transport = Some(FakeTransport::connected(&[
            r#"{"jsonrpc":"2.0","id":9,"error":{"code":-32601,"message":"нет ноды"}}"#,
        ]));
        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"node_get","arguments":{"id":"x"}}}"#,
            &mut transport,
        ) else {
            panic!("tools/call должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("ответ парсится");
        assert_eq!(parsed["result"]["isError"], true);
        assert!(parsed["result"]["content"][0]["text"]
            .as_str()
            .expect("текст")
            .contains("нет ноды"));

        let mut transport = Some(FakeTransport::connected(&[
            r#"{"jsonrpc":"2.0","id":4,"result":{"isError":true,"content":[{"type":"text","text":"упс"}]}}"#,
        ]));
        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"node_get","arguments":{}}}"#,
            &mut transport,
        ) else {
            panic!("tools/call должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("ответ парсится");
        assert_eq!(parsed["result"]["isError"], true);
        assert_eq!(parsed["result"]["content"][0]["text"], "упс");

        let mut transport = Some(FakeTransport::connected(&[r#"[1,2]"#]));
        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"nodes_list","arguments":{}}}"#,
            &mut transport,
        ) else {
            panic!("tools/call должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("ответ парсится");
        assert_eq!(parsed["result"]["content"][0]["text"], json!("[1,2]"));
        assert!(parsed["result"]
            .as_object()
            .unwrap()
            .get("structuredContent")
            .is_none());
    }

    /// FR-034: batch-массив — ответы по каждому запросу; уведомления молчат;
    /// пустой батч и мусорный элемент → -32600; одиночная строка как раньше.
    #[test]
    fn batch_requests() {
        let mut transport: Option<FakeTransport> = None;
        let outcomes = handle_input(
            r#"[{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}},{"jsonrpc":"2.0","id":2,"method":"ping"}]"#,
            &mut transport,
        );
        assert_eq!(outcomes.len(), 2, "по ответу на каждый запрос");
        let HandleOutcome::Reply(first) = &outcomes[0] else {
            panic!("первый — ответ");
        };
        let parsed: Value = serde_json::from_str(first).expect("initialize ответ");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["result"]["protocolVersion"], "2025-06-18");
        let HandleOutcome::Reply(second) = &outcomes[1] else {
            panic!("второй — ответ");
        };
        let parsed: Value = serde_json::from_str(second).expect("ping ответ");
        assert_eq!(parsed["id"], 2);
        assert_eq!(parsed["result"], json!({}));

        // Батч с уведомлением: Reply на запрос, Silent на уведомление
        // (stdout пишет только Reply — Silent пропускается циклом run_stdio)
        let outcomes = handle_input(
            r#"[{"jsonrpc":"2.0","id":3,"method":"ping"},{"jsonrpc":"2.0","method":"notifications/initialized"}]"#,
            &mut transport,
        );
        assert_eq!(outcomes.len(), 2);
        assert!(matches!(outcomes[0], HandleOutcome::Reply(_)));
        assert_eq!(outcomes[1], HandleOutcome::Silent);

        // Батч целиком из уведомлений — тишина (ни одного Reply, stdout молчит)
        let outcomes = handle_input(
            r#"[{"jsonrpc":"2.0","method":"notifications/initialized"},{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}]"#,
            &mut transport,
        );
        assert!(outcomes.iter().all(|o| matches!(o, HandleOutcome::Silent)));

        // Пустой батч невалиден
        let outcomes = handle_input("[]", &mut transport);
        let HandleOutcome::Reply(reply) = &outcomes[0] else {
            panic!("ответ ожидался");
        };
        let parsed: Value = serde_json::from_str(reply).expect("ответ парсится");
        assert_eq!(parsed["error"]["code"], -32600);

        // Мусорный элемент батча — -32600 по нему, сосед отвечает
        let outcomes = handle_input(
            r#"[42,{"jsonrpc":"2.0","id":7,"method":"ping"}]"#,
            &mut transport,
        );
        assert_eq!(outcomes.len(), 2);
        let HandleOutcome::Reply(reply) = &outcomes[0] else {
            panic!("ответ ожидался");
        };
        let parsed: Value = serde_json::from_str(reply).expect("ответ парсится");
        assert_eq!(parsed["error"]["code"], -32600);

        // Одиночная строка-мусор — как раньше -32700
        let outcomes = handle_input("}}}", &mut transport);
        assert_eq!(outcomes.len(), 1);
        let HandleOutcome::Reply(reply) = &outcomes[0] else {
            panic!("ответ ожидался");
        };
        let parsed: Value = serde_json::from_str(reply).expect("ответ парсится");
        assert_eq!(parsed["error"]["code"], -32700);
    }

    /// ADR-0009: недоступное приложение — не краш. Handshake успешен всегда;
    /// tools/call → isError «не запущен» (и с None, и с разорванным транспортом).
    #[test]
    fn offline_handshake_and_calls() {
        let mut transport: Option<FakeTransport> = None;
        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#,
            &mut transport,
        ) else {
            panic!("initialize должен ответить и без приложения");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("initialize ответ");
        assert_eq!(parsed["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(parsed["result"]["serverInfo"]["name"], "canvasdesk");

        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"node_get","arguments":{"id":"x"}}}"#,
            &mut transport,
        ) else {
            panic!("tools/call должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("isError ответ");
        assert_eq!(parsed["result"]["isError"], true);
        assert!(parsed["result"]["content"][0]["text"]
            .as_str()
            .expect("текст")
            .contains("CanvasDesk не запущен"));

        // Разорванный транспорт — то же поведение isError
        let mut disconnected = Some(FakeTransport {
            sent: Vec::new(),
            inbox: VecDeque::new(),
            connected: false,
        });
        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"canvas_info","arguments":{}}}"#,
            &mut disconnected,
        ) else {
            panic!("tools/call должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("isError ответ");
        assert_eq!(parsed["result"]["isError"], true);
    }

    /// FR-034: толерантные заглушки read-only методов и notifications/cancelled
    /// (хосты зондируют их безотносительно заявленных capabilities).
    #[test]
    fn read_only_stubs_and_cancelled() {
        let mut transport: Option<FakeTransport> = None;
        for (method, key) in [
            ("resources/list", "resources"),
            ("prompts/list", "prompts"),
            ("resources/templates/list", "resourceTemplates"),
        ] {
            let line = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}"}}"#);
            let HandleOutcome::Reply(reply) = handle_line(&line, &mut transport) else {
                panic!("{method} должен ответить");
            };
            let parsed: Value = serde_json::from_str(&reply).expect("ответ парсится");
            assert_eq!(parsed["result"][key], json!([]), "{method}");
        }

        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"logging/setLevel","params":{"level":"info"}}"#,
            &mut transport,
        ) else {
            panic!("logging/setLevel должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("ответ парсится");
        assert_eq!(parsed["result"], json!({}));

        assert_eq!(
            handle_line(
                r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":5}}"#,
                &mut transport
            ),
            HandleOutcome::Silent
        );
    }

    /// Таймаут ответа приложения (recv_line → None) — isError с текстом про таймаут.
    #[test]
    fn call_timeout_is_error() {
        let mut transport = Some(FakeTransport::connected(&[]));
        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"nodes_list","arguments":{}}}"#,
            &mut transport,
        ) else {
            panic!("tools/call должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("isError ответ");
        assert_eq!(parsed["result"]["isError"], true);
        assert!(parsed["result"]["content"][0]["text"]
            .as_str()
            .expect("текст")
            .contains("таймаут"));
    }

    /// Неизвестный метод → JSON-RPC -32601; notification с мусором → -32700.
    #[test]
    fn unknown_method_and_garbage() {
        let mut transport: Option<FakeTransport> = None;
        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":5,"method":"resources/read","params":{}}"#,
            &mut transport,
        ) else {
            panic!("ответ ожидался");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("ответ парсится");
        assert_eq!(parsed["error"]["code"], -32601);

        let HandleOutcome::Reply(reply) = handle_line("}}}", &mut transport) else {
            panic!("ответ ожидался");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("ответ парсится");
        assert_eq!(parsed["error"]["code"], -32700);
    }

    /// FR-035/ADR-0010: автоспавн изолирует stdio — на всех трёх стандартных
    /// дескрипторах ребёнка /dev/null, никакого наследования stdout/stderr
    /// моста (корень «Invalid JSON \x1b[2m…» у hermes) и кражи stdin.
    /// Поведенческая проверка: ребёнок сам читает свои fd и репортит в файл
    /// (unix; на Windows тот же код применяет Stdio::null).
    /// wasm исключён — реальная ФС/процессы, test-only cfg, прецедент FR-036.
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    #[test]
    fn spawn_service_command_isolates_stdio() {
        let dir = std::env::temp_dir().join(format!(
            "canvasdesk_mcp_fr035_stdio_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let report = dir.join("fds.txt");
        // ВАЖНО: тип fd фиксируем ДО любого редиректа (redirect printf на
        // файл сам бы переключил fd1). /dev/null — символьное устройство,
        // файл/pipe — нет: «ccc» = все три дескриптора /dev/null.
        let script = format!(
            "[ -c /dev/fd/0 ] && a=c || a=x; [ -c /dev/fd/1 ] && b=c || b=x; \
             [ -c /dev/fd/2 ] && d=c || d=x; printf '%s%s%s\\n' \"$a\" \"$b\" \"$d\" > '{}'",
            report.display()
        );
        let mut cmd = spawn_service_command(std::path::Path::new("sh"));
        cmd.arg("-c").arg(&script);
        let status = cmd.status().expect("sh доступен");
        assert!(status.success(), "ребёнок отчитался без ошибок");
        let text = std::fs::read_to_string(&report).expect("отчёт ребёнка");
        assert_eq!(
            text.trim(),
            "ccc",
            "stdin/stdout/stderr — /dev/null: {text:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FR-035/ADR-0010: ориентация автоспавна. Автономный мост
    /// `canvasdesk-mcp(.exe)` спавнит GUI-соседа `canvasdesk.exe`, а не сам
    /// себя (рекурсия двойников); соседа нет → None (offline). Единый
    /// бинарь `canvasdesk(.exe)` спавнит сам себя (GUI-режим без аргументов).
    /// wasm исключён — реальная ФС/процессы, test-only cfg, прецедент FR-036.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn autosprawn_target_prefers_sibling_gui_for_standalone_bridge() {
        let dir = std::env::temp_dir().join(format!(
            "canvasdesk_mcp_fr035_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("tmp dir");
        let bridge = dir.join("canvasdesk-mcp.exe");
        let gui = dir.join("canvasdesk.exe");
        std::fs::write(&bridge, b"").expect("файл моста");

        // GUI-соседа нет → автоспавн невозможен (offline-режим ADR-0009)
        assert_eq!(autosprawn_target(&bridge), None);
        // Сосед появился → спавним ЕГО, не мост
        std::fs::write(&gui, b"").expect("файл GUI");
        assert_eq!(autosprawn_target(&bridge), Some(gui.clone()));
        // Единый бинарь — сам себе GUI-цель
        assert_eq!(autosprawn_target(&gui), Some(gui.clone()));
        // Регистронезависимый стем (Windows-дистрибутивы бывают CAPS)
        let caps = dir.join("CANVASDESK-MCP.EXE");
        std::fs::write(&caps, b"").expect("файл CAPS");
        assert_eq!(autosprawn_target(&caps), Some(gui.clone()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
