# ADR-0015: Долгосрочная стратегия UI-стека — taffy как переход, своя UI-библиотека целью

- **Статус:** принято (владельцем 2026-09-25 — приказ «начинай первый этап внедрения FR-068»: волна W0 выполнена, гейты зелёные). Критический разбор ADR-0014 + многолетний план; реализация — FR-068 (W0 ✅ 2026-09-25, W1–W4 — по плану).
- **Дата:** 2026-09-24
- **Участники:** владелец (запрос 2026-09-24: «проанализировать ADR-0014 критическим взглядом группы экспертов; кроме внедрения taffy, спланировать дальнейший рефакторинг и переход на полностью свою UI-библиотеку, которая будет иметь все сильные стороны и минимизирует внешние зависимости; первично — причесать UI, далее поэтапный рефакторинг с проверяемыми результатами»), агент (критический анализ ADR-0014, аудит canvas-ui, разработка плана волн W0–W4)
- **Связанные:** ADR-0014 (taffy hybrid — переходное решение, статус «предложено → переходное»), ADR-0013 (заменено ADR-0014), PRD-0009 §7 V-3/V-4/V-5/V-6/V-7 (каркас UI), FR-051 (UiLayer/реестр), FR-053 (layout-примитивы F-7), FR-056 (scissor), FR-057 (Painter/WidgetState), FR-062 (собственные layout v2), FR-067 (план staged миграции на taffy — поглощается FR-068), FR-068 (поэтапный рефакторинг UI — ADR-0015 → FR-068), SPEC §6.3 (бюджеты UI), `docs/DEPENDENCIES.md` (тафy/cosmic-text/rustybuzz/ttf-parser — транзитивные), `docs/ui-kit.md`, `docs/plans/wasm-port.md` §8.8 (≤8 МБ raw / ≤4 МБ brotli)

## Контекст

Владелец (сессия 2026-09-24) запросил: *«проанализировать ADR-0014 критическим взглядом группы экспертов. Кроме реализации внедрения taffy, должен быть спланирован дальнейший рефакторинг и переход на полностью свою UI библиотеку, который будет иметь все сильные стороны и минимизирует внешние зависимости. Первично нам надо причесать UI, далее пойти по пути поэтапного рефакторинга с проверяемыми результатами»*.

Ответы на clarifying questions (сессия 2026-09-24) зафиксировали развилки:

- **Taffy — временный костыль.** Принять ADR-0014 (taffy opt-in за фичей), но параллельно проектировать собственный layout-движок; по достижении паритета — taffy вырезается. Срок: 6–9 месяцев.
- **Приоритеты рефакторинга (топ-3):** UI-паритет (HTML5 demo-layouts 1:1), минимум deps (снижение внешних UI-зависимостей), архитектура (kit.rs 2757 строк → компонентное дерево, retained-state).
- **Cosmic-text — trait boundary.** Изолировать за абстракцией (`Shaper` trait); миграция возможна позже, не блокер.
- **Свой layout — минимум:** flexbox (grow/shrink/basis/wrap) + overflow/clip/scroll. Grid оставить за taffy-опцией (тяжёлая часть CSS-семантики; покрывается, если появится явный потребитель).
- **Проверка (топ-3):** layout-линт (расширение G4 — выход за parent/viewport/silent-clips × N канонических сцен на CI), snapshot-тесты (Painter.items → строка-дамп, изменение — осознанный PR), perf-бюджет (reflow 1000 узлов < 1 мс; 60 fps на 5000 нод — SPEC §6.3).
- **Форма:** новый ADR-0015 + FR-068 (план волн W0–W4).

Состояние UI на момент анализа (2026-09-24, main `64ec34d`):

- `canvas-ui` — 7 080 строк чистой геометрии без wgpu/winit: `geometry.rs` 210, `layout.rs` 1 114 (FR-062 F-13…F-18), `kit.rs` 2 757 (Panel/Button/IconButton/Dropdown/Chip/Toast/Tooltip/Modal/TextField/Switch/Card/Icon/list+ScrollState), `measure.rs` 403 (TextMeasurer на cosmic-text), `paint.rs` 251 (Painter/PaintItem), `frame.rs` 335, `registry.rs` 298 (SurfaceRegistry/capture), `hit.rs` 247, `keyboard.rs` 412, `layer.rs` 106 (UiLayer 9 полос), `widget.rs` 249, `row_guides.rs` 331, `anim.rs` 109, `capture.rs` 59. 128 тестов крейта.
- Потребители layout API: ~497 мест `Child::fixed`/`Row::`/`Column::`/`lay_out(` по workspace (canvas-app ~400, canvas-render ~30, canvas-ui ~62).
- Зависимости `canvas-ui`: `cosmic-text` (workspace), `canvas-core` (workspace). Внешние — только `cosmic-text` (через `canvas-render`).
- Транзитивные через `cosmic-text`: `rustybuzz` 0.14 (RUSTSEC-2026-0206 unmaintained, `deny.toml` ignore), `ttf-parser` 0.20/0.21/0.25 (RUSTSEC-2026-0192 unmaintained, 3 копии — `[bans] multiple-versions = "warn"`), `swash`, `zeno`, `skrifa`, `read_fonts`, `font_types`, `ab_glyph`, `ab_glyph_rasterizer`, `owned_ttf_parser`, `unicode_*` (15+ крейтов). Объём wasm-бина (M8/W12): 7.0 МБ raw / 1.54 МБ brotli (wasm-port.md §8.8); из них cosmic-text+deps ≈ 250 КБ raw.
- ADR-0014 (2026-09-24) принял гибрид `trait LayoutBackend` + taffy opt-in + staged migration (FR-067 P1/P2/P3). Статус ADR-0013 — `заменено (ADR-0014)`.

## Критический разбор ADR-0014 (взгляд группы экспертов)

ADR-0014 — технически корректен, но содержит 7 структурных слабостей, которые требуют явной стратегии. Разбор по ролям:

### Архитектор (design critic)

**C1. Гибрид `trait LayoutBackend` + 2 backend'а — двойная поддержка навсегда.** ADR-0014 §Решение п.5 ставит staged migration P1/P2/P3, но **не описывает финальное состояние**: `NativeBackend` остаётся как «fast-path/фолбэк» (P3). Это значит — два движка поддерживать бессрочно, G4-линты ×2 backend'а в CI навсегда, каждый баг вёрстки — отлаживать на двух движках. Цена гибрида не结束на — она становится постоянной.

**C2. Retained-дерево taffy vs immediate-mode — фундаментальный конфликт, не решён.** ADR-0014 §Решение п.3: «`TaffyBackend` пересоздаёт `TaffyTree` на кадр (аллокации есть, но taffy имеет инкрементальный кэш — для неизменных поддеревьев можно сохранить node-id между кадрами, опция P3)». Проблема: taffy кэш работает ТОЛЬКО при стабильных node-id между кадрами — а immediate-mode пересоздаёт всё. План P3 «опционально retained node-id» — фактически признание, что без retained-слоя taffy работает в 5–10× медленнее потенциального. Решение: либо признать immediate-overhead (документировать), либо ввести retained-слой (а это уже архитектурное изменение, не «опция P3»).

**C3. `SqueezeTail` ≠ `flex_shrink` — расхождение, замалчиваемое как «документированное».** ADR-0014 §Контракт-4: «`SqueezeTail` — хвост до 0 ширины, half-open hit. `flex_shrink: 1.0, min_width: 0` в taffy — min-content пол». На практике: what-if бар (FR-017/CR-015) использует `SqueezeTail` для деградации узкого слота; перевод на taffy даст визуально другой результат (элементы сжимаются до min-content, не до 0). Это не «осознанный выбор потребителя» — это регрессия, которую G4-линт не поймает (он проверяет выход за parent, не семантику сжатия).

### Performance engineer

**C4. Замер wasm ~376 КБ raw — без учёта маржинальности.** ADR-0013 §Замер цены: «изолированный стенд тянет собственные куски std/alloc, в реальном бандле приложения часть кода общая — маржинальная цена ниже, но порядок тот же». ADR-0014 принимает «полный бюджет» без пере-замера на реальном бандле canvas-web (7.0 МБ → ожидается 7.4 МБ). Риск: если маржинальность окажется выше (например, taffy `cssparser` уже есть в дереве через другой путь) — бюджет может быть превышен. Решение: обязать P1 замер на canvas-web cdylib ДО merge, не после.

**C5. Профиль кадра не замерен.** ADR-0014 §Валидация: «профиль кадра: 60 fps на 5 000 нод + UI релайаут ≤1 мс (SPEC §6.3) — сохраняется». Но taffy 1 000 узлов ≈ 329 мкс (ADR-0013 §Контекст) — это для retained с кэшем. Immediate-пересоздание `TaffyTree` на кадр × 10² узлов/поверхность × 9 полос = 10³ узлов на кадр. Ожидание: 0.3–1.0 мс — на грани бюджета. Решение: замер P1 на pilot-поверхностях обязателен; если > 1 мс — retained node-id в P2 (не опционально).

### Tech lead / стратегия зависимостей

**C6. Taffy — четвёртая «тяжёлая» UI-зависимость (после cosmic-text/rustybuzz/ttf-parser), все с RUSTSEC.** Сегодня в `deny.toml [advisories].ignore`: RUSTSEC-2024-0436 (paste), RUSTSEC-2026-0206 (rustybuzz), RUSTSEC-2026-0192 (ttf-parser). Taffy — через `slotmap`/`smallvec`/`cssparser`/`serde`, пока без RUSTSEC, но любой unmaintained-крейт в транзитивном дереве — риск. Решение: taffy как переход — вырезать к W4; cosmic-text — изолировать за trait (W2); rustybuzz/ttf-parser — мигрируют с cosmic-text.

**C7. Нет конечной цели «своя UI-библиотека».** ADR-0014 — план «как внедрить taffy»; владельцу нужен план «как от него уйти». Без явной стратегии — taffy останется навсегда (C1). Решение: ADR-0015 декларирует taffy как переход, FR-068 — волны W0–W4 с конкретной целью «zero external UI deps» к концу W4.

### UX-engineer (consumer perspective)

**C8. 10 HTML5 demo-layouts — критерий приёмки, но не описаны.** ADR-0014 §Решение п.6: «эталонный набор HTML5 demo-layouts (mdn/css-tricks топ-10) — критерий приёмки (FR-067 §Verification)». Но конкретный список из 10 layouts — не зафиксирован (упомянуты: sticky-header-column, sidebar-content-overflow-auto, flexbox-navbar-space-between, css-grid-12-col-responsive, masonry-lite, card-list-aspect-ratio, modal-position-fixed-viewport-clip, dropdown-flip, scrollable-list-virtualization, complex-form-layout). Решение: ADR-0015 фиксирует канонический список 10 + 5 дополнительных для W2.

**C9. Расширение G4-линта — без спецификации.** ADR-0014 §P2: «новые проверки: выход за parent, viewport, ClipRect-вложенность». Но: какие именно canonical сцены? Какие размеры окна? Сколько языков? Текущий G4 (PRD-0009 AC-3.1) — 3 окна × 2 языка = 6 прогонов. ADR-0014 удваивает до 12 (×2 backend'а). Решение: FR-068 W0 специфицирует canonical сцены (5 типовых UI-экранов × 3 окна × 2 языка = 30 прогонов × N backend'ов).

### Итог разбора

ADR-0014 — корректный тактический план (внедрить taffy быстро), но не стратегический (нет конечной цели, не решены конфликты, не определены финальные состояния). Принимается как **переходное решение** с явной декларацией: taffy вырезается к W4 FR-068; параллельно проектируется собственный layout-движок (минимум: flexbox + overflow/clip/scroll); cosmic-text изолируется за trait boundary (W2); kit.rs рефакторится в компонентное дерево (W3).

## Драйверы решения (требования)

- **S1. Taffy — переходное.** ADR-0014 принят как тактическое решение; ADR-0015 декларирует taffy вырезаемым к концу W4. Конечная цель: `cargo build --no-default-features` = 0 внешних UI-зависимостей (только `canvas-core`, `canvas-ui`, `canvas-render`).
- **S2. UI-паритет с web.** Канонический набор из 15 HTML5 demo-layouts (mdn/css-tricks топ-10 + 5 CanvasDesk-специфичных) рендерится 1:1 в canvas-ui (без GPU, через Painter + UiRect dump) — golden-снапшоты. Это «доказательство близости к web ui html5 без web-технологий».
- **S3. Минимум внешних зависимостей.** Снижение: cosmic-text → trait boundary (W2), taffy → свой layout (W3), rustybuzz/ttf-parser — мигрируют с cosmic-text. Цель W4: 0 внешних UI-runtime-зависимостей (только dev/test).
- **S4. Архитектура — компонентное дерево + retained-state.** kit.rs 2 757 строк → 6 компонентов × ~500 строк (`button.rs`, `panel.rs`, `dropdown.rs`, `text_field.rs`, `list.rs`, `modal.rs`); retained-state для инкрементального reflow (опционально W3, обязательно W4 если профиль покажет нужду).
- **S5. Поэтапный рефакторинг с проверяемыми результатами.** Каждая волна W0–W4 — отдельный коммит (или несколько), с явными гейтами: layout-линт (расширение G4 — выход за parent/viewport/silent-clips × 5 canonical сцен × 3 окна × 2 языка = 30 прогонов × N backend'ов), snapshot-тесты (Painter.items → строка-дамп, изменение — осознанный PR), perf-бюджет (reflow 1000 узлов < 1 мс; 60 fps на 5000 нод — SPEC §6.3).
- **S6. Cosmic-text — trait boundary.** `Shaper` trait в canvas-ui (W2); cosmic-text — default impl; миграция возможна позже (fontdue/ab_glyph + свой layout). Не блокер W0/W1/W3.
- **S7. Свой layout — минимум.** Flexbox (grow/shrink/basis/wrap) + overflow/clip/scroll + position:absolute/fixed. Grid — за taffy-опцией (если появится явный потребитель); полный CSS — non-goal.
- **S8. Двери открыты.** ADR-0014 (taffy) сохраняется как fallback; ADR-0013 (свои примитивы FR-062) — основа для `NativeBackend` → развивается в собственный layout-движок W3.

## Рассмотренные варианты

### Вариант A: принять ADR-0014 как есть, без долгосрочной стратегии

- Плюсы: минимум планирования; taffy быстро; фокус на реализации.
- Минусы: C1 (двойная поддержка навсегда), C6 (4-я тяжёлая зависимость), C7 (нет конечной цели). Taffy остаётся навсегда; внешние зависимости растут; архитектура не улучшается.
- Вердикт: отвергнут — владелец явно запросил долгосрочный план.

### Вариант B: отвергнуть ADR-0014, сразу свой layout-движок

- Плюсы: 0 новых зависимостей; чистая архитектура; нет «костыля».
- Минусы: 2–3 месяца до первого результата (flexbox grow/shrink/basis/wrap с тестами паритета); UI остаётся «причёсанным» на старых примитивах FR-062; баги позиционирования не закрыты.
- Вердикт: отвергнут — владелец явно выбрал «Taffy временно» (clarifying questions 2026-09-24).

### Вариант C: гибрид ADR-0014 + долгосрочная стратегия (ADR-0015 + FR-068) ✅ рекомендован

Архитектура:

1. **W0 — UI hygiene (пред-тафy)**: расширение G4-линта (выход за parent/viewport/silent-clips × 5 canonical сцен × 3 окна × 2 языка); snapshot-тесты Painter.items; perf-бенчмарк baseline; рефакторинг kit.rs — вынос `dropdown/tooltip/modal/toast_area` viewport-clamp в общий хелпер; удаление дублирующих ручных clamp'ов; «причесать UI» до taffy.
2. **W1 — taffy opt-in (ADR-0014 P1+P2)**: `trait LayoutBackend` + `NativeBackend` (перенос FR-062 1:1) + `TaffyBackend` skeleton + adapter; pilot на 2 поверхностях; 10 HTML5 demos; viewport-clamp + overflow/scroll на taffy. Taffy — как переходное решение, за фичей `taffy` (default off).
3. **W2 — cosmic-text trait boundary + свой layout-движок (flexbox-only, минимум)**: `Shaper` trait в canvas-ui (cosmic-text — default impl); `FlexLayoutEngine` в `canvas-ui/src/layout/flex.rs` (grow/shrink/basis/wrap, побитовая идентичность с taffy на совместимых политиках); `Overflow/Clip/Scroll` примитивы (поверх `Painter::ClipRect` из W1); 5 дополнительных HTML5 demos. `NativeBackend` переключается на `FlexLayoutEngine` (если фича `taffy` off — свой движок; если on — taffy). Taffy остаётся для Grid (если используется).
4. **W3 — своя UI-библиотека (компонентное дерево)**: kit.rs 2 757 → 6 компонентов × ~500 строк (`button.rs`, `panel.rs`, `dropdown.rs`, `text_field.rs`, `list.rs`, `modal.rs`); `Component` trait с `props() -> Props`, `layout(backend) -> Vec<UiRect>`, `paint(painter)`; retained-state для инкрементального reflow (если профиль кадра W2 покажет > 1 мс); миграция canvas-app/render на компонентное API.
5. **W4 — dep-минимизация (вырезание taffy)**: taffy вырезается (если `FlexLayoutEngine` покрывает все использованные фичи — замер по grep-aудиту `--features taffy` в коде); cosmic-text остаётся за `Shaper` trait (миграция на fontdue/свой layout — отдельный ADR при появлении триггера); rustybuzz/ttf-parser — мигрируют с cosmic-text; final state: `canvas-ui` без taffy, cosmic-text — единственная внешняя UI-runtime-dep (за trait).

- Плюсы:
  - **S1 закрывается**: taffy вырезается к W4; `cargo build --no-default-features` = 0 внешних UI-runtime-deps (кроме cosmic-text за trait).
  - **S2 закрывается**: 15 HTML5 demos (10 W1 + 5 W2) — golden-снапшоты 1:1.
  - **S3 закрывается**: cosmic-text → trait (W2), taffy → вырезан (W4).
  - **S4 закрывается**: kit.rs → компонентное дерево (W3), retained-state.
  - **S5 закрывается**: каждая волна — гейты layout-линт + snapshot + perf.
  - **S6 закрывается**: `Shaper` trait (W2), cosmic-text — default impl.
  - **S7 закрывается**: `FlexLayoutEngine` минимум (W2), Grid — за taffy-опцией (W4 — вырезается, если не используется).
  - **Двери открыты**: ADR-0014 (taffy) сохраняется как fallback W1→W4; ADR-0013 (FR-062) — основа для `FlexLayoutEngine`.
- Минусы:
  - **Длительность**: 6–9 месяцев (W0 — 2–3 недели, W1 — 4–6 недель, W2 — 8–12 недель, W3 — 8–12 недель, W4 — 4–6 недель).
  - **Параллельная работа**: W1 (taffy) и W2 (свой layout) — частично параллельны (W2 стартует после W0, не ждёт W1); требует координации (один агент — layout, другой — kit).
  - **Риск W4**: если `FlexLayoutEngine` не покроет использованные фичи taffy (Grid) — taffy не вырезается; решение: grep-аудит `--features taffy` в W3, удаление неиспользуемого.
  - **Двойные тесты**: G4-линты ×2 backend'а до W4 (тафy + свой); митигируется golden-снапшотами как общими оракулами.

### Вариант D: принять ADR-0014 + косвенная стратегия (без нового ADR)

- Плюсы: меньше документации; решения принимаются по факту.
- Минусы: C1/C7 (нет конечной цели); taffy остаётся навсегда; владелец не увидит «долгосрочного плана».
- Вердикт: отвергнут — владелец явно запросил стратегию.

## Решение

1. **Принять Вариант C** — ADR-0015 + FR-068 (волны W0–W4). ADR-0014 — переходное решение (taffy opt-in W1, вырезается W4). FR-067 (план staged миграции на taffy) поглощается FR-068 (поэтапный рефакторинг UI, включает W1).
2. **Декларировать taffy как переходное** в ADR-0014 — добавить в §Статус: «переходное решение (см. ADR-0015, FR-068 W4 — вырезание)». ADR-0014 не отменяется, но его горизонт — W1–W3 FR-068; к W4 taffy вырезается.
3. **Принять 5 волн рефакторинга** (FR-068):
   - **W0 — UI hygiene (2–3 недели)**: расширение G4-линта (5 canonical сцен × 3 окна × 2 языка = 30 прогонов; проверка выхода за parent/viewport/silent-clips); snapshot-тесты Painter.items (10 существующих kit-компонентов → строка-дамп); perf baseline; kit.rs hygiene (общий хелпер viewport-clamp для dropdown/tooltip/modal/toast_area); устранение найденных G4-нарушений. **Цель**: «причесать UI» до taffy.
   - **W1 — taffy opt-in (4–6 недель, = ADR-0014 P1+P2)**: `trait LayoutBackend` + `NativeBackend` (перенос FR-062 1:1) + `TaffyBackend` skeleton + adapter; pilot на 2 поверхностях; 10 HTML5 demos; viewport-clamp + overflow/scroll на taffy; `PaintItem::ClipRect` в Painter. Taffy — за фичей `taffy` (default off).
   - **W2 — cosmic-text trait boundary + свой layout-движок (8–12 недель)**: `Shaper` trait (cosmic-text — default impl, мок для тестов); `FlexLayoutEngine` (grow/shrink/basis/wrap, побитовая идентичность с taffy на совместимых политиках); `Overflow/Clip/Scroll` примитивы; 5 дополнительных HTML5 demos. `NativeBackend` переключается на `FlexLayoutEngine` (если фича `taffy` off). Taffy остаётся для Grid.
   - **W3 — своя UI-библиотека (8–12 недель)**: kit.rs 2 757 → 6 компонентов × ~500 строк; `Component` trait (`props/layout/paint`); retained-state (если профиль W2 покажет > 1 мс); миграция canvas-app/render на компонентное API.
   - **W4 — dep-минимизация (4–6 недель)**: taffy вырезается (если `FlexLayoutEngine` покрывает использованные фичи — grep-аудит `--features taffy`); cosmic-text остаётся за `Shaper` trait; rustybuzz/ttf-parser мигрируют с cosmic-text; final state: `cargo build --no-default-features` = 0 внешних UI-runtime-deps (кроме cosmic-text за trait).
4. **Канонический набор 15 HTML5 demos** (S2): 10 в W1 (mdn/css-tricks топ-10: sticky-header-column, sidebar-content-overflow-auto, flexbox-navbar-space-between, css-grid-12-col-responsive, masonry-lite, card-list-aspect-ratio, modal-position-fixed-viewport-clip, dropdown-flip, scrollable-list-virtualization, complex-form-layout) + 5 в W2 (CanvasDesk-специфичные: palette-grid-multiline, whatif-bar-squeeze-tail-parity, kit-gallery-tab-focus, search-overlay-viewport-clip, fr061-tabular-body-grid).
5. **Layout-линт G4+** (S5): 5 canonical сцен × 3 окна (1280×800/1024×640/800×560) × 2 языка (RU/EN) = 30 прогонов; × N backend'ов (default NativeBackend, W1+ TaffyBackend, W2+ FlexLayoutEngine); проверки: (1) `UiRect::intersection(rect, parent).is_some()` для всех видимых элементов; (2) `UiRect::intersection(rect, viewport).is_some()` для L4+; (3) `Painter::ClipRect` вложенность корректна; (4) 0 silent-clips (manual `take`/`break`/`truncate` в мигрированном коде — grep-аудит).
6. **Snapshot-тесты** (S5): `Painter.items()` → нормализованная строка-дамп (округление до целого ui px, сортировка по `(x, y, w, h)`); 10 kit-компонентов × 3 состояния (default/hover/disabled) × 2 языка = 60 эталонов; изменение — осознанный PR с diff в коммите.
7. **Perf-бюджет** (S5): reflow 1000 узлов < 1 мс; 60 fps на 5000 нод (SPEC §6.3) — сохраняется на каждой волне; замер в CI (если доступно) или manual-профиль.
8. **Cosmic-text — trait boundary** (S6): `Shaper` trait в `canvas-ui/src/shaper.rs` (W2); cosmic-text — default impl за `#[cfg(not(feature = "mock-shaper"))]`; mock для тестов; миграция на fontdue/ab_glyph + свой layout — отдельный ADR при появлении триггера (RUSTSEC-эскалация или явная потребность).

## Обоснование

Решение закрывает запрос владельца стратегически: ADR-0014 — тактический план «внедрить taffy быстро»; ADR-0015 + FR-068 — стратегия «прийти к своей UI-библиотеке с минимумом внешних зависимостей». Критический разбор выявил 9 слабостей ADR-0014 (C1–C9) — все закрываются: C1 (двойная поддержка → taffy вырезается W4), C2 (retained конфликт → явная декларация immediate overhead W1, retained-state W3 если нужно), C3 (`SqueezeTail` ≠ `flex_shrink` → `FlexLayoutEngine` W2 реализует `SqueezeTail` дословно, taffy — fallback), C4 (замер wasm → обязать P1 замер на canvas-web), C5 (профиль кадра → гейты каждой волны), C6 (4-я тяжёлая зависимость → taffy вырезается W4, cosmic-text за trait W2), C7 (нет конечной цели → ADR-0015 декларирует zero-external-UI-deps W4), C8 (10 HTML5 demos → канонический список 15 в ADR-0015), C9 (G4 расширение → 5 canonical сцен × 3 окна × 2 языка = 30 прогонов × N backend'ов).

Чем пожертвовали: (1) длительность 6–9 месяцев — но волны поставляемы независимо (W0 — 2–3 недели, виден результат); (2) двойные тесты до W4 — цена гибрида, митигируется golden-снапшотами; (3) риск W4 (если FlexLayoutEngine не покроет Grid) — taffy остаётся за фичей для Grid-only, что приемлемо (S7 — Grid за taffy-опцией).

## Последствия

- **Положительные:** конечная цель «своя UI-библиотека» с минимумом внешних зависимостей (S1/S3); UI-паритет с web (S2 — 15 HTML5 demos); архитектура — компонентное дерево (S4); проверяемые результаты на каждой волне (S5 — layout-линт + snapshot + perf); cosmic-text изолирован (S6 — миграция возможна без переписывания потребителей); двери открыты (ADR-0014 как fallback, ADR-0013 как основа FlexLayoutEngine).
- **Отрицательные / принятые риски:** длительность 6–9 месяцев; двойные тесты до W4; риск W4 (Grid не покрыт) — митигируется grep-аудитом W3; параллельная работа W1/W2 требует координации.
- **Нейтральные (инерция):** ADR-0014 не отменяется — поглощается FR-068 W1; ADR-0013 остаётся как основа `NativeBackend`/`FlexLayoutEngine`; PRD-0009 V-5 интерфейс стабилен.

## Валидация

- **Каждая волна FR-068** — отдельный коммит (или несколько) с гейтами: `cargo test --workspace` (default + `--features taffy` для W1+, `--features flex-engine` для W2+); `cargo clippy -D warnings`; `cargo fmt --check`; `scripts/wasm_gate.sh --check`; замер wasm raw/brotli; layout-линт G4+ (30 прогонов × N backend'ов); snapshot-тесты (60 эталонов); perf-бюджет (reflow 1000 узлов < 1 мс).
- **W4 final state**: `cargo build --no-default-features` — 0 внешних UI-runtime-deps (только `canvas-core`, `canvas-ui`, `canvas-render`); taffy вырезан (если `FlexLayoutEngine` покрывает использованные фичи); cosmic-text за `Shaper` trait; `cargo deny check` зелёный; 15/15 HTML5 demos golden зелёные; 60/60 snapshot-тестов зелёные; perf ≤ SPEC §6.3.
- **Эскалация**: если W2 (`FlexLayoutEngine`) не достигает побитовой идентичности с taffy на >80% совместимых политик — taffy остаётся (W4 откладывается); если W3 (компонентное дерево) ухудшает perf > SPEC §6.3 — retained-state обязателен; если W4 grep-аудит показывает использование Grid в >3 местах — taffy остаётся за фичей для Grid-only (S7 — приемлемо).

## История

- `2026-09-25` — агент: **принято владельцем** — приказ «начинай первый этап внедрения» (сессия 2026-09-25): волна W0 FR-068 реализована 4 параллельными агентами (kit.rs hygiene `viewport_clamp`, snapshot-тесты 60 эталонов, G4-линт 30 прогонов — 0 нарушений, perf baseline 45.1 μs < 1 мс), все гейты зелёные (workspace 1941 passed, wasm-gate). ADR-0014 — переходное решение (горизонт W1–W3, вырезание W4) — подтверждено реализацией.
- `2026-09-24` — агент: создан ADR по запросу владельца «проанализировать ADR-0014 критическим взглядом группы экспертов; спланировать дальнейший рефакторинг и переход на полностью свою UI-библиотеку». Критический разбор выявил 9 слабостей (C1–C9); выбран Вариант C (гибрид ADR-0014 + долгосрочная стратегия W0–W4). FR-068 — план волн. ADR-0014 — переходное решение, статус дополняется «переходное (см. ADR-0015, FR-068 W4 — вырезание taffy)».
