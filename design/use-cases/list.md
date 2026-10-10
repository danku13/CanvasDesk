# Список и скролл (List)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/list.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Однородный список строк с прокруткой: пункты контекстных меню и подменю,
настройки, docs viewer, панель «Как считается». Выбор — K1.2
(`../rules/10-components.md`): список строк — List; табличные направляющие —
Row/Table (`../use-cases/row.md`, `../use-cases/table.md`); контейнер —
Panel. Кит даёт окно видимости и бегунок (`list_rows`/`scroll_bar` — K3);
фон/зебру/выделение/тексты строк рисует потребитель слотами своей темы.
Своя модель скролла в поверхности (`offset: f32` + ручной кламп) запрещена
(K3, аналог IN1).

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Высота строки | LIST_ROW_H 26 (S3; дефолт кита) |
| Зазор строк | LIST_ROW_GAP 6 (S1: SPACING_S) |
| Ширина бегунка | SCROLLBAR_WIDTH 4 |
| Мин. высота бегунка | SCROLLBAR_KNOB_MIN 20 |
| Модель скролла | `ScrollState { offset, content_h, viewport_h }` |
| Компонент | `List { props: ListProps, scroll }` (retained, FR-068 W3) |

`row_h`/`gap` — параметры потребителя (`ListProps`); константы кита — дефолты.
Контракт окна: `scroll.viewport_h == slot.h`; `content_h` — сумма высот всех
строк + зазоры (число строк `List` восстанавливает из `content_h`).

## 3. Токены (слоты KitPalette)
| Элемент | Слот |
|---|---|
| Фон/зебра/выделение строки | не рисуются китом — слоты темы потребителя (Normal — прозрачная строка) |
| Бегунок (`List::paint`) | control_border, рамка прозрачная, радиус = w/2 |

Зебра — не отдельный слот: фон прогона из ≥ TABLE_ZEBRA_RUN_MIN 4 строк
потребитель кладёт в fill сам (alpha-вариант слота — только через `tint()`,
K2.2). Интерактивные состояния бегунка (hover/drag), слот `scrollbar_thumb` и
поведение колеса/клавиатуры — контракт `../use-cases/scrollbar.md`; здесь —
только геометрия.

## 4. Состояния
Строки берут WidgetState потребителя (ST1, `../rules/08-states.md`); hover-
логики в List нет — образец `crates/canvas-app/src/app/overlays.rs`:
`chip_style(row_widget.kit_state(), …)` по rect'ам `list_rows`. Сам List —
контейнер, собственных состояний не имеет.

## 5. Взаимодействие
- **Окно видимости**: `list_rows(area, s, row_h, gap, count)` — чистая
  функция (не мутирует `ScrollState`); частичные строки на краях включаются;
  выдача — `(модельный индекс, rect)` в порядке индексов.
- **Скролл**: ведёт потребитель — `scroll_by` + `clamp()` (идемпотентен);
  `needs_scroll`/`max_offset` — производные поля модели K3.
- **Hit**: дефолтный `hit_test` — индекс строки, содержащей точку; точка в
  зазоре между строками — None (строка не перекрывает свой stride).
- **Paint**: только бегунок; строки не рисуются никогда (прозрачный rect —
  мусорный item draw-журнала Painter, G7).

## 6. Граничные случаи
- Пустой список (count 0), `row_h <= 0` или `viewport_h <= 0` → пустой Vec.
- Контент ≤ вьюпорта: `needs_scroll = false`, `max_offset = 0` — бегунка нет.
- `scroll_by` не клампит верх (только пол ≥ 0) — потребитель вызывает
  `clamp` для удержания в `[0, max_offset]`.
- Смещённый `offset`: частичная строка сверху получает отрицательный
  `screen_y` — полный rect отдаётся потребителю (клип — поверхность).

## 7. Где в коде
`crates/canvas-ui/src/component/list.rs` — `ScrollState`/`list_rows`/
`scroll_bar`/`List`+`ListProps`; константы LIST_ROW_H/LIST_ROW_GAP —
`crates/canvas-ui/src/component/mod.rs`; ре-экспорт —
`crates/canvas-ui/src/kit.rs` (`kit::list_rows`). Потребители:
`crates/canvas-app/src/calc_panel_ui.rs`, `crates/canvas-app/src/app/overlays.rs`,
`crates/canvas-app/src/app.rs`, `crates/canvas-app/src/settings_ui.rs`,
`crates/canvas-app/src/docs_ui.rs`; MIXED-stride список —
`crates/canvas-app/src/template_ui.rs` (`RowScroll`, LAY-W9). Витрина —
`crates/canvas-app/src/kit_ui.rs`, секция `SECTION_LIST` (K4).

## 8. Что меняется при правке файла
- Высота/зазор строки → LIST_ROW_* в `crates/canvas-ui/src/component/mod.rs`
  + строка S3 `../rules/03-spacing-radius.md` (I-5 паритет tokens).
- Окно/бегунок → K3 `../rules/10-components.md`: `list_rows` остаётся чистой
  функцией; геометрия бегунка — `../use-cases/scrollbar.md` (SCROLLBAR_*).
- Потребители `list_rows`/`ScrollState` — многие `*_ui.rs` surfaces; новая
  модель скролла рядом с K3 запрещена (LAY10).
- Правка ловится оракулами list.rs (layout ≡ `list_rows`, инварианты paint:
  1 item при переполнении / 0 без него).
