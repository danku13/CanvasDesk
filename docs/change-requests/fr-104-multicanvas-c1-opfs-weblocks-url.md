# FR-104: Мультиканвас C1 — OPFS-примитивы, Web Locks, URL-синк

- **Статус:** реализовано (код+тесты), ожидает приёмки владельца
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** danku13 (решения №1–54), агент (реализация)
- **Источник:** high-level issue #14 «Мультиканвас — менеджер канвасов, FS-first, Web Locks» + sub-issue #5 (волна C1); план `download/multicanvas-design-plan.md` v2.1 §4 (волны C1∥C2)
- **Связанные задачи:** FR-103 (контракты C0 — опора); волны C2 (#6, FS Access), C3 (#7, менеджер UI), C4 (#8), C5 (#9); FR-017/FR-064 (what-if — потребление ключа `active`); FR-040 (i18n — потребление ключей C0); wasm-port §4.1/§4.2 (OPFS-мост)
- **Создан:** 2026-10-10
- **Обновлён:** 2026-10-10 (реализация, код+тесты)

---

## Описание (What)

Волна C1 мультиканваса — платформенные примитивы web-слоя на контрактах C0
(FR-103): `OpfsStore` (реализация `WorkspaceStore` над OPFS: листинг/создание/
ренейм/мягкое удаление через JS-глю `opfsList/opfsRename/opfsDelete`),
AppEvent-конвейер хранилища (`RequestCanvasList/CanvasList/CanvasOpDone`),
Web Locks-детект «канвас уже открыт в другой вкладке» (№14b) с модалом
№35a, URL-синк `history.replaceState(?canvas=)` (№17a), новая семантика битой
ссылки `?canvas=` (№31c: фолбэк на существующий недавний + тост), запись/чтение
активного what-if сценария в `.canvas` (№32c/№36b, интеграция со схемой C0) и
`navigator.storage.persist()` (R-T3 — защита OPFS от eviction до переезда).

Пользовательского UI-списка канвасов в C1 нет (менеджер — волна C3):
видимые пользователю изменения — тосты (битая ссылка, восстановленный
сценарий), модал занятости вкладки и `?canvas=` в адресной строке.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| canvas-web `opfs_store` (новый) | `OpfsStore` над OPFS: зеркало «имя → ts» + фоновые мутации spawn_local; чистые функции №31c (`resolve_url_canvas`, `broken_link_fallback`) | FR-104, wasm-port §4.1 (дополнить в C5) |
| canvas-web `web_locks` (новый) | Имя лока `canvasdesk.canvas.{opfs\|disk}.<файл>` (чистая функция) + захват через глю `locksAcquire`; занятость → `AppEvent::CanvasLockBusy` | FR-104 |
| canvas-web `url_sync` (новый) | `replace_canvas_param` (чистая) + `history.replaceState` при каждой смене активного канваса (№17a) | FR-104 |
| canvas-web `web_requests` (новый, wasm-only) | Обработчик `WebRequest`-ов — обратный канал App→web (листинг/операции/фолбэк №35a→№31c) | FR-104 §Обратный канал |
| canvas-web `web_state` | `set_active` — единая точка смены канваса: захват Web Lock + URL-синк; thread_local `OPFS_WORKSPACE` (`Arc<OpfsStore>`) + `EVENT_PROXY` | FR-104 |
| canvas-web `opfs` (`init_scene`/`choose_canvas`) | Зеркало workspace сеется на старте; битый `?canvas=` не сеется — фолбэк №31c + `WebScene.broken_link`; `persist()` (R-T3) | wasm-port §4.2 |
| canvas-web `index.html` | Глю `opfsList/opfsRename/opfsDelete` + `locksAcquire` (Web Locks с `ifAvailable`) в `window.__canvasdesk` | FR-104 |
| canvas-app `app` | AppEvent-варианты `RequestCanvasList/CanvasList/CanvasOpDone/CanvasLockBusy` + `WebRequest`-очередь; `AppDialog::CanvasTabBusy` (модал №35a); тосты №31c/№36b; `switch_whatif_scenario`/`write_whatif_active` (№32c — одним undo-шагом) | FR-104, план §3.2 |
| canvas-app `overlays` | Чипы сценария/Базы и правки сценариев пишут `canvasdesk.whatif.active` тем же undo-шагом (№32c) | FR-017 v3 |
| canvas-scene `scene` | `restored_scenario` + восстановление `active_scenario`/`whatif_active` при загрузке (№32c/№36b) | FR-017/FR-064 |
| canvas-core `workspace` | `CanvasOp` (payload `CanvasOpDone`; в ядре — canvas-app не зависит от canvas-web) | FR-103 (дополнение) |
| Пользовательское поведение | `?canvas=` переживёт перезагрузку только для существующего файла; битый — фолбэк+тост; занятость вкладки — модал; сценарий восстанавливается с тостом | план §2 (№17a/№31c/№35a/№36b) |

## Анализ (Root Cause)

1. **Листинга OPFS не было.** `opfs.rs` держит только активный файл
   (MirrorStore + очередь bak-пар); менеджеру (C3) нужен список ВСЕХ
   канвасов с ts — глю `recentList` хранит хэндлы дисков, не OPFS-файлы.
2. **Мутаций имён не было.** Переименование/удаление/создание канваса как
   файлов отсутствовали вовсе (`?canvas=` только выбирал имя для чтения).
3. **Битая ссылка сеяла канвас.** `choose_canvas` отдавал имя из URL, а при
   отсутствии файла `init_scene` сеял новый под этим именем (seed_over) —
   ссылка на удалённый канвас материализовала пустой (противоречие №31c).
4. **URL не следовал за канвасом.** `?canvas=` читался только на старте,
   смена активного канваса (reopen/drop/фолбэк) не отражалась в адресе.
5. **Нет детекта двух вкладок.** Автосейв OPFS — тихий last-write-wins:
   два таба на один файл теряли правки друг друга без предупреждения (№14b).
6. **Активный сценарий — runtime-only.** `SceneState::active_scenario = None`
   при загрузке (`scene.rs:478`), ключ `canvasdesk.whatif.active` из C0
   не имел потребителя (№32c/№36b).
7. **App не может звать web-слой.** canvas-app платформенно-нейтрален
   (canvas-web зависит от него, не наоборот) — конвейеру хранилища нужен
   обратный канал, готового механизма не было (tour-сигналы — однонаправленные).

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| JS-глю OPFS | `index.html` (`window.__canvasdesk`) | `opfsList()` — все файлы корня `{name, ts=lastModified}` через `for await … entries()` (фильтрация — Rust); `opfsRename(old,new)` — `fileHandle.move(newName)` с фолбэком read→write→removeEntry, ОБЯЗАТЕЛЬНО перенос `.bak`-близнеца; `opfsDelete(name)` — мягкое: текст → `<name>.bak` (замещая прежний), затем `removeEntry`. Протокол ответов мутаций: `"ok" \| "notfound" \| "error:<текст>"` |
| OpfsStore | `opfs_store.rs` (новый) | Реализация `WorkspaceStore` (сигнатуры трейта не менялись): синхронные методы — по зеркалу «имя → ts»; create/rename/delete валидируются синхронно (семантика = `MemWorkspaceStore`, эталон C0: NameTaken регистронезависимо, NameInvalid без `.canvas`/пустое, LimitReached `MAX_CANVASES`, NotFound, мягкий delete), зеркало обновляется оптимистично, сама мутация — фоновый `spawn_local` (fire-and-forget, ошибки — tracing target `canvas_web` + refresh листингом). `list` — белый список `.canvas` (`.bak`/посторонние скрыты, R-T7) |
| Rust-обёртки | `opfs_store.rs` | `opfs_list/opfs_rename/opfs_delete` через `crate::js_glue::call` (паттерн `recent.rs`); разбор ответа — `parse_files` (битые элементы пропускаются) |
| Зеркало | `opfs_store.rs`, `opfs.rs` | `seed_listing` (полный opfsList) — при `init_scene` (внутри `choose_canvas`, до выбора имени) и по каждому `RequestCanvasList`; `seed_file` — точечное сеяние (стартовое default.canvas/фолбэк) |
| Общий экземпляр | `web_state.rs` | thread_local `OPFS_WORKSPACE: Option<Arc<OpfsStore>>` (паттерн `OPFS_STORAGE`); `JsValue` в трейт-объекты не попадает — `Send+Sync` не нарушены |
| AppEvent-конвейер | `app.rs` (маркированный блок `// FR-104` в enum), `handler.rs` | `RequestCanvasList` (запрос) / `CanvasList(Vec<CanvasEntry>)` (ответ) / `CanvasOpDone { op: CanvasOp, error: Option<String> }`; `CanvasLockBusy { name }` — занятость лока. `CanvasOp` — payload в canvas-core (Create/Rename/Delete), ошибка — человекочитаемый текст `WorkspaceError::to_string` |
| Обратный канал | `app.rs` (`WebRequest`), `tour_aware_app.rs`, `web_requests.rs` | Механизм — очередь `App::pending_web_requests` + дренаж обёрткой `TourAwareApp` после каждого события цикла (паттерн tour-сигналов FR-028 v2), ответы — `AppEvent`-ами через `EventLoopProxy` (`web_state::event_proxy`). Детали — §Обратный канал |
| Web Locks | `web_locks.rs`, `index.html`, `web_state.rs` | Биндинг `web_sys::LockManager` требует `--cfg=web_sys_unstable_apis` (сборка не передаёт) — захват через глю `locksAcquire` (`navigator.locks.request(name, {ifAvailable:true}, cb)`, сериализованная цепочка, предыдущий лок отпускается). Захват в `set_active` при каждой смене канваса; занятость → `CanvasLockBusy` (или pending-флаг при старте до event loop). Имя лока — чистая функция `lock_name_for` (нативный тест) |
| Модал №35a | `app.rs` (`AppDialog::CanvasTabBusy`), `overlays.rs` | Стандартный механизм диалога (T21: kit::modal, FR-060 измеренная геометрия — без новой вёрстки): заголовок `canvas.tab.already_open` (ключ C0 без `{name}` — имя показывает тело), кнопки `canvas.tab.open_anyway` (confirm — продолжить БЕЗ лока, last-write-wins, §Семантика) / `canvas.tab.choose_other` (cancel — `WebRequest::CanvasFallback`, TODO: C3 заменит на менеджер) |
| URL-синк №17a | `url_sync.rs`, `web_state.rs` | `replace_canvas_param` (чистая: заменить/добавить/убрать параметр, соседи сохранены, `%XX`-кодирование значения) + `sync_active` в `set_active`: OPFS → `?canvas=<имя>`, диск → убрать параметр; `replaceState` (не pushState — история не обрастает шагами) |
| Битая ссылка №31c | `opfs_store.rs`, `opfs.rs`, `handler.rs` | `?canvas=` задан, файла нет → НЕ сеять под этим именем: `resolve_url_canvas` (точное → регистронезависимое совпадение), фолбэк `broken_link_fallback` (верхний из недавних по ts, который существует и ≠ битому регистронезависимо), иначе `default.canvas` (сеять допустимо только его); тост `canvas.link.broken_toast` через `pending_broken_link` на первом кадре (паттерн `?focus`) |
| Фолбэк «Выбрать другой» | `web_requests.rs` | `open_fallback(avoid)`: механика №31c; недавних нет → СВЕЖЕЕ автоимя «Canvas N» (`auto_name` C0), НЕ default.canvas — он может быть сам занят другой вкладкой (иначе модал зациклится, §Фолбэк занятости); открытие — как у reopen (OPFS-текст → зеркало → `OpenScene`) |
| Активный сценарий №32c | `scene.rs`, `app.rs`, `overlays.rs` | Загрузка: `resolve_active(scenarios, active_from_canvas(canvas))` → `active_scenario`+`whatif_active`+`restored_scenario` (тост №36b, «База» — тихо); подмены входят в первый пересчёт (recompute_flow повторно). Запись: `write_whatif_active` (`active_to_canvas`) ВНУТРИ undo-шага правок сценариев — чипы сценария/Базы (`switch_whatif_scenario`), автосоздание, удаление активного, сброс all; инвариант: файл без активного сценария — round-trip байт-в-байт (тест) |
| persist() R-T3 | `opfs_store.rs`, `opfs.rs` | `request_storage_persist()` — один вызов в `init_scene` (до первой записи; fire-and-forget, результат в лог); TODO: C3 перенесёт в точку первого открытия менеджера |
| Тесты (TDD) | см. §Проверка | 23 нативных теста: фильтр листинга, семантика зеркала (= эталон C0), выбор фолбэка, имя лока, payload'ы, URL-параметр, интеграция active-сценария (загрузка/протухший/undo-шаг), модал. RED-верификация мутациями (broken_link_fallback-фильтр, восстановление сценария) — тесты падают, код восстановлен байт-в-байт |

## Обратный канал (App → web-слой)

App платформенно-нейтрален: canvas-web — зависимость НАД canvas-app
(`tour_aware_app` оборачивает `App`), прямой вызов невозможен. Готового
механизма не было (tour-сигналы — только App→JS, однонаправленные).
Выбрано простейшее честное решение:

- App копит `WebRequest`-ы в `pending_web_requests` (`pub fn
  request_canvas_list / request_canvas_op`; толкает же `on_canvas_fallback`);
- обёртка `TourAwareApp` (уже дренажирует tour-сигналы после КАЖДОГО события
  цикла) дренажирует и запросы: `app.drain_web_requests()` →
  `web_requests::handle(request, proxy)` — fire-and-forget spawn_local-таск;
- ответы приезжают `AppEvent`-ами (`CanvasList/CanvasOpDone`) через
  `EventLoopProxy`, зарегистрированный в `web_state::set_event_proxy` сразу
  после построения event loop;
- занятость Web Locks до построения event loop (старт: `init_scene` раньше)
  копится в `web_locks::PENDING_BUSY` и забирается `take_pending_busy` →
  `app.set_pending_canvas_lock` (модал на первом кадре, паттерн `?focus`).

Latency минимальна (дренаж — после каждого события, не `about_to_wait`);
`wasm`-only (`web_requests` под `cfg(target_arch = "wasm32")`), нативные
тесты проверяют очередь/дренаж/payload'ы без web-рана.

## Семантика «Всё равно открыть» (№35a)

Confirm модала продолжает работу БЕЗ лока: обе вкладки пишут в один
OPFS-файл, побеждает последняя запись (last-write-wins автосейва,
задокументирован как осознанная семантика). Перевзятия лока не выполняется:
`set_active` идемпотентен (повторная установка того же канваса — no-op,
не дёргает лок/URL), сцена уже загружена. `document.title`-сигнал и
diff-детект — вне скоупа C1 (title — №28a/C4).

## Фолбэк занятости («Выбрать другой»)

№35a говорит «фолбэк как у битой ссылки №31c», но у битой ссылки терминальный
фолбэк — `default.canvas`. Для занятости это цикл: если default.canvas сам
открыт в другой вкладке, повторный захват снова занят → модал → фолбэк → …
Поэтому `WebRequest::CanvasFallback` при исчерпании недавних берёт СВЕЖЕЕ
автоимя «Canvas N» (`auto_name` C0, №6/№39c): новый канвас гарантированно
не занят другой вкладкой. Осознанное расширение семантики №31c для этого
пути (зафиксировано здесь; менеджер C3 заменит весь путь выбором из списка).

## Отклонения от формулировки задачи

- **Зеркало «имя → (текст, ts)» → «имя → ts».** Тексты канвасов в зеркало
  workspace не входят: активный канвас обслуживается `OpfsStorage`
  (автосейв), листингу/операциям текст не нужен, а полное чтение всех
  канвасов на старте — лишний I/O. `.bak`-близнецы и посторонние файлы —
  В зеркале (ренейм-фолбэк и мягкое удаление их видят), скрыты только из
  `list` белым списком.
- **`web_sys::LockManager` → JS-глю.** Биндинг существует, но требует
  `--cfg=web_sys_unstable_apis` (сборка его не передаёт) — захват через глю
  `locksAcquire` (краткость + без флагов сборки), семантика `ifAvailable`
  сохранена.
- **Мутации — fire-and-forget с оптимистичным зеркалом.** Синхронная часть
  трейта возвращает результат ВАЛИДАЦИИ (по зеркалу); async-отказ I/O —
  tracing error + refresh зеркала листингом (реальность побеждает).
  `CanvasOpDone` несёт синхронную часть — для UI-тостов C3 этого довольно
  (I/O-отказы редки и самолечатся листингом).

## Открытые вопросы (TODO)

- **№36b (передано из плана v2.1/FR-103):** решение хранить активный
  сценарий в `.canvas` принято владельцем БЕЗ финального понимания
  семантики «мультиканвас × мультивкладка» — отдельное уточнение
  предстоит (включая тост «Активен сценарий "X"»). Реализация C1 следует
  решению: имя сценария в `extra`, восстановление на загрузке, тост №36b.
- Выход из what-if режима (Esc) и MCP-активации — runtime-only, ключ
  `active` не трогают (осознанно: только пользовательские переключения
  чипов персистентны; переосмысление — вместе с №36b).
- TODO(FR-104) в коде: C3 заменит «Выбрать другой» на менеджер канвасов;
  C3 перенесёт `persist()` в точку первого открытия менеджера.

## Точки входа (Entry Points)

После C1 обновляются (в своих волнах): `docs/SPEC.md` §3/§4.2 (дополнить в
C5); `docs/interface-objects/` (canvas-manager — C3, chip — C4); user-docs
(C5 — страницы канвасов/хранилища/сценариев); `docs/ACCEPTANCE.md` (C5);
FR-017 v3 (потребление ключа active — ссылка на этот FR). Индекс CR/FR —
этим коммитом.

## Проверка (Verification)

Гейты (все зелёные):

- `cargo fmt --check` — OK;
- `cargo clippy --workspace -- -D warnings` — OK;
- `cargo test --workspace` — 2868 passed / 0 failed (в т.ч. 23 новых
  теста C1: opfs_store ×6, url_sync ×4, web_locks ×1, web_state ×1,
  canvas-core workspace ×1, canvas-scene ×3, canvas-app ×7);
- `scripts/wasm_gate.sh --check` — компиляция wasm32 зелёная (L0);
- RED-верификация: мутация `broken_link_fallback` (снятие фильтра битого
  имени) и отключение восстановления сценария в `with_storage` —
  соответствующие тесты падают, код восстановлен байт-в-байт.

Ручной дым (браузер, `?log=debug`, Chrome/Edge — OPFS+Web Locks):

1. `trunk serve` (или web-Pages): URL `?canvas=default.canvas` → открылся
   default; перезагрузка — тот же канвас; адресная строка держит
   `?canvas=default.canvas` (№17a).
2. `?canvas=ghost.canvas` (несуществующий) → открыт верхний из недавних
   (или default), тост «Канвас "ghost" не найден…» (№31c), файл ghost
   НЕ создан.
3. Открыть вторую вкладку на тот же `?canvas=…` → в первой ничего, во
   второй модал «Этот канвас уже открыт в другой вкладке»: «Всё равно
   открыть» — обе вкладки живут (last-write-wins); «Выбрать другой» —
   открывается недавний/новый «Canvas N» (№35a).
4. Канвас с what-if сценарием: переключить чип сценария → перезагрузить →
   сценарий восстановлен, тост «Активен сценарий "X"»; чип «База» →
   перезагрузка — без тоста, сценарий не восстановлен (№32c/№36b).
5. F12 → Application: persisted-статус хранилища (R-T3), лог
   `navigator.storage.persist()` — `?log=debug`.

WASM L2 (пиксельный диф/браузерный стенд) не выполнялся: изменения не
трогают раскладку/рендер (нет новых поверхностей, кроме стандартного
диалога); поведение проверяется ручным дымом выше (окружение сборки
стенда занято параллельным воркстримом C2 — общий target). Инструкция —
выше; исполнитель — владелец/координатор.

## История изменений (Changelog)

- 2026-10-10 — агент (MC-C1, Super Z): создан по шаблону; реализация волны
  C1 (OpfsStore+глю, конвейер WebRequest, Web Locks+модал, URL-синк,
  битая ссылка №31c, активный сценарий №32c/№36b, persist R-T3, 23 теста);
  статус «реализовано, ожидает приёмки».

## Источники истины (References)

- План: `download/multicanvas-design-plan.md` v2.1 §2/§3/§4 (решения
  №14b/№17a/№31c/№32c/№36b/№35a, R-T3/R-T7) — в репо внедряется в C5.
- FR-103 (`fr-103-multicanvas-c0-contracts.md`) — контракты C0: трейт
  `WorkspaceStore`, `MemWorkspaceStore` (эталон семантики), ключ
  `canvasdesk.whatif.active`, i18n-ключи.
- `crates/canvas-web/src/opfs.rs` — паттерн зеркала `MirrorStore`
  (wasm-port §4.1/§4.2), `init_scene`/`choose_canvas`.
- `crates/canvas-web/src/js_glue.rs` + `index.html` — паттерн JS-глю
  (`window.__canvasdesk`, IndexedDB recent/handles).
- `crates/canvas-app/src/app.rs` — `AppEvent` (конвейер), `AppDialog`
  (модал №35a), `WebRequest` (обратный канал).
- `crates/canvas-scene/src/scene.rs` — `with_storage`
  (восстановление активного сценария).
- MDN: [File System Access API — OPFS](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API),
  [Web Locks API](https://developer.mozilla.org/en-US/docs/Web/API/Web_Locks_API),
  [navigator.storage.persist()](https://developer.mozilla.org/en-US/docs/Web/API/StorageManager/persist).
- GitHub: danku13/CanvasDesk issues #5 (эта волна), #14 (high-level).
