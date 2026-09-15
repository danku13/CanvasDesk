# FR-006: Отмена действий (Ctrl+Z, стек ≥50) и возврат (Ctrl+Y)

- **Статус:** выполнено
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** агент (реализация)
- **Источник:** сообщение пользователя (сессия 2026-09-14): «ctrl + Z - отмена действия (не менее 50 последних действий)»
- **Связанные задачи:** TASKS.md T7/T8 (правки нод/связей), CR-001 (мультивыделение), CR-002 (перепривязка), FR-003 (копипаст), FR-005 (MCP-мутации); SPEC.md §9 (автосейв)
- **Создан:** 2026-09-14
- **Обновлён:** 2026-09-16 (аудит реализации)
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

В приложении нет отмены действий: любая ошибка пользователя (случайное удаление
нод, неудачное перемещение, правка текста, перепривязка связи) необратима —
восстановить можно только из бэкапа автосейва. Требуется `Ctrl+Z` — отмена
последних действий с глубиной истории не менее 50, плюс возврат отменённого
(`Ctrl+Y` / `Ctrl+Shift+Z`) — без redo односторонний undo опасен сам по себе.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| Все мутации канваса | Перед мутацией — снапшот в undo-стек | `crates/canvas-app/src/main.rs` (App, SceneState) |
| Ввод | Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z (лат.+кир. я/н) | `docs/interface-objects/node.md` §4 |
| MCP | Мутирующие инструменты также отменяемы | `docs/BYOK.md`, FR-005 |
| Оверлей хоткеев | +3 строки (Ctrl+Z/Y/X) | `docs/change-requests/fr-004-hotkeys-overlay.md` |
| Автосейв | Undo помечает сцену грязной → сейв | `docs/SPEC.md` §9 |

## Анализ (Root Cause)

- `SceneState` (`main.rs:188`) не хранит историю: единственная точка возврата —
  `save_with_backup` (§9), т.е. файл на диске.
- Мутации разрозненны: `delete_selected`, `insert_nodes`, `create_note_at`,
  `insert_group`, drag/resize в `on_cursor_moved`, `finish_editing`,
  edge-drop/rebind в `on_left_button` (Released), меню ноды/связи,
  `create_text_file_node`, drop T9, 11 мутирующих веток `mcp_dispatch`.
- Общий знаменатель всех мутаций — модель `Canvas` (nodes + edges,
  `Clone + PartialEq`, `model.rs:286`): snapshot-undo не требует диффов.
- Рендер-кэши ключованы индексами нод (`invalidate_node_caches` — паттерн
  `delete_selected` `main.rs:1158-1180`): после отката индексы недостоверны,
  кэши сбрасываются полностью, spatial index перестраивается.

## Требуемые изменения (Changes)

1. `SceneState`: `undo_stack: VecDeque<Canvas>` (лимит `UNDO_LIMIT = 50`,
   старейшее вытесняется), `redo_stack: Vec<Canvas>`; `push_undo(snapshot)`
   сбрасывает redo (новое действие обнуляет ветку возврата).
2. `App::restore_canvas(canvas)`: замена модели + `SpatialIndex::build` +
   `invalidate_node_caches` + `thumbs_failed.clear()` + сброс интеракций
   (selected/selected_nodes/dragging/resizing/editing/menu/hovered/edge_drag/
   select_rect — индексы устарели) + `mark_dirty` + `sync_watch_dirs`.
3. Немедленный push перед мутацией: `create_note_at`, `insert_group`,
   `insert_nodes` (покрывает paste/duplicate), `create_text_file_node`,
   `delete_selected` (все ветки), drop T9, меню ноды (цвет) и связи
   (стиль/толщина/цвет), edge-drop (новая связь) и rebind (CR-002),
   мутационные ветки `mcp_dispatch` (после валидации, до применения).
4. Отложенный снапшот `pending_undo: Option<Canvas>` для «растянутых» действий:
   drag и resize (снапшот на Pressed, push на Released при фактическом
   изменении позиций/размеров), редактирование текста (снапшот на
   `begin_editing`, push в `finish_editing(commit && changed)`, отмена — дроп).
5. `App::undo_action` / `App::redo_action`: текущий Canvas → в противоположный
   стек, восстановление через `restore_canvas`.
6. Хоткеи: `Ctrl+Z` (кир. «я») — undo; `Ctrl+Y` (кир. «н») и `Ctrl+Shift+Z` —
   redo; срабатывают вне редактора/поиска (клавиатура у них приоритетна).
7. ОГРАНИЧЕНИЕ v1 (задокументировано): события вотчера (`on_file_events`) —
   внешние, undo их не покрывает; undo не восстанавливает буфер обмена
   FR-003 и панораму/зум (камера не входит в историю).

## Точки входа (Entry Points)

- `docs/interface-objects/node.md` — правки/удаления/перемещения отменяемы.
- `docs/interface-objects/edge.md` — создание/удаление/перепривязка отменяемы.
- `docs/change-requests/fr-004-hotkeys-overlay.md` — список хоткеев +3.
- `docs/ACCEPTANCE.md` — чек-лист ручной приёмки FR-006.
- `docs/BYOK.md` — MCP-мутации отменяемы через UI (не через MCP-протокол).

## Проверка (Verification)

- `Ctrl+Z` после: удаления набора, перемещения drag, resize, правки текста,
  смены цвета, создания заметки, вставки/дублирования, новой связи,
  перепривязки — состояние канваса возвращается к «до» (автосейв следует).
- 51-е действие вытесняет 1-е (глубина ровно 50).
- `Ctrl+Y` возвращает отменённое; новое действие после undo обнуляет redo.
- `Ctrl+Z` при пустом стеке — no-op (не падает).
- Undo во время drag/редактирования — интеракции сбрасываются консистентно.
- Unit-тесты: лимит стека; undo после `node_delete` (MCP) восстанавливает ноду
  и связи; redo-цепочка; push после валидационной ошибки MCP не создаёт
  пустой шаг.

## История изменений (Changelog)
- `2026-09-16` — агент (аудит реализации всех CR/FR, main `984ca6b`): аудит: реализация подтверждена — `UNDO_LIMIT = 50` (`main.rs:74`), `undo_stack`/`redo_stack` (`main.rs:260-263`), `push_undo` с вытеснением и обнулением redo, `restore_canvas` (spatial, кэши, интеракции, recompute_flow), push-точки во всех мутациях (вставка/удаление/drag/resize/текст/цвет/рёбра/MCP/палитра), хоткеи `Ctrl+Z/Я`, `Ctrl+Y/Н`, `Ctrl+Shift+Z`; тесты `undo_stack_limit_is_fifty`, `mcp_delete_undo_redo_roundtrip`, `new_action_clears_redo_branch`, `mcp_validation_errors_leave_no_steps`, `expr_undo_redo_restores_formula`. Приёмка §14 FR-006.1–6 — за владельцем. Статус `выполнено`.


- `2026-09-14` — агент: реализация (коммит feat(core,app): FR-006) —
  SceneState::undo_stack (VecDeque, UNDO_LIMIT=50, вытеснение) +
  redo_stack (новое действие обнуляет); App::restore_canvas (spatial
  rebuild + invalidate_node_caches + сброс интеракций); push-точки: все
  UI-мутации (создание/вставка/удаление/drop/edge-drag/rebind/меню) + 11
  MCP-веток (после валидации; no-op не шаг — сравнение PartialEq);
  pending_undo для drag/resize/редактирования (шаг только при фактическом
  изменении; Space-прерывание тоже закрывает шаг); хоткеи Ctrl+Z/Я,
  Ctrl+Y/Н, Ctrl+Shift+Z; HOTKEYS +2. Тесты: лимит 50,
  delete→undo→redo roundtrip (каскад связей), redo-сброс, no-op шаги.
  Статус `в работе` (до ручной приёмки — ACCEPTANCE §14 FR-006.1–6).
- `2026-09-14` — агент: документ создан по запросу пользователя, статус `в работе` (анализ и план готовы).

## Источники истины (References)

- `crates/canvas-app/src/main.rs:188` — `SceneState`; `1158-1180` — паттерн
  сброса кэшей; `3205+` — `on_key`; `2955-3200` — `mcp_dispatch`.
- `crates/canvas-core/src/model.rs:286` — `Canvas` (`Clone + PartialEq`).
- `docs/SPEC.md` §9 — автосейв (undo помечает сцену грязной).
- `docs/change-requests/fr-003-node-clipboard.md` — вставка/дублирование.
