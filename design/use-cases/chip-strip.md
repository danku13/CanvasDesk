# Полоса чипов (ChipStrip)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/chip.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Ряд одноразмерных чипов-фильтров (FR-UI-CHIP-STRIP): категории галереи схем,
preset-чипы настроек, чипы категорий шаблонов. Извлекает паттерн
`scheme_gallery_ui`/`template_ui`/`settings_ui`. Правило выбора — K1.2
(`../rules/10-components.md`): полоса чипов — ChipStrip (Row из Fit-чипов
с опциональным preset «Все»), параллельные барные решения не разводить.
Одиночный чип и бейджи — `../use-cases/chip-badge.md`; этот файл — только
ChipStrip.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Чип | измеренный текст + 2·CHIP_PAD_H 8 (бит-в-бит `chip_size`) |
| Высота чипа | CHIP_HEIGHT 24 (дизайнерская константа, не измеренная: 12·1.3 ≈ 15.6) |
| Зазор | CHIP_GAP = SPACING_S 6 (шкала S1) |
| Шрифт | CHIP_FONT_SIZE 12, семья CHIP_FAMILY «Noto Sans Display» (CR-015: замер тем же лицом, что рендер) |
| Выравнивание | CrossAlign::Start — чипы от левого края слота |
| Порядок | сохранён: result[i] ↔ items[i] |

«Все»/«All» — обычный первый item: кит не различает preset, семантику задаёт
потребитель.

## 3. Токены (слоты KitPalette)
Кит отдаёт только геометрию (`Vec<(rect, label)>`) — контракт F-8; стиль
каждого чипа — `chip_style(state, palette)` потребителя: control_fill /
control_hover_fill / control_selected_fill, рамка control_border, текст text
(таблица слотов — `../use-cases/chip-badge.md`). Собственных слотов у полосы
нет — K2 не расширяется (K2.1).

## 4. Состояния
Полоса — контейнер без состояний; каждый чип — контрол полной матрицы ST1
(`../rules/08-states.md`): Disabled > Error > Pressed > Hovered > Focused >
Dragged > Selected > Normal.
Selected — активный чип (галерея — одиночный выбор, палитра — множественный).
Полоса себя не анимирует (K1.3: переходы мгновенные, длительности M1 —
потребителя).

## 5. Взаимодействие
- Клик по чипу — контракт ST3: press внутри + release внутри, один раз;
  увод курсора отменяет нажатие.
- Hover — смена слота (`chip_style`); клавиатура — FocusRing поверхности
  (Tab/Shift+Tab), как у `../use-cases/button.md`.
- Клик по «Все» — семантика потребителя (сброс фильтра; category=None →
  chip_style(Selected)).

## 6. Граничные случаи
Три именованные политики переполнения (LAY3, `../rules/11-layouts.md`):

| Режим | Политика | Поведение |
|---|---|---|
| squeeze = false | RowPolicy::Fit | переполнение НЕ маскируется — чипы выходят за правый край (ловит G4) |
| squeeze = true | RowPolicy::SqueezeTail | хвост сжимается до 0 ширины (именованно; замена молчаливого break-клампа галереи) |
| `chip_strip_wrap` (max_rows) | RowPolicy::Wrap + cap | перенос на строки 0..max_rows, шаг строки = CHIP_HEIGHT + CHIP_GAP; чипы строк ≥ max_rows дропаются |

- Wrap закрыл кейс template_ui:965 — SqueezeTail срезал «unit-economics»
  mid-text (wasm-аудит 2026-09-25): перенос на 2-ю строку вместо сжатия.
- Пустой items или max_rows = 0 → пустой Vec (no-op).

## 7. Где в коде
`crates/canvas-ui/src/component/chip.rs` — `chip_strip`, `chip_strip_wrap`,
`CHIP_FAMILY`/`CHIP_FONT_SIZE`/`CHIP_GAP`; раскладка — `Row::lay_out_measured`
(`crates/canvas-ui/src/layout.rs`); re-export — `crates/canvas-ui/src/kit.rs`.
Потребители: `crates/canvas-app/src/scheme_gallery_ui.rs` (Fit),
`crates/canvas-app/src/template_ui.rs` (Wrap, max_rows 2). Секция chips в
витрине `crates/canvas-app/src/kit_ui.rs` показывает одиночный Chip; отдельной
демонстрации политик полосы (Fit/SqueezeTail/Wrap) нет — бэклог W2 (K4).

## 8. Что меняется при правке файла
- Метрики → константы модуля + шкалы S1/S3 `../rules/03-spacing-radius.md`
  (CHIP_HEIGHT 24 — строка S3; зазор 6 — S1).
- Политики/кап строк → LAY3/LAY4 `../rules/11-layouts.md`; новый режим —
  строка в K1 той же волной, что код (K1.1).
- Цвета — только слоты K2 (K2.1); юнит-тесты модуля
  (`fit_policy_does_not_mask_overflow`, `squeeze_tail_compresses_overflow`,
  `wrap_five_chips_narrow_slot_drops_beyond_max_rows`) — замок контракта.
