# FR-074: Расширения CSS-паритета canvas-ui — Auto/minmax-треки Grid, Horizontal sticky, Transform (rotate), Z-index per-element

- **Статус:** ✅ выполнено (2026-09-25, сессия агента; вместе с W4 FR-068)
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (планирование); решения — владелец проекта
- **Запрос владельца:** «Далее надо добавить свою реализацию для: Auto-треки Grid, Transform (rotate), minmax(), Horizontal sticky, Z-index per-element; и после этого реализуем W4. На потом если когда-нибудь потребуется оставляем: Auto-flow dense/column — masonry-галереи» (2026-09-25)
- **Связанные:** FR-068 (W4 — dep-минимизация), ADR-0015 (стратегия UI-стека), FR-062 (layout-примитивы), ADR-0013 (T2-триггер — авторасчётные треки)

## Суть

Расширить собственный UI-стек (`canvas-ui`) возможностями CSS, которых
не хватало для механического переноса веб-дизайна: авторасчётные треки
Grid, функция `minmax()`, горизонтальный sticky, повороты в paint-слое
и per-element z-порядок. Всё — без внешних зависимостей (zero-dep
инвариант G7) и до вырезания taffy (W4), чтобы зафиксировать паритет
против taffy-оракула, пока он существует.

## Реализация

### 1. Grid: Auto-треки + `minmax()` (FlexLayoutEngine + TaffyBackend)

- `SceneTrack::MinMax { min: TrackMin, max: TrackMax }` — CSS
  `minmax(min, max)`: `TrackMin ∈ {Auto, Length, Percent}` (CSS
  запрещает fr в min), `TrackMax ∈ {Auto, Length, Percent, Fill}`.
  Конструктор `SceneTrack::minmax(min, max)` (нормализация ≤ 0).
- Алгоритм колонок `FlexLayoutEngine` — шаги CSS Grid §11.5–11.8 в
  порядке taffy 0.14 (паритет подтверждён, см. ниже):
  1. Плейсмент row-major (sparse cursor со спанами) — единый оракул
     `grid_placements` (ранее курсор дублировался трижды).
  2. Контент-вклад трека = max max-content ширин span-1 ячеек
     (span>1 авторасчётные треки не растят — упрощение документировано;
     ячейка всё равно тянется на область спана).
  3. Base/limit: `Length/Percent` — definite; `Auto` — контент;
     `Fill` — base 0/limit ∞ (Fr); `MinMax` — base = min, limit =
     max definite | контент (max Auto) | ∞ (max Fill → Fr).
  4. §11.6 Maximise: свободное место поровну трекам с конечным лимитом,
     заморозка достигших (fr не участвует — taffy step 5: infinite
     growth limit → base).
  5. §11.7 Expand Flexible: `find_size_of_fr` для fr=1 — floored-треки
     становятся «inflexible», fr пересчитывается (пол min побеждает,
     переполнение видно — G4).
  6. §11.8 Stretch auto: остаток поровну AutoMax-трекам (taffy default
     `justify_content: STRETCH` — как в браузерах).
- Семантика grid-item уточнена до CSS: definite/Percent ячейки НЕ
  растягиваются в область трека (старт-выравнивание, переполнение
  видно), Fill/Auto — stretch (ранее ячейка всегда получала область
  трека).
- `TaffyBackend` (до вырезания): `MinMax` → нативный
  `TrackSizingFunction { min, max }` (`MinTrackSizingFunction`/
  `MaxTrackSizingFunction`, Fill = `fr(1.0)`).

**Паритет:** `tests/grid_tracks_parity.rs` (временный, удалён в W4
вместе с taffy) — 10 сцен (Auto×Length×Fill, два Auto, Auto+span,
minmax definite/Auto-min/Fill-пол/percent, полный микс, переполнение
полов) — 10/10 ПОБИТОВО против taffy на целых входах. Документированное
упрощение: min-content отдельно от max-content не моделируется
(«контент» = max-content span-1 ячеек).

### 2. Horizontal sticky

- `ScenePosition::Sticky { top: Option<f32>, left: Option<f32> }` —
  оси независимы (CSS `position: sticky`), `None` — нет прилипания.
- Ось-зависимый scroll-предок: вертикаль (`top`) — ближайший
  Y-scroll-предок (Column/Grid), горизонталь (`left`) — X-предок (Row).
  Формулы пост-обработки 1:1 с вертикальной: `x = max(flow_x − offset,
  container_x + left)`; кламп транслирует поддерево (fixed-потомки —
  viewport-контекст).
- Фикс попутный: ось корня сцены в `taffy_backend` всегда была Y —
  content-shift Row-корня уходил не в ту ось (теперь по `kind`).

### 3. Transform (rotate)

- `PaintItem::Transform { deg: f32, origin: UiPoint, items }` — CSS
  `transform: rotate()` как ДАННЫЕ (G7): layout не меняет (CSS-семантика),
  поворот по часовой в экранной системе (y-вниз) вокруг `origin`.
- `Painter::rotated(deg, origin, paint)` + `Painter::rotated_centered(
  area, deg, paint)` (origin = центр области); вложенность — композиция
  у потребителя; `walk()` — прозрачный спуск (не клип).
- Потребитель (`canvas-app`): прозрачный проход (прецедент ClipRect W1
  до scissor); конвертация в rotate-инстансы рендера — отдельная задача
  (формат инстансов `CardInstance` без rotation).

### 4. Z-index per-element

- `PaintItem::ZGroup { z: i32, items }` + `Painter::z_group(z, paint)` —
  CSS z-index внутри stacking context'а журнала Painter'а:
  `take_items()` СТАБИЛЬНО сортирует по z (больше — позже = поверх;
  равные — порядок вызовов; отрицательные — под z = 0); вложенный
  `z_group` — свой контекст (CSS stacking context), внутренний take
  сортируется при вкладывании.
- `walk()` — прозрачный спуск; потребитель рисует в уже
  отсортированном порядке (`kit_ui.paint_items` — прозрачный проход);
  hit-тест z — забота потребителя (данные доступны).

### 5. Отложено (решение владельца)

- **Auto-flow dense/column — masonry-галереи.** Плейсмент — только
  row-major sparse cursor; dense-упаковка и masonry-раскладка — по
  продуктовому триггеру.

## Гейты

- `cargo test -p canvas-ui --test flex_grid_tracks` — 11 контрактных
  тестов треков (содержит тесты §11.6/11.7/11.8, полов minmax,
  span-семантики, constructor normalization).
- `cargo test -p canvas-ui --test flex_scene_sticky` — 4 (горизонтальный
  кламп, отступ+поддерево, независимость осей, ось-зависимость предка).
- `cargo test -p canvas-ui --test grid_tracks_parity --features taffy` —
  10/10 бит-в-бит (временный, удалён в W4).
- paint-тесты: +4 (rotated payload/центр, walk-спуск, стабильная
  z-сортировка, вложенные контексты).
- taffy_backend: +2 (горизонтальный sticky, обе оси) — удалены в W4.
- Demo 01 (sticky header) golden — без изменений (Option-пара
  эквивалентна прежней семантике).
- Попутный фикс main: `tests/snapshot.rs` — match дампа не покрывал
  `PaintItem::Icon` (предсуществующий compile-fail), добавлены Icon +
  Transform/ZGroup маркеры.

## Документация

- `docs/DEPENDENCIES.md` — taffy вырезан (история + W4-финал).
- `docs/ui-kit.md` — backend'ы (движок один), grid-треки FR-074,
  perf-taffy → история.
- `docs/adr/adr-0014` / `adr-0015` — статусы.
- `index-cr-fr.md` — FR-074.
- `worklog.md` — запись сессии.

## История изменений (Changelog)

- `2026-09-25` — агент: создан и реализован (в одной сессии с W4
  FR-068, по приказу владельца). 4 фичи + отложенный masonry. Гейты
  зелёные; parity против taffy зафиксирован до вырезания. Вопрос
  онбординга/пользовательской документации: фичи — внутренние
  библиотечные примитивы `canvas-ui` без пользовательских поверхностей —
  онбординг/`user-docs` не затрагиваются.
