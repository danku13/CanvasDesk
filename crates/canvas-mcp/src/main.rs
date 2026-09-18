//! canvasdesk-mcp — исполняемый MCP-сервер-посредник: stdin/stdout ↔ named
//! pipe canvas-app (`\\.\pipe\canvasdesk`).
//!
//! Совместимая обёртка (FR-008): весь цикл и транспорт живут в библиотеке
//! `canvas_mcp::run_stdio` — тот же код исполняет и подкоманда
//! `canvasdesk mcp`, так что стек поднимается одним бинарником. Флаг
//! `--no-spawn` отключает автостарт сервиса при недоступном pipe.
//!
//! Протокол по stdio — newline-delimited JSON-RPC 2.0 (без Content-Length,
//! как предписывает MCP spec): каждое сообщение — одна строка, batch-массивы
//! разрешены (FR-034). На pipe уходит тот же line-framing. Handshake не
//! зависит от приложения (ADR-0009): initialize успешен всегда; pipe
//! недоступен → tools/call отвечает isError «не запущен», мост живёт, пока
//! жив stdio, и сам переподключается к pipe (FR-034).

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    canvas_mcp::run_stdio(&args)
}
