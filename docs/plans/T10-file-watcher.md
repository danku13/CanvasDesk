# План T10 — Файловый вотчер

Статус: план, к реализации в отдельной сессии (= один коммит).
Источники: `docs/TASKS.md` T10, `docs/SPEC.md` §7.5, `docs/RECIPES.md` R12
(известное ограничение: корзина через Explorer — не чинить здесь, T16).
Базовый коммит плана: 3f12ba9.

## 1. Цель и критерии приёмки

`notify` 6+ (актуальная стабильная — 8.x, API `EventKind` тот же):
NonRecursive-вотчер на каждую директорию, из которой есть файловые ноды.
Debounce 300 мс. События → модель через очередь → автосейв `.canvas`.

Критерии (TASKS.md): три сценария SPEC §2 п.5 работают < 1 с:
- **Modify** → инвалидировать тамбнейл-кэш, перезапросить;
- **Rename/Move** → обновить `file`-путь в модели и в `.canvas`
  (rename detection по notify events both mode);
- **Remove** → `brokenLink: true`: серая рамка, иконка «файл недоступен»,
  тултип со старым путём; восстановление файла по тому же пути — флаг
  снимается автоматически.

Тесты: интеграционные на tempdir — rename/remove/restore.

## 2. Что уже есть (инвентарь)

| Что | Где | Значение для T10 |
|---|---|---|
| `Node.broken_link: Option<bool>` (serde `brokenLink`) | `canvas-core/src/model.rs` | поле модели уже сериализуется в .canvas |
| Отрисовка broken-карточки | `canvas-render/src/cards.rs`: `BROKEN_BORDER`, `card_instance` (серая рамка + шейдер-флаг) | **рендер уже готов** — тест `instances_reflect_selection_and_broken` зелёный |
| `ThumbCache` (SQLite, mtime-инвалидация) | `canvas-shell/src/cache.rs`: `get(path, mtime, size_class)` | ключ — (path, mtime): mtime в ключе → Modify с новым mtime = промах кэша = перезапрос. Явного delete не нужно |
| `Renderer::set_thumbnail / has_thumbnail / invalidate_node_caches` | `canvas-render/src/renderer.rs` | сброс атласа при внешних изменениях |
| `thumbs_failed: HashSet<usize>` | `canvas-app/src/main.rs` | негативный кэш: восстановление файла → убрать из set |
| `AppEvent` + `EventLoopProxy` | main.rs | добавляем `AppEvent::FileEvents(Vec<FileEvent>)` |
| `SceneState.resolve_file_path` | main.rs | резолв нодных путей к абсолютным — вотчер работает в абсолютных |
| `canvas_core::Thumbnail` и провайдеры | core/shell | после Modify — заказ как обычно через `order_thumbnails` |

## 3. Архитектура

```
canvas-core/src/fs_events.rs   (платформенно-независимая модель)
  pub enum FileEvent { Modify(PathBuf), Rename(PathBuf, PathBuf), Remove(PathBuf), Create(PathBuf) }
  pub fn normalize_path(p: &Path) -> PathBuf      // \\?\-префикс, \ → /, на windows — lower-case
  pub fn path_matches(node_file: &str, abs: &Path) -> bool  // сравнение после резолва+нормализации
  pub fn apply_file_events(canvas, events) -> Vec<NodeChange>  // чистая, для тестов

canvas-shell/src/watcher.rs    (сервис; notify — не windows-only, как ThumbService)
  pub struct WatchService { ... }
  impl WatchService {
      pub fn new(sender: Arc<dyn Fn(Vec<FileEvent>) + Send + Sync>) -> Self
      pub fn sync_dirs(&mut self, canvas: &Canvas, canvas_dir: &Path)  // desired-set родительских директорий
  }
  - notify::recommended_watcher → mpsc-канал → поток-агрегатор:
    debounce 300 мс (окно собирает шквал Modify), склейка
    remove+create пары в Rename (RenameMode::Both приоритетнее),
    дедуп одинаковых событий, затем batch наружу
  - зависимость notify — в canvas-shell (общая, не под cfg(windows),
    чтобы интеграционные тесты гонялись и на Linux CI/dev)

canvas-app/src/main.rs
  AppEvent::FileEvents(Vec<FileEvent>) → scene.apply (через apply_file_events)
  → renderer-эффекты + mark_dirty + sync_dirs(если набор директорий изменился)
  hover-тултип битой ноды: screen-space текст со старым путём
```

## 4. Мэппинг событий → действия

| Событие | Модель | Рендер/кэш |
|---|---|---|
| Modify(path) | — (путь не меняется) | `thumbs_failed.remove(idx)`; `invalidate_node_caches()`; SQLite промахнётся по mtime сам; `request_redraw` |
| Rename(from,to) | `node.file = относительный путь к to`, если нода резолвится в from | `mark_dirty` (автосейв); `sync_dirs` |
| Remove(path) | `node.broken_link = Some(true)` | `request_redraw`; тамбнейл можно оставить (иначе пустая карточка) |
| Create(path) на ноде с `broken_link == Some(true)` и тем же путём | `broken_link = None` | `thumbs_failed.remove(idx)`; `request_redraw` |

Восстановление = Create по тому же пути — покрывается тем же Rename-мэппингом
(path_matches). «Тултип со старым путём» (SPEC): при hover на broken-ноде
рисуем `ScreenText` с `node.file` у курсора — ScreenText уже есть в
`FrameOverlay.screen_texts`, мини-шаг в main.rs, без нового рендер-модуля.

## 5. Пошаговый план (TDD)

**Шаг 1 — `canvas-core::fs_events::normalize_path` + `path_matches` + тесты.**
ReadDirectoryChangesW отдаёт пути с `\\?\`-префиксом и `\`-разделителями;
пути из Explorer/нашей модели — без. Нормализация: срезать `\\?\`,
унифицировать разделители к `\` (внутри сравнения), на Windows —
case-fold (NTFS case-insensitive), сравнение компонент. Тесты:
`\\?\C:\a\b.png` == `C:/a/b.png` (windows); относительный node.file
резолвится от каталога канваса и матчится.

**Шаг 2 — `FileEvent` + `apply_file_events` (чистая) + тесты.**
Вход: canvas + события; выход: Vec<NodeChange> (какие ноды/что поменяли),
мутация canvas. Тесты (без ОС, чисто модель):
- modify не меняет файл, но возвращает NodeChange::ThumbStale(idx);
- rename обновляет file у резолвящейся ноды, .canvas round-trip хранит
  новый путь (существующий тест-паттерн `to_json/from_str`);
- remove → broken_link=Some(true), create по тому же пути → None;
- remove ноды без ноды-владельца — игнор (чужой файл в той же папке);
- rename папки, содержащей ноды (все file внутри префикса) — обновить
  все вложенные пути (PATH-префикс матчинг, не только точный).

**Шаг 3 — `WatchService` (canvas-shell) + интеграционные тесты на tempdir.**
- `new(sender)`: `notify::recommended_watcher` с mpsc-каналом; поток
  агрегатор: recv_timeout(300 мс) окно, склейка Modify-шквала, пара
  Remove+Create одного базового имени → Rename (если notify сам дал
  RenameMode::Both — берём как есть), дедуп, батч → sender.
- `sync_dirs`: собрать set родительских директорий (каталог каждой
  файловой ноды после `resolve_file_path`) — добавить недостающие
  NonRecursive-вотчеры, снять лишние (watcher на директорию;
  notify one-watcher-per-dir — держим `HashMap<PathBuf, RecommendedWatcher>`).
- Тесты (tempdir, ждём `recv_timeout(2s)` — debounce 300 мс + запас):
  create файл → Create; modify (touch+write) → Modify; rename →
  Rename(from,to); remove → Remove; create заново → Create;
  rename-папки → события для детей не приходят (NonRecursive — папка
  наблюдалась родителем: событие на самой папке достаточно).
- Windows-специфика: rename внутри наблюдаемой папки приходит как
  `EventKind::Modify(Name(Both))` — тест фиксирует это (cfg-ветка).

**Шаг 4 — интеграция в main.rs.**
- `AppEvent::FileEvents(Vec<FileEvent>)`;
- создать `WatchService` в `main()` (sender → proxy), хранить в `App`;
- `user_event`: `apply_file_events` → по NodeChange:
  ThumbStale → invalidate + `thumbs_failed.remove` + redraw;
  PathUpdated → mark_dirty + sync_dirs; Broken/Restored → redraw;
- `sync_dirs` вызывать: после загрузки канваса, после Drop-создания нод
  (T9), после удаления нод, после Rename-события (набор директорий
  мог измениться);
- тултип битой ноды: в `RedrawRequested`/hover-логике — если
  `hovered` нода broken → `ScreenText` с путём у курсора.

**Шаг 5 — приёмка `< 1 с`.** Замер руками: rename в Explorer → рамка
серая < 1 с (300 мс debounce + event batch). Если медленнее — уменьшать
окно нельзя (шквал), смотреть на путь события.

**Шаг 6 — финал.** fmt/clippy/test --workspace, ручной чек-лист, коммит
`feat(core,shell,app): T10 — файловый вотчер (notify, debounce, brokenLink)`.

## 6. Сводка тестов

| Тест | Модуль | Что покрывает |
|---|---|---|
| `normalize_path_*` | core | `\\?\`, разделители, case (windows) |
| `path_matches_*` | core | нода-владелец события |
| `apply_modify_*` / `apply_rename_*` / `apply_remove_restore_*` | core | три сценария SPEC §2 п.5 на модели |
| `apply_rename_dir_*` | core | переименование каталога → все вложенные ноды |
| `watcher_*` (tempdir, реальное время) | shell | create/modify/rename/remove/restore, debounce, NonRecursive |
| round-trip `.canvas` после rename | core | путь и brokenLink сохраняются |

Ручная приёмка: rename фото в Explorer → путь в .canvas обновился,
карточка на месте, тамбнейл жив; remove → серая рамка < 1 с, hover →
старый путь; вернуть файл по тому же пути → рамка обычная; правка
файла извне (сохранить из редактора) → тамбнейл обновился; 20 Modify
подряд (шквал сохранений) → один батч, без 20 перерисовок атласа.

## 7. Риски, ограничения, фолбэки

- **Корзина Explorer** — вне T10 (это move в `$Recycle.Bin`, notify видит
  Remove — сработает как битая ссылка; полноценное решение —
  SHChangeNotifyRegister в T16, RECIPES R12). В UI это выглядит
  корректно (файл исчез → broken), значит сценарий уже приемлем.
- **Rename через несмежные директории** (move из наблюдаемой папки в
  ненаблюдаемую): у notify on Windows пары приходят только при вотчере
  общего родителя или RenameMode::Both; иначе видно Remove+Create в
  разных папках → фолбэк: Remove → broken, Create по новому пути →
  новая нода НЕ создаётся (Create обрабатываем только для восстановления
  битых). Документируем как ограничение; полное решение — вотчинг
  родителя общего (нет в T10).
- **Сетевые диски**: notify (ReadDirectoryChangesW) на SMB работает
  нестабильно — ошибки вотчера → warn + деградация (нет обновлений),
  не падаем (R14).
- **Debounce 300 мс vs критерий `< 1 с`** — ок: 0.3 с окно + event
  loop пробуждение ~мгновенно, запас 3×.
- **Потоки**: поток-агрегатор один, sender-замыкание только шлёт
  `EventLoopProxy::send_event` (как ThumbService) — рендер-поток не
  блокируется (AGENTS).
- **Автосейв**: rename/remove — `mark_dirty`, штатный debounce 2 с
  (SPEC §9); перед выходом уже есть форс-сейв.
- **Пути в .canvas — относительные, если файл рядом с канвасом**
  (конвенция JSON Canvas, см. `resolve_file_path`): при Rename-апдейте
  `node.file` пишем относительный путь, если новый абсолютный — внутри
  каталога .canvas, иначе абсолютный. Чистая функция `relative_if_inside`
  в core + тесты (T9 дроп тоже её использует для единообразия).
