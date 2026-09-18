# FR-037: Верификация MCP-реализации в WASM — крейт canvas-scene, in-process транспорт и headless MCP-сервер (wasmtime/wasip1)

- **Статус:** выявлено (план реализации; исполнение — по приказу владельца) · **Дата:** 2026-09-18 · **ADR:** 0012 (предложено) · **Связанные:** FR-036 (прецедент wasm-гейта), FR-008 (подкоманда `mcp`), FR-034/FR-035 (транспорт и чистота stdio), ADR-0004 (агентная сборка), ADR-0009/0010/0011, `docs/plans/wasm-port.md` (M8: W2, §9 волна 2), `docs/plans/product-roadmap.md` (CP4/CP6 — потребители)

## Описание

Владелец приказал: «теперь спланировать реализацию MCP для WASM, чтобы
можно было проверять не только UI, но и реализацию MCP». После FR-036
в wasm верифицируется ядро (301 тест в wasmtime), UI проверяется частично
через Xvfb-демо, но MCP-слой — 27 инструментов и протокольный мост —
проверяется вне Windows только нативными юнит-тестами, а end-to-end
MCP-сессия невозможна в принципе (нужен named pipe + GUI). Этот документ
фиксирует план: вынести слой инструментов в платформенно-нейтральный крейт,
исполнять его и протокольный цикл под `wasm32-wasip1` в wasmtime и поднять
headless MCP-сервер, с которым внешний клиент (драйвер, инспектор, агент)
может проводить реальные сессии в Linux-контейнере — без Windows и GUI.

## Влияние

| Объект | Что меняется | Где в документации |
|---|---|---|
| `mcp_dispatch` + SceneState | Перенос из `canvas-app/src/main.rs` в новый крейт `canvas-scene`; UI-поля остаются в App | ADR-0012, SPEC §13/§3 (при реализации), AGENTS (структура workspace, гейты) |
| `canvas-mcp` (мост) | Рефактор `run_stdio` → `run_stdio_with_transport`; включение в wasm-гейты | ADR-0009/0012, SPEC §13 |
| MCP-тесты | ~40 тестов переносятся из main.rs в canvas-scene; добавляются протокольные e2e headless | ACCEPTANCE §26+ (при реализации) |
| Верификация агента | Новая способность: полная MCP-сессия в wasmtime (`scripts/mcp_wasm_gate.sh`) | AGENTS «Сборка и тесты» (при реализации) |
| План M8 | W2 сужается (SceneState уже вынесен); волна 2 «MCP WebSocket-мост» получает серверную сторону | `docs/plans/wasm-port.md` §6/§9 (примечание) |

Продуктовое поведение (Windows: pipe-сервер, автоспавн, GUI, подкоманда
`canvasdesk mcp`) — **не меняется**; формат `.canvas` — не затронут.

## Анализ (факты кода на main `b9e011c`)

1. **Инструменты уже чистые.** `mcp_dispatch` (`main.rs:6074`, ~1550 строк
   вместе с `mcp_flow_v2:7359`, `mcp_analyze_bottlenecks:7445`,
   `mcp_graph_apply:7485`) — «чистая функция над SceneState + Camera —
   тестируется без окна и pipe» (комментарий `main.rs:6070`). Хелперы:
   `mcp_unwrap_call` (`main.rs:5952`), `mcp_req_str/f32`,
   `mcp_node_index/summary`, `mcp_side`, `mcp_edge_json` (`main.rs:5999`–`6068`).
   Вызовы в `SceneState`: только `mark_dirty` (15), `push_undo` (14),
   `recompute_flow` (9), `move_node` (1), `ensure_reserve_at` (1) —
   модельные операции. `node_create_file` файл на диске не создаёт;
   автосейв — обязанность App, не диспетчера; `Instant` используется только
   в `mark_dirty` (на wasip1 исполняется корректно).
2. **Тесты уже чистые и их много.** MCP-модуль тестов `main.rs:12060+`
   (~2970 строк, ~40 тестов из 71 в main.rs), включая гейты эталонов:
   `mcp_fr029_instagram_mvp_reference` (`main.rs:14183`, CP1), e2e CP3
   (graph_apply), e2e CP5 (анализ узких мест). Конструктор сцены —
   `SceneState::new(Canvas::default(), PathBuf)` без ФС; реестр —
   `TemplateRegistry::builtin()` (embedded `include_dir`, wasm-OK).
3. **Мост уже чистый.** `canvas-mcp/src/lib.rs` (1529 строк, 15 тестов):
   трейт `AppTransport` (`lib.rs:60`), протокольный автомат
   `handle_line/handle_input` (`lib.rs:462/523`), stdio-цикл `run_stdio`
   (`lib.rs:594`); платформенное — только Windows-pipe/автоспавн под
   `cfg(windows)`/`cfg(any(windows, test))` (`lib.rs:655–723`). Зависимости:
   serde_json + anyhow (+ windows cfg) — wasm-совместимы; **но под
   wasm-таргеты крейт ни разу не проверялся** (гейт FR-036 покрывает только
   core/render/widgets).
4. **Camera в диспетчере — 4 метода.** viewport_get/set
   (`main.rs:6855–6876`) используют только `position()/zoom()/
   set_center()/set_zoom()` (`crates/canvas-render/src/camera.rs:32–49`).
5. **SceneState смешивает модель и ввод.** Поля `selected`
   (`Option<Selection>`, canvas-render), `selected_nodes`, `dragging`
   (`Option<DragState>`, canvas-app lib.rs:555) — состояние ввода UI:
   58 + 25 + 16 = 99 мест доступа в main.rs. Остальные поля — модельные и
   переносятся целиком; доступ `scene.canvas` (385 мест) и др. **не
   меняется** (тип переезжает, путь доступа остаётся `self.scene.*`).
6. **Гейт-инфраструктура готова к расширению.** `scripts/wasm_gate.sh`
   (3 ступени), `.cargo/config.toml` (runner wasmtime wasip1),
   CI `wasm-check` (`ci.yml:45`) — расширение списком крейтов тривиально.

## Требуемые изменения

### Архитектура (решение ADR-0012)

```
crates/
  canvas-core/            # без изменений (wasm ✓, 301 тест wasip1 ✓)
  canvas-render/          # без изменений (wasm-check ✓)
  canvas-widgets/         # без изменений (wasm-check ✓)
  canvas-mcp/             # мост: + run_stdio_with_transport<T>; входит в wasm-гейты
  canvas-scene/           # NEW: SceneState (модель) + модуль mcp (27 инструментов)
  canvas-mcp-headless/    # NEW (лист): HeadlessSession за AppTransport + bin canvasdesk-mcp-headless
  canvas-app/             # main.rs худеет на ~5k строк; UI-состояние ввода остаётся в App
scripts/
  mcp_wasm_e2e.py         # NEW: драйвер реальной MCP-сессии в wasmtime
  mcp_wasm_gate.sh        # NEW: гейт (сборка wasip1 → wasmtime → драйвер)
```

Направления зависимостей: `canvas-scene → canvas-core` (serde_json,
tracing); `canvas-mcp-headless → canvas-mcp + canvas-scene`;
`canvas-app → canvas-scene` (вместо внутреннего кода). Оба новых крейта —
листья для нативных бинарников (`canvasdesk-mcp-headless` в граф
`canvasdesk.exe`/`canvasdesk-mcp.exe` не входит). **Новые внешние
зависимости: ноль** (лицензионный гейт CP0 и `docs/DEPENDENCIES.md` не
трогаются).

Ключевые решения:

- **Viewport-зеркало.** В `SceneState` появляется значение
  `Viewport {x, y, zoom}`; инструменты viewport_get/set работают с ним.
  Синхронизация с `Camera` — в `on_mcp_wake` (`cfg(windows)`, ~8 строк):
  до вызова диспетчера — снять `position()/zoom()`, после — применить
  `set_center/set_zoom`. Числа идентичны, поведение не меняется.
  Альтернатива (зависимость canvas-scene от canvas-render::Camera)
  отклонена: тянет wgpu/glyphon в wasm-тестируемый крейт и создаёт
  wasip1-риск линковки — см. «Открытые вопросы», Q2.
- **Конверт приложения в in-process транспорте** повторяет `on_mcp_wake`
  буквально: `parse_envelope → (id) → mcp_unwrap_call → mcp_dispatch →
  build_result | build_call_error`; notification (id == None) — тишина;
  битый конверт — `build_error` с null-id. Один источник семантики —
  тесты обеих сторон обязаны сходиться на одних кейсах.
- **Тестовые переносы без изменений ассертов.** Переносимые ~40 тестов
  сохраняют имена (`mcp_*`) и ассерты; меняется только конструкция
  viewport (вместо `Camera::default()` — `Viewport::default()` с теми же
  значениями 0/0/1). Число тестов workspace не убывает — 1034 остаётся
  нижней границей.

### Задачи (каждая — сессия = коммит, AGENTS; порядок строгий)

| ID | Объём | Задача и критерий приёмки |
|----|-------|---------------------------|
| MW1 | M | **Крейт `canvas-scene`.** Перенос SceneState (модельные поля + методы, `main.rs:551+`), модуль `mcp` (диспетчер + хелперы + `next_free_id` из `canvas-app/src/lib.rs:359`, реэкспорт для UI), `Viewport` + синк в `on_mcp_wake`; UI-поля `selected/selected_nodes/dragging` → поля App (~99 механических правок `self.scene.selected` → `self.selected`); перенос ~40 MCP-тестов (~2970 строк). Приёмка: нативные гейты зелёные (тестов ≥ 1034, те же имена/ассерты); `cargo check --target wasm32-unknown-unknown -p canvas-scene` ✓; `RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-scene` ✓ (~40 тестов в wasmtime); `cargo test -p canvas-shell` (pipe round-trip) ✓ |
| MW2 | S | **Мост под wasm.** `run_stdio` → `pub fn run_stdio_with_transport<T: AppTransport>` (чистое выделение; `run_stdio` — обёртка, поведение FR-008/034/035 не меняется); wasm-таргеты моста в гейтах; точечные `#[cfg(not(target_arch = "wasm32"))]` на тестах автоспавна (`std::process` в test-cfg — test-only cfg, прецедент FR-036). Приёмка: `cargo check --target wasm32-unknown-unknown -p canvas-mcp` ✓; `cargo test --target wasm32-wasip1 -p canvas-mcp` ✓ (тесты моста в wasmtime); регресс FR-008/034/035 — ноль |
| MW3 | M | **`canvas-mcp-headless`.** `HeadlessSession` (SceneState + `TemplateRegistry::builtin()`) с impl `AppTransport`; bin `canvasdesk-mcp-headless` = `run_stdio_with_transport(Some(session))`; lib-тесты протокольного цикла: initialize (эхо версии 2025-06-18) → tools/list (27) → tools/call → graph_apply мини-эталон №1 → flow oracle ±1% → analyze_bottlenecks (ρ-гейт CP5) → негативные ветки (isError неизвестного инструмента, −32601, −32700, batch, notification-тишина). Приёмка: lib-тесты под wasip1 ✓; ручная сессия: `wasmtime run target/wasm32-wasip1/debug/canvasdesk-mcp-headless.wasm` отвечает на initialize/tools_list через stdio |
| MW4 | M | **Гейт, CI, документация.** `scripts/mcp_wasm_e2e.py` (драйвер сессии: build wasip1 → wasmtime → сценарий MW3 + сохранение лога сессии для разбора падений) + `scripts/mcp_wasm_gate.sh`; CI `wasm-check` += `-p canvas-mcp -p canvas-scene -p canvas-mcp-headless` (компиляция; исполнение — локально, прецедент ADR-0011); AGENTS (структура workspace, «Сборка и тесты»: wasm-гейт теперь покрывает MCP-слой), SPEC §13/§3, ACCEPTANCE § «MCP-WASM-верификация» (сценарий 5 минут), wasm-port.md — примечание (уже добавлено этим документом). Приёмка: `scripts/mcp_wasm_gate.sh` — полный зелёный прогон в чистом контейнере; CI зелёный; документация синхронна |
| MW5 (опция) | S | **Инспектор-сессия владельца.** `scripts/mcp_wasm_inspector.sh`: обёртка для `npx @modelcontextprotocol/inspector` поверх wasmtime-запуска headless-сервера — живая ручная проверка MCP без Windows (устраняет зависимость ручной приёмки MCP от Windows-машины). Приёмка: инспектор подключается, tools/list отображается, graph_apply из UI инспектора сходится с oracle |
| MW6 (опция) | S | **Файловый режим headless.** `--canvas <путь>`: загрузка/сохранение `.canvas` через предоткрытый каталог wasmtime (`--dir`), автосейв — по shutdown-флагу. Приёмка: round-trip модель-файл-модель в wasmtime без потерь (инвариант формата) |

Суммарно MW1–MW4: ~3–5 фокус-дней. MW5/MW6 — по решению владельца (см.
«Открытые вопросы»).

### Точки входа (обновить при реализации)

- `AGENTS.md` — структура workspace (два новых крейта), раздел «Сборка и
  тесты» (wasm-гейт покрывает MCP-слой: `scripts/mcp_wasm_gate.sh`).
- `docs/SPEC.md` — §3 (строка wasm-таргетов: MCP-слой), §13 (MCP:
  headless-верификация, ссылка на гейт).
- `docs/ACCEPTANCE.md` — новый § «MCP-WASM-верификация» (сценарий:
  initialize → tools/list → graph_apply → oracle).
- `docs/plans/wasm-port.md` — примечание после §6 (связь W2/волны 2) —
  внесено этим коммитом.
- `docs/adr/README.md`, `docs/change-requests/index-cr-fr.md` — индексы
  (внесено этим коммитом); ADR-0012 — статус «принято» после одобрения.

## Проверка (критерии приёмки FR в целом)

1. `cargo check --target wasm32-unknown-unknown -p canvas-core
   -p canvas-render -p canvas-widgets -p canvas-mcp -p canvas-scene
   -p canvas-mcp-headless` — зелёный (расширение гейта FR-036).
2. `RUST_TEST_THREADS=1 cargo test --target wasm32-wasip1 -p canvas-core
   -p canvas-scene -p canvas-mcp` — зелёный; суммарно ≥ 356 тестов в
   wasm-рантайме (301 core + ~40 scene + ~15 мост).
3. `scripts/mcp_wasm_gate.sh` — полная MCP-сессия в wasmtime: initialize →
   tools/list = 27 → graph_apply мини-эталон №1 → числа эталона ±1%
   (ADR-0005/0006) → analyze_bottlenecks: базовая линия здорова,
   DAU×2 → Warn, DAU×5.35 → Overload (гейт CP5).
4. Нативный регресс — ноль: `cargo test --workspace` ≥ 1034 (состав
   сохранён); `cargo clippy --workspace -- -D warnings`, `cargo fmt
   --check` — чистые; pipe round-trip `canvas-shell` зелёные.
5. Поведение Windows-продукта не изменилось: подкоманда `canvasdesk mcp`,
   автоспавн, offline-режим — тесты FR-008/034/035 зелёные без правок
   ассертов.

## Не-цели (границы)

- Продуктовый веб-слой (W1–W12 M8: web_time, canvas-web, OPFS,
  wasm-bindgen-фасады) — бэклог S5 роадмапа; FR-037 не создаёт его.
- WebSocket-мост MCP (волна 2 M8 §9) — не реализуется; проектируется
  стык: `HeadlessSession` = серверная сторона будущего моста.
- CI-исполнение wasmtime-ступени — остаётся локальным (прецедент
  ADR-0011); перенос в CI — при появлении потребности.
- Полный вынос App в lib (остаток W2) — отдельная задача после FR-037.
- Статистика/параллелизм (волна S) — вне отношения к этому FR.

## Открытые вопросы (решение за владельцем; агент рекомендует)

| # | Вопрос | Рекомендация агента | Альтернативы |
|---|--------|--------------------|--------------|
| Q1 | Имена крейтов: `canvas-scene` + `canvas-mcp-headless`? | Да: роль первого — модель сцены (не только MCP), второй — лист-крейт по паттерну `canvas-web` (§3.5 M8) | Слить headless в `canvas-mcp` фичей `in-process` + `required-features` у второго bin (меньше крейтов, сложнее фича-гигиена); `canvas-mcp-tools` вместо scene (уже занят смыслом: там и undo/expr-кэш) |
| Q2 | Viewport: зеркало в SceneState или зависимость от canvas-render::Camera? | Зеркало + синк в `on_mcp_wake`: canvas-scene остаётся без wgpu (быстрые wasm-сборки, нет wasip1-риска линковки) | Трейт `ViewportCamera` с impl для Camera в canvas-app (без зеркала, но generics по всем вызовам); прямая зависимость (тянет wgpu/glyphon в тестируемый крейт — не рекомендуем) |
| Q3 | Включать wasmtime-ступень в CI сейчас? | Нет — локально, как FR-036 (CI-минуты дороги; компиляция в CI достаточна до накопления флака-статистики) | Да: отдельная джоба с `curl wasmtime` (медленнее, но гейт на каждый пуш) |
| Q4 | MW5 (инспектор-скрипт для ручных сессий владельца) — делать? | Да, S-объём: снимает зависимость ручной MCP-приёмки от Windows-машины | Отложить до волны V (демо) |
| Q5 | MW6 (файловый режим `--canvas`) в v1? | Нет: сессионный in-memory сценарий закрывает верификацию; файлы добавляют ФС-переменные | Да, если нужна проверка round-trip формата в wasm (уже покрыта тестами core) |

## Changelog

- 2026-09-18 — создан план (агент, приказ владельца «спланировать
  реализацию MCP для WASM…»): ADR-0012 (предложено), декомпозиция
  MW1–MW4 + опции MW5/MW6, критерии приёмки, 5 открытых вопросов;
  индексы (adr/README, index-cr-fr) и примечание в wasm-port.md внесены
  этим же коммитом. Реализация не начата.

## Источники истины

- `docs/adr/adr-0012-mcp-wasm-verification.md` — решение (варианты, границы).
- `docs/adr/adr-0011-wasm-build-gate.md` + FR-036 — прецедент wasm-гейта,
  runner wasmtime, wasip1.
- `crates/canvas-app/src/main.rs` — `mcp_dispatch:6074`, хелперы
  `5947–6073`, `SceneState:551`, `on_mcp_wake:5908`, тесты `12060+`,
  эталон `mcp_fr029_instagram_mvp_reference:14183`.
- `crates/canvas-mcp/src/lib.rs` — `AppTransport:60`, `handle_line:462`,
  `handle_input:523`, `run_stdio:594`, 15 тестов.
- `docs/plans/wasm-port.md` — §2 (факты wasm), §3.5 (лист-крейты), §6 (W2),
  §9 (волна 2: MCP WebSocket-мост).
- `docs/plans/product-roadmap.md` — §9 CP4/CP6 (потребители MCP-слоя),
  §4.5 S5 (продуктовый WASM — граница этого FR).
