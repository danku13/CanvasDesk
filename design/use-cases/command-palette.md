# Палитра команд (CommandPalette)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/command_palette.rs`. Актуализировано:
> 2026-10-10 (волны T/C/L, issue #47).

## 1. Назначение
Поиск действий по вводу (Cmd+K): единая action-поверхность кита. Цитата
модульного doc-comment: «Wave C §5.3.6: CommandPalette — поиск действий
(Cmd+K). Анатомия: `input` (поиск) + `results` (N actions) + `selected`.
Keyboard: ↑/↓ navigate, Enter activate, Esc close, Tab autocomplete»
(спека — `docs/dev-researches/ui-kit-deep-review-2026-10.md` §5.3.15, P1
Wave A, источник — `ActionRegistry`; из кода: «Wave A добавит
ActionRegistry… Сейчас — generic по slice `&[CommandAction]»`). Отличия:
панель поиска (`search-panel.md`, Ctrl+F) ищет по нодам/файлам канваса и
летит камерой; dropdown открывается от якоря и выбирает значение; палитра —
окно top-center, keyboard-first, без якоря.

## 2. Анатомия и размеры
| Элемент | Геометрия (из `command_palette_layout`) |
|---|---|
| Окно | ширина min(560, viewport.w − 80), top-center, отступ сверху 60 |
| Поле `input` | ширина окна × `input_h` (параметр; в тестах 40) |
| Строка результата | ширина окна × `row_h` (параметр; в тестах 32) |
| Видимых строк | min(длина результатов, `max_visible`) (параметр; в тестах 8) |
| Привязка строки | каждая строка несёт индекс действия: (rect, action_index) |
| Рамка/паддинг/радиус | в модуле нет — хром окна за потребителем |

Действие — `CommandAction {id, label, kbd, category}`: `kbd` — kbd-строка
(в коде: «cmd+k», «shift+0»); `category` — `CommandCategory` для
группировки: File, Edit, View, Navigate, Tools, Help, Ai.

## 3. Токены (слоты KitPalette)
В модуле стилевой функции нет — окно и строки красит потребитель из
реестра §K2 (`../rules/10-components.md`): фон окна panel_fill, рамка
panel_border, hover строки hover_fill, выбранный результат selected_fill,
фокус поля accent, метки (категории, kbd) text_muted; новые цвета —
только новые слоты (K2.1), не литералы.

## 4. Состояния
Палитра: открыта/закрыта (слой/capture в модуле не объявлены — контракт
`SurfaceDecl` по L3, `../rules/05-layering.md`). Строка результата —
матрица ST1 `../rules/08-states.md`; выделение — курсор `selected`-индекс
(один активный результат, keyboard-first); поле ввода — фокус поля кита
(KitState::Focused, state-layer 0.10). Клавиатурный контракт кодом не
обеспечен: enum `CommandPaletteKey` {Prev, Next, Enter, Escape} объявлен,
резолвера нет — расхождение, задача на рефакторинг.

## 5. Взаимодействие
- **Запрос/матчинг**: `search_actions` — пустой запрос возвращает все
  действия; непустой — case-insensitive подстрока по `label`
  (`label.to_lowercase().contains`); fuzzy-матчер — будущая замена
  (doc-comment). Дебаунса в модуле нет — троттлинг ввода за потребителем.
- **Мышь**: `command_palette_hit` — индекс действия по точке (клик в поле
  ввода — None); клик по строке = активация.
- **Клавиатура**: ↑/↓ — курсор, Enter — активация выбранного, Esc —
  закрытие (Esc-стек первым, L4), Tab — автодополнение (§4).
- **Touch**: зоны строк — LAY8.3 (`../rules/11-layouts.md`), 44 logical px
  на coarse-pointer.

## 6. Граничные случаи
- Нет совпадений: результатов 0 — модуль не рисует ни строк, ни empty-state;
  «ничего не найдено» за потребителем (эталон — `scheme-gallery.md`).
- Результатов больше `max_visible`: показываются первые N; скроллбара в
  модуле нет — окно прокрутки ведёт потребитель (K3).
- Узкий вьюпорт: ширина сжимается до viewport.w − 80 (отступ ≥ 40 с каждой
  стороны); гейт G4 при подключении.
- Длинные подписи: урезание строки — за потребителем (ellipsis по ширине
  окна); `kbd`-метка не должна вытеснять подпись.
- Не путать с `crates/canvas-app/src/palette.rs` — палитра выделения
  FR-009/FR-010; в LAY7 «Exceptions» — «command palette» (PAL_* 5/5/3/2).

## 7. Где в коде
`crates/canvas-ui/src/component/command_palette.rs` — `CommandAction`/
`CommandCategory`, `command_palette_layout`/`CommandPaletteLayout`,
`search_actions`, `command_palette_hit`, `CommandPaletteKey`; ре-экспорт из
`crates/canvas-ui/src/kit.rs` (строки 102–104). Потребителей в app нет
(Ctrl+K не подключён). Витрина: секция command_palette в
`crates/canvas-app/src/kit_sections_k4.rs` (K4) — окно палитры целиком
(input + строки результатов: Normal/Selected, Hovered на строке).

## 8. Что меняется при правке файла
- Геометрия → `command_palette_layout` + юнит-тесты модуля (centered-top,
  max_visible, hit); G4 — при подключении к поверхности.
- Матчинг → `search_actions` (fuzzy/`ActionRegistry` — волна A, §5.3.15);
  резолвер `CommandPaletteKey` закрывает расхождение из §4; Esc/фокус
  поля — по `../rules/09-input.md`.
- Витрина: секция command_palette в `crates/canvas-app/src/kit_sections_k4.rs`
  (K4); snapshot через `Comp` (FR-068 W0, Контракт-9) отсутствует.
