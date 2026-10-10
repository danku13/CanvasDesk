# Баннер (Banner)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/banner.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Статусная полоса с сообщением и кнопкой действия (FR-UI-BANNER): горизонтальный
баннер Info/Warning/Error — текст-сообщение слева, кнопка действия справа
(«Восстановить», «Retry»). Извлекает паттерн `autolink_ui.rs` (баннер
отклонённых элементов, У8) и explain-окна. Правило выбора — K1.2
(`../rules/10-components.md`): статусная полоса — Banner; параллельные
баннерные решения в поверхностях не разводить.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Высота | BANNER_H 34 (паритет `autolink_ui::BANNER_H − 8 = 34`; полоса слота autolink — 42) |
| Внутренний пад | BANNER_PAD = SPACING_SM 8 (шкала S1) |
| Радиус | BANNER_RADIUS = RADIUS_PANEL 10 (шкала S2) |
| Строка подписи | BANNER_LABEL_LINE_H 16 (паритет explain.rs `height = 16.0`) |
| Кнопка действия | высота BUTTON_HEIGHT 30; ширина = action_label_w + 2·BUTTON_PAD_H 12 |
| Привязка кнопки | правый край = slot.right(), вертикальный центр слота |
| Подпись | x/y = slot + BANNER_PAD; ширина = min(замер, остаток до кнопки − BANNER_PAD); зазор до кнопки = BANNER_PAD |
| Tint фона | BANNER_TINT_ALPHA 0.10 (шаг C2) |

## 3. Токены (слоты KitPalette)
| BannerKind | Заливка | Рамка | Цвет подписи |
|---|---|---|---|
| Info | tint(control_fill, 0.10) | control_fill | text |
| Warning | tint(control_warning, 0.10) | control_warning | control_warning |
| Error | tint(control_danger, 0.10) | control_danger | control_danger |

Подпись кнопки: text_title; при KitState::Disabled — disabled_text.
Alpha-tint — единственное исключение из П4 (слот-only kit,
`../rules/00-principles.md`): `paint::tint` меняет ТОЛЬКО альфа-канал слота
константой из шагов C2 (0.10), rgb не трогается — легализовано C8
(`../rules/01-colors.md`), запись в K2.2.

## 4. Состояния
Сам баннер — не контрол (ST6, `../rules/08-states.md`): у полосы нет hover
и нажатий. Интерактивен только action button — полная матрица ST1 (Disabled >
Pressed > Hovered > Selected > Normal), стиль через `kit::button_style`
(Secondary/Ghost; приоритеты — ST2). BannerKind — не состояние ST1, а
семантика: kind выбирает слот по таблице §3; новый kind = новая пара
«слот + константа альфы» (П4), не формула. state в `banner()` влияет только
на цвет подписи кнопки (Disabled → disabled_text).

## 5. Взаимодействие
- Кит рисует только фон: `paint_banner` → ровно 1 `PaintItem::Rect` (паритет
  `paint_chat_bubble`). Подпись и кнопка — потребитель: `Painter::label` с
  `style.label_color` и `kit::button_style` + `Painter::control` для кнопки.
- Клик по кнопке — контракт ST3 (`clicked = released_inside`); hover — смена
  слота кнопки (ST2); Disabled не «заряжает» клик.
- Кнопка участвует в FocusRing поверхности (см. `../use-cases/button.md`);
  Esc баннер не перехватывает (Esc-стек, `../rules/05-layering.md`).

## 6. Граничные случаи
- Узкий слот: кнопка всё равно прижата к правому краю и рисуется (деградация
  видна), label_area клипается до 0 — подпись не наезжает на кнопку.
  Переполнение не маскируется — ловит G4 (`../rules/00-principles.md`).
- Длинный текст: ширина подписи = min(замер, доступная ширина); усечение
  (ellipsis) — рендер/потребитель, кит геометрию не меняет.
- Замер — обязанность потребителя (API чистый): label_w/action_label_w
  приходят параметрами (`TextMeasurer::width_of`).

## 7. Где в коде
`crates/canvas-ui/src/component/banner.rs` — `banner`, `paint_banner`,
`BannerKind`, `BannerLayout`/`BannerStyle`, константы `BANNER_*`; re-export —
`crates/canvas-ui/src/kit.rs`. Потребители: `crates/canvas-app/src/autolink_ui.rs`
(`banner_layout`, Error), `crates/canvas-app/src/app/overlays.rs` (Warning),
`crates/canvas-app/src/app/explain.rs` (через `autolink_ui::banner_layout`).
Секции Banner в витрине `crates/canvas-app/src/kit_ui.rs` нет — бэклог W2 (K4).

## 8. Что меняется при правке файла
- Высоты/пады/радиус → константы модуля + паритет `autolink_ui`/`explain.rs`
  (шкалы S1/S2 `../rules/03-spacing-radius.md`); полоса 42 px — геометрия потребителя.
- Цвета → слоты K2 (K2.1: KitPalette + мост TH6 + таблица K2 одновременно);
  новая альфа → шаги C2 + C8-исключение.
- Юнит-тесты модуля (`kind_changes_style_slots`,
  `narrow_slot_label_clipped_but_button_drawn`) — замок контракта; секция
  витрины — в той же волне, что код (K1.1/K4).
