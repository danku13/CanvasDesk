# Радиокарточка (RadioCard)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/radio_card.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Эксклюзивный выбор одного варианта с заголовком и описанием: FR-UI-RADIO —
«radio-card компонент (label + desc + selected ring)»; канон эксклюзивного
выбора по K1.2 ([../rules/10-components.md](../rules/10-components.md)).
Паттерн извлечён из `ai_onboarding` (3×) и `graph_builder_ui` (3×).
Не путать: RadioGroup — компактный выбор без описаний (Wave C §5.3.4,
[../use-cases/radio.md](../use-cases/radio.md)); Checkbox — мульти-выбор
([../use-cases/checkbox.md](../use-cases/checkbox.md)).

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Карточка (`rect`) | весь слот; радиус RADIUS_PANEL (10) |
| Индикатор | `RADIO_INDICATOR_SIZE` 12×12 (явная константа kit-сущности, не выводится из RADIUS_PILL); слева (slot.x + SPACING_SM 8), по центру вертикали |
| Label | x = slot.x + pad + 12 + pad; y = slot.y + pad; высота `RADIO_LABEL_LINE_H` = 16 |
| Description | ниже label с зазором SPACING_S (6); высота `RADIO_DESC_LINE_H` = 14; `None` — без desc |
| Ширины | label_w/desc_w — замер потребителя (`TextMeasurer::width_of`); кламп .max(0.0) |

## 3. Токены (слоты KitPalette)
`radio_card(slot, label_w, desc_w, selected, state, palette)` →
(RadioCardLayout, RadioCardStyle):

| (selected, KitState) | card_fill | card_border | indicator_fill |
|---|---|---|---|
| (true, кроме Disabled) | tint(accent, 0.10) | accent | accent |
| (true, Disabled) | tint(accent, 0.10) | control_border | disabled_text |
| (false, Hovered/Pressed) | hover_fill | panel_border | text_muted |
| (false, Disabled) | panel_fill | panel_border | disabled_text |
| (false, остальные) | panel_fill | panel_border | text_muted |

label_color = text_title, desc_color = text_muted (Disabled → disabled_text
для обоих). Alpha 0.10 — историческое значение (`graph_builder_ui.rs`);
tint — именованный паттерн `crate::paint::tint` (K2.2: alpha-варианты слотов —
только через tint, rgb не меняется).

## 4. Состояния
`selected` (boolean) × `KitState` по ST1
([../rules/08-states.md](../rules/08-states.md)): Normal/Hovered/Disabled;
Pressed трактуется как Hovered — стиль не различает нажатие (строка K1:
«Normal/Hovered/Disabled + selected»). Focused/Dragged/Error отдельных
слотов не имеют.

## 5. Взаимодействие
- **Мышь**: зона клика — вся карточка (`rect` = весь слот); клик-контракт
  ST3; сам hit-тест и смена selected — у потребителя (кит отдаёт layout).
- **Клавиатура**: в модуле не зафиксирована; участие в фокус-кольце
  поверхности — зона потребителя.
- **Отрисовка**: `paint_radio_card` — ровно 2 `PaintItem` (фон карточки,
  затем индикатор поверх); индикатор — залитый кружок (radius = w/2);
  «кольцо» для unselected — ответственность потребителя (glyph-фолбэк
  `◉`/`○`).

## 6. Граничные случаи
- `desc_w = None` — карточка без описания: desc rect не возвращается.
- Отрицательные ширины label/desc клампятся .max(0.0) — за слот не выходят
  (тест layout_respects_slot_bounds).
- Pressed визуально = Hovered (не дифференцирован).
- Текст рисует потребитель через `Painter::label`/`TextMeasurer` (контракт
  F-8, PRD-0009 §8): кит отдаёт rect'ы подписи, цвет от геометрии не отделяет.

## 7. Где в коде
`crates/canvas-ui/src/component/radio_card.rs` — `RADIO_INDICATOR_SIZE`,
`RADIO_LABEL_LINE_H`, `RADIO_DESC_LINE_H`, `RadioCardLayout`,
`RadioCardStyle`, `radio_card`, `paint_radio_card`. Фасад —
`crates/canvas-ui/src/kit.rs` (re-export radio_card-группы). Потребители:
`crates/canvas-app/src/app/overlays.rs` (онбординг, kit::radio_card +
paint_radio_card), `crates/canvas-app/src/graph_builder_ui.rs` (glyph-фолбэк
`◉`/`○`). Витрина: секция radio_card в `crates/canvas-app/src/kit_ui.rs`
отсутствует — K4 прямо называет radio_card среди недостающих (W2 backlog).

## 8. Что меняется при правке файла
- Строка RadioCard в реестре K1 — синхронизировать анатомию/состояния/
  константы; сейчас в ней «RADIO_LABEL_LINE_H ≈ 17» против 16.0 в коде —
  расхождение таблицы реестра и константы, задача на сверку.
- Слоты/alpha → K2 ([../rules/10-components.md](../rules/10-components.md)):
  смена alpha 0.10 = правка K1-строки и этого документа одновременно (K2.2).
- Сигнатура `radio_card`/`paint_radio_card` ломает потребителя overlays.rs и
  graph_builder_ui.rs; порядок 2 PaintItem — golden-тест
  paint_emits_two_items_background_then_indicator.
- Контракт F-8 (PRD-0009 §8): цвета — только слоты; отступы/радиусы — только
  шкала SPACING_*/RADIUS_* ([../rules/03-spacing-radius.md](../rules/03-spacing-radius.md)).
