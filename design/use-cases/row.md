# Строка таблицы (Row)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/row.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Строка табличного тела на колоночных направляющих: маркер/лейбл/значение/
юнит/бейдж — панель «Как считается», витрина, админ-консоль. Выбор — K1.2
(`../rules/10-components.md`): табличные гайды — Row/Table. Разделение труда
(таблица K1): Row — направляющие + построчный рендер (замер → направляющие →
геометрия → стиль → отрисовка одной строки); Table — retained-оркестрация:
общий замер → направляющие → окно → per-row стиль → бегунок
(`../use-cases/table.md`); список без колонок — List
(`../use-cases/list.md`); тело ноды — на kit-функциях (FR-061).

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Кегль / семейство | ROW_DEFAULT_SIZE 12, ROW_DEFAULT_FAMILY Noto Sans Display |
| Высота строки | не фиксируется китом — слот потребителя (stage 22; Table — LIST_ROW_H 26) |
| Точка-маркер | ROW_DOT 6, отступ ROW_DOT_PAD 6 |
| Зона глифа («ƒ») | ≥ ROW_GLYPH_MIN_W 12, зазор ROW_GLYPH_GAP 2 |
| Текстовые зазоры | ROW_TEXT_GAP 6 |
| Зазор ячеек направляющих | opts.gap — TABLE_GUIDE_GAP 6 (тело ноды — TABLE_NODE_GUIDE_GAP 8) |
| Пад пилюли бейджа | ROW_BADGE_PAD_H 6 (ширина = текст + 2·пад) |
| Линейная коробка текста | size · ROW_LINE_FRAC 1.1 |
| Лидер | TABLE_LEADER_MIN 8, PAD 4, DASH 2, GAP 3, H 1, Y_FRAC 0.62 |

Колонки — двухпроход (FR-061 §3.1): замер всех строк (максимумы
value/unit/badge) → позиции от правого края (`RowGuides::with_right_edge`);
значение/юнит прижаты вправо (D-4), лидер — пунктир до направляющей чисел (D-5).

## 3. Токены (слоты KitPalette)
Стиль — `row_style(state)`, только слоты (F-8), без арифметики над цветами:
| Поле | Normal | Hovered | Selected | Disabled |
|---|---|---|---|---|
| fill/border строки | прозрачные | hover_fill / прозрачная рамка | selected_fill / accent | прозрачные |
| marker/label/value/badge | text | = Normal | text | disabled_text |
| unit | text_muted | = Normal | text_muted | disabled_text |
| badge_fill/badge_border | control_fill / control_border | = Normal | = Normal | прозрачные |

Потребитель переопределяет поля семантикой своей поверхности — plain data.
Зебра — не слот: фон прогона из ≥ TABLE_ZEBRA_RUN_MIN 4 строк кладётся в fill
сам (помощник — `zebra_run_flags`; tint-варианты — только K2.2).

## 4. Состояния
Компонент Row владеет `WidgetState` (FR-057) → `KitState` → `row_style`;
матрица и приоритеты — ST1 (`../rules/08-states.md`). Hover/press ведёт
потребитель (ST1/ST3); строка сама не кликает — hit-цель это rect[0]
выдачи `row_rects`.

## 5. Взаимодействие
- **Мышь**: hover/клик обрабатывает потребитель по слоту строки (rect[0]);
  точка/глиф/лидер/тексты — не hit-цели (данные `RowLayout`).
- **Клавиатура/Esc**: строка не перехватывает Esc (уходит Esc-стеку,
  `../rules/05-layering.md`); фокус — FocusRing поверхности.
- **Пайплайн**: `row_guides` → `row_layout` (усечение лейбла ellipsis,
  класс CR-015) → `paint_row` (порядок: фон/рамка → маркер → лейбл → лидер →
  значение → юнит → бейдж).
- **Замер**: продакшн — `layout_row_with` с общим FontSystem рендера
  (FR-094): внутренний пул компонента в wasm пуст — шейпинг упадёт.

## 6. Граничные случаи
- Пустое значение — ячейки нет и лидера нет (строки-формулы панели).
- Дорожка лидера короче TABLE_LEADER_MIN 8 — штрихи не рисуются.
- Лейбл усекается до ячейки значения/юнита/края строки — колонка левых
  текстов стабильна; `row_guides` на пустом списке → None (одна строка —
  направляющие всегда построены).

## 7. Где в коде
`crates/canvas-ui/src/component/row.rs` — `RowParts`/`RowStyle`/`row_style`/
`row_guides`/`row_layout`/`paint_row`/`leader_dash_rects`/`Row`+`RowProps`/
`row_rects`; направляющие — `crates/canvas-ui/src/row_guides.rs`; токены
TABLE_* — `crates/canvas-core/src/tokens.rs`; ре-экспорт —
`crates/canvas-ui/src/kit.rs`. Витрина — `crates/canvas-app/src/kit_ui.rs`
(`SECTION_COMPONENT_ROW`); админ-консоль — `crates/canvas-app/src/admin_ui.rs`;
тело ноды — свой проход (FR-061); лидер единый с `crates/canvas-render/src/text.rs`.

## 8. Что меняется при правке файла
- Константы (DOT/GAP/BADGE/LEADER) → row.rs +
  `crates/canvas-core/src/tokens.rs` (I-5; S1/S6 `../rules/03-spacing-radius.md`).
- Смена геометрии лидера — паритет с телом ноды (`crates/canvas-render/src/text.rs`).
- Новое поле стиля → RowStyle + TableRowStyle (`../use-cases/table.md`) +
  K1/K2 `../rules/10-components.md` (K2.1); состояния — `../rules/08-states.md`.
- Оракулы row.rs — тесты модуля; витрина K4: Row без живого образца —
  незакрытая волна.
