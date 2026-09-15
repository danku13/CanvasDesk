//! canvas-mcp — MCP-клиент-посредник: stdio (newline-delimited JSON-RPC 2.0,
//! БЕЗ Content-Length, как предписывает MCP spec) ↔ named pipe запущенного
//! canvas-app (`\\.\pipe\canvasdesk`, тот же line-framing).
//!
//! Вся логика — чистые функции в этом модуле (framing, JSON-RPC-огибающие,
//! tools/list, автомат handshake/tools-call), чтобы тестироваться без pipe.
//! Автостарта приложения нет: pipe недоступен → на `initialize` отвечаем
//! JSON-RPC ошибкой и завершаемся с кодом 2 (поведение задаёт bin через
//! `HandleOutcome::Exit`).

use serde_json::{json, Value};
use std::time::Duration;

/// Pipe, который слушает canvas-app (SPEC: один канал на инстанс приложения).
pub const PIPE_NAME: &str = r"\\.\pipe\canvasdesk";
/// Протокольные версии MCP, которые мы заявляем (последняя — первая).
pub const SUPPORTED_PROTOCOLS: [&str; 2] = ["2025-03-26", "2024-11-05"];
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

/// Результат обработки одной входной строки stdio.
#[derive(Debug, Clone, PartialEq)]
pub enum HandleOutcome {
    /// Ответить строкой на stdio.
    Reply(String),
    /// Молчать (notification без результата).
    Silent,
    /// Ответить и завершить процесс с кодом (pipe недоступен на initialize).
    Exit { code: i32, reply: String },
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

/// Ответ tools/call: единственный text-контент, payload — JSON-строка.
pub fn build_call_result(id: &Value, payload: &str) -> String {
    build_result(
        id,
        &json!({ "content": [{ "type": "text", "text": payload }] }),
    )
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

/// 16 инструментов канваса (сигнатуры — план MCP-задачи; FR-005 — node_edit).
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
        description: "Поиск нод: подстрока без учёта регистра по text/label/file",
        required: &["query"],
        properties: &[("query", STR)],
    },
    ToolSpec {
        name: "node_create_note",
        description: "Создать ноду-заметку; возвращает id. Размеры по умолчанию 260×120",
        required: &["x", "y"],
        properties: &[("x", NUM), ("y", NUM), ("text", STR), ("width", NUM), ("height", NUM)],
    },
    ToolSpec {
        name: "node_create_file",
        description: "Создать файловую ноду по пути (файл на диске НЕ создаётся); возвращает id",
        required: &["path", "x", "y"],
        properties: &[("path", STR), ("x", NUM), ("y", NUM), ("width", NUM), ("height", NUM)],
    },
    ToolSpec {
        name: "node_update_text",
        description: "Заменить текст ноды-заметки",
        required: &["id", "text"],
        properties: &[("id", STR), ("text", STR)],
    },
    ToolSpec {
        name: "node_edit",
        description: "Редактировать ноду одним вызовом: обновляет ТОЛЬКО переданные поля (text, label, color, expr, x, y, width, height); label/color/expr = null — сброс; expr — Numi-style формула, результат рендерится под текстом ноды (невалидная формула — ошибка, нода не меняется); возвращает обновлённую ноду",
        required: &["id"],
        properties: &[
            ("id", STR),
            ("text", STR),
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
        description: "Изменить размеры ноды (width, height)",
        required: &["id", "width", "height"],
        properties: &[("id", STR), ("width", NUM), ("height", NUM)],
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
        description: "Создать связь между нодами; стороны any|top|right|bottom|left (дефолт any — авто); возвращает id",
        required: &["from", "to"],
        properties: &[
            ("from", STR),
            ("to", STR),
            ("fromSide", SIDE_PROP),
            ("toSide", SIDE_PROP),
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
        description: "FR-014: пересчитать весь граф потока значений; возвращает карту {node_id: {value, unit}} для формульных нод (ошибки — {error: текст}); downstream учитывает значения upstream",
        required: &[],
        properties: &[],
    },
    ToolSpec {
        name: "flow_cycle_check",
        description: "FR-014: проверка DAG-инварианта value-рёбер: [] — циклов нет, иначе список id участников цикла",
        required: &[],
        properties: &[],
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
        name: "template_list",
        description: "FR-018: список шаблонов реестра — id, name, version, category, description, expr (Numi-формула с $param), icon, color, params ({type, default, unit?, min?, max?}). Те же шаблоны, что видит пользователь в палитре (Ctrl+P) и wheel-меню",
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
];

/// tools/list: массив дескрипторов с name/description/inputSchema.
pub fn tools_list() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
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

/// Текст ошибки «приложение не запущено» (единый для initialize и tools/call).
fn not_running_message() -> String {
    format!("CanvasDesk не запущен (pipe {PIPE_NAME} не найден)")
}

/// Обработать одну строку stdio в конечном автомате MCP.
///
/// - `initialize` → ответ с protocolVersion/capabilities/serverInfo; pipe
///   недоступен → JSON-RPC ошибка + `Exit(2)` (завершение делает bin);
/// - `notifications/initialized` → Silent;
/// - `ping` → `{}`;
/// - `tools/list` → 18 инструментов с inputSchema;
/// - `tools/call` → форвард строки на pipe, ответ приложения — в text-контенте;
///   pipe мёртв → isError «не запущен», таймаут ответа (в транспорте) → isError;
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
            if transport.as_ref().is_some_and(AppTransport::is_connected) {
                let client_version = request
                    .params
                    .get("protocolVersion")
                    .and_then(Value::as_str);
                HandleOutcome::Reply(build_result(&id, &initialize_result(client_version)))
            } else {
                HandleOutcome::Exit {
                    code: 2,
                    reply: build_error(Some(&id), -32002, &not_running_message()),
                }
            }
        }
        "notifications/initialized" => HandleOutcome::Silent,
        "ping" => HandleOutcome::Reply(build_result(&id, &json!({}))),
        "tools/list" => HandleOutcome::Reply(build_result(&id, &tools_list())),
        "tools/call" => {
            let Some(pipe) = transport.as_mut().filter(|t| t.is_connected()) else {
                return HandleOutcome::Reply(build_call_error(&id, &not_running_message()));
            };
            if let Err(err) = pipe.send_line(line) {
                return HandleOutcome::Reply(build_call_error(&id, &err));
            }
            match pipe.recv_line() {
                Some(payload) => HandleOutcome::Reply(build_call_result(&id, &payload)),
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

/// Заглушка транспорта для сборки вне Windows: pipe всегда недоступен —
/// bin ответит ошибкой на initialize (код 2).
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

/// Запустить stdio-MCP-посредник (FR-008): цикл бинарника `canvasdesk-mcp`,
/// доступный и как `canvasdesk mcp` — один exe на весь стек. Автостарт: если
/// pipe приложения недоступен и в `args` нет `--no-spawn`, поднимаем сервис
/// (`current_exe` без аргументов, это GUI-режим того же бинарника) и ждём
/// pipe до SPAWN_WAIT_SECS — «сервис + MCP одной командой». Неудача — прежнее
/// поведение: initialize ответит JSON-RPC-ошибкой и вернёт код 2.
/// Молчалив по stdout (там протокол MCP); диагностика — в stderr.
pub fn run_stdio(args: &[String]) -> anyhow::Result<()> {
    let no_spawn = args.iter().any(|arg| arg == "--no-spawn");
    #[cfg(windows)]
    let mut transport = connect_app(no_spawn);
    // Вне Windows pipe нет — OfflineTransport::None, lib ответит ошибкой
    // на initialize (код 2); автостарта не делаем (GUI-режим Windows-only)
    #[cfg(not(windows))]
    let _ = no_spawn;
    #[cfg(not(windows))]
    let mut transport: Option<OfflineTransport> = None;

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
            match handle_line(&line, &mut transport) {
                HandleOutcome::Reply(reply) => {
                    writeln!(stdout, "{reply}")?;
                    stdout.flush()?;
                }
                HandleOutcome::Silent => {}
                HandleOutcome::Exit { code, reply } => {
                    writeln!(stdout, "{reply}")?;
                    stdout.flush()?;
                    std::process::exit(code);
                }
            }
        }
    }
    Ok(())
}

/// Подключение к приложению с автостартом (FR-008): сервис уже работает —
/// короткое ожидание; нет — спавним себя (GUI-режим) и ждём подъёма pipe.
#[cfg(windows)]
fn connect_app(no_spawn: bool) -> Option<PipeTransport> {
    if let Some(transport) = try_connect(CONNECT_WAIT_MS) {
        return Some(transport);
    }
    if no_spawn {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    match std::process::Command::new(exe).spawn() {
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
    #[test]
    fn initialize_protocol_negotiation() {
        let accepted = initialize_result(Some("2025-03-26"));
        assert_eq!(accepted["protocolVersion"], "2025-03-26");
        assert_eq!(accepted["capabilities"], json!({"tools": {}}));
        assert_eq!(accepted["serverInfo"]["name"], "canvasdesk");

        let fallback = initialize_result(Some("1999-01-01"));
        assert_eq!(fallback["protocolVersion"], DEFAULT_PROTOCOL);
        let none = initialize_result(None);
        assert_eq!(none["protocolVersion"], DEFAULT_PROTOCOL);
    }

    /// tools/list: ровно 20 инструментов, у каждого inputSchema с required.
    #[test]
    fn tools_list_has_all_with_schemas() {
        let list = tools_list();
        let tools = list["tools"].as_array().expect("массив tools");
        assert_eq!(tools.len(), 22, "ровно 22 инструмента");
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
            "edge_delete",
            "flow_set_kind",
            "flow_recalc",
            "flow_cycle_check",
            "edge_ports",
            "template_list",
            "template_instantiate",
            "viewport_get",
            "viewport_set",
        ] {
            assert!(names.contains(&expected), "нет инструмента {expected}");
        }
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
    }

    /// Автомат: initialize → initialized → tools/list → tools/call форвардит
    /// строку и заворачивает ответ приложения в text-контент.
    #[test]
    fn handshake_and_call_with_connected_pipe() {
        let mut transport = Some(FakeTransport::connected(&[r#"{"nodes":3}"#]));
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test"}}}"#;
        let HandleOutcome::Reply(reply) = handle_line(line, &mut transport) else {
            panic!("initialize должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("initialize ответ");
        assert_eq!(parsed["result"]["protocolVersion"], "2025-03-26");

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
        assert_eq!(
            parsed["result"]["tools"].as_array().expect("tools").len(),
            22
        );

        let call = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"canvas_info","arguments":{}}}"#;
        let HandleOutcome::Reply(reply) = handle_line(call, &mut transport) else {
            panic!("tools/call должен ответить");
        };
        let transport = transport.as_ref().expect("транспорт");
        assert_eq!(transport.sent, vec![call], "строка форвардится как есть");
        let parsed: Value = serde_json::from_str(&reply).expect("call ответ");
        assert_eq!(parsed["id"], 3);
        assert_eq!(
            parsed["result"]["content"],
            json!([{ "type": "text", "text": r#"{"nodes":3}"# }])
        );
        assert!(!parsed["result"]
            .as_object()
            .unwrap()
            .contains_key("isError"));
    }

    /// Pipe недоступен: initialize → Exit(2) с JSON-RPC ошибкой; tools/call → isError.
    #[test]
    fn pipe_unavailable_scenarios() {
        let mut transport: Option<FakeTransport> = None;
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test"}}}"#;
        let outcome = handle_line(line, &mut transport);
        let HandleOutcome::Exit { code, reply } = outcome else {
            panic!("ожидался Exit, получено {outcome:?}");
        };
        assert_eq!(code, 2);
        let parsed: Value = serde_json::from_str(&reply).expect("ошибка парсится");
        assert!(parsed["error"]["message"]
            .as_str()
            .expect("сообщение")
            .contains("CanvasDesk не запущен"));

        // С отсоединённым транспортом tools/call — isError, не JSON-RPC error
        let mut disconnected = Some(FakeTransport {
            sent: Vec::new(),
            inbox: VecDeque::new(),
            connected: false,
        });
        let HandleOutcome::Reply(reply) = handle_line(
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"node_get","arguments":{"id":"x"}}}"#,
            &mut disconnected,
        ) else {
            panic!("tools/call должен ответить");
        };
        let parsed: Value = serde_json::from_str(&reply).expect("isError ответ");
        assert_eq!(parsed["result"]["isError"], true);
        assert!(parsed["result"]["content"][0]["text"]
            .as_str()
            .expect("текст")
            .contains("CanvasDesk не запущен"));
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
}
