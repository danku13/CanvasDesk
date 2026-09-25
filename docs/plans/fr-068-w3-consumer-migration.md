# FR-068 W3. Каталог миграции потребителей на компонентное API

- **Статус:** в исполнении — W3.1 выполнено 2026-09-25 (пилот: whatif_ui бар
  → `MeasuredItem` × `Row::lay_out_measured`, бит-в-бит; Child::fixed в
  canvas-app 42 → 32). **W3.2 выполнено 2026-09-25:** settings_ui (12),
  scheme_gallery_ui (10), template_ui (5), search_ui (3) → `MeasuredItem`
  через `Row::lay_out_measured` (F-13) и НОВЫЙ `Column::lay_out_measured/_with`
  (вертикальный симметричный аналог F-13); потребители canvas-app/canvas-render:
  32 → 1 (демо kit_ui F-14 — решение W3.3 за владельцем); W3.3 — план.
- **Дата:** 2026-09-25
- **Задача-источник:** FR-068 (`docs/change-requests/fr-068-ui-refactoring-long-term.md`),
  §Волна W3 «своя UI-библиотека», строка таблицы «Миграция canvas-app/render
  (~497 мест)» и гейт `grep -r "Child::fixed" crates/ | wc -l`
- **Автор:** агент 3-d (волна W3); base-коммит среза `2c67bff` (сплит
  kit.rs → `component/*` до Component-слоя агентов 3-a…3-d)
- **Связанные:** FR-062 (F-13 `MeasuredItem`, F-14 flex, F-15 `Wrap`),
  FR-059/FR-060 (волны миграции kit v1), ADR-0014/ADR-0015, PRD-0009 §7.4 (V-5)

---

## 1. Метод среза

Счётчики сняты на base-коммите `2c67bff` рабочей ветки
`feature/fr-068-w3-agent-3d` (из корня репозитория):

```text
rg -n "Child::fixed" crates/canvas-app/src crates/canvas-render/src   → 42 строки (42 вхождения)
rg -n "\.lay_out\("  crates/canvas-app/src crates/canvas-render/src   → 14 строк
rg -n "Child::fixed" crates/                                          → 184 строки
```

Плановая оценка FR-068 «~497 мест» снята до W1 и считалась шире (все
layout-примитивы `Row{`/`Column{`/`stack`/`pad` + `Child::fixed` + `lay_out`).
Фактическая поверхность прямого перевода «`Child::fixed` → `MeasuredItem`» и
«`Row{...}.lay_out(...)` → `Component::layout`» — **42 + 14 мест**; ещё ~30
мест — ручные проводки `width_of → размер` без `Child::fixed`
(см. §2, строку «ручные проводки»). Расхождение с оценкой планирования
(~9×) учтено в перефиксации гейта (§4).

## 2. Фактическая поверхность по файлам

| Файл | `Child::fixed` | `.lay_out(` | `Row{`/`Column{` | Характер использования |
|---|---:|---:|---:|---|
| `canvas-app/src/settings_ui.rs` | 12 | 5 | 5 | Скелет модалки настроек: 2 колонки (`Row` gap 0), nav-колонка (`Column` константной высоты `MODAL_NAV_ITEM_H`), content-flow `[пад, заголовок, зазор 4, контент]`, карточки тем 2×(`Row`), flow строк тем |
| `canvas-app/src/whatif_ui.rs` | 10 | 1 | 1 | What-if бар: ручная проводка `chip_width`/`btn_width` (`width_of` + пад) → `Child::fixed`, политика `SqueezeTail`, `CrossAlign::Center` |
| `canvas-app/src/scheme_gallery_ui.rs` | 10 | 4 | 4 | Скелет панели галереи (`Column` + распорки `SPACING_S`), чипы категорий (константные `CHIP_W`), строки окна видимости, кнопки пустого состояния (2 в ряд, `cross End`) |
| `canvas-app/src/template_ui.rs` | 5 | 3 | 3 | Скелет панели шаблонов (`Column` gap `SPACING_S`), чипы категорий с `RowPolicy::Wrap` (ручная `category_chip_width`), кнопка сворачивания (`spacer` + `fixed 22×22`) |
| `canvas-render/src/search_ui.rs` | 3 | 1 | 1 | Колонка оверлея поиска: поле + распорка + строки результатов; слот `h = INFINITY`, обрезка — `viewport_clamp` (W0) |
| `canvas-app/src/kit_ui.rs` | 2 | 0 | 6 | Демо-галерея: фиксированный ребёнок 64 px рядом с `flexible` (демонстрация семантики grow FR-062 F-14); остальная галерея уже на `lay_out_measured`/`grid_cells`/`lay_out_with(pilot_backend())` — 21 место (пилот FR-068 W1) |
| **Итого потребители** | **42** | **14** | **20** | |

Потребители из списка FR-068 с **нулём** `Child::fixed` (проверено тем же
срезом): `explain_ui.rs` (уже `kit::modal`), `palette.rs` (ручная геометрия
колеса — вне поверхности миграции), `docs_ui.rs` (ручной перенос текста
`width_of` — ~20 мест, кандидат на `MeasuredItem::Text`/`TextMeasurer::wrap`
отдельным решением), `debug_overlay.rs` (1 `width_of`-лейбл), `app.rs`
(доменные структуры, `Child` не использует); `admin_ui`/`calc_panel_ui`/
`flowmap_ui`/`hints_ui`/`autolink_ui`/`onboarding_ui` — 0/0.

### 2.1 kit-строки (Row-потребители) — особая группа

`kit::row_guides`/`kit::row_layout`/`kit::paint_row` (перенесены в
`component/row.rs`, агент 3-d добавил `impl Component for Row`) потребляют:

- `canvas-app/src/app/stage.rs` (тело ноды FR-061: var/формульные строки на
  ОБЩИХ направляющих ноды — свой проход A),
- `canvas-app/src/app/overlays.rs`, `canvas-app/src/admin_ui.rs`
  (демо/оверлеи на общих направляющих),
- `canvas-app/src/kit_ui.rs` (демо-галерея кита).

`Component`/`RowProps` v1 (агент 3-d) — **однострочный**: `Row::layout_row`
строит направляющие одной строки от края слота. Таблицы с общими
направляющими (проход A по всем строкам ноды) остаются на kit-функциях
(`row_guides` + `row_layout` — сигнатуры стабильны, §Контракт-1); их перевод
на компоненты — отдельное решение (Table-компонент v2, вне среза W3.1–W3.3).
Однострочные потребители (элементы списков, строки футеров) могут переходить
на `Row`/`RowProps` сразу — API совместим (паритет-тест
`component_layout_matches_row_layout_oracle`).

## 3. Группировка по подсистемам и волнам миграции

| Подсистема | Файлы | fixed/lay_out | Предлагаемый компонент (`component/*`) / API | Риск | Волна |
|---|---|---|---|---|---|
| What-if бар (чипы сценариев) | `whatif_ui.rs` | 10 / 1 | `MeasuredItem::Text{text, max_w, min_w}` в `lay_out_measured` (политика `SqueezeTail` поддержана 1:1; ручные `chip_width`/`btn_width` удаляются — закрытие класса CR-015 по построению); позже — `chip_layout` | **низкий** — оракул эквивалентности `measured_row_matches_manual_fixed_oracle` (layout.rs), тесты `integration_whatif` | **W3.1** |
| Оверлей поиска (render) | `search_ui.rs` | 3 / 1 | `list_rows` (`LIST_ROW_H`-константы) + `text_field`; высоты — дизайн-константы → `MeasuredItem::Fixed` там, где текст не источник размера | низко-средний (слот ∞-высоты; golden demo 14) | **W3.1** |
| Чипы категорий шаблонов | `template_ui.rs:764–773` | 1 / 1 | `MeasuredItem::Text` + `RowPolicy::Wrap` (поддержан `lay_out_measured`); `category_chip_width` остаётся для `min_w`/пада | **низкий** — те же ширины тем же `TextMeasurer` | **W3.1** |
| Скелеты панелей (Column + распорки) | `scheme_gallery_ui.rs`, `template_ui.rs:738–743` | 8 / 2 | `panel_rect`/`panel_content` + `Column`-компонент; распорки — `MeasuredItem::Spacer` | средне-низкий (механический перенос тех же размеров) | **W3.2** |
| Модалка настроек (каркас) | `settings_ui.rs` | 12 / 5 | 2-колоночный `Row` + nav-колонка `list_rows` + карточки тем `card` (panel.rs); content-flow — `Column` | **средний** (каркас модалки: insets, распорки, кламп 320×240 — покрыт тестами settings) | **W3.2** |
| Строки галереи/шаблонов (окно видимости) | `scheme_gallery_ui.rs:266`, `template_ui.rs:808+` | 2 / 0 | `list_rows` + `ScrollState` | средне-низкий | **W3.2** |
| Демо-галерея кита | `kit_ui.rs` | 2 / 0 | **оставить** `Child::fixed(64, …)` — фиксированный ребёнок здесь часть демонстрации flex-grow (F-14); опционально `MeasuredItem::Fixed` | — | W3.3 (опц.) |
| Ручные проводки текста без `Child` | `docs_ui.rs`, `debug_overlay.rs`, `kit_ui.rs:468` | 0 / 0 | `MeasuredItem::Text` / будущий `TextMeasurer::wrap` | средний (перенос строк — не однострочный замер) | W3.3 |
| Таблицы ноды на общих направляющих | `app/stage.rs`, `app/overlays.rs`, `admin_ui.rs`, `kit_ui.rs:690–706` | 0 / 0 | kit-функции остаются (§Контракт-1); `Row`-компонент v1 — только однострочные места; Table v2 — отдельное решение | — | W3.3 (аудит) |

## 4. Гейт W3 (`grep -r "Child::fixed" crates/ | wc -l`) — достижимость

База среза: **184** = потребители **42** + `canvas-ui` **142**, из них:

- движок/адаптер (не мигрируют — это сам API): `layout.rs` 5 вне тестов
  (строки 43, 435–436, 463, 480 — доки и `MeasuredItem::resolve`),
  `taffy_backend.rs` 14 (адаптер);
- оракулы/фикстуры (мигрировать НЕ рекомендуется — они пинят семантику
  `Child` для паритета и G4): тесты `layout.rs` 27, `backend_parity.rs` 63,
  `g4_lint.rs` 13, `html5_demos.rs` 5, `perf_*` 12, `flex_vs_taffy_parity.rs` 3.

Сценарии снижения:

| Сценарий | Снято мест | Глобальный grep | Снижение |
|---|---:|---:|---:|
| W3.1 (measured-перевод: whatif, template-чипы, search) | −15 | 184→169 | −8% |
| W3.1+W3.2 (все скелеты на компоненты; kit_ui demo оставлен) | −40 | 184→144 | −22% |
| W3.1+W3.2+W3.3 (в т.ч. kit_ui demo → `MeasuredItem::Fixed`) | −42 | 184→142 | **−23%** |
| (не рекомендуется) + фикстуры `g4_lint.rs` → `MeasuredItem::Fixed` | −55 | 184→129 | −30% |

**Вывод:** абсолютная цель гейта «снижение на ~30–50%» писалась от плановой
оценки ~497 потребительских мест; фактическая потребительская поверхность —
42 места, поэтому честный потолок без порчи оракулов — **−23% глобального
grep** (−100% потребительской поверхности). Рекомендация к фиксации в
FR-068: гейт W3 по `Child::fixed` считать выполненным при
**«0 `Child::fixed` в `crates/canvas-app/src` и `crates/canvas-render/src`»**
(достижимо W3.1+W3.2, W3.3 — опционально), глобальный счётчик —
вести как наблюдательный (информационный), а не как порог.

**Факт после W3.2 (2026-09-25):** потребители canvas-app/canvas-render —
1 `Child::fixed` (демо kit_ui F-14, рекомендация каталога «оставить»);
перефиксированный гейт достигнут с оговоркой демо. Глобальный grep — 61
(остальное — оракулы/фикстуры движка, мигрировать не рекомендуется §4).

## 5. ТОП-5 мест первичной миграции (минимальный риск)

1. **`crates/canvas-app/src/whatif_ui.rs:222–265`** — сборка `items` и
   `Row{SqueezeTail}.lay_out(items_slot, &items)`: 10× `Child::fixed`
   (из них 7 — ширины из `chip_width`/`btn_width`) →
   `MeasuredItem::Text{max_w: None, min_w: <пад>}` +
   `Row::lay_out_measured(...)`. Политика `SqueezeTail` поддержана
   `lay_out_measured` дословно; оракул побитовой эквивалентности существует
   (`layout.rs::measured_row_matches_manual_fixed_oracle`); поведение пинят
   `integration_whatif`. Максимальный съём при минимальном риске.
2. **`crates/canvas-app/src/template_ui.rs:764–773`** — чипы категорий с
   `RowPolicy::Wrap`: `Child::fixed(category_chip_width(c, m, fs), …)` →
   `MeasuredItem::Text` (замер тот же `TextMeasurer` — rect'ы идентичны;
   `Wrap` поддержан); `category_chip_width` остаётся источником `min_w`.
3. **`crates/canvas-render/src/search_ui.rs:369–374`** — колонка результатов:
   `Child::fixed(inner_w, INPUT_HEIGHT/ROW_HEIGHT)` → `list_rows`/`Column`
   (высоты — дизайн-константы → `MeasuredItem::Fixed`); клип по вьюпорту уже
   гарантирован `viewport_clamp` (W0) и golden `html5_demos/14`.
4. **`crates/canvas-app/src/scheme_gallery_ui.rs:200–216`** — скелет панели
   галереи: 5× `Child::fixed` + 2× `spacer` → `panel_rect`/`panel_content` +
   `Column`-компонент (те же размеры — механический перенос); далее той же
   волной чипы (`:238–249`) и строки (`:262–266`) на `chip_layout`/`list_rows`.
5. **`crates/canvas-app/src/settings_ui.rs:830–858` и `:948–952`** —
   nav-колонка (`Column` из `MODAL_NAV_ITEM_H`) и flow строк тем →
   `list_rows`/`Column`; после пилотов 1–4 (каркас модалки — самый связный
   кусок: insets/распорки/кламп, покрытие тестами settings-модалки).

## 6. Контракты (обязательные ограничения миграции)

- **§Контракт-1 FR-068:** сигнатуры `Row::lay_out`/`Column::lay_out`/
  `grid_cells`/`Child`/`MeasuredItem`/`RowPolicy` — стабильны; миграция
  выполняется ТОЛЬКО на НОВЫЙ `Component` API (W3) и существующий
  `Row::lay_out_measured` (F-13) — старые вызовы не переписываются
  принудительно, каждая волна — отдельный коммит по подсистеме.
- Порядок колонок/размеры — без изменений (ноль визуального скачка, I-1
  FR-046): `MeasuredItem::Text` замеряет тем же `TextMeasurer`/семейством/
  кеглем, что ручная проводка — оракул эквивалентности побитовый.
- G4-линт (§Контракт-3) и golden'ы (snapshot 60, demos 15/15) обязаны
  оставаться зелёными на каждой волне; SqueezeTail/Wrap-деградации —
  именованные политики, не заменять `flex_shrink`'ом (§Контракт-4).
- `Component::layout` для `Row` backend-независим (замер текста един у
  backend'ов, §F-13) — перевод на компонент не требует выбора backend'а.

## 7. Чек-лист волн

- **W3.1** (measured-перевод, ~−15 fixed): whatif_ui; template_ui чипы +
  collapse; search_ui. Гейты: workspace зелёный, integration_whatif,
  golden demos 12/14.
  ✅ выполнено 2026-09-25 частично (пилот whatif_ui; чипы template ушли в
  W3.2, search — в W3.2 по запросу владельца).
- **W3.2** (скелеты на компоненты, ~−25 fixed): settings_ui; scheme_gallery_ui;
  template_ui скелет. Гейты: settings/gallery тесты, G4 × 2 backend'а.
  ✅ выполнено 2026-09-25 (вместе с search_ui — перенесён сюда из W3.1;
  после W4 гейт «G4 × 2 backend'а» выродился: taffy вырезан, оракулы —
  Flex-единственный + Native-пилоты). Реализация: `Row::lay_out_measured`
  (существующий F-13) + НОВЫЙ `Column::lay_out_measured/_with`
  (вертикальный симметричный аналог: resolve `MeasuredItem` → `Child`
  единой точкой `MeasuredItem::resolve` до backend — политика Column
  только Fit, эквивалентно разрешению внутри backend'а; +2 оракула
  бит-в-бит в layout.rs). Чипы template — W3.1-паттерн whatif-бара:
  `MeasuredItem::Fixed{category_chip_width}` (тот же замер — rect'ы
  бит-в-бит; пад-семантики в F-13 нет). Отклонения от предложений
  каталога §3: nav-колонка settings и строки галереи —
  `Column::lay_out_measured`, НЕ `list_rows` (list_rows клипует окно
  видимости и даёт частичные строки на краях — другое поведение в
  вырожденных клампах 320×240; окно видимости уже управляется
  scroll_top/clamp_scroll — раскладка ВИДИМОГО списка целиком сохраняет
  ритм бит-в-бит). Находка (зафиксирована оракулом): `Child::spacer`
  в `Column` занимает 0 по высоте (main-ось колонки — высота, длина
  spacer'а — это w) — «распорки SPACING_S» скелета галереи фактических
  вертикальных зазоров не давали (латентный дефект с FR-049); перенос
  бит-в-бит сохраняет статус-кво, решение по зазорам — за владельцем.
  Гейты: canvas-ui 190, canvas-app 376, canvas-render 376 — 0 failed;
  workspace 2061/0; fmt; clippy -D warnings; wasm_gate --check — зелёные.
- **W3.3** (доводка/аудит): kit_ui demo (опционально), docs_ui/debug_overlay
  (ручные проводки), аудит kit-строк на общих направляющих (Table v2 —
  решение владельца). Гейт-перефиксация §4 в FR-068.

## 8. Ограничения среза

- Счётчики — по строкам с вхождением (`rg -n`); кратные вхождения в строку
  учтены отдельно (`rg -o` = 42 для потребителей — совпадает).
- `stack`/`pad`/`constrain`-вызовы (вне `Child`/`lay_out`) не считались
  миграционной поверхностью: это примитивы U3, остающиеся API.
- Числа фиксируют base `2c67bff`; волны агентов 3-a…3-c (Component-слой
  button/panel/dropdown/modal/text_field/list) счётчики потребителей не
  меняют.
