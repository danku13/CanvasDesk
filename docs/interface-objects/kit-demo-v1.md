# Wave D v1 — интерактивная demo-витрина ui-kit (issue #32)

**Статус:** реализовано (v1, ожидает приёмки владельца).
**База:** `crates/canvas-app/src/kit_demo.rs` (модель состояния, TDD-ядро) +
`crates/canvas-app/src/kit_ui.rs` (раскладка/отрисовка TO-BE) + проводка
(реестр/ввод/линт/web). Контракты компонентов —
`docs/interface-objects/kit-component-contracts.md`.

## Режимы (D4)

| Режим | Поведение |
|---|---|
| **AS-IS** (по умолчанию) | Прежняя статичная витрина дословно — golden-пины `gallery_layout_ruler_golden_column_skeleton` и остальные тесты в силе; demo-поле `GalleryLayout::demo == None`. |
| **TO-BE** (тумблер в тулбаре / `?ui=demo`) | Интерактивная demo: тулбар D3/D4, сайдбар D5, секции Wave C D2, интерактивные контролы D1. |

## Тулбар (D3/D4)

AS-IS/TO-BE · Тема (цикл пресетов `canvas_core::theme_presets::PRESETS` —
применяется к приложению, `settings.theme_preset`) · RU/EN (оверрайд языка
демо) · Размер (Xs→Sm→Md→Lg цикл) · Плотность (Compact→Comfortable→Spacious).

## Навигация (D5)

Левый сайдбар: группы **Inputs / Navigation / Containers / Data display /
Feedback** (реестр `kit_demo::SECTIONS`, 26 секций), клик — скролл к секции
(`DemoLayout::section_offsets` — базовые y заголовков до сдвига FR-059).
Секции layout-инженерии (measured/grow/wrap/grid/…, LAY-SHOWCASE) в сайдбар
не входят — это демонстрация движка раскладки, а не компонентов.

## Секции Wave C (D2) и их API

| Секция | API (реальный компонент) | Интерактив v1 |
|---|---|---|
| Checkbox | `component::checkbox_layout/style/glyph` | клик — цикл Unchecked→Checked→Indeterminate (+2 статичных состояния) |
| Slider | `component::slider_layout/style` | drag ручки (on_cursor_moved), клик по треку |
| Radio | `component::radio_group_layout/style` | выбор пункта |
| Tabs | `component::tabs_layout` | переключение панелей |
| Segmented | `component::segmented_layout` | выбор сегмента |
| Command palette | demo-панель на kit-примитивах (компонент `command_palette` — v2) | открытие, выбор пункта, Esc |
| Tree | demo-ряды на kit-примитивах (`component::tree` — v2) | expand/collapse корня, выбор листа |
| Modal | `kit::modal` (панель) | открыть/закрыть (крестик, Esc) |
| Accordion | `component::accordion_layout/style` | expand/collapse ×3 |
| Badge | `component::badge_layout/style` | Dot / Count(3) / Text("NEW") |
| Progress | `component::progress_layout/style` | bar 66% + spinner (кольцо) |
| Skeleton | `component::skeleton_layout` | паттерн TextLines(3) |
| Snackbar | demo-бар на kit-примитивах (`component::snackbar` — v2) | показать/закрыть |
| Popover | demo-пузырь на kit-примитивах (`component::popover` — v2) | якорь/закрытие |

Компоненты Wave C, где v1 использует kit-примитивы вместо paint-слоя
компонента, помечены «— v2»: раскладка/поведение соответствуют контракту
компонента, отрисовка — базовые контролы кита (замена на компонентный
paint — кандидат Wave D v2).

## Интерактивность существующих секций (D1)

В TO-BE поверх статичных матриц состояний (те же rect'ы — непрозрачные
заливки) рисуются интерактивные контролы: кнопки (hover/press/click от
курсора), чипы (клик — Selected), Switch (клик — toggle), TextField (клик —
фокус, ввод с клавиатуры/IME, каретка), List (выбор строки), Dropdown
(якорь открывает меню, пункт выбирает), Toast (показ/скрытие).

## Доступ (D6)

- Натив: `?` → «О интерфейсе» (без режима) → тумблер TO-BE в тулбаре.
- Web: `?ui=demo` — витрина сразу в TO-BE (`url_params::WebParams::ui_demo`,
  потребление — `app_spawn::spawn_desk_web` → `App::open_kit_demo`).
- Standalone `canvasdesk-demo` — v2 (упомянут в issue как опциональный).

## Документация секций (D7)

- В TO-BE каждая секция Wave C несёт строку описания (D7-ключи
  `kit.desc.*`, RU/EN): компонент · API · интерактив/a11y.
- a11y-контракты клавиатуры (Space для toggle-ов, стрелки для радио/табов,
  Esc для модалок) зафиксированы в
  `docs/interface-objects/kit-component-contracts.md`; v1 витрины
  реализует клик-интерактив + Esc-лестницу (модалка/палитра/поповер/dropdown),
  полный keyboard-ring по демо — v2.

## Гейты

- `cargo test -p canvas-app` — TDD-ядро переходов (`kit_demo::tests`) +
  структурные тесты TO-BE раскладки (`tobe_demo_layout_structure`,
  `tobe_dropdown_and_hit_ids`) + прежние golden-пины (AS-IS не тронут).
- G4-линт: `lint_kit_demo_tobe` (максимальный кадр: dropdown+модалка+
  снекбар+поповер+палитра открыты; вьюпорты × RU/EN — overlaps/viewport).
- fmt / clippy -D warnings / lay7-lint / wasm-gate — по AGENTS.md.
