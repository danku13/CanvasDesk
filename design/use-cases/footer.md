# Футер диалога (Footer)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/footer.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Правый ряд кнопок диалога/мастера (FR-UI-FOOTER): Cancel/OK, Prev/Next,
Generate/Cancel. Извлекает паттерн `autolink_ui`, `onboarding`, `settings`,
`graph_builder_ui`. Реестр K1 (`../rules/10-components.md`): строка Footer —
3-кнопочный правый футер; split-вариант «кнопка слева + кнопка справа» —
FR-UI-SPLIT-FOOTER (Prev|Next, Privacy|Continue). Кнопки —
`../use-cases/button.md`.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Высота кнопок | BUTTON_HEIGHT 30, вертикальный центр слота |
| Зазор | GAP_CONTROLS 8 (SPACING_SM, шкала S1) |
| Ширины | измеренные: текст + 2·BUTTON_PAD_H 12 (`TextMeasurer::width_of` / `button_size`) |
| Привязка | правый край последней кнопки = slot.right() (inset 0 в каноне кита; inset'ит потребитель) |
| BUTTON_WIDTH 100 | reference-default (`component/mod.rs`); фиксированный вариант `footer_buttons(slot, n)` удалён (FR-UI-FOOTER-R: 0 потребителей) |
| Split | левая группа — от slot.x + inset слева-направо; правая — от slot.right() − inset справа-налево |

## 3. Токены (слоты KitPalette)
Кит не выбирает цвет/текст — только геометрию (F-8). Стиль каждой кнопки —
`button_style(variant, state, palette)` потребителя; варианты Primary/
Secondary/Danger по индексу; слоты вариантов — таблица §3
`../use-cases/button.md` (control_primary / control_fill / control_danger,
hover-слоты, disabled_text).

## 4. Состояния
Футер — контейнер без собственных состояний; каждая кнопка — контрол полной
матрицы ST1 (`../rules/08-states.md`): Disabled > Error > Pressed > Hovered >
Focused > Dragged > Selected > Normal (Selected не используется). Disabled-кнопка не «заряжает» клик (ST1).

## 5. Взаимодействие
- Клик — контракт ST3: press внутри + release внутри; увод курсора при
  зажатии отменяет нажатие.
- Порядок действий задаёт потребитель индексацией и вариантами (канон
  Cancel/OK — primary справа); hover — смена слота кнопки.
- Клавиатура: кнопки в FocusRing поверхности (Tab/Shift+Tab, Enter/Space);
  Esc футер не перехватывает (Esc-стек, `../rules/05-layering.md`).

## 6. Граничные случаи
- Кнопки не помещаются: переполнение НЕ маскируется — start_x уходит влево за
  слот (ловит G4 `../rules/00-principles.md`); потребитель обязан дать
  достаточную ширину слота.
- n = 0 → пустой Vec (no-op); ширина клампится w.max(0.0).
- Split: возврат в порядке left-group → right-group; index — позиция внутри
  группы; при нехватке ширины группы накладываются (G4).

## 7. Где в коде
`crates/canvas-ui/src/component/footer.rs` — `footer_buttons_measured`,
`split_footer_buttons`, `FooterGroup`; константы —
`crates/canvas-ui/src/component/mod.rs` (BUTTON_HEIGHT/GAP_CONTROLS/
BUTTON_WIDTH); re-export — `crates/canvas-ui/src/kit.rs`. Потребители:
`crates/canvas-app/src/autolink_ui.rs` и `crates/canvas-app/src/app/graph_builder_ui.rs`
(измеряемый футер), `crates/canvas-app/src/onboarding_ui.rs` (split). Секции
Footer в витрине `crates/canvas-app/src/kit_ui.rs` нет — бэклог W2 (K4).

## 8. Что меняется при правке файла
- Зазор/высота → GAP_CONTROLS/BUTTON_HEIGHT (S1/S3
  `../rules/03-spacing-radius.md`); фиксированные ширины — только через правку
  контракта (CR-015: без эвристик «длина × 0.62»).
- API раскладок → паритеты onboarding (`button_rect` Prev/Next) и autolink
  (слот футера inset 16 px — геометрия потребителя).
- Юнит-тесты модуля (`measured_widths_right_aligned`,
  `split_footer_one_left_one_right_at_opposite_edges`) — замок контракта.
