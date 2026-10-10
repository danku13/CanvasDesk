# Пузырь чата (ChatBubble)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/chat_bubble.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Сообщение агента в чате (FR-UI-CHAT): пузырь Normal/Error/Success со строками
tool-calls. Правило выбора — K1.2 (`../rules/10-components.md`): сообщение
агента — ChatBubble (не Banner, не Toast). Извлекает паттерн
`agent_panel.rs` (бабл бота: kind-семантика, строки tool_calls, подпись
отправителя «AI Агент»).

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Пузырь | rect = весь слот; ширина = slot.w |
| Внутренний пад | SPACING_SM 8 (шкала S1) |
| Строка текста | CHAT_LINE_H 14 (кегль 11 × 1.3 → 14; источник agent_panel.rs) |
| Строка tool_call | CHAT_TOOL_CALL_H = CHAT_LINE_H 14 |
| Header-строка | CHAT_HEADER_H 12 (кегль 9 × 1.3 ≈ 11.7 → 12; канон Q2) |
| Header: смещение | y = slot.y + SPACING_SM·0.5 (историческое `msg_y + 4.0`); x/w — text_area-выравнены |
| text_area | y = header.bottom() (без доп. зазора) либо slot.y + пад; высота = n_lines · CHAT_LINE_H |
| Строки tool_call | ниже text_area с зазором SPACING_S 6, каждая высотой CHAT_TOOL_CALL_H |
| Радиус | RADIUS_PANEL 10 |

## 3. Токены (слоты KitPalette)
| ChatBubbleKind | Заливка | Рамка | Текст |
|---|---|---|---|
| Normal | panel_fill | panel_border | text |
| Error | tint(control_danger, 0.08) | tint(control_danger, 0.38) | control_danger |
| Success | tint(control_success, 0.08) | tint(control_success, 0.38) | control_success |

Строки tool_call — приглушённо: text_muted (рисует потребитель).
Alpha-tint — исключение П4/C8 (`../rules/01-colors.md`): `paint::tint` меняет
только альфу слота константами шагов C2 — 0.08/0.38 (0.38 введён аудитом
2026-10-09 как реально используемый, источник agent_panel.rs); rgb не
трогается, запись в K2.2.

## 4. Состояния
Пузырь — не контрол (ST6, `../rules/08-states.md`): нет hover/нажатий,
не пикется (`HitRect::decoration`). ChatBubbleKind — семантика (Normal/
Error/Success), выбирающая слоты, а не состояние матрицы ST1. Матрица ST1
применима только к контролам, которые потребитель разместит внутри пузыря.

## 5. Взаимодействие
- Кит рисует только фон: `paint_chat_bubble` → 1 `PaintItem::Rect`; текст
  и tool_call-строки — потребитель (`Painter::label` со `style.text_color` /
  text_muted, замер `TextMeasurer`).
- Пузырь не кликабелен и Esc не перехватывает (пассивный контейнер);
  выделение/копирование сообщений — v2, вне контракта v1.

## 6. Граничные случаи
- Высота слота — обязанность потребителя: кит НЕ маскирует переполнение
  (нужно n_lines·CHAT_LINE_H + header + tool_calls + зазоры; деградацию
  ловит G4 `../rules/00-principles.md`).
- Узкая ширина: text_area.w = slot.w − 2·SPACING_SM, клампится в 0.
- n_tool_calls = 0 → пустой Vec tool_call_rows; header/text при этом валидны.
- Перенос длинного текста — потребитель: число строк n_lines подаётся
  параметром, пузырь раскладывает строки по CHAT_LINE_H.

## 7. Где в коде
`crates/canvas-ui/src/component/chat_bubble.rs` — `chat_bubble`,
`paint_chat_bubble`, `ChatBubbleKind`, `ChatBubbleLayout`/`ChatBubbleStyle`,
`CHAT_LINE_H`/`CHAT_TOOL_CALL_H`/`CHAT_HEADER_H`; re-export —
`crates/canvas-ui/src/kit.rs`. Потребитель:
`crates/canvas-app/src/app/agent_panel.rs` (AgentMsgKind → ChatBubbleKind,
рендер бабла бота, Q2). Секции ChatBubble в витрине
`crates/canvas-app/src/kit_ui.rs` нет — бэклог W2 (K4).

## 8. Что меняется при правке файла
- Метрики строк/пады → константы модуля + паритет agent_panel (шкала S1
  `../rules/03-spacing-radius.md`); CHAT_HEADER_H 12 — канон Q2, не откат к 14.
- Цвета/альфы → слоты K2 + шаги C2 (K2.1/K2.2); новый kind → строка K1
  и слоты (П4), не формула.
- Юнит-тесты модуля (`kind_error_uses_danger_slot_tint`,
  `header_area_stacks_above_text_area`, `paint_emits_single_background_rect`)
  — замок контракта.
