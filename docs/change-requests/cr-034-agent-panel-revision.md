# CR-034 — Ревизия агент-панели Ctrl+I: фокус поля, иконки AI-поверхностей, вёрстка по киту

- **Статус:** реализовано
- **Дата:** 2026-10-09
- **Источник:** UR-005 (сообщение владельца + скриншот)
- **Скоуп:** canvas-app (agent_panel, ai_status_panel, input, handler),
  canvas-render (icon_data/icon_pipeline/text), assets (icons, fonts), scripts.

## Постановка

Агент-панель (Ctrl+I) и статусная панель AI отрисовывали неактивное поле
ввода, отсутствующие иконки (send/zap/pause/play — тофу) и нарушали
design/rules (шкалы S1–S3, П5/П6, A4). Ввод не принимал пробелы, выделения
не существовало. Требуется полная ревизия поверхности на ките.

## Решение

1. **Иконки (двухслойно):**
   - SVG-атлас: имена `send`, `zap`, `pause`, `play` в 4 UI-наборах
     (lucide/feather — stroke, material/bootstrap — fill; официальные пути
     библиотек с пермиссивными лицензиями). Реестр 39→43 имён, атлас
     1376×160, растеризация build-time (ADR-0011 — байты вшиты).
   - Glyph-фолбэк: сабсет «CanvasDesk Symbols» расширен ⚡ (U+26A1),
     ➤ (U+27A4), ⏸ (U+23F8), ▶ (U+25B6) — оригинальные полигональные
     силуэты в стиле соседних глифов (▲ 805×697 @ adv 907); генератор
     `scripts/extend_symbol_font.py` (fontTools, идемпотентен).
   - Дрейн: обе AI-панели отдают `Vec<IconInstance>` третьим элементом,
     handler расширяет `icon_instances` (иконки KitDraw больше не теряются).
2. **Поле ввода — состояние фокуса (ST/A4):** `AgentState.input_focused`
   (+ kit `TextFieldModel` вместо `input: String`/`caret: usize`-байтов).
   Фокус: рамка слота `accent` + кольцо params.y=1, каретка 1.5px accent,
   выделение — подложка accent α0.25. busy → Disabled-слот текста.
   Потеря/возврат фокуса: клик мимо панели ↔ клик по Input; Ctrl+I — фокус
   при открытии.
3. **Клавиатура:** Space (Named(Space) — регрессия winit), стрелки ±1 с
   Shift-расширением, Home/End, Backspace/Delete (sel-aware), Ctrl+A/C/X/V
   (латиница/кириллица/control-коды), Enter — отправка, Esc — закрыть.
   Без фокуса ветка пропускает клавиши канвасу. Тот же Space-фикс в поиске.
4. **Вёрстка по киту (S1–S3, П5/П6):**
   - `AgentPanelLayout::build` — единый источник rect'ов close/input/send/
     quick для draw и hit (урок CR-033);
   - высоты: поле TEXT_FIELD_HEIGHT(30), кнопки ICON_BUTTON_SIZE(26),
     чипы/пилюли CHIP_HEIGHT(24); паддинги SPACING_LG/SM/S; радиусы —
     RADIUS_CHIP/RADIUS_PILL;
   - чипы контекста: замер ширин шрифтом (+2·SPACING_SM пад), короткая
     подпись провайдера, ellipsis у context-чипа (молчаливая обрезка
     запрещена);
   - empty-state: `TextMeasurer::wrap` — перенос по реальной ширине.
5. **Минимапа × AI-статус-панель:** гейт UR-003 расширен — минимапа скрыта
   также при видимой статус-панели (viewport-рамка minimap-pass рисовалась
   поверх панели; класс наложения CR-033 №4).

## Совместимость/риски

- Публичный API `AgentState` (`input`, `caret`) удалён — единственный
  потребитель `canvas-app` (поле `field` + хелперы `input_text`/`clear_input`).
- Атлас +4 колонки — `ICONS_PER_ROW`/`ATLAS_W` производные; шейдеры/GPU-путь
  без изменений (только UV-арифметика от реестра).
- Поведение «Enter при провайдере Off сохраняет черновик» — без изменений
  (legacy ранний выход до push User-сообщения).

## Гейты

fmt; check --workspace; test --workspace (2756, 0 failed); clippy
--all-targets -D warnings; wasm_gate --check; headless-верификация
(ввод/выделение/фокус/иконки) — см. UR-005.
