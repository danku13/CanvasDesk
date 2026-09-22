# FR-054: Слои, вёрстка и UI kit — этап U5 PRD-0009: миграция 4 поверхностей, лестница on_key на KeyboardRouter, layout-линты в CI, docs/ui-kit.md

- **Статус:** выполнено (U5)
- **Тип:** FR
- **Приоритет:** критично (завершающий этап PoC PRD-0009 — G8 и DoD §15)
- **Владелец:** агент (по приказу владельца «Продолжай U5»)
- **Источник:** PRD-0009 §13 этап U5; FR-051 (U1), FR-052 (U2), FR-053 (U3)
- **Связанные задачи:** PRD-0009 Q4-a, F-11, G4, G5, G8, DoD §15; FR-053 (примитивы/TextMeasurer); AGENTS.md (вопрос об онбординге — отвечен владельцем в приказе)
- **Создан:** 2026-09-22
- **Обновлён:** 2026-09-22

---

## 1. Описание (What)

Этап U5 дорожной карты PRD-0009 — завершающий:

1. **Миграция оставшихся 4 поверхностей** на layout-примитивы + TextMeasurer + токены (U3-механика пилотов): **панель поиска** (`canvas-render::search_ui`), **модалка настроек** (`settings_ui`), **просмотрщик документации/help** (`docs_ui`), **палитра шаблонов** (`template_ui`: развёрнутый док, свёрнутая полоса, flyout). Вместе с пилотами U3 — 6 поверхностей на реестре+примитивах (гейт G8).
2. **Вывод лестницы `on_key` на `KeyboardRouter`** (Q4-a): head-диспетчер `KeyOwner` (ручной match поверх `esc_stack`) заменяется доставкой по скоуп-стеку роутера из реестра; Esc-лестница остаётся на `esc_stack` (уже реестровая с U2); NUMI-хоткеи канваса и лестница команд канваса не тронуты (Q4).
3. **Layout-линты в CI (F-11)**: сквозной G4-линт полного кадра (`build_frame_at` → `overlaps_within_layer` + выходы за вьюпорт) на канонических состояниях приложения × 3 окна (1280×800, 1024×640, 800×560) × RU/EN — исполняется `cargo test --workspace` (гейт CI).
4. **`docs/ui-kit.md`** — гайд каркаса («поверхность за 3 шага», примитивы, измеренный текст, политики).
5. **Interface-objects** — контракт поверхности реестра `docs/interface-objects/surface-registry.md` (дополнение к node.md/edge.md, US-5).
6. **Ретро PoC** — итоги U0–U5 в PRD-0009.

Решения владельца в приказе: **онбординг не дорабатывать** («если для пользователя ничего не менялось, то доработок в онбординг не нужно») — миграция поверхностей U5 сохраняет пользовательское поведение, онбординг-поверхность (L5) не входит в 4 целевые и не затрагивается. Пользовательская документация не меняется (поведение/хоткеи прежние).

Этап **U4 (UI kit v1 F-8, DebugOverlay F-10, витрина, scissor-бакеты)** владельцем заказан не был и в U5 не входит; его гейт G6 остаётся за U4. U5 исполняется поверх механики U3 (примитивы/TextMeasurer/токены) без кит-виджетов — G8 этого не требует («реестр+примитивы»).

## 2. Влияние (Impact)

| Объект | Что меняется | Где |
|---|---|---|
| Панель поиска | раскладка примитивами (Stack/Column/constrain); единый формат rect'ов xywh≡UiRect (исправление конвертации hit-rect'а в реестре); заголовки результатов — без ручных срезов | `canvas-render/src/search_ui.rs`, `canvas-app/src/app/ui_registry.rs` |
| Настройки | `modal_layout`/`dropdown_layout` примитивами (constrain размера, stack-центрирование, Column строк, Row карточек тем); значения — токены spacing | `canvas-app/src/settings_ui.rs` |
| Docs/help | вёрстка страниц — измеренный текст (`TextMeasurer` вместо консервативной эвристики `0.62·кегль`); `viewer_rect`/меню помощи примитивами | `canvas-app/src/docs_ui.rs` |
| Палитра шаблонов | ширина чипов категорий — измеренная (вместо `chars·7.5+20`); чипы — `SqueezeTail` (вместо молчаливого `break`-клампа); `panel_layout`/`dock_strip_layout`/flyout примитивами | `canvas-app/src/template_ui.rs` |
| Клавиатура | head `on_key` = `KeyboardRouter::deliver` по скоуп-стеку реестра (тела веток — в методы `route_key_*` дословно); `KeyOwner`-match выводится | `canvas-app/src/app.rs`, `app/ui_registry.rs` |
| CI-линты | сквозной G4-тест полного кадра на канонических состояниях × 3 окна × RU/EN (F-11) | `canvas-app/src/ui_layout_lint.rs` (test-only модуль) |
| Документация | `docs/ui-kit.md` (новый), `docs/interface-objects/surface-registry.md` (новый), PRD-0009 (статус/ретро), index-cr-fr, docs/prd/README.md, docs/ACCEPTANCE.md | `docs/` |

## 3. Анализ (Root Cause — что осталось от PRD-0009)

1. **4 поверхности всё ещё на ручной математике** (PRD-0009 §17): `search_ui.rs:294–335` (клампы ширины/скролла), `settings_ui.rs:726–825` (рассыпанная арифметика модалки), `template_ui.rs:662–752` + эвристика `category_chip_width` (`chars·7.5+20`, урок CR-015 того же класса, что what-if `0.62·кегль`), `docs_ui.rs:410–426` (`CHAR_W_FACTOR`/`SPACE_W_FACTOR` — вся вёрстка страниц на консервативной оценке: строки переносятся раньше фактической границы, текст «недоиспользует» ширину панели).
2. **Молчаливый `break`-кламп чипов палитры** (`template_ui.rs:697–699`): категории за правым краем панели исчезают без следа (тот класс дефекта, что U3 устранил в галерее break-клампом → примитив `SqueezeTail`).
3. **Двойная запись клавиатуры**: head `on_key` — ручной match `KeyOwner` (9 веток, зеркалит `esc_stack`), Esc — реестр (с U2). Источник рассинхронов при добавлении поверхности (R-5 PRD): новая поверхность обязана помнить и про реестр, и про match. `KeyboardRouter::from_registry` (FR-051) существует с U1, но не используется приложением.
4. **G4-линты пилотов U3 — локальные** (по модулю на поверхность); сквозного линта полного кадра нет: пересечение интерактивных rect'ов ДВУХ разных поверхностей одной полосы ловится только `overlaps_within_layer` кадра (FR-051), в CI не исполняется.
5. **Расхождение форматов rect'ов**: `search_ui::PanelLayout` — xyxy, остальные — xywh; адаптер реестра U2 конвертирует search-rect как xywh → hit-rect панели поиска покрывает лишнюю область (право/низ экрана) — расхождение pick и видимой панели.

## 4. Требуемые изменения (Changes)

### 4.1 Поиск (`canvas-render/src/search_ui.rs`)

- `layout()` — примитивами: панель = `stack` (top-center), внутренняя колонка = `Column{gap: PANEL_PADDING}` [поле ввода, строки…], ширина = `constrain` (мин 0, макс PANEL_WIDTH, желаемая vw−2·маржа); вырожденное окно — прежнее схлопывание в точку. Числа PanelLayout — дословно прежние (тесты структуры остаются).
- Формат rect'ов PanelLayout унифицирован до xywh (комментарий-контракт); конвертация в адаптере реестра — корректная (фикс п.5 анализа); потребители (`app.rs:4975` `rect_xywh`, ввод 10129) — сверены.
- TDD: тест-линт панели (3 окна), pick-тест реестра «клик по панели/мимо».

### 4.2 Настройки (`canvas-app/src/settings_ui.rs`)

- `modal_size` = `constrain(min=MODAL_MIN, max=viewport−2·SETTINGS_MARGIN)`; центрирование = `stack(Center, Center)`; `modal_layout` = pad + Row (навигация | контент) + Column (строки, gap 0) + Row (карточки тем, gap 10); `dropdown_layout` — прежняя логика клампов с комментарием (popup-семантика FR-021, не маскируемая примитивами) — R-3 `Custom`-исключение.
- Числа — дословно прежние (тесты `modal_layout_*`, `control_rects_*`, `dropdown_layout_clamps` остаются).

### 4.3 Docs (`canvas-app/src/docs_ui.rs`)

- `text_width`/`CHAR_W_FACTOR`/`SPACE_W_FACTOR` удалены; `wrap_spans`/`layout_page`/табличная раскладка принимают `&mut TextMeasurer` + `&mut FontSystem` (паттерн пилотов U3: measurer создаётся на вызов перекомпоновки, не на кадр — layout_page кэшируется по (страница, ширина)).
- Перенос слов — по измеренной ширине слова/пробела (реальный шейпинг); свойство «строка не вылезает за ширину» сохраняется по построению (сумма измеренных ширин ≤ max_w).
- `viewer_rect`, `help_menu_rect`, `help_submenu_*` — `stack`/`Column` от вьюпорта; числа прежние.
- Тесты переноса обновлены на измеренные ширины (детерминированный FontSystem — паттерн `measure.rs`-тестов).

### 4.4 Палитра шаблонов (`canvas-app/src/template_ui.rs`)

- `category_chip_width(name)` → измеренная ширина (TextMeasurer; паддинг 20 — прежний, значение — токен `chip`-пада); сигнатуры `panel_layout`/`dock_strip_layout` принимают measurer+fs (call-sites: `app.rs` draw/input, `ui_registry::fill_hit_rects`).
- Чипы панели: `Row{policy: SqueezeTail}` — вместо `break`-клампа (хвост сжимается, не исчезает; переполнение видно линту).
- `panel_layout` — Column [header, input, chips, rows…, footer] с резервом футера (CR-011); строки-шаблоны — прежняя пагинация MAX_VISIBLE_ROWS/прокрутка (именованное поведение, не кламп); `collapse_rect` — `Row` от края панели.
- Эвристика `:77–79` удалена (G8-аудит модуля).

### 4.5 Клавиатура → KeyboardRouter (Q4-a)

- `on_key`: `let router = KeyboardRouter::from_registry(&registry);` → `router.deliver(|a| self.route_key(a.surface.as_str(), event))`; `route_key` — тела прежних head-веток (`Onboarding/Gallery/Editor/Search/TemplatePanel/Dialog/Explain/Stage`) дословно, `true` = поглотил. Несохранённые скоупы (settings-dropdown и пр.) не поглощают → событие уходит вниз к Canvas-scope — лестнице команд/NUMI (без изменений, Q4).
- Esc-лестница — без изменений (`registry.esc_stack()` + `dispatch_esc`, с U2); «Stage закрывается любой клавишей» — через route_key(STAGE).
- `KeyOwner`/`key_owner` выводятся из боевого пути; тесты U2 переописаны через `router.deliver` (эквивалентность «роутер == прежний head» — TDD).

### 4.6 CI-линты (F-11)

- Новый test-only модуль `canvas-app/src/ui_layout_lint.rs` (паттерн `scheme_cjm_tests`): канонические состояния (поиск, настройки+dropdown, docs, help-меню, палитра док/полоса+flyout, галерея, онбординг, диалог, stage, what-if, wheel, хоткеи, empty, idle) × {1280×800, 1024×640, 800×560} × {RU, EN}:
  - `frame.overlaps_within_layer()` пуст (0 пересечений интерактивных rect'ов разных поверхностей одной полосы);
  - каждый интерактивный hit-rect внутри вьюпорта (0 выходов; hide-деградации whatif — учтены);
  - pop-матрица U2 зелёная на тех же состояниях (регресс-нить R-4).
- Исполняется `cargo test --workspace` → попадает в CI автоматически (F-11 «линты в CI»).

### 4.7 Документация

- `docs/ui-kit.md` — гайд: слои/политики/реестр, «поверхность за 3 шага», примитивы и политики переполнения, TextMeasurer/ellipsis, линты G4/G5, статус кита (примитивы U3 готовы; кит-виджеты F-8 — U4).
- `docs/interface-objects/surface-registry.md` — контракт поверхности (US-5): декларация, hit-rect'ы, capture-контракт, keyboard-scope, деградация; таблица 21 поверхности.
- PRD-0009: статус «U0–U5 выполнены», changelog U5, ретро-секция (итоги PoC: дифф-проверка добавления поверхности, метрики линтов, сознательные отказы); index-cr-fr (указатель — следующий номер); docs/prd/README.md; docs/ACCEPTANCE.md — US-1–US-5 сценарии.

## 5. Точки входа

- `docs/ui-kit.md` (новый, §14 PRD-0009), `docs/interface-objects/surface-registry.md` (новый, US-5).
- `docs/prd/prd-0009-ui-layering-uikit.md` — §3 статус, §13 этап U5, §15 DoD, §16 история/ретро.
- `docs/change-requests/index-cr-fr.md` — строка FR-054.
- `docs/prd/README.md`, `docs/index.md` — статус PRD-0009.
- `docs/ACCEPTANCE.md` — приёмочные US-1–US-5.
- `docs/SPEC.md` §6 — не меняется (бюджеты рендера не задеты; scissor — U4).

## 6. Проверка (Verification)

- G4: сквозной линт `ui_layout_lint` зелёный на всех состояниях × 3 окна × RU/EN (0 пересечений/0 выходов).
- G5: grep-аудит мигрированных модулей — 0 `take(`-срезов, 0 `break`-клампов раскладки, 0 `truncate_chars`; эвристики `CHAR_W_FACTOR`/`category_chip_width` удалены.
- G8: 6 поверхностей на реестре+примитивах (search, settings, docs, template panel/strip, whatif, gallery) — grep-аудит литеральной rect-математики.
- R-4: все z-тесты, pick-матрица U2, esc-тесты, stage-тест — зелёные; клавиатурная эквивалентность роутера покрыта тестом.
- Гейты: fmt / clippy -D warnings / test --workspace / wasm_gate.sh / mcp_wasm_gate.sh — зелёные; wasm-прирост ≤100 КБ (G7, 0 новых внешних зависимостей).
- Онбординг: не изменён (решение владельца; поверхность не входит в 4 целевые) — вопрос AGENTS.md закрыт приказом.

## 7. История изменений

- `2026-09-22` — агент: статус `выполнено (U5)`. Реализация: 6 поверхностей на примитивах/измеренном тексте (G8; эвристики `CHAR_W_FACTOR`/`SPACE_W_FACTOR`/`chars·7.5+20` удалены; чипы палитры — SqueezeTail), on_key → `KeyboardRouter::deliver` (Q4-a; NUMI/лестница команд не тронуты), сквозной CI-линт `app::ui_layout_lint` (F-11). Линт и тесты роутера вскрыли 4 дефекта, погашенных в этапе: (1) конвертер hit-rect'ов реестра трактовал xywh как xyxy — завышенные зоны pick всех поверхностей; (2) панель хоткеев налезала на полосу палитры (сдвиг правее, `hotkeys_panel_rect_at`); (3) модаль настроек (Panels) рисовалась под полосой/карточкой — слой Panels→Modals; (4) регресс U2: док палитры в фокусе терял клавиатуру при видимой палитре выделения (роутер-проход по скоуп-стеку). Дельты нормализованы и зафиксированы в PRD-0009 §16.1/индексе. Гейты: fmt/clippy -D warnings/test 1509/wasm/mcp-wasm — зелёные; онбординг не изменён (решение владельца — пользовательских изменений нет).
- `2026-09-22` — агент: создан документ (`FR-054`), статус `в работе` — приказ владельца «Продолжай U5» (+ решение: онбординг не дорабатывать, пользовательских изменений нет).

## 8. Источники истины

- `docs/prd/prd-0009-ui-layering-uikit.md` — §7.4, §11 Q4, §13 (этап U5), §15 (DoD), §17 (модули ручной математики).
- `crates/canvas-ui/src/{layout,measure,keyboard,frame}.rs` — примитивы, TextMeasurer, роутер, кадр/линты (FR-051/FR-053).
- `crates/canvas-render/src/search_ui.rs:294–335`, `crates/canvas-app/src/{settings_ui,docs_ui,template_ui}.rs`, `crates/canvas-app/src/app.rs` (`on_key`, `search_overlay`), `crates/canvas-app/src/app/ui_registry.rs` — точки миграции.
- `docs/change-requests/fr-053-ui-layering-u3-pilots.md` — механика пилотов (паттерн переноса measurer).
- AGENTS.md — дисциплина гейтов; вопрос об онбординге — отвечен владельцем в приказе U5.
