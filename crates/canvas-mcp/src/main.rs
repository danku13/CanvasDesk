//! canvasdesk-mcp — исполняемый MCP-посредник: stdin/stdout ↔ named pipe
//! canvas-app (`\\.\pipe\canvasdesk`).
//!
//! Совместимая обёртка (FR-008): весь цикл и транспорт живут в библиотеке
//! `canvas_mcp::run_stdio` — тот же код исполняет и подкоманда
//! `canvasdesk mcp`, так что стек поднимается одним бинарником. Флаг
//! `--no-spawn` отключает автостарт сервиса при недоступном pipe.
//!
//! Протокол по stdio — newline-delimited JSON-RPC 2.0 (без Content-Length,
//! как предписывает MCP spec для stdio-транспорта): каждое сообщение — одна
//! строка. На pipe уходит тот же line-framing. Автостарта нет только с
//! `--no-spawn` (или если спавн не удался): pipe недоступен → на initialize
//! lib вернёт `Exit { code: 2, .. }` — отвечаем JSON-RPC ошибкой и
//! завершаемся с кодом 2.

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    canvas_mcp::run_stdio(&args)
}
