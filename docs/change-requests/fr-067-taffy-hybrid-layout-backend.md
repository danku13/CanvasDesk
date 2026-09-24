# FR-067: Taffy как opt-in LayoutBackend — гибридная интеграция и staged миграция canvas-ui на CSS-семантику

- **Статус:** выявлено (план; ADR-0014 предложен, утверждается владельцем)
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (планирование); решения — владелец проекта
- **Источник:** запрос владельца 2026-09-24: «спланировать быструю и более глубокую интеграцию taffy ui для canvas-ui, чтобы максимизировать адаптивность вёрстки всех ui-элементов, исключить баги с неправильным визуальным позиционированием элементов за пределами родительских элементов или относительно вьюпорта и др. Так же нужно прийти наиболее близко к возможностям относительно вёрстки и взаимодействию с интерфейсом, которые даёт web ui, как тот же html5, но только без применения web-технологий». Ответы на clarifying questions: гибрид v2 (ADR-0013 переоткрыть) + полный бюджет (flexbox+grid) + staged migration (Remarks: «Миграция нужен гибрид api и staged подход») + ADR+FR форма.
- **Связанные задачи:** ADR-0014 (решение — гибридный `trait LayoutBackend` + taffy opt-in), ADR-0013 (заменённое решение — `NativeBackend` = FR-062 F-13…F-18), PRD-0009 §7.4 V-5 (интерфейс стабилен), §9.5 (дверь taffy исполнена ADR-0014), FR-053 U3 F-7 (layout-примитивы — основа `NativeBackend`), FR-056 (scissor клиппинг — переиспользуется P2), FR-057 (Painter/WidgetState — draw-слой ортогонален), FR-059/060 (волна 2 миграции — `NativeBackend` потребители), FR-062 (собственные layout v2 — `NativeBackend` основа), FR-061 этап E (kit-Row — pilot P1), FR-051 (UiLayer 9 полос — ортогонален), SPEC §6.3 (бюджеты UI), `docs/DEPENDENCIES.md` §3→§2 (taffy миграция после P1), `docs/ui-kit.md` (карта переносов), `docs/SPEC.md` §6.3 (комментарий о параллельном пути).
- **Создан:** 2026-09-24
- **Обновлён:** 2026-09-24
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

**Что добавляется.** Taffy 0.14 как opt-in бэкенд вёрстки `canvas-ui` за cargo-фичей `taffy`, через `trait LayoutBackend` с двумя реализациями:

- **`NativeBackend`** (default) — текущие примитивы FR-062 F-13…F-18 (`Row/Column{gap, main, cross, policy}`, `Child::fixed/spacer/flexible`, `MeasuredItem`, `RowPolicy{Fit, SqueezeTail, Wrap}`, `grid_cells`), перенесённые в методы трейта 1:1. Поведение идентично — G4-тесты FR-062 продолжают работать; B2B zero-dep инвариант сохранён (default сборка без taffy).
- **`TaffyBackend`** (за `#[cfg(feature = "taffy")]`) — retained-дерево taffy `TaffyTree` с адаптером `Row/Column/Child/MeasuredItem → taffy::Style`; `lay_out_*` строит дерево на кадр, `compute_layout(root, AvailableSpace::Definite(slot.size()))`, читает `tree.layout(node)` → `Vec<UiRect>`. measure-функции на текстовых узлах — через `TextMeasurer` (тот же cosmic-text шейпинг, что у рендера).

**Staged migration (3 фазы, ~18–25 сессий агента):**

- **P1 — `trait` + 2 backend'а + pilot на 2 поверхностях**: `trait LayoutBackend` в `layout.rs` с методами `lay_out_row/column/grid/measured`; `NativeBackend` (перенос текущих Row/Column/grid_cells 1:1); `TaffyBackend` skeleton + adapter; pilot на `kit_ui::explain_dialog` (простой модал) + `fr061_row_grid` (tabular body — T2-триггер ADR-0013, неравные треки Grid). Гейты: G4-линты + golden FR-062 F-18 на обоих backend'ах + замер wasm с/без фичи.
- **P2 — viewport-clamp + overflow/scroll**: перенос `kit::dropdown_menu/tooltip/modal/toast_area` на taffy-бэкенд с `position: absolute/fixed` семантикой через адаптер; `ScrollState` → `overflow:auto` (taffy не имеет scroll-native — надстройка: `overflow:hidden` в layout + `ScrollState` для offset,Painter clip-rect); расширение G4-линта на новые типы дефектов (выход за parent, viewport-clip — `UiRect::intersection` с viewport обязано быть непустым для всех видимых элементов).
- **P3 — полная миграция kit + app.rs + canvas-render**: перевод kit.rs (~62 мест `Child::fixed`+`lay_out`) + canvas-app (~500 мест) + canvas-render (~30 мест) на taffy backend по умолчанию; `NativeBackend` остаётся как fast-path/фолбэк. Опционально — retained node-id кэш для инкрементального релайаута (если профиль кадра SPEC §6.3 покажет >1 мс).

**Зачем.** Цель владельца «близость к web ui html5 без web-технологий» требует CSS-совместимой семантики вёрстки (full flexbox grow/shrink/basis/wrap, CSS Grid 2D неравные треки/span/auto-flow/minmax, auto-sizing min/max/fit-content, overflow/clip/scroll, position %/ratio/aspect-ratio, sticky/transform, z-index/isolation). Taffy 0.14 покрывает ~80% из коробки (flexbox+grid+block); остальные ~20% (overflow:auto, sticky, transform, blend) — надстройки поверх во P2/P3. Устранение классов багов позиционирования (выход за parent, viewport, z-order, текст-мера, resize, scroll/overflow) — через overflow:hidden + measure-функции taffy + viewport-clamp адаптер в kit.rs.

**Границы FR-067:**

- `trait LayoutBackend` — НОВЫЙ; сигнатуры `Row::lay_out`/`Column::lay_out`/`grid_cells`/`Child::fixed/spacer/flexible`/`MeasuredItem`/`RowPolicy`/`MainAlign/CrossAlign` — **СТАБИЛЬНЫ** (контракт PRD-0009 §7.4 V-5: потребители не переписываются).
- Фича `taffy = ["dep:taffy"]` — НЕ default; B2B zero-dep инвариант сохранён (default сборка без taffy — 0 новых deps, 0 прироста wasm).
- `UiLayer` (FR-051, 9 полос) и `Painter` (FR-057) — **НЕ трогаются**; движок вёрстки ортогонален слоям/capture/draw-порядку (D8 ADR-0013).
- `geometry.rs` (`UiRect/UiVec2/UiPoint/EdgeInsets`) — **НЕ меняется**.
- Вся работа — внутри `canvas-ui/src/layout.rs` (новый `trait LayoutBackend` + `NativeBackend` + `TaffyBackend` за cfg) + `canvas-ui/Cargo.toml` (фича `taffy`) + `[workspace.dependencies]` (`taffy` опциональная) + новая директория `canvas-ui/tests/html5_demos/` (golden-эталоны) + staged миграция потребителей в P2/P3.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `canvas-ui/src/layout.rs` | НОВЫЙ `trait LayoutBackend` (`lay_out_row/column/grid/measured` + `available_features() -> LayoutFeatures`); `NativeBackend` (перенос текущих Row/Column/grid_cells 1:1 — поведение идентично); `TaffyBackend` за `#[cfg(feature="taffy")]` (adapter Row/Column/Child → taffy::Style + measure-функции через TextMeasurer). `Row/Column` остаются как API, делегируют в backend по `cfg`/runtime-выбору. | `docs/ui-kit.md` §«Layout-примитивы», `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` |
| `canvas-ui/Cargo.toml` | НОВАЯ фича `taffy = ["dep:taffy"]`; default-features = `[]` (zero-dep инвариант). `taffy` в `[workspace.dependencies]` корневого `Cargo.toml` (`version = "0.14", default-features = false, features = ["std", "taffy_tree", "flexbox", "grid", "block"]`). | `docs/DEPENDENCIES.md` §3→§2 (после merge P1) |
| `canvas-ui/src/kit.rs` (P2/P3) | `dropdown_menu/tooltip/modal/toast_area` — переход на `position: absolute/fixed` семантику через taffy backend; `ScrollState` → `overflow:auto` (taffy `overflow:hidden` + `ScrollState.offset` для скролла, `Painter::clip_rect` для клиппинга); ~62 мест `Child::fixed`+`lay_out` — миграция на backend-вызов `lay_out_*`. | `docs/ui-kit.md` §«Kit-компоненты» |
| `canvas-app/src/*.rs` (P3) | ~500 мест `Child::fixed`+`lay_out` — миграция (`debug_overlay/docs_ui/template_ui/settings_ui/whatif_ui/scheme_gallery_ui/kit_ui/app.rs`). Сигнатуры не меняются — меняется только точка вызова (`Row{...}.lay_out(slot, &items)` → `backend.lay_out_row(Row{...}, slot, &items)` или `Row{...}.lay_out(slot, &items)` с backend по cfg). | `docs/ui-kit.md` §«Карта переносов» |
| `canvas-render/src/{search_ui,row_grid}.rs` (P3) | ~30 мест — миграция на backend-вызов. | — |
| `canvas-ui/tests/html5_demos/` (P1) | НОВАЯ директория: 10 эталонных HTML5 demo-layouts (mdn/css-tricks топ-10) → golden-снапшоты `UiRect`-дампов; сравнение `NativeBackend` vs `TaffyBackend` для совместимых политик; для taffy-only политик — `TaffyBackend` единственный оракул. | `docs/ui-kit.md` §«Эталоны вёрстки» |
| `canvas-ui/src/paint.rs` (P2) | РАСШИРЕНИЕ `PaintItem` — новый `PaintItem::ClipRect { rect: UiRect, items: Vec<PaintItem> }` для overflow:hidden семантики; consumer (canvas-app kit_ui) конвертирует в scissor FR-056. | `docs/ui-kit.md` §«Painter» |
| Тесты | G4-линт прогоняется ×2 backend'а (`cargo test --features taffy`); canonical сцены 3 окна × 2 языка × 2 backend'а = 12 прогонов; golden FR-062 F-18 параметризуется на backend; 10 HTML5 demos — новые golden-снапшоты. | `docs/prd/prd-0009-ui-layering-uikit.md` §AC-3 |
| Wasm-бандл | +~376 КБ raw / ~180 КБ gzip при `--features taffy`; default без изменений. Гейты `scripts/wasm_gate.sh --check` (default) + дополнительный замер с фичей. | `docs/plans/wasm-port.md` §8.8, §6.1 п.4 |
| Документация | `docs/DEPENDENCIES.md` §3→§2 (taffy — прямая прод-зависимость после P1); `docs/SPEC.md` §6.3 (параллельный путь backend'ов); `docs/ui-kit.md` (карта переносов, эталоны вёрстки); `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` (решение); `docs/change-requests/index-cr-fr.md` (статус FR-067). | — |

## Анализ (Root Cause)

Слой canvas-ui после FR-051/053/056/057/058/059/060/062 (2026-09-23): immediate-mode layout-примитивы V-5 (Row/Column/grid_cells/stack/constrain/pad), TextMeasurer (cosmic-text шейпинг + кэш), Painter (draw-слой PaintItem), UiLayer (9 полос), SurfaceRegistry (capture/hit), KitState/WidgetState (FR-057). Всего ~7 080 строк чистой геометрии без wgpu/winit (headless-TDD).

Чего нет сегодня (аудит репо 2026-09-24, по запросу владельца):

- **Нет CSS-семантики в полном объёме.** `Child::fixed/spacer/flexible` — fixed или grow (1.0) только. `RowPolicy{Fit, SqueezeTail, Wrap}` — 3 политики; taffy: `flex_shrink/basis`, `flex_wrap: Wrap/Reverse`, `align_items: stretch` (cross-axis auto-stretch в Row). CSS Grid: `grid_cells` (равные треки, FR-062 F-16) vs taffy full Grid (неравные треки, `span`, `auto-flow`, `minmax`, `auto-fit/fill`). Position: `stack/constrain` — нет `position: absolute/fixed/sticky`, нет % sizing от parent, нет `aspect-ratio/calc()`.
- **Нет overflow-clip в layout.** `RowPolicy::Fit` — переполнение НЕ маскируется (видимый выход за parent ловится линтом G4, но не клипится автоматически); `SqueezeTail` — ручная деградация (хвост до 0); `Wrap` — многострочность, но без `overflow:hidden` клиппинга. Painter `PaintItem::{Rect, Text}` — нет `ClipRect` (clip-семантика на стороне `canvas-render` scissor FR-056, не в layout).
- **Viewport-clamp — ручной, в kit.rs.** `dropdown_menu/tooltip/modal/toast_area` явно `max(viewport.x)`/`min(viewport.right())` (`kit.rs:389,437,445`); ~5 функций, ~15 строк клампинга на каждую. Taffy не имеет viewport-семантики (`position: absolute/fixed` — относительно nearest positioned ancestor, не viewport). Решение FR-067 P2: адаптер `kit::viewport_*` поверх taffy backend — viewport-clamp остаётся в kit.rs как слой над layout.
- **`ScrollState` — ручной.** `offset/content_h/viewport_h` + `list_rows` функция (`kit.rs:741,782`) — вычисляет видимые строки по offset. Taffy не имеет scroll-native (это layout-движок, не scroll-container). Решение FR-067 P2: `overflow:hidden` в layout + `ScrollState.offset` для content-shift + Painter `ClipRect` для видимой области. Аналог HTML `overflow:auto` — но scroll-mechanics остаются приложением.
- **z-order — `UiLayer` (9 полос), без per-element stacking context.** FR-051 решает z-проблему на уровне слоёв; внутри слоя — порядок вызовов Painter. CSS `z-index/isolation/mix-blend-mode` — не нужны в текущем UX (достаточно UiLayer), но при появлении сложных overlay (sticky header + dropdown над sticky) — потребуют per-element z. FR-067 P2: оставляем UiLayer как primary; per-element stacking — опция P3 (если появится потребность).
- **`SqueezeTail` ≠ `flex_shrink` (расхождение контракта).** `SqueezeTail` — хвост до 0 ширины, half-open hit (`UiRect::contains` — half-open, FR-053 F-11c). `flex_shrink: 1.0` + `min_width: 0` в taffy — близко, но не дословно (taffy оставит min-content, не 0). Решение FR-067: `SqueezeTail` остаётся как семантика `NativeBackend`; в `TaffyBackend` мапится на `flex_shrink: 1.0, min_width: 0` — расхождение документировано; G4-линт ловит; потребитель выбирает backend осознанно.
- **Текст-мера — FR-061 T9 прецедент.** Замер `TextMeasurer` (cosmic-text Weight::MEDIUM) расходился с шейпингом (mono_attrs = 400) — наложения чисел/юнитов. Taffy `measure` функции — тот же `TextMeasurer`, но в layout-движке (а не ручная проводка `width_of → Child::fixed`); закрытие класса багов: контент-размер = layout-размер автоматически.
- **Resize/reflow — ручной.** Оконные/сцена resize → пересчёт `panel_rect`/`modal`/`tooltip` вручную через `panel_rect(slot, ...)`/`modal(slot, ...)`; нет auto-flex % sizing (slot.w — px, не %). Taffy `percent` sizing (`width: 50%`) — решает класс.
- **Триггеры T1–T4 ADR-0013 формально не сработали** (FR-062 закрыл auto-размеры/flex/wrap/Grid-lite); но цель владельца «близость к web ui html5» — это **T4 как цель продукта** (не ожидание триггера). ADR-0014 явно декларирует: цель владельца преобладает над триггерами.

## Требуемые изменения (Changes)

### Что → где → как

| Что | Где | Как |
|---|---|---|
| НОВЫЙ `trait LayoutBackend` | `canvas-ui/src/layout.rs` (рядом с `Row/Column`, F-7) | `pub trait LayoutBackend { fn lay_out_row(&self, row: Row, slot: UiRect, items: &[Child]) -> Vec<UiRect>; fn lay_out_column(&self, col: Column, slot: UiRect, items: &[Child]) -> Vec<UiRect>; fn lay_out_measured(&self, row: Row, slot: UiRect, items: &[MeasuredItem], m, fs, family, size) -> Vec<UiRect>; fn lay_out_grid(&self, slot: UiRect, cols: &[f32], rows: usize, row_h: f32, gap: UiVec2) -> Vec<UiRect>; fn features(&self) -> LayoutFeatures; }`. `LayoutFeatures` — битовая маска: `FLEX_GROW/FLEX_SHRINK/FLEX_BASIS/FLEX_WRAP/GRID_2D/AUTO_SIZE/OVERFLOW_CLIP/PERCENT/ASPECT_RATIO/STICKY`. `Row::lay_out(slot, items)` делегирует в `default_backend()` по `cfg`/runtime-выбору. |
| `NativeBackend` (default, FR-062 основа) | `canvas-ui/src/layout.rs` | `pub struct NativeBackend;` — обёртка над существующими `Row/Column/grid_cells` (текущая реализация переносится в методы трейта 1:1 — `Row::fit/squeeze_tail/wrap` становятся `NativeBackend::lay_out_row_*`). Поведение идентично — G4-тесты FR-062 продолжают работать. |
| `TaffyBackend` (за фичей `taffy`) | `canvas-ui/src/layout.rs` (модуль `taffy_backend`) | `#[cfg(feature="taffy")] pub struct TaffyBackend { tree: taffy::TaffyTree }`. Адаптер: `Row{gap, main, cross, policy}` → `taffy::Style { gap, justify_content, align_items, ... }`; `Child::fixed/spacer/flexible` → `Style { size, flex_grow, flex_shrink, flex_basis, ... }`; `MeasuredItem::Text` → `Style + measure_func` (closure на `TextMeasurer::width_of`); `RowPolicy::Wrap` → `flex_wrap: Wrap`; `SqueezeTail` → `flex_shrink: 1.0, min_width: 0` (документированное расхождение); `grid_cells` (равные треки) → `display: Grid + grid_template_columns: repeat(n, 1fr)`; для неравных треков P1 — Nовой API `lay_out_grid_v2(slot, tracks: &[TrackSize], span: &[GridSpan])`. `compute_layout(root, AvailableSpace::Definite(slot.size()))` → чтение `tree.layout(node)` → `Vec<UiRect>`. |
| Фича `taffy` | `canvas-ui/Cargo.toml` + корневой `Cargo.toml` | `[features] taffy = ["dep:taffy"]` (default-features = `[]`); `[dependencies] taffy = { workspace = true, optional = true }`. Workspace: `taffy = { version = "0.14", default-features = false, features = ["std", "taffy_tree", "flexbox", "grid", "block"] }`. |
| Pilot P1 — `kit_ui::explain_dialog` | `crates/canvas-app/src/explain_ui.rs`/`kit_ui.rs` (модальный диалог с списком полей) | Перевод с `NativeBackend` на `TaffyBackend` (через backend-выбор в `kit_ui::kit_draw`); `Row{gap: 8, main: Start, cross: Center}.lay_out(slot, &items)` — поведение идентично (линейный layout, нет Grid/shrink); golden FR-062 F-18 — тот же оракул (побитово). Тест: `cargo test -p canvas-ui --features taffy explain_dialog_golden`. |
| Pilot P1 — `fr061_row_grid` (tabular body) | `crates/canvas-render/src/row_grid.rs`/`crates/canvas-ui/src/row_guides.rs` (FR-061 D-3) | T2-триггер ADR-0013: tabular body FR-061 этап E использует `RowGuides::measure_row_cells` (равные треки); для неравных треков (auto-fit, minmax) — `TaffyBackend::lay_out_grid_v2(slot, tracks, span)`. Pilot проверяет: golden-снапшот `measure_row_cells` (NativeBackend) vs `lay_out_grid_v2` (TaffyBackend) — для равных треков побитово; для неравных — новый оракул. |
| P2 — `kit::dropdown_menu/tooltip/modal/toast_area` viewport-clamp | `canvas-ui/src/kit.rs:389,423,478,458` | Адаптер `position: absolute/fixed` через `TaffyBackend`: layout не знает viewport — kit.rs передаёт `viewport: UiRect` как `AvailableSpace` + post-layout clamp `rect.intersection(viewport).unwrap_or(rect)`. Taffy `position: absolute` (относительно nearest positioned ancestor) — не используется (canvas-ui immediate, нет positioned ancestors). `position: fixed` (относительно viewport) — моделируется как `AvailableSpace::Definite(viewport.size())` + clamp. |
| P2 — `ScrollState` → `overflow:auto` | `canvas-ui/src/kit.rs:741` (ScrollState) + `paint.rs` (ClipRect) | `overflow:hidden` в layout (taffy `overflow: Hidden` в Style) → контент клипится; `ScrollState.offset` для content-shift (translate всех детей на `-offset`); `Painter::clip_rect(rect, items)` — новый `PaintItem::ClipRect` для передачи во consumer (canvas-app → canvas-render scissor FR-056). Аналог HTML `overflow:auto` без scroll-mechanics — scroll/wheel остаётся приложением. |
| P2 — расширение G4-линта | `canvas-ui/tests/g4_layout_lint.rs` (или inline в kit.rs tests) | Новые проверки: (1) `UiRect::intersection(rect, parent_rect).is_some()` для всех видимых элементов (выход за parent); (2) `UiRect::intersection(rect, viewport).is_some()` для всех L4+ поверхностей (выход за viewport); (3) `painter.items()` ClipRect-вложенность корректна (stacking-context). Прогон ×2 backend'а: `cargo test --features taffy --test g4_layout_lint`. |
| P3 — миграция kit.rs (~62 мест) | `canvas-ui/src/kit.rs` (panel/button/icon_button/chip/dropdown/tooltip/toast/modal/text_field/list_rows) | Backend-вызов `Row/Column::lay_out(slot, &items)` — внутри делегирует в `default_backend()` (cfg/runtime). Поведение идентично для `NativeBackend`; для `TaffyBackend` — те же rect'ы (golden параметризован на backend). |
| P3 — миграция canvas-app (~500 мест) | `crates/canvas-app/src/{debug_overlay,docs_ui,template_ui,settings_ui,whatif_ui,scheme_gallery_ui,kit_ui,app}.rs` | Механическая миграция: `Row{...}.lay_out(slot, &items)` не меняется (делегирует в backend); `Child::fixed(w, h)` — там, где есть measure-функция taffy, заменить на `MeasuredItem::Text` (auto-size); manual `width_of → Child::fixed` — удалить (закрытие класса багов текст-мера). |
| P3 — миграция canvas-render (~30 мест) | `crates/canvas-render/src/{search_ui,row_grid}.rs` | Аналогично P3 canvas-app; `search_ui` — overlay-листы, `row_grid` — tabular body FR-061. |
| P3 (опц.) — retained node-id кэш | `canvas-ui/src/layout.rs` (TaffyBackend) | Если профиль кадра SPEC §6.3 покажет >1 мс на taffy backend — сохранение `TaffyTree` между кадрами с инкрементальным dirty-обновлением (taffy built-in cache). Опция P3, не блокер P1/P2. |
| Эталон HTML5 demos (P1) | `canvas-ui/tests/html5_demos/` (новая директория) | 10 layouts: (1) sticky-header-column, (2) sidebar-content-overflow-auto, (3) flexbox-navbar-space-between, (4) css-grid-12-col-responsive, (5) masonry-lite, (6) card-list-aspect-ratio, (7) modal-position-fixed-viewport-clip, (8) dropdown-flip, (9) scrollable-list-virtualization, (10) complex-form-layout. Каждый — `.rs` файл с эталонной структурой + `.txt` golden-снапшот `UiRect`-дампа (нормализованная строка с округлением до целого ui px — методология FR-062 F-18). |
| Документация | `docs/DEPENDENCIES.md` §3→§2 (после merge P1), `docs/SPEC.md` §6.3 (комментарий backend'ов), `docs/ui-kit.md` (карта переносов, эталоны вёрстки), `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` (решение), `docs/change-requests/index-cr-fr.md` (статус FR-067). | — |

## Контракты на стыках

Контракты из ADR-0014 §Решение — замороженные швы между фазами P1/P2/P3. Нарушение = конфликт слияния на ревью.

- **§Контракт-1 — интерфейс V-5 PRD-0009 стабилен.** Сигнатуры `Row::lay_out(slot, items) -> Vec<UiRect>`, `Column::lay_out(...)`, `grid_cells(slot, cols, rows, row_h, gap)`, `Child::fixed/spacer/flexible`, `MeasuredItem`, `RowPolicy`, `MainAlign/CrossAlign` — НЕ меняются. Потребители не переписываются. P1/P2/P3 — только внутренности (делегирование в backend) + новые методы для taffy-only фич (`lay_out_grid_v2`).
- **§Контракт-2 — фича `taffy` выключена по умолчанию.** Default сборка без taffy = 0 новых deps, 0 прироста wasm (B2B zero-dep инвариант PRD-0009 G7). `cargo build --no-default-features` — `TaffyBackend` не компилируется.
- **§Контракт-3 — G4-линты на обоих backend'ах.** `cargo test -p canvas-ui` (default) + `cargo test -p canvas-ui --features taffy` — оба обязаны быть зелёными на каждой фазе. Canonical сцены 3 окна × 2 языка × 2 backend'а = 12 прогонов (AC-3.1 PRD-0009).
- **§Контракт-4 — `SqueezeTail` ≠ `flex_shrink` — документированное расхождение.** `NativeBackend::SqueezeTail` — хвост до 0 ширины (half-open hit). `TaffyBackend` мапит на `flex_shrink: 1.0, min_width: 0` — min-content пол. G4-линт помечает расхождение как допустимое для `SqueezeTail`-политик; потребитель выбирает backend осознанно.
- **§Контракт-5 — `UiLayer` (FR-051) и `Painter` (FR-057) ортогональны.** Движок вёрстки не меняет слои/capture/draw-порядок (D8 ADR-0013). P2 добавляет `PaintItem::ClipRect` (clip-семантика в layout), но не меняет UiLayer (9 полос) или SurfaceRegistry.
- **§Контракт-6 — wasm-гейты зелёные на каждой фазе.** `scripts/wasm_gate.sh --check` (default, без taffy) + дополнительный замер с `--features taffy` (если компилируется на wasm32 — taffy заявляет no_std-совместимость, `default-features = false`). `scripts/mcp_wasm_gate.sh --check` — без изменений (canvas-mcp не зависит от canvas-ui).
- **§Контракт-7 — замер wasm обязателен до merge P1.** `cargo build --release --target wasm32-unknown-unknown --features canvas-ui/taffy` (через canvas-web cdylib) + `ls -l target/wasm32-unknown-unknown/release/*.wasm`. Ожидание: raw +370…380 КБ; gzip +170…190 КБ. Бюджет: ≤8 МБ raw / ≤4 МБ brotli (wasm-port.md §8.8). Превышение — явное решение владельца.

**Подчеркнуть (границы владений):** НЕ трогать `geometry.rs`, `UiLayer` (FR-051), `SurfaceRegistry`/`capture` (FR-051), `KeyboardRouter` (FR-051), `Painter` (FR-057 — кроме P2 ClipRect). Сигнатуры `Row::lay_out`/`Column::lay_out`/`grid_cells`/`Child`/`MeasuredItem`/`RowPolicy` — НЕ менять. Вся работа — внутри `canvas-ui/src/layout.rs` + активация фичи в `Cargo.toml` + staged миграция потребителей в P2/P3 + эталонные demos.

## Фазы и проверяемые коммиты

Три фазы — отдельные коммиты (паттерн «одна сессия агента = один коммит», AGENTS.md); каждая поставляема и приёмлема независимо. Между фазами — G4-аудит + замер wasm + golden-сравнение backend'ов.

### P1 — `trait LayoutBackend` + 2 backend'а + pilot

- **Коммит:** `feat(ui/layout): trait LayoutBackend + NativeBackend + TaffyBackend pilot (FR-067 P1, ADR-0014)`
- **Что:**
  - НОВЫЙ `pub trait LayoutBackend` в `canvas-ui/src/layout.rs` с методами `lay_out_row/column/grid/measured` + `features() -> LayoutFeatures`.
  - `pub struct NativeBackend;` — перенос текущих `Row/Column/grid_cells` в методы трейта 1:1 (поведение идентично).
  - `#[cfg(feature="taffy")] pub struct TaffyBackend { tree: taffy::TaffyTree }` — skeleton + adapter `Row/Column/Child/MeasuredItem → taffy::Style` + measure-функции через `TextMeasurer`.
  - Фича `taffy = ["dep:taffy"]` в `canvas-ui/Cargo.toml`; `taffy` в `[workspace.dependencies]`.
  - Pilot: `kit_ui::explain_dialog` (модальный диалог) + `fr061_row_grid` (tabular body) — перевод на `TaffyBackend` (через backend-выбор).
  - 10 эталонных HTML5 demos в `canvas-ui/tests/html5_demos/` — golden-снапшоты `UiRect`-дампов.
- **Гейт:**
  - `cargo test -p canvas-ui` (default) — G4-линты + golden FR-062 F-18 на `NativeBackend` (поведение идентично).
  - `cargo test -p canvas-ui --features taffy` — G4-линты + golden на `TaffyBackend` (для совместимых политик побитово; для taffy-only — новые оракулы).
  - `cargo build --no-default-features` — `TaffyBackend` не компилируется, zero-dep.
  - `cargo build --release --target wasm32-unknown-unknown --features canvas-ui/taffy` (через canvas-web cdylib) — замер wasm raw/gzip; ≤8 МБ raw / ≤4 МБ brotli.
  - `scripts/wasm_gate.sh --check` (default) + `cargo deny check` (taffy + транзитивные в allowlist).
  - `cargo clippy -D warnings` (default + `--features taffy`); `cargo fmt --check`.
  - HTML5 demos: 10/10 golden зелёные на `TaffyBackend`.

### P2 — viewport-clamp + overflow/scroll

- **Коммит:** `feat(ui/kit): viewport-clamp + overflow/scroll on TaffyBackend (FR-067 P2, ADR-0014)`
- **Что:**
  - Адаптер `kit::viewport_dropdown/tooltip/modal/toast` поверх `TaffyBackend` — `position: absolute/fixed` семантика через `AvailableSpace::Definite(viewport.size())` + post-layout `rect.intersection(viewport)`.
  - `ScrollState` → `overflow:auto` — `overflow:hidden` в layout (taffy `overflow: Hidden` в Style) + `ScrollState.offset` для content-shift + `Painter::clip_rect` для видимой области.
  - НОВЫЙ `PaintItem::ClipRect { rect: UiRect, items: Vec<PaintItem> }` в `canvas-ui/src/paint.rs` — clip-семантика в draw-слой; consumer (canvas-app kit_ui) конвертирует в scissor FR-056.
  - Расширение G4-линта: выход за parent (`UiRect::intersection(rect, parent).is_some()`), выход за viewport (`UiRect::intersection(rect, viewport).is_some()` для L4+), ClipRect-вложенность корректна.
  - Перевод `palette/dropdown/tooltip/modal/toast` поверхностей на `TaffyBackend` с viewport-clamp.
- **Гейт:**
  - G4-линты ×2 backend'а: 0 выходов за parent, 0 выходов за viewport (новые проверки).
  - Golden HTML5 demos: 7-й (modal-position-fixed-viewport-clip) + 8-й (dropdown-flip) + 9-й (scrollable-list-virtualization) — зелёные.
  - `cargo test -p canvas-ui --features taffy` — все тесты зелёные.
  - Замер wasm с фичей (после P2) — прирост ≤400 КБ raw (P1+P2).
  - `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.

### P3 — полная миграция kit + app + render

- **Коммит(ы):** `feat(ui): migrate kit + app + render to TaffyBackend default (FR-067 P3, ADR-0014)` (возможно 2–3 коммита по подсистемам: kit, app, render)
- **Что:**
  - Миграция kit.rs (~62 мест) на backend-вызов (делегирование в `default_backend()`).
  - Миграция canvas-app (~500 мест) + canvas-render (~30 мест) — `Child::fixed(w, h)` где есть measure → `MeasuredItem::Text` (auto-size); удаление manual `width_of → Child::fixed` (закрытие класса багов текст-мера).
  - `default_backend()` переключается на `TaffyBackend` (если фича `taffy` активна) или `NativeBackend` (default off); runtime-выбор через `LayoutBackend::default()` или явный параметр.
  - Опционально: retained node-id кэш для инкрементального релайаута (если профиль кадра SPEC §6.3 покажет >1 мс на taffy).
- **Гейт:**
  - `cargo test --workspace --features canvas-ui/taffy` — все тесты зелёные на taffy backend.
  - `cargo test --workspace` (default) — все тесты зелёные на NativeBackend (zero-dep).
  - G4-линты ×2 backend'а: 0 регрессий на canonical сценах.
  - Golden HTML5 demos: 10/10 зелёные на обоих backend'ах (где семантика совпадает).
  - Профиль кадра: 60 fps на 5 000 нод + UI релайаут ≤1 мс (SPEC §6.3) — сохраняется.
  - Замер wasm: ≤8 МБ raw / ≤4 МБ brotli.
  - `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.

## Ограничения для агента-реализатора

- **Staged migration — обязательно.** Не делать P2/P3 до зелёного P1. Pilot на 2 поверхностях — проверка гипотезы; при провале (golden-расхождения >30% не устранимы адаптером) — откат, ADR-0014 → `отклонено`, ADR-0013 восстанавливается.
- **Гибрид `trait LayoutBackend`** — не полная замена. `NativeBackend` остаётся как default; потребители не переписываются (сигнатуры стабильны). Миграция P2/P3 — механическая (делегирование в backend), не переписывание.
- **`cfg(not(target_arch="wasm32"))` для тяжёлых операций taffy** — опционально (taffy заявляет no_std-совместимость); если wasm-билд с `--features taffy` не собирается — fallback на `NativeBackend` (runtime-выбор) или `cfg`-гейт (фича `taffy` не активна на wasm). Проверить на P1.
- **`SqueezeTail` ≠ `flex_shrink` — документировать.** Не пытаться выровнять дословно — оставить как семантику `NativeBackend`; в `TaffyBackend` мапить на `flex_shrink: 1.0, min_width: 0` + комментарий в коде + запись в `docs/ui-kit.md` §«Расхождения backend'ов».
- **collect-then-reduce для golden-снапшотов** — `UiRect`-дамп сортировать по `(x, y, w, h)` перед сравнением (детерминированный порядок; taffy `par_iter`-эквивалента нет, но `compute_layout` детерминирован).
- **`cargo build --no-default-features` — zero-dep инвариант.** Без `taffy` фичи `TaffyBackend` не компилируется, `taffy` dep не резолвится. `cargo deny check` зелёный; B2B-сборка не тащит taffy.
- **Golden-эталоны HTML5 demos — побитово идентичны между backend'ами** только для совместимых политик (`Fit`/`Start`/`End`/`SpaceBetween`/`Wrap`/равная 2D-сетка). Для taffy-only (неравные треки, `flex_shrink`, `aspect-ratio`, `percent`) — `TaffyBackend` единственный оракул; NativeBackend не поддерживает, golden создаётся под taffy.
- **Не трогать:** `geometry.rs`, `UiLayer`, `SurfaceRegistry`/`capture`, `KeyboardRouter`, `Painter` (кроме P2 `ClipRect`), `frame.rs`, `hit.rs`, `widget.rs`. Сигнатуры `Row::lay_out`/`Column::lay_out`/`grid_cells`/`Child`/`MeasuredItem`/`RowPolicy` — НЕ менять.
- **Эскалация при провале P1:** если golden-расхождения `TaffyBackend` vs `NativeBackend` на pilot-поверхностях >30% (не устранимы адаптером) — откат, ADR-0014 переходит в `отклонено`, ADR-0013 восстанавливается как действующий. Taffy не активируется в main, остаётся за фичей для экспериментов.

## Наглядная проверка (5 минут)

**Демо.** 10 эталонных HTML5 demo-layouts в `canvas-ui/tests/html5_demos/` — каждый рендерится 1:1 в canvas-ui через `Painter` + `UiRect`-дамп; golden-снапшоты сравниваются с эталоном (mdn/css-tricks). На demo-визуализации: sticky-header-column, sidebar-content-overflow-auto, css-grid-12-col, masonry-lite — видны как корректные rect'ы (без выхода за parent, без выхода за viewport, без наложений).

**Автотесты (зелёные):**

- `cargo test -p canvas-ui` (default) — `NativeBackend` G4-линты + golden FR-062 F-18 (поведение идентично).
- `cargo test -p canvas-ui --features taffy` — `TaffyBackend` G4-линты + golden (для совместимых политик побитово; для taffy-only — новые оракулы).
- HTML5 demos: 10/10 golden зелёные на `TaffyBackend` (P1+); на `NativeBackend` — только совместимые политики (5/10, остальные taffy-only).
- G4-линты ×2 backend'а: 0 выходов за parent (P2+), 0 выходов за viewport (P2+), 0 ClipRect-вложенных ошибок (P2+).
- `cargo build --no-default-features` — `TaffyBackend` не компилируется, zero-dep.
- `cargo build --release --target wasm32-unknown-unknown --features canvas-ui/taffy` (через canvas-web cdylib) — замер wasm ≤8 МБ raw / ≤4 МБ brotli.
- `scripts/wasm_gate.sh --check` (default) — зелёный.
- `cargo deny check` — taffy + транзитивные deps в allowlist.

## Проверка (Verification)

Чек-лист приёмки (каждая фаза — все зелёные):

### P1

- [ ] `cargo test -p canvas-ui` (default) — G4-линты + golden FR-062 F-18 на `NativeBackend` (поведение идентично).
- [ ] `cargo test -p canvas-ui --features taffy` — G4-линты + golden на `TaffyBackend` (совместимые политики побитово; taffy-only — новые оракулы).
- [ ] `cargo build --no-default-features` — `TaffyBackend` не компилируется, zero-dep инвариант.
- [ ] `cargo build --release --target wasm32-unknown-unknown --features canvas-ui/taffy` — raw ≤8 МБ / brotli ≤4 МБ; прирост +370…380 КБ raw / +170…190 КБ gzip к default-бандлу.
- [ ] `scripts/wasm_gate.sh --check` (default) + `cargo deny check` — зелёные.
- [ ] `cargo clippy -D warnings` (default + `--features taffy`); `cargo fmt --check`.
- [ ] HTML5 demos: 10/10 golden зелёные на `TaffyBackend`.
- [ ] Pilot: `kit_ui::explain_dialog` + `fr061_row_grid` — на `TaffyBackend` golden совпадает с `NativeBackend` (для совместимых политик).

### P2

- [ ] G4-линты ×2 backend'а: 0 выходов за parent, 0 выходов за viewport, 0 ClipRect-вложенных ошибок.
- [ ] Golden HTML5 demos: 7/8/9 (modal/dropdown/scrollable-list) — зелёные на `TaffyBackend`.
- [ ] `cargo test -p canvas-ui --features taffy` — все тесты зелёные.
- [ ] Замер wasm с фичей (после P2) — прирост ≤400 КБ raw (P1+P2).
- [ ] `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.

### P3

- [ ] `cargo test --workspace --features canvas-ui/taffy` — все тесты зелёные на taffy backend.
- [ ] `cargo test --workspace` (default) — все тесты зелёные на NativeBackend (zero-dep).
- [ ] G4-линты ×2 backend'а: 0 регрессий на canonical сценах 3 окна × 2 языка.
- [ ] Golden HTML5 demos: 10/10 зелёные на обоих backend'ах (для совместимых политик).
- [ ] Профиль кадра: 60 fps на 5 000 нод + UI релайаут ≤1 мс (SPEC §6.3) — сохраняется.
- [ ] Замер wasm: ≤8 МБ raw / ≤4 МБ brotli.
- [ ] `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.
- [ ] `grep -r "Child::fixed" crates/ | wc -l` — снижение на ~30% (где заменено на `MeasuredItem::Text` auto-size).

## Точки входа (Entry Points)

- `docs/SPEC.md` §6.3 — бюджеты UI (комментарий о backend'ах вёрстки).
- `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` — решение (гибрид + staged).
- `docs/adr/adr-0013-taffy-vs-layout-primitives.md` — заменённое решение (замеры taffy воспроизводимы, триггеры T1–T4, протокол эскалации PRD-0009 §9.5).
- `docs/prd/prd-0009-ui-layering-uikit.md` §7.2 V-3 / §7.4 V-5 (интерфейс стабилен) / §9.5 (дверь taffy) / AC-3.1 (G4-линты).
- `docs/ui-kit.md` — карта переносов, эталоны вёрстки (HTML5 demos), расхождения backend'ов (`SqueezeTail` ≠ `flex_shrink`).
- `docs/DEPENDENCIES.md` §3→§2 (taffy миграция после P1).
- `docs/plans/wasm-port.md` §8.8 — бюджеты wasm (≤8 МБ raw / ≤4 МБ brotli).
- FR-053 (U3 F-7 — layout-примитивы, основа `NativeBackend`), FR-056 (scissor — переиспользуется P2 ClipRect), FR-057 (Painter — P2 расширение ClipRect), FR-062 (собственные layout v2 — `NativeBackend`), FR-061 этап E (kit-Row — pilot P1).

## История изменений (Changelog)

- `2026-09-24` — агент: создан документ (план, статус `выявлено`). Зафиксированы 3 фазы (P1 trait+pilot, P2 viewport+overflow, P3 полная миграция), контракты на стыках (§Контракты 1–7), ограничения для агента-реализатора (staged обязательно, гибрид не замена, SqueezeTail ≠ flex_shrink документировать, zero-dep инвариант). Привязан к ADR-0014 (решение); ADR-0013 → `заменено (ADR-0014)`. Решения за владельцем — до гейта P1.

## Источники истины (References)

- `crates/canvas-ui/src/layout.rs:1–1114` — `Row/Column/Child/MeasuredItem/RowPolicy/grid_cells/stack/constrain/pad/Custom` (FR-053 F-7 + FR-062 F-13…F-18) — основа `NativeBackend`; сигнатуры НЕ меняются (контракт §1).
- `crates/canvas-ui/src/kit.rs:389,423,478,458` — `dropdown_menu/tooltip/modal/toast_area` viewport-clamp (P2 — переход на `TaffyBackend` с `position: absolute/fixed` адаптером).
- `crates/canvas-ui/src/kit.rs:741,782` — `ScrollState/list_rows` (P2 — `overflow:auto` семантика через taffy `overflow:hidden` + `ScrollState.offset` + `Painter::clip_rect`).
- `crates/canvas-ui/src/paint.rs:31,52` — `PaintItem/Painter` (P2 — расширение `PaintItem::ClipRect` для clip-семантики в draw-слой).
- `crates/canvas-ui/src/layer.rs:11` — `UiLayer` 9 полос (FR-051, ОРТОГОНАЛЕН движку вёрстки — D8 ADR-0013).
- `crates/canvas-ui/src/geometry.rs:36,75,116,124,134,147` — `UiRect` (`new/right/bottom/contains/intersects/intersection/inset/translated`) — НЕ меняется; `intersection` переиспользуется в G4-линтах (P2 — проверка выхода за parent/viewport).
- `crates/canvas-ui/src/measure.rs` — `TextMeasurer` (FR-053 F-6, cosmic-text шейпинг + кэш) — переиспользуется в `TaffyBackend` measure-функциях (тот же шейпинг, что у рендера).
- `crates/canvas-ui/Cargo.toml` — `[features] default = []; taffy = ["dep:taffy"]` (P1); `[dependencies] taffy = { workspace = true, optional = true }`.
- `Cargo.toml` (workspace) — `[workspace.dependencies] taffy = { version = "0.14", default-features = false, features = ["std", "taffy_tree", "flexbox", "grid", "block"] }`.
- `crates/canvas-app/src/{kit_ui,explain_ui,whatif_ui,palette,settings_ui,docs_ui,template_ui,scheme_gallery_ui,debug_overlay,app}.rs` — ~500 мест `Child::fixed`+`lay_out` (P3 — механическая миграция на backend-вызов).
- `crates/canvas-render/src/{search_ui,row_grid}.rs` — ~30 мест (P3).
- `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` — решение (гибрид `trait LayoutBackend` + taffy opt-in + staged).
- `docs/adr/adr-0013-taffy-vs-layout-primitives.md` — заменённое решение (статус `заменено (ADR-0014)`; замеры taffy воспроизводимы; триггеры T1–T4; `NativeBackend` = FR-062 F-13…F-18).
- `docs/prd/prd-0009-ui-layering-uikit.md` §7.2 V-3 / §7.4 V-5 / §9.5 / AC-3.1 (G4-линты 3 окна × 2 языка).
- `docs/DEPENDENCIES.md` §3 (taffy — кандидат, «уже транзитивно в дереве через cosmic-text») → §2 (после merge P1 — прямая прод-зависимость).
- `docs/SPEC.md` §6.3 (бюджеты UI — комментарий о backend'ах).
- `docs/plans/wasm-port.md` §8.8 — ≤8 МБ raw / ≤4 МБ brotli (бюджеты wasm).
- `docs/ui-kit.md` — карта переносов, эталоны вёрстки (HTML5 demos), расхождения backend'ов.
- `deny.toml` `[licenses] allow = ["MIT", "Apache-2.0", ...]` — taffy + транзитивные (slotmap/smallvec/cssparser/serde) в allowlist.
- `scripts/wasm_gate.sh` / `scripts/mcp_wasm_gate.sh` — гейты wasm-совместимости (default + `--features taffy` для canvas-web).
- Taffy 0.14.0 — `https://crates.io/crates/taffy` (MIT; ~13.9 млн загрузок; продакшн-потребители: Servo, Bevy, Zed, Slint, Lapce, Floem — ADR-0013 §Контекст).
