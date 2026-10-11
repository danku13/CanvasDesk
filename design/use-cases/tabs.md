# Вкладки (Tabs)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/tabs.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Переключение панелей контента на месте: одна активная вкладка из N, контент
под полосой заменяется без наложения поверхностей. Цитата модульного
doc-comment: «Wave C §5.3.5: Tabs — панель вкладок с активным индикатором.
Анатомия: `bar` (полоса) + `tab`(N) + `indicator` (активный) + `content`.
Keyboard: ←/→ navigate tabs, Tab — enter content» (спека —
`docs/dev-researches/ui-kit-deep-review-2026-10.md` §5.3.5). Отличия:
SegmentedControl (`crates/canvas-ui/src/component/segmented.rs`) — режимный
переключатель без контент-области; Accordion — вертикальный стек раскрываемых
секций (раскладывает контент, а не заменяет). Таб-полоса настроек —
собственная реализация в `settings_ui.rs`, компонент кита не использует.

## 2. Анатомия и размеры
| Элемент | Геометрия (из `tabs_layout`) |
|---|---|
| Полоса `bar` | вся ширина слота × `bar_h` (параметр вызова; в тестах 36) |
| Таб | равные ширины: (`slot.w` − `gap`·(N−1)) / N, высота `bar_h` |
| Зазор `gap` | параметр вызова; 0 — «ритм без зазора» (LAY7.1 `../rules/11-layouts.md`) |
| Индикатор Underline | ширина tab_w·0.6, высота 2, низ полосы (y + bar_h − 2), по центру |
| Индикатор Fill | весь rect таба |
| Индикатор Pill | инсет 4 с каждой стороны таба |
| Радиус | Pill — `Shape::L` 12; Underline/Fill — `Shape::None` 0 |
| Контент | ниже полосы: y + bar_h, высота slot.h − bar_h (кламп ≥ 0) |

## 3. Токены (слоты KitPalette)
| Состояние/роль | Слот (реестр §K2 `../rules/10-components.md`) |
|---|---|
| fill Normal/Selected | прозрачный [0.0; 4] |
| fill Hovered/Pressed | hover_fill (= Pressed, ST2) |
| Индикатор активного | accent |
| Текст активного | text_title |
| Текст остальных | text |
| Текст Disabled | disabled_text (fill не дифференцируется) |

Слоты взяты из реестра §K2, новых нет (K2.1); Pressed в v1 = Hovered (ST2).

## 4. Состояния
Матрица ST1 `../rules/08-states.md`: Disabled > Error > Pressed > Hovered >
Focused > Dragged > Selected > Normal. Активная вкладка — отдельный флаг (`active`-индекс), а не
KitState::Selected: выбор постоянный, ортогонален hover (семантика Selected
из ST1). Keyboard-фокус — KitState::Focused (Wave T §5.1.1: state-layer
0.10, focus-ring по слоту `accent`); в `tab_style` ветки Focused нет —
overlay-механика `state_layer_alpha` — за потребителем.

## 5. Взаимодействие
- **Мышь**: `tabs_hit` — индекс по rect таба (клик ниже полосы — None);
  клик = выбор по контракту ST3 (press внутри + release внутри).
- **Клавиатура**: `tabs_key` + `TabKey::Prev`/`TabKey::Next` — ←/→ с
  закольцовкой (0 → N−1, N−1 → 0); один таб и пустой список — None. Tab
  уводит фокус в контент (doc-comment). Home/End/Enter в модуле нет.
- **Touch**: интерактивные зоны подчиняются LAY8.3 (`../rules/11-layouts.md`)
  — 44 logical px на coarse-pointer; высота полосы — параметр потребителя.

## 6. Граничные случаи
- Пустой список подписей: `tabs` пуст, bar/content всё равно раскладываются
  (тест `tabs_layout_empty_labels`).
- Длинная подпись: ellipsis по ширине таба (`TextMeasurer::ellipsis`);
  полная подпись (тултип) — за потребителем.
- Табы не влезают: ширины клампятся в ≥ 0; ветки SqueezeTail в коде нет,
  хотя doc-comment `tabs_layout` обещает «равные ширины … или SqueezeTail» —
  расхождение, задача на рефакторинг.
- Один таб: стрелки не двигают выбор (`tabs_key` → None).

## 7. Где в коде
`crates/canvas-ui/src/component/tabs.rs` — `TabStyle` (Underline/Fill/Pill),
`tabs_layout`/`TabsLayout`/`TabLayout`, `tab_style`/`TabStyleResult`,
`tabs_hit`, `tabs_key`/`TabKey`; ре-экспорт из `crates/canvas-ui/src/kit.rs`
(строки 125–126). Витрина UI-консоли: секции в
`crates/canvas-app/src/kit_ui.rs` нет (нарушение K4 — аналог W2 из аудита).

## 8. Что меняется при правке файла
- Геометрия/индикаторы → чистые функции `tabs_layout` + юнит-тесты модуля;
  при подключении к поверхности — гейт G4 (3 вьюпорта × RU/EN).
- Цвета → только слоты §K2; новый цвет = новый слот (K2.1) + правка
  `../rules/10-components.md` и моста `From<&ThemeColors>`.
- Строки компонента нет в реестре §K1 — по K1.1 добавляется в той же волне,
  что и правка (реестр отстаёт от кода).
- Витрина/эталоны: SECTION_* в `crates/canvas-app/src/kit_ui.rs`;
  snapshot-эталоны — через `Comp` в `crates/canvas-ui/tests/snapshot.rs`
  (FR-068 W0, Контракт-9; сейчас табов там нет).
