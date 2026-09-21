# FR-049: Шаблоны готовых схем — библиотека, инстансер, галерея и онбординг-мосты (T1–T5 PRD-0008)

- **Статус:** выполнено (v1, 2026-09-21)
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** danku13 (владелец продукта)
- **Источник:** PRD-0008 (docs/prd/prd-0008-canvas-scheme-templates.md, статус
  «в анализе»); решение владельца «давай реализуем PRD-0008» (сессия
  2026-09-21). Открытые вопросы PRD (Q1–Q5) закрываются предложенными в
  PRD вариантами по умолчанию: Q1 — манифест двуязычный, содержимое схем
  RU-заметки + латинские переменные и EN-единицы (кириллические единицы —
  v2 таблицы FR-013); Q2 — состав 6 схем §7.2 PRD; Q3 — вставка сразу в
  центр вьюпорта без диалога (undo покрывает откат); Q4 — `seed_canvas`
  сохраняется, empty-state работает при 0 нод; Q5 — MCP-инструменты v2.
- **Связанные задачи:** PRD-0008 (§7 решение, §8 F-1..F-9), FR-028
  (шаг карусели — действие), FR-031 (меню «?» — пункт галереи), FR-019
  (паттерн реестра `TemplateRegistry` + `include_dir`), FR-018
  (`instantiate` — образец чистого инстанса), FR-025/FR-029
  (value-рёбра/порты), FR-006 (undo-снапшоты), FR-040 (i18n RU/EN),
  `crates/canvas-scene/src/scene.rs:178` (`next_free_id`), `:191`
  (`seed_canvas`), `:334` (`recompute_flow`), `docs/SPEC.md` §5.1/§6.3
- **Создан:** 2026-09-21
- **Обновлён:** 2026-09-21
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

PRD-0008 постановил концепцию шаблонов готовых схем: built-in библиотека
целых канвасов (ноды + value-связи + Numi-расчёты), открываемая в текущий
канвас в один клик и встроенная в онбординг четырьмя мостами (empty-state,
действие на шаге карусели FR-028, пункт меню «?», web `?template=`).
Принцип: **шаблон = контент, не функциональность** — схема собирается из
существующих примитивов (text/group ноды, value-рёбра), формат `.canvas`
не расширяется, каждый шаблон верифицируется oracle-тестами.

FR ведёт реализацию T1–T5 дорожной карты PRD-0008: T1 — формат пакета и
реестр (canvas-core); T2 — чистый инстансер и апплаер (canvas-scene);
T3 — галерея, empty-state, мосты онбординга (canvas-app); T4 — стартовый
набор из 6 схем с oracle-тестами; T5 — web-параметр. Полнота постановки —
в PRD-0008 (§2 контекст, §3 метрики G1–G7, §6 CJM, §7 решение, §15 DoD);
FR не дублирует их, а фиксирует исполнение.

## Исследование бест-практик (Research)

Бест-практики (Miro first-run, Figma product tour, NN/g «пропуск виден
всегда», nag-limit) собраны в PRD-0008 §6/FR-028 §Research — здесь
фиксируются только кодовые паттерны репозитория, которые FR переиспользует
без новых решений:

- **Реестр как данные** — `EMBEDDED_TEMPLATES: include_dir` + `TemplateRegistry`
  с кэшем (`crates/canvas-core/src/templates.rs:38`, `:483`): схемы
  включаются тем же механизмом (`assets/canvas-schemes`, `include_dir`),
  0 I/O в hot path, wasm-совместимо (ADR-0011).
- **Чистый инстанс** — `instantiate` (`templates.rs:924`): манифест +
  окружение → нода, ошибки — типизированные `InstantiateError`. Для схем —
  `instantiate_scheme`: ремап id через `next_free_id` (`scene.rs:178`),
  bounding box → origin, чистая функция над `(манифест, &Canvas)`.
- **Поток вставки UI** — `instantiate_template_at` (`app.rs:4416`):
  `push_undo()` → вставка → `spatial.insert` → выделение → `mark_dirty()`
  → `recompute_flow()`. Апплаер схемы повторяет последовательность для N
  нод + M рёбер (`Canvas::add_edge`, `model.rs:795`).
- **Value-семантика** — рёбра схемы создаются `Edge::new` + `set_flow_kind`
  (как `create_edge`, `app.rs:10298`); вход нижестоящей ноды — `$in`
  (`Env::with_inbound`, `flow.rs:417`); единицы — только EN-токены таблицы
  FR-013 (`expr.rs:238-254`: ms/s/min/h, req, rps, req/s, B/KB/MB/GB, $/usd, %).
- **Screen-space панели** — паттерн `settings_overlay` (`app.rs:6915`) и
  палитры `TemplatePanel` (`template_ui.rs:103`): квады + тексты по кадру,
  hit-тесты в обработчике мыши, Esc двухэтапный. Галерея строится по этому
  образцу (собственный модуль, без врастания в `settings_ui`).
- **Камера** — `Camera::set_center`/`set_zoom`/`visible_world_rect`
  (`canvas-render/src/camera.rs:32-98`): zoom-to-fit содержимого схемы —
  вычисление bbox и обратная проекция в центр вьюпорта.

## Влияние (Impact)

| Объект | Что меняется | Где |
|---|---|---|
| `assets/canvas-schemes/<id>/scheme.json` | 6 новых пакетов схем (контент) | новый каталог ассетов |
| `canvas-core/src/schemes.rs` | новый модуль: `SchemeManifest` (serde), валидатор, `SchemeRegistry` (`include_dir` + `OnceLock`) | canvas-core |
| `canvas-scene/src/scheme_apply.rs` | новый модуль: чистый `instantiate_scheme` (ремап id, позиция) + `SchemeInstance` | canvas-scene |
| `canvas-app/src/scheme_gallery_ui.rs` | новый модуль: состояние и раскладка галереи; empty-state карточка | canvas-app |
| `canvas-app/src/app.rs` | хуки: открытие галереи (клавиша/меню/шаг тура/empty-state), клавиатура, клики, отрисовка, `apply_scheme` (undo/recompute/fit) | canvas-app |
| `canvas-app/src/onboarding_ui.rs` | `OnboardingStep.action: Option<OnboardingAction>` — шаг 7 «Шаблоны нод» получает «Попробовать» | canvas-app |
| `canvas-app/src/i18n.rs` | ~14 ключей RU/EN (галерея, empty-state, пункт меню) | canvas-app |
| `canvas-web/src/url_params.rs` | `WebParams.template` + разбор `?template=` | canvas-web |
| `canvas-web/src/app_spawn.rs` | авто-вставка схемы после инициализации App | canvas-web |
| `docs/SPEC.md`, `docs/interface-objects/*`, `user-docs/*`, `docs/ACCEPTANCE.md`, `docs/prd/README.md`, `docs/index.md` | точки входа §14 PRD-0008 | docs |

## Анализ (Root Cause — чего нет в коде)

Пробелы зафиксированы PRD-0008 §2.2 (формат пакета, реестр, мульти-инстансер,
галерея, empty-state, мосты онбординга, web-параметр). Зацепки — §2.3.
Дополнительные факты разведки FR:

- `OnboardingStep` (`onboarding_ui.rs:29`) не имеет поля `action` —
  резервация v1 существовала только комментарием (`onboarding_ui.rs:27`);
  поле добавляется опциональным — машина состояний не переписывается,
  существующие литералы 8 шагов дополняются `action: None`.
- `WebParams` (`url_params.rs:19`) расширяется мягким параметром `template`
  по образцу `canvas` (санитизация не нужна: id проверяется реестром —
  неизвестный id = тост + тихий старт).
- Единицы схем — только EN-токены таблицы FR-013 (кириллические синонимы
  отложены владельцем на v2 таблицы — `expr.rs:232-234`).

## Требуемые изменения (Changes)

### 1. Формат пакета `assets/canvas-schemes/<reverse-dns>/scheme.json`

```json
{
  "id": "com.canvasdesk.scheme.project-budget",
  "name_ru": "Бюджет проекта", "name_en": "Project budget",
  "description_ru": "…", "description_en": "…",
  "category": "planning", "category_ru": "Планирование", "category_en": "Planning",
  "version": "1.0.0",
  "content": {
    "nodes": [ {"id": "in-users", "type": "text", "text": "…Numi…", "x": 0, "y": 0, "width": 260, "height": 160, "color": "4"} ],
    "edges": [ {"id": "e1", "fromNode": "in-users", "toNode": "calc", "fromSide": "right", "toSide": "left", "flowKind": "value"} ]
  }
}
```

Правила валидатора: id схемы уникален (`com.canvasdesk.scheme.*`), id нод
уникальны внутри пакета, рёбра ссылаются на ноды пакета, белый список типов
`text`/`group`, обязательные координаты/размеры, цвет нод — только пресеты
JSON Canvas `1`..`6` (без hex — токен-линт и темы PRD-0006), лимиты
≤ 200 нод / ≤ 400 рёбер, обязательные RU/EN поля, `flowKind` — `value`
(value-ребро с `$in`) или отсутствие (обычная связь).

### 2. canvas-core: `schemes.rs`

- `SchemeManifest` (serde, `#[serde(default)]` для необязательных) +
  `SchemeContent` + `SchemeNode`/`SchemeEdge`.
- Валидатор `validate(&self) -> Result<(), SchemeValidationError>` — чистая
  функция, вызывается реестром на загрузке и тестами ассетов.
- `SchemeRegistry::shared() -> &'static SchemeRegistry` — `include_dir!`
  `assets/canvas-schemes`, разбор + валидация в `OnceLock`, API
  `list() -> &[&SchemeManifest]`, `get(id) -> Option<&SchemeManifest>`.

### 3. canvas-scene: `scheme_apply.rs`

- `pub struct SchemeInstance { pub nodes: Vec<Node>, pub edges: Vec<Edge>,
  pub bbox: [f32; 4] }`.
- `pub fn instantiate_scheme(manifest, canvas: &Canvas, origin: Vec2) ->
  Result<SchemeInstance, SchemeInstantiateError>` — ремап id (префиксы
  `note`/`group` через `next_free_id`), рёбра по карте id, сдвиг bbox к
  origin. Чистая функция — wasm-гейт покрывает тестами.
- Oracle-тесты: каждая built-in схема инстанцируется в пустой канвас →
  `SceneState::recompute_flow` → контрольные значения строк (G2 PRD-0008).

### 4. canvas-app: галерея, empty-state, мосты

- `scheme_gallery_ui.rs`: `SchemeGalleryState` (open/selected/category/
  search), раскладка панель-карточек (`panel_layout`-стиль), empty-state
  карточка. Отрисовка — квады/тексты по кадру из слотов `ThemeColors`
  (без новых констант — правило потока PRD-0006 §7.1).
- `app.rs`: поле `scheme_gallery`, открытие из 4 точек (клавиша, меню «?»,
  действие шага 7 тура, empty-state), клавиатура (↑↓/Enter/Esc), клики,
  `apply_scheme(manifest)`: `push_undo()` → вставка нод/рёбер → spatial →
  `recompute_flow()` → zoom-to-fit bbox (G3/G7).
- `onboarding_ui.rs`: `OnboardingAction::OpenSchemeGallery`, шаг 7 —
  кнопка «Попробовать» (открывает галерею, закрывает тур).
- i18n: ключи `gallery.*`, `empty.*`, `help.schemes` — RU/EN (тест полноты
  таблиц существующий).

### 5. canvas-web: `?template=<id>`

- `WebParams.template: Option<String>`, мягкий разбор; после инициализации
  App — `apply_scheme_by_id` (неизвестный id — тост, тихий старт; US-5).

### 6. Стартовый набор (Q2 — состав §7.2 PRD-0008)

`intro-calculations`, `intro-whatif`, `capacity-service`, `project-budget`,
`unit-economics`, `renovation-estimate` — контент на существующих
примитивах, единицы EN, заметки-подсказки RU, переменные латиницей (Q1).

## Точки входа

- `docs/SPEC.md` §5 (ассеты схем), §6.3 (бюджет G7), §8 (входы галереи).
- `docs/interface-objects/scheme-gallery.md` — новый; `onboarding.md` —
  дополнение (действие шага 7).
- `user-docs/quick-start.md` — сценарий «первые шаги через шаблон».
- `docs/ACCEPTANCE.md` — секция FR-049.
- `docs/prd/README.md` (статус PRD-0008 → «в работе»), `docs/index.md`.
- `worklog.md` репозитория — запись реализации.

## Проверка

- Oracle: все 6 схем — инстанс в пустой канвас + `recompute_flow` +
  контрольные значения (автотест); вставка в непустой канвас — 0 коллизий id.
- Undo: ровно один шаг на открытие схемы (автотест App/scene).
- Round-trip: канвас со схемой сохраняется/читается (JSON Canvas валиден).
- Гейты: `cargo fmt --all --check`; `CARGO_INCREMENTAL=0 cargo clippy
  --workspace --all-targets -- -D warnings`; `CARGO_INCREMENTAL=0
  CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace`;
  `scripts/wasm_gate.sh`; `scripts/mcp_wasm_gate.sh`; токен-линт.
- Web: `?template=` покрыт юнит-тестом парсера; wasm-гейт зелёный.
- Бюджет: суммарный вес ассетов схем ≤ 100 КБ (замер в PR/CI логе).

## История изменений

- `2026-09-21` — агент (сессия владельца): создан FR-049 (v1 PoC) по
  PRD-0008; статусы: FR «в работе»; Q1–Q5 PRD закрыты предложенными
  вариантами по умолчанию (решение владельца «давай реализуем»).
- `2026-09-21` — агент: реализация T1–T5 выполнена (ядро/инстансер/набор —
  коммит feat(core,scene); UI/онбординг-мосты — feat(app); web —
  feat(web,app)); статус → «выполнено (v1)». Уточнение примера манифеста:
  поле `accent` не введено (цвета нод — пресеты JSON Canvas в content) —
  эталон формата в `schemes.rs`/ассетах. Гейты: fmt/clippy/test (46
  бинар)/wasm/mcp-wasm/токен-линт — зелёные.

## Источники истины

- PRD-0008 (docs/prd/prd-0008-canvas-scheme-templates.md) — постановка,
  метрики, CJM, решение, DoD.
- Решение владельца (сессия 2026-09-21): «давай реализуем PRD-0008».
- Код: `templates.rs:38/:483/:924`, `scene.rs:178/:191/:334/:897`,
  `app.rs:4416/:6915/:10298`, `camera.rs:32-98`, `onboarding_ui.rs:27/:29`,
  `url_params.rs:19/:215`, `model.rs:795/:817`, `expr.rs:238-254`.
- FR-028/FR-031/FR-019/FR-025/FR-029/FR-006/FR-040 — смежные контракты.
