# FR-008: Один exe — запуск сервис+MCP одной командой (подкоманда `mcp`)

- **Статус:** выполнено
- **Тип:** FR
- **Приоритет:** желательно
- **Владелец:** агент (реализация)
- **Источник:** сообщение пользователя (сессия 2026-09-14): «понять как одновременно запускать сервис и mcp одной командой или в рамках сборки релиза завернуть в один exe файл»
- **Связанные задачи:** MCP-интеграция (canvas-mcp + mcp_pipe, коммит ccf0300); FR-005 (node_edit); README (запуск); BYOK (конфиг MCP-клиентов)
- **Создан:** 2026-09-14
- **Обновлён:** 2026-09-16 (аудит реализации)
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

Полный стек CanvasDesk состоит из двух исполняемых файлов: `canvasdesk`
(GUI-сервис, слушает named pipe `\\.\pipe\canvasdesk`) и `canvasdesk-mcp`
(MCP-посредник stdio↔pipe). Пользователю нужно поднимать обе части; в релизе
фактически два артефакта. Требуется: одна команда/один exe, который покрывает
оба режима, с автостартом сервиса из MCP-режима, если тот ещё не запущен.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `canvasdesk.exe` | Подкоманда `mcp`: stdio-MCP-посредник, тот же бинарник | `crates/canvas-app/src/main.rs`, README |
| `canvasdesk-mcp` | Совместимая обёртка над lib (сохраняется) | `crates/canvas-mcp/src/main.rs` |
| `canvas-mcp` lib | Публичный `run_stdio(args)` + автостарт сервиса | `crates/canvas-mcp/src/lib.rs` |
| MCP-клиенты (BYOK) | command: `canvasdesk.exe`, args: `["mcp"]` | `docs/BYOK.md` |

## Анализ (Root Cause)

- Цикл stdio-MCP живёт в `canvas-mcp/src/main.rs:17-52` — недоступен другим
  бинарям; lib (`canvas_mcp`) содержит только чистые функции протокола.
- `canvas-app` уже зависит от `canvas-mcp` (workspace-зависимость для
  `parse_envelope`/`build_result`/`PIPE_NAME`) — перенос цикла в lib даёт
  режим без новых зависимостей.
- Архитектурно MCP-процесс обязан быть отдельным ОС-процессом: stdio-каналы
  принадлежат MCP-хосту (AI-клиенту), GUI-сервис держит pipe-сервер. «Одна
  команда» корректно решается не потоком в сервисе, а подкомандой того же
  exe: `canvasdesk` — сервис, `canvasdesk mcp` — посредник.
- «Одновременно одной командой»: `canvasdesk mcp` при недоступном pipe сам
  спавнит `canvasdesk` (current_exe, без аргументов) и дожидается pipe —
  стек поднимается одним вызовом. Обратный порядок (сервис спавнит mcp)
  бессмысленен: stdio посредника должен быть подключён к MCP-хосту.
- Тонкость: логи сервиса пишутся в stdout — в MCP-режиме stdout занят
  протоколом. Перехват подкоманды обязан происходить ДО инициализации
  tracing; `run_stdio` молчалив (ошибки — anyhow в stderr).

## Требуемые изменения (Changes)

1. `canvas-mcp/src/lib.rs`: `pub fn run_stdio(args: &[String]) ->
   anyhow::Result<()>` — перенос цикла/транспорта из bin (stdio-framing,
   `HandleOutcome`, `PipeTransport`, reader-поток; windows-gate как в bin).
2. Автостарт: `connect_app()` не дождался pipe И НЕ `--no-spawn` →
   `Command::new(current_exe).spawn()` (GUI-сервис) → повторное ожидание
   pipe (поллинг WaitNamedPipeW до ~10 с) → соединение. Неудача — прежнее
   поведение: `initialize` отвечает JSON-RPC-ошибкой, код выхода 2.
3. `canvas-app/src/main.rs::main`: ранний перехват — первый аргумент `mcp`
   → `canvas_mcp::run_stdio(&argv[1..])` и выход (до tracing и parse_args);
   `--help` упоминает подкоманду; файл с именем `mcp` открывается как
   `./mcp`.
4. `canvas-mcp/src/main.rs`: тонкая обёртка `run_stdio(args)` — бинарник
   `canvasdesk-mcp` сохраняется для совместимости конфигов (флаг
   `--no-spawn` одинаково понимается обоими входами).
5. README/BYOK: конфиг MCP-клиента — `command: <путь>/canvasdesk.exe`,
   `args: ["mcp"]`; раздел «Запуск одной командой».

## Точки входа (Entry Points)

- `README.md` — раздел запуска: подкоманда mcp, одна команда на стек.
- `docs/BYOK.md` — конфиги MCP-клиентов.
- `docs/ACCEPTANCE.md` — чек-лист ручной приёмки FR-008.
- `crates/canvas-mcp/src/main.rs` — комментарий шапки (перенос в lib).

## Проверка (Verification)

- `canvasdesk mcp` при запущенном сервисе: handshake initialize →
  tools/list (инструментов теперь 20 — см. аудит 2026-09-16) → node_edit — без второго бинарника.
- `canvasdesk mcp` при НЕзапущенном сервисе: сервис поднялся сам (окно
  обоев), посредник подключился — «одна команда на весь стек».
- `canvasdesk mcp --no-spawn` без сервиса: initialize → JSON-RPC-ошибка,
  код выхода 2 (прежнее поведение canvasdesk-mcp).
- `canvasdesk` (без аргументов) — сервис как раньше; `canvasdesk --help`
  упоминает `mcp`; stdio MCP-режима не содержит логов (протокол чист).
- Linux: `cargo check`/тесты зелёные (windows-код за cfg-gate); bin
  `canvasdesk-mcp` не сломан (обёртка).

## История изменений (Changelog)
- `2026-09-16` — агент (аудит реализации всех CR/FR, main `984ca6b`): аудит: реализация подтверждена — подкоманда `mcp` до parse_args/tracing (`main.rs:5780-5783` → `canvas_mcp::run_stdio`), автостарт сервиса через `current_exe().spawn()` с ожиданием pipe до 15 с (`canvas-mcp/lib.rs:500-521`), флаг `--no-spawn`, `--help` упоминает `mcp [--no-spawn]`, совместимая обёртка `canvasdesk-mcp` сохранена (`canvas-mcp/main.rs:16-19`); Linux-смоук в доке. Windows-приёмка §14 FR-008.1–5 — за владельцем; уточнение: в FR-008.1 «tools/list (16)» устарело — инструментов теперь 20 (добавлены flow_*/edge_ports, CR-008/FR-014). Статус `выполнено`.


- `2026-09-14` — агент: реализация (коммит feat(mcp,app): FR-008) —
  canvas_mcp::run_stdio(args): stdio-цикл + PipeTransport/read_loop
  перенесены из bin в lib; автостарт (current_exe spawn + ожидание pipe до
  15 с, флаг --no-spawn); canvasdesk-mcp — тонкая совместимая обёртка;
  canvas-app main(): перехват «mcp» до трейсинга (stdout чист для
  протокола), --help дополнен; README/BYOK — конфиг клиентов
  (command=canvasdesk.exe, args=[mcp]). Смоук на Linux: initialize без
  pipe → JSON-RPC -32002 + exit 2 с обоих входов. Статус `в работе` (до
  ручной приёмки на Windows — ACCEPTANCE §14 FR-008.1–5).
- `2026-09-14` — агент: документ создан по запросу пользователя («понять
  как» — анализ дал решение: подкоманда одного exe + автостарт; переносы
  stdio-процесса в GUI-процесс отвергнуты архитектурно), статус `в работе`.

## Источники истины (References)

- `crates/canvas-mcp/src/main.rs:17-52` — stdio-цикл; `:57-86` — connect_app.
- `crates/canvas-shell/src/mcp_pipe.rs` — pipe-сервер сервиса.
- `crates/canvas-app/src/main.rs:4223` — `main()` (точка перехвата).
- `docs/BYOK.md` — подключение AI-клиентов.
