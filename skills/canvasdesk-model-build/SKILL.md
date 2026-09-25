---
name: canvasdesk-model-build
description: Сборка исполняемой математической модели на канвасе CanvasDesk через MCP — ноды, value-связи с адресацией портов (fromOutput/fromLine/toParam), атомарный батч graph_apply, вставка схем. Используйте, когда нужно создать или изменить модель, связать ноды, задать параметры. Triggers: build model, create nodes, edges, value flow, graph_apply, param_set, template_instantiate, schemes_apply.
version: 2
---

# Сборка модели на CanvasDesk

Рецепт собирает исполняемую модель: числа вводятся руками только в
стартовых допущениях, всё остальное вычисляется и проливается по
value-связям. Требуется подключённый MCP-сервер CanvasDesk (см. скилл
`canvasdesk-mcp`).

**Главное правило:** если число можно вычислить из другой ноды — его
нельзя писать руками. Оно должно пролиться по value-связи. Правка одного
стартового допущения тогда пересчитает всю модель.

## Шаг 1. Разведка: шаблоны и схемы

- `template_list` {} — библиотека расчётных ролей (CDN, балансировщик,
  БД, очередь…). По каждому: params (default, min/max, unit) и outputs
  (именованные выходы). Выпишите имена портов — они понадобятся для
  рёбер.
- `schemes_list` {} — галерея готовых схем. Если задача похожа на
  «бюджет», «unit-экономику», «ёмкость сервиса» — быстрее вставить
  готовую схему `schemes_apply` {id, x?, y?} и править её: id нод
  ремапятся без коллизий, вставка = один undo-шаг, ответ несёт bbox
  (для `viewport_set` {x, y}) и flow с готовыми значениями.
- `canvas_info` {} + `nodes_list` {text: true} + `edges_list` {} — что
  уже есть на канвасе, чтобы не дублировать и не конфликтовать.

## Шаг 2. Ноды

- **Стартовые допущения** — текстовая нода `node_create_note` {x, y,
  text}: строки `имя = значение единица` становятся переменными, строки
  с формулами (`avg_rps = dau × sess × req / 86400`) — вычисляемыми.
  Это единственное место, где числа вводятся руками. Переменные ноды
  становятся её именованными выходами (для fromOutput).
- **Заголовок карточки** (FR-072) — `node_create_note` {…, title} /
  `node_edit` {id, title}: явный заголовок живёт отдельно от текста и
  не меняется при правках тела. Без title заголовок — первая строка
  текста (legacy); title = null возвращает фолбэк.
- **Расчётные роли** — `template_instantiate` {id, x, y, params}:
  id шаблона из `template_list`; params — {имя: число} или
  {имя: {num, unit}}; значение вне min/max — ошибка.
- **Файловая нода** — `node_create_file` {path, x, y}: карточка-ссылка
  на файл (сам файл на диске не создаётся).
- Правки: `node_update_text` {id, text} — заменить текст целиком;
  `node_edit` {id, …} — точечная правка ТОЛЬКО переданных полей
  (text, title, label, color, expr, x, y, width, height; null сбрасывает
  label/color/expr/title; expr — Numi-формула, рендерится под текстом ноды).

**Многострочный текст:** перенос в JSON — настоящий `\n`
(`"text": "dau = 1000000\nsess = 4"`); двухсимвольный вариант
нормализуется толерантно. Проверка: `flow_recalc` возвращает по ноде
столько `lines`, сколько строк задумано.

## Шаг 3. Value-связи с адресацией портов

```
`edge_create` { from, to, kind: "value",
                 fromOutput: "<имя выхода>" | fromLine: <номер строки>,
                 toParam: "<имя параметра приёмника>" }
```

- `kind` обязателен для потока значений: дефолт `control` — визуальная
  связь, значение НЕ переносится.
- Исток (прямой `edge_create` {…}): `fromOutput` — именованный выход
  шаблона ИЛИ переменная Numi-листа текстовой ноды (имена — из
  `template_list` / outputs в `flow_recalc`); `fromLine` — номер строки
  текстовой ноды (0-based, живёт при сдвиге строк). Поля взаимно
  исключительны; многолинейный исток без адресации → `W-AMBIGUOUS-SRC`.
- Приёмник: `toParam` — параметр шаблона; проливание перекрывает
  локальное значение (только при kind "value"). Value-ребро без
  toParam создаёт у приёмника авто-строку «Объект.Поле».
- Валидация имён по снапшотам шаблонов: неизвестный порт →
  `E-PORT-UNKNOWN`; несовместимая единица → `E-UNIT`; второе ребро в
  тот же toParam → `E-DOUBLE-INPUT`; value-цикл → `E-CYCLE`.
- **Замена источника занятого toParam:** пара `edge_delete` {id} +
  `edge_create` {…} — второе ребро в занятый параметр падает
  E-DOUBLE-INPUT, поэтому удаление старого обязательно.
- `flow_set_kind` {id, kind} — переключить тип существующей связи;
  `edge_ports` {id, pin} — закрепить/отпустить стороны (pin: auto |
  from | to | both).

## Шаг 4. Быстрый путь: атомарный батч graph_apply

Всю сборку (ноды + параметры + рёбра) — ОДНИМ вызовом
`graph_apply` {operations}:

```
{"operations": [
  {"op": "node_create_note", "ref": "traffic", "x": 0, "y": 0, "text": "…"},
  {"op": "template_instantiate", "ref": "cdn", "template": "com.canvasdesk.cdn", "params": {"cache_hit": 0.6}, "x": 400, "y": 0},
  {"op": "edge_create", "fromRef": "traffic", "toRef": "cdn", "kind": "value", "fromLine": 6, "toParam": "rps"}
]}
```

Операции (поле op):

| op | Поля | Примечание |
|---|---|---|
| `node_create_note` | ref?, x, y, text?, title?, width?, height? | Numi-лист |
| `node_create_file` | ref?, x, y, path | файл не создаётся |
| `template_instantiate` | ref?, template, params?, x, y | вне min/max — ошибка |
| `edge_create` | fromRef\|from, toRef\|to, kind?, fromLine?, fromOutput?, toParam?, fromSide?, toSide? | порты как у шага 3, НО `fromOutput` в батче валидируется только по выходам шаблонов: текстовый исток адресуйте `fromLine` (переменная Numi-листа в батче — `E-PORT-UNKNOWN`) |
| `edge_delete` | id\|ref | удаление ребра (для замены источника) |
| `param_set` | ref\|id, param, value, unit? | правит ровно одну строку «param = value unit»; параметра нет — ошибка (append НЕ выполняется) |
| `node_move` | ref\|id, x, y | перемещение |

- **ref** адресует ноды, созданные ранее В ЭТОМ ЖЕ батче (forward-ref →
  `E-NOT-FOUND`; дубликат → `E-BAD-OP`).
- **Атомарность:** ошибка любой операции → `{ok: false, op_index,
  code, message}`, канвас байт-в-байт прежний. Успех → один undo-шаг
  на весь батч, полный пересчёт, автосейв.
- **Ответ успеха уже содержит `flow`** в формате `flow_recalc` —
  второй вызов пересчёта не нужен. Сверяйте числа здесь (скилл
  `canvasdesk-model-verify`).
- **Лимиты:** ≤ 256 операций, ≤ 128 новых нод на вызов.
- Коды ошибок операций: `E-BAD-OP`, `E-NOT-FOUND`, `E-PORT-UNKNOWN`,
  `E-CYCLE`, `E-PARAM-UNKNOWN`, `E-RANGE`.

## Шаг 5. Визуальная доводка

- `node_move` {id, x, y} — раскладка по течению слева направо с шагом
  ~450 по x; `node_resize` {id, width, height, fit?} — размер под текст;
  `fit: true` — подогнать высоту под видимый контент (фон ноды никогда
  не сжимается меньше контента). Текстовые ноды: width ≈ 500, height
  140–200 (эвристика; `fit: true` скорректирует height автоматически).
- `node_set_color` {id, color} — пресеты "1".."6" или null.
- `node_delete` {id} — удалить ноду (связи каскадно; дети группы
  остаются).
- `viewport_set` {x, y, zoom?} — показать пользователю результат
  (координаты — из bbox вставки или created-нод ответа батча).

## Эталонный пример: Instagram MVP (ADR-0005)

Полный готовый батч (12 нод, 10 value-рёбер, один вызов) —
[examples/instagram-mvp.json](examples/instagram-mvp.json). Ожидаемые
значения после вставки (допуск ±1 %): avg_rps ≈ 555.6, peak_rps ≈ 1388.9,
origin_rps ≈ 555.6, out_auth ≈ 83.3, out_feed ≈ 333.3, out_media ≈ 138.9,
db_qps ≈ 80, replica_load ≈ 40, consume_rate ≈ 333.3.

Проверка воспроизводимости: `param_set`/`node_edit` на
`dau = 2000000` → один `flow_recalc` → вся цепочка удваивается без
правки связей и формул.

## Чек-лист готовности сборки

1. Все вычислимые числа проливаются по value-связям (хардкодов нет).
2. Ответ `graph_apply`/`flow_recalc` сошёлся с ожиданиями модели.
3. `graph_validate` → `valid: true` (скилл `canvasdesk-model-verify`).
4. Любое стартовое допущение меняется одним вызовом, downstream
   пересчитывается без правки графа.
5. Модель показана пользователю (`viewport_set` по bbox).
