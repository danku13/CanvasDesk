//! canvasdesk-mcp — исполняемый MCP-сервер-посредник: stdin/stdout ↔ named
//! pipe canvas-app (`\\.\pipe\canvasdesk`).
//!
//! Совместимая обёртка (FR-008): весь цикл и транспорт живут в библиотеке
//! `canvas_mcp::run_stdio` — тот же код исполняет и подкоманда
//! `canvasdesk mcp`, так что стек поднимается одним бинарником. Флаг
//! `--no-spawn` отключает автостарт сервиса при недоступном pipe.
//!
//! Автоспавн (FR-035/ADR-0010): ребёнок поднимается с изолированным stdio
//! (`Stdio::null()` — наследование хэндлов моста давало ANSI-логи GUI в
//! JSON-RPC-канале). Ориентация по бинарю: `canvasdesk-mcp.exe` спавнит
//! GUI-соседа `canvasdesk.exe` из своего каталога дистрибутива (не самого
//! себя — рекурсия двойников); соседа нет — offline-режим ADR-0009.
//! stdout моста — только newline-delimited JSON-RPC; диагностика — stderr.
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
