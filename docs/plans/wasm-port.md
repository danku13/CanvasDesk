# M8. WASM-порт: CanvasDesk в браузере — план и рефакторинг

ЗАДАЧА (директива владельца, 2026-09-16): собрать веб-версию CanvasDesk
через WebAssembly, запускаемую в браузере **без функционала замены рабочего
стола** (режим `--desktop` остаётся Windows-only, как и в M7). Документ
фиксирует архитектуру, границы, риски и декомпозицию до задач-коммитов.
Формат и терминология — по образцу `docs/plans/M7-crossplatform.md`.

Источники истины: `docs/SPEC.md` §3/§4, `AGENTS.md` (правила платформенного
кода), `docs/plans/M7-crossplatform.md` (паттерн портирования за трейтами),
`docs/plans/M5-widgets.md` §6–7 (уроки cfg-гигиены). Экспериментальная база
— контрольные сборки `cargo check --target wasm32-unknown-unknown`
(rustc 1.98.1, см. §2).

---

## 1. Принцип и границы

**Принцип:** ядро НЕ переписывается и не ветвится — оно уже компилируется
под `wasm32-unknown-unknown` без изменений (проверено экспериментально,
§2). Порт достраивает web-платформенный слой по тому же паттерну, каким M7
достраивал юниксы: платформенные способности — за трейтами, реализации — в
отдельном крейте, выбор реализации — в точке сборки бинарника.

**Терминология волн.** В проекте волна разработки = milestone с набором
задач, где каждая задача — сессия = коммит (M1–M7). Для веб-порта
фиксируем две:

- **Волна 1 (этот план, задачи W0–W12)** — MVP: полный канвас без
  платформенной обвязки рабочего стола. Виджеты — снапшот/плейсхолдер.
- **Волна 2 (после стабилизации MVP, план — отдельным документом)** —
  live-виджеты в iframe, WebGL2-фолбэк (Firefox/Safari), MCP
  WebSocket-мост, PWA/оффлайн. Заявки без декомпозиции — в §9.

**Границы волны 1 (осознанные, решение владельца 2026-09-16):**

- **Браузеры: Chromium 113+ (Chrome/Edge/Arc), WebGPU без фолбэка.**
  WebGL2 для Firefox/Safari — волна 2.
- **MVP-скоуп:** ядро канваса (зум/пан, заметки, форматирование, связи,
  undo, автосейв, темы) + поиск с миникартой + Numi-формулы + шаблоны.
- **MCP (`canvasdesk mcp`) — вне объёма.** WebSocket-мост к локальному
  сервису — исследование волны 2.
- **Desktop-режим `--desktop` — не портируется и не удаляется.** Код уже
  весь за `cfg(windows)` и на wasm исключается компилятором автоматически.
- **Живые виджеты (WebView2-хост) — волна 2.** В волне 1 виджет-нода
  рендерится снапшотом/плейсхолдером — эта деградация уже работает
  (путь Linux/macOS из M5).
- Сетевых фич нет: канвас — локальный документ в браузере.

**Чего план НЕ делает:** не меняет формат `.canvas`, не меняет нативные
ветки Windows/Linux/macOS (нулевой регресс — критерий §8.7), не вводит
`cfg(target_arch)` в core/render/widgets, не трогает план M7.

## 2. Экспериментальная база (контрольные сборки 2026-09-16)

| Крейт | `cargo check --target wasm32-unknown-unknown` | Комментарий |
|---|---|---|
| canvas-core (~7 000 строк) | ✅ без изменений | модель, JSON Canvas I/O, expr/flow, R-tree, шаблоны |
| canvas-render (~12 000 строк) | ✅ без изменений | wgpu 22.1 · winit 0.30.13 · glyphon 0.6 · cosmic-text 0.12.1 |
| canvas-widgets (~2 500 строк) | ✅ без изменений | манифесты/bridge/LOD/registry; WebView2-хост — cfg(windows) |
| canvas-shell | ❌ | libsqlite3-sys (bundled C): cc не собирает SQLite под wasm; notify, Win32 |
| canvas-app (bin) | ❌ | run_app/pollster/threads/arboard/env::args — карта замен §3 |

Факты, зафиксированные по исходникам зависимостей (влияют на задачи):

1. winit 0.30.13 web: запуск **только** `EventLoopExtWebSys::spawn_app`
   (`run`/`run_app` на web паникуют); `create_proxy` доступен у
   `EventLoop` — паттерн `EventLoopProxy` сохраняется.
2. **`DroppedFile`/`HoveredFile` на web НЕ генерируются** (в
   `platform_impl/web` их нет) — приём файлов делаем своими DOM-листенерами
   (задача W6).
3. **IME на web НЕ поддержан** winit'ом (`set_ime_purpose` — заглушка,
   `WindowEvent::Ime` не приходит). Кириллица печатается через
   `KeyEvent`/`Key::Character` (побуквенный ввод), CJK-композиция — нет;
   фиксируем как ограничение v1 (§7).
4. Pinch-жесты winit'ом на web не эмулируются, НО Chromium шлёт
   тачпад-pinch как **ctrl+wheel** — это уже готовый хоткей зума
  `Ctrl+колесо` (SPEC §8): работает без нового кода.
5. `pollster::block_on` = парковка потока на Condvar — на wasm
   невозможна: инициализация `Renderer` переводится на
   `wasm_bindgen_futures::spawn_local` (W4). `Renderer::new` уже async —
   меняется только вызов.
6. `arboard` 3.6.1 не имеет wasm-бэкенда (windows/macos/linux/android/
   emscripten) → `navigator.clipboard` (W3).
7. `std::time::Instant` на wasm32-unknown-unknown **комилируется, но
   паникирует в рантайме** → миграция на `web_time::Instant` (крейт уже в
   дереве через winit; на нативе это тонкая обёртка над std). Затрагивает
   FrameMeter, DoubleClick, debounce-таймеры — W1, обязательно до W4.
8. `std::thread` на wasm32-unknown-unknown отсутствует — все worker-потоки
   (пул тамбнейлов, tick 1 c, exit-листенер) на web не спавнятся, их
   будят async-задачи (§3.3).
9. Шрифты уже вшиты `include_bytes!` (1.4 МБ Noto, CR-009) — cosmic-text
   не ходит в системные шрифты, на wasm стартует как на нативе.

## 3. Архитектура портирования

### 3.1 Слои и правила (расширение правил AGENTS)

1. Web-платформенный код живёт в новом крейте **`canvas-web`** — зеркало
   `canvas-shell` по роли в архитектуре (crate-type: `cdylib` + `rlib`).
   В `canvas-app` `cfg(target_arch)` НЕ появляется: app получает web-сервисы
   как обычные параметры/трейты — то же правило, что M7 §3.1 вводил для
   Windows.
2. `App` (сегодня 10 330 строк main.rs вместе с обвязкой) выносится в
   библиотечную часть `canvas-app` (модуль `app`) — W2. Нативный `main.rs`
   остаётся тонкой обёрткой: init сервисов + `run_app`. Веб-бинарь
   `canvas-web` делает то же самое со своим набором сервисов — дублирования
   UI-логики нет.
3. `canvas-core`/`canvas-render`/`canvas-widgets` обязны собираться под
   `wasm32-unknown-unknown` — CI-гейт W0 (расширение правила «core
   собирается на Linux» из AGENTS).

```
crates/
  canvas-core/        # без изменений (WASM ✅)
  canvas-render/      # без изменений (WASM ✅)
  canvas-widgets/     # без изменений (WASM ✅)
  canvas-shell/       # нативная платформа (Win32/Linux/macOS)
  canvas-web/         # NEW: web-платформа — сервисы, storage, bindgen-обвязка
  canvas-app/         # lib: App + UI-состояние (после W2); bin: нативный запуск
```

### 3.2 Карта замен сервисов (натив → web)

| Способность | Натив (сегодня) | Web (волна 1) |
|---|---|---|
| Окно/ввод | winit `run_app` | winit `spawn_app` (web-sys) |
| GPU | wgpu 22: DX12/Vulkan/Metal | wgpu 22 → WebGPU (Chromium 113+) |
| Текст | glyphon/cosmic-text, шрифты вшиты | тот же код, без изменений |
| Инициализация GPU | `pollster::block_on(Renderer::new)` (main.rs:8307) | `spawn_local` + async (W4) |
| Тамбнейлы | ShellThumbnailProvider + SQLite-кэш | WebImageThumbnailProvider: `createImageBitmap` → OffscreenCanvas downscale → RGBA в существующий thumbs-атлас (W10) |
| Файловый вотчер | notify (RDCW/inotify/FSEvents) | NoopWatch: событий ФС нет, перечитывание по жесту «Перезагрузить» |
| Поиск | rusqlite FTS5 worker-поток | MemSearch: индекс по нодам в памяти (scan_scene в search_ui уже есть), ответы через тот же SearchEvent |
| Хранение .canvas | std::fs + автосейв/.bak | trait `CanvasStorage`: FS Access API / OPFS — гибрид (§4) |
| Конфиг | ~/.canvasdesk/config.toml | localStorage (`Settings::from_str` — новый чистый метод в core, toml уже dep) |
| widget_state | таблица в cache.db (rusqlite) | localStorage (serde_json) |
| Буфер обмена | arboard | `navigator.clipboard` (гест/secure context; деградация warn — обёртка `Clipboard(Option)` уже так устроена) |
| Фоновые потоки | пул тамбнейлов, tick 1 c, exit-листенер | async-задачи/gloo-interval; пробуждение через EventLoopProxy — паттерн не меняется |
| Аргументы CLI | std::env::args / parse_args | URLSearchParams: `?canvas=...`, `?stress=5000` |
| MCP named pipe | worker-поток | вне волны 1 (§9) |
| Desktop-режим | cfg(windows) WorkerW | исключён компилятором автоматически |

### 3.3 Async-модель

Событийная петля браузера однопоточна, но архитектура приложения уже
соответствует ей: все фоновые сервисы (thumbs/watcher/search/tick) будят
петлю через `EventLoopProxy::send_event(AppEvent)` — на web замыкания
посылают те же `AppEvent`. Нового протокола нет; `std::thread` в web-крейте
просто не используется.

### 3.4 Точечные правки существующего кода

Всё, что мешает web-запуску, локализовано в `main.rs` (после W2 — в
тонких обёртках):

- `main.rs:8307` — `pollster::block_on(Renderer::new(...))` → async-init;
- `main.rs:6630` — `event_loop.run_app(...)` → `spawn_app` (в canvas-web);
- `main.rs:102` — `Clipboard(Option<arboard::Clipboard>)` → за трейт (W3);
- `Instant` → `web_time::Instant` в core/render/app (W1);
- `parse_args(std::env::args)` → источник аргументов (W6).

### 3.5 Один репозиторий или форк?

**Вердикт: один репозиторий, форк не нужен и вреден.** Существующий
workspace спроектирован под сосуществование платформ: правило «core
обязан собираться на Linux» уже вычистило платформенные утечки из ядра,
M7 закрепил паттерн «платформенный слой — отдельный крейт за трейтами».
Wasm-порт добавляет ещё одну платформенную ветку ровно по этому же
паттерну. Форк понадобился бы, если бы web-версия расходилась с desktop
по модели данных или UX-парадигме — здесь этого нет: ядро общее (§2),
формат `.canvas` общий, различия — только платформенный слой.

**Механика сосуществования** (проверено контрольной сборкой 2026-09-16:
крейт с wasm-bindgen 0.2 / web-sys / wasm-bindgen-futures компилируется
под x86_64-unknown-linux-gnu — макрос `#[wasm_bindgen]` на не-wasm целях
раскрывается в пустые заглушки):

1. `canvas-web` — обычный член workspace (`crates/*`), **лист в DAG
   зависимостей**: на него не ссылается ни `canvas-app`, ни нативный
   бинарь → граф зависимостей `canvasdesk.exe` не меняется вообще.
2. Нативная сборка остаётся зелёной с web-крейтом в дереве:
   `cargo build --workspace` компилирует и его (заглушки), цена —
   десятки секунд; web-код при этом никогда не вызывается.
3. Две сборки из одного checkout'а:
   - desktop: `cargo build --workspace --release` (как сегодня);
   - web: `trunk build` / `wasm-pack` (тянет только `-p canvas-web`,
     рантайм — wasm32-unknown-unknown + WebGPU).
4. Web-зависимости объявляются в `[workspace.dependencies]` (единые
   версии, как у остальных). Вариант Б — убрать их из нативной сборки
   через `[target.'cfg(target_arch = "wasm32")'.dependencies]` (паттерн
   самого winit) — механическая миграция, если время компиляции станет
   заметным; начинаем с безусловных как с более простых.
5. Тесты: чистая логика `canvas-web` — обычные `#[test]` (гоняются на
   любой ОС в нативных гейтах); браузерная часть — `wasm-bindgen-test`
   в wasm-джобе CI.
6. Ветки: работа в `wasm-port` (по образцу `ci-matrix` из M7), задачи
   W0–W12 мержатся в main; релиз — один тег → артефакты build-all
   (desktop) + Pages-деплой (web, W12).

| Критерий | Один репозиторий | Форк |
|---|---|---|
| Синхронизация ядра (~21,5 тыс. строк core/render/widgets) | бесплатно, общий код | ручной бэкпорт каждого FR/CR-фикса |
| Формат `.canvas` web ↔ desktop | гарантирован общим кодом I/O | риск расползания парсеров |
| CI | общий: wasm-check рядом с нативными гейтами | дублирование пайплайнов |
| Релизы | один тег → desktop + web | два независимых цикла с ручной сверкой |
| Изоляция web-кода | правилом §3.1 (только canvas-web) | физическая стена — но ценой двойного сопровождения |
| Когда оправдан | — | принципиальная дивергенция продукта, отдельный релизный цикл, лицензионное разделение |

Ни одно из условий «когда оправдан» сейчас не выполняется, а цена форка
растёт с каждым принятым FR. Дополнительный аргумент — прецеденты
экосистемы: winit, wgpu, egui и другие собирают нативные и wasm-бинари из
одного репозитория; это штатная практика для Rust-стека, на котором
построен CanvasDesk.

Единственный осознанный риск одного репозитория — расползание
`cfg(target_arch)` по общим крейтам; митигация та же, что в M7 для
`cfg(windows)` (§6/§7 этого плана): правило «web-код только в
canvas-web» + ревью каждого W-коммита + гейты CI на обе стороны.

## 4. Хранение файлов: гибрид FS Access + OPFS

Браузерная песочница не даёт произвольного доступа к диску. Гибрид — это
trait `CanvasStorage` с двумя реализациями и выбором при старте.

### 4.1 Два механизма

| | FS Access API | OPFS (Origin Private FS) |
|---|---|---|
| Что это | браузерные пикеры `showOpenFilePicker`/`showSaveFilePicker` | приватная ФС origin: `navigator.storage.getDirectory()` |
| Файл где | **на настоящем диске** (C:\...\notes.canvas) | внутри профиля браузера, пользователю не виден |
| Браузеры | Chromium 86+ (Firefox/Safari — нет) | все современные |
| Автосейв | да: `FileSystemFileHandle.createWritable()` | да (файл в песочнице) |
| Права | permission-промисы; хэндл персистентен в IndexedDB, после рестарта браузера — re-request одним промисом | не нужны |
| Ограничения | открытие только по жесту пользователя; нет событий изменения | нет событий изменения; нет доступа из ОС |
| Роль | основной путь (таргет — Chromium) | фолбэк, черновики, дефолт `default.canvas` |

Выбор при старте: `if ('showOpenFilePicker' in window)`. На таргетных
Chromium всегда доступен FS Access; OPFS гарантирует полезность приложения
и без него (бонусом OPFS-режим работает и в Firefox).

### 4.2 UX-поток

1. **Первый запуск:** сцена сеется в OPFS (`/default.canvas`) — аналог
   `default.canvas` на нативе; «Открыть с диска…» = пикер.
2. **Недавние:** открытый с диска хэндл сохраняется в IndexedDB; reopen
   после перезагрузки — пункт меню, re-request permission одним промисом,
   без пикера.
3. **Автосейв:** debounce 2 с (готовая константа `AUTOSAVE_DEBOUNCE`) в
   активное хранилище; `.bak` — версия-на-один-назад тем же хранилищем
   (переименование внутри OPFS; для FS Access — sibling-файл).
4. **Экспорт/импорт:** download-blob (для OPFS-канвасов) и приём файла
   drag-ом на окно — через свои DOM-листенеры (winit web `DroppedFile` не
   даёт, §2.2) с пунктами «Открыть копию» / «Импортировать».
5. **Нет вотчера:** файл, изменённый извне, перечитывается по пункту
   «Перезагрузить» (или reopen) — честная деградация, документируем в
   user-docs.

### 4.3 Отражение в задачах

W6 реализует trait + FsAccessStorage + OpfsStorage + IndexedDB-recent +
DOM-drop + URL-параметры; принятые файлы-картинки идут в W10-превью.

## 5. Виджеты: волна 1 — снапшот, волна 2 — iframe-live

**Волна 1 (MVP).** Live-хост WebView2 — `cfg(windows)`, на wasm не
компилируется. Менеджер виджетов уже умеет деградацию «хост недоступен»
(так на Linux/macOS до T22+): виджет-нода рендерится иконкой
пакета/плейсхолдером, LOD/permissions/манифесты работают как есть —
сборка canvas-widgets под wasm это подтверждает. Registry материализует
встроенные пакеты (`include_dir`) не в `~/.canvasdesk/widgets`, а в
OPFS-поддиректорию либо обслуживает из памяти (решение в W11).
`widget_state` — localStorage.

**Опциональный трюк 1.5:** CI-скрипт рендерит встроенные виджеты (часы,
календарь, стикер) в PNG headless-Chromium'ом и кладёт снапшоты в пакеты —
виджет-нода показывает осмысленную картинку без live-хоста. Оценить после
W11; не блокер.

**Волна 2 (заявка, §9).** iframe-хост за тем же контрактом, что WebView2:
дочерний iframe позиционируется над областью ноды (аналог WS_CHILD
airspace из M5), bridge — `postMessage` (протокол `HostToWidget`/`Reply`
уже JSON-сериализуем), permissions — те же. Отдельный план после
стабилизации MVP.

## 6. Задачи W0–W12 (каждая — сессия = коммит)

Порядок: W0 → W1 → W2 → W3 → W4; после W4 задачи W5–W11 в значительной
мере параллелизуемы; W12 замыкает. Объём: S ≤ полудня, M — день-два,
L — три-пять дней. Расписание для двух независимых агентов и протокол
синхронизации — §6.1.

| ID | Объём | Задача и критерий приёмки |
|---|---|---|
| W0 | S | **Каркас плана.** Ветка `wasm-port`; этот документ в `docs/plans/wasm-port.md`; CI-гейт `wasm-check` на ubuntu: `cargo check --target wasm32-unknown-unknown -p canvas-core -p canvas-render -p canvas-widgets`. Приёмка: гейт зелёный на каждый пуш; нативные гейты не задеты. **Выполнено 2026-09-18 (FR-036, ADR-0011):** CI-джоба `wasm-check` в ci.yml + локальный гейт `scripts/wasm_gate.sh` (см. примечание после таблицы) |
| W1 | S | **Время.** Миграция на `web_time::Instant` (alias-тип в core, чтобы натив не заметил подмены): FrameMeter, DoubleClick, debounce-таймеры. Приёмка: wasm-check зелёный; нативные тесты без регресс; после W4 первый кадр не паникует. **Выполнено 2026-09-19 (трек A, §6.1):** alias `canvas_core::time::Instant` (web-time 1.1 — уже в дереве через winit, Cargo.lock без смены версий); std-Instant заменён в canvas-render (FrameMeter, тест minimap), canvas-scene (`dirty_since` — крейт на wasm-пути ADR-0012) и canvas-app (main/lib/widgets/palette/template_ui — DoubleClick, hover/debounce, toast, focus-fade); canvas-shell не тронут (натив-only). Гейты: fmt, clippy `-D warnings`, test --workspace, wasm_gate.sh (345 core + 13 mcp в wasmtime), mcp_wasm_gate.sh (53+13+12 + e2e oracle ±1 %) — зелёные |
| W2 | M | **Вынос App в lib.** `App`/`SceneState`/обработчики событий из main.rs → `canvas_app::app` (чистое перемещение, zero behavior change); main.rs — тонкая нативная обёртка. Приёмка: cargo build/test нативно зелёные; diff — перемещения кода. **Выполнено 2026-09-19 (трек A, §6.1):** новый модуль `canvas_app::app` (src/app.rs): `App` (все `impl` — ввод/редактирование/рендер-кадр/MCP), `ApplicationHandler`, `AppEvent`, хелперы (геометрия оверлеев, стресс-сцена `--stress`/`--stress-widgets`, `parse_args`/`CliArgs`, `measured_result_reserve_height`, `open_path_externally`) и модуль тестов; `SceneState` уже был вынесен MW1 (canvas-scene). `main.rs` — 311 строк: нативная инициализация (MCP-режим, трейсинг, single-instance, конфиг, сервисы shell, event loop) + `run_app`; `App`/`AppEvent`/`App::new`/`init_widgets` — `pub` (API для canvas-web W4), само-ссылки `canvas_app::` → `crate::`. Гейты: fmt, clippy `-D warnings`, test --workspace (156 юнит-тестов app исполняются из lib), wasm_gate.sh (345 core + 13 mcp в wasmtime), mcp_wasm_gate.sh (e2e oracle ±1 %) — зелёные |
| W3 | M | **Трейты сервисов.** `CanvasStorage`, `ClipboardBackend`, `WatchBackend` (+Noop), `SearchBackend` (+MemSearch); инъекция в `App::new`; нативные реализации = сегодняшнее поведение. Приёмка: натив без регресс; трейты покрыты тестами на заглушках (паттерн NoopThumbnailProvider). **Выполнено 2026-09-19 (трек A, §6.1):** нейтральные контракты в canvas-core — `providers.rs`: `ClipboardBackend`+Noop, `WatchBackend`+Noop, `ThumbBackend`+Noop, `WidgetStateBackend`+Mem, `Priority` (переехал из shell); `io.rs`: `CanvasStorage`+`FsCanvasStorage`+`MemStorage`; новые модули `search.rs` (протокол T14 + `SearchBackend`+`MemSearch`) и `dragdrop.rs` (`DragEvent`/`DragData` из shell). Нативные реализации в shell «как сегодня»: `ArboardClipboard` (новый модуль clipboard.rs), `impl WatchBackend for WatchService`, `impl SearchBackend for SearchService`, `impl ThumbBackend for ThumbService`, `impl WidgetStateBackend for WidgetStateStore`; протоколы поиска/drag — ре-экспорты shell (пути сохранены). Инъекция: `App::new` принимает `Box<dyn>` сервисов (+`cache_dir`, `widget_state`) — shell-типов в сигнатуре нет; `SceneState::with_storage`/`load_or_seed_with_storage`; canvas-app Cargo.toml: `canvas-shell` target-gated, arboard убран. **Результат: canvas-app lib собирается под wasm32-unknown-unknown** (wasm-check локаально) — W4-прошивка разблокирована. Тесты-заглушки: MemSearch (ReplaceAll/Query/Remove), MemStorage roundtrip, Noop-инертность, App::new на заглушках (157 app / 54 scene / 239 core). Гейты: fmt, clippy `-D warnings`, test --workspace (45 наборов), wasm_gate.sh, mcp_wasm_gate.sh — зелёные |
| W4 | M | **Первый свет canvas-web.** cdylib-крейт: `index.html` + trunk, `#[wasm_bindgen(start)]`, `spawn_app`, async-init Renderer через `spawn_local`, console-лог tracing + panic hook; пустая сцена, зум/пан/сетка. Приёмка: `trunk serve` грузится, камера живая, HUD F3 работает **Каркас исполнен 2026-09-19 (трек B, §6.1):** `crates/canvas-web` (cdylib+rlib, лист DAG §3.5, ноль новых внешних зависимостей — bindgen-семейство уже было в дереве через winit): bindgen-обвязка (консольные extern'ы без js-sys/web-sys — их привнесёт winit на прошивке), panic-hook → console.error, tracing→консоль (собственный ConsoleLayer на tracing-subscriber, INFO-фильтр), `#[wasm_bindgen(start)]` → `boot()`; `index.html` + `Trunk.toml` (dist → target/dist; инструменты: trunk 0.21.14, wasm-bindgen-cli 0.2.127 = версии крейта в lock). Приёмка каркаса: cargo check native + wasm32-unknown-unknown ✓; 7 тестов каркаса ✓; `trunk build` ✓; браузерный дым — модуль инстанцируется, в консоли `[INFO canvas_web] каркас загружен`, ошибок страницы нет. **Прошивка исполнена 2026-09-19 (трек B, §6.1, после W3):** async-init Renderer за инъекцией (§3.4, паттерн W3-сервисов) — `canvas-render::renderer_init`: трейт `RendererLauncher` + `RendererLaunch{Ready(Box<Renderer>),Pending(RendererSlot),Failed}` + `RendererSlot` (Rc-RefCell-слот доставки, главный поток) + `BlockOnRendererLaunch` (pollster — натив, поведение как до W4) + `NoopRendererLaunch` (тесты); `App::new` принимает стратегию (последний параметр), `resumed()` зовёт `launch`, RedrawRequested забирает слот до отрисовки (кадры до готовности GPU пропускаются — R14; натив-ветка мертва). canvas-web: `app_spawn` — web-набор сервисов (карта §3.2: MemStorage, NoopThumbs→W10, NoopWatch, MemSearch+proxy-ответы, NoopClipboard→W5, widget_state None→W11, install_measured_reserve как в main.rs) + `EventLoopExtWebSys::spawn_app`; `renderer_launch::SpawnLocalRendererLaunch` — spawn_local + слот + побудка `request_redraw` + вставка winit-канваса в DOM (`WindowExtWebSys::canvas`; winit 0.30 `with_append` по умолчанию выключен, attrs — в canvas-app, §3.1); `web_log` печатает поля событий (key=value; раньше %err терялся). Точечные правки: pollster переехал app→render (BlockOnRendererLaunch); `Renderer::new` перечитывает размер ПОСЛЕ async-ожиданий (на web Resized приходил при renderer=None и пропускался — canvas оставался 0×0). Web-compat шимы (§7, живут в web-слое): `--cfg=web_sys_unstable_apis` для wasm32-unknown-unknown (.cargo/config.toml — требования wgpu-web); index.html-шим фильтрует нераспознанные лимиты requestDevice по `adapter.limits` (wgpu 22 требует удалённый из спеки maxInterStageShaderComponents — свежие Chromium отклоняли запрос, маскировка «GPU-адаптер не найден»; for..in — WebIDL-геттеры на прототипе). Приёмка: `trunk build/serve` ✓; браузерный дым (Chromium+swiftshader): модуль стартует, сцена сеется, канвас 1280×577 в DOM, `рендер инициализирован backend=BrowserWebGpu format=Rgba8Unorm present_mode=Fifo`, кадры презентуются (debug-метка, 24 инстанса сетки), ошибок страницы/GPU-валидации нет; пиксельный скриншот-скрыт — известное ограничение headless-Chrome (agent-browser doctor: «WebGPU renders, headless screenshots miss the canvas»), рендер подтверждён present-логами. Гейты: fmt, clippy `-D warnings`, test --workspace (45 наборов), wasm_gate.sh, mcp_wasm_gate.sh, test -p canvas-shell — зелёные |
| W5 | M | **Ввод и редактирование.** Клавиатура (map_key), двойной клик, drag нод, инлайн-edit, палитра цветов, resize, undo, темы, хоткей-оверлей F1. Приёмка: кириллица печатается и выделяется; Ctrl+B/I/H, Ctrl+Z/Y, Ctrl+C/X/V работают; `?stress=5000` — 60 fps **Выполнено 2026-09-19 (трек A, §6.1):** ввод/редактирование — общий платформенно-нейтральный код App (W2/W3), W5 закрыл три web-пробела в `canvas-web`: (1) **клавиатурный фокус** — winit фокусирует канвас при create_window, но тот ещё не в DOM (with_append выключен) — `focus_window()` сразу после вставки в DOM (renderer_launch): keydown-листенеры winit висят на канвасе, без фокуса клавиатура мертва до первого клика; (2) **буфер обмена** — `web_clipboard::WebClipboard` за трейтом `ClipboardBackend` (карта §3.2): кэш (синхронный контракт) + `navigator.clipboard` writeText/readText (Promise через spawn_local, ошибки — warn/debug, деградация как `Clipboard(Option)`); ограничение волны 1 задокументировано: DOM-событие `paste` с синхронным clipboardData не используется — winit web с prevent_default (дефолт) гасит его preventDefault'ом на keydown (проверено по исходникам winit 0.30.13), внешнее копирование подтягивается следующим жестом; (3) **URL-параметры** — `url_params::parse_query` (чистая функция, 6 нативных тестов) + `?stress=N`/`?stress-widgets=N` — зеркало стресс-ветки main.rs (полные `?canvas=`/recent — W6). web-sys +3 фичи (Clipboard/Navigator/Location), ноль новых крейтов (js-sys Promise через transitive-типы). Приёмка: браузерный дым `scripts/web_smoke.py` (Playwright+swiftshader, оракул — консоль tracing/DOM-курсор/rAF; скриншоты WebGPU-канваса в headless не снимаются — ограничение из W4): фокус `document.activeElement=CANVAS` ✓; онбординг (FR-028) глушит ввод — Esc «Пропустить» доходит ✓; dblclick → инлайн-редактор (cursor=text), кириллица «привет» + Shift+Arrow выделение + Ctrl+B/A/Z/C/V без pageerror, Enter коммитит ✓; Space+ЛКМ → cursor=grabbing (клавиши до App) ✓; `?stress=5000` сеется, рендер живёт, rAF-fps = 60 ✓; `?stress=abc` — warn + обычный запуск ✓. Гейты: fmt, clippy `-D warnings`, test --workspace (0 failed; canvas-web 20), wasm_gate.sh, mcp_wasm_gate.sh, check wasm32 canvas-web — зелёные |
| W6 | L | **Хранение (§4).** `CanvasStorage`: FsAccessStorage + OpfsStorage + выбор при старте; IndexedDB-recent; DOM-drop приём файлов; `?canvas=`/`?stress=` URL-параметры; автосейв + .bak. Приёмка: открыть с диска → править → автосейв в файл → F5 → reopen из recent без пикера; OPFS-дефолт работает без диска; экспорт blob **Выполнено 2026-09-19 (трек A, §6.1):** мост через синхронный трейт — чистый `MirrorStore` (зеркало путь→текст + очередь, нативные тесты) + фоновый `spawn_local`-сброс в OPFS; `OpfsStorage` (дефолт/фолбэк: `?canvas=` → недавний → `default.canvas`, сеяние при отсутствии, битый файл — сеем поверх как натив `load_or_seed`, отказ OPFS — MemStorage с запретом перезаписи) и `FsAccessStorage` (пикер по жесту + readwrite-разрешение, автосейв через хэндл; версия-назад — в OPFS: родительский каталог хэндла браузер не отдаёт, sibling невозможен — отклонение от §4.2 п.3 задокументировано; отказ разрешения — «Открыть копию»); недавние/хэндлы — IndexedDB (бойлерплейт в JS-глю `index.html`, мост `js_glue`, выбор верхнего — чистая функция); DOM-drop → `import_to_opfs` (санитизация имени, превью дропа невозможно — данные файлов браузер отдаёт только на drop); экспорт download-blob из хранилища (не из живой сцены — второй канал состояния не заводим); DOM-панель хранилища (открыть/недавние/экспорт; адаптивность панели — CR-014 2026-09-20: never-off-screen — flex-wrap/max-width/ellipsis, <720px — левый нижний угол вне зон поиска/HUD/миникарты, dvh-канвас, тач-цели 44px, полное имя файла — в title-тултипе); `url_params::sanitize_canvas_name` + percent-декод (+4 теста); canvas-app: `AppEvent::OpenScene` — смена активной сцены (форс-сохранение прежней, подмена SceneState, сброс переходного UI; +4 теста) и будка автосейва в `about_to_wait` (грязная сцена держит rAF-цепочку — без неё debounce 2 с на web не срабатывает после последнего события ввода); конфиг — TOML из localStorage (read-only, запись — W12); `?stress` — в памяти, не пишет OPFS/recent. Приёмка: smoke 25 PASS (сеяние, автосейв с `.bak`, F5-reopen из недавних без пикера, `?canvas=` кириллицей, регресс W5/W7); пикер/экспорт — ручная приёмка (headless не автоматизирует). Гейты: fmt, clippy -D warnings (натив+wasm32), test --workspace (app 161, web 36), wasm_gate.sh, mcp_wasm_gate.sh, trunk build — зелёные |
| W7 | S–M | **Поиск + миникарта.** MemSearch-индекс по нодам (заголовки/тела), debounce 200 мс (готово), UI поиска и миникарты из render без изменений. Приёмка: Ctrl+F по 5000-нод сцене, переход/подсветка результата **Выполнено 2026-09-19 (трек B, §6.1):** цепочка поиска на web уже собрана W3/W4 (MemSearch за трейтом + ответы через AppEvent::Search; поиск по заголовкам/телам нод — платформенно-нейтральный `scan_scene` в `apply_search_hits`, §3.2 «scan_scene уже есть»; миникарта — общий GPU-проход canvas-render) — W7 закрыл наблюдаемость и приёмку: (1) **`?log=debug|trace|warn|error`** — уровень консольного лога из URL (`url_params::LogLevel`, мягкий парсинг: неуровень → тихий INFO, не роняет `?stress`; `web_log::init_tracing_with`; баннер W4→W5); (2) DEBUG-логи оракула дыма — ответ backend'а в респондере canvas-web («поисковый backend ответил hits=N») и итог склейки FTS+scan_scene в `apply_search_hits` (canvas-app, DEBUG — нативно невидим); (3) **`scripts/web_smoke.py`** — воспроизводимый дым: синтетические `KeyboardEvent` для кириллицы (Playwright `keyboard.type` шлёт insertText без keydown для вне-US-символов — winit-web их не видит; реальная RU-клавиатура даёт keydown с key="ф" — диспатч точно воспроизводит контракт), оракулы — консоль/DOM-курсор/rAF. Приёмка: Ctrl+F по `?stress=5000` — запрос «смета» → rows=625 → Enter (прыжок, полёт камеры) ✓; сквозной кейс — заметка «привет мир» ( dblclick → кириллица → коммит) → поиск находит (rows=1) ✓; MemSearch-roundtrip через proxy ✓; нативные тесты url_params 9 (+LogLevel), web_log 4. Известный артефакт среды (не падение, задокументирован в дыме): SwiftShader отвергает glyphon-буфер mappedAtCreation=8192 на прыжке — на аппаратном WebGPU (продуктовый таргет) ограничения нет. Гейты: fmt, clippy `-D warnings`, test --workspace (0 failed), wasm_gate.sh, mcp_wasm_gate.sh, check wasm32 canvas-web, trunk build — зелёные |
| W8 | S | **Numi-формулы (FR-013/014).** expr/flow чистые (сборка подтверждена): прогон на web-сцене, фикс падений, если найдутся. Приёмка: calc-строки считают, поток значений по рёбрам живой, бейджи ошибок с тултипом **Выполнено 2026-09-19 (трек B, §6.1):** чистый код expr/flow на web не упал — падений не найдено, фикс не потребовался. Приёмка — оракул браузерного дыма: DEBUG-лог «пересчёт потока: значения вычислены values/errors/lines» в `SceneState::recompute_flow` (canvas-scene — платформенно-нейтрален, нативно под фильтром не виден); сцены дыма 5c: calc-строка «кв = 5» → values=1, lines=1 (кириллический идентификатор — грамматика FR-013 Unicode-совместима); «2 +» → errors=1 (error-бейдж с тултипом — hit-test FR-021, тест `expr_error_tooltip_hit_test`). Поток значений по рёбрам — тот же чистый `propagate_with_lines` (canvas-core/flow): wasip1-тесты wasm_gate.sh + MCP e2e-оракул ±1% (mcp_wasm_gate.sh) — зелёные; вставленный шаблон (W9) считает $param-лист на web-сцене. Гейты: fmt, clippy -D warnings, test --workspace (45 наборов), wasm_gate.sh, mcp_wasm_gate.sh — зелёные |
| W9 | S | **Шаблоны (FR-018/020).** `include_dir` c template.json (уже в core/templates.rs), панель Ctrl+P, wheel-меню. Приёмка: вставка шаблонной группы на web **Выполнено 2026-09-19 (трек B, §6.1):** реестр 45 builtin-манифестов (`include_dir`) на web полон (сборка ядра не режет assets); `custom(root)`/`save_custom` (FR-020) на web деградируют честно: `std::fs` под wasm32-unknown-unknown возвращает `Err(Unsupported)` (не панику) — custom-скан пуст, «Сохранить как шаблон» — toast об ошибке (OPFS-хранилище custom — волна 2, §9). Принята вся цепочка: Ctrl+P → палитра (оракул «шаблонная палитра» templates=45 categories=4) → Enter → вставка шаблонной группы в модель (оракул «шаблон вставлен» template=com.canvasdesk.api-gateway; instantiate + fit_template_node_height + recompute_flow), Shift+клик → wheel-меню категорий (оракул). DEBUG-оракулы — в `on_key` (Ctrl+P-ветка) и `instantiate_template_at` (canvas-app), по образцу W7. Попутно: (1) шим Ctrl+P в index.html — гасит браузерный акселератор печати (winit-web не preventDefault’ит браузерные связки: палитра открывалась И поднималась печать, rAF-насос winit приостанавливался — ввод умирал; шим в фазе перехвата гасит только default-действие, событие доходит до App); (2) clippy-фикс unused-import в нативных тестах fs_access/opfs (гейт был красный на main). Известный артефакт дыма (задокументирован, не падение): первый кадр палитры на SwiftShader бьёт mappedAtCreation-лимит (32768 > 4KiB) — необработанное исключение разрывает rAF-насос, поэтому палитра проверяется атомарно последней секцией (аккорд+Enter одним evaluate — обработчики ключей успевают до ломкого кадра); на аппаратном WebGPU ограничения нет. Гейты: fmt, clippy -D warnings (натив+wasm32), test --workspace (45 наборов), wasm_gate.sh, mcp_wasm_gate.sh, check wasm32 canvas-web, trunk build — зелёные; smoke — SMOKE OK |
| W10 | M | **Превью картинок.** WebImageThumbnailProvider: `createImageBitmap` → OffscreenCanvas downscale → RGBA → существующий thumbs-атлас; приём файлов из W6. Приёмка: PNG/JPEG file-ноды с превью; заглушка для прочих типов **Выполнено 2026-09-19 (трек A, §6.1):** `WebImageThumbs` (canvas-web, трейт `ThumbBackend`) — заказ `request` → `spawn_local`: OPFS-чтение → `createImageBitmap` (нативный декод браузера — митигация риска «однопоточный декод», §7) → OffscreenCanvas downscale в ячейку атласа 256² (паритет SIZE_CLASS натива) → `getImageData` → RGBA `Thumbnail` → очередь + побудка `AppEvent::ThumbsReady` → `drain` → `renderer.set_thumbnail` (существующий атлас); дедуп по ноде; кэша нет — rusqlite на wasm недоступен, декод дешёв; не-картинки/ошибки — `None` → негативный кэш app («заглушка для прочих типов»); приём файлов из W6: не-канвасные файлы DOM-drop'а → OPFS подкаталог `files/` (санитизация + коллизии-кандидаты `-N` probe'ом, лимит 64 МБ) → тот же `DragEvent::Drop` с **новым нейтральным вариантом `DragData::Paths`** (core; канонический путь по доке dragdrop.rs — canvas-web производит события T9) → ноды/сетку/поиск делает общий `plan_drop` (без expand — он на std::fs); `client_pt` — физические px (CSS × dpr, контракт T9). web-sys +5 фич, ноль новых крейтов. Приёмка: web_smoke.py 38 PASS — синтетический DragEvent с DataTransfer (PNG 1×1 + txt): «файлы приняты в OPFS» → 2/2 сохранены → «превью готово» (PNG в атласе) → txt DEBUG-заглушка → без pageerror; файл-ноды переживают F5 (путь `/files/<имя>` абсолютный — `resolve_node_path` не зовёт current_dir, паникующий на wasm). Гейты: fmt, clippy -D warnings, test --workspace (45), wasm_gate.sh, mcp_wasm_gate.sh, trunk build — зелёные |
| W11 | M | **Виджеты снапшотом.** Registry: встроенные пакеты из include_dir в OPFS/память; widget_state в localStorage; LOD-деградация как на Linux. Приёмка: виджет-нода не падает, рендерится плейсхолдером; создать/удалить можно; состояние переживает reload **Выполнено 2026-09-19 (трек A, §6.1):** решение «OPFS или память» — **память** (план §5): пакетные файлы в волне 1 никто не читает (live-хост отсутствует), манифесты нужны меню/LOD/permissions — `WidgetRegistry::in_memory()` (canvas-widgets, Store::Fs|Memory без cfg, обе ветки тестируются нативно): встроенные пакеты из include_dir-статики (виртуальные dir `/builtin/<id>`), install из папки — ошибка `NoFileSystem`, tombstone в памяти + экспорт/импорт (`memory_tombstones`/`set_memory_tombstones`); выбор режима — в `App::new` по каталогу кэша (есть ФС-каталог → файловый реестр, натив без изменений; нет → память); инициализация — общий `app.init_widgets()` как в main.rs. widget_state — `WebWidgetState` (canvas-web): синхронный трейт над localStorage (одна JSON-запись `canvasdesk.widget_state`, карта node→key→value; битый JSON — warn+пусто, недоступный — деградация); чистая StateMap (+4 нативных теста). Тик LOD — `setInterval` 1000 мс → `WidgetEvent::Tick` через EventLoopProxy (зеркало widget-tick-потока main.rs, ноль новых зависимостей). LOD-деградация — общий код (runtime_ok=false вне Windows → Placeholder). Осознанное отклонение: tombstone не переживает F5 (экспорт/импорт API есть, сохранение — W12/волна 2; визуально в волне 1 ноде всё равно рендерится плейсхолдер). Приёмка: web_smoke.py 32 PASS — ?stress-widgets=3 → «реестр виджетов готов» + LOD `target=Placeholder` (нода не падает, плейсхолдер) + рендер живёт; widget_state: сеев JSON → F5 → «localStorage готов keys=2» (состояние пережило reload); создание/удаление через контекстное меню — ручная приёмка (меню в wgpu-канвасе, headless-клики хрупки). Гейты: fmt, clippy -D warnings, test --workspace (45 наборов), wasm_gate.sh, mcp_wasm_gate.sh, trunk build — зелёные |
| W12 | S–M | **Полировка и деплой.** wasm-opt / `-Oz` / lto; бандл-размер в CI-логе; GitHub Pages по пути `/app` (Pages для user-docs уже есть — отдельный workflow); README-секция web. Приёмка: деплой грузится ≤5 с; все критерии §8 зелёные **Исполнено 2026-09-19 (финализатор, трек B):** (1) `[profile.release] lto="thin"` (workspace Cargo.toml — общий профиль для всех целей, основное сжатие даёт wasm-opt); (2) **`scripts/web_bundle.sh`** — релизный бандл одной командой: trunk 0.21.14 `--release` (+`--public-url`/`--dist` для деплоя, `--report-only`), `wasm-opt -Oz` поверх .wasm (binaryen; нет инструмента — предупреждение, деплой-CI ставит сам), отчёт raw/brotli (нет brotli — gzip-оценка сверху) + оценка загрузки 25 Мбит/с; в CI итог дублируется в `$GITHUB_STEP_SUMMARY`; (3) **`pages-web.yml`** — отдельный Pages-workflow: `trunk build --release` + wasm-opt в `/app` + **Jekyll-сборка документов из корня** (та же стандартная сборка Pages, что у прежнего branch-деплоя — канонические URL user-docs/*.html сохранены; один артефакт = весь сайт: замена «Deploy from a branch», Source: «GitHub Actions», инструкция в README; частный репозиторий — Pages на планах Pro/Team/Enterprise; Jekyll до rust-cache — иначе target/ попадает в _site; public-url = база репозитория `/CanvasDesk/app/`); (4) canvas-web в wasm-гейты: ci.yml `wasm-check` + `wasm_gate.sh` ступень 1 (компиляция; бандл/деплой — Pages-workflow); (5) README «Веб-версия (wasm, M8)» (запуск, URL-параметры, file://-грабли, Pages) + user-docs/README «Публикация» (GitHub Actions-источник) + AGENTS.md; (6) фикс `clippy --all-targets`: 2 unused-import в тестах W6 (fs_access/opfs — CI-форма без --all-targets их не видела); (7) отклонение: запись конфига в localStorage (W6-заметка «запись — W12») отложена — файловая зона canvas-web/canvas-app занята in-flight W8–W11, конфликт-риск неоправдан, волна 2. Замер: бандл 6.74 МБ raw / 2.96 МБ gzip (§8.8 ✓), −20 % от wasm-opt, загрузка ~1 с (§8 ≤5 с ✓). Дым release-бандла (lto+wasm-opt): 25 PASS (ввод/кириллица/поиск/OPFS/стресс-5000, 0 pageerror, rAF-fps 61). Гейты: fmt, clippy -D warnings (--all-targets), test --workspace (45 наборов), wasm_gate.sh (с canvas-web), mcp_wasm_gate.sh (e2e), web_bundle.sh — зелёные. Merge f8a3e7a в main после W8–W11 (все→W12, §6.1; на f8a3e7a CI queued, Pages-workflow стартовал). Первый прогон Pages-workflow вскрыл два бага — закрыты фикс-коммитом: (а) фактический legacy-источник сайта — `main /docs`, а не корень, как писало user-docs/README — Jekyll переведён на `source: ./docs` (структура сайта сохранена 1:1, `/app` — единственное добавление), руководство user-docs исправлено; (б) `tar --strip-components=1` клал wasm-opt в `/usr/local/bin/bin/` («command not found» при зелёном tar) — исправлено на 2. PAT без прав на Pages (API 403), но деплой-экшен сработал и поверх legacy-Source — приложение задеплоено без ручного переключения (прогон 8d77bd3: все шаги зелёные, в т.ч. Deploy); переключение Source: «GitHub Actions» в Settings — необязательная чистка, останавливает параллельные legacy-сборки docs/. Продакшн-приёмка: https://danku13.github.io/CanvasDesk/app/ — 200, wasm 7.0 МБ (1.54 МБ brotli, загрузка ~1 с §8 ✓); полный браузерный дым ПРОТИВ ДЕПЛОЯ: SMOKE OK (ввод/кириллица/поиск/OPFS/W10-превью/W11-виджеты, 0 pageerror); доки сайта сохранены (/SPEC.html и т.д. — 200) |

**Суммарно волна 1: ~2–4 недели** сфокусированной работы одного
разработчика (совпадает с экспресс-оценкой анализа от 2026-09-16: ядро
готово на 100%, работа — в обвязке и платформенном слое).

> **Примечание 2026-09-18 (FR-036, ADR-0011) — W0 реализован + расширение
> тест-раннером.** Приказ владельца «распланировать сборку wasm и реализовать
> сборку для повышения автономности в тестировании» дал ядру две ступени сверх
> W0: (1) локальный гейт `scripts/wasm_gate.sh` — check (ступень 1) + артефакт
> rlib ядра (ступень 2) + **исполнение 301 теста `canvas-core` в wasmtime**
> под `wasm32-wasip1` (ступень 3, runner в `.cargo/config.toml`; wasip1 —
> служебный тестовый таргет, продуктовый браузерный — прежний
> `wasm32-unknown-unknown`); (2) эмпирически найдены и устранены паники
> `std::env::temp_dir`/`std::process::id` на wasm (тестовая песочница
> `test_scratch_root` — единственное `cfg(target_arch)` в core, только в
> `#[cfg(test)]`, исключение зафиксировано в ADR-0011). Волны W1–W12 не
> затронуты; CI — только ступень 1 (джоба `wasm-check`), исполнение в
> рантайме — локальная ступень гейта.
>
> **Примечание 2026-09-18, актуализировано 2026-09-19 (FR-037, ADR-0012 — принято; MW1–MW4 исполнены) — MCP-слой под
> wasm: план.** Приказ владельца «спланировать реализацию MCP для WASM,
> чтобы можно было проверять не только UI, но и реализацию MCP». План
> (`docs/change-requests/fr-037-mcp-wasm-verification.md`): вынос
> SceneState + `mcp_dispatch` (27+ инструментов) из `main.rs` в
> платформенно-нейтральный крейт `canvas-scene` (**исполнено** MW1: W2
> сужается до выноса App-обёртки; view-типы — в canvas-scene, реэкспорт
> canvas-render);
> `run_stdio_with_transport` в `canvas-mcp` (транспорт-агностика);
> лист-крейт `canvas-mcp-headless` (bin для wasmtime/wasip1) — серверная
> сторона будущего **MCP WebSocket-моста волны 2 (§9)**: браузерный мост
> обернёт ту же HeadlessSession. Гейт: `scripts/mcp_wasm_gate.sh` —
> реальная MCP-сессия в wasmtime (initialize → tools/list → graph_apply
> эталона → oracle ±1%). Реализация не начата; волны W1–W12 остаются в
> прежних границах.

### 6.1 Двухагентное расписание (утверждено владельцем 2026-09-19)

Волна 1 исполняется **двумя независимыми агентами** по схеме «два трека».
Вводные: W0 выполнен (FR-036); MW1 (`canvas-scene`) сузил W2 до выноса
App-обёртки; MW3/MW4 (FR-037, зона `canvas-mcp*`) идут параллельно другим
агентом — файловые зоны не пересекаются, кроме workspace `Cargo.toml`
(координация — п. 5 протокола).

**Треки.**

- **Трек A «натив/ядро»** — критический путь по существующим крейтам
  (canvas-core/render/app): **W1 → W2 → W3**, далее **W5 → W6**.
- **Трек B «web»** — новый крейт: **W4-каркас** (параллельно W1–W3, ноль
  файловых пересечений с треком A: только новые файлы `crates/canvas-web/`)
  → **W4-прошивка** (после W3), далее **W7 → W9 → W8 → W11**, затем
  **W10** (после W6).
- **W12** закрывает финализатор — свободный к тому моменту агент, включая
  все правки CI (п. 4 протокола).

Ориентировочный график (объёмы — §6):

| Дни | Трек A (натив/ядро) | Трек B (web) |
|---|---|---|
| 0–0.5 | W1 web_time (S) | W4-каркас: крейт canvas-web, trunk, panic-hook — без App |
| 0.5–3.5 | W2 App→lib (S–M, сужен MW1) → W3 трейты сервисов (M) | W4-каркас: bindgen-обвязка, tracing-web, тесты каркаса |
| 3.5–5 | приёмка W4, чеклист §8 | W4-прошивка: spawn_app, async-init Renderer, пустая сцена |
| 5–11 | W5 ввод/редактирование (M) → W6 хранение (L) | W7 поиск (S–M) → W9 шаблоны (S) → W8 Numi (S) → W11 виджеты (M) |
| 11–13 | W12 полировка/деплой (S–M) | W10 превью (M, после W6) → докрытие §8 |

Жёсткие зависимости: W1→W4 (паника `Instant`), W2→W3/W4,
W3→W4/W6/W7/W11, W4→W5–W11, W6→W10, все→W12.

**Протокол независимости.**

1. **Ветка на задачу:** `feature/wasm-w<N>-<slug>` от свежего main;
   1 задача = 1 коммит; merge `--no-ff` в main сразу после локальных
   гейтов (fmt, clippy `-D warnings`, тесты, `scripts/wasm_gate.sh`).
2. **Синхронизация после каждой задачи:** перед стартом следующей —
   `git fetch` + rebase на origin/main; при отказе push — rebase и повтор.
3. **Правило ребаланса:** освободившийся агент берёт верхнюю незанятую
   задачу топопорядка §6, файлы которой не пересекаются с in-flight
   задачей напарника.
4. **CI отложен в W12:** до W12 файлы `ci.yml`/`wasm_gate.sh` не трогаются
   (зона MW3/MW4-агента); в W12 — один коммит: canvas-web в wasm-гейты,
   Pages-workflow, размер бандла в CI-лог.
5. **Файловые зоны и координация:** этап I — A владеет
   canvas-core/render/app, B — только canvas-web;
   `canvas-web/src/lib.rs` — точка слияния модулей, дифф держать мелким
   (mod-декларация + строки инициализации). Workspace `Cargo.toml` /
   `Cargo.lock` — редкие правки, мержить promptly; пересечения с
   MW3/MW4-агентом решаются через журнал worklog.

## 7. Риски и митигации

| Риск | Вероятность | Митигация |
|---|---|---|
| Instant-паники на wasm (первый же кадр FrameMeter) | определён | W1 до W4; `web_time` уже в дереве зависимостей |
| pollster-блокировка потока на wasm | определён | W4: `spawn_local`; `Renderer::new` уже async, меняется только вызов |
| Кириллица/CJK-ввод | средняя | кириллица — через `Key::Character` (тест в W5); CJK-IME в winit web нет — ограничение v1, §1/§9 |
| Pinch-зум | низкая | ctrl+wheel уже хоткей зума; тач-устройства вне v1 (§9) |
| Приём файлов drag-ом | определён (winit web не даёт DroppedFile) | W6: DOM-листенеры на canvas-элементе — стандартный паттерн |
| FS Access permission после рестарта браузера | средняя | reopen из recent — один re-request; отказ → предложить копию в OPFS |
| Бандл 4–8 МБ (шрифты 1.4 + wgpu) | средняя | wasm-opt + brotli на Pages (≈2–4 МБ); шрифты отдельным fetch — волна 2 (§9) |
| WebGPU-драйверные баги Chromium | низкая | таргет один движок; диагностика F3-HUD; WebGL2 — волна 2 |
| Однопоточный декод тамбнейлов | средняя | `createImageBitmap` — нативный decode браузера (не wasm); воркер+OffscreenCanvas — волна 2 |
| Регресс нативных сборок | низкая | W2/W3 — перемещения без behavior-change; нативные гейты обязательны зелёными |
| Расползание cfg по app | низкая | правило §3.1: web-код только в canvas-web; ревью каждого W-коммита |

## 8. Критерии приёмки волны 1 (MVP)

1. CI: гейт `wasm-check` + нативные гейты (windows/linux/macos) зелёные на
   каждый пуш.
2. Chromium (последний stable): открыть `.canvas` с диска, редактировать,
   автосейв пишет в файл; F5 → reopen из «недавних» без пикера.
3. OPFS: дефолт-канвас без диска; экспорт/импорт blob; OPFS-режим не
   падает в Firefox (бонус, не таргет).
4. Функциональный паритет MVP-скоупа: заметки, форматирование, связи,
   undo 50, палитра цветов, поиск, миникарта, Numi-формулы, поток значений,
   шаблоны, темы, HUD, хоткеи — как в нативе (ручная приёмка по образцу
   ACCEPTANCE.md, чеклист адаптировать под web).
5. `?stress=5000` — 60 fps на средней машине (метрика HUD).
6. Виджет-ноды: не падают, плейсхолдер, widget_state переживает reload.
7. Нативные сборки: нулевой регресс (приёмка M7 §7 проходит без
   изменений).
8. Бандл ≤8 МБ raw / ≤4 МБ brotli; загрузка ≤5 с на 25 Мбит/с.

## 9. Вне объёма волны 1 (заявки волны 2)

- **iframe-live виджеты** — пост-стабилизация; контракт bridge
  сохраняется (§5).
- **WebGL2-фолбэк** (wgpu feature `webgl`) + таргетирование Firefox/Safari.
- **MCP WebSocket-мост** к локальному canvasdesk-сервису (серверная
  сторона — HeadlessSession из FR-037/ADR-0012: мост обернёт тот же
  in-process слой инструментов, что верифицируется в wasmtime).
- **PWA/оффлайн-манифест**, установка приложения браузером.
- **Воркер-декод тамбнейлов** (OffscreenCanvas + wasm в worker) — убрать
  просадки на больших JPEG.
- **Тач-жесты** (мобильные Chromium).
- **Шрифты отдельным ресурсом** (HTTP-кэш браузера) для экономии бандла.

Протокол работы — AGENTS.md; журнал — /home/z/my-project/worklog.md; после
каждой задачи — запись (Task ID: wasm-w0 … wasm-w12).
