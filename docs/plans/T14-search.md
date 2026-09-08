# T14. Поиск (Ctrl+F, FTS5) — план (параллельная сессия с T13)

ЗАДАЧА: поиск по SPEC §5.2 / TASKS T14. Сессия идёт параллельно с T13 (миникарта);
файловые зоны воркеров не пересекаются. Владелец переупорядочил M3: T11/T12
отложены на период после релиза v1.0 → извлечение текста PDF недоступно до T11,
поиск индексирует имя + текст txt/md (см. §8).

## 1. Цель и критерии приёмки

- Ctrl+F — панель поиска поверх канваса (собственный оверлей: ScreenText +
  screen_instances; egui в стеке нет и не заводится).
- Индекс SQLite FTS5 по SPEC §5.2: `search_index` — имя файла + извлечённый текст
  (txt/md полностью, PDF/прочее — только имя до T11 после v1).
- Результаты списком; Enter/клик → камера плавно летит к ноде (300 мс, ease-out),
  нода подсвечивается пульсом (~1.2 с затухание).
- F3 / Shift+F3 — цикл по результатам (в т.ч. после закрытия панели).
- Критерий: поиск «смета» находит `смета_2026.xlsx`; переход плавный и точный.

## 2. Что уже есть (инвентарь)

- `canvas-shell/src/cache.rs` — паттерн SQLite (`ThumbCache::open`, bundled
  rusqlite 0.32; FTS5 доступен — проверено зондом: юникод-токенизация, bm25,
  префиксные запросы, экранирование кавычками).
- `canvas-shell/src/service.rs` — паттерн worker-сервиса (поток-владелец ресурса,
  канал команд, `EventLoopProxy` для пробуждения event loop).
- `canvas-render/src/text.rs` — `ScreenText`/`OverlayText` (оверлейный текст,
  лог. px); `cards.rs::CardInstance` — квады с fill/border/скруглением
  (screen_instances `FrameOverlay`) — из них собирается панель.
- `canvas-render/src/edit.rs` — `EditingSession` (многострочный редактор заметок);
  для однострочного поля поиска НЕ используется — свой лёгкий `SearchInput`.
- `canvas-render/src/camera.rs` — `Camera` (position = центр viewport; T14-B
  добавляет `set_center`/`set_zoom`).
- `canvas-app/src/main.rs` — `AppEvent`-протокол (proxy из shell-сервисов),
  хоткеи в `keyboard_input`, `mark_dirty`, `request_redraw`, drag/FS-события
  (T9/T10 — точки переиндексации).
- `canvas-core/src/fs_events.rs` — `path_matches` (сравнение путей, Windows ci).
- `canvas-core/src/model.rs` — `Node` (file с `file`-путём, text с содержимым).

## 3. Архитектура

**Индекс — `canvas-shell/src/search.rs` (T14-A), worker-поток.**
`SearchService::spawn(cache_dir, responder)` — поток-владелец `rusqlite::Connection`
(`~/.canvasdesk/cache.db`, таблица FTS5 `search_index(path UNINDEXED, display_name,
text)`; создание схемы идемпотентно). Команды через `mpsc::Sender<SearchCommand>`,
ответы — `responder: Arc<dyn Fn(SearchEvent) + Send + Sync>` (в main обёртка
отправляет `AppEvent::Search` через `EventLoopProxy`). Ошибка открытия БД —
деградация (warn, пустые результаты; индекс пересоздаваемый, SPEC §5.3).
Команды: `IndexFile`, `RemoveFile`, `ReplaceAll` (upsert всего набора file-нод,
удаление лишних записей — сценарий загрузки канваса), `Query`. Ответ: `Ready(Vec<SearchHit>)`
(на `Query`), `Indexed(usize)` (на ReplaceAll, для лога).

**Экранирование FTS** (проверено зондом): терм = фрагмент без пробелов; каждый
обёрнут `"..."` + префикс: `"смет" *` — ловит склонения (смету/смета) и составные
токены («смета_2026.xlsx» токенизируется как смета/2026/xlsx, регистронезависимо).
Термы склеиваются пробелом (AND). Пустой список термов → пустой результат БЕЗ
MATCH (пустой/битый MATCH — ошибка SQLite, обрабатывать как warn+пусто, не паника).
Ранжирование: `bm25(search_index)` ASC, LIMIT = параметр запроса.

**Извлечение текста** (в search.rs, worker-поток): `.txt`/`.md` — чтение ≤ 256 КиБ,
UTF-8 lossy; прочие расширения — пустой текст (только имя; PDF — после T11/v1,
см. §8). mtime не хранится: `IndexFile` — upsert, повторная индексация дешёвая.

**UI-модель — `canvas-render/src/search_ui.rs` (T14-B), чистая.**
`SearchInput` (query+каретка, insert/backspace/word-backspace/left/right/home/end);
`SearchPanel` (open, input, rows, selected, scroll_top; `set_results`,
`move_selection`, `ensure_selection_visible`, обработка Enter/Esc →
`PanelAction::Jump/Close`); `layout(window_w, window_h, &panel)` — геометрия панели
(топ-центр, ширина 460 кламп к окну−24, поле 36 px, строки 28 px, максимум 8 видимых
строк, скролл); `scan_scene(query, &[SceneEntry{node,title,text}]) -> Vec<usize>` —
in-memory substring-поиск (регистронезависимый) по заметкам и именам нод — для
нодов, которых нет в FTS (заметки без файла; файлы, ещё не проиндексированные).

**Анимация — `canvas-render/src/animate.rs` (T14-B), чистая.**
`Flight::new(start_center, start_zoom, target_center, target_zoom, 300 мс)` +
`sample(elapsed)` (ease-out cubic, интерполяция центра и зума), `is_finished`;
`pulse_alpha(elapsed)` — затухание 1→0 за 1200 мс (после — 0). `Camera` получает
`set_center(Vec2)` / `set_zoom(f32)` (кламп) — мелкая правка camera.rs, зона T14-B.

**Интеграция — T14-C (координатор, main.rs).**
Хоткеи: Ctrl+F — открыть/фокус (выделить прошлый запрос), Esc — закрыть,
Enter — прыжок к выбранной строке, Up/Down — выбор, F3/Shift+F3 — цикл (панель
закрыта — цикл по последним результатам с прыжком). Ввод текста — пока панель
открыта, символьные события идут в `SearchInput` (не в канвас), Esc в поле —
закрыть панель. Запрос: debounce 200 мс после правки → `SearchService::command(Query)`;
ответ `AppEvent::Search(Ready(hits))` → склейка с `scan_scene` (FTS-хиты первыми,
затем заметки/имена, дедуп по ноде) → `set_results` + перерисовка. Прыжок:
`Flight` к центру ноды, целевой зум = max(текущий, 0.8); каждый кадр
`camera.set_center/set_zoom(sample)`, по завершении — стоп; пульс —
`FrameOverlay.instances` с border-цветом alpha = `pulse_alpha` (CardInstance с
выбранным радиусом, заливка прозрачная). Индексация: при старте канваса
`ReplaceAll`; при дропе файлов (T9) — `IndexFile`; при FS-событиях (T10):
Create/Modify → `IndexFile`, Rename → Remove+Index (пути новые), Remove →
`RemoveFile` — в `on_file_events` вместе с существующей обработкой.

## 4. Мэппинг событий → действия

| Событие | Действие |
|---|---|
| Ctrl+F | панель открыта, фокус, прошлый запрос выделен |
| Печать в поле | `SearchInput::insert`, debounce 200 мс → Query |
| `AppEvent::Search(Ready)` | rows = FTS + `scan_scene`, дедуп, `set_results`, redraw |
| Enter / клик по строке | Flight к ноде (300 мс, zoom≥0.8) + пульс |
| F3 / Shift+F3 | selected += 1 / −1 (wrap), прыжок; работает и при закрытой панели |
| Esc | закрыть панель (результаты сохраняются для F3) |
| ЛКМ в панели | панель рисуется поверх; клики в rect панели не попадают в канвас-обработчики |
| Загрузка канваса / дроп / FS-события | `ReplaceAll` / `IndexFile` / Remove+Index / `RemoveFile` |

## 5. Пошаговый план (TDD)

Контракты засеяны координатором: `search.rs` (shell), `search_ui.rs`, `animate.rs`
(render) + `set_center/set_zoom` в camera.rs + `mod`-строки в lib.rs обоих крейтов.
Воркеры реализуют тела и тесты, НЕ меняя публичных сигнатур и lib.rs.

1. **T14-A** (`canvas-shell/src/search.rs`, только этот файл):
   - worker-поток: `Connection::open(cache.db)` + FTS5-схема; цикл `recv()`:
     IndexFile (извлечение текста + INSERT OR REPLACE… точнее DELETE+INSERT, т.к.
     FTS5 не умеет OR REPLACE — проверить и зафиксировать), RemoveFile (DELETE
     по path), ReplaceAll (transaction: удалить отсутствующие, upsert все),
     Query (экранирование → MATCH … ORDER BY bm25 LIMIT → Vec<SearchHit>).
   - ответы через responder; send-ошибки — молча (прототип как у watcher).
   - Тесты: tempdir-БД — create schema, upsert, query точный/префикс/несколько
     термов AND, экранирование спецсимволов (`"(` и пустой ввод — без паники,
     пустой результат), bm25-порядок (попадание в имя раньше попадания в текст
     — при равном bm25 не гарантируется, тестировать только «обе строки найдены»),
     RemoveFile, ReplaceAll (очистка лишних), лимит строк, кириллица,
     extraction: .md читается, .xlsx — пустой текст, файл 1 МиБ — усечён до 256 КиБ.
2. **T14-B** (`search_ui.rs`, `animate.rs`, `camera.rs` — только эти три):
   - `SearchInput` + `SearchPanel` + `PanelAction` + `layout` + `scan_scene`
     по контрактам §3; `Flight::sample`/`ease_out_cubic`/`pulse_alpha`;
     `Camera::set_center/set_zoom` (кламп MIN/MAX_ZOOM).
   - Тесты: input (вставка/bs/word-bs/стрелки/home/end, кириллица, UTF-8
     boundaries — каретка не рвёт многобайтный символ), panel (selection
     wrap, ensure_visible при 8+ строках, Enter/Esc → Action), layout (клампы,
     центр, высота по числу строк), scan_scene (substring, регистр, заметки,
     пустой запрос), flight (t=0 → старт, t=∞ → цель, монотонность, зум-кламп
     не здесь — камера клампит), pulse (0 при переполнении, 1 при 0, убывание),
     camera set_* (кламп).
3. **T14-C (координатор):** интеграция main.rs по §3/§4, гейты workspace, §8,
   коммит `feat(shell,render,app): T14 — поиск (FTS5, полёт камеры, пульс)`.

## 6. Сводка тестов

- canvas-shell (`cargo test -p canvas-shell`): search-модуль — §5.1 (≈ 12–14
  тестов на tempdir, кириллица обязательна).
- canvas-render (`cargo test -p canvas-render`): search_ui + animate + camera —
  §5.2 (≈ 18–22 теста).
- Интеграционно-ручная приёмка (владелец): критерий «смета» → смета_2026.xlsx,
  плавный переход, пульс, F3-цикл.

## 7. Риски, ограничения, фолбэки

- **FTS5 в bundled rusqlite** — проверено зондом (создание таблицы, MATCH,
  bm25, префиксы, экранирование; кириллица токенизируется). Ошибка рантайма
  MATCH (битый синтаксис) — недостижима при экранировании, но обрабатывается
  как warn + пусто.
- **Блокировка БД:** Connection принадлежит одному worker-потоку; конкурентных
  читателей нет; ReplaceAll — транзакция. Рост индекса ограничен набором нод
  канваса (256 КиБ × N файлов — приемлемо; кэш пересоздаваемый).
- **Рендер-поток не блокируется:** извлечение текста и запросы — в worker;
  ответ приходит событием. Debounce 200 мс гасит шквал запросов при печати.
- **Панель и канвас-хоткеи:** при открытой панели winit-события клавиатуры
  маршрутизируются в панель первыми (F3/HUD-переключения приглушаются).
- **Path→node матчинг:** FTS-хит по пути ищется в `canvas.nodes` через
  `path_matches` (fs_events, регистронезависимо на Windows); не найдено
  (файл не на канвасе) — строка не показывается.
- **Конфликт зон:** T14-B не правит `renderer.rs` (зона T13-B); lib.rs засеян
  координатором и не правится воркерами.

## 8. Отступления при реализации (сессия T14)

1. **PDF-текст отложен вместе с T11** до периода после v1.0 (решение владельца,
   зафиксировано в TASKS.md): индексируются txt/md (≤ 256 КиБ, lossy) + имена
   всех файлов; `extract_text` для прочих расширений — пустой текст.
2. **egui не заводился** (как и задумано в §3): панель — CardInstance-квады +
   ScreenText через FrameOverlay; поле ввода — лёгкий SearchInput, не
   EditingSession.
3. **UPSERT в FTS5** реализован как DELETE + INSERT в транзакции: INSERT OR
   REPLACE над виртуальной таблицей FTS5 НЕ является upsert — вставляет
   дубликат (выяснено дополнительным зондом T14-A).
4. **busy_timeout 5000 мс** на Connection: cache.db пишут на двоих ThumbCache и
   поиск — короткие писатели гасятся ожиданием, а не потерей записи
   (journal_mode — defaults, как в cache.rs).
5. **ReplaceAll — полный пересбор** (DELETE всех + INSERT) вместо вычитания
   «чужих» путей: объём = числу file-нод канваса, I/O вынесен за транзакцию.
6. **F3-конфликт с HUD (T5) разрешён приоритетом**: есть результаты поиска —
   F3/Shift+F3 цикл + прыжок (и при закрытой панели, rows сохранены); нет
   результатов — прежнее переключение HUD.
7. **Повторное Ctrl+F** (панель уже открыта) очищает поле запроса; первое
   открытие сохраняет прошлый запрос (выделение всего текста не реализовано —
   у SearchInput нет selection, MVP).
8. **Клавиатурный фокус панели абсолютен**: при открытой панели ВСЕ нажатия
   идут в панель (канвас-хоткеи, включая Space-пан, приглушены); открытие
   панели фиксирует активное редактирование заметки.
9. **Debounce 200 мс + анимации держат цикл** через request_redraw из
   about_to_wait: ControlFlow::Wait уснул бы до следующего события, и запрос
   не ушел бы без кадров-«будильников» (панель открыта — кадры дешёвые).
10. **Пульс — цвето-затухающая оверлейная рамка** (border.a = pulse_alpha,
    заливка прозрачна) вокруг ноды с расширением к концу. Для этого исправлен
    cards.wgsl: ранее шейдер игнорировал fill.a/border.a (латентный баг —
    дроп-призраки T9 (a=0.10) и зона дропа (заливка [0,0,0,0]) рисовались
    непрозрачными без рамки). Рамки выделения/битой ссылки не изменены
    (a=1). Цвета меню/настроек теперь честно полупрозрачны, как задумано.
11. **Целевой зум прыжка** = max(текущий, 0.8) — SPEC не задаёт зум прыжка;
    0.8 выбран, чтобы нода была читаема после прилёта.
12. **Индексация по FS-событиям** фильтруется путями нод канваса
    (path_matches): чужие файлы в наблюдаемых папках в индекс не попадают;
    rename → RemoveFile(from) + IndexFile(to).
13. **Кросс-проверка windows-таргета для canvas-app/shell невозможна из
    песочницы** (libsqlite3-sys bundled требует MSVC-кросс-компилятор C);
    authoritative-гейт — CI windows-latest, как в T9/T10.
