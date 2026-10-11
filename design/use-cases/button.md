# Кнопка (Button)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/button.rs`.
> Актуализировано: 2026-10-10 (волны T/C/L, issue #47).

## 1. Назначение
Базовый интерактивный элемент кита: primary (главное действие модалки/empty
state), secondary (обычное действие), ghost (второстепенное, без заливки),
danger (деструктивное — удаляет сценарий/связь). Используется поверхностями
через `kit::button*`; самостоятельных кнопок-квадов вне кита быть не должно.
Волна T (§5.1.3) добавила варианты Tertiary/Text/Inverse в `ButtonVariant`.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Высота | BUTTON_HEIGHT 30 |
| Гориз. паддинг | BUTTON_PAD_H 12 (SPACING_LG) |
| Иконка-кнопка | ICON_BUTTON_SIZE 26×26 |
| Радиус | RADIUS_CHIP 6 |
| Шрифт | BUTTON_FONT_SIZE 13 (BUTTON_FAMILY «Noto Sans Display»), line = size·1.3 |
| Текст | по центру; ширина = текст + 2·12 (замер TextMeasurer, минимум BUTTON_HEIGHT) |

## 3. Токены (слоты KitPalette)
| Вариант | Normal | Hovered | Disabled |
|---|---|---|---|
| Primary | control_primary | primary_hover_fill | control_primary + текст control_disabled_text |
| Secondary/Ghost | control_fill (прозрачный/панельный) | hover_fill | control_fill + control_disabled_text |
| Danger | control_danger (= error) | hover_fill (общий с secondary) | control_danger + control_disabled_text |

Рамка — control_border (Ghost/Text — прозрачная, hover/pressed — accent).
Текст — text_title (Ghost/Text — text; disabled — disabled_text);
Pressed = Hovered (ST2).

## 4. Состояния
Матрица `../rules/08-states.md`: Disabled > Error > Pressed > Hovered >
Focused > Dragged > Selected > Normal. Pressed визуально = Hovered (одни и те же hover-слоты в
`button_style`; полная дифференциация — v2 через правку
`../rules/08-states.md`). Волна T расширила KitState (Focused/Dragged/Error,
additive `active_states`) — базовый порядок матрицы не менялся.

## 5. Взаимодействие
- **Мышь**: press внутри → отпуск внутри = клик (один раз — ребро
  `WidgetState::clicked`). Увод курсора при зажатии отменяет нажатие
  (release вне — не клик). Hover — смена слота.
- **Клавиатура**: кнопка участвует в FocusRing поверхности (Tab/Shift+Tab);
  рамка фокуса — accent; активация — Enter/Space (через surface-обработчик).
- **Esc**: кнопка не перехватывает Esc (Esc уходит Esc-стеку).

## 6. Граничные случаи
- Текст не влезает: ширина кнопки = контент; в узком слоте кнопка сжимается
  до ширины слота, подпись — ellipsis (`button_layout`, деградация видима
  G4-линту).
- Кнопка primary на светлой теме: тот же слот (не дифференцирован) — I-1.
- Клик по disabled: ничего не происходит, press не армит.

## 7. Где в коде
`crates/canvas-ui/src/component/button.rs` — button_size/button_layout/
button_style (79–132), IconButton (137–149), Switch (240–291), глифы Icon
(321–351), stage_close_button (370); константы BUTTON_*/ICON_BUTTON_SIZE —
`crates/canvas-ui/src/component/mod.rs` (414–426); состояние и клик-ребро —
`crates/canvas-ui/src/widget.rs` (WidgetState). `kit.rs` — тонкий фасад.
Потребители: settings_ui, whatif_ui, scheme_gallery_ui, dialog (confirm).

## 8. Что меняется при правке файла
- Размеры/паддинг → константы `component/mod.rs` + при смене кегля шкала
  `../rules/02-typography.md`.
- Цвета состояний → слоты ThemeColors/KitPalette (не локальные); прогон G3.
- Новое состояние → новый слот + правка `../rules/08-states.md`.
