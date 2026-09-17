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
