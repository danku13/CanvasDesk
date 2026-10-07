# FR-100: Полный набор шорткатов редактора ноды (word-delete, undo/redo, Super-паритет, web-слой)

- **Статус:** выявлено
- **Тип:** FR
- **Приоритет:** критично
- **Владелец:** danku13 (симптом, решение), агент (анализ)
- **Источник:** UR-001-02 (`docs/user-reporting/ur-001-handtest-canvas.md`), ручной тест `handtest.canvas`, сессия 2026-10-08; платформа тестирования — web (wasm)
- **Связанные задачи:** FR-004 (оверлей хоткеёв), FR-095 (input-шим web), FR-039 (поле поиска — эталон word-delete), `user-docs/hotkeys.md`, SPEC.md §8
- **Создан:** 2026-10-08
- **Обновлён:** 2026-10-08

---

## Решения владельца (Q&A)

**Вопрос Q0 (сессия 2026-10-08): «На какой платформе тестировал?»** → **Web (wasm)** — определяет состав фикса (web-слой обязателен).

**Вопрос Q10: «Какой объём доработки шорткатов редактора?»**

| Вариант | Состав |
|---|---|
| Базовый паритет | Ctrl+Backspace/Delete (слово), Cmd-паритет, не глотать незнакомые Ctrl-комбинации |
| Базовый + undo | + редакторский undo/redo (Ctrl+Z/Y) |
| **Полный набор ← выбрано** | базовый + undo/redo + PageUp/PageDown + Tab-индент + умный Home (текст → начало строки) |

---

## Описание (What)

В редакторе ноды не работают «базовые» шорткаты: удаление слова Ctrl+Backspace (не реализовано нигде), а на web (платформа владельца) и Ctrl+A — из-за дефектов web-слоя. Требуется довести редактор до стандартного набора текстового редактора: word-delete, undo/redo, постраничная навигация, Tab-индент, умный Home, Cmd-паритет — и починить доставку клавиш на web. Ctrl+A на нативе уже реализован (`edit.rs:372`) и не пересматривается.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| Редактор заметки (`EditingSession`) | Новые `KeyCommand`: DeleteWordBackward/Forward, Undo/Redo, PageUp/Down, Tab/Shift+Tab, SmartHome | `docs/interface-objects/node.md` §редактирование |
| Клавиатурный роутер | Super/Cmd-паритет; незнакомые Ctrl-комбинации не глотаются (передаются глобальной лестнице) | SPEC.md §8 |
| Web-слой | `preventDefault` для chord'ов в шиме FR-095; возврат фокуса канвасу после DOM-интеракций | `docs/WASM-TESTING.md` |
| Документация | `user-docs/hotkeys.md` + `HOTKEYS` (F1-оверлей) пополняются | `user-docs/hotkeys.html` |

## Анализ (Root Cause)

Полный разбор — UR-001-02. Слои:

1. **Ctrl+Backspace/Delete отсутствует**: `map_key` игнорирует Ctrl для Backspace (`crates/canvas-render/src/edit.rs:346-347`); word-delete нет в `KeyCommand`/`apply`. При этом эталон уже в кодовой базе: панель поиска (`crates/canvas-app/src/search_ui.rs:79-84, 135-169`) и поле настроек (`app/input.rs:765-773`).
2. **Web (главная причина симптома Ctrl+A)**: (а) winit 0.30.13 web-backend шлёт `KeyboardInput` **до** `ModifiersChanged` и стирает модификаторы при blur (`winit/…/web/event_loop/window_target.rs:148-166, 86-99`) → первая Ctrl-комбинация после смены фокуса печатает символ; (б) форвардер шима FR-095 не зовёт `preventDefault` (`crates/canvas-web/src/ime.rs:292-337`) — браузер параллельно делает select-all; (в) после клика по DOM-элементам (тулбар, тур) фокус не возвращается канвасу — клавиатура мертва (`canvas-web/src/toolbar.rs:18-51`; winit слушает keydown только на канвасе).
3. **Super/Cmd не пробрасывается** (`app/input.rs:461-462` только `control_key()`) — Cmd+A на macOS печатает «a».
4. **«Глушитель»**: незнакомые клавиши глотаются (`app/input.rs:502-513`, `return true`) — Ctrl+Z/Y no-op, undo в редакторе отсутствует.
5. Состав `map_key` (edit.rs:327-394): нет Ctrl+Backspace/Delete, Ctrl+Z/Y, PageUp/PageDown, Tab; есть Ctrl+A/C/X/V, Ctrl+стрелки, Home/End, Shift-выделение.

## Требуемые изменения (Changes)

1. `crates/canvas-render/src/edit.rs` — новые `KeyCommand` (DeleteWordBackward/DeleteWordForward, Undo/Redo, PageUp/Down, Tab/Shift+Tab, SmartHome) + реализация в `apply` (word-delete: селект `Motion::LeftWord/RightWord` с extend + delete_selection, либо перенос эвристики `search_ui.rs:135-169` в plain-координаты; undo/redo — стек снимков plain+spans в сессии, один шаг на слово/вставку; SmartHome — двойное нажатие Home).
2. `crates/canvas-app/src/app/input.rs:461` + сигнатура `map_key` (edit.rs:327) — проброс `super_key()`; `input.rs:502-513` — незнакомые Ctrl/Super-комбинации передаются глобальной лестнице (не `return true`).
3. `crates/canvas-web/src/ime.rs:292-337` — `event.prevent_default()` для chord'ов (ctrl/meta/alt) в форвардере шима (подход шима Ctrl+P, `index.html:797-824`); возврат фокуса канвасу после кликов по toolbar/оверлеям.
4. Компенсация порядка событий winit-web (кэш модификаторов от `KeyboardInput` или апгрейд winit) — отдельно, при недоступности апгрейда: хранить последний известный набор модификаторов без сброса на blur.
5. Документация: `user-docs/hotkeys.md`, HOTKEYS (F1), `docs/interface-objects/node.md`.

## Точки входа (Entry Points)

- `user-docs/hotkeys.md`, `crates/canvas-app/src/lib.rs` (HOTKEYS), `docs/interface-objects/node.md` §редактирование.
- `docs/WASM-TESTING.md` §сценарии — добавить клавиатурный смоук web (Ctrl+A, Ctrl+Backspace).
- `docs/user-reporting/ur-001-handtest-canvas.md` — UR-001-02.

## Проверка (Verification)

- Web (wasm): Ctrl+A выделяет весь текст заметки (после клика по ноде, без предварительных кликов по тулбару); Ctrl+Backspace удаляет слово слева; Cmd+A на macOS-браузере — то же.
- Натив: Ctrl+Backspace/Delete — слово; Ctrl+Z/Ctrl+Y — undo/redo правок текста (в пределах сессии редактирования); PageUp/PageDown — страница текста; Tab — индент пробелами (Shift+Tab — убирает); Home — начало текста строки, повторное — начало самой строки.
- Ctrl-комбинации, неизвестные редактору, не глотаются (глобальные хоткеи работают при активном редакторе там, где это задумано).
- Гейты: `cargo test -p canvas-render edit`, app-тесты клавиатуры; web-смоук (`scripts/web_smoke.py`) расширен Control+a/Control+Backspace.

## История изменений (Changelog)

- `2026-10-08` — агент: документ создан по UR-001-02; платформа владельца — web; объём — полный набор; статус `выявлено`.

## Источники истины (References)

- `docs/user-reporting/ur-001-handtest-canvas.md` (UR-001-02).
- `crates/canvas-render/src/edit.rs:306-323,327-394,765-811,1582-1585`; `crates/canvas-app/src/app/input.rs:461-462,502-513,765-773`; `crates/canvas-app/src/search_ui.rs:79-84,135-169`; `crates/canvas-web/src/ime.rs:83-85,292-337`; `crates/canvas-web/src/toolbar.rs:18-51`.
- winit 0.30.13 `src/platform_impl/web/event_loop/window_target.rs:86-99,148-166`; `web_sys/canvas.rs:299-304,475`.
- `docs/change-requests/fr-095-virtual-keyboard-web.md`; `user-docs/interface.md:19`; `user-docs/hotkeys.md:71-78`.
