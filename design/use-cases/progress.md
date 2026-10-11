# Прогресс (Progress)

> **Контракт поведения компонента.** Нормативный документ контура `design/`:
> человек (владелец) читает его, чтобы понимать гарантии компонента; агент
> приводит реализацию к контракту. Расхождение кода с контрактом — задача
> на рефакторинг, а не правка контракта «под код». Источник истины по фактам:
> `crates/canvas-ui/src/component/progress.rs`. Актуализировано: 2026-10-10
> (волны T/C/L, issue #47).

## 1. Назначение
Индикатор хода длительной операции: determinate (доля известна) и
indeterminate (bar/spinner — ход неизвестен). Спека Wave C
(`docs/dev-researches/ui-kit-deep-review-2026-10.md` §5.3.7): «AI-агент busy,
graph_builder busy, scenario worker — используют текст "..." или ad-hoc».
Отличия: skeleton (`../use-cases/skeleton.md`) — форма будущего контента;
spinner — активность без процента; progress — метрика процесса.

## 2. Анатомия и размеры
| Kind | Геометрия |
|---|---|
| Linear { value } | track по слоту, track_h = max(button_h·0.2, 2) (Sm 30→6); fill_w = слот·value |
| LinearIndeterminate | тот же трек; fill = 40% ширины (позицию задаёт фаза) |
| Circular { value, radius } | круг Ø 2·radius, центр слота; fill = track |
| Spinner { radius } | круг Ø 2·radius, центр слота |

Трек вертикально центрируется в слоте; радиус — Shape::Full (pill/круг),
тень отсутствует (Elevation::None). `ControlSize::button_h`: Xs 24 / Sm 30 /
Md 36 / Lg 44.

## 3. Токены (слоты KitPalette)
`progress_style(state)` — цвет выбирается слотом, без арифметики (K2,
`../rules/10-components.md` §K2):
| Элемент | Normal/Hovered/… | Disabled |
|---|---|---|
| Трек | control_fill | control_fill (ветки совпадают) |
| Заполнение | control_primary | disabled_text |

Модуль возвращает цвет заполнения отдельно от `ControlStyle` трека — источник
один: слоты K2; alpha-варианты — только `tint()` (K2.2).

## 4. Состояния
Не контрол (hit/press нет) — состояний ST1 нет; Disabled влияет только на цвет
заполнения. Анимация — детерминированная фаза: `progress_phase(time_ms, kind)`
= (time_ms % period)/period; периоды: LinearIndeterminate 1500 мс, Spinner
1000 мс, determinate → 0.0. `time_ms` приходит от потребителя (`anim.rs`) —
dt-детерминизм M2 (`../rules/06-motion.md`): кит таймеров не хранит (M3.5),
тот же кадр = та же картинка.

## 5. Взаимодействие
- Не интерактивен: клики и наведение проходят мимо (decoration, ST6
  `../rules/08-states.md`), hit-зоны не регистрируются.
- Отмена операции — задача потребителя (кнопка рядом), не индикатора.
- Модуль не интерполирует value: смена в кадре — мгновенная (M3.3:
  отсутствие анимации — тоже состояние).

## 6. Граничные случаи
- value клампится [0, 1]: 1.5 → полная ширина, −0.5 → 0 (юнит-тесты модуля).
- Фаза зациклена (% period): 1500 мс → 0; у determinate всегда 0.
- track_h ≥ 2 на малых размерах; слот уже трека — трек повторяет слот.
- Периоды 1500/1000 — локальные константы модуля вне шкалы M1
  (150/300/600/1200/1600/2500): по M1 off-scale длительность = правка токена,
  не локальная константа — расхождение, задача рефакторинга.

## 7. Где в коде
`crates/canvas-ui/src/component/progress.rs` — `progress_layout`,
`progress_style`, `progress_phase`, `ProgressKind`; тесты клампа/фазы — в
модуле. Общие типы (`ControlSize`, `Shape`, `KitPalette`) —
`crates/canvas-ui/src/component/mod.rs`. Фасад —
`crates/canvas-ui/src/kit.rs` (108). Потребителей в `crates/canvas-app` нет. Витрина: секция progress в
`crates/canvas-app/src/kit_sections_k4.rs` (K4) — 5 рядов Linear/
LinearIndeterminate/Circular/Spinner в Normal + Linear в Disabled.

## 8. Что меняется при правке файла
- Формулы трека/фазы → модуль + его юнит-тесты.
- Периоды анимации → сначала правка `../rules/06-motion.md` и
  `design/tokens/motion.json` (M1), потом код — не локальные константы.
- Слоты → K2.1 одновременно: KitPalette + мост + таблица K2.
- Новый kind (например, кольцевой determinate с дугой) → реестр K1
  (`../rules/10-components.md`) + секция витрины в той же волне (K4).
