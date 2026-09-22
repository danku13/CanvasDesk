# Changelog скиллов CanvasDesk MCP

Формат: версия пакета, дата, изменения. Правила ведения —
[UPDATE-PROTOCOL.md](UPDATE-PROTOCOL.md).

## v1 — 2026-09-22

Начальный пакет: 4 скилла, синхронизированы с реестром из 39
инструментов (commit 17b9134, main).

- `canvasdesk-mcp` — подключение и разведка: транспорт stdio
  (offline/reconnect/протоколы), инварианты (value-связи,
  MCP-видимость = UI, каскад Р-1, undo-дисциплина), инструменты
  чтения, карта скиллов, подводные камни.
- `canvasdesk-model-build` — рецепт сборки (разведка → ноды →
  value-связи с адресацией портов → атомарный батч graph_apply →
  визуальная доводка), таблица операций и кодов ошибок, эталонный
  пример Instagram MVP (ADR-0005: 12 нод, 10 рёбер, оракулы ±1 %).
- `canvasdesk-model-verify` — flow_recalc (структура ответа: value/
  outputs/lines/spilled/autoRows), lineage (дерево, kind, via),
  flow_cycle_check, graph_validate (таблица кодов E-*/W-*),
  analyze_bottlenecks (severity/пороги), порядок верификации.
- `canvasdesk-whatif` — 9 инструментов сценариев, дельты,
  дисциплина apply/reset, типичный сеанс, ограничения.
- `references/tools.md` — полный каталог 39 инструментов по группам.
- Контракт-тест `skills_sync` (canvas-mcp): полнота каталога,
  покрытие скиллами, счётчик README, валидность call-позиций.
