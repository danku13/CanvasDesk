# Панель (Panel)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/panel.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Контейнер-хром поверхностей: заливка/рамка/пад/радиус из слотов — панели
настроек, галереи, docs viewer, подложки модалок; сюда же карточка с хедером
(`card`) и затемнение под модалью (`backdrop`). Выбор — K1.2 и таблица K1
(`../rules/10-components.md`): контейнер — Panel; заголовок панели —
PanelHeader (`../use-cases/panel-header.md`); табличный контент — Row/Table;
список строк — List (`../use-cases/list.md`). Панель неинтерактивна: стиль
не зависит от hover/pressed/disabled — `WidgetState` не нужен (FR-068 W3).

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Пад | SPACING_LG 12 (`panel_style().pad`, S1) |
| Радиус | RADIUS_PANEL 10 (S2) |
| Заливка/рамка | слоты panel_fill/panel_border (K2) |
| Размер | `constrain(min, max, desired)` (LAY2) |
| Позиция | `stack` по центру слота (выравнивание — потребитель) |
| Card | хедер + body внутри пада; header_h клампится к остатку |
| Backdrop | rect без трансформаций, радиус 0 |

Геометрия: `panel_rect` → `panel_content` (минус пад). Срез компонента —
ровно 2 rect'а в фиксированном порядке: `[панель, контент]`.

## 3. Токены (слоты KitPalette)
| Элемент | Слот |
|---|---|
| Заливка панели | panel_fill (тема: menu_fill) |
| Рамка панели | panel_border (тема: palette_border) |
| Backdrop | stage_dim, рамка прозрачная, радиус 0 |

Смена палитры меняет fill — инвариант «цвета только слоты» (F-8). Явные
слоты (миграция пилотов, I-1 ноль визуального скачка): `panel_style_of(fill,
border, radius, pad)` — потребитель передаёт слоты своей темы (радиус —
каноническая константа поверхности до унификации v2). Новый слот — только
K2.1 (KitPalette + bridge + таблица K2 одновременно); alpha-варианты — K2.2.

## 4. Состояния
Собственных состояний нет — контейнер (таблица K1: «chrome without states»).
Содержимое панели — контролы с матрицей ST1 (`../rules/08-states.md`).
Модальное использование — контракт `../use-cases/modal-dialog.md` (слой
Modals, capture Block, дим stage_dim).

## 5. Взаимодействие
- **Мышь**: панель — не контрол (ST6): hover-состояний нет; дефолтный
  `hit_test` возвращает `ComponentHit { index: 0 }` по панели (панель раньше
  контента в срезе) — полезно потребителю поверх детей.
- **Layout**: backend не участвует — раскладка constrain+stack; `rects[1]`
  (контент) — готовый слот для детей потребителя, отрисовке не подлежит
  (рисует детей сам).
- **Draw-порядок**: backdrop добавляется потребителем ПЕРЕД панелью модали
  (порядок вызовов = порядок журнала, FR-057).

## 6. Граничные случаи
- desired зажимается в [min, max] (тест: 900×500 → 600×400 по max); границы —
  решение потребителя, кит не изобретает минимумов.
- Пустой rects в paint — no-op (деградация, не паника).
- `header_h` больше внутренней высоты — хедер сжимается до inner.h, body
  нулевой (`card`).
- Усечение среза до одного rect'а допустимо только у потребителя (paint
  терпит срез из 1 элемента).

## 7. Где в коде
`crates/canvas-ui/src/component/panel.rs` — `panel_style`/`panel_rect`/
`panel_content`/`panel_style_of`/`control_style_of`/`card`/`backdrop`/
`Panel`+`PanelProps`; ре-экспорт — `crates/canvas-ui/src/kit.rs`. Потребители:
`crates/canvas-app/src/app/stage.rs` (backdrop, panel_style_of),
`crates/canvas-app/src/admin_ui.rs` (panel_rect); модалки —
`crates/canvas-ui/src/component/modal.rs` (modal = stage_dim + панель).
Витрина — `crates/canvas-app/src/kit_ui.rs` (секция
`SECTION_COMPONENT_PANEL`, `SECTION_CARD` — `kit::card`).

## 8. Что меняется при правке файла
- Пад/радиус → S1/S2 `../rules/03-spacing-radius.md` +
  `crates/canvas-core/src/tokens.rs` (I-5 паритет tokens.dimensions.json).
- Цвета → слоты K2 `../rules/10-components.md` (K2.1: KitPalette + bridge +
  таблица; rgb не меняется — K2.2).
- Слои/dim-поведение модалок → `../rules/05-layering.md` (правка через ADR,
  не через этот файл).
- Геометрия Card — golden-тест `snapshot_switch_and_card_geometry_golden`
  в panel.rs; сдвиг любого из 2 rect'ов среза — оракулы компонента
  (`component_panel_layout_panel_then_content` и др.).
