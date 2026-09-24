# ADR-0014: Переоткрытие ADR-0013 — taffy как opt-in бэкенд вёрстки за trait LayoutBackend

- **Статус:** предложено (анализ по прямому запросу владельца 2026-09-24; утверждение решения — за владельцем)
- **Дата:** 2026-09-24
- **Участники:** владелец (запрос «спланировать быструю и более глубокую интеграцию taffy ui для canvas-ui, чтобы максимизировать адаптивность вёрстки всех ui-элементов, исключить баги с неправильным визуальным позиционированием элементов за пределами родительских элементов или относительно вьюпорта, и прийти наиболее близко к возможностям web ui как html5, но без web-технологий»), агент (анализ кода и масштабов миграции, пере-замер цены taffy, сопоставление с FR-062)
- **Связанные:** ADR-0013 (заменяемое решение), PRD-0009 §7.2 V-3 / §9.5 / §7.4 (дверь taffy), FR-053 (U3 — layout-примитивы F-7), FR-056 (scissor клиппинг), FR-062 (FR-0013-recommended — собственные layout v2: F-13…F-18 выполнено), FR-059/060 (миграция волны 2), FR-061 (этап E kit-Row — потенциальный потребитель taffy), FR-051 (UiLayer 9 полос — ортогонален движку вёрстки), SPEC §6.3 (бюджеты UI), `docs/ui-kit.md`, `docs/DEPENDENCIES.md` (§3→§2 — миграция taffy), ADR-0008 (лицензионный allowlist — MIT OK), ADR-0011 (wasm-гейт), FR-067 (план реализации — ADR-0014 → FR-067)

## Контекст

Владелец (сессия 2026-09-24) запросил: *«спланировать быструю и более глубокую интеграцию taffy ui для canvas-ui, чтобы максимизировать адаптивность вёрстки всех ui-элементов, исключить баги с неправильным визуальным позиционированием элементов за пределами родительских элементов или относительно вьюпорта и др. Так же нужно прийти наиболее близко к возможностям относительно вёрстки и взаимодействию с интерфейсом, которые даёт web ui, как тот же html5, но только без применения web-технологий»*.

Контекст пере-оценки ADR-0013 (создан 2026-09-23, через день):

- **ADR-0013 уже выбрал Вариант B** (усиление собственных примитивов по FR-062). Решение мотивировано: (D1) G7-бюджет ≤100 КБ на волну — taffy flexbox-only ≈ весь остаток, grid +270 КБ сверх; (D2) immediate-mode против retained-дерева; (D5) «мощности вёрстки» не было в списке реальных дефектов UI-слоя; (D7) интерфейс V-5 совместим со слотами taffy, дверь оставлена открытой через триггеры T1–T4 и протокол эскалации §9.5 PRD-0009.
- **FR-062 выполнен** (2026-09-23, коммиты за сессию web-880a3b7e): F-13 measured-дети, F-14 grow/End, F-15 Wrap, F-16 grid_cells (равные треки), F-17 FocusRing×WidgetState, F-18 golden-снапшоты; витрина + Tab-фокус; гейты зелёные; уточнены контракты F-13/F-15.
- **FR-060 выполнен** (2026-09-23): миграция волны 2 (autolink/palette/explain/dialog/menu/hotkeys) на kit+Painter; замер +1.2 КБ ≤ 100 КБ.
- **Запрос владельца меняет ядро драйверов**: цель — «близость к web ui html5 без web-технологий» + устранение классов багов позиционирования (выход за parent, viewport, z-order, текст-мера, resize, scroll/overflow). Это переводит D5 ADR-0013 («мощности вёрстки не было в дефектах») в новое состояние: владелец явно декларирует класс дефектов, который ADR-0013 не закрывает в полной мере.

Замеры taffy 0.14.0 (ADR-0013 §Замер цены) **подтверждаются как воспроизводимые**: flexbox-only ≈ 108 КБ wasm raw; flexbox+grid ≈ 376 КБ raw (≈ 103/180 КБ gzip). Цены не изменились. Что изменилось — **готовность владельца принять цену**: выбран «полный бюджет» (flexbox+grid в один заход, +376 КБ raw / ~180 КБ gzip), с явным разрешением превысить G7-бюджет ≤100 КБ на волну 2.

Текущее состояние `canvas-ui` (2026-09-24, после FR-062):

- `layout.rs` (1 114 строк): `Row/Column{gap, main, cross, policy}`, `Child::fixed/spacer/flexible`, `RowPolicy{Fit, SqueezeTail, Wrap}`, `MeasuredItem::{Fixed, Text, Spacer}`, `grid_cells` (равные треки), `stack/constrain/pad/Custom`. CSS-подобные возможности: fixed/auto-измеренный/flex-grow/flex-wrap/равная 2D-сетка. **Нет**: per-child shrink/basis, неравных треков Grid, span, minmax, auto-flow, percentage sizing, aspect-ratio, calc(), инкрементального релайаута, retained-дерева.
- `kit.rs` (2 758 строк): Panel/Button/IconButton/Dropdown/Chip/Toast/Tooltip/Modal/TextField/Switch/Card/Icon/list+ScrollState — все потребляют `layout.rs` через `Row/Column::lay_out(slot, &[Child])`. ~62 мест `Child::fixed`+`lay_out` в `kit.rs`, всего **562 мест** использования layout API по workspace (canvas-app/canvas-render/canvas-ui).
- `geometry.rs`: `UiRect{x,y,w,h}` — примитив чистый, не меняется.
- `kit.rs` уже содержит ручные viewport-clamp'ы для dropdown/tooltip/modal/toast (`dropdown_menu/tooltip/modal/toast_area` принимают `viewport: UiRect` и явно `max(viewport.x)`/`min(viewport.right())` — примеры «самописной полупроводимости», которые taffy не заменяет: `position: absolute`/`fixed` — отдельный слой функциональности поверх taffy).
- `paint.rs`: `Painter` с `PaintItem::{Rect, Text}` — draw-порядок = порядок вызовов; слои `UiLayer` (9 полос) — ортогональны layout-движку (D8 ADR-0013).
- `paint.rs` НЕТ `clip_rect` / `overflow:hidden` / scroll-containers — клиппинг сегодня на стороне `canvas-render` (scissor FR-056); `Painter` рисует прямоугольники, текст и не знает о viewport-clip внутри раскладки.

## Драйверы решения

- **N1. Класс багов позиционирования — явно декларирован владельцем.** Выход за parent (нет overflow-clip в layout), выход за viewport (только ручные clamp'ы в dropdown/tooltip/modal/toast), z-order (решён `UiLayer`, но без per-element stacking-context), текст-мера (FR-061 T9 — прецедент наложения из-за расхождения замера и шейпинга), resize/reflow (ручной пересчёт без % sizing), scroll/overflow (`ScrollState` ручной, без HTML `overflow:auto` семантики).
- **N2. Цель «близость к web ui html5 без web-технологий»** — требует CSS-совместимой семантики: full flexbox (grow/shrink/basis/wrap), CSS Grid 2D (неравные треки/span/auto-flow/minmax), auto-sizing (min/max/fit-content), overflow/clip/scroll, position %/ratio/aspect-ratio, sticky/transform, z-index/isolation/blend. Taffy 0.14.0 покрывает ~80% (flexbox+grid+block, без position:sticky, без overflow:auto как сущности — эти надстройки строятся поверх).
- **N3. Готовность принять прирост wasm ~376 КБ raw / ~180 КБ gzip** — решение владельца «полный бюджет, допустимо 1 раз». После FR-060/062 wasm-бандл ~7.0 МБ raw / 1.54 МБ brotli (ADR-0013 §Валидация); +376 КБ raw → ~7.4 МБ / ~1.72 МБ brotli — ≤8 МБ raw / ≤4 МБ brotli (wasm-port.md §8.8) **сохраняется** с запасом.
- **N4. immediate-mode D2 сохраняется** — taffy `TaffyTree` retained-дерево конфликтует с пересборкой кадра; интеграция требует гибридной модели (см. §Рассмотренные варианты — Вариант D).
- **N5. Триггеры T1–T4 ADR-0013 формально не сработали** — FR-062 закрыл auto-размеры/flex/wrap/Grid-lite; `grep` `Child::fixed`+`width_of` ~30 порога не превышен; tabular stage FR-061 — на этапе E (`kit-Row`) использован `RowGuides`, не полный Grid; профиль кадра <1 мс. **Но цель владельца «близость к web ui html5» — это по сути T4 (CSS-compatible семантика) как решение, а не ожидание триггера.** ADR-0014 явно декларирует: цель владельца преобладает над триггерами.
- **N6. Дверь PRD-0009 §9.5 исполнена** — настоящий ADR (отдельный документ до реализации) и есть тот самый протокол эскалации; интерфейс V-5 совместим со слотами taffy (потребители не переписываются).
- **N7. Taffy — зрелый продакшн-движок** (Servo, Bevy, Zed, Slint, Lapce, Floem — ADR-0013 §Контекст); MIT; `default-features = false` no_std-совместим; wasm-биндинги не требуются (Rust → Rust).
- **N8. Гибридная модель** (выбранная владельцем) — `trait LayoutBackend` + два бэкенда (current primitives default, taffy opt-in) + **staged migration** (P1: 2–3 тяжёлых поверхностей; P2: расширение; P3: полная миграция). Решает «быстрая и более глубокая интеграция» одновременно: P1 быстрый pilot, P2/P3 углубление.

## Рассмотренные варианты

### Вариант D: гибрид — `trait LayoutBackend` + taffy opt-in за cargo-фичей + staged migration ✅ рекомендован

Архитектура:

1. **`trait LayoutBackend`** в `canvas-ui/src/layout.rs` — абстракция движка вёрстки. Методы: `lay_out_row(slot, items) -> Vec<UiRect>`, `lay_out_column(slot, items) -> Vec<UiRect>`, `lay_out_measured(slot, items, m, fs, family, size) -> Vec<UiRect>`, `lay_out_grid(slot, tracks, gap) -> Vec<UiRect>`, `available_features() -> LayoutFeatures` (битовая маска: flex/shrink/basis/wrap/grid-2d/auto-sizing/overflow/aspect-ratio/percent/sticky). Существующие `Row/Column/grid_cells` становятся методами этого трейта (или adapters) — сигнатуры потребителей не меняются (`Child::fixed/spacer/flexible`, `MeasuredItem`, `RowPolicy`, `MainAlign/CrossAlign` — стабильны).
2. **`struct NativeBackend;`** (default) — обёртка над существующими `Row/Column/grid_cells` в `layout.rs` (текущая реализация переносится в методы трейта 1:1 — поведение идентично, G4-тесты продолжают работать).
3. **`struct TaffyBackend { tree: TaffyTree }`** (за `#[cfg(feature = "taffy")]`) — retained-дерево taffy с адаптером: `Row/Column/Child/MeasuredItem` → `Style` taffy; `lay_out_*` строит `TaffyTree` (один кадр), `compute_layout(root, AvailableSpace::Definite(slot.size()))`, читает `tree.layout(node)` → `Vec<UiRect>`. measure-функции на текстовых узлах — через `TextMeasurer` (тот же шейпинг, что у рендера).
4. **Фича `taffy = ["dep:taffy"]`** в `canvas-ui/Cargo.toml`; `taffy = { version = "0.14", default-features = false, features = ["std", "taffy_tree", "flexbox", "grid", "block"] }` в `[workspace.dependencies]`. **Вне фичи** (default сборка) — `TaffyBackend` не компилируется, wasm-бандл не несёт taffy; **с фичей** — taffy активен, потребитель выбирает backend через `LayoutBackend::taffy()` или `LayoutBackend::native()`.
5. **Staged migration** (FR-067): P1 — `trait` + 2 backend'а + pilot на `kit_ui::explain_dialog` (модальный диалог, просто переводится) и `fr061_row_grid` (tabular body — там нужны неравные треки Grid, T2-триггер ADR-0013); P2 — перевод `palette/palette_dropdown/whatif_bar/toolbar` (viewport-clamp поверхности); P3 — полная миграция kit + app.rs (`debug_overlay/docs_ui/template_ui/settings_ui/whatif_ui/scheme_gallery_ui/kit_ui` — ~562 мест).

- Плюсы:
  - **Цель N2 закрывается**: taffy даёт full flexbox + CSS Grid 2D + auto-sizing + content-size — покрывает ~80% web-фич из коробки. Остальные (overflow:auto, sticky, transform) — надстройки поверх (см. FR-067 P2/P3).
  - **Цель N1 закрывается**: taffy `overflow: hidden` + measure-функции = «контент в parent»; viewport-clamp остаётся в `kit.rs` (taffy не знает о viewport — это слой выше).
  - **N4 сохраняется**: immediate-mode — `TaffyBackend` пересоздаёт `TaffyTree` на кадр (аллокации есть, но taffy имеет инкрементальный кэш — для неизменных поддеревьев можно сохранить node-id между кадрами, опция P3).
  - **Дверь N6 исполнена**: потребители НЕ переписываются (`Row/Column::lay_out` сигнатуры стабильны); фича opt-in (default off) — B2B-сборка без taffy, G7-бюджет сохранён.
  - **Staged** N8: P1 pilot проверяет гипотезу на 2 поверхностях; при успехе — P2/P3 углубление; при провале — откат, ADR-0013 остаётся в силе.
  - **R-6 (порог входа агентов)** — трейт даёт одну модель вёрстки (`LayoutBackend`); бэкенд прозрачен — агент пишет `Row{gap, main}.lay_out(slot, &children)` независимо от того, кто считает rect'ы.
- Минусы:
  - **+376 КБ wasm raw** (~180 КБ gzip) при активации фичи. Готово владельцем (N3); на default-сборке 0 прироста.
  - **Двойные тесты**: каждая поверхность, переведённая на taffy, тестируется с обоими backend'ами (`cargo test --features taffy` + default) — G4-линты на canonical сценах 3 окна × 2 языка прогоняются на обеих сборках. Митигируется: golden-снапшоты FR-062 F-18 — те же оракулы; добавляется параметризация backend'а.
  - **Семантические расхождения**: `SqueezeTail` (хвост до нулевой ширины, half-open hit) ≠ `flex-shrink` (min-content пол). Решение (FR-067): `SqueezeTail` остаётся как семантика слоя; в `TaffyBackend` мапится на `flex_shrink: 1.0` + `min_width: 0` (поведение не дословно — деградация видна как «эллипсис в длине», не «нулевой хвост»); G4-линт ловит расхождение; потребитель выбирает backend осознанно.
  - **Цена адаптера**: `Row → Style` taffy — перевод в каждой раскладке (аллокации `Style`-объектов). На immediate-пересборке ~10² узлов/поверхность — десятки мкс; taffy кэш срабатывает только при retained node-id (опция P3).

### Вариант A: taffy как единственный движок (полная замена layout.rs)

- Плюсы: одна модель вёрстки; нет двойных тестов; max выигрыш сразу.
- Минусы: big-bang миграция 562 мест за раз (R-6, риск регрессий G4 на canonical сценах); **нарушает явный выбор владельца** «гибрид API + staged подход» (Remarks к вопросам); `SqueezeTail`/`RowPolicy::Fit` семантика теряется без адаптера.
- Вердикт: отвергнут — владелец явно выбрал гибрид.

### Вариант B: продолжить FR-062 — усилить собственные примитивы до CSS-паритета

- Плюсы: 0 КБ wasm; 0 новых зависимостей; G7 сохранён.
- Минусы: закрытие ~80% CSS-семантики самописным кодом — каждый % / sticky / overflow:auto / aspect-ratio = ~50–100 строк + тесты + литы; «самописный flexbox растёт» (ADR-0013 §Последствия); срок до паритета с taffy ~ 5–10 FR-волн (4–6 недель работы); готовность taffy — это спека + 7 продакшн-потребителей.
- Вердикт: отвергнут владельцем — цель «близость к web ui html5» требует спек-семантики, а не самописного клонирования; FR-062 F-13…F-18 остаётся как `NativeBackend` в гибридной модели.

### Вариант C: taffy только для grid (flexbox — собственный)

- Плюсы: +270 КБ только за grid (flexbox-выигрыши уже закрыты FR-062); меньше прирост.
- Минусы: 2 модели вёрстки одновременно (`Row/Column` собственные + `grid_cells` taffy) — R-6 ухудшается; `MeasuredItem` кэш-интеграции дублируется.
- Вердикт: отвергнут — усложняет модель; taffy flexbox + grid стоят +108 КБ к уже оплаченной grid-части — лучше взять полный taffy за ту же архитектурную цену.

## Решение

1. **Переоткрыть ADR-0013**: статус ADR-0013 → `заменено (ADR-0014)`. Триггеры T1–T4 сохраняются как формальные, но цель владельца «близость к web ui» преобладает — это **T4-priority** (CSS-compatible семантика как явная цель продукта, а не эскалация).
2. **Принять Вариант D** — гибридный `trait LayoutBackend` + два бэкенда (`NativeBackend` default, `TaffyBackend` opt-in за фичей `taffy`) + staged migration (FR-067 P1/P2/P3). Интерфейс V-5 PRD-0009 остаётся стабильным — потребители не переписываются.
3. **Принять полный бюджет** — фича `taffy` включает flexbox + grid + block (`features = ["std", "taffy_tree", "flexbox", "grid", "block"]`). Замер на реальном бандле обязателен до merge (превышение ≤376 КБ raw / ~180 КБ gzip над базовой сборкой; ≤8 МБ raw / ≤4 МБ brotli §8.8 wasm-port.md).
4. **Фича `taffy` выключена по умолчанию** — B2B-сборка без taffy сохраняет 0 прироста wasm и 0 новых зависимостей (zero-dep инвариант PRD-0009 G7 для default). Wasm-гейты `scripts/wasm_gate.sh --check` и `mcp_wasm_gate.sh --check` обязаны быть зелёными на каждой фазе FR-067 — как без фичи (default), так и с `--features taffy` (если компилируется на wasm32 — taffy заявляет no_std-совместимость; проверка).
5. **Staged migration** (FR-067):
   - **P1 (1 коммит, ~3–5 сессий агента)**: `trait LayoutBackend` + `NativeBackend` (перенос текущих Row/Column/grid_cells в методы трейта 1:1) + `TaffyBackend` skeleton + adapter `Row/Column/Child/MeasuredItem → taffy::Style` + pilot на 2 поверхностях (`explain_dialog` — простой перевод; `fr061_row_grid` — T2-триггер, Grid неравные треки). Гейты: G4-линты + golden FR-062 F-18 + замер wasm с/без фичи.
   - **P2 (1–2 коммита, ~5–8 сессий)**: перевод viewport-clamp поверхностей (`palette/dropdown/tooltip/modal/toast_area` — `position: absolute` семантика через адаптер `kit::viewport_*`) + scroll/overflow containers (`ScrollState` → `overflow:auto` семантика через taffy + клиппинг Painter). Расширение G4-линта на новые типы дефектов (выход за parent, viewport).
   - **P3 (2–3 коммита, ~10+ сессий)**: полная миграция kit.rs (~62 мест) + canvas-app (~500 мест) + canvas-render (~30 мест) на taffy backend. Опционально — retained node-id кэш для инкрементального релайаута (если профиль кадра покажет瓶颈).
6. **Эталонный набор HTML5 demo-layouts** (mdn/css-tricks топ-10) — критерий приёмки (FR-067 §Verification): golden-снапшоты rect'ов на 10 layouts (столбец с sticky-header, sidebar + content с overflow:auto, flexbox navbar с SpaceBetween, CSS Grid 12-col responsive, masonry-lite, card-list с aspect-ratio, modal с position:fixed + viewport-clip, dropdown с flip, scrollable list с virtualization, complex form layout) рендерятся 1:1 в canvas-ui (без GPU, через Painter + UiRect dump).
7. **Лицензии**: taffy 0.14 MIT — в allowlist ADR-0008; транзитивные зависимости (slotmap, smallvec, cssparser, serde) — все MIT/Apache-2.0; `cargo deny check` обязателен на каждой фазе. Taffy мигрирует из §3 кандидатов DEPENDENCIES.md в §2 прямых прод-зависимостей (после merge P1).

## Обоснование

Решение закрывает запрос владельца архитектурно, а не тактически: цель «близость к web ui html5 без web-технологий» требует **CSS-совместимой семантики вёрстки** (N2), которую собственные примитивы FR-062 дают на ~80% ценой ~сотен строк на каждую недостающую фичу. Taffy — спека + продакшн-потребители (Servo/Bevy/Zed/Slint — N7), не «очередной самописный flexbox». Гибрид `trait LayoutBackend` (N8 staged) — решает «быстрая и более глубокая интеграция» одновременно: P1 — pilot на 2 поверхностях проверяет гипотезу (быстрая), P2/P3 — углубление до полной миграции (более глубокая). Фича opt-in (default off) сохраняет B2B zero-dep инвариант PRD-0009 G7 для default-сборки; «полный бюджет» ~376 КБ при включении — готов владельцем (N3), ≤8 МБ raw / ≤4 МБ brotli §8.8 wasm-port.md сохраняется с запасом. Триггеры T1–T4 ADR-0013 формально не сработали, но цель владельца — это **T4 как цель продукта**, а не ожидание эскалации; ADR-0014 явно декларирует приоритет целей владельца над триггерами.

Чем пожертвовали: (1) ~376 КБ wasm при активации фичи — готово владельцем; (2) двойные тесты (G4-линты на 2 backend'ах) — цена гибрида, митигируется golden-снапшотами FR-062 F-18 как общими оракулами; (3) `SqueezeTail` ≠ `flex_shrink` — расхождение документировано, потребитель выбирает backend осознанно; (4) retained-дерево taffy vs immediate-mode D2 — решается пересозданием `TaffyTree` на кадр (P1) с опцией retained node-id кэша в P3 (если профиль покажет нужду).

## Последствия

- **Положительные:** CSS-совместимая семантика вёрстки (N2); устранение классов багов позиционирования (N1 — parent/viewport/text-vs-measure через overflow-clip и measure-функции taffy; z-order — `UiLayer` + per-element stacking context в P2); зрелый движок (Servo/Bevy/Zed — N7); staged migration без big-bang (N8); B2B zero-dep инвариант сохранён (default off); дверь PRD-0009 §9.5 исполнена формально (отдельный ADR до реализации).
- **Отрицательные / принятые риски:** +376 КБ wasm raw при активации (N3 — готово); двойные тесты (G4-линты ×2 backend'а) — митигируется golden-снапшотами; `SqueezeTail` ≠ `flex_shrink` — документированное расхождение; адаптер `Row → Style` taffy — аллокации на кадр (метигируется retained node-id P3).
- **Нейтральные (инерция):** ADR-0013 заменён, но не отменён — `NativeBackend` = FR-062 F-13…F-18 остаётся как default backend; V-5 интерфейс PRD-0009 стабильный; `UiLayer` (FR-051) и `Painter` (FR-057) ортогональны — движок вёрстки не меняет слои/capture/draw-порядок (D8 ADR-0013).

## Валидация

- **Замер taffy 0.14.x на canvas-ui**: после P1 FR-067 — `cargo build --release --target wasm32-unknown-unknown --features canvas-ui/taffy` (через canvas-web cdylib) + `ls -l target/wasm32-unknown-unknown/release/*.wasm`. Ожидание: raw +370…380 КБ к default-бандлу; gzip +170…190 КБ. Бюджет: ≤8 МБ raw / ≤4 МБ brotli (wasm-port.md §8.8) — сохраняется с запасом ~1 МБ. При превышении — явное решение владельца (новый ADR или сужение фичи: grid-only / flexbox-only).
- **G4-линты на 2 backend'ах**: `cargo test -p canvas-ui --features taffy --test g4_layout_lint` — canonical сцены 3 окна × 2 языка × 2 backend'а = 12 прогонов; 0 переполнений текста, 0 выходов rect'ов за вьюпорт (PRD-0009 AC-3.1).
- **Golden-снапшоты FR-062 F-18**: те же оракулы на `NativeBackend` и `TaffyBackend` — побитово идентичны для тех же политик (`Fit`/`Start`/`End`/`SpaceBetween` — где семантика совпадает; для `SqueezeTail`/`Wrap` — задокументированное расхождение).
- **HTML5 demo-layouts** (FR-067 §Verification): 10 эталонных layouts (mdn/css-tricks топ-10) рендерятся в `canvas-ui::tests::html5_demo_golden` через `Painter` + `UiRect` dump — golden-снапшоты в `tests/golden/html5_*.txt`.
- **Wasm-гейты**: `scripts/wasm_gate.sh --check` (default — taffy не включён) + `scripts/mcp_wasm_gate.sh --check` — зелёные на каждой фазе. С фичей `taffy` — дополнительный замер бандла в CI.
- **`cargo deny check`**: taffy + транзитивные deps — все в allowlist; новых лицензий нет.
- **`cargo clippy -D warnings`** + `cargo fmt --check` — на default и `--features taffy` сборках.
- **Профиль кадра** (SPEC §6.3): 60 fps на 5 000 нод + UI релайаут ≤1 мс — сохраняется; taffy 1 000 узлов ≈ 329 мкс (ADR-0013 §Контекст) — запас x3.
- **Эскалация при провале P1**: если golden-расхождения `TaffyBackend` vs `NativeBackend` на pilot-поверхностях >30% (не устранимы адаптером) — откат, ADR-0014 переходит в `отклонено`, ADR-0013 восстанавливается как действующий. Taffy не активируется в main, остаётся за фичей для экспериментов.

## История

- `2026-09-24` — агент: создан ADR по запросу владельца «спланировать быструю и более глубокую интеграцию taffy ui для canvas-ui». Пере-оценка ADR-0013 (создан 2026-09-23): замеры taffy подтверждаются (flexbox-only ~108 КБ, full ~376 КБ raw); готовность владельца принять «полный бюджет» (N3) меняет D1 ADR-0013. Вариант D (гибрид `trait LayoutBackend` + 2 backend'а + staged) выбран владельцем явно (Remarks: «Миграция нужен гибрид api и staged подход»). FR-067 (план реализации) — отдельный документ.
