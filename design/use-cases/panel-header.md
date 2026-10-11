# Шапка панели (PanelHeader)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/panel_header.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Полоса заголовка панели с рядом выровненных кнопок (закрыть/тема/сброс/…):
галерея схем (`gallery_layout`), админ-консоль. Закрывает TODO K3/K4
FR-UI-PANEL-HEADER: `stage_close_button` ставит крест одиночно в углу, а
широкие toggle-кнопки должны стоять на одной y с крестом. Выбор — таблица K1
(`../rules/10-components.md`): заголовок панели — PanelHeader; контейнер —
Panel (`../use-cases/panel.md`); одиночная кнопка без ряда — Button
(`../use-cases/button.md`). Не плодить параллельные решения (K1.2).

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Высота полосы (kit-функция) | BUTTON_HEIGHT 30 — S-вариант шкалы S3 |
| Высоты S/M/L (выбор потребителя) | PANEL_HEADER_H_S/M/L 30/38/44 (tokens panel_header) |
| Inset справа | SPACING_SM 8 |
| Зазор кнопок | GAP_CONTROLS 8 |
| Icon-кнопка | ICON_BUTTON_SIZE 26×26, центр в полосе 30 (отступ 2) |
| Toggle-кнопка | label_w × BUTTON_HEIGHT (label_w меряет потребитель) |
| Сепаратор | линия 1px на `slot.y + BUTTON_HEIGHT` |

Живые отклонения от S/M/L (TODO W-d, миграция — волна W1 аудита):
scheme_gallery 40→38, autolink 58→44; до миграции значение кода приоритетно
(I-1). explain уже на шкале: HEADER_H = PANEL_HEADER_H_L (LAY-W7).

## 3. Токены (слоты KitPalette)
| Элемент | Слот |
|---|---|
| Сепаратор шапки | panel_border (та же рамка панели) |
| Кнопки Icon/Toggle | `button_style` потребителя — слоты состояний |

Кит не выбирает цвет/текст — только геометрия (F-8). Глиф иконки —
`icon_glyph` по `IconKind` (Close/Settings/Search/Custom; Custom —
потребитель рисует сам через Painter::icon). Фон шапки кит не рисует.

## 4. Состояния
Кнопки шапки — контролы с полной матрицей ST1 (Disabled > Error > Pressed >
Hovered > Focused > Dragged > Selected > Normal, `../rules/08-states.md`);
стиль — `button_style` (вариант
+ состояние) на потребителе. Полоса и сепаратор — не контролы (ST6): без
hover, как hit-цели не выбираются.

## 5. Взаимодействие
- **Порядок кнопок**: выстраиваются справа-налево от `slot.right() −
  SPACING_SM` с зазором GAP_CONTROLS; выдача — left-to-right с id (реверс
  потребителя — 1 строка).
- **Клики**: rect → обработчик по стабильному строковому id (как
  `kit_ui::HitSlot`); сам клик-контракт — ST3 кнопки
  (`../use-cases/button.md`).
- **Хит-зона**: Icon 26×26 уже полосы 30 — rect кнопки и есть hit; расширение
  зоны (hit ≥ визуал, A3) — решение потребителя.
- **Esc**: крест шапки — не часть Esc-стека; Esc-семантику ведёт поверхность
  (`../rules/05-layering.md`).

## 6. Граничные случаи
- Переполнение НЕ маскируется: сумма ширин + зазоров + inset'ов > slot.w —
  левые кнопки выходят за slot.x (ловит G4; потребитель обязан дать ширину).
- Пустой список кнопок валиден: rect шапки + сепаратор (без кнопок).
- Icon в полосе 30: вертикальный отступ (30−26)/2 = 2 — визуальный центр
  квадрата в полосе BUTTON_HEIGHT.
- Toggle без замера недопустим: label_w даёт `button_size` (замер
  TextMeasurer), не константа.

## 7. Где в коде
`crates/canvas-ui/src/component/panel_header.rs` — `panel_header`/
`paint_panel_separator`/`HeaderButton`/`IconKind`/`PanelHeaderLayout`/
`PanelHeaderStyle`; ре-экспорт — `crates/canvas-ui/src/kit.rs`. Потребители:
`crates/canvas-app/src/kit_ui.rs` (шапка витрины: `gallery_layout` и
hit-слоты реестра — один вызов даёт кнопки обоим), `crates/canvas-app/src/admin_ui.rs`
(`admin_layout`/`admin_hit_slots`). Токен-источник высот —
`crates/canvas-core/src/tokens.rs` (PANEL_HEADER_H_*). Витрина —
`crates/canvas-app/src/kit_ui.rs`: живой образец — хром самой панели галереи
(K4: section panel_header).

## 8. Что меняется при правке файла
- Высоты S/M/L → `crates/canvas-core/src/tokens.rs` + S3
  `../rules/03-spacing-radius.md` (I-5 паритет tokens.dimensions.json).
- Inset/зазор → SPACING_SM/GAP_CONTROLS (S1); смена схемы выстраивания —
  канон kit_ui/admin_ui (I-1: совпадение слотов hit и отрисовки).
- Стиль/поведение кнопок → `../use-cases/button.md`; сепаратор — слот
  panel_border (K2, без новых цветов — K2.1).
- Новая разновидность шапки/кнопки — строка K1 и секция K4
  `../rules/10-components.md` (без секции — незакрытая волна).
