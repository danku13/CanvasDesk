## 2026-09-19 — W9 + W8 (M8 wasm-порт, трек B): шаблоны и Numi-формулы в браузере — приёмка смоук-оракулами

- **Задача (§6, топопорядок после W7):** W9 «Шаблоны (FR-018/020)» —
  `include_dir` c template.json, панель Ctrl+P, wheel-меню; приёмка:
  вставка шаблонной группы на web. W8 «Numi-формулы (FR-013/014)» —
  прогон expr/flow на web-сцене, фикс падений; приёмка: calc-строки
  считают, поток значений живой, бейджи ошибок. Обе задачи — S; одна
  сессия, ветка feature/wasm-w9-templates от main 58e929b, два коммита
  (W9, W8), merge --no-ff.
- **Разведка:** реестр шаблонов — чистый код canvas-core
  (`TemplateRegistry::builtin` из `include_dir`) + скан custom-каталога
  через `std::fs` (в W3 уже подтянут под wasm32 — `custom()` возвращает
  пустой набор при `Err`); UI (палитра FR-024/025, wheel-меню, вставка)
  — платформенно-нейтральный canvas-app. Numi — чистые expr/flow.
  Ожидалось «работает как есть» — потребовалась приёмка и два fix'а
  web-слоя (см. ниже).
- **Сделано (W9):**
  - DEBUG-оракулы дыма (по образцу W7): в `on_key` Ctrl+P-ветка —
    «шаблонная палитра: док открыт/сфокусирован templates=N
    categories=M»; в `instantiate_template_at` — «шаблон вставлен
    template=<id> node=<id>» (canvas-app; на нативе под дефолтным
    фильтром не видны, на web — оракулы `scripts/web_smoke.py`).
  - Шим Ctrl+P в `index.html` (M8/W9, web-слой §3.1): гасит
    браузерный акселератор печати в фазе перехвата. Найдено дымом:
    winit-web НЕ preventDefault'ит браузерные связки — палитра
    открывалась, но Chromium параллельно поднимал печать, rAF-насос
    winit приостанавливался и весь ввод умирал (мышь/клавиатура
    безответны, кадры не презентуются, pageerror=0 — маскировка под
    «зависший модуль»). preventDefault не мешает propagation — до
    App событие доходит.
  - Приёмка-цепочка: Ctrl+P → «шаблонная палитра» templates=45
    categories=4; Enter → «шаблон вставлен template=com.canvasdesk.
    api-gateway» (instantiate + fit_template_node_height +
    recompute_flow); Shift+клик по пустому → wheel-меню (оракул
    «wheel-меню шаблонов: категории categories=4»).
  - clippy-фикс: unused `use canvas_core::CanvasStorage as _;` в
    нативных тестах fs_access.rs/opfs.rs (гейт clippy -D warnings был
    красный на main — импорт дублировался из `super::*`).
- **Сделано (W8):**
  - DEBUG-оракул в `SceneState::recompute_flow` (canvas-scene):
    «пересчёт потока: значения вычислены values=N errors=M lines=K».
  - Сцены дыма 5c: calc-строка «кв = 5» → values=1, lines=1
    (кириллический идентификатор — грамматика FR-013 Unicode-совместима);
    «2 +» → errors=1 (бейдж ошибки; hit-test тултипа — нативный тест
    `expr_error_tooltip_hit_test`). Падений expr/flow на web не
    найдено — фикс не потребовался.
  - Поток значений по рёбрам — тот же чистый `propagate_with_lines`:
    верифицирован wasip1-тестами гейта + MCP e2e-оракулом ±1%;
    вставленный шаблон считает $param-лист на web-сцене.
- **Деградации волны 1 (задокументированы, не блокеры):**
  - FR-020 custom-шаблоны: `std::fs` под wasm32-unknown-unknown возвращает
    `Err(Unsupported)` (не панику) — custom-скан пуст, «Сохранить как
    шаблон» честно показывает toast об ошибке. OPFS-хранилище custom —
    волна 2 (§9).
  - SwiftShader-артефакт дыма (не падение): первый кадр палитры бьёт
    mappedAtCreation-лимит (32768 > 4KiB) — необработанное исключение
    разрывает rAF-насос winit. Поэтому палитра проверяется атомарно
    последней секцией дыма (аккорд Ctrl+P + Enter одним evaluate —
    обработчики ключей успевают до ломкого кадра), а wheel-меню — до
    палитры. На аппаратном WebGPU (продуктовый таргет) ограничения нет;
    ср. W7 (там тот же артефакт на прыжке поиска, 8192 байта).
- **Гейты:** fmt ✓; clippy --workspace --all-targets -D warnings ✓
  (натив); cargo test --workspace — 45 наборов, 0 failed; check wasm32
  canvas-web/core/render/widgets/scene/mcp-headless ✓; wasm_gate.sh
  (345 core + 13 mcp в wasmtime) ✓; mcp_wasm_gate.sh (78 wasip1 +
  e2e-оракул ±1%) ✓; trunk build (trunk 0.21.14 + wasm-bindgen-cli
  0.2.127) ✓; браузерный дым web_smoke.py — SMOKE OK (все оракулы
  W4–W8 без регресса).
- **Дальше:** W11 (виджеты снапшотом) и W10 (превью картинок) —
  параллелизуемы (зона canvas-web); W12 — финализатор (полировка/
  деплой/CI). Отступление от «1 задача = 1 сессия» осознанное: обе
  задачи S, общая smoke-инфраструктура.

## 2026-09-19 — W6 (M8 wasm-порт, трек A): хранение в браузере — OPFS + FS Access + IndexedDB-recent + DOM-drop + ?canvas= + экспорт blob

- **Задача (§6, после W5):** «Хранение (§4)»: `CanvasStorage` —
  FsAccessStorage + OpfsStorage + выбор при старте; IndexedDB-recent;
  DOM-drop приём файлов; `?canvas=`/`?stress=` URL-параметры; автосейв +
  `.bak`. Приёмка: открыть с диска → править → автосейв в файл → F5 →
  reopen из recent без пикера; OPFS-дефолт работает без диска; экспорт
  blob. Ветка feature/wasm-w6-storage от main af05c5c.
- **Разведка:** трейт `CanvasStorage` синхронный (W3), браузер даёт только
  async — мост двухуровневый: чистый `MirrorStore` (зеркало путь→текст +
  очередь записей, тестируется нативно) + фоновый `spawn_local`-сброс в
  OPFS (fire-and-forget, ошибки — в консоль). Автосейв уже в App
  (`autosave_if_due`, debounce 2 с) — `save()` трейта честно доезжает до
  web-хранилища. Смена активной сцены в рантайме не существовала —
  добавлена (см. OpenScene).
- **Сделано:**
  - **`opfs.rs`** — OPFS-хранилище (§4.1, фолбэк/дефолт): `MirrorStore`
    (save_text ставит пару `.bak`-прежний + новый в очередь; seed_text —
    зеркало без очереди; load_text — NotFound как MemStorage);
    `OpfsStorage` за трейтом `CanvasStorage` (load из зеркала мгновенно,
    save — сериализация + drain); async-примитивы `opfs_root`/
    `read_opfs_text` (NotFoundError → None)/`write_opfs_text`
    (get_file_handle {create:true} → createWritable → write → close —
    close через unchecked_ref::<WritableStream>, биндинга на наследнике
    нет); `init_scene` — выбор `?canvas=` → недавний → `default.canvas`,
    файл есть → parse (битый — сеем поверх, как натив load_or_seed),
    нет → сеем; отказ OPFS целиком — деградация в MemStorage (страница
    открывается всегда; перезапись при read-ошибке запрещена — защита
    данных).
  - **`fs_access.rs`** — FS Access (§4.1, основной путь): пикер
    `showOpenFilePicker` c accept-фильтром .canvas (жест кнопки), permis-
    sion query→request readwrite, `FsAccessStorage` (зеркало + фоновый
    автосейв через хэндл; версия-назад — в OPFS: браузер не отдаёт
    родительский каталог файла-хэндла, sibling невозможен — отклонение
    от §4.2 п.3 задокументировано); отказ разрешения — «Открыть копию» в
    OPFS (текст уже прочитан, чтение разрешения не требует);
    `reopen_recent` — хэндл из IndexedDB → requestPermission (жест) →
    диск, иначе OPFS-копия.
  - **`recent.rs` + `js_glue.rs` + index.html** — IndexedDB-недавние:
    база `canvasdesk`, сторы `recent` (keyPath name, {name, ts}) и
    `handles` (name → FileSystemFileHandle); бойлерплейт open/upgrade —
    в JS-глю `window.__canvasdesk` (index.html), Rust-мост — `js_glue`
    (Promise → JsFuture), выбор верхней записи — чистая `recent_top_of`
    (нативные тесты).
  - **`drop_files.rs`** — DOM-drop: dragover глушит preventDefault'ом
    (иначе браузер уводит дроп в ОС), drop → первый .canvas →
    `import_to_opfs` (санитизация имени, OPFS-запись, недавние,
    OpenScene с OPFS-хранилищем); не-.canvas — info-лог (W10). Превью
    дропа (призраки T9) на web невозможно — данные файлов браузер отдаёт
    только в момент drop (деградация, §4.2).
  - **`export.rs`** — экспорт download-blob: активный канвас читается из
    ХРАНИЛИЩА (диск-хэндл или OPFS-файл), не из живой сцены — второй
    канал состояния сознательно не заводим; отставание ≤ debounce 2 с.
    Blob(application/json) → objectURL → `<a download>` → revoke.
  - **`toolbar.rs` + index.html** — DOM-панель хранилища («Открыть с
    диска…» / «Недавние: <имя>» / «Экспорт .canvas»), листенеры из Rust
    (Closure::forget — singleton), подпись недавних синхронится из
    web_state. Стиль — минимальный, полировка W12.
  - **`web_state.rs`** — thread_local web-оболочки: активный канвас
    (имя+тип), дисковый хэндл, общее `Arc<OpfsStorage>` (весь web-код —
    главный поток, JsValue-типы не трогают Send+Sync-трейты).
  - **`url_params.rs`** — `?canvas=имя`: `sanitize_canvas_name` (≤80
    символов; без `/` `\` `..` и управляющих; суффикс .canvas
    дописывается; кириллица разрешена; опасное имя — мягкий None),
    percent-декод значения (браузер кодирует кириллицу в
    location.search; байтовые срезы — без паник на мультибайте); +4
    теста (12 в url_params).
  - **canvas-app: `AppEvent::OpenScene {path, json, storage}`** —
    платформенно-нейтральная смена активной сцены: форс-сохранение
    прежней (если dirty) → парсинг (битый — тост, сцена живёт) →
    подмена `SceneState::with_storage` (None — текущее хранилище,
    Some — новое, диск-хэндл) → сброс переходного UI (селекция/drag/
    редактор/поиск/миникарта/полёт — индексы старой сцены несовместимы)
    → камера дефолт старта. +4 теста (161 в app: замена/сброс, битый
    json, флаш dirty в СТАРОЕ хранилище, явное хранилище).
  - **canvas-app: `about_to_wait` — будка автосейва** — пока сцена
    грязная, request_redraw держит rAF-цепочку web-цикла живой: без
    этого после коммита тишина → about_to_wait не вызывается → debounce
    2 с никогда не срабатывает (инструментально доказано пробой).
    Натив: пара лишних кадров за 2 с после правки — поведение то же.
  - **`app_spawn.rs`** — старт переехал в async (`spawn_desk_web`):
    init_scene (OPFS/недавние/сеяние) ДО построения App; `?stress` —
    MemStorage (не писать OPFS/recent); конфиг — TOML из localStorage
    `canvasdesk.config` (read-only, клампы CR-003/FR-028; запись — W12);
    подключение toolbar+drop; native-путь rlib-тестов — прежний
    синхронный (`spawn_desk_native`).
- **Диагностика по пути:** (1) запись OPFS без `{create:true}` —
  NotFoundError замаскирован под «сеется, но не пишется» (зеркало
  скрывало отсутствие I/O — вскрывается только консольными оракулами);
  (2) navigator.storage недоступен на about:blank — probe-скрипты
  проверять только на странице приложения (secure context localhost);
  (3) rAF-засыпание цикла (см. будку автосейва).
- **Приёмка (smoke, 25 PASS / 8 новых W6):** первый запуск —
  «сеется новый канвас» + рендер ✓; правка («w6 автосейв») → «канвас
  сохранён» + OPFS: `default.canvas.bak` (прежняя версия) + `default.canvas`
  (новая) ✓; F5 — «стартовый канвас из недавних» + «канвас загружен из
  OPFS» без пикера ✓; `?canvas=w6-имя` (кириллица, percent-декод) —
  «стартовый канвас из URL» + сеяние именованного ✓; W5/W7-набор
  (редактор, кириллица, стресс 5000 @ rAF-fps 61, поиск rows=625,
  `?stress=abc`) не сломан ✓; pageerror 0 ✓. «Открыть с диска»/экспорт —
  ручная приёмка (пикер/скачивание в headless не автоматизируются);
  путь автосейва на диск покрыт тем же drain-контрактом, что и OPFS.
- **Гейты:** fmt ✓; clippy -D warnings (canvas-app + canvas-web,
  натив + wasm32) ✓; test --workspace 0 failed (canvas-app 161,
  canvas-web 36); wasm_gate.sh ✓; mcp_wasm_gate.sh ✓ (e2e сошлась);
  check wasm32 canvas-web ✓; trunk build ✓; web_smoke.py SMOKE OK.
- **Дальше:** трек A свободен — ребаланс §6.1: верхняя незанятая — W8
  (Numi, S) или W9 (шаблоны, S); W10 (превью, M) разблокирован W6;
  W11 (виджеты, M); W12 закрывает финализатор.

## 2026-09-19 — W7 (M8 wasm-порт, трек B): поиск + миникарта в браузере — ?log=debug, DEBUG-оракулы, усиленный smoke

- **Задача (§6):** трек B (после W5-трек A): «Поиск + миникарта» —
  MemSearch-индекс по нодам, debounce 200 мс, UI поиска/миникарта из
  render без изменений. Приёмка: Ctrl+F по 5000-нод сцене, переход/
  подсветка результата. Ветка feature/wasm-w7-search от main 0569e6b.
- **Разведка:** цепочка поиска на web уже собрана W3/W4: MemSearch за
  трейтом SearchBackend, ответы через AppEvent::Search (EventLoopProxy на
  web — mpsc + Waker, работает), поиск по заголовкам/телам нод —
  платформенно-нейтральный scan_scene в apply_search_hits (§3.2
  «scan_scene уже есть»), миникарта — общий GPU-проход canvas-render.
  Дебаунс 200 мс (about_to_wait + request_redraw — цикл самоподдерживается
  на web через rAF-回调 request_redraw, проверено инструментально). W7 —
  наблюдаемость и приёмка (слепые зоны дыма W5: редактор/панель не видны).
- **Сделано:**
  - **`?log=debug|trace|warn|error`** — уровень консоли из URL:
    `url_params::LogLevel` (мягкий парсинг — неуровень → тихий INFO,
    не роняет `?stress`; +3 теста), `web_log::init_tracing_with` (+1
    тест), boot() читает параметры ДО инициализации трейсинга (баннер
    W4→W5). Диагностика на web — прямой аналог RUST_LOG (W12 может
    расширить), снял слепоту DEBUG-канала для приёмки.
  - **DEBUG-оракулы:** ответ backend'а логируется в респондере
    canvas-web («поисковый backend ответил hits=N» — доказательство круга
    Query → MemSearch → SearchEvent → proxy); итог склейки FTS+scan_scene
    — в `apply_search_hits` canvas-app («поиск завершён rows=N» — DEBUG,
    нативно невидим, нулевой регресс).
  - **`scripts/web_smoke.py`** (замена точечного дыма W5, переносим):
    синтетические `KeyboardEvent` для кириллицы — Playwright
    `keyboard.type()` шлёт insertText БЕЗ keydown для вне-US-символов,
    winit-web их не видит (IME на web в winit 0.30 нет); реальная
    RU-клавиатура даёт keydown с key="ф" — диспатч KeyboardEvent точно
    воспроизводит контракт браузера; фильтр известного SwiftShader-
    артефакта (mappedAtCreation 8192 в glyphon — см. ниже).
- **Диагностика по пути (инструментально, по исходникам winit 0.30.13):**
  «умирание клавиатуры после Ctrl+F» оказалось иллюзией — Playwright
  type() не даёт keydown для кириллицы; панель поиска честно открывается
  и глотает клавиши (доказано курсором: Space+ЛКМ до Ctrl+F → grabbing,
  при открытой панели → пусто, после Esc → grabbing). ControlFlow::Wait на
  web не планирует тик — цикл живёт через request_redraw-rAF (дебаунс
  сходится, проверено tick-пробой — временные пробы удалены).
- **Приёмка (smoke, 16 PASS):** Ctrl+F по `?stress=5000&log=debug` —
  запрос «смета» → rows=625 → Enter (прыжок/полёт) ✓; сквозной кейс —
  dblclick → «привет мир» (синтетическая кириллица) → коммит → Ctrl+F →
  «привет» → rows=1 ✓ (текст дошёл до модели и ищется); MemSearch-roundtrip
  ✓; W5-набор (фокус CANVAS, онбординг-гейт, cursor=text/grabbing,
  копипаст WebClipboard) не сломан ✓; `?stress=abc` — деградация ✓;
  rAF-fps = 61 ✓. Известный артефакт среды (задокументирован в дыме,
  не падение): SwiftShader отвергает mappedAtCreation=8192 (glyphon
  create_oversized_buffer) на прыжке — продуктовый таргет (аппаратный
  WebGPU, Chromium 113+) ограничения не имеет.
- **Гейты:** fmt ✓; clippy --workspace --all-targets -D warnings ✓;
  test --workspace 0 failed (canvas-web 24: url_params 9, web_log 4);
  wasm_gate.sh ✓; mcp_wasm_gate.sh ✓ (e2e-сессия сошлась); check wasm32
  canvas-web ✓; trunk build ✓.
- **Дальше:** трек B — W9 шаблоны (S) → W8 Numi (S) → W11 виджеты (M);
  трек A — W6 хранение (L); W10 после W6; W12 финализатор.

## 2026-09-19 — W5 (M8 wasm-порт, трек A): ввод и редактирование в браузере — фокус, буфер обмена (navigator.clipboard), ?stress URL-параметры

- **Задача (§6):** трек A после W3/W4: «Ввод и редактирование» — клавиатура
  (map_key), двойной клик, drag нод, инлайн-edit, палитра, resize, undo,
  темы, F1. Приёмка: кириллица печатается/выделяется; Ctrl+B/I/H,
  Ctrl+Z/Y, Ctrl+C/X/V; `?stress=5000` — 60 fps. Ветка
  feature/wasm-w5-input-editing от main ce69af2 (fetch — origin не ушёл).
- **Разведка:** ввод/редактирование — общий код App (W2/W3), платформенных
  веток не требует; по исходникам winit 0.30.13 (web-бэкенд) найдены три
  web-пробела: (1) фокус — winit фокусирует canvas при create_window
  (with_active), но канвас тогда НЕ в DOM (with_append выключен; вставка —
  платформенный слой) → focus() на оторванном элементе теряется, keydown
  (листенеры на канвасе) мертвы до первого клика; (2) буфер — NoopClipboard;
  (3) стресс — CLI-флаги нативной обёртки недоступны на web.
- **Сделано (всё в canvas-web — правило §3.1):**
  - **фокус:** `renderer_launch` — `window.focus_window()` сразу после
    attach_canvas_to_dom (canvas.focus() → FocusEvent → Focused(true) →
    has_focus; winit сам разруливает «фокус до регистрации листенера»
    через active_element-проверку).
  - **`web_clipboard.rs`:** `WebClipboard` за трейтом `ClipboardBackend`
    (паттерн W3-сервисов, инъекция в App::new вместо NoopClipboard):
    синхронный контракт через кэш (Rc<RefCell>) + системный буфер
    `navigator.clipboard` — set_text: кэш сразу, writeText Promise
    fire-and-forget (spawn_local, ошибки warn — user activation есть от
    Ctrl+C/X); get_text: кэш сразу, фоновый readText обновляет кэш к
    следующему Ctrl+V. Ограничение волны 1 задокументировано в шапке
    модуля: DOM-`paste` с синхронным clipboardData не используем — winit
    web prevent_default (дефолт) гасит его preventDefault'ом на keydown;
    внешнее копирование подтягивается следующим жестом. Натив (rlib-тесты):
    кэш-only, 4 теста (roundtrip, None до set, замена, трейт-объект).
  - **`url_params.rs`:** `parse_query` — чистая функция над строкой
    запроса (без web-sys, 6 нативных тестов: stress, оба параметра,
    неизвестные ключи, пустые пары, битые числа → Err, пустой запрос) +
    wasm-обвязка `read_params()` (location.search; Err → warn + дефолт).
    `app_spawn`: `?stress=N` → `SceneState::with_storage(stress_canvas(n),
    stress.canvas, …)` (зеркало main.rs, автосейв не затирает
    default.canvas), `?stress-widgets=N` → add_stress_widgets + mark_dirty.
  - web-sys +3 фичи (Clipboard/Navigator/Location); ноль новых крейтов
    (js_sys::Promise используется через transitive-типы, сам крейт не
    нужен). Cargo.lock не изменился.
  - **`scripts/web_smoke.py`** — воспроизводимый браузерный дым приёмки
    (Playwright + Chromium+swiftshader `--enable-unsafe-webgpu
    --use-angle=swiftshader --enable-features=Vulkan`; WEB_SMOKE_URL).
- **Приёмка (браузерный дым, оракул — консоль/DOM/rAF; скриншоты WebGPU-
  канваса в headless не снимаются — ограничение из W4):** фокус
  `document.activeElement=CANVAS` ✓; негативный контроль — онбординг
  (FR-028, показывается на первом запуске) глушит ввод ✓; Esc «Пропустить»
  доходит до App ✓; dblclick → инлайн-редактор (cursor=text через
  sync_cursor_icon) ✓; кириллица «привет» + Shift+Arrow + Ctrl+B/A/Z/C/V
  — ноль pageerror ✓; Enter коммитит (cursor обратно) ✓; Space+ЛКМ →
  cursor=grabbing ✓ (клавиши до App; нюанс: при space_pressed
  on_left_button уходит в ранний return — курсор синкается в
  on_cursor_moved, в дыме жест с движением, как в реальности);
  `?stress=5000` — сеется (лог nodes=5000), рендер живёт, rAF-fps = 60 ✓;
  `?stress=abc` — warn «битый URL-параметр» + обычный запуск ✓.
- **Гейты:** fmt ✓; clippy --workspace --all-targets -D warnings ✓;
  test --workspace — 0 failed (canvas-web: 20 тестов — каркас 13 + W5 11
  с учётом app_spawn); wasm_gate.sh — полный ✓; mcp_wasm_gate.sh —
  полный ✓ (e2e-сессия сошлась); cargo check wasm32 canvas-web ✓;
  trunk build ✓. Нюанс окружения: trunk 0.21.14 падает на NO_COLOR=1
  (парсит как --no-color=1) — в этом контейнере вызывать с env -u
  NO_COLOR; rustc 1.98.1 = версия плана §2.
- **Дальше:** трек A — W6 хранение (L: FS Access + OPFS, IndexedDB-recent,
  DOM-drop, `?canvas=`); трек B — W7 поиск+миникарта, W9 шаблоны, W8 Numi,
  W11 виджеты; W10 после W6; W12 финализатор.

## 2026-09-19 — W4-прошивка (M8 wasm-порт, трек B): первый свет canvas-web — App в браузере, async-init Renderer, WebGPU

- **Задача (§6.1):** трек B, шаг после W4-каркаса (1056d89) и слитого
  W3 трека A (af0219a — «W4-прошивка разблокирована»): «Первый свет
  canvas-web» — spawn_app, async-init Renderer через spawn_local, пустая
  сцена, зум/пан/сетка, HUD F3. Ветка feature/wasm-w4-proshivka от main
  af0219a (fetch перед стартом — origin не ушёл, гонки нет).
- **Async-init Renderer за инъекцией (§3.4, паттерн W3-сервисов):**
  - `canvas-render::renderer_init` (новый модуль): `RendererLauncher`
    (launch(window, prefer_dx12) -> RendererLaunch), варианты
    `Ready(Box<Renderer>)`/`Pending(RendererSlot)`/`Failed(anyhow)`,
    `RendererSlot` — Rc-RefCell одноразовый слот (главный поток:
    web-футура spawn_local и кадр-цикл не пересекаются по потокам);
    `BlockOnRendererLaunch` — pollster::block_on (натив, поведение как
    до W4); `NoopRendererLaunch` — заглушка для тестов.
  - canvas-app: `App::new` + параметр `renderer_launcher`;
    `resumed()` зовёт `launch` (натив — Ready/Failed синхронно, web —
    Pending+слот); тело Ok-ветки выделено в `install_renderer`;
    RedrawRequested забирает слот `and_then(RendererSlot::take)` ДО
    отрисовки — кадры до готовности GPU пропускаются (renderer None,
    R14), натив-ветка мертва (слот всегда None). pollster убран из
    deps canvas-app (переехал в canvas-render).
  - canvas-web `renderer_launch::SpawnLocalRendererLaunch`:
    spawn_local(Renderer::new) → put(слот) → window.request_redraw();
    вставка winit-канваса в DOM (WindowExtWebSys::canvas + append,
    идемпотентно — winit 0.30 `with_append` по умолчанию выключен,
    attrs строятся в canvas-app, §3.1: web-знания туда не идут).
- **canvas-web `app_spawn` (зеркало нативного main.rs):**
  install_measured_reserve (тот же хук) → SceneState::
  load_or_seed_with_storage(default.canvas, MemStorage) →
  EventLoop<AppEvent> + spawn_app (EventLoopExtWebSys) → App::new с
  web-набором (карта §3.2): NoopThumbs (W10), NoopWatch, MemSearch с
  proxy-ответами (AppEvent::Search), NoopClipboard (W5+),
  widget_state None (W11), Settings::default (W6), SpawnLocalRenderer-
  Launch. `start()` = boot() + spawn_desk(); нативные тесты каркаса не
  зовут spawn (EventLoop требует JS-рунтайм/дисплей).
- **Web-compat шимы (§7 «WebGPU-драйверные баги», web-слой, wgpu не
  патчится):** (1) `--cfg=web_sys_unstable_apis` для
  wasm32-unknown-unknown в .cargo/config.toml — стандартное требование
  wgpu-web; (2) index.html-шим: GPUAdapter.prototype.requestDevice
  фильтрует requiredLimits по именам adapter.limits (for..in —
  WebIDL-геттеры) — wgpu 22 требует удалённый из WebGPU лимит
  maxInterStageShaderComponents (переименование), свежие Chromium
  отклоняли весь requestDevice («The limit … is not recognized») —
  маскировалось под «GPU-адаптер не найден» (GpuContext::new возвращает
  None и при ошибке device). Найдено перехватом requestAdapter/
  requestDevice в консоли.
- **Точечные правки ядра:** `Renderer::new` перечитывает размер ПОСЛЕ
  async-ожиданий (было: читал до → на web 0×0, Resized приходил при
  renderer=None и пропускался → canvas 300×150; нативу не вредит —
  block_on в том же кадре); `web_log` ConsoleLayer печатает поля
  событий key=value-суффиксами (раньше терялись — диагностика web-ошибок
  без полей невозможна); метка «кадр презентован» — debug-уровень.
- **Workspace/Cargo.toml:** canvas-app в [workspace.dependencies] (lib
  для canvas-web), wasm-bindgen-futures + web-sys (фичи у потребителя),
  canvas-app: pollster убран; canvas-render: pollster в [dependencies]
  (был dev-only); canvas-web: +app/core/render/scene/widgets/winit/
  anyhow/wasm-bindgen-futures/web-sys. Cargo.lock: новые
  consumer-строки, версии не менялись. CI-файлы не тронуты (протокол
  §6.1 п.4 — заморозка до W12).
- **Приёмка (весь каскад зелёный):** cargo fmt ✓; clippy --workspace
  --all-targets -D warnings ✓; test --workspace ✓ (45 наборов, 0 failed;
  +3 теста renderer_init, +2 canvas-web, app-тест на NoopRendererLaunch);
  wasm_gate.sh ✓; mcp_wasm_gate.sh ✓ (e2e oracle ±1 %); test -p
  canvas-shell ✓ (129). trunk build ✓ (dist: index.html 4.7 КБ + глю
  78 КБ + wasm 20.6 МБ debug). Браузерный дым (Chromium 153 + swiftshader,
  agent-browser --webgpu): модуль стартует → каркас → сцена сеется →
  окно+канвас 1280×577 в DOM (context: rgba8unorm/opaque) → «рендер
  инициализирован backend=BrowserWebGpu format=Rgba8Unorm
  present_mode=Fifo» → кадры презентуются (24 инстанса сетки), ошибок
  страницы и GPU-валидации (uncapturederror-listener) нет. Пиксельный
  скриншот WebGPU-канваса в headless — известное ограничение платформы
  (agent-browser doctor: «WebGPU renders, but headless screenshots miss
  the canvas»); рендер подтверждён present-логами и doctor-пробой
  GPU-readback. Зум/пан/F3 проверяются владельцем в `trunk serve` на
  обычном десктоп-браузере (headless-песочница принципиально не даёт
  визуального канала).
- **Уроки (для W6/W10/W11):** NO_COLOR=1 в песочнице ломает trunk
  0.21.14 (clap: «invalid value '1' for --no-color'») — запускать
  NO_COLOR=false; WebGPU-диагностика — uncapturederror-listener + поле-
  суффиксы web_log; wgpu-web деградации молчат (None без причины) —
  differentiate adapter/device в GpuContext::new при будущих правках.
- **Коммит:** 1 коммит на feature/wasm-w4-proshivka → merge --no-ff в
  main (протокол §6.1).

## 2026-09-19 — W3 (M8 wasm-порт, трек A): трейты сервисов в core, инъекция в App::new, canvas-app lib под wasm32

- **Задача (§6.1):** трек A, шаг 3 после W2 (8482183): «Трейты сервисов»
  (wasm-port §6 W3): `CanvasStorage`, `ClipboardBackend`, `WatchBackend`,
  `SearchBackend` (+MemSearch); инъекция в `App::new`; нативные реализации =
  сегодняшнее поведение. Ветка feature/wasm-w3-service-traits от main
  8482183 (origin/main не ушёл — гонки нет).
- **Survey → расширение скоупа (документировано):** приёмка W3→W4 требует
  собрать `App::new` без нативных типов — проверка
  `cargo check -p canvas-app --target wasm32` показала, что shell (rusqlite
  bundled → clang) под wasm не собирается, а в lib-коде кроме 4 сервисов
  живут `ThumbService`, `WidgetStateStore` (widgets.rs, T21-E) и
  `DragEvent`/`DragData` (T9). Итого 6 трейтов + перенос drag-типов.
- **canvas-core (контракты, паттерн NoopThumbnailProvider):**
  - `providers.rs` += `ClipboardBackend`+`NoopClipboard`,
    `WatchBackend`+`NoopWatch`, `ThumbBackend`+`NoopThumbs`,
    `WidgetStateBackend`+`MemWidgetState`, `Priority` (из shell);
  - `io.rs` += `CanvasStorage` + `FsCanvasStorage` (натив: load +
    save_with_backup = `.bak`, SPEC §9) + `MemStorage` (тесты);
  - new `search.rs`: протокол T14 (`IndexEntry`/`SearchCommand`/
    `SearchEvent`/`SearchHit`/`SearchResponder`) + `SearchBackend` +
    `MemSearch` (BTreeMap-индекс имён, §3.2 «ответы через тот же
    SearchEvent»);
  - new `dragdrop.rs`: `DragData`/`DragEvent` из shell (shell и web-бинарь
    (W6) — производители, app — потребитель).
- **canvas-shell (нативные реализации «как сегодня»):** new `clipboard.rs`
  `ArboardClipboard` (код `Clipboard` из app.rs); `impl WatchBackend for
  WatchService`; `impl SearchBackend for SearchService`; `impl ThumbBackend
  for ThumbService`; `impl WidgetStateBackend for WidgetStateStore`;
  протоколы поиска/drag — ре-экспорты из core (`canvas_shell::SearchCommand`
  и `canvas_shell::dragdrop::DragEvent` — пути потребителей сохранены);
  arboard — dep shell (из canvas-app).
- **canvas-scene:** `SceneState.storage: Arc<dyn CanvasStorage>`;
  `with_storage`/`load_or_seed_with_storage` (сигнатуры `new`/
  `load_or_seed` сохранены — 0 изменений в существующих тестах/MCP);
  `save_now` идёт через хранилище.
- **canvas-app:** `App::new` — инъекция: `Box<dyn ThumbBackend/WatchBackend/
  SearchBackend/ClipboardBackend>` + `widget_state` + `cache_dir`
  (12 параметров, shell-типов нет); `Clipboard` удалён из app.rs; поля
  App — трейт-объекты; `AppEvent::Drag(canvas_core::dragdrop::DragEvent)`;
  widgets.rs — `state_store: Option<Box<dyn WidgetStateBackend>>` (открытие
  store — работа вызывающего); Cargo.toml: `canvas-shell` →
  `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, arboard убран.
- **main.rs:** нативная сборка — `FsCanvasStorage` в сцену,
  `ArboardClipboard`, боксы над shell-сервисами, `WidgetStateStore::open`.
- **Результат:** `cargo check -p canvas-app --lib --target
  wasm32-unknown-unknown` — зелёный (исторически: весь UI-слой wasm-чист,
  W4-прошивка разблокирована).
- **Тесты-заглушки (приёмка):** core — MemSearch (ReplaceAll→Indexed,
  Query case-insensitive/limit/пустой, IndexFile/RemoveFile), MemStorage
  roundtrip, FsStorage `.bak` (native-only — wasip1-гейт без temp_dir),
  Noop-инертность; scene — save_now через инъекцию; app —
  `app_assembles_on_stub_backends` (App целиком на заглушках, roundtrip
  widget-state, NoopClipboard). Итого 157 app / 54 scene / 239 core.
- **Гейты (все зелёные):** fmt ✓; clippy --workspace --all-targets
  `-D warnings` ✓ (0w); test --workspace ✓ (45 наборов, 0 failed);
  wasm_gate.sh ✓; mcp_wasm_gate.sh ✓ (e2e, exit 0); wasm-check
  canvas-app lib — ✓ (локально, вне CI до W12).
- **Доки:** wasm-port.md строка W3 «Выполнено»; CI-файлы не тронуты
  (заморозка до W12); Cargo.lock — только ребро arboard app→shell.
- **Коммит:** 1 коммит на feature/wasm-w3-service-traits → merge `--no-ff`
  в main (протокол §6.1).

## 2026-09-19 — W2 (M8 wasm-порт, трек A): вынос App в lib — `canvas_app::app`, main.rs — тонкая нативная обёртка

- **Задача (§6.1):** трек A, шаг 2 после W1 (884309e→705e923): «Вынос App
  в lib» (wasm-port §6, строка W2; MW1 уже сузил задачу — SceneState
  уехал в canvas-scene). Чистое перемещение, zero behavior change;
  синхронизация main: origin/main ушёл вперёд на W4-каркас трека B
  (1056d89) — ff-pull, зоны не пересеклись.
- **Сделано (механический сплит, скрипт с assert'ами границ):**
  - **`crates/canvas-app/src/app.rs`** (новый, ~10.9k строк) — модуль
    `canvas_app::app`: `App` (все четыре `impl` — new, ввод/редактирование,
    рендер-кадр, MCP/виджеты/поиск), `ApplicationHandler<AppEvent>`,
    `AppEvent`, типы-обвязка (Clipboard/OwnedScreenText/DropPreview/
    AppDialog/SettleAnim/HelpMenuState/DocsViewer/WhatIfOverrideRow),
    хелперы (геометрия оверлеев, slugify/шаблоны, стресс-сцена
    `--stress`/`--stress-widgets`, `parse_args`/`CliArgs`,
    `measured_result_reserve_height`, `open_path_externally`) и весь
    `mod tests` (юнит-тесты переехали вместе с кодом).
  - **`main.rs`** — 311 строк (было 11 172): нативная инициализация
    (mcp-режим FR-008, трейсинг FR-035, crash-recovery/single-instance
    T17/T15, конфиг, пул тамбнейлов/вотчер/поиск/виджеты/MCP-pipe) +
    `run_app`. Зависимости бинаря: только `canvas_app::app::*` (pub),
    canvas-core/-scene/-shell/-widgets, winit.
  - **Видимость:** `App`/`AppEvent`/`App::new`/`init_widgets` — `pub`
    (кросс-крейтный API для canvas-web W4-прошивка); bin — отдельный
    крейт, поэтому `pub(crate)` не виден (E0603) — все символы main.rs —
    `pub`; остальные хелперы остались приватными в модуле.
  - Само-ссылки `canvas_app::` → `crate::` (55 шт., перенесённый код);
    lib.rs — `pub mod app;` с докой (§3.1 п. 2).
- **Гейты (все зелёные):** fmt --check ✓; clippy --workspace
  --all-targets `-D warnings` ✓ (0w); test --workspace ✓ — 45 наборов,
  0 failed, юнит-тесты canvas-app исполняются из lib (156, было в bin);
  wasm_gate.sh ✓ (345 core + 13 mcp в wasmtime); mcp_wasm_gate.sh ✓
  (e2e-сессия, oracle ±1 %, exit 0). Прогон с
  CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 (урок W1 — диск).
- **Доки:** wasm-port.md строка W2 — «Выполнено 2026-09-19 (трек A,
  §6.1)»; AGENTS.md не тронут (структура workspace без изменений);
  CI-файлы заморожены до W12 (п. 4 протокола).
- **Коммит:** 1 коммит на feature/wasm-w2-app-to-lib → merge `--no-ff`
  в main (протокол §6.1).

## 2026-09-19 — W4-каркас (M8 wasm-порт, трек B): крейт canvas-web — bindgen-обвязка, panic-hook, tracing-консоль

- **Задача (владелец):** «Ты отвечаешь за трек B» — двухагентное
  расписание wasm-port §6.1 (утверждено 14bf50f): трек B стартует
  W4-каркасом параллельно W1–W3 трека A (ноль файловых пересечений —
  только новые файлы `crates/canvas-web/` + редкая правка workspace-
  манифеста). Протокол: ветка на задачу, merge `--no-ff` после гейтов,
  CI-файлы заморожены до W12 (не тронуты).
- **Сделано:**
  - **`crates/canvas-web/`** (новый крейт, cdylib+rlib — зеркало роли
    canvas-shell, §3.1; лист в DAG, §3.5): `src/lib.rs` — точка слияния
    (mod-декларации + init-строки): `#[wasm_bindgen(start)]` →
    `boot()` (panic-hook + tracing + баннер версии); `src/panic_hook.rs`
    — паники → console.error (wasm) / stderr (натив), формат — чистая
    функция; `src/web_log.rs` — консольные extern'ы (log/info/warn/error,
    без js-sys/web-sys — их привнесёт winit на прошивке) + собственный
    `ConsoleLayer` на tracing-subscriber (Registry + INFO-фильтр,
    идемпотентный init) — ноль новых внешних зависимостей сверх
    bindgen-семейства; `index.html` (заглушка каркаса) + `Trunk.toml`
    (dist → target/dist, serve :8080).
  - **Зависимости:** wasm-bindgen 0.2.127 — прямая prod-зависимость;
    семейство уже было в Cargo.lock транзитивно (winit, wasm-цели) —
    lock-дифф только запись canvas-web, счёт внешних крейтов не изменился
    (382). Версия пинится js-sys 0.3.104 (=0.2.127). Лицензионный ритуал
    CP0: `cargo deny check` — advisories/bans/licenses/sources ok;
    THIRD-PARTY-NOTICES.md перегенерирован (+bindgen-семейство);
    DEPENDENCIES.md — first-party строка canvas-web + прямая
    зависимость wasm-bindgen (lock 2026-09-19).
  - **Инструменты сборки (в контейнер, версии зафиксированы):**
    trunk 0.21.14, wasm-bindgen-cli 0.2.127 (= версии крейта в lock —
    обязательное совпадение), cargo-deny 0.20.2, cargo-about 0.9.2.
    Нюанс trunk: env NO_COLOR=1 валивается его clap (`--no-color`
    ожидает true/false) — запускать с NO_COLOR=true.
  - **Доки:** wasm-port.md W4 — аннотация «Каркас исполнен» (приёмка
    каркаса, прошивка после W3); AGENTS.md — строка canvas-web в
    структуре workspace.
- **Приёмка каркаса:** cargo check -p canvas-web (native) ✓; cargo
  check --target wasm32-unknown-unknown -p canvas-web ✓; cargo test
  -p canvas-web — 7/7 (формат строки события/паники, идемпотентность
  init, проводка событий через слой, паника через hook без смерти
  unwind, баннер версии); `trunk build` ✓ (target/dist: index.html +
  JS-глю 5,6 КБ + wasm 602 КБ debug); браузерный дым (agent-browser +
  http.server): страница грузится, в консоли
  `[INFO canvas_web] canvas-web каркас загружен (W4): версия 0.1.0`,
  ошибок страницы нет — модуль инстанцируется, start() отрабатывает.
- **Гейты протокола §6.1:** fmt ✓; clippy --workspace --all-targets
  -D warnings ✓; cargo test --workspace — 1096/0 (1089 + 7 каркаса);
  scripts/wasm_gate.sh ✓; CI-файлы не тронуты (заморозка до W12).
- **Гонка с треком A (протокол п.2):** первый push отклонён — параллельно
  влит W1 (705e923, web_time). main сброшен на origin, ветка
  перемержена поверх W1 (конфликт только worklog.md — обе записи
  сохранены; Cargo.toml/lock автомерж: web-time + wasm-bindgen/
  canvas-web сосуществуют). Гейты перегнаны на объединённом дереве:
  fmt/clippy ✓; 1096/0 (W1 тестов не добавил); wasm_gate.sh ✓;
  cargo deny ✓; trunk build ✓.
- **Дальше (трек B):** W4-прошивка после W3 трека A (spawn_app,
  async-init Renderer через spawn_local, пустая сцена); затем
  W7 → W9 → W8 → W11; W10 — после W6 трека A.

## 2026-09-19 — M8/W1: web_time::Instant вместо std::time::Instant (трек A, §6.1)

- **Задача:** W1 (S) из `docs/plans/wasm-port.md` §6 — «Время»: миграция на
  `web_time::Instant` (alias в core), до W4. std-Instant на
  wasm32-unknown-unknown компилируется, но паникует в рантайме (§2 п. 7) —
  первый же кадр FrameMeter/debounce падал бы.
- **Сделано:**
  - `canvas-core/src/time.rs` (новый модуль) — alias
    `canvas_core::time::Instant` над `web_time::Instant`: на нативе
    прозрачная обёртка над std, на wasm32 — `performance.now()`;
  - `web-time = "1.1"` в `[workspace.dependencies]` + dep canvas-core —
    версия уже в дереве через winit 0.30, Cargo.lock получил только ребро
    `canvas-core → web-time` (смен версий нет);
  - замена std::Instant: `canvas-render` (renderer FrameMeter `cpu_start`,
    тест minimap), `canvas-scene` (`scene.rs` `dirty_since` — крейт на
    wasm-пути, ADR-0012), `canvas-app` (main/lib/widgets/palette/
    template_ui — DoubleClick-детектор, hover/debounce-таймеры, toast,
    focus-fade, search_pending, explorer-tracker);
  - `canvas-shell` не тронут — натив-only, вне скоупа волны 1 (§1);
  - wasm-port.md: строка W1 «Выполнено 2026-09-19 (трек A, §6.1)».
- **Гейты (все зелёные):** `cargo fmt --check`; `cargo clippy --workspace
  --all-targets -- -D warnings` (0 предупреждений); `cargo test
  --workspace` (все suites ok, 0 провалов; прогон с
  `CARGO_PROFILE_DEV_DEBUG=0` — на 9,9-ГБ диске полные debug-артефакты
  workspace не помещаются, env-оверрайд без правок репо);
  `scripts/wasm_gate.sh` (canvas-core 345 + canvas-mcp 13 в wasmtime,
  web-time 1.1 собрался под wasm32-unknown-unknown);
  `scripts/mcp_wasm_gate.sh` (scene 53 + мост 13 + headless 12, e2e-сессия
  oracle ±1 %).
- **Протокол:** §6.1 — трек A, ветка `feature/wasm-w1-web-time`,
  1 задача = 1 коммит, merge `--no-ff` в main после гейтов.

## 2026-09-19 — FR-037 MW5: инспектор-сессия владельца — mcp_wasm_inspector.sh (ADR-0012)

- **Задача (владелец):** «Бери в работу MW5 (S)» — опция FR-037: обёртка
  официального инспектора `npx @modelcontextprotocol/inspector` поверх
  wasmtime-запуска headless-сервера — живая ручная проверка MCP без
  Windows (снимает зависимость ручной MCP-приёмки от Windows-машины).
- **Сделано:**
  - **`scripts/mcp_wasm_inspector.sh`** (новый, один файл): два режима —
    web UI по умолчанию (сервер предподключён позиционной целью:
    `--web wasmtime run …canvasdesk-mcp-headless.wasm`; браузер →
    http://127.0.0.1:6274, токен-URL печатает сам инспектор; CLIENT_PORT
    пробрасывается) и `--check` — автоматическая приёмка инспектором как
    реальным MCP-клиентом: initialize → tools/list (36, graph_apply в
    списке) → tools/call graph_apply мини-эталон №1 → oracle ±1 %
    (MINI_OPS/ORACLE/close_1pct импортируются из scripts/mcp_wasm_e2e.py —
    один источник истины; артефакты — target/tmp/mcp_inspector_*.json).
    Сборка wasip1 внутри скрипта (с кэшем — секунды) или --skip-build;
    exit 0/1/2 как у e2e-драйвера.
  - Нюансы CLI инспектора 2.7.0 (найдены разведкой, зафиксированы в
    FR-037): версия запинена (`INSPECTOR_PACKAGE` — воспроизводимость);
    массивные аргументы инструментов — только через `--tool-args-json`
    (значения дословно; `--tool-arg key=value` коэрцитует значение в
    строку → сервер отвечает «отсутствует параметр operations»);
    schema-portability предупреждения — в stderr, stdout — чистый JSON;
    первый запуск npx требует npm registry (node 18+), дальше кэш.
  - **Доки:** FR-037 — статус «реализовано (MW1–MW5)», MW5 «Выполнено»,
    changelog; AGENTS «Сборка и тесты» — инспектор-сессия (node/npx вне
    гейтов); SPEC §13 — строка про инспектора в Headless-подразделе;
    ACCEPTANCE §29 FR-037.10 — конкретный сценарий (--check + web UI).
    Гейты/CI не менялись: npx — внешняя зависимость для автономных
    гейтов, инспектор — ручная способность владельца (Q4 ADR-0012).
- **Гейты:** fmt ✓; clippy --workspace --all-targets -D warnings ✓;
  cargo test --workspace 1089/0 (регресс — ноль, Rust-код не менялся);
  scripts/mcp_wasm_gate.sh — полный зелёный (78 wasip1-тестов + e2e);
  scripts/mcp_wasm_inspector.sh --check — зелёный (36 инструментов,
  5 чисел oracle ±1 %, exit 0); web UI — дым HTTP 200 в headless.
- **Инцидент диска №4** (ld signal 7 при линковке, 100 %): удалены
  target/debug/incremental (1,9 Г) + устаревшие тест-бинари >100 М;
  тесты перепрогнаны с CARGO_PROFILE_TEST_DEBUG=0 — итог 3+ Г свободно.
- **Дальше:** MW6 (файловый режим `--canvas`, опция — рекомендация Q5
  «нет»); по роадмапу — продуктовый веб-слой S5 (W1–W12 M8), волна 2
  (WebSocket-мост поверх HeadlessSession).

## 2026-09-19 — Реализация MW2 (FR-037: мост canvas-mcp под wasm)

- **Задача (владелец):** «распланируй и реализуй MW2 с 3 сабагентами» —
  план `docs/plans/mw2-wasm-bridge.md`: мост `canvas-mcp` под wasm
  (`run_stdio` → `run_stdio_with_transport`, гварды автоспавн-тестов,
  включение моста в wasm-гейты); MW2 без MW1; ветка
  `feature/fr-037-mw2-wasm-bridge`, один коммит.
- **Сделано** (MW2-a ∥ MW2-b → верификация/коммит MW2-c):
  - **`crates/canvas-mcp/src/lib.rs`** (MW2-a): тело stdio-цикла
    (stdin → `split_frames` → `handle_input` → stdout; буфер 8192,
    EOF = штатный выход) перенесено в
    `pub fn run_stdio_with_transport<T, R>(transport: Option<T>,
    reconnect: R)` (`T: AppTransport`, `R: FnMut(&mut Option<T>)`) —
    хук `reconnect(&mut transport)` перед каждым пакетом вместо
    cfg-вызова `refresh_transport`; `run_stdio(&[String])` — тонкая
    обёртка (автоспавн FR-035 / offline ADR-0009 / хук reconnect
    FR-034), pub-сигнатура неизменна, `main.rs` не тронут; гварды
    `#[cfg(all(unix, not(target_arch = "wasm32")))]`
    (`spawn_service_command_isolates_stdio`) и
    `#[cfg(not(target_arch = "wasm32"))]`
    (`autosprawn_target_prefers_sibling_gui_for_standalone_bridge`) —
    15 тестов, имена/ассерты не менялись; мин-правка MW2-c: убран
    лишний `mut` у `transport` в обёртке (clippy `unused_mut` после
    выделения цикла, семантика прежняя).
  - **`scripts/wasm_gate.sh`** (MW2-b): `CRATES` += `-p canvas-mcp`
    (ступени 1–2); ступень 3 — явный список `-p canvas-core
    -p canvas-mcp` (не `$CRATES`: render/widgets под wasip1 не
    тестируются — wgpu-тесты требуют GPU-адаптер); шапка/echo
    синхронизированы.
  - **`.github/workflows/ci.yml`** (MW2-b): джоба `wasm-check` —
    `run` += `-p canvas-mcp`, имя шага и комментарий (FR-037/MW2)
    актуализированы; `targets`/прочие джобы не тронуты.
  - **`docs/plans/mw2-wasm-bridge.md`** (оркестратор): план MW2 —
    декомпозиция MW2-a/b/c, сигнатура с хуком reconnect (§3), риски,
    критерии приёмки.
  - **`docs/change-requests/fr-037-mcp-wasm-verification.md`**:
    строка MW2 «Выполнено 2026-09-19» + Changelog.
- **Гейты:** `cargo fmt --check` ✓; `cargo clippy -p canvas-mcp
  --all-targets -- -D warnings` ✓; `cargo test -p canvas-mcp` нативно —
  15 passed/0 failed (регресс FR-008/034/035 — ноль);
  `cargo check --target wasm32-unknown-unknown -p canvas-mcp` ✓ (R1);
  `RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-mcp`
  — 13 passed/0 failed в wasmtime (R2; 2 автоспавн-теста исключены
  гвардами); `scripts/wasm_gate.sh` — полный зелёный прогон ступеней
  1–3 (canvas-core 318 + canvas-mcp 13 тестов в wasmtime).
- **Решения:** сигнатура выделенного цикла — с хуком
  `R: FnMut(&mut Option<T>)`: reconnect (FR-034) платформенный и не
  прячется в трейт `AppTransport` (§3 плана); MW3 (headless) вызовет
  `run_stdio_with_transport(Some(session), |_| {})`. Контингенция
  wasmtime-флагов не понадобилась (wasmtime 48.0.2 принял runner из
  `.cargo/config.toml`); после ребейза на апстрим (CP6/FR-017, main
  `83d43a9`) применена вторая контингенция плана (§4 MW2-a п.4): cfg
  хелперов `spawn_service_command`/`autosprawn_target` расширен до
  `#[cfg(any(windows, all(test, not(target_arch = "wasm32"))))]` —
  dead_code-warning'и под wasip1-test устранены, `std::process` полностью
  вне wasm-сборки; итоговый полный гейт — 0 warnings.

---

## 2026-09-18 — Реализация CP5 (FR-016: индикаторы узких мест, волна B1)

- **Задача (владелец):** «Реализуй CP5» — по `docs/plans/product-roadmap.md`
  §9: FR-016 (волна B1) — bottleneck/queue-risk индикаторы по ρ и W из
  уже посчитанного потока; гейт — «эталон №1 визуально exposes узкие
  места без чтения чисел в нодах», автотесты — юниты классификации
  (пороги ρ/L) + e2e на эталоне.
- **Сделано:**
  - **`canvas-core/src/analyze.rs`** (новый, чистая функция — образец
    validate.rs): `AnalysisFlags {utilization, queue_length, wait_sec,
    severity}` / `Severity {None, Warn, Critical, Overload}` /
    `AnalysisConfig` (пороги 0.7/0.9, 100 ms/1 s, 1/10 — дефолты
    документа FR-016, вынесены в данные — инвариант 2) / `badge_text`
    (строка бейджа — канвас = MCP, инвариант 4) / `has_risk` (гейт
    авто-включения). Детекция v1 — актуализация под скалярный движок
    (документ писал про `Value::Struct` от mm1 — это v2 движка):
    `Err(Overload{rho})` → Overload (ρ из ошибки); named-выход
    `utilization` (Percent, доля, ≥1 → Overload) / Percent-значение
    ноды → ρ; Time-значение → W; named `wait_time`/`queue_length` —
    точки расширения; Count сам по себе НЕ очередь (анти-ложные
    срабатывания на innocent «10 req»); SLA — v2. Unit-хелперы
    expr.rs (`Atom::new`, `Unit::dims/scale`) подняты до pub(crate).
  - **Манифесты**: 13 queue-шаблонов (cdn, tcp-lb, lb, api-gateway,
    auth-service, http-endpoint, cache-redis, db-sql-master/-replica,
    queue-kafka, worker, graphql, grpc-service) + именованный выход
    `utilization` (зеркалит аргументы своего mm1), версии +minor —
    ρ стал данными потока через инфраструктуру FR-029.
  - **canvas-app**: `SceneState.analysis` (runtime-кэш, хвост
    `recompute_flow` — покрывает все 20+ точек мутаций); настройка
    `bottleneck_overlay` (config.toml, персистентная, дефолт выкл);
    тоглы — Ctrl+B (канвас-уровень, кириллица «и»; Bold в редакторе
    не конфликтует — маршрутизация выше), пункт меню канваса «Узкие
    места (Ctrl+B)» (6-й, ✓), строка панели настроек «Индикаторы
    узких мест»; авто-включение один раз за запуск при первом риске
    (Warn+) с тостом, ручной тогл глушит авто до перезапуска.
  - **Рендер**: рамка серьёзности на карточке (приоритет шейдера
    selected > broken > border.a; выделенная нода с риском — внешнее
    кольцо `analysis_ring_instance`, обе рамки видны — ручная
    приёмка); бейдж — моно-текст `badge_text` цветом серьёзности над
    правым верхним углом (шапка занята заголовком/иконкой —
    адаптация документирована в FR-016 changelog); LOD: бейджи ≥ 0.6
    эфф. зума (физическая читаемость, как titles_visible), рамки
    Warn/Critical ≥ 0.25, Overload — всегда; цвета/тексты — по темам
    (контраст CR-007: тёмно-красный #7A0010 из документа читаем на
    светлом, на тёмном — яркие аналоги); SceneView/TitleFrame +
    поля `analysis`/`analysis_overlay`/`analysis_badges`.
  - **MCP**: инструмент `analyze_bottlenecks` (TOOLS 26 → 27) —
    {nodes: [{id, severity, utilization?, queue_length?, wait_sec?,
    badge}], thresholds}; свежий пересчёт как flow_recalc; чтение
    (e2e проверяет канвас/undo байт-в-байт).
  - **Тесты** (+22): 16 юнит analyze (все уровни, пороги-как-данные,
    ρ из ошибки, named-выходы, детерминизм, сериализация, бейджи);
    4 юнит рендера (палитры серьёзностей по темам, LOD-пороги,
    геометрия кольца, текстовые цвета); e2e
    `analyze_bottlenecks_reference_and_growth` — мини-эталон №1
    (ADR-0006): базовая линия CDN ρ 0.417/W 34.29 ms (здоров),
    node_update_text DAU ×2 → Warn ρ 0.833 + бейдж «83% · W: 120 ms»,
    DAU ×5.35 → Overload ρ 2.23 (ветка C эталона №2) ±1 %, GW
    здоров (ρ 0.334), read-only-инварианты, синхронность
    runtime-кэша. Обновлены пины версий манифестов (lb 1.1→1.2) и
    счётчики меню (6)/инструментов (27).
  - **Доки**: FR-016 → «реализовано (v1)» + Changelog (актуализация
    детекции, бейдж над карточкой, цвета по темам); index-cr-fr;
    roadmap Changelog (4); SPEC §4 (27) + §13 analyze_bottlenecks;
    ACCEPTANCE §25 (FR-016.1–9); user-docs hotkeys (Ctrl+B) /
    interface (раздел «Индикаторы узких мест») / calculations
    (связка ρ→оверлей); interface-objects/node.md §5 (состояние
    серьёзности).
- **Гейты:** cargo fmt ✓; clippy --workspace --all-targets -D warnings ✓;
  cargo test --workspace — 1034 passed / 0 failed. Инцидент: линковка
  упала ld signal 7 — диск 100 % (target/ 9.1G, тот же диагноз, что в
  записи FR-035); cargo clean → прогон заново.
- **Открытые пункты (следующие шаги):** CP4 = R5 (рецепт агента
  user-docs/agent-recipe.md — не начат), CP6 = FR-017 v1 (what-if —
  переиспользует analyze + AnalysisConfig на override-результатах);
  v2-хвосты FR-016: SLA-сравнение, кастомные пороги в конфиге,
  паттерны рамки для цветовой слепоты, heatmap.

---

## 2026-09-18 — Реализация CP0 (волна 0) + CP1 (FR-029 порты значений)

- **Задача (владелец):** на основании product-roadmap.md реализовать CP0 и CP1.
- **Сделано:**

  **CP0 (волна 0 — гигиена):**
  - `deny.toml` (cargo-deny): allowlist §7.2 архдока + `Apache-2.0 WITH
    LLVM-exception` с обоснованием (строго пермиссивнее Apache-2.0), bans
    (wildcards deny, множественные версии — warn), sources (только crates.io),
    advisories: yanked deny, три осознанных ignore unmaintained рендер-стека
    (RUSTSEC-2024-0436 paste / RUSTSEC-2026-0206 rustybuzz /
    RUSTSEC-2026-0192 ttf-parser) с планом миграции после гейта Go;
  - `publish = false` всем 7 workspace-крейтам (закрытый B2B-продукт,
    защита от случайной публикации; требуется private-ignore в cargo-deny);
  - CI (ci.yml): джоба `licenses` (EmbarkStudios/cargo-deny-action@v2) на
    каждый пуш/PR; артефактные сборки — `cargo auditable build`
    (SBOM-in-binary); build-all.yml: джоба `notices-sbom` — регенерация
    THIRD-PARTY-NOTICES + дрифт-контроль + артефакт;
  - `about.toml` + `docs/templates/third-party-notices.hbs` + сгенерированный
    `THIRD-PARTY-NOTICES.md` (382 крейта, все лицензии пермиссивные);
  - `docs/DEPENDENCIES.md` — реестр: first-party, 16 прямых прод-зависимостей
    с лицензиями, кандидаты волны S из архдока §4.2, долг сопровождения
    (unmaintained), рецепты (notices/SBOM/добавление зависимости);
  - cargo-фичи `stats`/`parallel` в canvas-core (пустые гейты волны S,
    archdoc M1); `include_dir`/`image` централизованы в
    [workspace.dependencies] (SPEC §3);
  - триаж волны 0: FR-023 → «выполнено (v1)» (`7551e12` + CR-010/CR-012),
    FR-024 → «выполнено (v1)» (`74b4333`, расширен FR-030), CR-005 →
    бэклог §7 (точечно по мере боли); changelog-записи в документах +
    строки index-cr-fr.md.

  **CP1 (FR-029 — порты значений, волна A1):**
  - `model.rs`: `Edge.from_output`/`to_param` (сериализация `fromOutput`/
    `toParam`, мягкое чтение как `fromLine`, round-trip старых `.canvas`
    байт-в-байт — тесты); перепривязка from-конца сбрасывает `from_output`;
  - `templates.rs`: `OutputSpec { name, unit, source: Line(i)|Expr(s) }`,
    секция `outputs` манифеста (схема 1.1, опциональна — 45 builtin
    валидны), снапшот `canvasdesk.template.outputs` (ключ только при
    непустой секции), перенос outputs в custom-шаблон при «Сохранить
    как шаблон»;
  - `flow.rs`: `FlowSolutions.named` + `warnings`; именованные выходы
    шаблонных нод (Line — из построчных, Expr — в окружении ноды) и
    текстовых (переменные Numi-листа — адресация живёт при сдвиге строк);
    проливание `toParam` ПОСЛЕ локальных параметров («проливание сильнее
    дефолта»), рёбра с `toParam` вне позиционных слотов, конфликты —
    последнее по `canvas.edges` + предупреждение;
  - MCP: `edge_create` v2 (kind/fromLine/fromOutput/toParam, взаимные
    исключения, валидация имён по снапшотам шаблонов/переменным листа),
    новый `edges_list` (23 инструмента), `flow_recalc` v2 (value + outputs
    + lines + warnings), `template_list` отдаёт outputs;
  - манифесты: outputs для cdn, tcp-lb (fan-out auth/feed/media по долям
    — новые параметры), graphql (db_qps/events), db-sql-master
    (replica_load), db-sql-replica, queue-kafka (consume_rate),
    api-gateway, lb, cache-redis;
  - тесты: round-trip/lenient (model), проливание/перекрытие/совместимость
    слотов/сдвиг строк/конфликты/тихая деградация (flow), Instagram MVP
    e2e `mcp_fr029_instagram_mvp_reference` — 12 нод собираются MCP,
    10 адресованных рёбер, oracle ADR-0005 ±1% (avg 555.6 / peak 1388.9 /
    origin 555.6 / auth 83.3 / feed 333.3 / media 138.9 / db_qps 80 /
    events 333.3), правка DAU одним `node_edit` удваивает цепочку;
  - документация: SPEC §5.1 (поля рёбер + outputs снимка), user-docs/
    calculations.md («Проливание в параметры»), fr-029 changelog + статус,
    index, roadmap changelog.

  **Гейты:** `cargo fmt --check` ✓, `cargo clippy --workspace --all-targets
  -D warnings` ✓, `cargo test --workspace` — 981 passed / 0 failed ✓,
  `cargo deny check` — licenses/bans/sources/advisories ok ✓.
- **Коммит:** см. git log — CP0/CP1.
- **Интеграция с CP2 (FR-032, влит параллельным коммитом `1fb63e6`):**
  выполнен чек-лист из changelog FR-032 — `port_contract_issues`
  (E-UNIT/E-PORT-UNKNOWN/E-DOUBLE-INPUT на адресации FR-029),
  W-AMBIGUOUS-SRC с `fromOutput`, W-UNUSED-SLOT без `toParam`-рёбер,
  `mcp_edge_json` с полями портов (edges_list/edge_get), дубликат ветки
  edges_list устранён (TOOLs = 25); гейт A2 — 3 подсаженные ошибки на
  эталоне Instagram → ровно 3 issue (в составе e2e
  `mcp_fr029_instagram_mvp_reference`).
- **Открытые пункты (следующие шаги):** CP3 = FR-033 (graph_apply), CP4 =
  R5 (рецепт агента user-docs/agent-recipe.md), CP5 = FR-016, CP6 =
  FR-017 v1.

---

## 2026-09-18 — CP2: FR-032 v1 — чтение графа + graph_validate (код)

- **Задача (владелец):** реализовать CP2 продуктового роадмапа (`product-roadmap.md`
  §9) — FR-032 (R2, волна A2). Параллельно другой агент ведёт CP0/CP1 (FR-029 —
  порты значений); работа CP2 — в собственном клоне, ветка
  `feature/fr-032-graph-read-validate`.
- **Сделано (v1 — всё, что не зависит от полей FR-029):**
  - `canvas-core/src/validate.rs` (новый, чистая функция без I/O):
    `ValidationIssue {severity, code, node_id, edge_id, message}` — сериализация
    snake_case, коды — стабильный контракт; `validate(&Canvas) ->
    Vec<ValidationIssue>`; реализованы `E-CYCLE` (при цикле отчёт
    ограничивается топологией — без шума слот-предупреждений), `E-OVERLOAD`
    (из `propagate_with_lines`), `W-AMBIGUOUS-SRC` (многолинейный исток без
    `fromLine`), `W-UNUSED-SLOT` (обход дерева формул приёмника: `$N`/`$in`,
    фенсы и проза пропускаются; диагностика G5 — позиционное ребро в шаблонную
    ноду теряется); `E-UNIT`/`E-PORT-UNKNOWN`/`E-DOUBLE-INPUT` — на полях
    FR-029, точка добавления — `port_contract_issues` (контракт в док-комменте);
  - `canvas-mcp`: TOOLS 22 → 25 — `edges_list`, `edge_get`, `graph_validate`
    (схемы + описание контракта кодов); тест схем обновлён;
  - `canvas-app` `mcp_dispatch`: три ветки чтения (валидация не мутирует:
    без undo/автосейва/`recompute_flow`); каноническая схема ребра —
    `mcp_edge_json` (единственное место, куда FR-029 добавит `toParam`/
    `fromOutput`);
  - тесты: 10 юнит в `validate.rs` (чистая сцена → `issues == []`, фиксстура
    на каждый код, `$in`/`$N`-семантика слотов, шаблонная нода, детерминизм,
    контракт сериализации) + 3 e2e `mcp_dispatch` (`mcp_edges_list_and_get`,
    `mcp_graph_validate_clean_and_cycle`, `mcp_graph_validate_overload_and_unused_slot`);
  - дока: SPEC.md §4 (структура: validate.rs, 25 инструментов) + §5.1 (буллет
    MCP-чтения/валидации), ACCEPTANCE.md §21 (чек-лист, сценарии 10–12 —
    «после CP1»), interface-objects/edge.md (строка «Чтение топологии»,
    §6-буллет, источник), FR-032 → «в работе (v1)» + Changelog (включая
    чек-лист интеграции CP1 из 5 пунктов), index-cr-fr.md.
- **Гейты:** fmt/clippy(-D warnings, --all-targets)/test --workspace — зелёные
  (все 36 сюит; ядро 175 тестов, +13 новых).
- **Интеграция CP1 (для агента, вливающего FR-029):** см. Changelog FR-032 —
  заполнить `port_contract_issues`, два условия в `validate()`, поля в
  `mcp_edge_json`, сценарии ACCEPTANCE FR-032.10–12 и гейт A2 «3 подсаженные
  ошибки → ровно 3 issue». До влития CP1 три из кодов гейта A2 определить не
  на чем (контракта портов ещё не существует) — это ожидаемое состояние
  параллельной разработки, не дефект.
- **Коммит/ветка:** `feature/fr-032-graph-read-validate` (база 433e3fa),
  коммит — см. git log (feat(core,mcp,app): FR-032 v1).
---

## 2026-09-18 — FR-033 graph_apply: атомарная батч-композиция (CP3, ветка feature/fr-033-graph-apply)

- **Задача (владелец):** по `docs/plans/product-roadmap.md` реализовать CP3
  (FR-033, волна A3) — параллельно: агент_1 ведёт CP0+CP1 (волна 0 + FR-029),
  агент_2 — CP2 (FR-032). Работа в отдельной ветке для бесконфликтной сборки.
- **Сделано (код):**
  - `canvas-mcp`: инструмент `graph_apply` (TOOLS 22 → 23 на ветке) — схема
    `operations: [Op; 1..=256]`, тег op с 6 вариантами; лимиты в описании;
  - `canvas-app`: `mcp_graph_apply` (транзакция: клон → apply → commit;
    ошибка операции → `{ok:false, op_index, code, message}`, канвас байт-в-байт
    прежний; успех → ровно один undo-шаг + spatial + dirty + recompute_flow),
    `batch_apply_op` (6 операций: node_create_note/file, template_instantiate,
    edge_create с портами FR-029 и валидацией имён/циклов, param_set с правкой
    ровно одной строки и синхронизацией снапшота шаблона, node_move),
    ref-резолв (дубликат ref — E-BAD-OP), `mcp_flow_v2` (flow_recalc v2:
    value/unit/outputs/lines/error);
  - `canvas-core` (минимальный контур FR-029 — необходим гейту CP3, при
    интеграции уступает полной реализации CP1): `Edge.to_param`/`from_output`
    (мягкое чтение, round-trip, сброс при перепривязке истока), `OutputSpec`/
    `OutputSource` в манифесте и снапшоте `TemplateRef`, проливание `to_param`
    поверх локальных параметров («последнее ребро побеждает»), резолв
    `from_output` (Line/Expr) в `propagate_with_lines`, `FlowSolutions.named`;
  - `assets/templates/com.canvasdesk.cdn`: именованный выход `origin` (демо
    мини-эталона; остальные манифесты — за полной реализацией FR-029).
- **Тесты:** +14 (7 `graph_apply_*` в main.rs: oracle e2e ±1 % по эталону №1
  ADR-0006 — 208.33 rps / 34.29 ms / 20.83 rps / 3.20 ms, атомарность,
  ref-правила, param_set, лимиты, undo/redo, цикл; 7 flow-тестов проливания;
  2 round-trip порта в model.rs). Гейты зелёные: fmt, clippy -D warnings,
  test --workspace — 988 тестов, 0 провалов.
- **Документация:** SPEC §13 «MCP-инструменты канваса» (graph_apply: схема,
  лимиты, транзакция, коды ошибок) + счётчик 23 в §4; ACCEPTANCE §21
  (FR-033.1–FR-033.10); fr-033 — статус «реализовано (v1)» + Changelog;
  index-cr-fr — статус.
- **Коммит:** ветка `feature/fr-033-graph-apply` (не main — параллельные
  CP0/CP1/CP2 других агентов; порядок интеграции: CP0/CP1 → CP2 → CP3,
  контур FR-029 в этой ветке уступить ветке CP1).

---

## 2026-09-18 — Критический путь: детализация + FR-032/FR-033 (разметка FR/CR)

- **Задача (владелец):** прописать критический путь с обоснованием, разметить
  и заполнить FR/CR; каждый этап должен быть наглядным в части тестирования.
- **Сделано:**
  - product-roadmap.md: новый §9 «Критический путь: обоснование и тестовая
    наглядность» — CP0–CP7 (правило наглядности «наблюдаемо без чтения кода»,
    обоснование позиции каждого шага, наглядный тест-демо на 5 минут +
    автотесты; блок «почему порядок нельзя переставить»); таблицы §4.2/§5/§6
    обновлены ссылками на созданные документы; Changelog — запись (2);
  - создан fr-032-graph-read-validate.md (R2: edges_list/edge_get +
    graph_validate, коды E-*/W-* как стабильный контракт, чистая функция
    validate.rs, секция «Наглядная проверка (5 минут)»);
  - создан fr-033-graph-apply-batch.md (R3: graph_apply — транзакция
    «всё или ничего», один undo-шаг, ref-резолв, лимиты ≤256 операций,
    секция «Наглядная проверка (5 минут)»);
  - разметка: CR-013 — таблица R1–R5 (R2 → FR-032, R3 → FR-033, R4 → после
    гейта S4, R5 → A4, CP-позиции) + Changelog; FR-029 — связки с FR-032/033,
    «Наглядная проверка (5 минут, A1/CP1)», Changelog; FR-016 — CP5;
    FR-017 — CP6 + разграничение v1/S2; index-cr-fr.md — строки FR-032/033,
    примечание о нумерации (следующий — FR-034).
- **Коммит:** см. git log — docs(cr,plans): критический путь + FR-032/FR-033.

---

# Worklog — журнал работ агентов по репозиторию CanvasDesk

Журнал дополняется сверху вниз (новые записи — выше). Ссылка на этот файл — в
подвале `docs/change-requests/index-cr-fr.md` (там, где «отчёты в worklog.md репо»).
Формат записи: дата, задача, что сделано, артефакты/коммиты.

---

## 2026-09-18 — Пересмотр продуктового роадмапа (по ADR-0008 + math-computing-stack.md)

- **Задача (владелец):** полностью пересмотреть роадмап разработки на основании
  `docs/architecture/math-computing-stack.md` и
  `docs/adr/adr-0008-math-computing-stack.md`; анализ документации + продуктовое
  интервью (8 вопросов) + оформление решений в репо.
- **Решения владельца (интервью):** канал проверки — догфудинг + живые демо;
  «вау» — полная петля «агент собирает → человек крутит»; эталоны — все №1–№5
  (ответ «D» — трактовка «все варианты выше», №1 и №5 первыми); дедлайна
  B2B/реестра нет; ADR-0008 — принять; инфра-хвосты и WASM — бэклог; гейт —
  ≥ 5 внешних пользователей сами построили модель и вернулись второй раз.
- **Сделано:**
  - создан `docs/plans/product-roadmap.md` — волны 0/A/B/V/S, гейты, привязка
    к M0–M6 архдока и R1–R5 CR-013, нумерация FR (FR-032 = R2, FR-033 = R3),
    бэклог с триггерами, операционализация метрики гейта;
  - ADR-0008 + архдок: статус «предложено» → «принято» (решение владельца
    2026-09-18), в §9 архдока добавлено продуктовое время этапов;
  - README: раздел «Дорожная карта» переписан под волны (старый пункт
    «Композиция моделей» заменён);
  - AGENTS.md: роадмап добавлен в источники истины; SPEC §12: битая ссылка
    на несуществующий `roadmap.md` → `docs/plans/product-roadmap.md`;
  - index-cr-fr.md: статусы FR-018/019/020 синхронизированы с заголовками
    документов («выполнено (v1)»; индекс отставал от документов);
    примечание о нумерации — FR-032/FR-033 закреплены, следующий — FR-034;
  - CR-013: запись в Changelog о привязке R1–R5 к волнам;
    docs/adr/README.md: строка ADR-0008 → «принято»;
  - создан настоящий worklog.md (до этого файл не существовал, хотя индекс
    CR/FR ссылался на него).
- **Коммит:** см. `git log` — docs(adr,plans): продуктовый роадмап + ADR-0008 принято.
- **Открытые пункты (следующие шаги):** волна 0 (cargo-deny в CI + deny.toml,
  реестр зависимостей, фичи `stats`/`parallel`, триаж FR-023/FR-024/CR-005);
  затем FR-029 (критический путь волны A).

---

## 2026-09-16 — Аудит реализации всех CR/FR (main `984ca6b`)

- Аудит провёл агент; отчёты — в файлах Changelog соответствующих CR/FR
  документов `docs/change-requests/` (обновления статусов «выполнено» = код в
  main + автотесты зелёные). Ручная приёмка по `docs/ACCEPTANCE.md` §13–14 —
  отдельный процесс владельца.

## 2026-09-18 — Интеграция: merge feature/fr-033-graph-apply → main (все CP слиты)

- **Операция:** слияние CP3 (FR-033 graph_apply) в main поверх интегрированных
  CP0 (лицензионная гигиена), CP2 (FR-032) и CP1 волны A (FR-029); ветки
  fr-027 и fr-032 были уже полностью слиты ранее.
- **Разрешение конфликтов (10 файлов, ~37 гунков):** во всех зонах FR-029
  (model.rs, templates.rs, flow.rs, edgegeom.rs) приоритет полной реализации
  CP1 из main — минимальный контур ветки CP3 уступил по её же оговорке;
  тесты main.rs сохранены ОБЕ стороны (CP1 Instagram MVP + CP3 graph_apply,
  коллизий имён нет); canvas-mcp — счётчик инструментов 22 → 26, ассерты
  FR-032 и FR-033 объединены; манифест cdn — схема CP1 (плоская), выход
  `origin_rps`, дубль ключа `outputs` от автослияния устранён; дубли полей
  в литералах (templates.rs, main.rs, json_canvas_io.rs) вычищены; вызов
  `inbound_slots_with_lines` приведён к 4-арговой сигнатуре CP1.
- **Адаптация CP3:** oracle-тест graph_apply переведён на выход `origin_rps`
  (контракт манифеста CP1) — значения оракула не изменились (20.8333/41.6667
  rps, ±1 %); SPEC §13 и счётчики (26) актуализированы, ACCEPTANCE §22,
  index-cr-fr: fr-033 «реализовано (v1)».
- **Гейты:** cargo fmt — ок; clippy --workspace --all-targets -D warnings —
  ок; cargo test --workspace — 1007 passed, 0 failed.

## 2026-09-18 — Стабилизация CI: таймаут search-тестов 2с → 10с (флейк windows-latest)

- **Диагноз:** merge-коммит f305972 (CP3) уронил CI на windows-latest — все 9
  search-тестов canvas-shell упали по таймауту «событие поиска не пришло:
  Timeout» (search.rs:412, RECV=2с), при зелёных macOS/Linux и зелёных
  watcher-тестах того же бинарника (порог первого события — 5с). Тестовый
  бинарник canvas-shell между зелёным 9fc11f3 и красным f305972 идентичен
  (CP1/CP3 не трогали canvas-shell и Cargo-манифесты) — регрессии нет,
  чистый флейк: медленный раннер отдаёт событие позже 2с. CI на ветку CP3
  не гонялся (мерж пушем в main, без PR) — потому и всплыл только на main.
- **Фикс:** RECV в тестах search.rs 2с → 10с — выше порога watcher (5с) с
  запасом на Windows-раннеры (параллельные тесты, сканирование свежих
  cache.db). Таймаут теста — защита от зависания, не гейт производительности.
- **Гейты:** fmt — ок; clippy --workspace --all-targets -D warnings — ок;
  cargo test --workspace — 1007 passed / 0 failed (порог merge-коммита).
- **Открытые пункты:** флейк-политика «сначала зелёный main»: слияния CP
  в main делать через PR (ci.yml гоняет гейты на pull_request) либо
  локально прогонять полный гейт перед пушем в main.

## 2026-09-18 — Стабилизация CI (продолжение): watcher_debounce_burst + FIRST 5с→10с

- **Диагноз №2:** после фикса search-таймаутов (b6b7718) Windows стал зелёным,
  но ubuntu уронил `watcher_debounce_burst` («шквал должен схлопнуться в ≤2
  батча, пришло 3»). Жёсткое допущение неверно: trailing-дебаунсер ОБЯЗАН
  разорвать шквал на ≥2 батча, если сами 10 записей растянулись больше окна
  DEBOUNCE=300 мс (медленный IO раннера под параллельными тестами) — это
  корректное поведение, не баг. Локально 3 прогона подряд — 129/0.
- **Фиксы (watcher.rs):** 1) лимит батчей производен от длительности шквала —
  `burst_elapsed/DEBOUNCE + 2` (на быстрой машине = исходные 2; не-дебаунсинг
  ловится: 10 отдельных батчей требуют ≥8×DEBOUNCE на записи 10 байтовых
  строк); 2) FIRST 5с → 10с — выровнен с RECV search-тестов.
- **Гейты:** fmt — ок; clippy --workspace --all-targets -D warnings — ок;
  cargo test --workspace — 1007 passed / 0 failed; canvas-shell ×3 — стабильно
  129/0.

## 2026-09-18 — FR-034: перепроектирование MCP-транспорта (ADR-0009) — hermes agent подключается

- **Приказ владельца:** «перепроектировать MCP — hermes agent не может
  нормально подключиться». Уточнено: hermes — на той же машине; транспорт
  нужен спек-совместимый; формат — ADR + реализация.
- **Диагноз** (чтение кода + прогон эмуляции клиентской сессии против
  canvasdesk-mcp, скрипт mcp_client_probe.sh): 5 дефектов моста —
  1) даунгрейд версии: SUPPORTED_PROTOCOLS без 2025-06-18 → клиенту с
  актуальной версией отвечали 2024-11-05, строгие SDK рвут соединение;
  2) batch-массивы (JSON-RPC, spec 2025-03-26) → -32600 «ожидался объект»;
  3) двойная упаковка tools/call: прод-приложение отвечает JSON-RPC-конвертом
  (on_mcp_wake → build_result), мост клал конверт ЦЕЛИКОМ в content[0].text
  (юнит-тест этого не ловил — FakeTransport возвращал чистое значение);
  4) initialize без pipe → JSON-RPC ошибка -32002 и exit(2) — клиент видит
  краш сервера, сессии нет; 5) транспорт фиксировался на старте —
  приложение, поднявшееся позже, не подхватывалось. Дополнительно:
  resources/list, prompts/list, resources/templates/list, logging/setLevel,
  notifications/cancelled → -32601 (хосты зондируют безотносительно
  capabilities).
- **ADR-0009** (docs/adr/adr-0009-mcp-transport-compatibility.md, статус
  «принято»): выбран вариант C — точечное перепроектирование конвертного
  слоя canvas-mcp; отклонены A (сетевой транспорт Streamable HTTP — hermes
  локальный, SPEC §7.6 «только локально»; вернуться отдельным ADR при
  удалённых агентах, с Bearer-токеном) и B (переход на официальный MCP SDK —
  несоразмерно). FR-034 (docs/change-requests/fr-034-mcp-transport-
  compatibility.md) — реализация решения.
- **Реализация (crates/canvas-mcp/src/lib.rs, main.rs):**
  SUPPORTED_PROTOCOLS = [2025-06-18, 2025-03-26, 2024-11-05] (эхо клиентской);
  handle_input — batch-разбор (поэлементно, пустой массив → -32600,
  все-уведомления → тишина); unwrap_app_payload — разворот конверта
  приложения: result → чистый JSON в content[0].text + structuredContent
  (объект), error → isError, isError-результат приложения — насквозь;
  initialize успешен ВСЕГДА (HandleOutcome::Exit удалён — процесс живёт до
  закрытия stdio); refresh_transport — reconnect перед каждым пакетом
  (WaitNamedPipeW 500 мс, без автоспавна); толерантные заглушки read-only
  методов. build_call_result сменил сигнатуру (&str → &Value).
  Автоспавн (FR-008), pipe-сервер canvas-shell и mcp_dispatch (26
  инструментов) — без изменений.
- **Тесты canvas-mcp:** новые batch_requests, call_result_unwrapping,
  offline_handshake_and_calls (вместо pipe_unavailable_scenarios),
  read_only_stubs_and_cancelled; handshake_and_call_with_connected_pipe
  переведён на прод-конверт (envelope в FakeTransport) + structuredContent;
  initialize_protocol_negotiation — эхо 2025-06-18. Итог: 17 тестов крейта.
- **Верификация реальной сессией (probe):** initialize без приложения →
  success + эхо 2025-06-18, exit 0 (было: -32002 + exit 2); batch
  [initialize, ping] → 2 ответа; tools/call offline → isError «не запущен».
- **Доки:** SPEC.md §13 «Транспорт stdio (FR-034, ADR-0009)»; BYOK.md §3 —
  гарантии транспорта, Hermes Agent в списке клиентов; ACCEPTANCE.md §23 —
  8 пунктов чек-листа; index-cr-fr.md — строка FR-034, следующий номер
  FR-035.
- **Гейты:** fmt — ок; clippy --workspace --all-targets -D warnings — ок;
  cargo test --workspace — 1010 passed / 0 failed.

## 2026-09-18 — FR-035: чистота stdout MCP-потока (ADR-0010) — «Invalid JSON \x1b[2m…» у hermes устранён

- **Триггер:** владелец прислал лог hermes после FR-034: сервер регистрируется
  (parked → connected, инструменты видны), но вызовы падают —
  `Invalid JSON: expected value at line 1 column 1,
  input_value='\x1b[2m2026-09-18T11:16:…canvasdesk\\widgets"'`.
- **Диагноз (улика разобрана, цепочка подтверждена кодом):** `\x1b[2m` — ANSI
  escape формата tracing_subscriber; в stdout моста попадали логи
  автоспавненного GUI. 1) `connect_app` спавнил `Command::new(exe).spawn()`
  без Stdio — в Rust это НАСЛЕДОВАНИЕ stdin/stdout/stderr родителя;
  2) GUI инициализировал `tracing_subscriber::fmt()` — writer по умолчанию
  stdout, ANSI включён; 3) старт GUI (версия, реестр виджетов
  `…\canvasdesk\widgets`, wgpu) заливал цветной текст прямо в JSON-RPC-канал;
  4) hermes парсит stdout как newline-delimited JSON → каждая лог-строка =
  Invalid JSON → вызов падает → сессия парковится/возрождается (в логе два
  «revived» за 12 с), цикл. Доп. дефект: автономный `canvasdesk-mcp.exe`
  спавнил current_exe = САМ СЕБЯ (двойник крадёт stdin, рекурсивный спавн).
- **ADR-0010** (docs/adr/adr-0010-mcp-stdio-purity.md, «принято»): вариант C —
  изоляция stdio автоспавна + ориентация спавна + stderr-логи GUI
  (defense-in-depth); отклонены: фильтрация мусора в мосту (лечение симптома),
  отказ от автоспавна (регрессия FR-008), только stderr без изоляции
  (GUI может писать в stdout не через tracing).
- **Реализация canvas-mcp:** `spawn_service_command` — Stdio::null() на все
  три хэндла; `autosprawn_target` — единый `canvasdesk` спавнит сам себя
  (GUI-режим), автономный `canvasdesk-mcp` ищет соседа `canvasdesk.exe` в
  своём каталоге, нет соседа → offline (ADR-0009); `connect_app` переведён на
  хелперы. `#[cfg(any(windows, test))]` — без dead_code на Linux.
- **Реализация canvas-app:** tracing_subscriber → `.with_writer(io::stderr)`
  + `.with_ansi(io::IsTerminal::is_terminal(&stderr()))`; актуализирован
  комментарий перехвата подкоманды `mcp`.
- **Тесты (canvas-mcp 15):** `spawn_service_command_isolates_stdio` —
  поведенческий (unix): ребёнок репортит `[ -c /dev/fd/N ]` по трём fd в файл
  до любого редиректа → «ccc» (/dev/null); `autosprawn_target_…` — мост без
  соседа → None, с соседом → Some(gui), единый бинарь → сам себя, CAPS-стем.
  Нюанс: `Command::get_stdin/get_stdout/get_stderr` НЕ стабилизированы —
  первый вариант теста заменён поведенческим.
- **Probe реальной сессии** (/home/z/my-project/scripts/
  mcp_stdio_purity_probe.sh): initialize (эхо 2025-06-18) + batch + tools/call
  offline + cancelled + мусор; каждая строка stdout — валидный JSON
  (bad=0), контракт FR-034 сохранён. PROBE PASS.
- **Инцидент гейтов:** первый полный `cargo test --workspace` упал
  `ld: signal 7 (Bus error)` на линковке thumbs_smoke — диск 100%
  (target/ = 8.4G). cargo clean (−9 ГБ) → прогон заново.
- **Доки:** SPEC §13 «Чистота stdout (FR-035, ADR-0010)»; BYOK §3 — гарантия
  чистоты канала; ACCEPTANCE §24 (7 пунктов); index-cr-fr — строка FR-035,
  следующий номер FR-036; adr/README.md — добавлены ADR-0009 (пропуск
  прошлой итерации) и ADR-0010.
- **Гейты:** fmt — ок; clippy --workspace --all-targets -D warnings — ок;
  cargo test --workspace — 1012 passed / 0 failed (+2 к FR-034).

## 2026-09-18 — FR-036: wasm-сборка ядра — гейт wasm32-unknown-unknown + исполнение тестов в wasmtime (ADR-0011)

- **Триггер:** приказ владельца «распланировать сборку wasm и реализовать
  сборку для повышения твоей автономности в тестировании». Уточнение скоупа
  (AskUserQuestion): гейт + тесты в wasm-рантайме; таргеты unknown-unknown
  (продукт) + wasip1 (тесты); CI-джоба в ci.yml; коммит сразу в main.
- **Инцидент среды:** контейнер откатился к старому снапшоту (HEAD = 28b4dc8,
  состояние после ADR-0008; ~250 файлов «modified» = смена прав 100644→100755;
  Rust-тулчейн пропал). Восстановлено из origin/main: `git fetch` +
  `git reset --hard origin/main` (= 3a13c33, FR-035) + переустановка rustup
  stable 1.98.1 (rustfmt, clippy). Урок: работы сессий живут только в пуше.
- **Исследование:** план M8 (`docs/plans/wasm-port.md`, §2) уже фиксировал
  wasm-совместимость ядра экспериментом 2026-09-16; W0 (CI-гейт) — открыт.
  Эмпирика на 3a13c33: check ядра под wasm32-unknown-unknown зелёный (55.8 с).
- **Компиляция ≠ исполнение:** wasmtime 48.0.2 (преduccт-бинарь в
  ~/.local/bin) + wasm32-wasip1 → тесты canvas-core падали: паника
  `std::env::temp_dir()` (std на wasm её не реализует) и — после
  первого фикса — паника `std::process::id()` в scene_ops (frame 11
  бэктрейса wasmtime). Флэйк первого прогона маскировал счётчиком «283
  passed» — реальный полный набор 301 (spatial 6 + templates_schema 8 не
  влезли в обрезанный вывод).
- **ADR-0011** (docs/adr/adr-0011-wasm-build-gate.md, «принято»): вариант B —
  гейт (check + rlib) + исполнение тестов ядра в wasmtime; отклонены:
  только W0-check (исполнение не доказано), wasm-bindgen-фасад (W4/W12),
  вынос mcp_dispatch в lib (W2, отдельная задача).
- **Реализация:** `scripts/wasm_gate.sh` (ступени: check ядра → build rlib →
  `RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-core`;
  режим `--check` без wasmtime); `.cargo/config.toml` — runner
  `wasmtime run -S inherit-env --dir .::/ --dir /tmp::/tmp` (только при
  явном `--target wasm32-wasip1`); CI-джоба `wasm-check` (ubuntu, ступень 1 —
  приёмка W0); тестовая песочница `test_scratch_root()` в `#[cfg(test)]`
  lib.rs (натив — temp_dir, wasm — `.wasi-scratch` в CWD) + локальная копия
  в scene_ops.rs (интеграционные тесты не видят cfg(test)-хелперы) + замена
  process::id → SystemTime; `.gitignore` += `.wasi-scratch/`.
- **Результат:** 301/301 тестов canvas-core под wasip1 (wasmtime 48) и
  301/301 нативно (нулевой регресс); rlib ядра 21M; полный
  `scripts/wasm_gate.sh` — OK.
- **Доки:** ADR-0011; FR-036 (docs/change-requests/fr-036-wasm-build-gate.md);
  ACCEPTANCE §25 (7 пунктов); index-cr-fr — строка FR-036, следующий номер
  FR-037; adr/README.md; wasm-port.md — W0 «выполнено» + примечание о
  тест-раннере; AGENTS «Сборка и тесты» — wasm-гейт; SPEC §3 — строка
  wasm-таргетов.
- **Гейты:** wasm-gate — OK (3 ступени + `--check`); fmt — ок; clippy
  --workspace --all-targets -D warnings — ок; cargo test --workspace —
  1012 passed / 0 failed (без изменений к FR-035 — нулевой регресс).

---

## 2026-09-18 — FR-037/ADR-0012: план верификации MCP-реализации в WASM (без реализации)

- **Задача (владелец):** «теперь спланировать реализацию MCP для WASM,
  чтобы можно было проверять не только UI, но и реализацию MCP».
- **Формат:** только планирование — документы и индексы; код не меняется
  (Task ID сессии агента: fr-037-plan).

**Исследование (факты кода, main `b9e011c`):**
- `mcp_dispatch` (main.rs:6074, ~1550 строк, 27 инструментов) — уже чистая
  функция: зависимости только SceneState (mark_dirty/push_undo/
  recompute_flow/move_node/ensure_reserve_at), Camera (4 метода:
  position/zoom/set_center/set_zoom, main.rs:6855–6876),
  TemplateRegistry::builtin() (embedded). `node_create_file` диск не трогает.
- Мост canvas-mcp — чистый протокол за трейтом AppTransport (lib.rs:60) +
  run_stdio (lib.rs:594); платформенное только под cfg(windows). Под
  wasm-таргеты мост НЕ проверялся (гейт FR-036 = core/render/widgets).
- ~40 MCP-тестов в main.rs:12060+ (~2970 строк), включая эталонные гейты
  CP1 (mcp_fr029_instagram_mvp_reference:14183), CP3, CP5 — уже исполняются
  нативно на Linux, но не в wasm-рантайме; end-to-end сессия требует
  Windows pipe + GUI.
- SceneState смешивает модель и ввод: UI-поля selected/selected_nodes/
  dragging = 99 мест доступа; модельные поля — путь доступа self.scene.*
  не меняется при переносе типа.

**Документы (этот коммит):**
- **ADR-0012** (docs/adr/adr-0012-mcp-wasm-verification.md, «предложено»):
  вариант D — крейт canvas-scene (SceneState-модель + модуль mcp; ноль
  новых внешних зависимостей; viewport-зеркало вместо зависимости от
  canvas-render) + canvas-mcp: run_stdio_with_transport<T> + лист-крейт
  canvas-mcp-headless (HeadlessSession за AppTransport, bin для
  wasmtime/wasip1) + драйвер mcp_wasm_e2e.py и гейт mcp_wasm_gate.sh.
  Отклонены: «ничего не выносить» (нет сессии), полный W2 (объём),
  wasm-bindgen-фасад (волна 2, прецедент ADR-0011-C).
- **FR-037** (docs/change-requests/fr-037-mcp-wasm-verification.md,
  «выявлено (план)»): задачи MW1 (canvas-scene, M) → MW2 (мост под wasm,
  S) → MW3 (headless, M) → MW4 (гейт/CI/доки, M) + опции MW5 (инспектор)
  / MW6 (файловый режим); критерии приёмки (≥356 wasm-тестов, oracle ±1%
  эталона №1, нативный регресс 0); 5 открытых вопросов (Q1 имена, Q2
  viewport, Q3 CI-wasmtime, Q4 инспектор, Q5 файлы) с рекомендациями.
- Индексы: adr/README.md += ADR-0012; index-cr-fr.md += FR-037, следующий
  номер FR-038; wasm-port.md — примечание FR-037 (W2 сужается; волна 2
  MCP-мост = HeadlessSession) + §9-строка моста дополнена ссылкой.

**Границы:** продуктовое поведение не меняется; реализация MW1–MW4 не
начата — по плану, каждая задача = сессия = коммит.

## 2026-09-18 — FR-037 MW1: крейт canvas-scene — вынос SceneState + MCP-инструментов + тестов из canvas-app (ADR-0012)

- **Задача:** MW1 плана FR-037 (первая задача реализации; ADR-0012 вариант
  D) — платформенно-нейтральный крейт `canvas-scene` для верификации
  MCP-слоя в wasm; нулевое изменение поведения (те же ассерты, те же имена
  тестов).
- **Сделано:**
  - **`crates/canvas-scene`** (lib, лист в DAG): `scene.rs` — SceneState
    модельного слоя (canvas/spatial/path/dirty_since/undo/redo/
    expr_results/expr_line_results/analysis + viewport-зеркало; поля и
    методы pub) + `Viewport {x, y, zoom}` (дефолт 0/0/1 = Camera::default;
    MIN/MAX_ZOOM-кламп синхронен canvas-render) + `next_free_id`
    (из canvas-app/src/lib.rs, путь `canvas_app::ui::next_free_id`
    сохранён реэкспортом) + seed_canvas/outputs_to_results/
    split_formula_lines/AUTOSAVE_DEBOUNCE/UNDO_LIMIT; `measure.rs` —
    CR-010/CR-012 двухуровневый refit: уровень 1 (оценка) в крейте,
    уровень 2 (точный шейпинг canvas-render) инжектируется через
    `install_measured_reserve` (canvas-scene от canvas-render не зависит);
    `mcp.rs` — диспетчер 27 инструментов + хелперы + FR-033 graph_apply
    (перенос 1:1 из main.rs:5946–7575; viewport_get/set работают со
    значением `Viewport` в SceneState, зум клампится как Camera::set_zoom;
    `DEFAULT_FILE_CARD_W/H` — парные константы вместо canvas_app::ui::
    DROP_CARD_*, паритет — тестом).
  - **canvas-app**: main.rs 15026 → 10352 строк (−4674): вырезаны
    SceneState+impl (551–915), mcp-секция (5946–7575), measure-хелперы
    (256–331, 344–390), outputs_to_results/seed_canvas, константы
    AUTOSAVE_DEBOUNCE/UNDO_LIMIT. UI-поля ввода `selected`
    (Option<Selection>)/`selected_nodes`/`dragging` (Option<DragState>) —
    поля App (73 механические замены self.scene.* → self.*; в canvas-scene
    их нет). `on_mcp_wake` (cfg(windows)): viewport-синк зеркалом (~8
    строк: до диспетчера camera.position()/zoom() → scene.viewport, после
    — set_center/set_zoom; числа идентичны, клампы Camera — тождество,
    поведение не меняется) + чистка выделения после успешного node_delete
    (единственный инструмент, чистивший выделение в старом SceneState —
    индексы сдвинулись). `main()`: инъекция уровня 2 ДО загрузки сцены
    (стартовые высоты точные, как до выноса; headless/wasm — уровень 1).
  - **Тесты**: 50 MCP-тестов перенесены в canvas-scene/src/tests.rs
    (диспетчер, undo-инварианты, формулы/поток, эталоны CP1
    mcp_fr029_instagram_mvp_reference / CP3 graph_apply / CP5
    analyze_bottlenecks) — имена/ассерты без изменений, изменились только
    конструкция viewport (Viewport вместо Camera, значения те же 0/0/1;
    камера ушла из сигнатуры dispatch — viewport живёт в SceneState) и
    пути импортов (MAX_ZOOM, DROP_CARD → константы canvas-scene тех же
    значений). В main.rs остались canvas-render-зависимые тесты точного
    измерения (уровень 2 устанавливается в каждом тесте до создания сцены
    — паритет поведения) + новый паритет-тест
    `measure_layout_consts_match_render` (метрики refit, зум, дефолты
    файловой карточки).
- **Гейты:** fmt ✓; clippy --workspace --all-targets -D warnings ✓;
  cargo test --workspace — 1035 passed / 0 failed (было 1034, +1
  паритет-тест; 50 перенесённых тестов считаются в canvas-scene) —
  нулевой регресс; cargo check --target wasm32-unknown-unknown
  -p canvas-scene ✓; RUST_TEST_THREADS=1 cargo test --target
  wasm32-wasip1 -p canvas-scene — 50/50 в wasmtime (включая oracle-гейты
  эталонов CP1/CP3/CP5); cargo test -p canvas-shell — 129/129 (pipe
  round-trip). Инцидент линковки (диск 84 %, повтор FR-035) — почищен
  target/debug/incremental + устаревшие canvasdesk-бинари, депы
  сохранены.
- **Дальше (план FR-037):** MW2 — мост canvas-mcp под wasm
  (run_stdio_with_transport); MW3 — canvas-mcp-headless; MW4 — гейт/CI/
  документация (AGENTS/SPEC/ACCEPTANCE НЕ тронуты этим коммитом —
  задача MW4).
- **Ребейс на параллельную волну владельца (доведён этой сессией):**
  после коммита MW1 в origin/main параллельной сессией владельца была
  влита волна CP4/CP6 (867bf8d визуализация проливания, f43eb44 CP4
  рецепт агента, 15b228a CP6 what-if FR-017, merge 83d43a9). Ребейс MW1
  поверх 83d43a9: конфликт main.rs (CP6 добавлял ~800 строк what-if в
  вырезанную MW1 область) разрешён интеграцией CP6-кода в пост-MW1
  структуру — App получил WhatIfOverrideRow и панель what-if поверх
  scene-модели. Вью-типы `SpillView`/`WhatIfNode` (чистые данные, без
  GPU/шрифтов) перенесены из canvas-render в новый
  `canvas-scene/src/view.rs`; canvas-render зависит от canvas-scene и
  реэкспортирует (слои: core → scene → render → app). +3 теста
  view/паритета в tests.rs. Итог: main.rs 11171 (10352 + CP6);
  workspace 1077/0 (1035 + 42 теста CP6 what-if); wasip1 canvas-scene
  53/53 в wasmtime; fmt/clippy чистые. Инцидент диска (третий:
  100 % при сборке тестов) — cargo clean + локальный прогон гейтов с
  CARGO_PROFILE_DEV_DEBUG=0/CARGO_PROFILE_TEST_DEBUG=0 (на семантику
  тестов не влияет; в CI профили дефолтные).

## 2026-09-18 — FR-037 MW3: canvas-mcp-headless — headless MCP-сервер в wasmtime (ADR-0012)

- **Задача:** MW3 плана FR-037 — headless MCP-сервер для реальных сессий
  в Linux-контейнере без Windows/GUI (приказ владельца «работай сам»).
- **Сделано:**
  - **`crates/canvas-mcp-headless`** (lib + bin, лист в DAG — паттерн
    canvas-web §3.5 M8; нативный граф canvasdesk.exe не затронут):
    `HeadlessSession` — SceneState (in-memory, пустой канвас, модель
    собирается graph_apply, как в гейтах эталонов) +
    `TemplateRegistry::builtin()` за `AppTransport`. `send_line`
    воспроизводит конверт `on_mcp_wake` буквально: parse_envelope →
    (id) → mcp_unwrap_call → mcp_dispatch → build_result |
    build_call_error; notification (id == None) — тишина (recv → None,
    мост трактует как таймаут — семантика прод-приложения); битый
    конверт — build_error с null-id. Viewport-зеркало уже в SceneState
    — синк с Camera не нужен (ADR-0012). Bin `canvasdesk-mcp-headless`
    = run_stdio_with_transport (reconnect — no-op, in-process).
  - **12 lib-тестов протокольного цикла** (через мост handle_line/
    handle_input, не напрямую в диспетчер): initialize-эхо 2025-06-18;
    tools/list = 36; tools/call roundtrip (text + structuredContent,
    FR-034); CP3-гейт graph_apply мини-эталон №1 — oracle ±1 % (208.33
    rps / CDN W 34.29 ms ρ 0.417 / origin 20.83 / GW W 3.2 ms / смета
    86 — те же числа, что canvas-scene); CP5-гейт ρ-лестница (базовая
    none 0.417 → DAU×2 warn 0.833 → DAU×5.35 overload 2.229, бейджи);
    негативные ветки: isError неизвестного инструмента, isError ошибки
    внутри инструмента, −32601, −32700 (handle_input), batch из 2,
    notification-тишина; персистентность состояния между вызовами.
  - Нюансы реализации тестов (зафиксированы в комментариях): тексты с
    переносами строк — только через serde_json-маршаллинг (сырые \n в
    format!-конверте = parse error control-character); массивные
    результаты (nodes_list) приходят в text-контенте — structuredContent
    мост даёт только объектным результатам (FR-034).
- **Гейты:** fmt ✓; clippy --workspace --all-targets -D warnings ✓
  (мин-правка len_zero → is_empty); cargo test --workspace — 1089/0
  (1077 + 12 headless); cargo check --target wasm32-unknown-unknown
  -p canvas-mcp-headless ✓; RUST_TEST_THREADS=1 cargo test --target
  wasm32-wasip1 -p canvas-mcp-headless — 12/12 в wasmtime; **ручная
  сессия в wasmtime** (критерий приёмки MW3): initialize (эхо
  2025-06-18, serverInfo canvasdesk) → tools/list (36) → tools/call
  canvas_info (structuredContent, isError=false) → EOF — штатный выход
  (exit 0).
- **Дальше:** MW4 — mcp_wasm_e2e.py (драйвер) + mcp_wasm_gate.sh (гейт
  одной командой) + CI wasm-check (компиляция трёх крейтов) + AGENTS/
  SPEC/ACCEPTANCE; опции MW5/MW6 — по решению владельца.

## 2026-09-18 — FR-037 MW4: гейт, CI и документация — FR-037 закрыт (MW1–MW4)

- **Задача:** MW4 плана FR-037 — верификационные артефакты и синхронизация
  документации; закрытие FR (приказ владельца «работай сам»).
- **Сделано:**
  - **`scripts/mcp_wasm_e2e.py`** (драйвер реальной MCP-сессии, ноль
    внешних зависимостей): build wasip1 → wasmtime run → сценарий:
    initialize (эхо 2025-06-18) → tools/list (36) → graph_apply
    мини-эталон №1 (oracle ±1 %: 208.33 rps / CDN W 34.29 ms ρ 0.417 /
    origin 20.83 / GW W 3.2 ms / смета 86) → analyze_bottlenecks
    ρ-лестница CP5 (none 0.417 → warn 0.833 → overload 2.229, бейджи) →
    негативные ветки (isError, −32601, batch из 2, notification-тишина —
    проверена трюком «следующий ответ уже на ping») → EOF stdin —
    штатный exit 0. Лог сессии (каждый конверт с меткой времени) —
    target/tmp/mcp_wasm_session.log. select-таймаут 60 с на ответ.
  - **`scripts/mcp_wasm_gate.sh`** (3 ступени, паттерн wasm_gate.sh):
    1/3 check wasm32-unknown-unknown (scene/mcp/headless); 2/3 wasip1-
    тесты (RUST_TEST_THREADS=1: 53+13+12=78 в wasmtime); 3/3 e2e-драйвер.
    Режим --check — только компиляция (эквивалент CI).
  - **CI** `wasm-check` += `-p canvas-scene -p canvas-mcp-headless`
    (компиляция; исполнение — локально, прецедент ADR-0011).
  - **Доки:** AGENTS (структура workspace: canvas-scene/
    canvas-mcp-headless; раздел «Сборка и тесты»: MCP-wasm-гейт);
    SPEC §3 (строка wasm-таргетов: MCP-слой) + §3-дерево + §13
    (подраздел «Headless-верификация MCP»); ACCEPTANCE §29 (чек-лист
    FR-037, 10 пунктов); FR-037: статус «реализовано (MW1–MW4)», MW4
    «Выполнено», changelog; ADR-0012 → «принято»; wasm-port.md —
    примечание актуализировано (W2-вынос исполнен).
- **Гейты:** `scripts/mcp_wasm_gate.sh` — полный зелёный прогон, exit 0
  (78 wasip1-тестов + e2e-сессия сошлась по всем оракулам); fmt ✓
  (Rust-код не менялся с MW3 — workspace 1089/0 остаётся в силе).
- **FR-037 закрыт.** Опции MW5 (инспектор-сессия владельца)/MW6 (файловый
  режим headless) — по решению владельца (рекомендации агента: Q4 да/Q5
  нет). Открытые пункты роадмапа: продуктовый веб-слой S5 (W1–W12 M8).
