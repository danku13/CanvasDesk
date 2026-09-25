# FR-068 W3. Table-компонент v2 — дизайн (табличные строки на общих направляющих)

- **Статус:** исполнено (M1–M6; волна 4 субагентов, 2026-09-26).
  Коммиты: M1 — c8640f0/198cfb5/6e9708a; M2–M6 — параллельная волна,
  коммиты лидом (итог см. §8).
- **Дата:** 2026-09-26
- **Задача-источник:** FR-068
  (`docs/change-requests/fr-068-ui-refactoring-long-term.md`), каталог
  миграции `fr-068-w3-consumer-migration.md` §9.3.5/§9.5 — «Table v2 (18
  мест): stage.rs 8, admin_ui 4, kit_ui 6, overlays 2. Последний крупный
  класс вне компонентной модели; строки панели „Как считается“ main stage —
  первый кандидат».
- **Автор:** агент сессии пересборки (W3-продолжение; previous: db94b91 —
  stage-хром на Painter-пути, 5e9e2c6 — каталог миграции §9).
- **Связанные:** FR-061 (`RowGuides`/`row_layout`/`paint_row` — D-3/D-4/D-5,
  D-15), FR-059/FR-060 (волны kit v1), FR-068 W3 (слой `Component`), FR-044
  (прототип панели «Как считается», Р-4/Р-5/Q3), FR-057 (WidgetState),
  ADR-0013/ADR-0014 (layout-backend), PRD-0009 §8 (F-8 — цвета только
  слотами), CR-015 (ellipsis), I-1/G4/G5/G7 (инварианты миграции).

---

## 1. Мотивация и постановка

Кит v1 (FR-061, этап E/D-15) даёт **строку** таблицы: `row_guides` →
`row_layout` → `paint_row`. Направляющие «max по колонке» строятся по
переданному срезу строк, но компонента **таблицы** нет: оркестрация
«замерить все строки → построить направляющие → окно видимости → per-row
состояние/стиль → отрисовать строки → бегунок» дублируется потребителями
вручную. По каталогу §9.3.5 это 18 мест: `stage.rs` (8 — панель «Как
считывается»), `admin_ui` (4), `kit_ui` (6), `overlays` (2). Это последний
крупный класс поверхностей вне компонентной модели `Component` (W3).

Постановка: спроектировать `Table` — retained-компонент v2 поверх
kit-функций Row (v1 остаются стабильным API, §Контракт-1 PRD-0009 V-5),
который владеет:

1. **данными строк** (декларативный вход кадра — `Vec<TableRow>`);
2. **общими направляющими** (проход A max-по-колонке по ВСЕМ строкам
   модели, проход B от правого края — колонка значений стабильна при
   прокрутке; та же двухпроходность, что у тела ноды FR-061 §3.2);
3. **окном видимости** (`ScrollState` + `list_rows` — кит-скролл FR-059);
4. **per-row состоянием и стилем** (`KitState` + переопределения слотов —
   plain data, F-8);
5. **отрисовкой** (строки → бегунок, draw-порядок stage) и
   **hit-тестом** (индекс видимой строки → модельный индекс).

Первый кандидат миграции — панель «Как считается» main stage
(`stage.rs:1274–1422`, две таблицы: «Переменные» и «Расчёт · формулы»).
После db94b91 обе уже на Painter-пути компонентной модели (kit-Row),
поэтому Table v2 ложится поверх **без смены конвертации** (каталог §9.5,
п.2) — миграция сводится к замене ручной оркестрации на компонент при
бит-в-бит геометрии и draw-порядке (I-1).

## 2. Текущее состояние: что именно дублируется

Срез ручной оркестрации по потребителям (base `5e9e2c6`):

| Потребитель | Строки | Что делает вручную | Параметры таблицы |
|---|---|---|---|
| `stage.rs` — «Переменные» | 1274–1367 | `RowParts` из модели → `row_guides` (право = `vars_area.right() − 6`) → цикл по `panel.var_rows`: `WidgetState`+`set_selected` (фокус Р-5), fill `search_row_fill·alpha`, border фокус/`UNMAPPED_EDGE_COLOR`, marker/value/label из слота значения (Ok/Err/Unmapped) × alpha → `row_layout`+`paint_row`; отдельно бегунок | row_h `PANEL_ROW_H`=22, row_gap 0, leader off, кегль 11.0, family `SANS_FAMILY` |
| `stage.rs` — «Расчёт·формулы» | 1368–1422 | `RowParts` → РУЧНАЯ деградация направляющих (`RowGuides{0,0,0, value_x=unit_x=slot.right()}`, stage.rs:1386–1392) → тот же цикл (ƒ-маркер, значения нет) → `row_layout`+`paint_row` | те же; right_pad 0 (направляющие не строятся) |
| `kit_ui.rs` — секция Row | 637–716 | демо 4 строк (Dot/Dot+zebra/Glyph ƒ+бейдж/Σ-Selected) → `row_guides` (право = край контрол-колонки) → цикл `row_layout` (+`RowOpts::default`, leader on) | row_h `GALLERY_ROW_H`, кегль `LABEL_SIZE`, leader on |
| `admin_ui.rs` — «Наполнение» | 1085–1135, 1284–1292 | `FillTableDemo` (9 строк: state Normal/Zebra/Selected, zebra-override `hover_fill`) → `row_guides` → цикл `row_layout`; paint — `paint_row` по предвычисленной геометрии | кегль `LABEL_SIZE`, leader on |
| `overlays.rs` — 2 места | 1240–1252 (+аналог) | `row_style(state)` + zebra-override → `paint_row` по rect'ам своей раскладки | строки уже на своих направляющих |

Общий повторяющийся каркас (то, что уходит в Table v2):

```text
parts: Vec<RowParts>  = модель → строки          (5 мест)
guides                = row_guides(все строки)   (4 места; formulas — ручная деградация)
цикл по видимым:                                 (18 мест-строк)
  state  = WidgetState → KitState                (stage/kit/admin)
  style  = row_style(state) + override полей     (stage — 6 полей, kit/admin — zebra)
  lay    = row_layout(slot, guides, parts, opts)
  paint_row(p, lay, parts, style, size)
бегунок            = scroll_bar + rect           (stage; у остальных скролла нет)
```

Не-дублируемое (остаётся за потребителем): источник данных строк (модель
домена), семантика состояний (фокус Р-5, зебра), цвета-слоты своей темы,
координаты вьюпорта.

## 3. Принципы дизайна

1. **I-1 (бит-в-бит).** Геометрия и draw-порядок каждой мигрируемой
   поверхности воспроизводятся точно. Проверяется оракулами §7
   (прецедент: оракулы `Column::lay_out_measured` W3.2, `cells_with`
   W1).
2. **F-8 (цвета — только слоты).** Кит не содержит цветов и не делает
   арифметики над ними. Per-row стили — `KitState` (слоты `row_style`)
   + переопределения полей `RowStyle` plain data (None = слот базового
   состояния). Вся семантика (Ok/Err/Unmapped, приглушение `1.0 −
   0.5·dim`, янтарный контур unmapped, зебра) вычисляется потребителем
   и приходит готовыми слотами.
3. **KISS / additive.** Один новый файл `component/table.rs`; kit.rs —
   реэкспорт (фасад 1:1). Ноль изменений kit-функций Row v1 и
   `RowGuides` — Table их ПОТРЕБЛЯЕТ (`row_guides`/`row_layout`/
   `paint_row`/`list_rows`/`scroll_bar`).
4. **Преемственность прецедентов.** Retained-владение и immediate-вёрстка
   — как `Row`/`List` (W3): Props+rows публичные, замерщик приватный
   (`RefCell<TextMeasurer>` + `RefCell<FontSystem>`, один инстанс на
   компонент — практика measure.rs §14/Q6), `Component::layout` — только
   rect'ы, бегунок в `paint` — как у `List` (слот `control_border`,
   радиус w/2).
5. **G7 (кит без рендера).** Никаких зависимостей от canvas-render;
   cosmic-text уже в canvas-ui (row.rs) — новых зависимостей нет,
   wasm-гейт не затрагивается.

## 4. API

### 4.1 Данные

```rust
// crates/canvas-ui/src/component/table.rs (новый файл, additive)

/// Строка таблицы (retained-данные; владеет строками — прецедент RowProps).
#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub marker: RowMarker,        // Dot / Glyph(&'static str) / None (Р-4)
    pub label: String,            // левый текст; усечение ellipsis (CR-015)
    pub value: String,            // пустая — ячейки нет (строки-формулы)
    pub unit: String,             // пустая — скаляр
    pub badge: Option<String>,    // None/пустая — бейдж-колонки нет
    /// Состояние строки (Selected/Normal/Hovered/Disabled) — слоты
    /// row_style. Hover/press машина (WidgetState) — у потребителя, если
    /// нужна: строки панели hover не имеют (фокус Р-5 — Selected).
    pub state: KitState,
    /// Переопределения слотов (plain data, F-8): None — слот
    /// row_style(state). Семантика потребителя (ошибка/unmapped/зебра/
    /// приглушение) — заполнением этих полей.
    pub style: TableRowStyle,
}

/// Переопределение слотов стиля строки. Каждое поле — готовый цвет-слот
/// (арифметика — на стороне потребителя до присвоения).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TableRowStyle {
    pub fill: Option<[f32; 4]>,
    pub border: Option<[f32; 4]>,
    pub marker: Option<[f32; 4]>,
    pub label: Option<[f32; 4]>,
    pub value: Option<[f32; 4]>,
    pub unit: Option<[f32; 4]>,
    pub badge: Option<[f32; 4]>,
    pub badge_fill: Option<[f32; 4]>,
    pub badge_border: Option<[f32; 4]>,
}

impl TableRowStyle {
    /// Слияние с базой row_style(state): Some — замена поля, None — слот базы.
    pub fn apply(self, base: RowStyle) -> RowStyle { /* … */ }
}

/// Опции таблицы (плоско; горизонтальный зазор направляющих — тот же
/// параметр, что RowOpts.gap / RowGuides::with_right_edge).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableOpts {
    pub leader: bool,      // дорожка лидера D-5 (stage — off, тело ноды — on)
    pub gap: f32,          // TABLE_GUIDE_GAP (дефолт)
    pub row_h: f32,        // высота строки (PANEL_ROW_H 22 / GALLERY_ROW_H)
    pub row_gap: f32,      // вертикальный зазор строк list_rows (stage — 0)
    pub right_pad: f32,    // отступ правого края направляющих от вьюпорта
}                          // (stage vars — 6; формулы — 0; kit — 0)

pub struct TableProps {
    pub size: f32,               // кегль (stage 11.0; ROW_DEFAULT_SIZE — база)
    pub family: &'static str,    // SANS_FAMILY = "Noto Sans Display"
    pub opts: TableOpts,
    pub palette: KitPalette,     // слоты состояний FR-053 (бегунок, база)
}
```

Параметризация кегля/семейства — осознанное отличие от `Row` v1: там
`ROW_DEFAULT_SIZE`/`ROW_DEFAULT_FAMILY` захардкожены («не-дефолтный кегль —
расширение в v2», row.rs:419–425). Table v2 и есть это расширение: stage
рисует панель кеглем 11.0, kit/admin — `LABEL_SIZE`; замер обязан идти тем
же кеглем, что шейпинг (T9 FR-061).

### 4.2 Направляющие: implicit-колонки и правило деградации

Колонки таблицы — те же 4 зоны kit-Row: лидер (elastic) → значение → юнит
→ бейдж (порядок справа налево D-3/§3.1). Наличие правой колонки
**выводится из данных** (как в `row_guides` сегодня): ширина колонки — max
по ВСЕМ строкам модели; колонка, пустая у всех строк, схлопывается.
Явные флаги `columns` не вводятся — обоснование в §6.

```rust
impl Table {
    /// Проход A+B: направляющие по ВСЕМ строкам модели (не окна
    /// видимости!) — колонка значений стабильна при прокрутке (§3.1
    /// FR-061). right_edge = viewport.right() − opts.right_pad.
    /// Делегирование: row_guides(m, fs, family, size, parts, right, gap).
    pub fn guides(&self, viewport_w: f32) -> Option<RowGuides>;
}
```

**Правило деградации** (обязательное — иначе бит-в-бит не сходится):
если правые колонки пусты у всех строк (`value_w+unit_w+badge_w == 0`)
или строк нет — `guides()` возвращает `None`, а `row_layout_at` строит
деградированные направляющие от правого края строки:
`RowGuides { value_w:0, unit_w:0, badge_w:0, value_x: slot.right(),
unit_x: slot.right() }` — точный паритет ручной деградации stage.rs
(1386–1392). Причина: `RowGuides::with_right_edge` вычитает структурный
`gap` безусловно, и «наивные» нулевые направляющие сдвинули бы
`label_right` (левый текст) на `gap` влево относительно текущего рендера
формул (label_right был бы `right − gap − ROW_TEXT_GAP` вместо
`slot.right() − ROW_TEXT_GAP`). Замечание: у строк без значения и юнита
`row_layout` вычисляет `label_right = slot.right() − ROW_TEXT_GAP` вообще
без направляющих — правило деградации даёт тот же результат и для
смешанных случаев фиксирует единый контракт.

Для «Переменных» stage бит-в-бит тривиален: `value` непустой у всех строк
(Ok/Err/Unmapped-текст), юнит/бейдж пустые → `unit_w=badge_w=0`. Точная
арифметика kit (`RowGuides::with_right_edge`, row_guides.rs:89–93):
структурный зазор `gap` вычитается на КАЖДОЙ границе цепочки
value|unit|badge (их две) — при пустых unit/badge ширинах
`value_right = right_edge − 2·gap` (два зазора: value|unit и unit|badge),
а не «− gap» (уточнено при M1; прежняя формулировка — неточность
описания). Мастер-оракул — бит-в-бит с `row_guides` (T1: обе стороны
вызывают одну kit-функцию) — паритет текущей геометрии
(`row_guides` stage.rs:1304–1312, `right_edge = vars_area.right() − 6`).

### 4.3 Окно видимости и скролл

```rust
impl Table {
    /// Видимые строки: (МОДЕЛЬНЫЙ индекс, усечённый rect) —
    /// list_rows(viewport, scroll, row_h, row_gap, n) + пересечение
    /// с viewport (клип-семантика stage: частичная строка усекается,
    /// текст центрируется в усечённом слоте — calc_panel_ui::visible_rows).
    pub fn visible_rows(&self, viewport: UiRect) -> Vec<(usize, UiRect)>;
}
```

Отличие от `List::layout` (возвращает полные rect'ы частичных строк) —
осознанное и задокументированное: Table принимает семантику stage
(усечение пересечением, центрирование в усечённом слоте) как бит-в-бит
требование I-1; `List` не меняется (его потребители рисуют по полным
rect'ам). `Painter::clip_rect` для строк не нужен — усечённый rect сам
есть клип по вертикали; горизонтального переполнения нет (ellipsis).

Синхронизация скролла — устранение класса багов на источнике: в отличие
от `List` (восстанавливает `count` из `content_h`), Table ЗНАЕТ строки:

```rust
impl Table {
    /// Замена строк + content_h = n·row_h + (n−1)·row_gap (инвариант
    /// sync_scroll держит сам компонент). offset/clamp — потребитель
    /// (scroll_by по колесу, как сейчас: ctx.vars_scroll).
    pub fn set_rows(&mut self, rows: Vec<TableRow>);
}
```

`viewport_h` потребитель выставляет из раскладки панели каждый кадр
(площади знает только он) + `scroll.clamp()` — идемпотентно, как сейчас
(calc_panel_ui::sync_scroll; сам `sync_scroll` после миграции stage
сокращается до двух присвоений).

### 4.4 Отрисовка

```rust
impl Component for Table {
    type Props = TableProps;
    /// Слоты видимых строк (усечённые rect'ы, порядок модельных индексов).
    fn layout(&self, _backend: &dyn LayoutBackend, viewport: UiRect)
        -> Vec<UiRect>;                                   // visible_rows(...).rects
    /// Строки → бегунок (draw-порядок stage 1313–1436):
    /// для каждой видимой строки —
    ///   base = row_style(row.state, props.palette);
    ///   style = row.style.apply(base);         // Some — замена слота
    ///   lay = row_layout_at(slot, model_index);
    ///   paint_row(painter, lay, parts_of(row), style, props.size);
    /// затем бегунок scroll_bar(viewport, scroll, palette) —
    /// rect слотом control_border, радиус w/2 (паритет List::paint и
    /// stage:1429–1436; None — 0 items).
    fn paint(&self, painter: &mut Painter, rects: &[UiRect]);
    // hit_test — дефолтный ComponentHit::pick (§4.5)
}
```

`paint` пересчитывает `visible_rows` из `self.scroll` (чистая функция,
детерминизм — инвариант 2 FR-050): контракт `rects == layout(viewport)`
в том же порядке; модельный индекс i-го rect'а — `visible_rows[i].0`.
`row_layout_at` переиспользует замеры через кэш `TextMeasurer` ( RefCell —
`&self`, как у `Row`).

Гранулярные методы (реализованы в M1): `paint_rows_with(p, m, fs,
viewport)` — ТОЛЬКО строки; `paint_scrollbar(p, viewport)` — ТОЛЬКО
бегунок; `paint_with` — их композиция (строки → бегунок своей таблицы).
Нужны потребителям с составным draw-порядком кадра: панель stage рисует
СТРОКИ ОБОИХ таблиц, затем ОБА бегунка (rows vars → rows formulas →
sb vars → sb formulas) — составной `paint_with` каждой таблицы по
отдельности перемешал бы порядок items журнала (I-1).

### 4.5 Hit-test

- Компонентный уровень: дефолт `ComponentHit::pick(rects, p)` — индекс
  усечённого rect'а видимой строки (зазоры row_gap>0 — None, как у List).
- Потребительский (модельный): `Table::model_index_at(viewport, p) ->
  Option<usize>` — через `visible_rows` (замена `CalcPanelLayout::
  var_row_at/formula_row_at` при миграции input.rs — геометрия идентична,
  оракул T2).

### 4.6 Владение и производительность

- `Table` создаётся ОДИН раз на поверхность (retained): `FontSystem::new()`
  дорогой — на кадр не создаётся (прецедент Row; stage: две таблицы → два
  инстанса `Table`, по одному `FontSystem` каждый; прикладной объём —
  десятки строк).
- Строки (`String`) пересобираются только при смене модели
  (`set_rows`); per-frame анимации (приглушение Q3 `1.0 − 0.5·dim`,
  рамка фокуса) — мутацией по месту `rows[i].style.*` (без пересборки
  строк). Для stage это сохранит текущую стоимость: `RowParts` сегодня
  заимствуют из модели (zero-copy), `TableRow` владеет — компенсируется
  тем, что при статичной модели копий нет вовсе.
- Кэш ширин `TextMeasurer` переживает кадры (общий на компонент) —
  повторный замер неизменных текстов дешёвый (практика row.rs).

### 4.7 Два слоя замера (продакшн-паритет)

Производственный замер stage идёт через ОБЩИЙ `FontSystem` рендера
(`canvas_render::text::measure_font_system()` — `MutexGuard<'static,
FontSystem>`, статика с вшитыми шрифтами) — свежий `FontSystem::new()`
может резолвить семейство иначе (системные шрифты) и бит-в-бит замера не
даст. Поэтому Table — два слоя, по прецеденту двойственности
«kit-функции (внешний замер) + Component (собственный)»:

1. **Основной (потребительский) слой** — методы `*_with` с внешним
   замерщиком (сигнатуры — как у kit-функций Row):
   `guides_with(&mut m, &mut fs, viewport_right)`, `visible_rows(viewport)`
   (замера не требует), `row_layout_with(&mut m, &mut fs, viewport_right,
   slot, i)`, `paint_rows_with(&mut p, &mut m, &mut fs, viewport)` /
   `paint_scrollbar(&mut p, viewport)` (гранулярные — составной
   draw-порядок stage, §4.4) и `paint_with(&mut p, &mut m, &mut fs,
   viewport)`. ЭТИ методы используют продакшн-потребители (stage M2) —
   бит-в-бит замера с текущим кодом. Семантика параметра направляющих —
   `viewport_right`: ПРАВЫЙ КРАЙ вьюпорта В СИСТЕМЕ КООРДИНАТ СЛОТОВ (те
   же координаты, что у rect'ов `visible_rows`; при вьюпорте с x=0
   совпадает с шириной) — НЕ ширина: stage-слоты экранные (x≠0),
   «ширина» дала бы сдвиг направляющих на x вьюпорта (уточнено при M1;
   T1 покрывает вьюпорт x≠0).
2. **Компонентный слой** — `impl Component` (собственные
   `RefCell<TextMeasurer>`/`RefCell<FontSystem>`, прецедент `Row`) —
   для компонентной модели W3 и потребителей без общего замерщика;
   делегирует внутренним общим функциям.

Оракулы T1/T3 проверяют ОБА слоя и их эквивалентность: `*_with` ≡
kit-функции (побитово), Component-слой ≡ `*_with` (те же входы,
детерминированный шрифт тестов).

## 5. Эскиз скелета (не исчерпывающий)

```rust
pub struct Table {
    pub props: TableProps,
    pub rows: Vec<TableRow>,
    pub scroll: ScrollState,               // публичное — как у List
    measurer: RefCell<TextMeasurer>,
    font_system: RefCell<cosmic_text::FontSystem>,
}

impl Table {
    pub fn new(props: TableProps) -> Self { /* measurer/fs — один раз */ }
    pub fn set_rows(&mut self, rows: Vec<TableRow>) { /* + content_h */ }
    fn parts_of(&self, i: usize) -> RowParts<'_> { /* заимствование полей */ }
    fn merged_style(&self, i: usize) -> RowStyle {
        self.rows[i].style.apply(row_style(self.rows[i].state, &self.props.palette))
    }
    pub fn guides(&self, viewport_w: f32) -> Option<RowGuides> { /* §4.2 */ }
    pub fn visible_rows(&self, viewport: UiRect) -> Vec<(usize, UiRect)> { /* §4.3 */ }
    pub fn row_layout_at(&self, slot: UiRect, i: usize) -> Option<RowLayout> { /* §4.2 */ }
    pub fn model_index_at(&self, viewport: UiRect, p: UiPoint) -> Option<usize> { /* §4.5 */ }
}

impl Component for Table { /* §4.4 */ }
```

Реэкспорт в `kit.rs` (фасад 1:1, §Контракт-1):
`pub use crate::component::table::{Table, TableOpts, TableRow, TableRowStyle, TableProps};`

## 6. Отклонение от эскиза каталога: `columns` — implicit

Каталог §9.3.5 набросал `TableProps { columns, rows }`. В дизайне
`columns` НЕявные (§4.2). Обоснование:

1. **Бит-в-бит.** «Переменные» stage: юнит/бейдж пустые у всех строк, но
   структурные зазоры живой колонки значения должны сохраниться. Точная
   арифметика kit (`with_right_edge` вычитает `gap` на каждой границе
   цепочки value|unit|badge — §4.2): `value_right = right_edge − 2·gap`
   (два зазора; уточнено при M1 — прежняя формулировка «right − gap»
   была неточностью описания). Явный `columns{unit: off, badge: off}`
   при исключении колонки из цепочки зазоров СДВИНУЛ бы числа
   вправо; при сохранении зазоров — дублирует то, что выводится из
   данных. Implicit-вывод `row_guides` воспроизводит оба случая точно.
2. **Меньше состояния.** Флаги колонок — второй источник истины рядом с
   данными строк (рассинхрон при `set_rows`).
3. **Обратная совместимость.** Все 18 мест сегодня описываются данными
   `RowParts` — явные флаги никем не востребованы (YAGNI); при появлении
   потребности (например, колонка-подсказка) `TableOpts` расширяется
   additively.

## 7. Оракулы и тесты

Детерминированный шрифт тестов — `component/test_support` (тот же файл
`NotoSansDisplay-Medium.ttf`, что FONT_DATA рендера; практика row_guides/
row.rs тестов). Допуски — как в `cells_with`-тестах W1: целые входы —
побитово, дробные — 1e-3.

- **T1 (направляющие ≡ kit).** `table.guides(w)` ≡ `row_guides(те же
  строки, right, gap)` — на данных kit-демо (4 строки с бейджем) и
  stage-подобных (только label+value).
- **T2 (окно ≡ stage-семантика).** `visible_rows` ≡ `list_rows`+
  пересечение: offset 0 / mid-scroll / частичные строки сверху и снизу
  (усечённые rect'ы, модельные индексы); чистота (без мутаций scroll).
- **T3 (paint ≡ ручной цикл).** `Table::paint` ≡ последовательность
  `paint_row` с `merged_style` по видимым строкам + бегунок последним —
  равенство `Vec<PaintItem>` (данные kit-демо: зебра-override, Selected;
  и stage-подобные: border-override).
- **T4 (деградация ≡ формулы stage).** Все правые ячейки пусты →
  `guides()` = None; `row_layout_at` даёт `label_right =
  slot.right() − ROW_TEXT_GAP` и пустые value/unit rect'ы — паритет
  stage.rs:1386–1402.
- **T5 (детерминизм/порядок).** Перестановка строк не меняет направляющие
  (max коммутативен); повторный `guides()` идентичен (инвариант 2
  FR-050).
- **T6 (content_h).** `set_rows` выставляет `content_h = n·row_h +
  (n−1)·row_gap`; `clamp` идемпотентен; `model_index_at` вне вьюпорта —
  None.
- **T7 (stage, после M2).** Существующие тесты панели (app.rs:8870+,
  calc_panel_ui) зелёные без правок ожиданий; журнал Painter панели до/
  после миграции — равенство (референс старого пути сохраняется в тесте,
  как в оракулах W3.2).

Гейты этапов: `cargo test --workspace` (0 failed; база 2063), `cargo
clippy --all-targets -- -D warnings`, `cargo fmt` (мои файлы), `cargo
wasm --check`/wasm_gate (canvas-ui в wasm-сборке).

## 8. План миграции (18 мест)

| Этап | Объём | Что происходит | Риск |
|---|---|---|---|
| **M1 ✅** | canvas-ui | `component/table.rs` + реэкспорт kit.rs + T1–T6. Additive, потребители не тронуты | низкий |
| **M2 ✅ — первый кандидат** | stage.rs (8) | 2×Table (vars/formulas) в stage-состоянии (retained); `set_rows` при смене модели; per-frame — мутация `rows[i].style/state` (dim/фокус/unmapped); vars: `right_pad=6`, формулы: `right_pad=0` (деградация §4.2); бегунки — paint Table (убрать ручной блок 1423–1436); calc_panel_ui: площади/скролл остаются, `visible_rows`/`sync_scroll` сокращаются (T2/T7) | средний |
| **M3 ✅** | kit_ui (6) | секция Row: ТЕ ЖЕ данные → Table-демо рядом с Row-демо (витрина примитива сохраняется; решение о полной замене — владелец); зебра/Selected — TableRow.state/style | низкий |
| **M4 ✅** | admin_ui (4) | `FillTableDemo` → Table (геометрия демо предвычислена fill_body — переходит на `Table::visible_rows`) | низкий |
| **M5 ✅ (аудит)** | overlays (2) | `row_style`+zebra циклы → Table — АУДИТ: единственное kit-row место overlays (секция Row витрины, paint-половина) — НЕ кандидат (см. статус под таблицей) | низкий |
| **M6 ✅ (этот коммит)** | docs | каталог §9.3.5/§9.5 → «исполнено»; ACCEPTANCE FR-068 — запись; surface-registry — компонент Table | — |

Статус (2026-09-26, волна 4 субагентов): **M1–M4 исполнены** (M1 —
коммиты c8640f0/198cfb5/6e9708a: `component/table.rs`, реэкспорт kit.rs,
T1–T6, гранулярные `paint_rows_with`/`paint_scrollbar` и семантика
`viewport_right`; M2–M4 — параллельная волна, коммиты лидом).
**M5 — исполнено аудитом (честный результат):** в `overlays.rs` ровно
одно kit-row место (2 вызова — `row_style`/`paint_row`)
— paint-половина демо-таблицы витрины (секция Row): строки УЖЕ на общих
направляющих, но геометрия/скролл (row_guides/row_layout, сдвиг на
offset, фильтр ПОЛНОЙ видимости) принадлежат `kit_ui::gallery_layout`
(M3), paint-половина уже полностью на kit-функциях по предвычисленным
раскладкам; семантика видимости витрины (полная видимость + сдвиг
ячеек) отличается от клипа Table (`visible_rows` — пересечение,
усечение частичных строк) — замена меняла бы рендер краёв (I-1) без
выигрыша и требовала дублирования геометрии витрины в overlays
(решение о полной замене демо — за владельцем, вместе с M3).
**M6 — этим коммитом** (шапка/§4.2/§4.4/§4.7/§8 этого файла; каталог
§9.3.5/§9.5; ui-kit §10).

Порядок M2 → M3–M5: самый нагруженный потребитель первым (каталог §9.3.5
«stage/admin — самые нагруженные таблицы»), пока оракулы свежие; M3–M5
независимы, дробятся по времени.

## 9. Риски, границы, non-goals

- **Частичные строки.** Семантика усечения (пересечение, не clip_rect) —
  зафиксирована T2; иначе центр текста частичной строки сместился бы на
  несколько px (I-1).
- **Dim-анимация.** Overrides меняются каждый кадр — мутация по месту;
  пересборка строк только по смене модели (§4.6). Альтернативный вариант
  «`dim: f32` в Props кита» отвергнут — арифметика над цветами в ките
  (F-8).
- **Магическая `6.0`** (right_pad vars stage) — параметр потребителя,
  кандидат в токен (вне среза, как и прочие перефиксации констант).
- **Non-goals:** sticky-заголовки (кандидат FR-074 §9.3.3), сортировка/
  ресайз/горизонтальный скролл колонок, многострочные ячейки, зебра
  внутри кита (override потребителя), retained-диффинг Props (решение
  W3 «не вводится»), миграция тела ноды (канвас-домен, каталог §9.4).
- **Row v1 не меняется:** `Row` остаётся витриной однострочного примитива
  (направляющие от края слота); Table — композиция на общих направляющих.

## 10. Гейты приёмки дизайна

1. Все 18 мест каталога покрываются API без расширений (проверено
   попараметрово в §2/§8) — новых полей, кроме перечисленных, не нужно.
2. Каждый оракул §7 формулируется ДО реализации соответствующего этапа.
3. Ни один этап не меняет kit-функции и `RowGuides` (git-дифф M1–M5
   касается только новых файлов и потребителей).
4. Финал: workspace/clippy/fmt/wasm зелёные; каталог §9.5 п.2 закрыт.
