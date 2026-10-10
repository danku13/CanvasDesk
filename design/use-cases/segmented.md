# Сегментированный переключатель (SegmentedControl)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/segmented.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Переключение взаимоисключающих режимов/видов (Apple HIG segmented control —
Wave C §5.3.12; целевые места — what-if mode switcher, view-mode switcher).
Не путать: RadioGroup/RadioCard — выбор значения в форме (K1.2:
эксклюзивный выбор — RadioCard, [../rules/10-components.md](../rules/10-components.md));
Checkbox — мульти-выбор ([../use-cases/checkbox.md](../use-cases/checkbox.md));
чипы-фильтры ([../use-cases/chip-badge.md](../use-cases/chip-badge.md)) —
независимые фильтры, не режимы. K1.2 для режимных переключателей канона не
называет — параллельные решения из кнопок не разводить.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Контейнер (`container`) | весь слот (pill-фон) |
| Сегменты (`segments`) | N штук равной ширины: seg_w = slot.w / N |
| Активный (`active_rect`) | rect сегмента с индексом `active` (заливка) |
| Радиус | `Shape::L.px()` = 12 (= RADIUS_PILL, S2 в [../rules/03-spacing-radius.md](../rules/03-spacing-radius.md)) |
| Высота | не фиксируется компонентом — высота слота |

## 3. Токены (слоты KitPalette)
`segmented_style(active, state)` → (контейнер, стиль активного/контейнера):

| Элемент | Normal | Hovered/Pressed | Текст |
|---|---|---|---|
| Контейнер (неактивный сегмент) | control_fill, рамка прозрачная | hover_fill | text |
| Активный сегмент | control_primary | control_primary (hover не различается) | text_title |

Слоты — реестр K2 ([../rules/10-components.md](../rules/10-components.md));
elevation — `Elevation::None`.

## 4. Состояния
Собственное состояние — `active: usize` (индекс активного сегмента) ×
`KitState` по ST1 ([../rules/08-states.md](../rules/08-states.md)).
Дифференцируются Hovered/Pressed (контейнер → hover_fill); Focused/Dragged/
Selected/Error и Disabled — без отдельного слота (Disabled-хром — зона
потребителя, как в ST6).

## 5. Взаимодействие
- **Мышь**: клик по сегменту активирует его — `segmented_hit` возвращает
  индекс сегмента или None; клик-контракт ST3 (press + release внутри).
- **Клавиатура**: ←/→ — `SegmentedKey::Prev/Next` с циклическим обходом;
  `segmented_key` возвращает Some(новый индекс), если он изменился, иначе
  None; count = 0 → None.
- **Touch**: hit-зоны = rect'ы сегментов (равные доли слота — узкие при
  большом N); правило 44 px (LAY8.3,
  [../rules/11-layouts.md](../rules/11-layouts.md)) — зона потребителя.

## 6. Граничные случаи
- Пустой список labels: `segments` пуст, `active_rect` — нулевой rect
  (`UiRect::default()`), рисовать нечего.
- `active` вне диапазона: `active_rect` = нулевой rect
  (`segments.get(active)` → default) — заливки нет, проверок нет.
- Длинная подпись не расширяет сегмент: ширины строго равны slot.w/N;
  укладку/ellipsis текста делает потребитель (замерщики переданы в layout,
  но на ширины не влияют).
- Wrap-навигация у краёв: Next от последнего → 0, Prev от 0 → последний.

## 7. Где в коде
`crates/canvas-ui/src/component/segmented.rs` — `SegmentedLayout`,
`SegmentedKey`, `segmented_layout`, `segmented_style`, `segmented_hit`,
`segmented_key` + unit-тесты. Фасад — `crates/canvas-ui/src/kit.rs`
(re-export «Wave C §5.3»). Витрина `crates/canvas-app/src/kit_ui.rs` секции
для segmented не содержит (K4: незакрытая волна). Потребителей в app пока нет.

## 8. Что меняется при правке файла
- Строки в реестре K1 нет — по K1.1 компонент обязан появиться в таблице K1
  той же волной, что и правка.
- Слоты → синхронная правка K2-таблицы
  ([../rules/10-components.md](../rules/10-components.md)); радиус — S2
  ([../rules/03-spacing-radius.md](../rules/03-spacing-radius.md)).
- Равные ширины, wrap-навигация и hit-индексы закреплены unit-тестами модуля
  (golden-поведение); анимации переключения не вводить (K1.3 — компонент
  себя не анимирует).
- Волна-спека: Wave C §5.3.12
  (`docs/dev-researches/ui-kit-deep-review-2026-10.md`).
