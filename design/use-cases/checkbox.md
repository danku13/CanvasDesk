# Чекбокс (Checkbox)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/checkbox.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Мульти-выбор независимых опций: «двух/трёхсостояющий toggle с подписью»
(Wave C §5.3.1). Типовой сценарий — «select all» с Indeterminate («выбраны
не все»). Не путать: Switch — бинарный вкл/выкл в виде пилюли
([../use-cases/switch.md](../use-cases/switch.md)); RadioGroup/RadioCard —
эксклюзивный выбор одного (правило выбора K1.2,
[../rules/10-components.md](../rules/10-components.md)); SegmentedControl —
переключение режимов, а не опций.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Квадрат (`box_rect`) | сторона = `ControlSize::chip_h()`: Xs 20 / Sm 24 (default) / Md 28 / Lg 32 |
| Позиция квадрата | слева, по центру вертикали слота: y = slot.y + (slot.h − box).max(0)/2 |
| Подпись (`label_rect`) | справа от квадрата; зазор `label_gap` (обычно `Spacing::Sm` = 8 px) |
| Ширина подписи | замер `TextMeasurer` + ellipsis до slot.right() − label_x (кламп ≥ 0) |
| Глифы (`checkbox_glyph`) | Unchecked «», Checked «✓», Indeterminate «−» |
| Радиус | `Shape::S.px()` = 8 |

## 3. Токены (слоты KitPalette)
| Ситуация | Заливка | Рамка | Текст |
|---|---|---|---|
| Checked, Normal | control_primary | прозрачная | text_title |
| Indeterminate/Unchecked, Normal | control_fill | Unchecked → control_border; Indeterminate → прозрачная | text |
| любой, Hovered/Pressed | hover_fill | Unchecked + hover → accent | по CheckboxState |
| любой, Disabled | по CheckboxState | по CheckboxState | disabled_text |

Слоты — реестр K2 ([../rules/10-components.md](../rules/10-components.md));
новые цвета не вычисляются.

## 4. Состояния
Собственный enum `CheckboxState`: Unchecked (default) / Checked / Indeterminate
— матрица × `KitState` по ST1 ([../rules/08-states.md](../rules/08-states.md)).
`checkbox_style` дифференцирует Hovered/Pressed (= hover_fill) и Disabled
(= disabled_text); Focused/Dragged/Selected/Error визуально не отличаются от
Normal. Заголовок модуля фиксирует «3 × 7 KitState» — написан до появления
8-го значения Error (Wave T §5.1.1). Клавиатура: Space toggles; touch:
hit ≥ 44 px (box + label) — MIN_TOUCH_TARGET, LAY8.3
([../rules/11-layouts.md](../rules/11-layouts.md)).

## 5. Взаимодействие
- **Мышь**: `checkbox_hit` — клик по квадрату ИЛИ подписи переключает;
  клик-контракт ST3 (press + release внутри).
- **Клавиатура**: Space toggles (doc-comment модуля); перевод Indeterminate →
  Checked — решение потребителя (toggle-функции в модуле нет).
- **Touch**: хит-зона = box + label; правило 44 px — LAY8.3, расширение hit —
  на потребителе (`ControlSize::Lg` даёт квадрат 32).

## 6. Граничные случаи
- Пустая подпись: замер не выполняется, `label` = пустая строка.
- Узкий слот: подпись обрезается ellipsis, ширина клампится до 0 — за слот не
  выходит; квадрат по высоте клампится внутрь слота.
- Indeterminate: глиф «−»; состояние хранит потребитель — модуль его не меняет.
- Disabled: клик игнорируется потребителем (ST1: press не армит).

## 7. Где в коде
`crates/canvas-ui/src/component/checkbox.rs` — `CheckboxState`,
`checkbox_glyph`, `checkbox_layout`, `checkbox_style`, `checkbox_hit` +
unit-тесты. Фасад — `crates/canvas-ui/src/kit.rs` (re-export «Wave C §5.3»).
Витрина `crates/canvas-app/src/kit_ui.rs` секции для checkbox не содержит
(K4: компонент без секции витрины — незакрытая волна). Потребителей в app
пока нет.

## 8. Что меняется при правке файла
- Строки в реестре K1 нет — по K1.1 компонент обязан появиться в таблице K1
  той же волной, что и правка.
- Слоты → синхронная правка K2-таблицы
  ([../rules/10-components.md](../rules/10-components.md)); геометрия — шкалы
  S ([../rules/03-spacing-radius.md](../rules/03-spacing-radius.md)).
- Глифы «✓»/«−», центрирование квадрата и hit box+label закреплены
  unit-тестами модуля (golden-поведение).
- Волна-спека: Wave C §5.3.1
  (`docs/dev-researches/ui-kit-deep-review-2026-10.md`).
