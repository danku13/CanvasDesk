# FR-035: Чистота stdout MCP-потока — изоляция stdio автоспавна, ориентация спавна и stderr-логи GUI

**Статус:** реализовано (v1) · **Дата:** 2026-09-18 · **ADR:** 0010 · **Связанные:** ADR-0009 (FR-034), ADR-0004, FR-008

## Проблема

После FR-034 hermes регистрирует `canvasdesk` и видит инструменты, но
вызовы падают: клиент парсит stdout моста как newline-delimited JSON-RPC
и получает не-JSON строки:

```
Invalid JSON: expected value at line 1 column 1
input_value='\x1b[2m2026-09-18T11:16:…canvasdesk\\widgets"'
```

| # | Дефект | Наблюдение |
|---|--------|-----------|
| 1 | Автоспавн наследует stdio | `Command::new(exe).spawn()` без `Stdio` — ребёнок пишет в stdout моста |
| 2 | tracing GUI пишет в stdout + ANSI | `tracing_subscriber::fmt()` по умолчанию; логи старта (версия, реестр виджетов `…\canvasdesk\widgets`, wgpu) попадают в JSON-RPC-канал |
| 3 | Рекурсивный спавн автономного моста | `canvasdesk-mcp.exe` спавнит `current_exe` — сам себя; двойники конкурируют за stdin и спавнят следующих |

## Требование (решение ADR-0010)

1. Ребёнок автоспавна — с `stdin/stdout/stderr = Stdio::null()`
   (`spawn_service_command`); контракт проверяется юнит-тестом.
2. Ориентация спавна (`autosprawn_target`): единый `canvasdesk` → сам
   себя (GUI-режим); автономный `canvasdesk-mcp` → сосед `canvasdesk.exe`
   в том же каталоге; соседа нет → offline-режим (ADR-0009), без спавна.
3. Логи GUI — `with_writer(io::stderr)`, ANSI — по
   `io::stderr().is_terminal()`.
4. stdout моста — только JSON-RPC (фиксация в SPEC §13); pipe-протокол и
   26 инструментов — без изменений.

## Критерии приёмки

- Юнит-тесты `canvas-mcp`: `spawn_service_command_isolates_stdio`,
  `autosprawn_target_prefers_sibling_gui_for_standalone_bridge` — зелёные.
- Прогон реальной сессии (probe-скрипт): все строки stdout моста —
  валидный JSON; initialize/batch/tools/call — как в FR-034.
- Регресс: `cargo test --workspace` зелёный; тесты FR-034 не изменены.

## Реализация

- `crates/canvas-mcp/src/lib.rs`: `spawn_service_command`,
  `autosprawn_target` + тесты; `connect_app` переведён на них.
- `crates/canvas-app/src/main.rs`: `tracing_subscriber::fmt()` →
  `.with_writer(std::io::stderr).with_ansi(std::io::IsTerminal::is_terminal(&std::io::stderr()))`;
  уточнён комментарий перехвата подкоманды `mcp`.
- Документы: ADR-0010, `docs/SPEC.md` §13, `docs/ACCEPTANCE.md` §24,
  `docs/change-requests/index-cr-fr.md`.
