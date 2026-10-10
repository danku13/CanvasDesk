# Чипы и бейджи (Chip / Badge)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/chip.rs` и
> `crates/canvas-ui/src/component/badge.rs`.
> Актуализировано: 2026-10-10 (волны T/C/L, issue #47).

## 1. Назначение
- **Чип** — компактный интерактивный маркер-фильтр/тег: чипы категорий
  галереи схем, чипы what-if сценариев в баре, чипы фильтров палитры.
- **Бейдж** — неинтерактивный индикатор: severity-бейджи строк таблицы,
  дельта-бейджи what-if, пилюли FR-069; кит-бейдж Wave C — счётчик/текст/точка.

## 2. Анатомия и размеры
| Элемент | Значение |
|---|---|
| Чип высота | CHIP_HEIGHT 24 |
| Чип гориз. паддинг | CHIP_PAD_H 8 |
| Чип радиус | RADIUS_CHIP 6 (M1: галерейные чипы — kit::chip_strip; «пилюли RADIUS_PILL 12» — v1, не пересматривалось) |
| Кит-бейдж (Wave C) | Dot 8×8; Count/Text — pill h 16 (мин), w = текст+8 (мин 16), радиус Shape::Full; угол якоря — верхне-правый |
| Чип what-if | шрифт 13 (CHIP_FONT), паддинг 12 (CHIP_PAD_X), высота 24 (kit::CHIP_HEIGHT; ранее 26) |

## 3. Токены
| Случай | Слоты |
|---|---|
| Чип фильтра normal/hover/selected | control_fill / hover_fill / selected_fill; рамка control_border; текст text (disabled — disabled_text) |
| Кит-бейдж (BadgeTone) | Default→control_fill/text; Primary→control_primary; Success→control_success; Warning→control_warning; Danger→control_danger; Info→accent; остальные — text_title; без рамки |
| What-if чип активный | WHATIF_CHIP [0.30,0.55,0.95,1] |
| What-if чип приглушённый | WHATIF_CHIP_DIM #242A33 |
| Дельта-бейдж | WHATIF_BADGE #DFA63E |
| Severity-бейджи | таблицы severity.dark/light (colors.json ↔ tokens.rs SEVERITY_*, паритет I-5) + text_none [154,154,162] |
| Результат-бейджи пилюли | BadgeTone Spill/Delta/Error (canvas-render/row_grid.rs, FR-069; капсула h/2, шрифт badge 10/12) |

## 4. Состояния
Чип — контрол (полная матрица ST1; полоса чипов — chip_strip, зазор
CHIP_GAP = SPACING_S). Бейдж — decoration (не пикется), кроме бейджа ошибки
строки: hover-зона (+10 px, LINE_ERROR_HIT_PAD_PX) с тултипом.

## 5. Взаимодействие
- Чип фильтра: клик — переключение выбора (одиночный выбор в галерее,
  множественный — в палитре); hover — слот.
- What-if чип: клик — выбор активного сценария; ✕ (CLOSE_WIDTH 28) — удаление
  сценария (через диалог confirm, danger-семантика).
- Драг what-if чипа между «сценариями» не поддерживается v1.
- Бейдж ошибки строки: hover — тултип с текстом ошибки (500 мс).

## 6. Граничные случаи
- Длинный текст чипа: ширина из TextMeasurer; переполнение ловится линтом G4
  (не маскируется); для what-if чипов — обрезка ellipsis по max-ширине бара.
- Прогон зебры таблицы: бейджи не влияют на ширину колонок — колонки задаются
  RowGuides по всем строкам ноды (max), бейдж-колонка = текст +
  2·ROW_BADGE_PAD_H (6).
- Много чипов в баре: ряд Wrap (template_ui, max_rows 2), высота бара
  растёт (BAR_HEIGHT 44 базово).

## 7. Где в коде
`crates/canvas-ui/src/component/chip.rs` — chip_strip/chip_strip_wrap
(Fit/SqueezeTail/Wrap, CHIP_FONT_SIZE 12, CHIP_GAP);
`crates/canvas-ui/src/component/button.rs` — chip_size/chip_layout/chip_style;
константы CHIP_HEIGHT/CHIP_PAD_H — `crates/canvas-ui/src/component/mod.rs`
(428–430); кит-бейдж Wave C — `crates/canvas-ui/src/component/badge.rs`;
бейджи строк — `crates/canvas-ui/src/component/row.rs` (ROW_BADGE_PAD_H 6)
+ `crates/canvas-render/src/row_grid.rs` (FR-069). What-if чипы —
`whatif_ui.rs` (CLOSE_WIDTH 28, LIST_WIDTH 480); галерейные —
`scheme_gallery_ui.rs` (kit::chip_strip; M1: CHIP_W/CHIP_ALL_W удалены).

## 8. Что меняется при правке файла
- Высоты/паддинги → константы соответствующих модулей (единый прогон линтов
  G4 по трём вьюпортам обязателен).
- Цвета → слоты; severity-таблицы → tokens.rs + colors.json (паритет I-5);
  новый тон кит-бейджа → BadgeTone + badge_style + правка K2.
