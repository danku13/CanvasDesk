# FR-105: Мультиканвас C2 — FS Access-хранилище и миграция на диск

- **Статус:** реализовано (код+тесты), ожидает приёмки владельца
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** danku13 (решения №41c/№42a/№44b/№45b/№52a/№53b, R-T6), агент (реализация)
- **Источник:** high-level issue #14 «Мультиканвас — менеджер канвасов, FS-first, Web Locks» + sub-issue #6 (волна C2); план `download/multicanvas-design-plan.md` v2.1
- **Связанные задачи:** волны C0 (#4, FR-103 — контракты), C1 (#5, FR-104 — OpfsStore/конвейер событий), C3 (#7 — менеджер-оверлей), C4 (#8), C5 (#9); R-T6 (rename через `handle.move()`); wasm-port §4.1/§4.2 (паттерны OPFS/FS Access)
- **Создан:** 2026-10-10
- **Обновлён:** 2026-10-10 (реализация, код+тесты)

---

## Описание (What)

Волна C2 мультиканваса: рабочее пространство канвасов в granted-папке
через FS Access API (Chromium 86+) и переезд из браузерного OPFS на диск.

Состав (sub-issue #6):

- **`FsAccessStore`** — реализация трейта `WorkspaceStore` (контракт C0,
  сигнатуры НЕ менялись) над granted-папкой: синхронное зеркало
  «имя → ts» (паттерн `MirrorStore` из `opfs.rs`) + очередь платформенных
  операций, сливаемая фоновым `spawn_local`-таском; rename через
  `handle.move()` с фолбэком write+delete и ОБЯЗАТЕЛЬНЫМ переносом
  `.bak`-близнеца (R-T6); мягкое удаление — файл → `<name>.bak` (№15a).
- **Тихий старт №41c:** сохранённый dir-хэндл (IndexedDB, ключ
  `"workspace"`) И `queryPermission({mode:'readwrite'}) == 'granted'` →
  режим папки; иначе — тихо OPFS. `requestPermission` на старте НЕ
  вызывается (только query — жеста нет). В Firefox/Safari режима папки
  нет (`fs_access::available()` — false) — всегда OPFS, строка хранилища
  ничего не обещает (браузерная матрица).
- **Миграция OPFS→папка (№42a/№52a):** диалог с чекбокс-списком канвасов
  OPFS (активный — заблокированная галочка: сцена не может остаться в
  OPFS при переключении режима); план — чистая `migration_plan` из C0;
  исполнитель копирует выбранные в папку, проверяет цель и только потом
  удаляет OPFS-оригиналы (+ их `.bak`-близнецы — «чистый переезд» без
  устаревших дублей); тост `canvas.migrate.done_toast` / при частичном
  сбое — `canvas.migrate.failed_toast` (оригиналы целы).
- **Баннер потери доступа №44b:** полоса сверху-по-центру (не блокирует
  работу) «Переподключить» (requestPermission в жесте клика) /
  «Переключиться в браузерное» (активный канвас сеется в OPFS, режим —
  OPFS). Стиль — `kit::banner` Warning, геометрия — токены/константы кита.
- **Watch внешних изменений (№45b/№53b):** poll `lastModified` на
  focus/visibilitychange (только в режиме папки); изменение АКТИВНОГО
  канваса → тост `canvas.ext.changed_toast` ВСЕГДА (даже без локальных
  правок) с кнопкой «Перезагрузить»; при перезагрузке с несохранёнными
  правками локальная версия сперва уходит в `.bak` («правки — в .bak»).
  Собственные записи обновляют базу сравнения — свои сейвы не считаются
  внешними. Diff снимков — чистая функция `snapshot_changed` (C0, ядро).

`navigator.storage.persist()` — НЕ часть волны (делает C1, FR-104).

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| canvas-web `fs_folder` (новый) | `FsAccessStore` (зеркало+очередь), пикер папки, персист dir-хэндла, старт №41c, миграция, watch; обратные вызовы — через конвейер `WebRequest`/`AppEvent` (унаследован от C1, отдельного моста нет) | FR-105 |
| canvas-web `workspace` | Исполнитель миграции: трейт `MigrationIo` (IO-шов для нативных тестов), `MigrationDriver` (машина состояний: копирование → проверка → удаление), `MigrationReport`/`MigrationStep`, `execute_migration` | FR-105, контракт C0 (FR-103) не тронут |
| canvas-web `web_state` | `ActiveKind::Folder`, `FOLDER_HANDLE`/`FS_STORE` (thread_local, JsValue !Send — в трейт-объекты не попадают), канал `send_event` для фоновых тасков | wasm-port §4 |
| canvas-web `fs_access`/`opfs`/`export`/`app_spawn` | Автосейв режима папки (файл И `.bak` — в папку, R-T6; NotAllowedError → баннер), точка тихого старта в `init_scene` (минимальная, помечена `// FR-105`), экспорт активной версии из папки, `?migrate=1` (временный отладочный вход до C3) | FR-105 |
| canvas-web `index.html` (JS-глю) | `dirHandlePut/dirHandleGet` (персист DIRECTORY-хэндла, ключ `"workspace"` — по образцу `handlePut/handleGet`), `dirList()` → Array<{name, ts}>, `handleMove(handle, newName)` | FR-105 |
| canvas-app `app`/`handler`/`input`/`overlays`/`ui_registry` | AppEvent-блок `// FR-105` (StorageAccessLost/StorageReconnected/MigrateShowDialog/MigrateOpfsList/MigrateDone/MigrateFailed/ExtFileChanged), запросы — через `WebRequest`-конвейер C1 (`pending_web_requests` + дренаж TourAwareApp), баннер/диалог/тост-действие (render+hit+клавиатура+колесо), тост с действием живёт 8 с | FR-105 |
| canvas-app `storage_ui` (новый) | Чистые UI-модели: раскладка баннера, `MigrateState`/`MigrateLayout` (+ скролл-следование выбора), `toast_action_rect` | FR-105 |
| canvas-core `workspace` | Чистая часть watch: `WatchSnapshot`, `snapshot_changed` (№45b/№53b) + нативные тесты (исполняются и под wasip1) | FR-103 (дополнение) |
| canvas-ui `kit` | Фасадные реэкспорты констант баннера (`BANNER_*` — потребители не тянут внутренний путь `component::banner`) | FR-055 |
| canvas-app `i18n` | 2 новых ключа: `canvas.migrate.failed_toast`, `canvas.ext.reload_action` (остальные `canvas.storage.*`/`canvas.migrate.*` — из C0, потреблены) | FR-040 |
| Пользовательское поведение | Новое: тихий старт в папке (Chromium, granted), баннер при потере доступа, диалог переезда, тост внешних изменений с «Перезагрузить» | — |

## Анализ (Root Cause)

До C2 web-слой работал только с одним активным файлом (`fs_access.rs`:
ОДИНОЧНЫЙ file-хэндл «Открыть с диска…», `.bak` — в OPFS, т.к. браузер не
отдаёт родителя file-хэндла; `opfs.rs` — одноканвасная OPFS-библиотека
C1). Отсутствовало:

1. **Хранилище над папкой**: `window.showDirectoryPicker()` не вызывался
   вовсе; dir-хэндл не персистился (IndexedDB-глю хранил только file-хэндлы).
2. **Выбор режима старта** (№41c): логики «granted dir-хэндл → папка, иначе
   тихо OPFS» не было; права запрашивались только по жесту.
3. **Миграция** (№42a/№52a): план — чистая функция C0, но исполнителя
   (копирование/проверка/удаление с отказоустойчивостью) не существовало.
4. **Потеря доступа** (№44b): NotAllowedError молча ронял автосейв в лог;
   пользователь не понимал, что правки не сохраняются.
5. **Watch внешних изменений** (№45b/№53b): DOM-панель W6 не следила за
   файлом; внешняя правка затиралась следующим автосейвом без предупреждения.

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Зеркало папки | `canvas-web/src/fs_folder.rs` (новый) | `FsAccessStore { inner: Mutex<FolderState> }`; `FolderState` — BTreeMap «имя → ts» + монотонный clock + `outbox: Vec<FolderOp>`; `seed_listing` — перестройка из `dirList()`. Семантика = `MemWorkspaceStore` (эталон C0): create — NameInvalid/NameTaken (регистронезависимо)/LimitReached, сеет пустой `Canvas::default`; rename — файл + `.bak`-близнец, идемпотентный no-op тем же именем; delete — мягкое (`.bak`, из листинга исчезает); list — белый список `.canvas`, kind=Folder; exists — по зеркалу |
| Очередь операций | там же | `FolderOp::{Write, Rename, Delete}` → `spawn_folder_ops` (wasm): write — createWritable→write→close; delete — прочесть текст → записать `.bak` → removeEntry (НЕ удаляем оригинал без страховки); NotAllowedError → `AppEvent::StorageAccessLost` (№44b), прочее — warn |
| rename R-T6 | там же | `dir_rename`: для `(old→new)` и `(old.bak→new.bak)` — сначала JS-глю `handleMove(handle, newName)` (нестандартный `handle.move()` Chromium; ответ "moved"/"unsupported"/"error"), фолбэк — read+write+delete |
| Старт №41c | там же | Чистая `choose_start_mode(fs_available, saved_handle, granted)` (тесты на все комбинации; Firefox/Safari → всегда Opfs); `try_folder_start` вызывается из `opfs::init_scene` ДО OPFS-пути (правка минимальна, помечена `// FR-105`): query-разрешение без жеста, стартовый канвас `?canvas=` → недавний → default; файла нет — сеется пустой |
| Персист хэндла | `index.html` + `fs_folder` | Глю `dirHandlePut/dirHandleGet` (хранилище `handles`, ключ `"workspace"` — образец `handlePut/handleGet`); `saved_dir_handle`/`store_dir_handle` |
| Миграция | `canvas-web/src/workspace.rs` + `fs_folder` | План — `canvas_core::workspace::migration_plan` (C0, без изменений). Исполнитель: трейт `MigrationIo` (read_source/write_target/target_has/remove_source — шов для нативных двойников) + `MigrationDriver` (шаги `Read/Write/Verify/Remove/Done` с ответами `*_complete/*_fail` — та же машина на wasm, что в тестах) + `execute_migration` (нативный прогон). **ПОРЯДОК ВАЖЕН (№52a): все копирования — до первого удаления; оригинал удаляется только после успешных Write И Verify.** Частичный сбой: отказ копирования — оригинал не тронут (`failed`); отказ удаления после копии — обе стороны живы (`kept`); повторный прогон идемпотентен (коллизия цели — авто-суффикс нового плана). Отчёт `MigrationReport { moved, failed, kept, missing }` |
| Диалог миграции | `canvas-app/src/storage_ui.rs` (новый) + `app` | `MigrateState` (entries/checked/selected/scroll_top/active): по умолчанию все галочки (№42a — переезжает всё, пользователь снимает лишнее); активный канвас обязателен (`is_mandatory`, toggle не снимает). Раскладка `migrate_layout` — чистая (панель 480, окно 10 строк `LIST_ROW_H`, футер «Переехать на диск…»/«Отмена»); колесо — `migrate_wheel_scroll`; стрелки ведут окно (`migrate_scroll_to_reveal`, инвариант `RowScroll::ensure_visible` LAY-W9/K3). Оверлей — паттерн галереи схем (модаль Modals/Block, Esc/«Отмена»/«✕»/клик мимо — закрыть), рисование `overlays.rs` (kit::modal_style, TextMeasurer/ellipsis, чекбокс — кнопка кита + глиф «✓») |
| Точка входа миграции | `app_spawn` | Кнопка «Переехать…» диалога → мост `run_migration(selected)` → `pick_folder_and_migrate` (пикер в жесте клика): страховка активного в selected → пикер → персист хэндла → план → `MigrationDriver` по async-шагам → чистка `.bak`-близнецов переехавших → переключение в режим папки (`activate_folder` + `OpenScene` + `MigrateDone`). Строка менеджера «Переехать на диск…» — волна C3 (AppEvent `MigrateShowDialog` готов); до неё — ОТЛАДОЧНЫЙ вход `?migrate=1` (временный, удалить в C3) |
| Баннер №44b | `storage_ui` + `app`/`handler`/`input`/`ui_registry` | `AppEvent::StorageAccessLost { detail }` → `storage_banner` Some → полоса Panels (Capture, hit-rect'ы ТОЛЬКО кнопки — канвас под баннером жив, не блокирует работу). «Переподключить» → мост `reconnect_folder` → `request_dir_granted` (requestPermission В ЖЕСТЕ) → `StorageReconnected`. «Переключиться в браузерное» → `switch_to_browser(name, json)`: активный канвас с текущим содержимым сеется в OPFS, режим — OPFS (dir-хэндл остаётся в IndexedDB; следующий старт — тихий OPFS, №41c) |
| Watch №45b/№53b | `canvas-core/workspace.rs` + `fs_folder` | Чистые `WatchSnapshot` (BTreeMap имя→ts) и `snapshot_changed` (изменившийся ts или новый файл; удалённые — не «изменение»); `folder_watch_snapshot` — белый список `.canvas`. `install_watch` (DOM focus/visibilitychange → `poll_external_changes`): только в режиме папки; query ≠ granted → баннер №44b; diff базы → изменение активного → `AppEvent::ExtFileChanged`. База обновляется после старта/миграции/переподключения/собственных записей (`schedule_watch_refresh` — иначе свой автосейв счёлся бы внешним). «Перезагрузить» → `reload_after_external`: локальный json (были правки) сперва в `.bak`, сцена переоткрывается текстом из папки (`OpenScene`) |
| Тост с действием | `app`/`handler`/`ui_registry` + `storage_ui` | `toast_action: Option<ToastAction>` (пока только `ReloadExternal`): кнопка «Перезагрузить» справа от центрированного текста (`toast_action_rect`), hit-зона — поверхность TOAST (Toasts/Capture, интерактивна ТОЛЬКО кнопка); TTL 8 с (прочитать и нажать), действие умирает вместе с тостом. Полоса с действием растёт до `TOAST_ACTION_STRIP_H` (= `kit::BUTTON_HEIGHT`) — scissor-клип полосы (FR-CLIP) не режет низ кнопки |
| Интеграция с ядром | `fs_access.rs` | `spawn_disk_flush` в режиме папки пишет И файл, И `.bak` в granted-папку (R-T6: у dir-хэндла есть родитель — sibling-`.bak` возможен, в отличие от одиночного файла); NotAllowedError → баннер; папка утрачена — страховка OPFS |
| i18n | `canvas-app/src/i18n.rs` | Новые: `canvas.migrate.failed_toast` (частичный сбой — оригиналы целы), `canvas.ext.reload_action` («Перезагрузить»). Потреблены ключи C0: `canvas.storage.{lost_banner,reconnect,switch_browser,move_to_disk}`, `canvas.migrate.{title,hint,done_toast}`, `canvas.ext.changed_toast` |

## Отказоустойчивость миграции (№52a — дублирую порядок фаз)

Инвариант исполнителя: **источник удаляется только после успешных
Write И Verify; ВСЕ копирования — до первого удаления** (журнал
операций тест-двойника это пинит: `migration_writes_all_before_first_remove`).

- отказ `read`/`write` одной пары → пара в `failed`, оригинал не тронут,
  остальные пары доезжают (сбой не блокирует соседей);
- отказ `verify` (цели нет) или `remove` → пара в `kept`: копия в папке +
  оригинал в OPFS — данные не теряются, повторный прогон идемпотентен
  (коллизия в цели решится авто-суффиксом нового плана №26b);
- чистка `.bak`-близнецов переехавших — лучший-эффорт после успехов
  (отказ не блокирует переезд, дубль-бак не содержит данных сверх копии).

Если активный канвас не переехал (частичный сбой) — переключения режима
НЕ происходит, тост `canvas.migrate.failed_toast`, сцена продолжает жить
в OPFS. Успех: `MigrateDone` + тост `canvas.migrate.done_toast`, сцена
переоткрывается из папки (`OpenScene` + папочное `FsAccessStorage`).

## Точки входа (Entry Points)

- Волна C3 (менеджер-оверлей): строка «Хранилище: браузерное
  [Переехать на диск…]» (№51a) открывает диалог миграции — событие
  `AppEvent::MigrateShowDialog` уже в конвейере; **убрать отладочный
  `?migrate=1`** (`url_params.rs` + `app_spawn.rs`, помечено «временный
  до C3»); `fs_store()` (`web_state`) — общий листинг папки для менеджера.
- Волна C4: перенос ключа камеры при ренейме (`camera_key_for`, №30b).
- `docs/SPEC.md` §3/§4 (модель/платформенный слой), user-docs — волна C5.
- `docs/ACCEPTANCE.md` — сценарий «переезд» (список тот же, OPFS пуст) —
  ручная приёмка владельца после выката.

## Проверка (Verification)

Автогейты (зелёные на коммите):

- `cargo fmt --check` — OK;
- `cargo clippy --workspace -- -D warnings` — OK;
- `cargo test --workspace` — 2954 passed / 0 failed (в т.ч. 31 новый тест
  C2: core `snapshot_changed` ×2; web-`workspace` миграция ×7 (порядок фаз,
  частичный сбой, kept, суффиксы, машина состояний, идемпотентность
  ответов); web-`fs_folder` ×6 (старт-режим, контракт зеркала ×4,
  seed_listing); app `storage_ui` ×10 (баннер ×2, состояние/раскладка/
  скролл ×6, тост ×1, минимальный вьюпорт ×1); app-поведение ×5 (баннер,
  диалог, тост-действие, клавиатура-скролл, hit-rect'ы поверхностей);
  `url_params` ×1; i18n-полнота — инвариантом FR-040);
- `scripts/wasm_gate.sh --check` — OK (canvas-web компилируется под
  wasm32, unstable-cfg включён).

RED-верификация (мутации, тест обязаны падать; после каждой — код
восстановлен байт-в-байт):

1. фазы миграции «удаление после первой копии» → 3 теста миграции падают;
2. `toggle` без guard обязательной строки → 2 теста падают;
3. `choose_start_mode` без условия granted → тест старт-режима падает;
4. `migrate_scroll_to_reveal` no-op → 2 теста (storage_ui + app) падают.

WASM L2 (браузерный стенд) НЕ выполнялся: wasm-bindgen CLI отсутствует
в окружении (минимальный rustup-тулчейн), машина 2 ядра/4 ГБ — установка
`wasm-bindgen-cli` съест бюджет сессии; рендер-пути канваса не тронуты
(дельта — оверлеи на существующем конвейере полос). Ручная проверка
владельцем (Chromium, web-стенд `trunk serve` / Pages):

1. **Тихий старт №41c**: первый визит — канвас в OPFS (без диалогов);
   «переезд» (ниже) + перезагрузка страницы → старт в режиме папки без
   вопросов (granted сохраняется в сессии Chromium); вкладка с
   отозванным разрешением (chrome://settings — снять доступ) → старт
   тихо OPFS.
2. **Сценарий «переезд»** (приёмка issue #6): `?migrate=1` → диалог:
   список канвасов OPFS с галочками (все отмечены, активный заблокирован)
   → «Переехать на диск…» → пикер папки → тост «Канвасы переехали на
   диск»; в папке лежат все выбранные `.canvas`; OPFS пуст (DevTools →
   Application → OPFS); автосейв после правки пишет в папку (файл
   меняется на диске).
3. **Баннер №44b**: снять разрешение папки (chrome://settings) → правка
   на канвасе → баннер «нет доступа к папке» сверху; «Переподключить» →
   запрос → баннер снят, сейв живой; «Переключиться в браузерное» →
   режим OPFS, канвас с правками сеется в OPFS; канвас под баннером
   редактируется (баннер не блокирует).
4. **Watch №45b**: папка подключена → править `.canvas` внешним
   редактором → Alt-Tab обратно → тост «Файл изменился снаружи —
   перезагрузить?» (всегда, даже без локальных правок) → «Перезагрузить»
   → сцена из файла; при локальных правках прежняя версия — в
   `<имя>.canvas.bak` в папке.
5. **Firefox/Safari**: приложение открывается в OPFS-режиме; `?migrate=1`
   → диалог, но пикер не открывается (API нет) — тихий отказ в лог.
6. Клавиатура диалога: ↑/↓ (выбор, окно следует), Space (галочка, кроме
   активного), Enter («Переехать…»), Esc/«Отмена»/«✕»/клик мимо — закрыть.

## История изменений (Changelog)

- 2026-10-10 — агент (Super Z, subagent MC-C2): создан по шаблону;
  реализация волны C2 (fs_folder + исполнитель миграции + баннер/диалог/
  тост-действие + watch + i18n-ключи, 30 нативных тестов); статус
  «реализовано, ожидает приёмки».
- 2026-10-10 — финализация после прерывания сессии: ребейз на main
  (990ed92 — C1-конвейер), замена собственного моста StorageBridge на
  переиспользование WebRequest-конвейера C1 (унификация обратного канала),
  fmt-дочистка дерева (вкл. волю Wave T), повторные гейты — 2954/0;
  координатор (Super Z main): ревью задела, FR-105-сверка, работа завершена
  от имени MC-C2.

## Источники истины (References)

- План: `download/multicanvas-design-plan.md` v2.1 (решения №41c/№42a/
  №44b/№45b/№52a/№53b, риск R-T6).
- Контракты C0: `crates/canvas-core/src/workspace.rs` (migration_plan),
  `crates/canvas-web/src/workspace.rs` (WorkspaceStore/MemWorkspaceStore),
  `crates/canvas-app/src/i18n.rs` (canvas.*).
- FR-103 (`fr-103-multicanvas-c0-contracts.md`) — контракты; FR-104
  (волна C1, wt-c1) — конвейер AppEvent/WebRequest, параллельная волна.
- Паттерны: `crates/canvas-web/src/opfs.rs` (MirrorStore, init_scene),
  `fs_access.rs` (FsAccessStorage, spawn_disk_flush, available()),
  `js_glue.rs` + `index.html` (window.__canvasdesk, IndexedDB-глю),
  `crates/canvas-app/src/scheme_gallery_ui.rs` (оверлей-диалог),
  `crates/canvas-ui/src/kit.rs` (banner/modal/button, константы).
- GitHub: danku13/CanvasDesk issues #6 (эта волна), #14 (high-level).
