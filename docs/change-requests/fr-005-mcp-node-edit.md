# FR-005: MCP-инструмент редактирования ноды (node_edit)

- **Статус:** в работе
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** агент (реализация)
- **Источник:** сообщение пользователя (сессия 2026-09-13): «нужно добавить экшн редактирования ноды через mcp»
- **Связанные задачи:** MCP-интеграция (canvas-mcp + mcp_pipe, коммит ccf0300); TASKS.md T7; docs/interface-objects/node.md §6
- **Создан:** 2026-09-13
- **Обновлён:** 2026-09-13
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

MCP-инструменты редактирования ноды точечные и разрозненные: `node_update_text`
(только text), `node_move` (x,y), `node_resize` (width,height), `node_set_color`
(цвет). Внешний агент не может отредактировать ноду одним вызовом и не может
изменить `label` (подпись группы) вообще никак. Требуется инструмент
`node_edit` — редактирование произвольного набора полей одним вызовом.
Выявлено пользователем.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `canvas-mcp` | Новый инструмент в `TOOLS` (tools/list отдаёт 16-м) | `crates/canvas-mcp/src/lib.rs:175` |
| `canvas-app` | Ветка `node_edit` в `mcp_dispatch`: обновление только переданных полей | `crates/canvas-app/src/main.rs:2689` |
| `node` | label редактируем и через MCP (ранее — только через UI-редактор) | `docs/interface-objects/node.md` |

## Анализ (Root Cause)

- `TOOLS` (`crates/canvas-mcp/src/lib.rs:175-271`): 15 инструментов, общий
  edit-инструмента нет; поле `label` не покрывает ни один.
- `mcp_dispatch` (`crates/canvas-app/src/main.rs:2689-2896`): мутации полей
  разнесены по веткам `node_update_text`/`node_move`/`node_resize`/
  `node_set_color`; каждая требует отдельного round-trip по pipe.
- Побочные эффекты, которые надо соблюсти в одной ветке: `move_node`
  (spatial, `main.rs` через `scene.move_node`), resize (spatial update),
  color (валидация пресетов `"1".."6"`/null — паттерн `node_set_color`,
  `main.rs:2823-2846`), text (кэш заголовков текстовой системы сам замечает
  изменение — `CacheKey` по контенту, `crates/canvas-render/src/text.rs:696`).

## Требуемые изменения (Changes)

1. `canvas-mcp/src/lib.rs`: `ToolSpec node_edit` — required `id`; properties:
   `text` (STR), `label` (STR или null), `color` (COLOR_PROP), `x`, `y`,
   `width`, `height` (NUM) — все опциональны; описание: «обновляет только
   переданные поля».
2. `main.rs` `mcp_dispatch`: ветка `"node_edit"` — применить переданные поля
   (валидация: color-пресет как в `node_set_color`; width/height > 0;
   координаты — f32 без ограничений), spatial-обновление при геометрии,
   `mark_dirty`; ответ — сводка ноды (`mcp_node_summary`, text включён).
3. `label: null` — сброс подписи (None); отсутствие поля — не трогать.

## Точки входа (Entry Points)

- `docs/BYOK.md` / README (список MCP-инструментов, если фиксируется там) —
  добавить `node_edit`.
- `docs/ACCEPTANCE.md` — раздел ручной приёмки FR-005.
- `docs/change-requests/fr-005-mcp-node-edit.md` (этот файл) — статусы.

## Проверка (Verification)

- `tools/list` содержит `node_edit` с inputSchema (required: id).
- `node_edit {id, text}` — меняет только text (координаты/размер не тронуты).
- `node_edit {id, label: null}` — подпись сброшена; `{id, label: "X"}` — задана.
- `node_edit {id, color: "9"}` — ошибка валидации (isError), сцена не меняется.
- `node_edit {id, width: -5}` — ошибка валидации.
- Геометрия: `node_edit {id, x, y, width, height}` одним вызовом — spatial
  валиден (hit-test находит ноду по новым координатам).
- Несуществующий id — ошибка «нода не найдена».
- Unit-тесты: canvas-mcp (tools_list содержит node_edit), canvas-app
  (выборочные поля, валидация, label null, каскадных эффектов нет).

## История изменений (Changelog)

- `2026-09-14` — ревью владельца: пункты FR-005.1–005.3 (ручная приёмка MCP
  через AI-клиента) отложены как тест-долг — владелец оставил тесты «на
  будущее». Единица фиксации: `docs/ACCEPTANCE.md` §13, пометка «тест-долг».
  Юнит-уровень (tools/list содержит node_edit, ветки dispatch, валидация)
  покрыт автотестами и зелёный. Реализация считается кодово завершённой;
  ручная приёмка MCP-канала — отложена. Статус `в работе` (тест-долг).
- `2026-09-13` — агент: реализация (коммит feat(mcp,app): FR-005) —
  ToolSpec node_edit (16-й инструмент, tools/list тесты обновлены),
  ветка mcp_dispatch: обновление только переданных полей, label/color
  null — сброс, валидация (color-пресет, width/height > 0, label
  строка/null), spatial-обновление при геометрии + 2 теста (поля,
  валидация). Статус `в работе` (до ручной приёмки).
- `2026-09-13` — агент: документ создан по запросу пользователя, статус `в анализе`.

## Источники истины (References)

- `crates/canvas-mcp/src/lib.rs:175-271` — `TOOLS` (15 инструментов).
- `crates/canvas-app/src/main.rs:2689-2896` — `mcp_dispatch`.
- `crates/canvas-app/src/main.rs:2823-2846` — паттерн валидации цвета.
- `crates/canvas-render/src/text.rs:696` — `cache_fresh` (кэш по контенту).
