# FR-061: Табличное тело ноды (Н-3, пре-PRD PRD-0004) — этап A: части значения (D-1) + замер колоночных направляющих (D-3)

- **Статус:** в работе (этап A реализуется; этапы B–E — постановка)
- **Тип:** FR
- **Приоритет:** высоко
- **Владелец:** агент сессии 2026-09-23 (этап A); этапы B–E — любой агент против замороженных контрактов
- **Источник:** `docs/interface-objects/node-tabular-body-analysis.md` (§3 спецификация с прототипа 1:1, §4 D-1…D-15, §6 этапы A–E, §5 инварианты, §9 вопросы Q1–Q9); приказ владельца «оформи CR-061 и начни этап A… важно, чтобы максимально использовался canvas-ui и вся логика и утилитарные функции правильно структурировались архитектурно, чтобы ui стал максимально декларативным» (сессия 2026-09-23)
- **Связанные задачи:** PRD-0004 (анатомия ноды — Н-3 тело); FR-013 (бейджи результата), FR-017 (what-if), FR-025 (построчные порты), FR-044 (stage-якоря/панель), FR-050 (авто-строки, инвариант 2), FR-053 (TextMeasurer F-6), FR-057/FR-058 (Painter/WidgetState/компоненты v2 — фундамент этапа E); Q9 решён владельцем 2026-09-23 (направляющие невидимы — только DebugOverlay)
- **Создан:** 2026-09-23
- **Обновлён:** 2026-09-23

## Описание
Тело ноды переводится на табличную вёрстку по спецификации прототипа (`node-tabular-body-analysis.md` §3): строка данных = ячейки имя/= /формула/лидер/значение/юнит/бейдж, числа и юниты стоят на колоночных направляющих (правое выравнивание по max-ширине по ноде), деградация ширины — лестницей §3.4, модель данных и `.canvas` не меняются. Программа из 15 доработок D-1…D-15 разбита на этапы A–E (§6). **Этап A (этот FR)** — ядро без рендера: D-1 (структурные части значения `ValueParts` — домен, canvas-core) и D-3 (замер колоночных направляющих — чистая layout-утилита, canvas-ui), мержится независимо. Этапы B (RowGrid + мультибуферный рендер), C (Н-3 режимы), D (полировка), E (kit-Row) — против замороженных ниже контрактов.

Архитектурное правило владельца (приказ 2026-09-23): максимальное использование canvas-ui; логика/утилиты — по слоям (canvas-core — домен, canvas-ui — чистые layout/измерения, canvas-render — исполнение), UI — декларативный (потребители дают данные, каркас считает геометрию).

## Влияние
| Объект | Что меняется | Где в документации |
|---|---|---|
| `expr` (canvas-core) | `Value::display_parts`, `join_parts`, `DeltaParts`/`whatif_delta_parts`; `Display` и `whatif_full_delta` — композиции над частями (байт-паритет) | анализ §4 D-1 |
| `flow` (canvas-core) | `AutoRowParts` + `AutoRow::display_parts`; `display_text` — композиция над частями (инвариант 2 FR-050) | анализ §4 D-1 |
| `row_guides` (canvas-ui, новый модуль) | `RowCellWidths`, `RowGuides::measure/with_right_edge`, `measure_row_cells` (TextMeasurer) — проход A двухпроходной раскладки | анализ §3.2, §4 D-3 |
| Потребители (этапы B–E) | `row_grid.rs`/`text.rs` — сборка ячеек и рендер по частям и направляющим; MCP-тексты — старый `Display` без изменений | анализ §4 D-2/D-4 |

## Анализ
- `canvas-core/src/expr.rs:394` — `Display for Value` уже форматирован как `{num} {unit}` (`format_num` + `unit.display()`): части выделяются без дублирования форматирования, `Display` становится композицией `join_parts(display_parts())` — единая точка сборки (байт-паритет тестом).
- `canvas-core/src/expr.rs:1821` — `whatif_full_delta` = `«{base} → {whatif} ({delta})»`: строка дельты (`whatif_delta_str`, единственная точка спец-логики «пп»/знак) не трогается; добавляется структурный вариант `whatif_delta_parts { base, new, delta }` для бейдж-колонки этапа B.
- `canvas-core/src/flow.rs:1093` — `AutoRow` хранит `path` + `Option<Value>`: `display_parts` выводится тривиально; unmapped → `num = "—"`, `unit = ""` (Р-3 диагностика сохраняется); `display_text` — композиция над частями, байт-паритет тестом (инвариант 2 FR-050: единая точка сборки).
- `canvas-ui/src/measure.rs` — `TextMeasurer::width_of` (F-6, кэш по (текст, семейство, кегль)) готов принимать замеры ячеек; направляющие — чистая функция max по строкам + арифметика правого края, без canvas-core-зависимости (G7: canvas-ui не получает новых зависимостей).
- `canvas-render/src/guides.rs` (Ф-14) — слово «guides» занято snap-осями: новый тип именуется `RowGuides` (дисциплина §5), модуль — `row_guides.rs` (canvas-ui) / `row_grid.rs` (canvas-render, этап B).
- Риски этапа A: низкие — ядро без рендера, обратная совместимость через байт-паритет `Display`/`whatif_full_delta`/`display_text` (MCP-тексты не меняются).

## Требуемые изменения
| Что | Где | Как |
|---|---|---|
| Части значения | `canvas-core/src/expr.rs` | `Value::display_parts(&self) -> (String, String)`; `pub fn join_parts(num, unit) -> String` — единственная сборка «num unit»/«num»; `Display` и `whatif_full_delta` — композиции; `DeltaParts { base, new, delta }` + `whatif_delta_parts` |
| Части авто-строки | `canvas-core/src/flow.rs` | `AutoRowParts { path, num, unit }` + `AutoRow::display_parts`; `display_text` — композиция над частями |
| Направляющие | `canvas-ui/src/row_guides.rs` (новый) + `lib.rs` export | `RowCellWidths { value_w, unit_w, badge_w }`; `RowGuides { value_w, unit_w, badge_w, value_x, unit_x }` + `measure(rows) -> Option<RowGuides>` (max по строкам) + `with_right_edge(right_edge, gap)` (x-позиции: бейдж → юнит → значение справа налево) + `measure_row_cells(measurer, fs, value, unit, badge_w, family, size)` (естественные ширины ячейки через `width_of`) |
| Тесты | оба крейта | T1-oracle частей; байт-паритет Display/whatif/AutoRow (корпус: скаляр, юнит, составной `ms·req/s`, unmapped, «пп»); T3-oracle направляющих (max по ноде, детерминизм, право-край арифметика, замер через реальный шрифт) |

## Замороженные контракты (этапы B–E и параллельные агенты кодируют против них)
```rust
// canvas-core::expr — D-1
impl Value {
    /// («800», «rps»); скаляр → (число, «»); число — как в Display (группировка/округление).
    pub fn display_parts(&self) -> (String, String);
}
/// Единственная сборка отображения значения: «num unit» | «num».
pub fn join_parts(num: &str, unit: &str) -> String;
pub struct DeltaParts { pub base: (String, String), pub new: (String, String), pub delta: String }
/// None ⇔ чтоif-дельты нет (совпадает с whatif_delta_str).
pub fn whatif_delta_parts(base: &Value, whatif: &Value) -> Option<DeltaParts>;

// canvas-core::flow — D-1
pub struct AutoRowParts { pub path: String, pub num: String, pub unit: String } // unmapped: num = "—"
impl AutoRow { pub fn display_parts(&self) -> AutoRowParts; }

// canvas-ui::row_guides — D-3 (Ф-14: имя RowGuides, не Guides)
pub struct RowCellWidths { pub value_w: f32, pub unit_w: f32, pub badge_w: f32 }
pub struct RowGuides {
    pub value_w: f32, pub unit_w: f32, pub badge_w: f32, // max по строкам (проход A)
    pub value_x: f32, pub unit_x: f32,                   // левые края ячеек (проход B ноды)
}
impl RowGuides {
    pub fn measure(rows: &[RowCellWidths]) -> Option<Self>;            // None — строк нет
    pub fn with_right_edge(self, right_edge: f32, gap: f32) -> Self;   // x от правого края тела
}
/// Естественные ширины ячеек строки: value/unit — width_of (право-выравнивание
/// считает потребитель), badge_w — passes через (пилюля/иконка).
pub fn measure_row_cells(
    measurer: &mut TextMeasurer, fs: &mut cosmic_text::FontSystem,
    value: &str, unit: &str, badge_w: f32, family: &str, size: f32,
) -> RowCellWidths;
```
Инварианты: (1) `format!("{v}") == join_parts_parts(v.display_parts())` на любом `Value` (тест-свойство); (2) текст ноды — единственный источник истины, модель/`.canvas` не меняются; (3) canvas-ui — без новых зависимостей (G7); (4) имена — Ф-14 (`RowGuides`, не `Guides`).

## Права на файлы (для параллельных агентов)
- **Этап A (эта ветка):** `crates/canvas-core/src/expr.rs`, `crates/canvas-core/src/flow.rs`, `crates/canvas-ui/src/row_guides.rs` (новый), `crates/canvas-ui/src/lib.rs` (только export), тесты обоих крейтов.
- **Этап B (следующий):** `crates/canvas-render/src/row_grid.rs` (новый), `crates/canvas-render/src/text.rs` (`with_body_stack`, сборка ячеек), `cards.rs` (пилюли-бейджи).
- **Запрещено трогать в рамках FR-061 вообще:** онбординг (`onboarding_ui.rs`, заморожен владельцем); зоны FR-059/FR-060 (`hints_ui.rs`, `flowmap_ui.rs`, `calc_panel_ui.rs`, `autolink_ui.rs`, `palette.rs`, `explain_ui.rs`, `kit_ui.rs`, хвосты app.rs dialog/menu/hotkeys); MCP-текстовые мосты (`mcp_text.rs` — остаётся на `Display`).
- **app.rs** — только этап C/D (D-13 правка, S) по отдельному согласованию.

## Зависимости
| Отношение | С кем |
|---|---|
| Кодировать | сразу — этап A зависит только от U3 (TextMeasurer, в main); волна 2 (FR-056/057/058) в main — фон |
| Сливать | независимо от FR-059/FR-060 (зоны файлов не пересекаются; canvas-core/canvas-ui против их canvas-app) |
| После этапа A | этап B (D-2/D-4/D-5/D-6/D-11 — `row_grid.rs` + мультибуферный рендер), затем C (Q1–Q4), D, E (kit-Row на `RowGuides` + Painter/WidgetState) |

## Точки входа
- `docs/interface-objects/node-tabular-body-analysis.md` — §4 (D-1/D-3 статусы), §6 (этапы), §9 (вопросы).
- `docs/change-requests/index-cr-fr.md` — статус FR-061.
- `docs/prd/README.md` — строка prd-0004 (упоминание дизайна табличного тела + FR-061).
- `worklog.md` — двойная запись этапа A.

## Проверка
- T1-oracle: «800 rps» → («800», «rps»); скаляр → юнит пуст; составной `ms·req/s`; группировка как Display; unmapped авто-строки → num «—».
- Байт-паритет: `Display`, `whatif_full_delta` (вкл. «+0 пп»-ветку), `AutoRow::display_text` — точные строки корпуса до/после.
- T3-oracle: `RowGuides::measure` = max по строкам (в т.ч. пустой список → None); `with_right_edge` — арифметика правого края (badge → unit → value, зазор gap); детерминизм (одинаковые входы → идентичные направляющие); `measure_row_cells` — реальный шрифт (NotoSansDisplay), ширina растёт с длиной, mono-базис.
- Гейты: `cargo fmt --all --check`; `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings`; `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace`; `bash scripts/wasm_gate.sh`; `bash scripts/mcp_wasm_gate.sh` — зелёные.
- Пользовательских изменений нет (ядро без рендера) — онбординг не трогается.

## История изменений
- `2026-09-23` — агент: создан документ (приказ владельца «оформи CR-061 и начни этап A»); контракты D-1/D-3 заморожены, этапы B–E поставлены, права на файлы разведены; этап A реализуется в ветке `feature/fr-061-node-tabular-stage-a`.

## Источники истины
- `docs/interface-objects/node-tabular-body-analysis.md` — §3 (спецификация), §4 (D-1…D-15), §5 (инварианты/Ф-14), §6 (этапы), §7 (T1/T3), §9 (вопросы).
- `crates/canvas-core/src/expr.rs` — `Display for Value` (394), `format_num` (364), `whatif_delta_str`/`whatif_full_delta` (1800–1825).
- `crates/canvas-core/src/flow.rs` — `AutoRow` (1093), `display_text` (1117).
- `crates/canvas-ui/src/measure.rs` — `TextMeasurer` (70), `width_of` (132).
- `crates/canvas-render/src/guides.rs` — Ф-14 коллизия имён.
