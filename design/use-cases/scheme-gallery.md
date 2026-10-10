# Галерея схем + Empty state (Scheme Gallery)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-app/src/scheme_gallery_ui.rs`,
> `crates/canvas-app/src/app/overlays.rs`,
> `crates/canvas-app/src/app/ui_registry.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Модальный каталог готовых схем (эталонов): фильтр по категориям, поиск,
выбор → открытие сцены. При пустом канвасе — карточка empty state с быстрыми
действиями.

## 2. Анатомия и размеры (screen px)
| Параметр | Значение |
|---|---|
| Ширина панели | PANEL_W 660 (аудит CI 2026-09-29: 5 категорий — ряд чипов измеренный; было 560) |
| Хедер | HEADER_H 40 |
| Поле поиска | INPUT_H 34 |
| Чипы категорий | CHIP_H = kit::CHIP_HEIGHT 24 (M1 FR-UI-CHIP-STRIP; было 28); ширина — измеренная chip_strip (литералы 108/56 удалены) |
| Строка схемы | ROW_H 62 (внутр. 56; было 56/50) |
| Футер | FOOTER_H 26 |
| Паддинг панели | PANEL_PAD 12 (SPACING_LG) |
| Шрифты | строки 13 (FONT_BODY), описание 11 (FONT_CAPTION) |
| Отступ панели ↔ вьюпорт | SPACING_XL 24 |
| Радиус | RADIUS_PANEL 10; чипы — пилюли (RADIUS_PILL) |
| Empty-кнопки | высота 34 (EMPTY_BTN_H), радиус 6 |

## 3. Токены
Панель — канонический стиль модали кита: panel_fill + panel_border, радиус
RADIUS_PANEL (CR-031/S5; раньше menu_fill/palette_border как у попапа); чипы
— palette_chip_fill/активная palette_selected_fill; строки — прозрачные на
подложке панели (panel_fill), hover — control_hover_fill, selected —
control_selected_fill. Empty state: карточка menu_fill + palette_border,
радиус 10, title 16, тело 12; primary-кнопка DIALOG_BUTTON_PRIMARY (hover
control_primary_hover_fill), secondary menu_fill (hover control_hover_fill).

## 4. Состояния
Закрыта / открыта; строка: Normal/Hovered/Selected; фильтры: активная
категория; пустой результат поиска — empty-строка. Слой **Modals (L5)**,
capture = **Block** (клик мимо — закрытие).

## 5. Взаимодействие
- **Открытие**: empty state («Открыть галерею») / меню.
- **Фильтры**: клик чипа — выбор категории (одиночный); поле — живой поиск.
- **Выбор схемы**: клик строки / Enter; ↑/↓ — навигация; открывается сцена
  (замена текущей — через confirm, если есть несохранённые изменения).
- **Клавиатура**: ↑/↓ — выбор, Enter — открыть, Esc — закрыть; printable-
  символы сразу попадают в поле поиска (панель — keyboard-scope).
- **Скролл**: видимое окно строк, скроллбар 4 px; скролл целыми строками.

## 6. Граничные случаи
- Панель клампится к вьюпорту (инвариант 320×240 минимум видимого окна).
- Длинные названия/описания: ellipsis по фактической ширине строки (13/11),
  полное — в тултипе строки.
- Пустая категория: empty-строка в списке, чип остаётся выбранным.
- Открытие поверх what-if режима: галерея Block глотает ввод; what-if бар
  скрывается под димом (L3 < L5).

## 7. Где в коде
`crates/canvas-app/src/scheme_gallery_ui.rs` (8–10 клавиатурный контракт, 37–84
константы), empty state — `crates/canvas-app/src/app/overlays.rs`
(empty_state_overlay 2249), гейт/клик — `crates/canvas-app/src/app.rs`
(empty_state_visible 6127, click_empty_state 8629), каталог — ADR-0006
эталоны, surface — `crates/canvas-app/src/app/ui_registry.rs`
(GALLERY: Modals/Block).

## 8. Что меняется при правке файла
Размеры → scheme_gallery_ui.rs. Поведение клавиатуры — контракт клавиш
панели (синк с user-docs). Цвета — слоты (palette_*/control_*).
