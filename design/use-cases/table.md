# Таблица (Table)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/table.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Retained-оркестрация строк Row: панель «Как считается» (колонки
переменные/формулы), таблицы админ-консоли, витрина. Выбор — K1.2
(`../rules/10-components.md`): табличные гайды — Row/Table. Оркестрация по
таблице K1: общий замер всех строк → общие направляющие → окно видимости →
per-row стиль → строки → бегунок; одна строка — Row (`../use-cases/row.md`),
однородный список — List (`../use-cases/list.md`); Row v1 не меняется —
Table потребляет kit-функции (additive, FR-068).

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Высота строки | opts.row_h — LIST_ROW_H 26 (дефолт; stage — своя) |
| Вертикальный зазор | opts.row_gap — 0 (дефолт) |
| Зазор направляющих | opts.gap — TABLE_GUIDE_GAP 6 |
| Отступ направляющих от правого края | opts.right_pad — 0 («Переменные» — 6) |
| Лидер | opts.leader — true (stage — off) |
| Зебра | TABLE_ZEBRA_RUN_MIN 4 |
| Кегль/семейство | props.size/family (дефолт ROW_DEFAULT_SIZE 12; stage — 11) |
| Компонент | `Table { props, rows, scroll }` (retained) |

`content_h` ведёт сам компонент: `set_rows` пересчитывает
`n·row_h + (n−1)·row_gap`; offset/clamp — потребитель (viewport_h — каждый кадр).

## 3. Токены (слоты KitPalette)
База строки — `row_style(row.state)` (слоты — `../use-cases/row.md`);
поверх — `TableRowStyle` (Option-поля fill/border/marker/label/value/unit/
badge/badge_fill/badge_border): `Some` — замена, `None` — слот базы
(`TableRowStyle::apply`). Семантика потребителя (ошибка/unmapped/зебра/
приглушение) — готовыми слотами (F-8). Бегунок — control_border (радиус w/2);
интерактив — `../use-cases/scrollbar.md`. Зебра: фон прогона из
≥ TABLE_ZEBRA_RUN_MIN 4 строк — в `TableRowStyle::fill` (помощник
`zebra_run_flags`; tint — K2.2).

## 4. Состояния
Per-row KitState (`TableRow.state`) → слоты `row_style`; матрица ST1 —
`../rules/08-states.md`. Hover/press-машина — у потребителя (строки панели
hover не имеют — фокус Р-5 на Selected). Сам Table — контейнер, без состояний.

## 5. Взаимодействие
- **Окно видимости**: `visible_rows(viewport)` — `list_rows` + пересечение
  каждого rect с вьюпортом (частичная строка усекается, текст центрируется в
  усечённом слоте) — отличие от `List::layout` (полные rect'ы), I-1.
- **Hit**: `model_index_at(viewport, point)` — первый усечённый слот,
  содержащий точку; вне вьюпорта/в зазоре row_gap — None.
- **Направляющие**: по ВСЕМ строкам модели (не окна) от правого края
  вьюпорта (`viewport_right − opts.right_pad`, в координатах слотов) —
  колонка значений стабильна при прокрутке (FR-061 §3.1).
- **Draw-порядок**: строки → бегунок; гранулярно —
  `paint_rows_with`/`paint_scrollbar` (панель «Как считается»: строки обеих
  колонок → оба бегунка, порядок не перемешивать — I-1).

## 6. Граничные случаи
- Строк нет ИЛИ правые колонки пусты у всех строк → деградация направляющих:
  вход от правого края строки без структурного зазора (паритет
  `crates/canvas-app/src/app/stage.rs`, 1386–1392).
- `index` вне строк модели — `row_layout_with` → None.
- Частичные строки усечены пересечением; верхняя может получить
  отрицательный y (клип — поверхность).
- `table_layout_immediate`: пустые входы → пустой Vec; шапка высоты 0 не
  рисуется; сумма ширин колонок > slot.w — выход за край (G4).

## 7. Где в коде
`crates/canvas-ui/src/component/table.rs` — `TableRow`/`TableRowStyle`/
`TableOpts`/`TableProps`/`table_layout_immediate`/`Table` (`set_rows`,
`guides_with`, `visible_rows`, `row_layout_with`, `paint_with`,
`paint_rows_with`, `paint_scrollbar`, `model_index_at`); ре-экспорт —
`crates/canvas-ui/src/kit.rs`. Потребители: `crates/canvas-app/src/app/stage.rs`
(retained-таблицы «Как считается»; FontSystem создаётся один раз),
`crates/canvas-app/src/admin_ui.rs`; витрина —
`crates/canvas-app/src/kit_ui.rs` (`SECTION_TABLE`, оракул «Row ≡ Table»).

## 8. Что меняется при правке файла
- Правка геометрии строки → сначала `../use-cases/row.md` (additive: Row v1
  не меняется, Table потребляет kit-функции).
- Оракулы I-1 (`*_with` ≡ kit-функции, Component ≡ `*_with`) — тесты модуля;
  деградация §4.2 держит паритет stage.rs (1386–1392).
- Дефолты opts → S3/S1 `../rules/03-spacing-radius.md`; зебра —
  TABLE_ZEBRA_RUN_MIN (S6).
- Новое поле стиля → TableRowStyle + `../use-cases/row.md` + K1/K2
  `../rules/10-components.md`; витрина — `crates/canvas-app/src/kit_ui.rs`.
