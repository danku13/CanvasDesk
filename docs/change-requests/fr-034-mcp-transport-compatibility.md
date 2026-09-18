# FR-034: Перепроектирование MCP-транспорта — протокольная совместимость для внешних агентов (hermes)

**Статус:** реализовано (v1) · **Дата:** 2026-09-18 · **ADR:** 0009 · **Связанные:** ADR-0004, FR-005/FR-008/FR-032/FR-033

## Проблема

Внешний MCP-клиент (агент hermes, та же машина) не может нормально
подключиться к `canvasdesk-mcp`. Диагностика (прогон эмуляции клиентской
сессии против моста, 2026-09-18) выявила пять дефектов транспортного
конверта:

| # | Дефект | Наблюдение |
|---|--------|-----------|
| 1 | Даунгрейд версии протокола | клиент `2025-06-18` → ответ `2024-11-05`; строгие SDK рвут соединение |
| 2 | Batch-запросы (JSON-RPC массив) | `-32600 «ожидался JSON-RPC объект»` |
| 3 | Двойная упаковка `tools/call` | в `content[0].text` попадает весь JSON-RPC-конверт приложения вместо результата |
| 4 | `exit(2)` при недоступном приложении | клиент видит краш сервера; сессия не устанавливается |
| 5 | Нет reconnect | приложение, поднявшееся позже моста, не подхватывается |

Дополнительно: `resources/list`/`prompts/list`/`resources/templates/list`/
`logging/setLevel`/`notifications/cancelled` отвечают `-32601` — часть
хостов зондирует их безотносительно capabilities.

## Требование

Реализовать решение ADR-0009 в крейте `canvas-mcp`:

1. `SUPPORTED_PROTOCOLS` += `2025-06-18` (эхо клиентской версии).
2. Поддержка batch-массивов: поэлементная обработка, сборный ответ,
   пустой массив → `-32600`, все-уведомления → тишина.
3. Разворот конверта приложения в мосту: `content[0].text` — чистый JSON
   результата, `structuredContent` — объектный результат; error-конверт →
   `isError`.
4. `initialize` успешен всегда (без бэкенда тоже); `Exit` убран из автомата;
   `tools/call` без приложения → `isError` «CanvasDesk не запущен…».
5. Reconnect перед `tools/call` (короткая попытка 500 мс, без автоспавна).
6. Толерантные заглушки read-only методов.
7. Pipe-протокол мост ↔ приложение и `mcp_dispatch` (26 инструментов) —
   без изменений.

## Критерии приёмки

- Все 6 пунктов покрыты юнит-тестами `crates/canvas-mcp/src/lib.rs`
  (эхо версии, batch ×3, разворот result/error-конверта, offline-handshake,
  reconnect-fail, заглушки).
- Регресс: `cargo test --workspace` зелёный; pipe round-trip тесты
  `canvas-shell` не затронуты.
- Конфиги из `docs/BYOK.md` (`canvasdesk.exe mcp`) работают без изменений.

## Реализация

- `crates/canvas-mcp/src/lib.rs`: `handle_input` (batch-разбор),
  `unwrap_app_payload`, `initialize_result` без условия о транспорте,
  reconnect в `run_stdio`, заглушки read-only методов, `HandleOutcome`
  без `Exit`; `build_call_result(id, result: &Value)` с structuredContent.
- `crates/canvas-mcp/src/main.rs`: документация нового поведения.
- Документы: ADR-0009, `docs/SPEC.md` §13, `docs/BYOK.md` §3,
  `docs/ACCEPTANCE.md` §23, `docs/change-requests/index-cr-fr.md`.
