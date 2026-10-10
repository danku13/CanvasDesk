# FR-106: Мультиканвас C3 — менеджер канвасов (оверлей)

- **Статус:** реализовано (код+тесты), ожидает приёмки владельца
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** danku13 (решения №1–54), агент (реализация)
- **Источник:** high-level issue #14 + sub-issue #7 (волна C3, после C1 #5 + C2 #6); план v2.1 §3.2
- **Связанные задачи:** FR-103 (контракты C0), FR-104 (конвейер C1), FR-105 (хранилище/миграция C2); C4 #8 (чип — вход в менеджер), C5 #9 (desktop/приёмка)
- **Создан:** 2026-10-10
- **Обновлён:** 2026-10-10 (реализация)

---

## Описание (What)

Оверлей «Менеджер канвасов» — центральная поверхность мультиканваса: список
канвасов workspace-хранилища с поиском, сортировкой, группами, созданием
(4 источника), инлайн-ренеймом, мягким удалением с undo, строкой хранилища
и пустым состоянием. Построен на конвейере C1 (WebRequest/AppEvent + OpfsStore)
и хранилище C2 (FsAccessStore/миграция/баннер) поверх контрактов C0.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| canvas-app `canvas_manager_ui` (новый) | `CanvasManagerState { open, filter, selected, scroll, entries, editing }`, чистые layout/rows/hit-зоны (`row_at`/`name_at`), `StorageRowMode` (№51a), скролл/клавиатура — 15 тестов | FR-106 |
| canvas-app `app`/`handler`/`input`/`overlays`/`ui_registry` | AppEvent-блок `// FR-106` (CanvasManagerOpen/CanvasSavedAsCopy/CanvasDeleteUndo…), открытие/закрытие, клавиатура (Esc/Enter/F2/↑↓/PgUp-PgDn/ввод в ренейм), двойной клик (строка — открыть №49; имя — ренейм №9), действия, тосты | FR-106 |
| canvas-web `web_requests` | операции менеджера: открытие канваса из workspace, дубликат (полный `.canvas`, №27a), импорт файла (пикер + №26b авто-суффикс), undo удаления (`.bak`→файл), перенос ключа камеры при ренейме активного (№12/№30b), удаление из recent (№15a) | FR-106 |
| canvas-web `drop_files` | DOM-drop с коллизией имени — авто-суффикс + тост «создана копия» (№26b; было: тихая перезапись create:true) | FR-106 |
| canvas-web `toolbar` | кнопка «Недавние» → `CanvasManagerOpen` (вход в менеджер до чипа C4; №37b — «Недавние» уходит) | FR-106 |
| canvas-app `i18n` | +8 ключей (delete_toast/export/name_taken/name_invalid/op_failed/badge_browser/folder/disk) RU/EN; остальные ~39 `canvas.*` — из C0, потреблены | FR-040 |
| Пользовательское поведение | новый оверлей: управление канвасами без правки URL; менеджер поверх сцены, не блокирует автосейв | — |

## Анализ (Root Cause)

1. До C3 переключение канвасов на web — только `?canvas=` в URL (ручная правка);
   «Недавние»reopen только дисковых файлов; списка/создания/ренейма/удаления в UI нет.
2. Модель C (канвас = сущность) требует поверхность управления (план §1): менеджер —
   тот единственный вход, где сходятся все решения №6–№27/№37b/№40b/№43a/№51a.

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Состояние | `canvas_manager_ui.rs` | `CanvasManagerState` (по паттерну scheme_gallery): open/filter/selected/scroll_top/entries/`editing: Option<(индекс, буфер)>`; `set_entries` + clamp; `begin_rename/edit_insert/edit_backspace/cancel_edit/take_edit` (инлайн №9) |
| Список | там же | `rows(entries, filter, sort)` — поиск по display_name (регистронезависимо), SortMode toggle, группы `group_entries` №43a (корень плоско + `repos/<имя>` с заголовками, фильтр действует и в группах) |
| Навигация | там же | `move_selection` (пропускает заголовки), `scroll_to_reveal`, `wheel_scroll_top`; клавиатура: Esc (в ренейме — отмена ренейма, иначе закрыть), Enter (в ренейме — применить; в списке — открыть), F2 — ренейм, ↑/↓/PgUp/PgDn, ввод — в буфер |
| Hit-зоны | там же | `manager_layout` (чистая), `row_at` (строка), `name_at` (имя — зона ренейма дабл-кликом); draw==hit |
| Строка хранилища | там же + web | `StorageRowMode::{Browser, Folder, Unsupported}`: «Хранилище: браузерное [Переехать на диск…]» (№51a всегда видна); папка — без кнопки; Firefox/Safari — без кнопки (не обещать диск). Кнопка → `MigrateShowDialog` (C2) |
| Действия | `web_requests.rs` | открытие (чтение workspace → OpenScene + set_active + recent; активный — просто закрыть); дубликат №27a (copy_name с i18n-суффиксом, полный `.canvas`, сразу активен); импорт файла (пикер → санитизация → №26b коллизия → копия в workspace); undo удаления №15a (`.bak`→файл, без переключения сцены); удаление — store.delete + remove_recent |
| После удаления активного | app + web | №22c/№40b: менеджер остаётся открытым, под ним создаётся и активируется новый пустой «Canvas N» (auto_name №39c) |
| Ренейм активного | web | перенос ключа камеры `canvasdesk.camera.<old>→<new>` в localStorage (№12/№30b, формат C0) |
| Пустое состояние | там же | CTA «Создать канвас» + вторичное «Открыть файл с диска…» (№23a) |
| Шаблон | app | «Из шаблона…» (№38a): создаёт пустой канвас → `manager_after_open` → галерея-пикер (`set_pending_scheme`) поверх нового |
| Drop-коллизия | `drop_files.rs` | №26b: существующее имя → `collision_suffix` + тост «создана копия» (CanvasSavedAsCopy, ПОСЛЕ OpenScene) |
| default.canvas | rows | обычная строка, без спец-логики (№24a) |

## Точки входа (Entry Points)

Кнопка «Недавние» тулбара (web) → `CanvasManagerOpen` (вход временный до чипа C4 №21c);
строка хранилища → диалог миграции C2; «Открыть файл с диска…» → существующий
пикер fs_access. index-cr-fr — этим коммитом; interface-objects/user-docs — C5.

## Проверка (Verification)

- `cargo test --workspace` — 3001 passed / 0 failed (22 новых: canvas_manager_ui×15,
  app-поведение×5, web_requests×2);
- `cargo clippy --workspace -- -D warnings`; `cargo fmt --check` — зелёные;
- `scripts/wasm_gate.sh --check` — OK (вкл. аудит времени: `manager_last_click`
  — `canvas_core::time::Instant`);
- i18n-инвариант FR-040 (RU/EN полнота +8 ключей);
- WASM L2 не выполнялся (окружение: нет wasm-bindgen CLI, 2 ядра) — ручные
  сценарии владельцем (Chromium, ?log=debug): (1) «Недавние» → оверлей:
  список/поиск/сортировка/группы; (2) Esc/Enter/F2/двойной клик; (3) создать
  пустой/шаблон/дубликат/импорт; (4) ренейм с коллизией → запрет+тост; (5)
  удалить активный → менеджер открыт, под ним «Canvas N», тост «Отменить»
  восстанавливает; (6) строка хранилища: браузерное/папка/Firefox; (7) drop
  с коллизией → «имя (1)» + тост; (8) тёмная/светлая, RU/EN.

## История изменений (Changelog)

- 2026-10-10 — агент (subagent MC-C3, сессия прерывалась): реализация оверлея
  (canvas_manager_ui + интеграция app/web, 22 теста). Координатор (Super Z main):
  фикс wasm-блока (std::time::Instant → canvas_core::time::Instant; владение
  в drop_files/web_requests — 3 ошибки E0382/E0521/E0308, видны только под
  wasm32-таргетом), повторные гейты, FR-106, работа с worklog.

## Источники истины (References)

- План: `download/multicanvas-design-plan.md` v2.1 (решения №6–№9/№15a/№22c/
  №23a/№24a/№25b/№26b/№27a/№37b/№38a/№40b/№43a/№49/№51a).
- FR-103/FR-104/FR-105 — контракты/конвейер/хранилище; `scheme_gallery_ui.rs` —
  паттерн оверлея; `storage_ui.rs` (C2) — эталон строки хранилища.
- GitHub: danku13/CanvasDesk issues #7 (эта волна), #14 (high-level).
