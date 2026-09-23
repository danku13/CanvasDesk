# FR-060: Миграция волна 2 — autolink_ui, palette.rs, explain_ui + хвосты app.rs (dialog/menu/hotkeys) + финальный замер wasm

- **Статус:** выявлено (постановка волны 2 кита; реализация по приказу владельца)
- **Тип:** FR
- **Приоритет:** важно
- **Владелец:** не назначен (любой агент — права на файлы см. отдельный раздел)
- **Источник:** гэп-анализ кита (сессия 2026-09-23); PRD-0009 §13 (G8-стиль аудит), §15 G5
- **Связанные задачи:** PRD-0009; FR-057/FR-058 (ядро и компоненты v2), FR-059 (волна 1 — строгий порядок), FR-048 (explain), FR-050 (flowmap — соседний паттерн), FR-009/FR-010 (palette)
- **Создан:** 2026-09-23
- **Обновлён:** 2026-09-23

## Описание
Последние hand-rolled поверхности интерфейса: `autolink_ui.rs` (494 строки — диалог ревью автосвязи), `palette.rs` (1869 — плавающий тулбар выделения), `explain_ui.rs` (1751 — окно проверки цепочки расчёта) плюс хвосты app.rs: `dialog_button_rects` (15671 — диалог подтверждения, фиксированные геометрии), `menu_open_rect` (16391 — главное меню), hotkeys-панель. Перенос на кит (Modal/Panel/Card/список/IconButton+Icon/Chip/Tooltip) завершает перевод кастомного UI; фиксируется финальный замер wasm. Выявлено агентом.

## Non-goals (фиксация, чтобы не раздувать постановку)
- **onboarding_ui.rs — НЕ трогать** (решение владельца: без user-visible изменений доработок онбординга нет).
- `wheel_overlay`, `minimap`, `HUD` — world-декорации/деградация, отдельное решение (не панельный UI).
- search/settings/docs/template/whatif/галерея схем — уже на примитивах U5; их перевод с примитивов на виджеты кита **опционален и не требуется**.
- Slider — не существует в ките (non-goal FR-058); экранных потребителей нет.

## Влияние
| Объект | Что меняется | Где в документации |
|---|---|---|
| `autolink` (диалог ревью) | диалог/список групп → Modal/Panel + список | FR-048 (X4), PRD-0009 G5 |
| `palette` (тулбар выделения) | кнопки/квады → Panel + IconButton/Icon + Chip + Tooltip | FR-009/FR-010, CR-011, PRD-0009 G5 |
| `explain` (окно цепочки) | секции/строки/пилюли → Panel/Card + список + Tooltip | FR-048 (X2), PRD-0009 G5 |
| `app.rs` хвосты | dialog → kit::modal + button_layout; menu → dropdown_menu; hotkeys → panel_rect + Column | PRD-0009 §15 G5/G8 |
| Замер wasm | финальный (кумулятивно волна 2) | G7 PRD-0009 |

## Анализ
- `crates/canvas-app/src/palette.rs` (1869 строк) — плавающий тулбар: ручные кнопки-квады, ховеры, клампы к краю; самый крупный hand-rolled модуль.
- `crates/canvas-app/src/explain_ui.rs` (1751) — ручные секции/строки/пилюли с hover; паттерн «чистая модель + app.rs-рендер» (шапка модуля).
- `crates/canvas-app/src/autolink_ui.rs` (494) — диалог со списком групп «исток → приёмник» и ручными состояниями строк.
- `app.rs:15671` `dialog_button_rects` — `[[f32; 4]; 2]` фиксированной геометрии (класс дефекта «диалог не адаптируется под текст» — PRD-0009 §2); `app.rs:16391` `menu_open_rect` — пункты меню вручную.
- Контракт миграции — как FR-059: числа дословно, замены только там, где устраняется эвристика; hit-rect'ы реестра не трогаются (G1/G2).

## Требуемые изменения
| Модуль | Было (hand-rolled) | Стало (кит) |
|---|---|---|
| `autolink_ui.rs` | ручной диалог + группы строк + состояния | `kit::modal`/`panel_rect` + `list_rows` + `WidgetState`, Painter |
| `palette.rs` | ручные кнопки-квады/ховеры/клампы | `kit::panel_rect` + `icon_button`/`Icon` + `chip` + `tooltip` (delay), Painter |
| `explain_ui.rs` | ручные секции/строки/пилюли | `kit::panel_rect`/`card` + `list_rows` + `tooltip`, Painter |
| `app.rs` dialog | `dialog_button_rects` — фиксированные 2 кнопки | `kit::modal` + `button_layout` (Primary/Danger) — геометрия от измеренного текста |
| `app.rs` menu | `menu_open_rect` — ручные пункты | `kit::dropdown_menu` + строки списком |
| `app.rs` hotkeys | ручная панель | `kit::panel_rect` + Column |
| Замер wasm | — | финальный прогон гейтов + размер бина (кумулятив волны 2) в worklog |

## Права на файлы (для параллельных агентов)
- **Вправе править:** `crates/canvas-app/src/autolink_ui.rs`, `palette.rs`, `explain_ui.rs`, `app.rs` — **только** функции `dialog_button_rects`, `menu_open_rect`, hotkeys-функция, их ветки в `on_left_button`/`on_key`/Esc-лестнице; перечень зафиксировать в worklog при старте; `docs/ui-kit.md` §8 (статус кита).
- **Запрещено трогать:** `crates/canvas-ui/**` (только использование), `crates/canvas-render/**`, модули волны 1 (`hints_ui.rs`/`flowmap_ui.rs`/`calc_panel_ui.rs`/`kit_ui.rs`), `onboarding_ui.rs` (заморожен владельцем), `settings_ui.rs`/`template_ui.rs`/`docs_ui.rs`/`whatif_ui.rs`/`scheme_gallery_ui.rs`.

## Зависимости
| Отношение | С кем |
|---|---|
| Кодировать | после слияния FR-059 (общий `app.rs` — строгая последовательность) |
| Сливать | последним в волне; финальный замер G7 — на этом FR |
| Независим | от FR-056 к моменту старта (к этому шагу все слиты) |

## Точки входа
- `docs/ui-kit.md` §8 — статус кита (что перенесено, что сознательно осталось).
- `docs/prd/prd-0009-ui-layering-uikit.md` — §16 (история), §15 (G5/G7/G8 — финальный аудит).
- `worklog.md` — запись реализации + сводка «что осталось hand-rolled» (ожидаемо: wheel/minimap/HUD/onboarding — сознательно).

## Проверка
- G4-линт-матрица (3 окна × RU/EN) — 0 налезаний/переполнений/выходов на перенесённых поверхностях.
- G5-аудит: 0 `take(`/break-клампов/`truncate_chars` в перенесённых модулях; grep-аудит отсутствия литеральной rect-математики (стиль G8).
- Диалог подтверждения адаптируется под измеренный текст (фикс класса дефекта h=150).
- 0 визуального скачка (числа дословно, кроме устранения эвристик); headless-тесты pick/hit/клавиатуры поверхностей зелёные.
- Все гейты (fmt/clippy -D warnings/test/wasm/mcp-wasm) зелёные; финальный замер wasm записан (кумулятив волны ≤ 100 КБ сверх baseline FR-056).

## История изменений
- `2026-09-23` — агент: создан документ (постановка волны 2 кита, FR-060), статус «выявлено»; non-goals зафиксированы (onboarding заморожен, wheel/minimap/HUD — вне скоупа).

## Источники истины
- `crates/canvas-app/src/palette.rs` (1869), `explain_ui.rs` (1751), `autolink_ui.rs` (494) — шапки модулей.
- `crates/canvas-app/src/app.rs` — `dialog_button_rects` (15671), `menu_open_rect` (16391).
- `docs/change-requests/fr-054-ui-layering-u5-migration.md` — паттерн миграции.
- `docs/prd/prd-0009-ui-layering-uikit.md` — §2 (класс дефектов фиксированных геометрий), §15 G5/G7/G8.
