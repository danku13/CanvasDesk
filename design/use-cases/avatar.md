# Аватар (Avatar)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/avatar.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Метка пользователя или AI-агента: инициалы, иконка-плейсхолдер или растровое
изображение. Спека Wave C (`docs/dev-researches/ui-kit-deep-review-2026-10.md`
§5.3.10): «Комментарии, collaboration, AI-агент — нуждаются в avatar
(инициалы/иконка/изображение)». Отличия: Icon
(`crates/canvas-ui/src/component/icon.rs`) — монохромный глиф по `IconKind`;
avatar — идентификатор субъекта с тоном фона; badge — счётчик/статус.

## 2. Анатомия и размеры
| Параметр | Значение |
|---|---|
| Форма | квадрат со стороной `size.icon_btn()`, центрирован в слоте |
| ControlSize | Xs 20 / Sm 26 (= ICON_BUTTON_SIZE) / Md 32 / Lg 44 (= MIN_TOUCH_TARGET) |
| Источник | Initials(String) («AB» — 2 буквы), Icon(&'static str), Image(u64 — texture_id потребителя) |
| Радиус | Shape::Full (круг), тень Elevation::None, рамки нет |
| Текст | инициалы поверх fill, text_title |

## 3. Токены (слоты KitPalette)
`avatar_style(tone)` — тон → слот без арифметики (K2,
`../rules/10-components.md` §K2):
| BadgeTone | Слот |
|---|---|
| Default | control_fill |
| Primary | control_primary |
| Success | control_success |
| Warning | control_warning |
| Danger | control_danger |
| Info | accent |

Текст инициалов — text_title. `avatar_layout` всегда ставит tone
BadgeTone::Default: тон выбирает потребитель, передавая его в `avatar_style`.

## 4. Состояния
Не контрол: состояний ST1 нет (как у Icon; ST6 `../rules/08-states.md`).
Hovered/Selected-эффекты — у обёртки потребителя (строка списка, карточка
сообщения); state-layer 8/10% (`state_layer_alpha`) применяется к обёртке,
не к аватару.

## 5. Взаимодействие
- Не интерактивен: hit-зоны нет, клики проходят к контейнеру (ST6: decoration,
  ловящий клик, обязан стать контролом с полной матрицей ST1).
- Слот меньше стороны: аватар не сжимается — сдвиг клампится до 0 (max(0.0)),
  возможно перекрытие соседей.
- Image(u64): рендер текстуры — внешние системы потребителя; модуль хранит
  только id.

## 6. Граничные случаи
- Инициалы длиннее 2 букв: модуль не обрезает — контракт источника
  («"AB" — 2 буквы»), контроль за потребителем.
- Нет изображения: потребитель подменяет Image на Initials/Icon
  (плейсхолдер) — отказ-стратегия в модуле не задана.
- Аватар в плотном ряду: бок о бок с зазором из шкалы S1
  (`../rules/03-spacing-radius.md`), полукруглые пересечения не допускаются.

## 7. Где в коде
`crates/canvas-ui/src/component/avatar.rs` — `avatar_layout`, `avatar_style`,
`AvatarSource`; тесты центрирования — в модуле. Общие типы: `ControlSize`/
`Shape` — `crates/canvas-ui/src/component/mod.rs`, `BadgeTone` —
`crates/canvas-ui/src/component/badge.rs`. Фасад —
`crates/canvas-ui/src/kit.rs` (97). Потребителей в `crates/canvas-app` нет. Витрина: секция avatar в
`crates/canvas-app/src/kit_sections_k4.rs` (K4) — ряд размеров (Lg,
3 источника) и ряд тонов (Md); аватар decoration — ST1-состояний нет.

## 8. Что меняется при правке файла
- Размеры → `ControlSize::icon_btn` (`crates/canvas-ui/src/component/mod.rs`),
  не локальные константы; новый тон → K2.1: KitPalette + мост + таблица K2.
- Клик-обёртка (меню агента, карточка участника) → сначала правка контракта
  (матрица ST1, hit-зона) — аватар перестаёт быть decoration.
- Появление потребителей → секция витрины в той же волне (K4).
