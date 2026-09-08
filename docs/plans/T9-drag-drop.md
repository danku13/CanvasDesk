# План T9 — Drag-drop из Explorer

Статус: план, к реализации в отдельной сессии (= один коммит).
Источники: `docs/TASKS.md` T9, `docs/SPEC.md` §7.3, AGENTS.md (правила unsafe/COM).
Базовый коммит плана: 3f12ba9 (после рефакторинга `canvas_app::ui`).

## 1. Цель и критерии приёмки

IDropTarget на окне canvas-app: приём `CF_HDROP` (файлы и папки, включая
множественный выбор), текстового URL, подсветка зоны дропа при DragOver.

Критерии (TASKS.md):
- перетаскивание из Explorer работает, включая множественный выбор;
- позиция сетки предсказуема: шаг = размер карточки + 24 px, перенос
  строки после 5 карточек;
- drop папки → обход глубины 1, скрытые/системные игнорируются;
- drop URL → нода-заметка с текстом ссылки;
- во время DragOver — визуальная подсветка зоны дропа.

## 2. Что уже есть (инвентарь)

| Что | Где | Значение для T9 |
|---|---|---|
| `AppEvent` + `EventLoopProxy` | `canvas-app/src/main.rs` (`AppEvent::ThumbsReady`) | паттерн «worker будит event loop»; добавляем `AppEvent::Drag(DragEvent)` |
| `FrameOverlay { instances, screen_instances, ... }` | `canvas-render/src/renderer.rs` | world-space квады для подсветки зоны дропа (как оверлей меню) |
| `Node::file(id, file, x, y, w, h)` / `Node::text` | `canvas-core/src/model.rs` | конструкторы нод для дропа |
| `canvas_app::ui` (чистые helpers + тесты) | `canvas-app/src/lib.rs` | сюда кладём раскладку/парсинг — тестируется без окна |
| `SceneState` (spatial, mark_dirty, autosave) | `canvas-app/src/main.rs` | вставка нод: `push` + `spatial.insert(index, node)` |
| `order_thumbnails()` | `canvas-app/src/main.rs` | тамбнейлы новых нод подтянутся на следующем кадре сами |
| `next_note_id` | `canvas-app/src/lib.rs` | обобщаем до `next_free_id(canvas, prefix)` для `file-N` |
| windows crate 0.62 (фичи Com/Shell уже вкл.) | `Cargo.toml` | добавить `Win32_System_Ole`, `Win32_System_Memory`, `Win32_System_SystemServices` |

## 3. Ключевое проектное решение: свой IDropTarget vs winit

winit 0.30 на Windows **сам** регистрирует drop-обработчик при создании
окна (`RegisterDragDrop`, assert S_OK) и отдаёт события `DroppedFile` /
`HoveredFile`. Этого хватило бы для файлов, но:

- URL-текст и `FileGroupDescriptor` (SPEC §7.3) winit не принимает;
- нужен контроль эффектов курсора (DROPEFFECT_COPY/NONE) и точка дропа
  в DragEnter (для превью сетки до отпускания кнопки);
- TASKS.md прямо требует `IDropTarget`.

**Решение:** отключаем winit-обработчик и ставим свой.

Проверено по исходникам winit 0.30.13 (`platform_impl/windows/`):

1. `Window::default_attributes().with_drag_and_drop(false)`
   (`platform::windows::WindowAttributesExtWindows`) — winit не вызывает
   `OleInitialize`/`RegisterDragDrop` вовсе → конфликтов нет;
2. в `WM_DESTROY` winit вызывает `RevokeDragDrop(hwnd)` **без проверки
   результата** (event_loop.rs:1262) — отзовёт наш обработчик без паники;
   наш COM-объект отпускаем сами в `Drop`.

`OleInitialize` обязателен на потоке event loop (STA) перед
`RegisterDragDrop` — вызываем в `install()`; `S_FALSE` (уже был) — норм.

## 4. Архитектура

```
crates/canvas-shell/src/dragdrop.rs      (cfg(windows), весь unsafe здесь)
  DropTargetHandler  — COM IDropTarget (impl windows::core::implement)
  pub enum DragItem { Path(PathBuf), Text(String) }
  pub enum DragEvent {
      Enter { items: Vec<DragItem>, client_pt: (f32, f32) },  // физ. px
      Over  { client_pt: (f32, f32) },
      Leave,
      Drop  { items: Vec<DragItem>, client_pt: (f32, f32) },
  }
  pub struct DropWatcher { ... }        // владеет COM-объектом, Drop → RevokeDragDrop
  pub fn install(hwnd: HWND, sender: Arc<dyn Fn(DragEvent) + Send + Sync>) -> windows::core::Result<DropWatcher>

crates/canvas-app/src/lib.rs (ui)       (чистое, кроссплатформенное, тестируемое)
  pub fn next_free_id(canvas, prefix)                       // file-N/note-N
  pub fn parse_hdrop_bytes(bytes: &[u16]) -> Vec<PathBuf>   // CF_HDROP: UTF-16, DOUBLE null
  pub fn expand_drop_paths(paths: &[PathBuf]) -> Vec<PathBuf>  // папки → глубина 1, без скрытых
  pub fn drop_grid(origin: Vec2, count: usize) -> Vec<Vec2>   // сетка 5 в ряд
  pub fn drop_text_kind(text: &str) -> DropTextKind           // Url | Plain

crates/canvas-app/src/main.rs           (интеграция)
  AppEvent::Drag(DragEvent)
  App.drop_preview: Option<DropPreview { origin: Vec2, rows: usize, cols: usize }>
  install — в resumed() после создания окна (HWND через WindowExtWindows)
```

Конвертация координат: shell передаёт `client_pt` в физических px
(ScreenToClient уже сделан в DragEnter/Over/Drop), app делит на
`scale_factor` → логические → `camera.screen_to_world` → world.
`client_pt` приходит с каждым событием, т.к. DragOver без координат
не даёт позицию превью.

## 5. Пошаговый план (TDD: тест до реализации)

Каждый шаг — зелёные `cargo test` перед переходом к следующему.

**Шаг 1 — `ui::drop_grid` + тесты.** Сетка от origin: колонок 5, шаг
`CARD_W + 24`, `CARD_H + 24` (CARD_W/H = 320/220 — как seed-карточки;
константы в ui). Тесты: 1 файл → 1 позиция == origin; 5 → одна строка;
6 → вторая строка с origin.x; 11 → третья строка. Позиции детерминированы
— «позиция сетки предсказуема» из критерия.

**Шаг 2 — `ui::expand_drop_paths` + тесты (tempdir).** Папка → её
children (глубина 1: подпапка не разворачивается, сама становится нодой).
Скрытые/системные: Windows — `FILE_ATTRIBUTE_HIDDEN|SYSTEM` через
`std::os::windows::fs::MetadataExt::file_attributes()` (cfg-ветка);
Unix — имя с ведущей точкой (тесты гоняются на CI windows + локально).
Симлинки не следуем (is_symlink → пропустить, чтобы не зациклиться).

**Шаг 3 — `ui::parse_hdrop_bytes` + тесты.** Формат CF_HDROP: массив
UTF-16 строк, каждая завершена `\0`, список завершён вторым `\0`
(DOUBLE_NULL_TERMINATED). Парсим сами — без DragQueryFileW — это чистая
функция от байтов, тестируется на любом ОС синтетическим буфером:
`"a.txt\0b.jpg\0\0"` в UTF-16 → 2 пути; `\0` → пусто; чётность байтов не
наша забота (байты уже &[u16]). Windows-интеграционный тест (cfg windows):
`GlobalAlloc` → записать → сравнить с `DragQueryFileW` по количеству/путям.

**Шаг 4 — `ui::next_free_id` + `ui::drop_text_kind` + тесты.**
`next_free_id` обобщает `next_note_id` (тот удалить, вызов в
`create_note_at` заменить; тесты `note_id_first_free` переносятся).
`drop_text_kind`: `http://`/`https://` в начале (после trim) → Url,
иначе Plain. Критерий «Drop текстового URL → нода-заметка с текстом».

**Шаг 5 — `canvas-shell::dragdrop` (cfg windows).** COM-объект
`IDropTarget` через `windows::core::implement`:

- `DragEnter(IDataObject, keys, pt, effect)` — снять CF_HDROP
  (`GetData` + `GlobalLock`/`GlobalSize` → `parse_hdrop_bytes`); нет
  HDROP → попробовать `CF_UNICODETEXT` → `DragItem::Text`. Кэшировать
  items до Drop (Explorer отдаёт DATA в Enter, в Drop они те же);
  `*effect = DROPEFFECT_COPY` при валидных, `DROPEFFECT_NONE` иначе.
- `DragOver` — обновить pt, `*effect` как в Enter.
- `DragLeave`, `Drop` — событие наружу; в Drop — повторно прочитать
  IDataObject (не полагаться на кэш).
- `install()`: `OleInitialize` (S_FALSE ок; OLE_E_WRONGCOMPOBJ —
  anyhow-ошибка наружу) → `RegisterDragDrop(hwnd, handler)`. Держим
  handler в `DropWatcher`; `Drop` → `RevokeDragDrop` + отпустить COM.
- Весь unsafe — здесь, каждый блок с SAFETY-комментарием (AGENTS §unsafe).
- Ошибка `install` → `tracing::warn!` + приложение живёт без drag-drop
  (graceful degradation, RECIPES R14).

**Шаг 6 — интеграция в main.rs.** В `resumed()`:
`with_drag_and_drop(false)` в атрибутах окна; после создания окна —
`install(hwnd, sender)` где sender шлёт `AppEvent::Drag` через
`EventLoopProxy` (копия паттерна ThumbService). Обработка:

- `Enter/Over` → `drop_preview = Some(...)` (origin из client_pt,
  кол-во = items после expand), `request_redraw`;
- `Leave` → `drop_preview = None`;
- `Drop` → создать ноды: paths (после expand) → `Node::file(next_free_id
  ("file"), path_string, pos...)` по `drop_grid`; `Text` c Url →
  `Node::text(next_free_id("note"), text, origin)`; `Plain`-текст —
  тоже заметка (бонус, дёшево). Вставка: `push` + `spatial.insert`
  (как `create_note_at`), выделение последней группы,
  `scene.mark_dirty()` (автосейв сработает сам), `request_redraw`
  (тамбнейлы закажет `order_thumbnails` в кадре).

**Шаг 7 — подсветка зоны дропа.** `drop_preview` → в
`RedrawRequested` собрать квады-«призраки» сетки (полупрозрачные
`CardInstance` в `FrameOverlay.instances`, world-space) + рамку.
Дёшево: 1 квад на файл, не больше ~50 (обрезать кол-во превью).

**Шаг 8 — FileGroupDescriptor (после критериев, опционально).**
SPEC §7.3 упоминает, критерий T9 — нет. Реализовать вторым шагом, если
юзер драгит из Outlook: `FileGroupDescriptorW` + `FileContents`
(IStream). Не блокирует приёмку.

**Шаг 9 — финал.** `cargo fmt --check`, `cargo clippy --workspace --
-D warnings`, `cargo test --workspace`, ручной чек-лист ниже, коммит
`feat(shell,app): T9 — drag-drop из Explorer (IDropTarget, CF_HDROP)`.

## 6. Сводка тестов

| Тест | Модуль | Что покрывает |
|---|---|---|
| `drop_grid_positions` | ui | 1/5/6/11 позиций, предсказуемость |
| `expand_drop_*` (tempdir) | ui | папка→depth1, скрытые/системные skip, симлинки |
| `parse_hdrop_*` | ui | DOUBLE-NULL парсинг, пустой, одна строка |
| `next_free_id_*` | ui | file-N/note-N первый свободный |
| `drop_text_kind_*` | ui | http/https/plain/мусор |
| `hdrop_matches_dragqueryfile` | shell (cfg windows) | парсер == DragQueryFileW |
| интеграция bin | main.rs | drop → ноды в модели (можно только через parse_hdrop+drop_grid — COM вручную не гоняем) |

Ручная приёмка (Windows, dev-машина): 5 файлов из Explorer → ряд из 5;
6-й файл → вторая строка; папка с 10 фото (1 скрытое) → 9 нод, тамбнейлы
подтягиваются; ссылка из браузера → заметка с URL; во время drag — рамка
у курсора; отпускание вне окна/ESC → подсветка снята; DragOver поверх
канваса не мешает панорамированию.

## 7. Риски и фолбэки

- **COM-квартир:** RegisterDragDrop требует STA на потоке event loop;
  winit с `drag_and_drop=false)` не OLE-инициализирует поток — наш
  `OleInitialize` обязателен. `RPC_E_CHANGED_MODE` → warn + drag-drop
  выключен, приложение не падает.
- **Panic в winit при втором окне:** `assert_eq!(RegisterDragDrop, S_OK)`
  в winit сработает, если создадим второе окно с `drag_and_drop=true`
  после нашего `RegisterDragDrop` — держим все окна с `false`.
- **Относительные пути:** Explorer отдаёт абсолютные — храним как есть;
  `resolve_file_path` уже абсолютизирует при заказе тамбнейлов. Вопрос
  «хранить относительный, если файл внутри каталога канваса» —
  конвенция JSON Canvas, решаем в T10 (см. план T10 §7).
- **CF_HDROP vs paths в UTF-16 >260 символов:** Explorer (long paths) —
  парсер не ограничен MAX_PATH, работаем по буферу целиком.
- **Потоки:** IDataObject читаем в методах IDropTarget — это поток
  event loop (STA), чтение HGLOBAL копией байтов быстро; тяжёлой
  работы (IStream FileContents) — выносить в worker (не нужно для
  CF_HDROP).
