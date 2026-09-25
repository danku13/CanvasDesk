# FR-075: Паритет элементов шелла карточки с web-ui (CSS-модель кита)

- **Статус:** в работе (W0 исполнено 2026-09-26)
- **Тип:** FR (архитектура + серия исправлений)
- **Приоритет:** важно
- **Владелец:** агент (deep dive по запросу владельца)
- **Источник:** сообщение владельца (сессия web-77d836dd, 2026-09-26): «нужно чтобы все
  элементы (шелл карточки: позиция/размер ноды, полоса категории, иконка, таблица
  тела, порты) адекватно отрисовывались, сейчас с этим есть проблемы; deep dive как
  дать этим элементам функционал паритетный web-ui».
- **Связанные:** FR-068 W3 §9.4 (граница «не мигрировать: канвас-домен»),
  FR-074 (CSS-паритет кита — screen-space), FR-061 (kit-выравнивание тела),
  FR-056 (scissor-клипы), FR-016 (LOD-гашение декора), FR-045 R-3 (pending-порты),
  `docs/architecture/shell-world-space.md` (архитектура шелла), CR-010 (шапка).
- **Создан:** 2026-09-26
- **Обновлён:** 2026-09-26

---

## 1. Описание (What)

Владелец зафиксировал, что элементы шелла карточки отрисовываются с проблемами и
требует deep dive: как дать им функционал, **паритетный web-ui** — то есть
CSS-подобной модели кита `canvas-ui` (layout, Painter с `Rect/Text/Icon/ClipRect/
Transform/ZGroup`, состояния `KitState`, иконки SVG-атласом, тема слотами).
Речь НЕ о миграции шелла на кит (решение FR-068 W3 §9.4 в силе: кит — screen-space,
карточка — world-space), а о **паритете возможностей**: мир-квады должны уметь то
же, что умеют экранные PaintItem — пер-угловые радиусы, клипы, повороты, z-группы,
SVG-иконки, состояния/анимации.

## 2. Влияние (Impact)

| Элемент шелла | Текущее состояние | Что не паритетно web-ui |
|---|---|---|
| Карточка (позиция/размер) | `cards.rs::card_instance` → SDF `cards.wgsl` | радиус один на все 4 угла (нет CSS `border-radius`-пер-угол); нет rotate; рамка/тень — жёсткие ветки шейдера, нет box-shadow-модели |
| Полоса категории | `cards.rs::template_band_instance` (6 px, квадратные углы) | рисуется ПОВЕРХ карточки после неё (renderer.rs ~1518–1543) — квадратные «уголки» выступают за скруглённый силуэт (радиус карточки 8, полоса 6) — **визуальный дефект, фикс W0** |
| Квад-иконка роли | `cards.rs::template_icon_quads` (8 кодов из плоских квадов) | решение владельца FR-018; но params.x = 1.0 на каждом подкваде → штрихи 1.2–2.5 px рендерятся капсулами; кит-`Icon` = SVG-атлас с tint (`icons.wgsl`) — screen-only, мировых иконок нет |
| Таблица тела | `row_grid.rs` + `text.rs`; геометрия на ките (FR-061) | тела без `ClipRect`-семантики (клипирование точечное — заголовок/тело клампами); нет `ZGroup`-семантики; пилюли/Σ — кастомные квады без общих примитивов |
| Порты/якоря | `cards.rs::build_port_instances/line/param` (мировой «хвост» кадра) | рисуются ПОСЛЕ всех z-сегментов → точки портов фоновых нод поверх карточек переднего плана (класс дефекта, чинившийся z-планом T5 для тамбнейлов — zorder.rs:1–14); без culling (полный обход `canvas.nodes`, renderer.rs ~1602–1626); состояния: только hover — нет Pressed/Disabled/Selected (кит: `KitState` canvas-ui/src/widget.rs:68) и transition-анимаций; pending-контур R-3 FR-045 не реализован |

## 3. Анализ (Root Cause)

Мир-домен сложился исторически как «строй инстансы руками»: каждый элемент —
своя функция-строитель с литералом `CardInstance` (16×f32: `pos/size/fill/border/
params`), а общие возможности (скругление, клип, поворот, состояние) не
выделялись в примитивы. Экранный домен через FR-057/FR-074 получил painter-модель
(примитивы как данные), а мир-домен — нет; мост `screen_instance_to_world`
(renderer.rs:209–217) и `StageTransform::instance_to_world` (stage.rs:61–78)
покрывают только `pos/size/params[0]`.

**Дефект-ревизия (по коду):**

1. **Полоса категории — квадратные углы поверх скруглённой карточки.**
   `template_band_instance` (cards.rs:399–411): `params [0,0,0,1]` (радиус 0),
   `pos [node.x, node.y]`, ширина `node.width`; инстанс пушится ПОСЛЕ карточки
   (renderer.rs ~1518–1523) → в зоне скруглённых углов карточки (радиус
   `CARD_CORNER_RADIUS = 8`, tokens.rs:149; полоса 6 px) квадратный угол полосы
   выступает за дугу силуэта (на y=0 вылет до ~7.7 px). Артефакт виден на всех
   шаблонных нодах.
2. **`CardInstance` без пер-углового радиуса.** Шейдер `sd_rounded_box`
   (cards.wgsl) принимает один радиус `params.x` — CSS-примитив
   `border-radius: tl tr br bl` недоступен ни полосе, ни будущим элементам.
3. **Мировые иконки отсутствуют как примитив.** `IconPipeline` — screen-space
   (`ViewportUniform`, icon_pipeline.rs:8–10,90–103); `PaintItem::Icon` в
   band/stage-конвертациях игнорируется (app/support.rs:118–122); квад-иконки
   ролей — ручные коды на 8 икон (cards.rs:420+), штрихи с радиусом 1.0.
4. **Порты вне z-плана.** Построчные порты/якоря собираются в world-«хвост»
   после последнего сегмента (renderer.rs ~1583–1675 + zorder::plan_tail_ranges)
   → перекрытие карточек переднего плана; полный обход нод без culling.
5. **Состояния/анимации портов.** Кит даёт матрицу `Disabled > Pressed >
   Hovered > Selected > Normal` (canvas-ui/src/widget.rs:11,68–75); мир-порты —
   только булев hover + drag-compat (cards.rs:1304–1371); анимации нет
   (animate.rs:92–128 — пульс ноды/ребра/полёт камеры).
6. **Transform/ZGroup не исполнены в рендере** (fr-074:80–82: «формат инстансов
   `CardInstance` без rotation» — отложенная задача), ClipRect мира — нет
   (scissor FR-056 есть только у screen-полос).

## 4. Требуемые изменения (Changes) — архитектура паритета

Принцип: **общая модель примитивов, два исполнения** (продолжение «единой
геометрии» fr-061 §4 до уровня примитивов отрисовки). Не миграция на кит, а
появление у мир-домена тех же примитивов.

### 4.1 Волна 0 — исполнено в этом изменении

**Пер-угловой радиус = CSS `border-radius` (примитив мира-квада):**

- `CardInstance` + поле `corners: [f32; 4]` (порядок CSS `[tl, tr, br, bl]`,
  world px); stride 16 → 20 f32 (`FLOATS`), группа `write_to`, атрибут
  `5 => Float32x4` в `CardsPipeline` (cards.rs);
- шейдер `cards.wgsl`: `corner_radius(corners, fallback, p)` — выбор радиуса по
  квадранту (p — физ. px, y-вниз); **все нули corners → `params.x`** (обратная
  совместимость: ~97 существующих строителей не задают corners — рендер
  бит-в-бит);
- Rust-оракул `corner_radius_at` (cards.rs) зеркалит шейдер — юнит-тесты пинят
  семантику (шейдер юнит-тестами не покрывается; все шейдеры валидированы naga);
- конверторы: `screen_instance_to_world` (renderer.rs) и
  `StageTransform::instance_to_world` (stage.rs) делят corners на зум (× scale
  для stage) — иначе экранная геометрия зазумится дважды;
- **фикс дефекта №1:** `template_band_instance` →
  `corners: [CORNER_RADIUS, CORNER_RADIUS, 0.0, 0.0]` — верх полосы повторяет
  дугу карточки, «уголки» исчезли. Отличие от CSS задокументировано: радиусы НЕ
  редуцируются при перекрытии (клип-семантика SDF; CSS уменьшил бы радиус до
  полувысоты — для полосы это вернуло бы артефакт).
- гейты: cargo test canvas-render 380 / canvas-app 398 (0 failed), clippy
  `-D warnings`, fmt, wasm-gate ступень 1 (check wasm32), naga-валидация всех 7
  WGSL; визуальная приёмка владельца — открыть шаблонную ноду (intro-calculations)
  и убедиться в отсутствии «уголков» у полосы.

### 4.2 Волна 1 — z-сегментация портов + culling

Построчные порты/якоря строить ПОСЕГМЕНТНО: в цикле z-сегментов рендера
собирать инстансы портов видимых нод (culling бесплатно) в отдельный буфер с
диапазонами, рисовать между карточками и текст-группой сегмента (как тамбнейлы);
hover-стороны (`build_port_instances`) остаются в world-хвосте (hover-нода
поверх — ожидаемое поведение). Гейт: регресс-тест «порт фоновой ноды не
перекрывает карточку переднего плана» (аналог оракула zorder).

### 4.3 Волна 2 — мировые SVG-иконки (примитив Icon в мире)

`icons.wgsl` получает `CameraUniform` (или world-вариант пайплайна) →
`IconInstance` в world-координатах; квад-иконки ролей заменяются SVG-атласом
(FR-ICONS, tint, `w/zoom`-константный экран НЕ нужен — иконки роли зумятся с
карточкой, LOD-гашение по аналогии с `ANALYSIS_BADGES_MIN_ZOOM`); радиус
подквадов-штрихов (params.x = 1.0) уходит вместе с квад-кодами. Кит-паритет:
`PaintItem::Icon` перестаёт игнорироваться в world-конвертациях.

### 4.4 Волна 3 — ClipRect мира (клип-семантика тела)

Клип-прямоугольники в world-координатах → scissor-бакеты через world→screen
(паттерн `band_scissor_rect` FR-056 с ceil/floor-инвариантом); тело таблицы и
переливы получают `overflow: hidden`-семантику вместо точечных клампов
(`title_clip_width`, `clamp_desc_text`). ZGroup мира → диапазоны z-плана
(механика `draw_ranges` уже поддерживает).

### 4.5 Волна 4 — состояния и анимации портов

Матрица `KitState` для портов: Disabled (слот без параметра), Pressed (drag
порта), Selected (активная связь), Normal; transition — `BoolAnim`-механика
кита (dt-детерминированная), цвета — через слоты темы (F-8); реализация
pending-контура R-3 FR-045 (пунктирный порт-контур) на этом же примитиве.

### 4.6 Волна 5 — Transform (rotate-инстансы, совместная с fr-074)

`CardInstance`/`IconInstance` + угол поворота (packing: свободных слотов нет —
новый атрибут или расширение буфера); исполнение — в вершинном шейдере
(поворот квада вокруг origin до world_to_screen); закрывает «отдельную задачу»
fr-074:80–82 для обоих доменов.

### 4.7 Non-goals

- Миграция шелла на компонентную модель кита — FR-068 W3 §9.4 в силе.
- Замена SDF-хрома карточки (тень/AA) на Painter-примитивы — перф-инвариант
  «один instanced draw» сохраняется во всех волнах.
- Пиксельные golden-тесты — отвергнуты PRD-0009 (геометрические оракулы).

## 5. Точки входа

- `docs/architecture/shell-world-space.md` — §4 (состав шелла), §5 (GPU-путь:
  `CardInstance` 20×f32), §8 (мост: corners/zoom), §11 (инвариант I-токены).
- `docs/ui-kit.md` — раздел про примитивы мира (после волны 2/3).
- `crates/canvas-render/src/cards.rs`, `shaders/cards.wgsl`, `renderer.rs`,
  `stage.rs` — док-комментарии FR-075.
- `docs/plans/fr-074*` — снять отложенную задачу rotate-инстансов (волна 5).

## 6. Проверка (Verification)

- **W0 (исполнено):** `cargo test -p canvas-render --lib` — 380 passed
  (новые: `instance_stride_matches_serialization` — corners-смещения 64..80,
  `template_band_has_top_corner_radius`, `corner_radius_quadrant_selection`);
  `cargo test -p canvas-app --lib` — 398 passed; `cargo clippy --workspace
  --all-targets -- -D warnings`; `cargo fmt --check`; wasm-gate ступень 1;
  naga-валидация 7/7 шейдеров. Ручная приёмка: полоса категории без «уголков»,
  карточки/полосы/иконки/порты визуально идентичны прежнему (I-1: corners
  нулевые у всех прежних строителей → fallback `params.x`).
- **W1–W5:** по своим гейтам в каждой волне (см. §4.2–4.6).

## 7. История изменений

- `2026-09-26` — агент: создан FR-075 (deep dive по запросу владельца); статус
  `в работе`; исполнена волна W0 (пер-угловой радиус, фикс квадратных углов
  полосы категории, конверторы, оракул+тесты, naga-валидация шейдеров);
  дефект-ревизия §2–3 зафиксирована с file:line; волновой план W1–W5.

## 8. Источники истины

- `crates/canvas-render/src/cards.rs` — `CardInstance` (268–304),
  `corner_radius_at` (305–321), `template_band_instance` (399–411),
  `template_icon_quads` (420+), `build_port_instances`/line/param
  (1254–1371), `CardsPipeline` (1503+).
- `crates/canvas-render/src/shaders/cards.wgsl` — `world_to_screen`,
  `corner_radius`, `fs_main` (SDF-слои).
- `crates/canvas-render/src/renderer.rs` — `screen_instance_to_world`
  (209–225), сборка кадра (~1455–1589), world-хвост/порты (~1583–1675),
  `band_scissor_rect` (247–268).
- `crates/canvas-render/src/stage.rs` — `StageTransform::instance_to_world`
  (61–87); `crates/canvas-render/src/zorder.rs` — z-сегменты.
- `crates/canvas-ui/src/{widget,paint}.rs` — `KitState`, `PaintItem`
  (CSS-паритет экрана); `crates/canvas-core/src/tokens.rs` —
  `CARD_CORNER_RADIUS = 8.0` (:149), `CARD_HEADER_HEIGHT = 34.0` (:151).
- `docs/architecture/shell-world-space.md`; `docs/plans/
  fr-061-template-node-kit-alignment.md`; `docs/change-requests/
  fr-074-canvas-ui-css-parity-extensions.md`; `fr-068-w3-consumer-migration.md`
  §9.4; `fr-056-ui-scissor-clipping.md`; `fr-045-...` (R-3).
