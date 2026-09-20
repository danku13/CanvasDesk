# FR-043: Всеядный импорт машиночитаемых описаний архитектуры (по PRD-0003)

- **Статус:** в анализе
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** агент (оформление реализации PRD-0003 по запросу владельца, сессия 2026-09-20)
- **Источник:** `docs/prd/prd-0003-universal-architecture-import.md` — этап I0 дорожной карты PRD («FR-043 по шаблону `cr-template.md` + фиксация решений Q1–Q3»); исходный запрос владельца от 2026-09-20
- **Связанные задачи:** PRD-0003 (требования), FR-032 (`graph_validate`, коды E-*/W-*), FR-033 (`graph_apply` — прецедент батч-мутаций графа), FR-010 (автораскладка `layout.rs`), FR-011/FR-012 (группы: `type: "group"` + `children`), FR-006 (undo-стек), FR-008 (single-exe), ADR-0008 (allowlist зависимостей, без нативного C), ADR-0011 (wasm-гейт), PRD-0001/PRD-0002 (смежные: данные→вводные, LOD плотных графов); SPEC.md §5.1/§6.3/§8; `docs/DEPENDENCIES.md` §3
- **Создан:** 2026-09-20
- **Обновлён:** 2026-09-20

## Описание

Требуется реализовать PoC всеядного импорта: пользователь бросает на канвас файл поддержанного формата (draw.io, mermaid, PlantUML, Graphviz DOT, docker-compose, terraform, ansible) или вставляет текст диаграммы — продукт распознаёт формат, строит структурный граф (ноды, связи, группы), показывает превью со счётчиками и предупреждениями W-* и после подтверждения размещает архитектуру на канвасе (координаты формата или автолейаут). Импортированные ноды — обычные ноды канваса: к ним применимы расчётные шаблоны, value-рёбра и все существующие жесты (US-7 PRD).

Выявил агент: в коде нет ни одного парсера сторонних форматов, нет промежуточного представления (IR) «структура → граф канваса», нет UI-контракта импорта (превью/опции/диагностика) и нет политики безопасности входных файлов. Отсутствует модуль `canvas-core/src/import/` (проверено 2026-09-20: в `crates/canvas-core/src/` модуля нет; grep `mermaid|plantuml|drawio|hcl|terraform` — ноль вхождений). Продуктовое обоснование, сценарии, CJM с точками отвала и non-goals — в PRD-0003; этот FR фиксирует «где и как».

## Решения I0 (фиксация открытых вопросов PRD §11 — может переопределить владелец до этапа I2)

| Вопрос PRD | Решение | Обоснование |
|---|---|---|
| Q1 — порядок must-форматов | **mermaid → DOT → PlantUML → draw.io** (этапы I2a–I2d) | От текстовых подмножеств к XML: рост сложности парсера и фикстур; каждый шаг отдаёт самостоятельную ценность (mermaid — самый частый формат README) |
| Q2 — лимиты PoC | **Текст ≤ 5 МБ, ≤ 2000 импортируемых нод** (как в PRD §9.4) | Стартовые значения достаточно щедрые для эталонных демо (30–100 нод) и безопасные для перф-бюджета G5; пересмотр по замерам I5 |
| Q3 — draw.io swimlane/страницы | **Swimlane/pool (container=1) → группа** канваса; **страницы mxfile — выбор в превью**: одна выбранная страница или все страницы как отдельные группы одного канваса | Прямо следует из AC-1.1/AC-1.3 PRD; вариант «отдельные канвасы» отклонён — усложняет undo/мерж |
| Q6 — подсказки шаблонов (F-17) | **Вне PoC (v2)** | Точка отвала D5 закрыта origin-метаданными (F-14); F-17 — эвристический слой без golden-критериев, расширяет скоуп без вклада в G1–G5 |

Вопросы Q4 (продюсеры UML XMI), Q5 (Kubernetes/Helm), Q7 (кодировки — PoC: UTF-8), Q8 (`.tfstate`) — остаются открытыми, все за пределами PoC (PRD §10).

## Влияние

| Объект/подсистема | Что меняется | Где в документации |
|---|---|---|
| `canvas-core` | Новый платформенно-нейтральный модуль `src/import/` (IR, парсеры, конвертер) — pure Rust, wasm-безопасно | PRD-0003 §7.2, SPEC.md §3 |
| Зависимости | +5 крейтов: `quick-xml`, YAML-парсер, `hcl-rs`, `flate2` (backend `miniz_oxide`), `base64` — все MIT/Apache (allowlist ADR-0008), без нативного C | `docs/DEPENDENCIES.md` §3, ADR-0008 |
| Формат `.canvas` | Новых обязательных полей нет; только `canvasdesk.import` в `extra` (round-trip Obsidian сохраняется) | SPEC.md §5.1, `docs/interface-objects/node.md` |
| Drag-drop / вставка | Расширение списка структурных расширений (`.drawio/.xml/.mmd/.puml/.dot/.yml/.yaml/.tf`) + распознавание текста в буфере | SPEC.md §8, user-docs |
| UI (desktop/web) | Новый диалог-превью импорта (оверлей по образцу `dialog`/`docs_ui.rs`), опции размещения, мерж/новый канвас | PRD-0003 §7.2, US-6 |
| MCP | Без изменений в PoC (`graph_import` — v2, PRD §4 Персона 4) | CR-013 |
| user-docs | Новая страница «Импорт» + синк индексов | `user-docs/README.md`, `index.md` |

## Анализ (что отсутствует и на что опираемся)

**Якоря графа (проверены 2026-09-20):** `Canvas` — `crates/canvas-core/src/model.rs:678`, `Node` — `model.rs:171`, `Edge` — `model.rs:416`, `next_edge_id` — `model.rs:732`, механизм `extra` — `CanvasdeskExt` (`model.rs:123`), round-trip `extra` — `crates/canvas-core/src/io.rs` (чтение/запись `.extra`, строки 162–186). Валидация — `validate.rs` (FR-032: коды E-CYCLE/E-OVERLOAD/E-UNIT/E-PORT-UNKNOWN/E-DOUBLE-INPUT, W-AMBIGUOUS-SRC/W-UNUSED-SLOT — стабильный контракт, переиспользуем для W-отчёта импорта). Автораскладка — `layout.rs` (FR-010). Батч-прецедент — `graph_apply` (FR-033): импорт = тот же контракт детерминированных мутаций, источник — парсер, а не агент. Drag-drop сегодня: `canvas-core/src/dragdrop.rs`, `canvas-shell/src/dragdrop/` (нативно), `canvas-web/src/drop_files.rs` (`MAX_DROP_FILE_BYTES = 64 МБ`) — чужой файл создаёт файловую ноду, структура не извлекается.

**Зависимости (все новые; в `crates/canvas-core/Cargo.toml` сейчас только `serde/serde_json/toml/rstar/tracing/include_dir/web-time/thiserror`):**

| Крейт | Назначение | Лицензия | Риски |
|---|---|---|---|
| `quick-xml` | draw.io mxGraph XML (сжатый mxfile: flate2+base64) | MIT/Apache | Без резолвинга DTD — защита от entity-expansion |
| YAML-парсер | compose, ansible inventory | MIT/Apache | ⚠️ `serde_yaml` архивирован автором (2024) — кандидат `serde-yaml-ng` (fork, MIT/Apache) или собственный парсер подмножества; финальное решение на I1 по критериям: allowlist ADR-0008 + wasm-безопасность + живость сопровождения |
| `hcl-rs` | terraform `.tf` (блоки/атрибуты/ссылки; выражения — opaque) | MIT | Зрелость на полном HCL — риск из PRD §12; план B — собственный парсер заголовков блоков |
| `flate2` | распаковка сжатых mxfile | MIT/Apache | Backend `miniz_oxide` — pure Rust; ограничение коэффициента распаковки (zip-бомбы) |
| `base64` | mxfile payload | MIT/Apache | — |

Регистрация кандидатов в `docs/DEPENDENCIES.md` §3 («Кандидаты на будущее — только по продуктовому триггеру»): триггер — этот FR.

## Требуемые изменения

| # | Изменение | Где | Этап |
|---|---|---|---|
| 1 | **IR**: `ImportGraph { nodes, edges, groups, warnings }`; узел `{source_id, label, kind_hint, pos?, size?, color?, group_path?}`; ошибки `ImportError`; детерминированные чистые функции (стабильная сортировка входов и id-маппинг: одинаковый файл → одинаковый канвас) | `canvas-core/src/import/mod.rs`, `ir.rs` | I1 |
| 2 | **Конвертер IR → Canvas**: id-префиксация (по образцу `next_edge_id`, `model.rs:732`), группы `type: "group"` + `children` (FR-011/FR-012), координаты формата либо вызов автолейаута `layout.rs` (FR-010); прогон `validate.rs` (FR-032) до применения | `canvas-core/src/import/convert.rs` | I1 |
| 3 | **Golden-фикстуры инфраструктура**: `tests/import/fixtures/<format>/…` (минимальный + реальный экспорт на must-формат), тесты парсеров и детерминизма (два прогона — байт-в-байт); правило «парсер без golden-фикстур не принимается» (PRD §12) | `crates/canvas-core/tests/import/` | I1 |
| 4 | **Парсер mermaid**: flowchart-подмножество — узлы с метками, `A -->\|label\| B`, `subgraph` → группа | `import/mermaid.rs` | I2a |
| 5 | **Парсер DOT**: `digraph`, атрибуты `label/color/style`, `subgraph cluster_*` → группа, `pos` → координаты | `import/dot.rs` | I2b |
| 6 | **Парсер PlantUML**: component/deployment-подмножество (`component/actor/database/queue/node`, `-->`/`..>`, `package/rectangle`); `!include`/`!includeurl` не разворачиваются — W | `import/plantuml.rs` | I2c |
| 7 | **Парсер draw.io**: mxfile сжатый/несжатый, `mxCell` vertex/edge, `geometry` → world-координаты, `fillColor` → цвет, контейнеры → группы, страницы — решение Q3 | `import/drawio.rs` | I2d |
| 8 | **Парсеры IaC**: compose (`services`, `depends_on`/`links`, `networks`), terraform (`resource`, `module` 1 уровень, `depends_on`/ссылки, `count`/`for_each` — пометка множественности), ansible (inventory INI/YAML, plays → связи) | `import/compose.rs`, `import/terraform.rs`, `import/ansible.rs` | I3 |
| 9 | **Диалог-превью**: формат, счётчики нод/связей/групп, W-отчёт с пагинацией, опции (новый канвас/мерж, координаты/автолейаут, выбор страницы draw.io); отмена не меняет канвас; инвариант G3 — ни одна вставка без показанного отчёта | `canvas-app` (нативно) + `canvas-web` (паритет, G4) | I4 |
| 10 | **Вход**: расширение drag-drop (структурные расширения), распознавание формата по содержимому (F-2, не по расширению), вставка текста Ctrl+V (`@startuml`/`flowchart`/`digraph`) | `dragdrop.rs`, `drop_files.rs`, обработчик вставки | I4 |
| 11 | **Origin-метаданные**: `canvasdesk.import` = `{format, source, ts}` в `Canvas.extra` + на нодах; тултип происхождения (US-7 AC-7.2) | `model.rs` (`CanvasdeskExt`), `io.rs` | I4 |
| 12 | **Мерж в текущий канвас**: префиксация импортированных id (F-16, should), один undo-шаг (FR-006) | сцена/undo-стек | I4 |
| 13 | **Безопасность входа**: лимиты Q2, XML без DTD-резолвинга, ограничение глубины/коэффициента распаковки, диагностика нераспознанного (причина + список форматов + лимиты, F-15), без исполнения IaC, без сети (AGENTS.md) | парсеры + входной слой | I1–I4 |

## Точки входа

- `docs/change-requests/fr-043-universal-architecture-import.md` — этот FR (реестр `index-cr-fr.md` обновлён тем же коммитом).
- `docs/prd/prd-0003-universal-architecture-import.md` — статус «в работе (FR-043 создан)», история §16 (этот коммит).
- `docs/DEPENDENCIES.md` §3 — кандидаты quick-xml/YAML/hcl-rs/flate2/base64 (при I1, первой кодовой правке).
- `docs/SPEC.md` §3 (зависимости), §5.1 (`canvasdesk.import`), §8 (ввод) — при I4/I5.
- `docs/interface-objects/node.md` — происхождение импортированных нод, группы — при I5.
- `user-docs/` — страница «Импорт» + синк `user-docs/README.md`, `index.md` (относительные `*.html`-ссылки, тест `docs_ui` линков) — при I5.
- `docs/ACCEPTANCE.md` — приёмочный чек-лист PoC — при I5.
- `docs/prd/README.md` — статус каталога (этот коммит).

## Проверка

Чек-лист PoC (Definition of Done — по PRD §15; ручная приёмка оформляется в `docs/ACCEPTANCE.md` на I5):

1. Golden-фикстуры: по каждому must-формату ≥ 2 (минимальный + реальный экспорт); тесты парсеров, конвертера и детерминизма зелёные.
2. Демо-сценарии: draw.io на 30 нод → канвас ≤ 10 с (G1); terraform-пример 50 ресурсов → граф с зависимостями и группами модулей; mermaid-блок из README → граф вставкой текста.
3. Инвариант превью (G3): ни одна вставка не происходит без показанных счётчиков и W-отчёта; отмена превью оставляет канвас байт-в-байт прежним; подтверждение мержа — один undo-шаг.
4. Round-trip `.canvas` с `canvasdesk.import` — без потерь (`json_canvas_io.rs`); Obsidian-совместимость не нарушена.
5. Зловредные фикстуры: XML entity-expansion и deflate-бомба — отвергаются лимитами с внятной диагностикой (F-15), без паники.
6. Гейты: `cargo fmt`, `cargo clippy -D warnings`, `cargo test --workspace`, `scripts/wasm_gate.sh` — зелёные (G4, ADR-0011); после импорта 500 нод — 60 fps пан/зум, автолейаут ≤ 2 с (G5, замер `--stress`).
7. Web-паритет: импорт работает в web-сборке (парсеры в `canvas-core`, wasm-безопасные зависимости).

## История изменений

- `2026-09-20` — агент: создан документ (FR-043) по этапу I0 PRD-0003: скоуп PoC перенесён из PRD (US-1…US-7, F-1–F-18), зафиксированы решения Q1–Q3 и Q6, план I1–I5, зависимости с замечанием по `serde_yaml` (архивирован — кандидат на замену, решение I1), DoD; статус «в анализе».

## Источники истины

- `docs/prd/prd-0003-universal-architecture-import.md` — требования, CJM, non-goals, риски, дорожная карта (§13).
- `crates/canvas-core/src/model.rs` (`Canvas` — 678, `Node` — 171, `Edge` — 416, `CanvasdeskExt` — 123, `next_edge_id` — 732), `crates/canvas-core/src/io.rs` (round-trip `extra`), `crates/canvas-core/src/validate.rs` (E-*/W-*), `crates/canvas-core/src/layout.rs` (FR-010), `crates/canvas-core/src/dragdrop.rs`, `crates/canvas-shell/src/dragdrop/`, `crates/canvas-web/src/drop_files.rs` (`MAX_DROP_FILE_BYTES`).
- `docs/change-requests/fr-032-graph-read-validate.md`, `fr-033-graph-apply-batch.md`, `fr-010-related-cards-auto-layout.md`, `fr-011-mindmap-object.md`, `fr-012-node-insert-into-group.md`, `fr-006-undo-stack.md`.
- `docs/adr/adr-0008-math-computing-stack.md` (allowlist `MIT, Apache-2.0, BSD-2/3, ISC, Zlib`, без нативного C), `docs/adr/adr-0011-wasm-build-gate.md`, `docs/DEPENDENCIES.md` §3.
- `docs/prd/prd-0001-duckdb-data-connector.md` (FR-041), `docs/prd/prd-0002-edge-lod-main-stage.md` (FR-042) — смежные PRD-волны.
- Внешние спецификации: jsoncanvas.org/spec/1.0; mermaid.js (flowchart); plantuml.com (component/deployment); graphviz.org (DOT); draw.io/mxGraph (mxfile, deflate+base64); hashicorp HCL spec; compose-spec; docs.ansible.com (inventory, playbook).
