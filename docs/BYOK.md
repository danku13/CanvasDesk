# BYOK — Bring Your Own Knowledge (формат для CanvasDesk)

BYOK — это расширение формата `.canvas` (или отдельный файл `.byok`) для описания входного текста и желаемой структуры выхода, совместимое с JSON Canvas 1.0. Формат используется для генерации Mindmap или структурированных заметок через MCP-сервер или встроенный генератор.

## 1. Формат файла `.byok`

```json
{
  "format": "byok/1.0",
  "source": {
    "type": "text | file_path",
    "value": "Проект Альфа: цели, риски, сроки...",
    "encoding": "utf-8"
  },
  "mode": "mindmap | structured_notes | outline | summary",
  "target": {
    "type": "nodes | edges | full_canvas",
    "nodes": [
      {
        "id": "n_1",
        "label": "Проект Альфа",
        "type": "group | file | text | widget",
        "parent": null,
        "x": 0,
        "y": 0,
        "content": "Описание проекта...",
        "tags": ["brainstorm", "m4"]
      },
      {
        "id": "n_2",
        "label": "Цели",
        "type": "text",
        "parent": "n_1",
        "content": "Запуск до Q3",
        "tags": ["goal"]
      }
    ],
    "edges": [
      { "id": "e1", "fromNode": "n_2", "fromSide": "right", "toNode": "n_3", "toSide": "left", "label": "блокирует" }
    ]
  },
  "params": {
    "language": "ru | en",
    "max_depth": 3,
    "detail_level": "brief | full",
    "focus_dim_alpha": 0.5,
    "focus_hide_others": false
  }
}
```

## 2. Режимы генерации

| Режим | Описание | Выход |
|---|---|---|
| `mindmap` | Дерево нод с родительскими ссылками (`parent`) и связями (`edges`) | Ноды типа `group` (корень) + `text` (дети) + `edges` |
| `structured_notes` | Плоский список заметок с тегами | Ноды типа `text`, без `edges`, с полем `tags` |
| `outline` | Иерархия заголовков и подзаголовков | `group` для уровней, `text` для пунктов |
| `summary` | Краткое резюме текста в одну или несколько связанных нод | 1–3 ноды `text` с кратким содержанием |

## 3. Интеграция с MCP (Model Context Protocol)

MCP-посредник (stdio ↔ named pipe `\\.\pipe\canvasdesk` приложения) слушает
команды в формате JSON-RPC и возвращает `.canvas`-совместимый JSON.

### Подключение AI-клиента (один exe, FR-008)

Один бинарник покрывает весь стек: `canvasdesk.exe` — GUI-сервис,
`canvasdesk.exe mcp` — MCP-посредник. Конфиг MCP-клиента (Claude Desktop и
т.п.): `command` = путь к `canvasdesk.exe`, `args` = `["mcp"]`. Если сервис
не запущен, посредник поднимет его сам (автостарт; `--no-spawn` отключает) —
весь стек стартует одной командой. Отдельный `canvasdesk-mcp.exe`
сохраняется для совместимости старых конфигов.

### Команды агента (Hermes Agent → MCP)

- `/canvasdesk.generate` — принимает `source` (текст или путь) и `mode`; возвращает `target_nodes` + `edges`.
- `/canvasdesk.insert` — вставляет результат в текущий открытый `.canvas` с сохранением неизвестных полей (round-trip).
- `/canvasdesk.byok_read` — читает `.byok`-файл и возвращает JSON.

### Пример запроса (JSON-RPC 2.0)

```json
{
  "jsonrpc": "2.0",
  "method": "/canvasdesk.generate",
  "params": {
    "mode": "mindmap",
    "source": { "type": "file_path", "value": "docs/project_notes.md" },
    "params": { "language": "ru", "max_depth": 3 }
  },
  "id": 1
}
```

### Пример ответа

```json
{
  "jsonrpc": "2.0",
  "result": {
    "format": "canvas/1.0",
    "nodes": [ ... ],
    "edges": [ ... ]
  },
  "id": 1
}
```

## 4. Безопасность и ограничения

- MCP-сервер работает только локально (`localhost` или `stdio`); сетевых вызовов в хост-приложение нет (в соответствии с `SPEC.md` §7.6 и `AGENTS.md` п. 9).
- Формат `.byok` не содержит исполняемого кода; только JSON с описанием структуры.
- Виджеты, использующие BYOK (`assets/widgets/byok-generator/`), работают в sandbox WebView2, без `network`-пермишна, и вызывают MCP-сервер через `postMessage` (bridge `canvasdesk` → `byokGenerate`).
- Результат генерации — чистый `.canvas`-JSON, совместимый с `jsoncanvas.org` и Obsidian.

## 5. Связь с задачами

- **T23** — динамическая подсветка связей; `params.focus_dim_alpha` и `focus_hide_others` используются для визуальной настройки режима фокуса при мозговом штурме.
- **T24** — BYOK-формат и MCP-сервер.
- **T25** (опционально) — SDK-виджет для BYOK.

## 6. Пример `.byok`

Пример файла для режима `mindmap` из текста:

```json
{
  "format": "byok/1.0",
  "source": { "type": "text", "value": "Проект Альфа: запуск продукта в Q3. Риски: конкуренция, бюджет. Цели: MVP, тестирование." },
  "mode": "mindmap",
  "target": { "type": "full_canvas" },
  "params": { "language": "ru", "max_depth": 2, "detail_level": "brief" }
}
```

После генерации через `canvas-mcp` или виджет `byok-generator` результат вставляется в текущий `.canvas` с сохранением всех неизвестных полей (совместимость с `SPEC.md` §5.1 и `AGENTS.md` п. 3).
