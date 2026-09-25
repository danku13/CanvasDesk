# Каталог инструментов CanvasDesk MCP — 41 инструмент

Полный реестр инструментов MCP-сервера CanvasDesk по группам. Канонический
источник — ответ `tools/list` живого сервера (реестр `TOOLS` в
`crates/canvas-mcp/src/lib.rs`); при расхождении текста и схемы права схема.
Пакет скиллов синхронен этому каталогу (контракт-тест `skills_sync` в
`canvas-mcp`, см. [UPDATE-PROTOCOL](../../UPDATE-PROTOCOL.md)).

Обозначения: `?` — необязательный параметр; перечисление через `|`;
типы — string / number / boolean / integer / null. Вызовы показаны в
канонической форме `инструмент` {параметры}.

## Разведка и навигация (10)

| Инструмент | Сигнатура | Назначение |
|---|---|---|
| `canvas_info` {} | — | Сводка по канвасу: число нод и связей, путь к файлу .canvas, версия приложения |
| `nodes_list` {text?} | text: boolean | Список нод: id, тип, координаты, размеры, подпись, файл; поле text — только при text=true |
| `node_get` {id} | id: string | Одна нода по id со всеми полями (включая text) |
| `nodes_search` {query} | query: string | Поиск нод: подстрока без учёта регистра по text/title/label/file (title — явный заголовок canvasdesk.title, FR-072) |
| `edges_list` {} | — | Все связи: {id, from, to, kind, fromLine?, fromOutput?, toParam?, fromSide, toSide} — восстановление топологии графа |
| `edge_get` {id} | id: string | Одна связь по id — схема как у элементов edges_list |
| `template_list` {} | — | Реестр шаблонов: id, name, version, category, expr, params, outputs (именованные выходы для fromOutput) |
| `schemes_list` {} | — | Галерея встроенных схем: {id, name, name_en, category, version, description, nodes, edges} — RU-первично |
| `viewport_get` {} | — | Центр viewport в world-координатах и зум |
| `viewport_set` {x, y, zoom?} | x, y: number; zoom: number | Установить центр viewport и опционально зум |

## Ноды: создание и правка (8)

| Инструмент | Сигнатура | Назначение |
|---|---|---|
| `node_create_note` {x, y, text?, title?, width?, height?} | x, y: number | Создать ноду-заметку (Numi-лист); возвращает id. Дефолт 260×120. title (FR-072) — явный заголовок карточки; без него заголовок — первая строка текста (legacy) |
| `node_create_file` {path, x, y, width?, height?} | path: string | Создать файловую ноду по пути (файл на диске НЕ создаётся) |
| `node_update_text` {id, text} | id, text: string | Заменить текст ноды-заметки целиком |
| `node_edit` {id, text?, title?, label?, color?, expr?, x?, y?, width?, height?} | id: string | Править ТОЛЬКО переданные поля; label/color/expr/title = null — сброс (title = null — возврат к фолбэку «первая строка», FR-072); expr — Numi-формула; возвращает обновлённую ноду |
| `node_move` {id, x, y} | — | Переместить ноду в world-координаты |
| `node_resize` {id, width, height, fit?} | width/height: number > 0; fit: boolean (опц., default false) | Изменить размеры ноды. fit:true — подогнать высоту под видимый контент (фон не сжимается меньше контента). Возвращает {id, width, height, fit_applied} |
| `node_delete` {id} | — | Удалить ноду (связи — каскадно; дети группы НЕ удаляются) |
| `node_set_color` {id, color} | color: "1".."6" \| null | Пресет цвета ноды или null для сброса |

## Связи и поток (4)

| Инструмент | Сигнатура | Назначение |
|---|---|---|
| `edge_create` {from, to, kind?, fromLine?, fromOutput?, toParam?, fromSide?, toSide?} | from, to: id нод | Создать связь; kind "value" включает поток значений (дефолт "control" — визуальная); fromLine — построчный исток; fromOutput — именованный выход; toParam — проливание в параметр приёмника |
| `edge_delete` {id} | id: string | Удалить связь по id |
| `flow_set_kind` {id, kind} | kind: value \| control | Тип потока связи; тогл в value, замыкающий цикл, — ошибка; пересчёт сразу |
| `edge_ports` {id, pin} | pin: auto \| from \| to \| both | Стороны подключения: "auto" — снять закрепления, остальное — закрепить текущие эффективные стороны (WYSIWYG) |

## Батч-композиция (1)

| Инструмент | Сигнатура | Назначение |
|---|---|---|
| `graph_apply` {operations} | массив 1..256 объектов с полем op | Атомарный батч «всё или ничего»: node_create_note, node_create_file, template_instantiate, edge_create, edge_delete, param_set, node_move (+ ref-адресация внутри батча); один undo-шаг; лимит ≤ 128 новых нод |

## Шаблоны и схемы (2)

| Инструмент | Сигнатура | Назначение |
|---|---|---|
| `template_instantiate` {id, x, y, params?} | id: id шаблона | Создать text-ноду из шаблона; params — {имя: число \| {num, unit}}; вне min/max — ошибка |
| `schemes_apply` {id, x?, y?} | id: id схемы | Вставить схему галереи в текущий канвас: ремап id без коллизий, один undo-шаг, ответ {applied, name, nodes, edges, bbox, flow} |

## Вычисление и проверка (7)

| Инструмент | Сигнатура | Назначение |
|---|---|---|
| `flow_recalc` {} | — | Карта значений потока АКТИВНОГО what-if состояния: {node_id: {value, unit, outputs, lines, warnings?, spilled?, autoRows?}} |
| `lineage` {node_id, line?} | line: integer \| null | Дерево происхождения цифры (паритет с окном проверки цепочки): {root, nodes[]}, kind calc\|leaf\|cycle\|unmapped\|unlinked\|truncated, via-рёбра |
| `explain_number` {node_id, line?} | line: integer \| null | Объяснение цифры ТЕКСТОМ (F-9, PRD-0007): линейная развёртка дерева с адресами и значениями; ответ {render: "text", text, root, nodes, truncated} — мост отдаёт text как content text |
| `flow_cycle_check` {} | — | Проверка DAG-инварианта value-рёбер: [] — циклов нет, иначе список id участников |
| `graph_validate` {} | — | Валидация модели: {valid, issues: [{severity, code, node_id, edge_id, message}]} — коды E-CYCLE, E-OVERLOAD, E-UNIT, E-PORT-UNKNOWN, E-DOUBLE-INPUT, W-AMBIGUOUS-SRC, W-UNUSED-SLOT |
| `analyze_bottlenecks` {} | — | Узкие места и риск очередей АКТИВНОГО состояния: {nodes: [{id, severity, utilization?, queue_length?, wait_sec?, badge}], thresholds} |
| `monte_carlo_run` {runs, params, mode?, seed?, quantiles?} | runs: integer 1..10⁶; params: {"node:param": {dist, mean/sd \| lambda}}; mode: qmc \| mc (дефолт qmc); seed: u64 (дефолт 0); quantiles: [0..1] | FR-066: MC/QMC-прогон N ≥ 10⁴ с распределёнными параметрами → квантили P50/P90/P99 итогов/строк/именованных выходов + analysis (узкие места на хвостовом P90); dist: normal/lognormal {mean, sd} (натуральное пространство), exp/poisson {lambda}; параметр — строка «param = …» листа; тот же seed → те же квантили (native-only: wasm без qmc, §5.8) |

## What-if сценарии (9)

| Инструмент | Сигнатура | Назначение |
|---|---|---|
| `whatif_set_override` {node_id, line, expr} | line: integer ≥ 0 | Построчная подмена активного сценария; режим/сценарий поднимаются автоматически |
| `whatif_set_param` {node_id, param, value} | value: string | Sugar для шаблонных нод: находит строку «param = …» сам; параметра нет — ошибка |
| `whatif_scenario_list` {} | — | {active, whatif_active, scenarios: [{name, overrides, stale}]} — stale = протухшие подмены |
| `whatif_scenario_create` {name?} | — | Создать именованный сценарий (лимит 3), сразу активен; freeze в .canvas, один undo-шаг |
| `whatif_scenario_delete` {name} | — | Удалить сценарий (мутация .canvas, undo-шаг) |
| `whatif_scenario_activate` {name} | name: «База» \| имя | Переключить активный сценарий; runtime-only, файл не меняется |
| `whatif_deltas` {} | — | Дельты активного сценария против базы: {node:line \| node:value: {node, line?, base, whatif, delta}} |
| `whatif_apply` {} | — | Записать подмены в канвас (один undo-шаг), сценарий удаляется |
| `whatif_reset` {} | — | Сбросить подмены активного сценария (runtime); режим остаётся активным |
