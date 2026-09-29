# Penpot MCP — клиент и сидер дизайн-системы CanvasDesk

**Дата:** 27.09.2026
**Автор:** агент-сессия (Super Z)
**Тип:** инструкция + спецификация интеграции
**Связанные документы:** `design/rules/01-colors.md`, `design/rules/02-typography.md`, `design/tokens/colors.json`, `design/tokens/dimensions.json`, `docs/dev-researches/inline-ai-autocomplete-hypothesis.md`

## 1. Назначение

Скрипты `scripts/penpot_mcp.py` и `scripts/penpot_seed_design_system.py` — это интеграция CanvasDesk с облачным Penpot MCP-сервером. Они позволяют:

- из Python-кода или CLI вызывать инструменты Penpot MCP (`execute_code`, `tools/list`, `high_level_overview`, `penpot_api_info`, `export_shape`);
- заливать актуальную дизайн-систему CanvasDesk (цвета, типографика, spacing, радиусы, карта анатомии) в любой файл Penpot одной командой;
- в будущем — расширять сценарии: чтение эталонных схем из Penpot, двухсторонняя синхронизация токенов `design/tokens/*.json` ↔ Penpot, экспорт компонентов в код.

## 2. Безопасность токена — КРИТИЧНО

**Токен Penpot MCP не хранится в репозитории.** Это сознательное решение владельца: токен передаётся в каждом запуске через переменные окружения.

### 2.1 Как передавать токен

| Переменная | Что содержит | Приоритет |
|---|---|---|
| `PENPOT_MCP_URL` | Полный URL с `userToken`: `https://design.penpot.app/mcp/stream?userToken=...` | 1 (используется, если задан) |
| `PENPOT_MCP_TOKEN` | Только токен (`eyJhbGciOi...`) | 2 |
| `PENPOT_MCP_HOST` | Переопределение хоста (по умолчанию `https://design.penpot.app`) | — |

Если ни `PENPOT_MCP_URL`, ни `PENPOT_MCP_TOKEN` не заданы — клиент выбрасывает `PenpotMCPConfigError` и завершается с кодом 2.

### 2.2 Защита от утечки в репозиторий

`.gitignore` явно отбрасывает:

```
.env
.env.*
*.local
*.local.*
scripts/penpot_mcp_local*.py
scripts/penpot_token*.txt
**/penpot_token*
```

Любой файл, попадающий в эти паттерны — гарантированно вне репозитория. Скрипты `penpot_mcp.py` и `penpot_seed_design_system.py` **сами по себе не содержат токена** — в коде нет ни одной hardcoded строки, git-blame чист.

### 2.3 Если токен утёк

Сразу: **Penpot → Your account → Integrations → MCP Server → Regenerate MCP key**. Старый токен перестаёт работать; новый — отдельной сессией.

## 3. Установка и быстрый старт

Требований к зависимостям нет — используется только стандартная библиотека Python 3.10+ (`urllib`, `json`, `argparse`).

### 3.1 Предусловия со стороны Penpot

1. Зарегистрироваться на https://design.penpot.app (или на self-hosted инстансе — меняется `PENPOT_MCP_HOST`).
2. **Your account → Integrations → MCP Server → Status: Enabled**.
3. Создать MCP key (если ещё нет). **Сохранить токен** — он показывается один раз.
4. Открыть любой файл Penpot.
5. **File → MCP Server → Connect** — подключить плагин к этому файлу.
6. Не закрывать/не усыплять вкладку Penpot во время MCP-сессии (см. [Penpot docs: Keep the Penpot tab active](https://help.penpot.app/mcp/#keep-the-penpot-tab-active)).

Без шага 5 `execute_code` и `export_shape` падают с:
> `No Penpot instance connected for user token. Please ensure that Penpot is connected and that the MCP client connection is using the correct token.`

### 3.2 Первый запуск

```bash
# Передавать токен каждый раз (любой из вариантов):
export PENPOT_MCP_URL='https://design.penpot.app/mcp/stream?userToken=eyJ...'

# Проверить handshake:
python scripts/penpot_mcp.py init

# Список инструментов:
python scripts/penpot_mcp.py tools-list

# Залить дизайн-систему:
python scripts/penpot_mcp.py seed-design-system
```

Альтернатива (без export, через inline env — для разовых запусков):

```bash
PENPOT_MCP_TOKEN='eyJ...' python scripts/penpot_mcp.py tools-list
```

Альтернатива через `.env.local` (запрещён в `.gitignore`):

```bash
# .env.local (НЕ КОММИТИТСЯ):
#   PENPOT_MCP_URL=https://design.penpot.app/mcp/stream?userToken=eyJ...
#   (или)
#   PENPOT_MCP_TOKEN=eyJ...

# Загрузить env через любой env-loader (например, через shell):
set -a; source .env.local; set +a
python scripts/penpot_mcp.py seed-design-system
```

## 4. CLI-команды

| Команда | Что делает |
|---|---|
| `init` | Только `initialize` — проверяет handshake и session_id |
| `tools-list` | Список доступных инструментов MCP-сервера |
| `tools-call <name> --args '<json>'` | Произвольный вызов инструмента |
| `code '<js>'` | Исполнить JavaScript в контексте Penpot plugin (через `execute_code`) |
| `code -` | Прочитать JS из stdin |
| `api-info` | Документация по Penpot Plugin API (`penpot_api_info`) |
| `overview` | High-level overview по Penpot (`high_level_overview`) |
| `seed-design-system` | Залить дизайн-систему CanvasDesk (см. раздел 5) |

Флаг `--verbose` выводит отладочные сообщения в stderr.

### 4.1 Примеры

```bash
# Получить имя активного файла и страницы:
python scripts/penpot_mcp.py code 'return { file: penpot.file?.name, page: penpot.page?.name };'

# Получить список всех страниц файла:
python scripts/penpot_mcp.py code 'return penpot.file.pages.map(p => ({ id: p.id, name: p.name }));'

# Экспортировать shape по id в PNG:
python scripts/penpot_mcp.py tools-call export_shape \
  --args '{"shapeId": "abc123...", "format": "png", "scale": 2}'
```

## 5. Сидер дизайн-системы (`seed-design-system`)

Команда `seed-design-system` исполняет в активном файле Penpot JS-код, который:

1. **Создаёт library colors** (29 шт.):
   - accent (`#65A0F7`)
   - edge.default / edge.flow / edge.draft
   - state.broken / state.highlight
   - whatif_fill / whatif_chip / whatif_badge
   - error / hud / pulse_result / explain_leaf
   - severity.warning / .danger / .critical
   - dialog.fill / .border / .btn_primary / .btn_secondary / .btn_border / .text / .text_muted
   - toast.text
   - wheel.dim / .category / .template / .hover / .border

2. **Создаёт library typographies** (5 шт.):
   - Noto Sans Display Medium (body)
   - Noto Sans Display Bold (заголовки)
   - Noto Sans Mono Regular (Numi-строки)
   - Noto Sans Mono Bold (моно-жирный)
   - CanvasDesk Mono Oblique (пролитые значения)

3. **Создаёт страницу «CanvasDesk Design System»** с 4 boards:
   - **Color palette** — все цвета с подписями (rgba/hex)
   - **Typography scale** — title/body/edge_label/result/zone_label/hud/badge
   - **Spacing & radius** — s/sm/md/lg/xl и chip/card/panel/pill
   - **Card anatomy** — header (34px) + body (400px ширина с правки 2 от 27.09.2026) + result strip

### 5.1 Источник данных

Сидер хранит ЗЕРКАЛО токенов из `design/tokens/{colors,dimensions}.json` (CR-012 паритет). При обновлении токенов в JSON — обновить константы `COLORS`, `TYPOGRAPHIES`, `TYPE_SCALE`, `SPACING`, `RADII`, `CARD_ANATOMY` в `scripts/penpot_seed_design_system.py`.

### 5.2 Idempotent ли сидер?

Частично: при повторном запуске на том же файле — создаются дубли library colors/typographies с теми же именами (Penpot допускает дубликаты имён). Boards — каждый раз новые. Полностью idempotent-сидер = будущая задача (нужна стратегия diff-upsert через plugin API).

Чтобы избежать дублирования при повторных запусках — открывайте новый файл Penpot или удаляйте страницу «CanvasDesk Design System» перед перезапуском.

## 6. Python API

```python
import sys
sys.path.insert(0, 'scripts')
from penpot_mcp import PenpotMCP
from penpot_seed_design_system import seed_design_system

# Токен из env
client = PenpotMCP.from_env(verbose=True)
client.initialize()
client._notify_initialized()  # явный шаг инициализации сессии

# Чтение
page_info = client.execute_code('return { name: penpot.page?.name, id: penpot.page?.id };')
print(page_info)

# Запись
result = seed_design_system(client)
print(result)
```

## 7. Ограничения remote MCP

Документация Penpot MCP явно фиксирует, что remote-режим:

- **не даёт `import_image`** — нет доступа к локальной файловой системе;
- **`export_shape` ограничен** — нет прямого экспорта в локальный путь, только base64/URL в ответе.

Для полного набора нужен local MCP через `npx @penpot/mcp-server` (слушает `http://localhost:4401/mcp`). Это отдельный сценарий — не реализован в этом скрипте.

## 8. Troubleshooting

| Симптом | Причина | Решение |
|---|---|---|
| `403 Cloudflare: browser_signature_banned` | urllib без `User-Agent` | Уже исправлено в `penpot_mcp.py` (заголовки `BROWSER_HEADERS`) |
| `initialize failed: status=400` | Не передан session_id в последующих запросах | Внутренняя логика — `_post()` автоматически подставляет `Mcp-Session-Id` после `initialize()` |
| `No Penpot instance connected for user token` | Penpot-плагин не подключён к файлу | File → MCP Server → Connect в Penpot; держать вкладку активной |
| `Tool execution failed: ...` в JS | Ошибка в коде пользователя | Прочитать сообщение; проверить Penpot API через `python scripts/penpot_mcp.py api-info` |
| `PENPOT_MCP_URL или PENPOT_MCP_TOKEN обязаны быть установлены` | Не задан токен в env | См. раздел 2.1 |

## 9. Связанные артефакты

- `scripts/penpot_mcp.py` — MCP-клиент (Python lib + CLI)
- `scripts/penpot_seed_design_system.py` — сидер дизайн-системы
- `design/tokens/colors.json` — источник правды для цветов
- `design/tokens/dimensions.json` — источник правды для spacing/radius/метрик
- `design/rules/01-colors.md`, `design/rules/02-typography.md` — правила применения
- https://help.penpot.app/mcp/ — официальная документация Penpot MCP

## 10. Дорожная карта

- [ ] Полностью idempotent-сидер (diff-upsert library colors/typographies по имени)
- [ ] Двухсторонняя синхронизация: изменения в Penpot → `design/tokens/*.json`
- [ ] Импорт эталонных схем CanvasDesk из Penpot в репозиторий
- [ ] Local MCP через `npx` (расширенные возможности — `import_image`, локальный экспорт)
- [ ] CI-проверка: ни одного hardcoded токена в репозитории (grep + pre-commit hook)
