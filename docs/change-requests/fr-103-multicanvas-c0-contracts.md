# FR-103: Мультиканвас C0 — контракты и чистые функции (WorkspaceStore)

- **Статус:** реализовано (код+тесты), ожидает приёмки владельца
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** danku13 (решения №1–54, RQ-6..10), агент (реализация)
- **Источник:** high-level issue #14 «Мультиканвас — менеджер канвасов, FS-first, Web Locks» + sub-issue #4 (волна C0); план `download/multicanvas-design-plan.md` v2.1 (внедряется в репо в C5 вместе с PRD-0011)
- **Связанные задачи:** волны C1 (#5), C2 (#6), C3 (#7), C4 (#8), C5 (#9); FR-017/FR-064 (what-if — расширяется ключом `active`); FR-040 (i18n-паттерн); wasm-port §4.1/§4.2 (OPFS-мост)
- **Создан:** 2026-10-10
- **Обновлён:** 2026-10-10 (реализация, код+тесты)

---

## Описание (What)

Волна C0 мультиканваса — контракт-первый этап: заморозить контракты и чистые
функции, на которых параллельно работают воркстримы C1 (OpfsStore + JS-глю +
URL-синк + Web Locks) и C2 (FsAccessStore + миграция), затем C3 (менеджер UI)
и C4/C5. До C0 web-слой имел только «одноканвасные» примитивы OPFS
(`opfs.rs`: чтение/запись одного файла активной сцены) — ни трейта
хранилища-«рабочего стола», ни модели записей, ни правил имён/коллизий,
ни ключа активного what-if сценария в `.canvas` не существовало.

Состав (sub-issue #4): трейт `WorkspaceStore` + тест-двойник; модель
`CanvasEntry` и группировка листинга (№43a); чистые функции имён
(«Canvas N» №39c, суффикс «(N)» №13a, «(копия)» №27a), валидация, сортировка,
план миграции OPFS→папка (№42a); схема ключа активного сценария
`canvasdesk.whatif.active` (№32c) с валидацией (протухший → «База»);
i18n-каркас RU/EN (~30 ключей).

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| canvas-core `workspace` (новый) | `CanvasEntry`/`EntryKind`/`MAX_CANVASES` (№16) + чистые функции имён/сортировки/групп/миграции; контракт для C1–C3 | SPEC §3 (модель данных — дополнить в C5), FR-103 |
| canvas-core `whatif` | Ключ `canvasdesk.whatif.active` (имя активного сценария, №32c): чтение/запись/резолв с валидацией; соседи `scenarios`/`frozen` не затрагиваются | FR-017 v3, FR-064 (дополнение ключом active) |
| canvas-web `workspace` (новый) | Трейт `WorkspaceStore` (list/create/rename/delete/exists) + `WorkspaceError` + тест-двойник `MemWorkspaceStore`; реализации OpfsStore/FsAccessStore — C1/C2 | wasm-port §4 (дополнить в C5) |
| canvas-app `i18n` | ~30 ключей `canvas.*`: менеджер, хранилище, миграция, вкладки, тосты (каркас — без потребителей до C3/C4) | FR-040 |
| Пользовательское поведение | Не меняется: C0 не содержит UI/ввода/рендера — только контракты и данные | — |

## Анализ (Root Cause)

1. **Хранилище web — один файл.** `opfs.rs` хранит только активный канвас:
   `MirrorStore` (зеркало «путь → текст» + очередь bak-пар), `init_scene`
   выбирает `?canvas=` → недавний → `default.canvas`. Листинга/создания/
   ренейма/удаления нет — менеджеру (C3) не к чему обращаться.
2. **Модель записей отсутствует.** `ActiveKind::{Opfs,Disk}` (web_state)
   описывает только активный файл, а не список; записей с ts/видом/бейджем
   репо (№43a/PRD-0011) нет ни в одном слое.
3. **Правила имён размазаны по решениям, не по коду.** №39c (латиница),
   №13a («(N)»), №27a («(копия)»), №26b (авто-суффикс + тост) — нигде не
   вычисляются; inline-ренейм (№9) требует валидации коллизий/символов.
4. **Активный what-if сценарий — runtime-only.** `scene.rs:478`:
   `active_scenario = None` при загрузке; в `.canvas` пишутся только
   `scenarios`/`frozen` (`whatif.rs`) — восстановление сценария при
   открытии канваса (№32c/№36b) нечем поддержать.

## Требуемые изменения (Changes)

| Что | Где | Как |
|---|---|---|
| Модель записей | `canvas-core/src/workspace.rs` (новый) | `CanvasEntry { name (имя файла с .canvas), ts (ms), kind: EntryKind::{Opfs, Folder, Disk}, repo: Option<String> }`; `EntryKind` — источник записи; `repo` — Some ⇒ запись в группе `repos/<имя>` (№43a) |
| Лимит | там же | `MAX_CANVASES: usize = 10_000` — единая точка проверки (№16: без пользовательского лимита, санити-гвард на бессмысленные объёмы) |
| Имена | там же | `auto_name` — «Canvas»/«Canvas 2»/… первый свободный, латиницей в обеих локалях (№39c); `collision_suffix` — « (1)», « (2)»… при занятости (№13a/№26b); `copy_name(base, existing, suffix)` — «{имя} {суффикс}» (№27a, суффикс — из i18n: RU «(копия)»/EN «(copy)»), коллизия поверх — авто-суффикс |
| Валидация | там же | `validate_canvas_name(raw)` — trim, непусто, ≤120 симв., запрет `/ \ : * ? " < > |` и управляющих (безопасность файлов на всех платформах), не `.`/`..`; `name_taken` — регистронезависимо (granted-папки на Windows нечувствительны к регистру — не создаём «notes»/«Notes» двойников) |
| Отображение | там же | `display_name` — без `.canvas`; `to_file_name(display)` — добавить расширение, если нет |
| Сортировка/группы | там же | `SortMode::{Name, ModifiedDesc}` + `sorted_entries` (чистая); `ListingGroups::group_entries` — корень плоско + группы `repos/<имя>` (BTreeMap — алфавит групп) |
| Ключ камеры | там же | `camera_key_for(name)` = `canvasdesk.camera.{name}` — контракт переноса при ренейме (№12/№30b, потребитель — C4) |
| План миграции | там же | `migration_plan(source, selected, target_existing)` → `MigrationPlan { copies: Vec<(из, куда)>, missing }` — коллизии в цели решаются авто-суффиксом (№26b), отсутствующие собираются отдельно (чекбокс-лист №42a не валидирует заранее) |
| Активный сценарий | `canvas-core/src/whatif.rs` | `active_from_canvas` / `active_to_canvas` (ключ `canvasdesk.whatif.active`, строка-ИМЯ сценария, `None` удаляет ключ и опустевшие контейнеры — паттерн `set_whatif_key`); `resolve_active(scenarios, stored)` — валидация имени по списку, протухший/отсутствующий → `None` = «База». **TODO (№36b): решение хранить активный сценарий в `.canvas` принято без финального понимания семантики multi-канвас×multi-вкладка; требует отдельного уточнения у владельца (в т.ч. тост восстановления)** |
| Трейт хранилища | `canvas-web/src/workspace.rs` (новый) | `trait WorkspaceStore { list/create/rename/delete/exists }` — синхронные сигнатуры по плану v2.1 §3.1: реализации держат зеркало (паттерн `MirrorStore`), async-конвейер — AppEvent (C1/C2). Семантика: list — белый список `.canvas`, `.bak` скрыт; create — пустой `Canvas::default()`, отказ при коллизии/лимите/невалидном имени; rename — атомарно файл + `.bak`-близнец, ключ камеры переносит вызывающий (`camera_key_for`); delete — мягкое: файл → `<name>.bak` (№15a). `WorkspaceError { NotFound, NameTaken, NameInvalid, LimitReached, AccessLost, Io }` (std-only impl Display/Error, без новых зависимостей) |
| Тест-двойник | там же | `MemWorkspaceStore` — BTreeMap-реализация трейта в памяти: контрактные тесты трейта (нативно) и эталон семантики для C1/C2 |
| i18n | `canvas-app/src/i18n.rs` | Ключи `canvas.manager.*` (заголовок/поиск/создать/пустой/шаблон/дубликат/импорт/ренейм/удалить/undo/пустое состояние/открыть с диска/сортировки/группа репо), `canvas.storage.*` (строка хранилища №51a, переезд, баннер потери №44b), `canvas.migrate.*` (№42a), `canvas.tab.*` (№35a), тосты: `canvas.switch.scenario_toast` (№36b), `canvas.link.broken_toast` (№31c), `canvas.ext.changed_toast` (№45b), `canvas.drop.renamed_toast` (№26b), `canvas.chip.rename_hint` (№9) |
| Тесты (TDD) | `workspace.rs` (core), `whatif.rs`, `workspace.rs` (web) | Core: имена/коллизии/копия/валидация/сортировка/группы/миграция/камера-ключ (12+ тестов). What-if: round-trip active, протухший → None, соседи сохраняются, пустой список → None. Web: контракт MemWorkspaceStore (создание/листинг фильтрует .bak/ренейм с .bak-близнецом/мягкое удаление/коллизии) |

## Точки входа (Entry Points)

После C0 обновляются (в своих волнах): `docs/SPEC.md` §3 (CanvasEntry — C5);
`docs/change-requests/fr-017-*.md` (v3 — ключ active, TODO-пометка);
`docs/interface-objects/` (canvas-manager — C3); user-docs (C5 — страницы
канвасов/хранилища); `docs/ACCEPTANCE.md` (C5). Индекс CR/FR — этим коммитом.

## Проверка (Verification)

- `cargo test --workspace` — включая новые модульные тесты (core/web);
- `cargo clippy --workspace -- -D warnings`; `cargo fmt --check`;
- `scripts/wasm_gate.sh --check` — canvas-core/canvas-web компилируются под
  wasm32 (трейт/типы — платформенно-нейтральные);
- i18n-инвариант FR-040: `tables_are_complete_and_consistent` (RU/EN полнота
  новых ключей);
- UI не меняется (ключи без потребителей, код путей не тронут) — WASM L2
  не требуется; L0-гейтом покрыт `wasm_gate --check`.

## История изменений (Changelog)

- 2026-10-10 — агент (Super Z): создан по шаблону; реализация волны C0
  (контракты canvas-core/canvas-web, ключ active, i18n-каркас, тесты);
  статус «реализовано, ожидает приёмки».

## Источники истины (References)

- План: `download/multicanvas-design-plan.md` v2.1 (сессия web-b5714086,
  решения №1–54 + RQ-6..10) — в репо внедряется в C5.
- `crates/canvas-web/src/opfs.rs` — MirrorStore-паттерн зеркала (§4.1/§4.2
  wasm-port), прецедент синхронного трейта поверх async-OPFS.
- `crates/canvas-core/src/whatif.rs` — паттерн ключей extra
  (`set_whatif_key`, round-trip инвариант 5 FR-017).
- `crates/canvas-app/src/i18n.rs` — FR-040: ключи-фразы, таблицы RU/EN,
  тест полноты.
- GitHub: danku13/CanvasDesk issues #4 (эта волна), #14 (high-level).
