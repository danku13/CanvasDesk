# Слайдер (Slider)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/slider.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Выбор числового значения в диапазоне: continuous либо discrete (`step`).
Целевые места — what-if сценарии и настройки (cost-limit, conf-threshold —
Wave C §5.3.3). Не путать: скроллбар — просмотр содержимого, не ввод значения
([../use-cases/scrollbar.md](../use-cases/scrollbar.md)); текстовое поле —
точный ввод значения ([../use-cases/text-field.md](../use-cases/text-field.md));
Progress — индикация без ввода.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Трек (`track`) | высота = `button_h(size)`·0.4, минимум 4 (Sm → 12); вся ширина слота, по центру вертикали |
| Заполнение (`filled`) | от начала трека до значения: ширина = slot.w × norm |
| Бегунок (`knob`) | сторона = `button_h(size)`·0.6 (Sm → 18); центр на значении |
| `SliderOpts` | min 0, max 1 (default); `step: None` — continuous, `Some(step)` — discrete; `discrete_ticks`; `label_format` (например «50 ms») |
| Нормализация | norm = clamp((value − min)/(max − min), 0..1); guard диапазона 1e-6 |
| Радиус трека | `Shape::Full.px()` (pill) |

## 3. Токены (слоты KitPalette)
`slider_style(state, p)` → (ControlStyle трека, заливка бегунка):

| Элемент | Normal | Hovered/Pressed | Disabled |
|---|---|---|---|
| Трек | control_fill + рамка control_border | hover_fill | control_fill |
| Бегунок | control_primary | control_primary | disabled_text |

Focused/Dragged/Error `slider_style` не дифференцирует; фокус-ринг и
state-layer — на потребителе (Wave T §5.1.1, ST2).

Doc-comment: «track — control_fill, filled — primary, knob —
primary/text_title»; elevation — `Elevation::None`. Слоты — реестр K2
([../rules/10-components.md](../rules/10-components.md)).

## 4. Состояния
Собственного enum нет: состояние = значение value в [min..max] × `KitState`
по ST1 ([../rules/08-states.md](../rules/08-states.md)). `slider_style`
дифференцирует Hovered/Pressed (hover_fill) и Disabled (disabled_text);
Focused/Dragged/Selected/Error — как Normal. Pressed здесь — зажатый drag
бегунка (SliderHit::Knob).

## 5. Взаимодействие
- **Мышь**: `slider_hit` → `SliderHit::Knob` (drag) или `SliderHit::Track`
  (jump-to-click — клик-контракт ST3: press + release внутри трека).
- **Клавиатура** (`slider_key` + `SliderKey`): ←/→ (Decrement/Increment) —
  step; Home/End — min/max; PageUp/PageDown — big step. step по умолчанию =
  1% диапазона, big step = 10%; при заданном `step` PageUp/Down используют
  его же. Результат клампится в [min..max]; необработанный ключ — None.
- **Touch**: hit-зоны — rect'ы knob/track; правило 44 px (LAY8.3) — зона
  потребителя, в модуле не зафиксировано.

## 6. Граничные случаи
- value вне диапазона: norm клампится (15 при max 10 → полная заливка; −5 → 0).
- Вырожденный диапазон max − min → 0: guard 1e-6 против деления на ноль.
- step не задан: 1% / 10% от диапазона, не абсолютные константы.
- Клавиатурный шаг у границы: Decrement от 2 при min 0 → ровно min (кламп).
- Метки `discrete_ticks` и value-label `label_format` рисует потребитель —
  кит отдаёт layout/opts (текст — через TextMeasurer, LAY6).

## 7. Где в коде
`crates/canvas-ui/src/component/slider.rs` — `SliderOpts`, `SliderLayout`,
`SliderHit`, `SliderKey`, `slider_layout`, `slider_style`, `slider_hit`,
`slider_key` + unit-тесты. Фасад — `crates/canvas-ui/src/kit.rs` (re-export
«Wave C §5.3»). Витрина `crates/canvas-app/src/kit_ui.rs` секции для slider
не содержит (K4: незакрытая волна). Потребителей в app пока нет.

## 8. Что меняется при правке файла
- Строки в реестре K1 нет — по K1.1 компонент обязан появиться в таблице K1
  той же волной, что и правка.
- Слоты → синхронная правка K2-таблицы
  ([../rules/10-components.md](../rules/10-components.md)); высоты трека/knob
  производны от `ControlSize` (S3-шкалы,
  [../rules/03-spacing-radius.md](../rules/03-spacing-radius.md)).
- Центрирование трека, клампы значения и шаги клавиатуры закреплены
  unit-тестами модуля (golden-поведение).
- Волна-спека: Wave C §5.3.3
  (`docs/dev-researches/ui-kit-deep-review-2026-10.md`).
