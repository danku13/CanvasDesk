# Дерево (Tree)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/tree.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Иерархический список с раскрытием ветвей. Цитата модульного doc-comment:
«Wave C §5.3.11: Tree — иерархический список (expand/collapse)… Keyboard:
↑/↓ navigate, ←/→ expand/collapse, Enter activate» (спека —
`docs/dev-researches/ui-kit-deep-review-2026-10.md` §5.3.11; референсы —
Carbon tree, M3 list; потребители по спеке — explain panel, будущий
layers panel). Отличие от List (§K1): List — плоский список строк одной
высоты; Tree — flatten видимых узлов с отступами по глубине и шевронами.
Данные — рекурсивный `TreeNode` (id/label/children/expanded/depth/icon);
раскрытие — состояние данных (`expanded`), а не кита.

## 2. Анатомия и размеры
| Элемент | Геометрия (из `tree_layout`) |
|---|---|
| Строка | вся ширина слота × `row_h` (параметр; в тестах 26 = LIST_ROW_H §K1) |
| Отступ | `indent` = depth × `indent_step` (параметр; в тестах 16) |
| Шеврон | 16 px, только если есть children; иначе rect нулевой |
| Иконка | 20 px, только если `icon` задан |
| Зазоры | шеврон → иконка 4; иконка → подпись 4 (без иконки — 0) |
| Подпись | остаток ширины: slot.w − (label_x − x), кламп ≥ 0 |
| Текст | ellipsis по ширине подписи (`TextMeasurer::ellipsis`) |

Видимые строки: дети попадают в flatten только при `expanded = true`
родителя (порядок — обход сверху вниз).

## 3. Токены (слоты KitPalette)
В модуле стилевой функции нет — цвета строки назначает потребитель по ST1/
ST2 (реестр §K2 `../rules/10-components.md`): строка Normal/Hovered/
Pressed/Disabled — как у List (rows — «consumer's WidgetState» в §K1);
выбранный узел — selected_fill; шеврон — text_muted; фокус-рамка — accent.
Новые цвета — только новые слоты (K2.1), не локальные литералы.

## 4. Состояния
Матрица ST1 `../rules/08-states.md`: Disabled > Error > Pressed > Hovered >
Focused > Dragged > Selected > Normal — на строку (не на узел данных). Модель выбора: один выделенный узел
(id); Selected ортогонален hover (ST1); раскрытость — свойство узла, не
состояние контрола. Keyboard-фокус — KitState::Focused (state-layer 0.10,
focus-ring `accent`, Wave T §5.1.1). Клавиатурный контракт обеспечен
кодом (issue #67 V2): enum `TreeAction` {FocusNext, FocusPrev, Expand,
Collapse, Activate, None} + резолвер `tree_key(TreeKey, TreeKeyCtx) ->
TreeAction` в модуле (↑/↓ — FocusPrev/FocusNext, ←/→ — Collapse/Expand
узла с детьми, Enter — Activate; избыточные ←/→ — None), покрыт юнит-тестами.

## 5. Взаимодействие
- **Мышь**: `tree_hit` — id строки по точке (клик мимо строк — None); клик
  по шеврону и по строке различает потребитель по `chevron`-rect внутри
  `TreeRow`; toggle раскрытия = мутация `expanded` узла + перелэйаут.
- **Клавиатура** (контракт этого файла): ↑/↓ — соседние видимые строки
  (FocusPrev/FocusNext), ←/→ — Collapse/Expand, Enter — Activate; резолвер
  `tree_key` в модуле (issue #67 V2; demo-потребитель — стрелки группы
  дерева витрины, `kit_demo::apply_group_arrow`).
- **Touch**: зоны строк/шевронов — LAY8.3 (`../rules/11-layouts.md`), 44
  logical px на coarse-pointer, центр, кламп в строку.

## 6. Граничные случаи
- Глубокая вложенность: indent растёт линейно, подпись сжимается до 0 и
  урезается ellipsis (кламп `max(0.0)`); минимальная читаемая ширина —
  забота потребителя (политика слота SqueezeTail, LAY3).
- Строк больше слота: `tree_layout` раскладывает все видимые строки вниз от
  slot.y без окна и скролла — окно видимости ведёт потребитель (шаг строк
  однородный: окно `ScrollState` по K3, частичные строки — `list_rows`).
- Лист без детей: `has_children = false`, rect шеврона нулевой — клик по
  месту шеврона попадает в строку.
- Пустое дерево: одна строка корня (без детей детей не бывает).

## 7. Где в коде
`crates/canvas-ui/src/component/tree.rs` — `TreeNode` (+ билдеры
`new`/`child`/`expanded`), `tree_layout`/`TreeRow`, `tree_hit`, `TreeAction`;
ре-экспорт из `crates/canvas-ui/src/kit.rs` (строки 129–130). Витрина: секция tree в `crates/canvas-app/src/kit_sections_k4.rs`
(K4) — 4 ряда Normal/Hovered/Selected/Disabled, раскрытые и свёрнутые
узлы (индент уровня 16).

## 8. Что меняется при правке файла
- Геометрия/flatten → `tree_layout` + юнит-тесты модуля
  (`tree_flatten_expanded`, `tree_flatten_collapsed`, `tree_hit_returns_id`);
  G4 — при подключении к поверхности.
- `tree_key` (резолвер ↑/↓/←/→/Enter → `TreeAction`) реализован
  (issue #67 V2), расхождение из §4 закрыто; контракт зафиксирован этим файлом.
- Высота/отступ → параметры потребителя; выравнивание на константы §K1/
  `../rules/03-spacing-radius.md` (LIST_ROW_H 26).
- Витрина/эталоны: SECTION_* в `crates/canvas-app/src/kit_ui.rs`;
  snapshot-эталон через `Comp` (`crates/canvas-ui/tests/snapshot.rs`,
  FR-068 W0, Контракт-9) — сейчас дерева там нет.
