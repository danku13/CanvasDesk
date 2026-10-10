# CanvasDesk — инструкции для агента

## Проект

CanvasDesk — **визуальная система математического моделирования** (ADR-0007):
бесконечный зумируемый канвас, на котором исполняемые математические модели
строятся из расчётных нод (Numi-листы и шаблоны), значения проливаются по
value-связям (DAG-движок), доменная математика (единицы, queueing, финансы)
встроена в ядро, а ИИ-агент собирает и проверяет модели через MCP
(`canvas-mcp`). Носитель модели — файловый канвас (карточки — настоящие
файлы), JS/HTML-виджеты (M5) и режим «вместо рабочего стола» (M4).
**Кроссплатформенный (M7): Windows 10/11 x64 — полная функциональность;
Linux (X11/Wayland) и macOS — оконное приложение, платформенные разрывы
закрываются по плану `docs/plans/M7-crossplatform.md`.** Язык — Rust stable
(1.80+), edition 2021.

**Текущее состояние репозитория:** M1–M4 выполнены, M5 (T20–T22) выполнен
целиком — рантайм виджетов, bridge/permissions, SDK и документация
(`docs/WIDGETS.md`); M6 — T24 (MCP) выполнен. Кроссплатформенность (M7) —
план `docs/plans/M7-crossplatform.md`. Статус задач — README.md и
`docs/TASKS.md`.

## Документация — единственные источники истины

- `docs/SPEC.md` — спецификация: стек, модель данных, рендер, shell-интеграция, виджеты,
  риски, критерии готовности milestone'ов. Читать перед любой задачей.
- `docs/TASKS.md` — план инфраструктурной волны (T0–T22, M1–M5). Порядок внутри
  milestone'а строгий.
- `docs/adr/` — архитектурные решения (индекс — `docs/adr/README.md`, шаблон —
  `docs/adr/adr-template.md`). Позиционирование продукта и контракты
  «один шаблон = один расчёт», портов значений и MCP-композиции — здесь.
  ADR пишется ДО реализации решения; `Статус: принято` — только по прямому
  запросу владельца (ADR-0001, п. 2 правил).
- `docs/change-requests/` — FR/CR-документы по шаблону `docs/change-requests/cr-template.md` (индекс —
  `docs/change-requests/index-cr-fr.md`). Расчётная волна (моделирование) — FR-013…FR-029, CR-013:
  эталоны приёмки — ADR-0005 (Instagram MVP) и ADR-0006 (каталог №1–№5).
  Реализация — только по документу; по завершении агент обновляет статус
  («реализовано»), Changelog документа и `docs/ACCEPTANCE.md`.
- `skills/` — пакет скиллов CanvasDesk MCP для внешних ИИ-агентов
  (публикуемая производная реестра инструментов; актуальный счётчик
  инструментов — только `skills/README.md`, единственный источник числа;
  при правке руководствоваться им, не памятью). **Любое изменение состава/семантики MCP-инструментов
  (`TOOLS` в canvas-mcp, `crates/canvas-scene/src/mcp.rs` в canvas-scene) обязано обновлять
  `skills/` в том же коммите**: контракт-тест `skills_*` в canvas-mcp
  (полнота каталога, покрытие скиллами, счётчик README, call-позиции)
  валит CI при рассинхроне. Протокол — `skills/UPDATE-PROTOCOL.md`.
- `docs/plans/product-roadmap.md` — продуктовый роадмап (принят владельцем
  2026-09-18 вместе с ADR-0008): волны 0/A/B/V/S, гейт востребованности
  «≥ 5 внешних пользователей сами построили модель и вернулись второй раз»,
  привязка M0–M6 архдока и R1–R5 CR-013 к волнам. Порядок реализации
  FR/CR моделирования и новых зависимостей сверяется с этим документом.
- `docs/RECIPES.md` — рецепты R1–R17 по встройке в десктоп (анализ Lively Wallpaper
  и Seelen UI) + список файлов-источников. **Обязателен перед T15–T18** и любой задачей
  по shell-интеграции.
- `docs/DEMO.md` — рецепт быстрой подготовки демо-стенда под Linux (Xvfb + lavapipe
  + `scripts/xdemo.py`): скриншоты и GIF с живого запуска. Читать перед записью
  любых демо-материалов.
- `docs/WASM-TESTING.md` — рецепт быстрой настройки WASM-тестирования UI
  (уровни L0–L3, сборка web-стенда без trunk, Chromium/WebGPU под Xvfb,
  сценарии и оракулы, грабли). Читать перед первой wasm-проверкой UI
  (разовая настройка ~10 минут; команды — в шпаргалке §6 рецепта).

Правило: агент сверяется с этими документами, а не с собственной памятью — API меняются,
недокументированное поведение Windows различается по версиям ОС (см. §7.4 SPEC:
иерархия Progman/WorkerW/DefView в Win11 24H2+ отличается от классической).

Именование ссылок в документации: внутри репозитория ссылки на md-источники —
только относительные `*.md`; `*.html` допустим только для реальных артефактов
на диске (например `docs/prototypes/*.html`) и внутри `user-docs/` (FR-031,
линк-чек в тестах `docs_ui`). Ссылки на собранные страницы Pages — абсолютными
web-путями. Гейт: `python3 scripts/doc_lint.py` (CI-джоба `docs-lint`;
0 ошибок обязательна, правило введено 2026-10-10, issue #18).

## Обязательный вопрос при реализации FR/CR: онбординг и документация

**Завершая реализацию любого FR или CR, агент ОБЯЗАН явно спросить владельца,
требуется ли доработка онбординга и пользовательской документации.** Вопрос задаётся
в итоговом ответе по задаче — молчание владельца не отменяет его необходимости.
Что проверять:

1. **Онбординг** (`crates/canvas-app/src/onboarding_ui.rs`, FR-028): новая
   функциональность меняет шаги тура? Добавить/переформулировать шаги
   (8±2, «один шаг = одна мысль»), обновить юнит-тесты шагов и
   `docs/interface-objects/onboarding.md`.
2. **Пользовательская документация** (`user-docs/`, 7 страниц; вшита в бинарь
   просмотрщиком FR-031 — `include_str!`, источник `crates/canvas-app/src/docs_ui.rs`):
   не устарели ли формулировки (поведение, хоткеи, названия объектов)?
   Обновить затронутые страницы + таблицы `user-docs/README.md` и `user-docs/index.md`;
   ссылки между страницами — только относительные `*.html` (линк-чек в тестах
   `docs_ui` валит CI на битых). Горячие клавиши держать в синкре со списком
   `HOTKEYS` (`crates/canvas-app/src/lib.rs`, F1-оверлей) и `user-docs/hotkeys.md`.

Изменение доков/онбординга может быть отдельным коммитом по решению владельца;
вопрос не подразумевает автоматической правки без подтверждения.

## Целевой стек и структура workspace

Стек (детали и версии — SPEC §3): winit 0.30 (окно/ввод), wgpu 22+ (рендер),
glyphon/cosmic-text (текст), rstar (R-tree, spatial index), serde_json (формат
`.canvas` — JSON Canvas spec 1.0), rusqlite bundled (тамбнейл-кэш, FTS5-поиск, сессии),
notify 6+ (файловый вотчер), windows-rs (Win32/COM), pdfium-render (PDF), image,
webview2-com + WebView2 Evergreen (виджеты M5), tracing (логи), cargo-wix (MSI).

Структура workspace после T0 (SPEC §4):

```
Cargo.toml                 # workspace
crates/
  canvas-core/             # модель данных, JSON Canvas I/O, spatial index, Numi-движок (expr/),
                           # DAG-поток значений (flow.rs), реестр шаблонов (templates.rs)
  canvas-render/           # wgpu-рендер: камера, батчинг, текст, текстуры, LOD
  canvas-shell/            # Windows-only: тамбнейлы, preview handlers, drag-drop, WorkerW
  canvas-preview-host/     # отдельный exe — песочница для IPreviewHandler
  canvas-widgets/          # M5: WebView2-хост, bridge, манифесты, снапшоты
  canvas-mcp/              # MCP-посредник: stdio JSON-RPC ↔ named pipe, run_stdio_with_transport (FR-037)
  canvas-scene/            # модель сцены + mcp_dispatch (каталог инструментов — реестр TOOLS в canvas-mcp; счётчик — skills/README.md) — платформенно-нейтральный, wasm (FR-037/ADR-0012)
  canvas-mcp-headless/     # headless MCP-сервер для wasmtime/wasip1 — верификация MCP-сессий без Windows (FR-037, лист-крейт)
  canvas-web/              # M8/W4 (wasm-port): web-платформенный слой — bindgen-обвязка, web-сервисы; трек B, лист в DAG (каркас)
  canvas-app/              # приложение: event loop, команды, UI-состояние, main()
assets/                    # шрифты, иконки нод, встроенные виджеты (assets/widgets/), шаблоны (assets/templates/)
docs/                      # SPEC.md, TASKS.md, RECIPES.md, adr/, change-requests/
```

## Правила архитектуры

1. `canvas-core` не импортирует ничего из `canvas-shell`, `canvas-render` и вообще
   не зависит от ОС и GPU. Платформенная логика — только за трейтами
   (`ThumbnailProvider`, `PreviewProvider`, `ShellIntegration`), чтобы core
   тестировался на любой ОС.
2. Платформенный код — в `canvas-shell`/`canvas-widgets`/`canvas-mcp` под
   `cfg(windows)`/`cfg(unix)`, либо за трейтами из canvas-core (паттерн
   `ThumbnailProvider` + `NoopThumbnailProvider`). Core и render обязаны
   собираться и тестироваться на всех трёх ОС (CI-матрица M7).
   Платформенные ветки не расползаются по `canvas-app`: app выбирает
   реализацию трейта, а не ветвится по cfg на каждом вызове.
3. Все координаты канваса — в логических пикселях (world-space); рендер — в физических
   (`scale_factor`). DPI awareness — Per-Monitor V2. После репарентинга в десктоп
   (M4) `window.scale_factor()` не доверять — поллинг `GetDpiForWindow` (RECIPES R10).
4. Не блокировать рендер-поток: весь I/O, COM и тяжёлые декодеры — в worker-потоки
   (пул тамбнейлов — 4 потока, результаты через каналы). Тяжёлый пересчёт потока
   значений — сценарный воркер FR-064 (`canvas-scene/src/worker.rs`: double buffer
   `Arc<RwLock<FlowSolutions>>`, wake через `EventLoopProxy<AppEvent>`; выводка O(N) —
   на UI-треде). Правило «фолбэк + warn» на этом стыке: при отказе/таймауте (3 с)/
   панике воркера — синхронный пересчёт на UI-треде + `tracing::warn!` (результат
   побитово идентичен — golden-тесты `crates/canvas-scene/tests/worker_smoke.rs`); на wasm — sync-путь штатно.
5. Раскладка (координаты, размеры, связи) — только в `.canvas`-файле. SQLite
   (`~/.canvasdesk/cache.db`) — пересоздаваемый кэш, его удаление ничего не ломает.
6. Автосейв `.canvas` с debounce 2 с + `.bak` предыдущей версии.
7. LOD по zoom (SPEC §6.2): < 0.25 — прямоугольник+иконка; 0.25–0.6 — тамбнейл+имя;
   0.6–1.5 — превью; > 1.5 — живое превью (максимум 3 одновременно, гистерезис 10%).
   Виджеты: живой WebView2 при zoom ≥ 0.25, иначе snapshot; лимит live-инстансов 6 (LRU).

## Правила Win32 / shell

1. Недокументированные приёмы (WorkerW, сообщения 0x052C и 0x7402) — только за
   рантайм-детектом с фолбэком на обычное окно, по рецептам `docs/RECIPES.md`;
   каждый — с комментарием-ссылкой на рецепт. Утверждения о поведении
   Progman/WorkerW/DefView, не подтверждённые RECIPES или рантайм-детектом, считаются
   непроверенными.
2. Рецепты реализуются чисто: Lively — GPL-3.0, Seelen UI — AGPL-3.0. Копирование кода,
   идентификаторов, структуры и комментариев запрещено; переносим только механику.
3. Ключевые ограничения из RECIPES: 0x052C слать ТОЛЬКО если WorkerW отсутствует (R4);
   `WS_EX_LAYERED` + `SetLayeredWindowAttributes(255)` строго до `SetParent` (R2);
   верификация стилей после репарентинга (R3); идемпотентное скрытие иконок через
   чтение `SHGetSetSettings` перед toggle 0x7402 (R5); НЕ использовать
   `SPI_SETDESKWALLPAPER` на raised desktop (R9); `SHGetSetSettings` с fSet в Win10+
   не работает — не тратить время.
4. Enum-обёртки Win32 — по идиоме RECIPES R13 (boxed closure через LPARAM,
   `extern "system"` трамплин, safe API наружу).
5. Любая ошибка шага встройки в десктоп → фолбэк на обычное окно + предупреждение
   пользователю (не только при неизвестной версии ОС).

## Правила безопасности и виджетов (M5)

- Виджеты — только локальные пакеты (`%APPDATA%/canvasdesk/widgets/<id>/`), ставятся
  явным копированием пользователем. Remote URL как виджет — запрещён архитектурно.
- Permissions из `widget.json` проверяются на КАЖДЫЙ bridge-вызов; без `network` все
  внешние `WebResourceRequested` блокируются; навигация вне пакета запрещена;
  `SetVirtualHostNameToFolderMapping` + CSP `default-src 'self'`.
- Bridge — типизированный JSON-RPC поверх postMessage; все сообщения валидируются
  serde-схемами; невалидное сообщение = drop + warn, никогда не паника.
- Никаких сетевых вызовов в хосте — продукт локальный; сеть только внутри виджетов
  с permission `network`.

## Код-стиль и качество

- Разработка — по TDD: перед реализацией задачи сначала пишутся тесты, описывающие
  требуемое поведение (по критериям приёмки из `docs/TASKS.md`); реализация считается
  готовой, когда тесты зелёные. GPU/shell-код, не поддающийся юнит-тестам, выносит
  логику в чистые функции, которые тестируются.
- Ошибки: `thiserror` (библиотечные типы) + `anyhow` (границы приложения);
  никаких `unwrap`/`expect` в production-путях.
- `unsafe` — только в `canvas-shell`/`canvas-widgets`, каждый блок с SAFETY-комментарием.
- Комментарии и документация — на русском (язык проекта). Недокументированные Win32-приёмы —
  с ссылкой на рецепт RECIPES.
- Каждая задача завершается одним коммитом с сообщением по conventional commits
  (задача = сессия = коммит, не давать несколько задач сразу).
- Новые зависимости — только с обоснованием в описании коммита/PR.

## Планирование работ: GitHub issues + Projects (директива 2026-10-10)

**Порядок любой доработки: сначала задачи — потом код; issue закрывается
только по факту проверки.**

1. **Планирование = создание задач.** Доработка (фича, волна, инкремент)
   начинается с high-level issue в GitHub — до первой строчки кода.
   Работы без issue не стартуют.
2. **High-level issue = атомарная единица доработки** (единица ценности,
   не пачка правок). Не создавать issue на каждый промежуточный шаг:
   косметика и подзадачи живут внутри задачи.
3. **Sub-issues — части реализации**: волны, этапы, разбивка на параллельных
   сабагентов (Task ID) оформляются sub-issues родительской задачи; прогресс
   агрегируется полем Sub-issues progress на доске.
4. **Доска.** Задачи (high-level и sub-issues) попадают на Projects #1
   (CanvasDesk): Todo → In Progress → Done; In Progress — одна волна за раз.
5. **Закрытие = факт проверки.** Issue/sub-issue закрывается только когда:
   гейты зелёные (`cargo test --workspace`, clippy `-D warnings`, fmt,
   `wasm_gate --check` / `mcp_wasm_gate.sh` — по контексту задачи), критерии
   приёмки из тела выполнены, WASM-проверка UI — по разделу «Самопроверка
   UI на WASM». При закрытии — комментарий-сводка: что сделано, ключевые
   коммиты, как проверено.
6. **Инструмент — `scripts/github_tasks.py`** (все команды идемпотентны;
   токены: `GITHUB_TOKEN` — issues, `GITHUB_PROJECT_TOKEN` — доска, см.
   шапку скрипта). Команда `plan` создаёт high-level задачу + sub-issues +
   доску одной командой:
   ```
   GITHUB_TOKEN=ghp_... python scripts/github_tasks.py plan --spec plan.json --dry-run
   ```

## Учёт токенов по задачам (трейсинг стоимости разработки)

**Каждая выполненная задача — реализация CR/FR, исправление дефекта, написание
документа, исследование — завершается фиксацией потраченных токенов** (правило
владельца от 2026-10-08). Данные нужны для сквозного трейсинга стоимости
разработки: задача → оценка → факт → калибровка будущих оценок.

Правила:

1. Токены считаются **на задачу** (scope — конкретный CR/FR/дефект/документ),
   а не на сессию целиком. Субагенты отчитываются по своим задачам так же.
2. Запись — в worklog (репозиторный `worklog.md`, в записи соответствующей
   задачи; системный журнал сессии — туда же, для исторических данных).
2а. **Ротация worklog** (issue #23, 2026-10-10): `worklog.md` — только
   текущий период (последние ~7 дней) + Journal index в шапке (одна строка
   на запись: дата, Task ID, scope, расположение — «текущий» или имя файла
   архива). Записи старше периода переносятся в `worklog/archive/`
   (один файл на период: `worklog-<год-мм-dd>_<мм-dd>.md`) в начале месяца
   или при превышении worklog'ом ~120 КБ. Перенос — без потерь (число
   записей до/после сверяется), текст записей не редактируется; строка
   Journal index обновляет расположение. Агент, читающий журнал, читает
   индекс и только нужные записи; архив по умолчанию не читается.
3. Формат строки:
   `Tokens: in≈<N>, out≈<N>, total≈<N>, model=<модели>, scope=<CR-019|FR-100|...>`.
4. Прямого доступа к счётчику API у агента нет — указывается **обоснованная
   оценка** с пометкой `estimate`: по объёму прочитанного/написанного текста
   (ориентир: ~4 символа ≈ 1 токен для смешанного RU/EN/кода) плюс накладные
   на контекст сессии и вызовы инструментов.
5. Оценки накапливаются в worklog как исторические данные и используются
   для калибровки трудоёмкости и стоимости; свод по scope делает владелец
   или отдельная задача агрегации.

## UI-кит — обязательное правило при вёрстке на Rust

**Любая вёрстка UI на Rust ведётся через существующий `canvas-ui` kit**
(`crates/canvas-ui/src/`). Хардкод квадов/цветов/геометрии в обход кита
запрещён — это ломает согласованность тем, переиспользование и делает
визуальные регрессии невидимыми до ручного теста.

### Что использовать из кита

| Нужда | Канонический путь в kit |
|---|---|
| Цвета UI-элементов | `KitPalette` (слоты `ThemeColors` v2: `control_fill`, `control_danger`, `text`, `palette_border`, `menu_fill`, …) — НИКОГДА не инлайнить `[f32;4]` литералы rgba |
| Отступы/радиусы/зазоры | `canvas_core::tokens` (`SPACING_*`, `RADIUS_*`) — НИКОГДА не дублировать `4.0/8.0/16.0` магическими константами |
| Размеры контролов | `kit::BUTTON_HEIGHT`, `kit::ICON_BUTTON_SIZE`, `kit::LIST_ROW_H`, `kit::TEXT_FIELD_HEIGHT`, `kit::CHIP_HEIGHT`, `kit::SWITCH_W/H` — НИКОГДА не переобъявлять `HEADER_H=30.0`, `ROW_H=26.0`, `INPUT_H=32.0` локально |
| Раскладка (позиционирование) | `kit::constrain`, `kit::stack`, `kit::pad`, `Column`/`Row`/`Child`/`MeasuredItem`/`RowPolicy` — НИКОГДА не считать `x = panel.right() - 28.0, y = panel.y + 6.0` магическими числами |
| Текст (шейпинг + ширина + перенос) | `TextMeasurer` (`width_of`, `wrap`, `ellipsis`) — НИКОГДА не оценивать ширину как `len() * 5.5 + 14.0` (CR-015 запрещает это явно) и не строить `Buffer::new` + `set_text` вручную в overlay-логике |
| Кнопки / чипы / иконки / свитчи / поля | `kit::button_layout/style/size`, `kit::chip_*`, `kit::icon_button`, `kit::switch`, `kit::text_field` |
| Списки и скролл | `kit::list_rows` + `kit::ScrollState` + `kit::scroll_bar` |
| Попапы / модали / тултипы | `kit::dropdown_menu`, `kit::modal`, `kit::tooltip` (anchor + flip + delay) |
| Панели / карточки | `kit::panel_rect/style`, `kit::card` |
| Таблицы | `kit::Table` (retained) или `kit::row_guides` + `kit::paint_row` |
| Draw-слой (квады на экран) | `Painter::rect/panel/control/label` + `PaintItem` (НЕ ручной `CardInstance { pos, size, fill, border, params, corners }`) |

Полный гайд — `docs/ui-kit.md`; архитектура — `docs/prd/prd-0009-ui-layering-uikit.md`,
контракт поверхности — `docs/interface-objects/surface-registry.md`,
токены — `crates/canvas-core/src/tokens.rs` и `docs/change-requests/fr-046-design-tokens.md`.

### Что запрещено (ловится на ревью и в линтах)

1. **Инлайн rgba литералы в UI-коде** — `[0.30, 0.75, 0.55, 1.0]`,
   `Color::rgba(77, 191, 140, 255)` в overlay/panel логике. Даже
   «единоразово» — это будущий дрейф цвета. Маршрутизируйте через
   `KitPalette` (существующий или новый слот).
2. **Магические числа геометрии** — `4.0, 6.0, 22.0, 28.0, 110.0` в
   `UiRect::new(...)` и `pos: [...]`. Берите из `kit::*` констант или
   `tokens::SPACING_*`.
3. **Дублированные локальные константы** — `HEADER_H`, `ROW_H`,
   `INPUT_H`, `FONT_*` в каждом `*_ui.rs`. Они обязаны либо `pub use`
   из `kit`, либо быть новым `kit::CONST` (если значение уникально).
4. **Ручной `Buffer::new` + `set_text` + `shape` в overlay-логике** —
   только через `TextMeasurer` (или `Shaper` под trait boundary, FR-068 W2).
   `set_text`+`shape_until_scroll` допустим только внутри kit/component
   и в `canvas-render/src/text.rs` (там это сам рендер).
5. **Эвристики ширины текста** — `len() as f32 * factor + pad`. Класс
   дефекта CR-015: разные глифы дают разную ширину, кириллица шире
   латиницы, эмодзи «съедают» место. Только `TextMeasurer::width_of`.
6. **Ручной `CardInstance { pos, size, fill, border, params, corners }`**
   в overlay-логике — маршрутизируйте через `Painter::rect/panel/control`
   или `KitDraw` (адаптер Painter↔`Vec<CardInstance>`). `CardInstance`
   напрямую — только в `canvas-render` (это его тип) и в адаптерах
   `app/support.rs` (граница слоёв).

### Если в kit чего-то не хватает

**Агент ОБЯЗАН предложить доработку kit, а не обходить его хардкодом.**
Порядок:

1. **Идентифицировать пробел** — какой компонент/токен/слот отсутствует,
   на каком паттерне повторяется в нескольких местах (≥ 2 файла →
   кандидат в kit).
2. **Предложить расширение kit** — в задаче/PR:
   - **Новый компонент** → добавить в `crates/canvas-ui/src/component/`
     (или расширить существующий) + экспорт через `crates/canvas-ui/src/kit.rs`. Контракт
     F-8 PRD-0009: только слоты палитры, только шкала токенов, текст
     только через `TextMeasurer`.
   - **Новый цветовой слот `KitPalette`** → добавить поле в
     `KitPalette` (`crates/canvas-ui/src/component/mod.rs`) +
     маппинг в `ThemeColors` (`crates/canvas-render/src/theme.rs`) +
     пресеты (`crates/canvas-render/src/theme_presets.rs`) + ключ в `REQUIRED_KEYS` (тест
     паритета семантики).
   - **Новый токен геометрии** → `crates/canvas-core/src/tokens.rs`
     (`SPACING_*`, `RADIUS_*`, высоты контролов) + зеркальный JSON
     в `design/tokens/` (тест паритета JSON↔Rust).
   - **Новый layout-паттерн** (radio_card, chat_bubble, crumbs,
     tree_layout, anchored_stack, footer_buttons, chip_strip,
     two_column, backdrop, banner) → компонент в
     `crates/canvas-ui/src/component/` + экспорт `crates/canvas-ui/src/kit.rs`.
3. **Оформить FR-документ** (если расширение значимое) по шаблону
   `docs/change-requests/cr-template.md`: What/Impact/Changes/Tests.
   Малые расширения (новый слот палитры) можно в коммите-задаче без FR.
4. **Реализовать доработку kit ПЕРВЫМ** — только после этого верстать
   поверхность через новый kit-компонент. Не наоборот.

### Чек-лист ревью UI-задачи

Перед сдачей задачи, затрагивающей UI (overlay/panel/dialog/контрол),
агент проверяет:

- [ ] Цвета берутся из `KitPalette`, нет инлайн rgba литералов
  (grep `\[\s*0\.[0-9]+\s*,\s*0\.[0-9]+` в изменённых строках).
- [ ] Геометрия из `kit::*` констант и `tokens::SPACING_*`/`RADIUS_*`,
  нет новых `const HEADER_H: f32 = 30.0` в `*_ui.rs`.
- [ ] Раскладка через `Column`/`Row`/`stack`/`constrain`/`pad`,
  нет ручных `x = panel.right() - MAGIC` формул.
- [ ] Текст через `TextMeasurer`, нет `Buffer::new` в overlay-логике
  и нет `len() * factor` эвристик ширины.
- [ ] Квады через `Painter::rect/panel/control` или `KitDraw`,
  нет ручных `CardInstance { ... }` литералов в overlay-логике.
- [ ] Если добавлен новый компонент/слот/токен — он в `canvas-ui`,
  а не в `canvas-app` (по G7: kit не зависит от рендера/ОС; рендер
  зависит от kit, не наоборот).
- [ ] Если что-то отсутствовало — агент явно заявил это в задаче
  (FR или коммит-заметка), не молча обойдя хардкодом.

### Исключения (acceptable hardcoding)

- **Тестовые фикстуры** — `KitPalette::default()` с `[0.0;4]` слотами
  для проверки геометрии, `Color::rgba(...)` в тестах (`*_ui.rs::tests`,
  `admin_ui.rs::test_palette`) — без UI-смысла, только asserts.
- **Адаптеры слоёв** — `app/support.rs::paint_items_to_band`,
  `app/overlays.rs::KitDraw` — граница Painter↔GPU-инстансы, `CardInstance`
  строится здесь по праву (это и есть адаптер kit→renderer).
- **Рендер** — `canvas-render/src/{cards.rs,renderer.rs,text.rs}` —
  это бэкенд GPU, `CardInstance` его собственный тип; токены
  (`tokens::EDGE_*`, `tokens::ACCENT`) уже каноничны.
- **Diagnostic overlays** — `crates/canvas-app/src/debug_overlay.rs` — цвета слоёв по
  дизайну «диагностические, не тема»; документировано в шапке файла.
- **Специализированные примитивы** (polar wheel в template_ui,
  sector SDF в `canvas-render/src/sectors.rs`) — escape-hatch через
  `Custom(rect)` с комментарием-обоснованием (G8 grep-аудит).

### Ссылки

- `docs/ui-kit.md` — гайд kit (3 шага добавить поверхность, layout-примитивы, измерение текста, линты).
- `docs/prd/prd-0009-ui-layering-uikit.md` — архитектура слоя UI/kit.
- `docs/prd/prd-0006-design-system-tokens.md` — design-токены (FR-046).
- `docs/change-requests/fr-046-design-tokens.md` — токены в коде.
- `docs/change-requests/fr-051-ui-layering-uikit.md` — слой UI (U1).
- `docs/change-requests/fr-053-ui-layering-u3-pilots.md` — layout-примитивы (U3).
- `docs/change-requests/fr-055-ui-layering-u4-kit.md` — kit v1 (U4).
- `docs/change-requests/fr-057-ui-kit-painter-widget-state.md` — Painter + WidgetState.
- `docs/dev-researches/ui-hardcode-audit.md` — аудит хардкода (пробелы kit, порядок миграции).

## Сборка и тесты

После T0 в репозитории должны работать (CI на ubuntu/windows/macos — матрица
M7, ветка `ci-matrix`; до её активации — windows-latest в ci.yml + 3-ОС
build-all):

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

WASM-гейт (FR-036, ADR-0011): ядро (canvas-core/canvas-render/canvas-widgets)
обязано собираться под wasm32-unknown-unknown, а canvas-core — исполняться в
wasm-рантайме; CI-джоба `wasm-check` проверяет компиляцию на каждый пуш,
локальный гейт — обе части. M8/W12 (wasm-port §6.1 п.4): в ступень компиляции
включён и продуктовый web-слой `canvas-web`:

```
scripts/wasm_gate.sh          # check wasm-таргета (core/render/widgets/mcp/web) + rlib ядра + тесты canvas-core под wasip1 (wasmtime)
scripts/wasm_gate.sh --check  # только компиляция — без wasmtime (эквивалент CI-джобы)
```

Web-бандл и деплой (M8/W12): `scripts/web_bundle.sh` — релизная сборка
canvas-web (trunk 0.21.14, `[profile.release] lto="thin"`), оптимизация
`wasm-opt -Oz` и отчёт о размере (§8.8: ≤8 МБ raw / ≤4 МБ brotli; в CI итог
дублируется в $GITHUB_STEP_SUMMARY). Публикация на GitHub Pages (путь `/app` +
Jekyll-сборка docs/ — как у прежнего branch-деплоя) — workflow `pages-web.yml`
(Source: «GitHub Actions», см. README «Веб-версия»).

MCP-wasm-гейт (FR-037, ADR-0012): контрактный слой (canvas-scene,
canvas-mcp, canvas-mcp-headless) собирается под wasm32-unknown-unknown,
тесты исполняются под wasip1 в wasmtime, и драйвер проводит РЕАЛЬНУЮ
MCP-сессию (initialize → tools/list → graph_apply oracle ±1 % →
analyze_bottlenecks ρ-гейт → негативные ветки) с headless-сервером —
регресс контракта ADR-0004 ловится без Windows и GUI:

```
scripts/mcp_wasm_gate.sh           # check + wasip1-тесты (scene 53 + мост 13 + headless 12) + e2e-сессия
scripts/mcp_wasm_gate.sh --check   # только компиляция — без wasmtime
```

Инспектор-сессия (FR-037 MW5) — живая ручная проверка MCP владельцем без
Windows, официальным инспектором `@modelcontextprotocol/inspector`
(требует node 18+/npx, первый запуск качает пакет; в гейты/CI не входит):

```
scripts/mcp_wasm_inspector.sh           # web UI: браузер → 127.0.0.1:6274, сервер предподключён
scripts/mcp_wasm_inspector.sh --check   # автоприёмка инспектором-клиентом: tools/list + graph_apply oracle ±1 %
```

Требования к тестам:
- Юнит-тесты для `canvas-core` обязательны (трансформации камеры round-trip, round-trip
  `.canvas` без потерь неизвестных полей, парсинг примеров с jsoncanvas.org).
- `canvas-shell` — интеграционные тесты там, где возможно (вотчер на tempdir и т.п.).
- Производительность — часть приёмки: 5 000 нод на 60 fps (пан/зум), холодный старт
  < 2 с, открытие канваса на 1 000 нод < 500 мс, память < 500 МБ (SPEC §6.3);
  нагрузочный тест `--stress N` (T5); 10 виджетов не роняют fps ниже 60 (M5).
- Ручная приёмка критериев milestone'ов (SPEC §10) выполняется владельцем после
  каждого тега (v0.1–v1.1) — агент её не заменяет.

## Самопроверка UI на WASM — обязательна перед отчётом

Правило (директива владельца 2026-09-25): **любое изменение, затрагивающее
UI (раскладка, ввод, панели, hit-тесты, рендер, темы), агент проверяет на
веб-сборке САМ** — нативные юнит-тесты закрывают логику, но не подменяют
поведенческую проверку на wasm-платформе. Порядок:

1. L0 — компиляционный гейт: `scripts/wasm_gate.sh --check` (или точечный
   `cargo check --target wasm32-unknown-unknown -p <крейт>`);
2. нативные тесты (`cargo test`) — логика;
3. L2 — браузерный стенд: `scripts/wasm_ui_test.sh` (сборка canvas-web без
   trunk + Chromium/WebGPU под Xvfb + сценарий с пиксельным диффом) —
   поведение. Сценарий/координаты — по рецепту `docs/WASM-TESTING.md` §3.

Если среда не позволяет L2 (нет node/playwright/Xvfb, сборка сломана,
диск/время) — в итоговом отчёте по задаче ОБЯЗАТЕЛЬНО: (а) явно указать,
что WASM-проверка не выполнялась и почему; (б) приложить инструкцию для
ручной проверки: что открыть (web-версия Pages/`trunk serve`), куда
кликнуть, что считается успехом. Формулировка «нативные тесты зелёные»
сама по себе UI-приёмку не закрывает.

## Не делать

- Не менять формат `.canvas` несовместимо с jsoncanvas.org без явной задачи;
  неизвестные поля и типы нод обязаны сохраняться при round-trip.
- Не добавлять сетевые вызовы в хост.
- Не блокировать рендер-поток (см. выше).
- Не слать 0x052C при существующем WorkerW; не использовать SPI_SETDESKWALLPAPER
  на raised desktop.
- Не заменять shell (таскбар, трей остаются Explorer).
- Не расползаться платформенным кодом по `canvas-app` — только трейты и
  cfg-секции платформенных крейтов (см. «Правила архитектуры» п.2); юникс-экв
  ачивенты Win32-приёмов — только по таблице решений `docs/plans/M7-crossplatform.md` §3.2, не по памяти.
