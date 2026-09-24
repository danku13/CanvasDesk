# Кнопка (Button)

## 1. Назначение
Базовый интерактивный элемент кита: primary (главное действие модалки/empty
state), secondary (обычное действие), ghost (второстепенное, без заливки),
danger (деструктивное — удаляет сценарий/связь). Используется поверхностями
через `kit::button*`; самостоятельных кнопок-квадов вне кита быть не должно.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Высота | BUTTON_HEIGHT 30 |
| Гориз. паддинг | BUTTON_PAD_H 12 (SPACING_LG) |
| Иконка-кнопка | ICON_BUTTON_SIZE 26×26 |
| Радиус | RADIUS_CHIP 6 |
| Шрифт | body 14 (экранный), line = size·1.3 |
| Текст | по центру; ширина = текст + 2·12 (замер TextMeasurer) |

## 3. Токены (слоты KitPalette)
| Вариант | Normal | Hovered | Disabled |
|---|---|---|---|
| Primary | control_primary (#2952A0-родственный диалоговый) | control_primary_hover_fill [0.248,0.456,0.84,1] | control_primary + текст control_disabled_text |
| Secondary/Ghost | control_fill (прозрачный/панельный) | control_hover_fill | control_fill + control_disabled_text |
| Danger | control_danger (= error) | = error (v1 не дифференцируется) | control_fill |

Рамка — control_border; текст — text (primary: белый/ink по контрасту).

## 4. Состояния
Матрица `rules/08-states.md`: Disabled > Pressed > Hovered > Selected >
Normal. Pressed в v1 визуально = Normal (дифференциация — v2 через правку
`rules/08-states.md`).

## 5. Взаимодействие
- **Мышь**: press внутри → отпуск внутри = клик (один раз). Увод курсора при
  зажатии отменяет нажатие (release вне — не клик). Hover — смена слота.
- **Клавиатура**: кнопка участвует в FocusRing поверхности (Tab/Shift+Tab);
  рамка фокуса — accent; активация — Enter/Space (через surface-обработчик).
- **Esc**: кнопка не перехватывает Esc (Esc уходит Esc-стеку).

## 6. Граничные случаи
- Текст не влезает: ширина кнопки = контент (кнопки не сжимают текст;
  ужимание — задача родительского слота SqueezeTail).
- Кнопка primary на светлой теме: тот же слот (не дифференцирован) — I-1.
- Клик по disabled: ничего не происходит, press не армит.

## 7. Где в коде
`crates/canvas-ui/src/kit.rs` — BUTTON_HEIGHT/BUTTON_PAD_H (122–166),
`button_style` (254–290), `ButtonVariant` (48–57); состояние — `widget.rs`
WidgetState. Потребители: settings_ui, whatif_ui, scheme_gallery_ui, dialog
(confirm) в app.rs.

## 8. Что меняется при правке файла
- Размеры/паддинг → константы kit.rs + при смене кегля шкала `rules/02-typography.md`.
- Цвета состояний → слоты ThemeColors/KitPalette (не локальные значения),
  после правки — прогон G3.
- Новое состояние → новый слот + правка `rules/08-states.md`.
