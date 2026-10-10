# Радиокнопка (Radio / RadioGroup)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/radio.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Эксклюзивный выбор одного из N в компактных формах: «круг + подпись»
(Wave C §5.3.4 — введён, потому что `radio_card` для плотных форм крупен).
Не путать: RadioCard — карточный выбор с описанием, канон эксклюзивного
выбора по K1.2 ([../rules/10-components.md](../rules/10-components.md),
[../use-cases/radio-card.md](../use-cases/radio-card.md)); Checkbox —
мульти-выбор ([../use-cases/checkbox.md](../use-cases/checkbox.md));
SegmentedControl — переключение режимов, а не значений формы.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Круг (`circle`) | диаметр = `ControlSize::chip_h()`: Xs 20 / Sm 24 (default) / Md 28 / Lg 32 |
| Подпись (`label_rect`) | справа от круга, зазор `label_gap`; ширина = замер `TextMeasurer` + ellipsis (кламп ≥ 0) |
| Группа | `RadioGroup { options, selected }`; `new` → selected = 0 |
| Ориентация | `RadioOrientation::Vertical` (default) / Horizontal; Vertical: y += circle_d + gap; Horizontal: x += (label_w + label_gap + circle_d) + gap |
| Радиус круга | `Shape::Full.px()` (окружность) |

## 3. Токены (слоты KitPalette)
`radio_style(selected, state)`:

| selected | Заливка | Рамка | Текст |
|---|---|---|---|
| true | control_primary; Hovered/Pressed → hover_fill | control_primary | text; Disabled → disabled_text |
| false | прозрачная; Hovered/Pressed → hover_fill | control_border | text; Disabled → disabled_text |

Слоты — реестр K2 ([../rules/10-components.md](../rules/10-components.md)).

## 4. Состояния
Собственное состояние — `selected` пункта (bool; группа хранит индекс
`selected: usize`) × `KitState` по ST1
([../rules/08-states.md](../rules/08-states.md)). `radio_style`
дифференцирует Hovered/Pressed (hover_fill) и Disabled (disabled_text);
Focused/Dragged/Selected/Error — как Normal. Выбор эксклюзивный: выбор
другого пункта снимает его с прежнего (ведёт группа, не пункт).

## 5. Взаимодействие
- **Мышь**: `radio_hit` — клик по кругу ИЛИ подписи; клик-контракт ST3.
- **Клавиатура**: ←/→/↑/↓ — FocusZone pattern (заголовок модуля):
  `RadioKey::Prev` (←/↑) и `Next` (→/↓) с циклическим обходом; семантика
  одинакова для обеих ориентаций. `radio_group_key` возвращает true, если
  selected изменился.
- **Touch**: hit = круг + подпись; правило 44 px (LAY8.3,
  [../rules/11-layouts.md](../rules/11-layouts.md)) — зона потребителя,
  в модуле не зафиксировано.

## 6. Граничные случаи
- Пустая группа (options пуст): `radio_group_key` — no-op (false).
- Wrap: Next от последнего → 0; Prev от 0 → последний.
- Некорректный индекс `selected` (вне диапазона): ни один пункт не выделен —
  сравнение `i == group.selected` без клампа.
- Горизонтальная ориентация: ширина пункта считается замером
  (`TextMeasurer::width_of`), шаг — по фактической ширине, не по общей.
- Узкий слот: подпись ellipsis, ширина клампится до 0.

## 7. Где в коде
`crates/canvas-ui/src/component/radio.rs` — `RadioGroup`,
`RadioOrientation`, `RadioLayout`, `RadioKey`, `radio_group_layout`,
`radio_style`, `radio_hit`, `radio_group_key` + unit-тесты. Фасад —
`crates/canvas-ui/src/kit.rs` (re-export «Wave C §5.3»). Витрина
`crates/canvas-app/src/kit_ui.rs` секции для radio не содержит (K4:
незакрытая волна). Потребителей в app пока нет.

## 8. Что меняется при правке файла
- Строки в реестре K1 нет — по K1.1 компонент обязан появиться в таблице K1
  той же волной, что и правка (K1.2: не плодить параллельных решений с
  RadioCard).
- Слоты → синхронная правка K2-таблицы
  ([../rules/10-components.md](../rules/10-components.md)); геометрия —
  шкалы S ([../rules/03-spacing-radius.md](../rules/03-spacing-radius.md)).
- Шаг вертикали (circle_d + gap), wrap-навигация и hit круг+подпись
  закреплены unit-тестами модуля (golden-поведение).
- Волна-спека: Wave C §5.3.4
  (`docs/dev-researches/ui-kit-deep-review-2026-10.md`).
