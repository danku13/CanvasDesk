# Две колонки (TwoColumn)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/two_column.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Раскладка [sidebar | content]: фиксированная боковая колонка + заполняющий
контент (FR-UI-TWO-COLUMN; аудит §6.1 — MEDIUM-приоритетный kit gap).
Извлекает паттерн `calc_panel_ui`, `admin_ui`, `settings_ui`. Реестр K1
(`../rules/10-components.md`): строка TwoColumn — контейнер; анти-паттерн
K1.2 «не разводить параллельные решения» относится и к каркасам: новые
двухколоночные панели берут kit-API, а не ручную арифметику поверхностей.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Sidebar | x = slot.x, ширина sidebar_w, высота slot.h (right-вариант: right = slot.right()) |
| Content | x = slot.x + sidebar_w + gap; ширина = slot.w − sidebar_w − gap; высота slot.h |
| Инвариант | колонки не перекрываются: sidebar.right() + gap = content.x (right-вариант зеркально); content.right() = slot.right() |
| Зазор | параметр gap потребителя; типовой GAP_CONTROLS 8 (SPACING_SM; шкала S1) |
| Стиль панелей | `panel_style(palette)` потребителя (panel_fill/panel_border) |

Замечание: док-комментарий модуля называет типовой зазор ещё и «`SPACING_MD`
= 8» — по шкале S1 `SPACING_MD` = 10; расхождение комментария — на
рефакторинг, контракт считает каноном `GAP_CONTROLS` 8.

## 3. Токены (слоты KitPalette)
Контейнер собственных слотов не имеет (K1: «containers»): кит отдаёт только
геометрию — F-8. Панели колонок рисует потребитель через `panel_style`
(panel_fill, panel_border, pad SPACING_LG 12); строки sidebar — контролы
потребителя со слотами control_fill / control_hover_fill /
control_selected_fill (образец — `settings_ui`/`admin_ui`).

## 4. Состояния
TwoColumn — контейнер: состояний ST1 не имеет (`../rules/08-states.md`).
Все контролы внутри (строки списка sidebar, кнопки content) несут полную
матрицу ST1 на стороне потребителя.

## 5. Взаимодействие
- Сам не интерактивен: hit-зоны — только у содержимого колонок (ST6:
  не-контролы не пикются).
- Скролл длинного контента — модель K3 (`../rules/10-components.md`):
  `ScrollState` + `list_rows`/`scroll_bar`; собственных скроллов у
  TwoColumn нет.

## 6. Граничные случаи
- sidebar_w + gap > slot.w → content вырождается в пустой rect (w = 0, не
  отрицательный) — класс переполнения НЕ маскируется (ловит G4
  `../rules/00-principles.md`).
- Sidebar не клампится к slot.w (может быть шире слота — валидация на
  потребителе).
- Right-вариант (`two_column_right`): sidebar прижат к правому краю
  (sidebar.right() = slot.right()), content заполняет левую часть; инвариант
  неперекрытия тот же.

## 7. Где в коде
`crates/canvas-ui/src/component/two_column.rs` — `two_column`,
`two_column_right`, `TwoColumnLayout`; re-export —
`crates/canvas-ui/src/kit.rs`. Прямых вызовов в `canvas-app` пока нет:
поверхности (`calc_panel_ui`, `admin_ui`, `settings_ui`) воспроизводят
паттерн вручную — миграция на kit-API задача рефакторинга (K1.2). Витрина: секция TwoColumn в `crates/canvas-app/src/kit_sections_k4.rs`
(K4) — две строки sidebar+content; раскладочный паттерн, ST1-состояния
панелей не дифференцирует.

## 8. Что меняется при правке файла
- Геометрия/инварианты → модуль + юнит-тесты
  (`left_sidebar_layout_no_overlap_correct_widths`,
  `overflow_content_degenerates_to_empty_rect`).
- Новые раскладочные механизмы (flex, percent, minmax, sticky) — через
  `../rules/11-layouts.md` (LAY3–LAY5), не в этот компонент.
- Миграция потребителей — той же волной, что правка строки K1 (K1.1);
  зазоры — только шкала S1 `../rules/03-spacing-radius.md` (G5).
