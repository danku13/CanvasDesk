# Аккордеон (Accordion)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/accordion.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Сворачиваемая секция: заголовок с шевроном, раскрывающий контент под собой.
Цитата модульного doc-comment: «Wave C §5.3.6: Accordion — сворачиваемая
секция. Анатомия: `header` (chevron + label) + `content` (expanded/collapsed).
Click on header toggles expanded» (спека —
`docs/dev-researches/ui-kit-deep-review-2026-10.md` §5.3.6; референсы —
egui `CollapsingHeader`, M3 Expansion panels). Отличия: от Tabs — секции
стакаются вертикально и раскрываются независимо (своё `expanded`), контент
занимает место в потоке, а не заменяется; плоский список — List (§K1),
иерархия — Tree.

## 2. Анатомия и размеры
| Элемент | Геометрия (из `accordion_layout`) |
|---|---|
| Заголовок `header` | вся ширина слота × `header_h` (параметр; в тестах 32) |
| Шеврон `chevron` | левая полоса `chevron_w` × `header_h` (параметр; в тестах 24), ▶/▼ |
| Контент `content` | ниже заголовка: раскрыт — `content_h` (параметр; в тестах 100), свёрнут — 0 |
| Радиус заголовка | `Shape::S` 8, elevation None (`ControlStyle`) |
| Признак expanded | цвет шеврона, не геометрия |

Высоту контента фиксирует вызывающий (самозамера нет); подпись модуль не
урезает — параметры `m/fs/family/font_size` в сигнатуре не используются.

## 3. Токены (слоты KitPalette)
| Состояние/роль | Слот (реестр §K2 `../rules/10-components.md`) |
|---|---|
| fill Normal | control_fill |
| fill Hovered/Pressed | hover_fill (= Pressed, ST2) |
| Рамка | control_border |
| Текст заголовка | text |
| Шеврон свёрнутой | text_muted |
| Шеврон раскрытой | accent |

Всё из реестра §K2, новых слотов нет (K2.1).

## 4. Состояния
Состояния ST1 `../rules/08-states.md` относятся к заголовку-контролу:
Disabled > Pressed > Hovered > Normal. Раскрытость — раскладка, а не
KitState. Disabled в `accordion_style` не дифференцирован (fill = control_fill,
текст = text) — расхождение со ST2: нужен disabled_text (рефакторинг).
KitState::Focused (Tab-фокус, state-layer 0.10) — через
`state_layer_alpha` у потребителя. Шеврон лежит внутри `header`, поэтому
клик по нему и по заголовку равнозначны (`accordion_hit` проверяет только
header).

## 5. Взаимодействие
- **Мышь**: `accordion_hit` — попадание в заголовок (включая шеврон) →
  toggle `expanded`; контракт клика ST3 (press внутри + release внутри);
  клик по контенту ничего не переключает.
- **Клавиатуры в модуле нет**: ни Enter/Space-активации, ни Tab-обхода —
  контракт (Enter = toggle, Tab — FocusRing поверхности) фиксируется этим
  файлом как будущая правка.
- **Touch**: зона клика = `header` на всю ширину слота; на coarse-pointer —
  LAY8.3 (44 logical px, `../rules/11-layouts.md`).

## 6. Граничные случаи
- Свёрнутая секция: content высотой 0; зазоры между секциями — за
  потребителем (LAY7: `MeasuredItem::Fixed`, не «пустота»).
- `content_h` меньше нужного: усечение/скролл внутри контента — за
  потребителем (скролл-модель K3).
- Длинный заголовок: ellipsis в модуле отсутствует (см. §2) — правка кода
  до контракта «подпись урезается по ширине header − chevron_w».
- Стек секций: раскладку стопкой (зазор SPACING_S/SM) ведёт потребитель.

## 7. Где в коде
`crates/canvas-ui/src/component/accordion.rs` — `accordion_layout`/
`AccordionLayout`, `accordion_style` (возвращает `ControlStyle` + цвет
шеврона), `accordion_hit`; ре-экспорт из `crates/canvas-ui/src/kit.rs`
(строки 94–95). Витрина UI-консоли: секции в
`crates/canvas-app/src/kit_ui.rs` нет (нарушение K4 — аналог W2).

## 8. Что меняется при правке файла
- Геометрия → `accordion_layout` + юнит-тесты модуля (expanded/collapsed,
  `accordion_hit_header`); гейт G4 — при подключении к поверхности.
- Цвета → слоты §K2 (control_fill/hover_fill/accent/text_muted); disabled =
  добавление disabled_text, не локальный цвет (K2.1).
- Строки нет в §K1 — по K1.1 реестр пополняется в той же волне, что и
  правка кода.
- Витрина/эталоны: SECTION_* в `crates/canvas-app/src/kit_ui.rs`;
  snapshot-эталон через `Comp` (`crates/canvas-ui/tests/snapshot.rs`,
  FR-068 W0, Контракт-9) — сейчас секции нет.
