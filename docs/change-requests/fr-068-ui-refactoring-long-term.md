# FR-068: Поэтапный рефакторинг UI — волны W0–W4 (стратегия ADR-0015)

- **Статус:** в работе (W0 выполнено 2026-09-25 — гейты зелёные; W1 выполнено 2026-09-25 — гейты зелёные, taffy opt-in за фичей, wasm +283.9 КБ raw post-opt; ADR-0015 принят владельцем; W2–W4 — по плану; замечание владельцу: абсолютный wasm-лимит raw 8 МБ превышен ДО W1 на default-бандле (8.72 МБ — рост после W12 6.74 МБ, вне рамок W1) — решение по §Контракту-7 за владельцем)
- **Тип:** FR (Feature Request)
- **Приоритет:** важно
- **Владелец:** агент (планирование); решения — владелец проекта
- **Источник:** запрос владельца 2026-09-24: «проанализировать ADR-0014 критическим взглядом группы экспертов. Кроме реализации внедрения taffy, должен быть спланирован дальнейший рефакторинг и переход на полностью свою UI библиотеку, который будет иметь все сильные стороны и минимизирует внешние зависимости. Первично нам надо причесать UI, далее пойти по пути поэтапного рефакторинга с проверяемыми результатами». Ответы на clarifying questions: taffy временно (переход), приоритеты — UI-паритет + минимум deps + архитектура, cosmic-text — trait boundary, свой layout — минимум (flexbox + overflow/clip/scroll; Grid за taffy-опцией), проверка — layout-линт + snapshot-тесты + perf-бюджет, форма — ADR + FR (ADR-0015 + FR-068).
- **Связанные задачи:** ADR-0015 (стратегия долгосрочного UI-стека — критический разбор ADR-0014 + план волн W0–W4), ADR-0014 (taffy hybrid — переходное решение, поглощается W1), ADR-0013 (заменено ADR-0014 — основа `NativeBackend`/`FlexLayoutEngine`), PRD-0009 §7 V-3/V-4/V-5/V-6/V-7 (каркас UI), FR-051 (UiLayer/реестр — ортогонален), FR-053 (layout-примитивы F-7 — основа `NativeBackend`), FR-056 (scissor — переиспользуется W1 `PaintItem::ClipRect`), FR-057 (Painter/WidgetState — расширяется W1), FR-062 (собственные layout v2 — основа `NativeBackend`/`FlexLayoutEngine`), FR-067 (план staged миграции на taffy — поглощается W1), FR-061 этап E (kit-Row — pilot W1), SPEC §6.3 (бюджеты UI), `docs/DEPENDENCIES.md` (taffy/cosmic-text/rustybuzz/ttf-parser — транзитивные), `docs/ui-kit.md`, `docs/plans/wasm-port.md` §8.8 (≤8 МБ raw / ≤4 МБ brotli).
- **Создан:** 2026-09-24
- **Обновлён:** 2026-09-25
- **Документ-шаблон:** `docs/change-requests/cr-template.md`

---

## Описание (What)

**Что добавляется.** Долгосрочный план поэтапного рефакторинга `canvas-ui` — 5 волн (W0–W4), каждая поставляема и приёмлема независимо, с явными гейтами (layout-линт + snapshot-тесты + perf-бюджет). Конечная цель W4: собственная UI-библиотека с минимумом внешних зависимостей (`cargo build --no-default-features` = 0 внешних UI-runtime-deps, кроме cosmic-text за `Shaper` trait).

**Волны:**

- **W0 — UI hygiene (2–3 недели, ~5–8 сессий агента)**: «причесать UI» до taffy. Расширение G4-линта (5 canonical сцен × 3 окна × 2 языка = 30 прогонов; проверки выхода за parent/viewport/silent-clips); snapshot-тесты Painter.items (10 существующих kit-компонентов × 3 состояния × 2 языка = 60 эталонов); perf baseline; kit.rs hygiene (общий хелпер `viewport_clamp(rect, viewport)` для dropdown/tooltip/modal/toast_area — устранение дублирующих ручных clamp'ов); устранение найденных G4-нарушений.
- **W1 — taffy opt-in (4–6 недель, ~10–15 сессий, = ADR-0014 P1+P2)**: `trait LayoutBackend` + `NativeBackend` (перенос FR-062 1:1) + `TaffyBackend` skeleton + adapter; pilot на 2 поверхностях (`kit_ui::explain_dialog` + `fr061_row_grid`); 10 HTML5 demos (mdn/css-tricks топ-10); viewport-clamp + overflow/scroll на taffy; `PaintItem::ClipRect` в Painter. Taffy — за фичей `taffy` (default off).
- **W2 — cosmic-text trait boundary + свой layout-движок (8–12 недель, ~20–30 сессий)**: `Shaper` trait в canvas-ui (cosmic-text — default impl, mock для тестов); `FlexLayoutEngine` в `canvas-ui/src/layout/flex.rs` (grow/shrink/basis/wrap, побитовая идентичность с taffy на совместимых политиках — regression test 1000 случайных деревьев); `Overflow/Clip/Scroll` примитивы (поверх `PaintItem::ClipRect`); 5 дополнительных HTML5 demos (CanvasDesk-специфичные). `NativeBackend` переключается на `FlexLayoutEngine` (если фича `taffy` off — свой движок; если on — taffy). Taffy остаётся для Grid.
- **W3 — своя UI-библиотека (8–12 недель, ~20–30 сессий)**: kit.rs 2 757 строк → 6 компонентов × ~500 строк (`button.rs`, `panel.rs`, `dropdown.rs`, `text_field.rs`, `list.rs`, `modal.rs`); `Component` trait (`props() -> Props`, `layout(backend) -> Vec<UiRect>`, `paint(painter)`); retained-state для инкрементального reflow (если профиль кадра W2 покажет > 1 мс); миграция canvas-app/render (~497 мест) на компонентное API.
- **W4 — dep-минимизация (4–6 недель, ~10–15 сессий)**: taffy вырезается (если `FlexLayoutEngine` покрывает использованные фичи — grep-аудит `--features taffy` в коде W3); cosmic-text остаётся за `Shaper` trait (миграция на fontdue/свой layout — отдельный ADR при появлении триггера); rustybuzz/ttf-parser — мигрируют с cosmic-text; final state: `cargo build --no-default-features` = 0 внешних UI-runtime-deps (кроме cosmic-text за trait).

**Зачем.** Запрос владельца: «прийти к полностью своей UI-библиотеке, которая будет иметь все сильные стороны и минимизирует внешние зависимости» — требует долгосрочной стратегии. ADR-0014 (taffy opt-in) — тактическое решение «внедрить taffy быстро»; FR-068 — стратегия «прийти к своей UI-библиотеке». Критический разбор ADR-0015 выявил 9 слабостей ADR-0014 (двойная поддержка навсегда, retained-конфликт, `SqueezeTail` ≠ `flex_shrink`, замер wasm, профиль кадра, 4-я тяжёлая зависимость, нет конечной цели, 10 HTML5 demos не описаны, G4 расширение не специфицировано) — все закрываются волнами W0–W4.

**Границы FR-068:**

- `trait LayoutBackend` (W1) и `Component` trait (W3) — НОВЫЕ; сигнатуры `Row::lay_out`/`Column::lay_out`/`grid_cells`/`Child`/`MeasuredItem`/`RowPolicy` — **СТАБИЛЬНЫ** (контракт PRD-0009 §7.4 V-5: потребители не переписываются до W3).
- Фичи `taffy` (W1), `flex-engine` (W2) — НЕ default; B2B zero-dep инвариант сохранён (default сборка без фич = 0 внешних UI-runtime-deps, кроме cosmic-text за trait).
- `UiLayer` (FR-051, 9 полос), `SurfaceRegistry`/`capture`, `KeyboardRouter`, `Painter` (кроме W1 `ClipRect`), `geometry.rs` — **НЕ трогаются**; движок вёрстки ортогонален слоям/capture/draw-порядку (D8 ADR-0013).
- Вся работа — внутри `canvas-ui/src/` (новые модули `layout/{flex.rs, backend.rs}`, `shaper.rs`, `component/`, `tests/{g4_lint, snapshot, html5_demos}/`) + активация фич в `Cargo.toml` + staged миграция потребителей в W3.

## Влияние (Impact)

| Объект | Что меняется | Где в документации |
|---|---|---|
| `canvas-ui/src/layout.rs` (W1) | НОВЫЙ `trait LayoutBackend` (`lay_out_row/column/grid/measured` + `features() -> LayoutFeatures`); `NativeBackend` (перенос FR-062 1:1); `TaffyBackend` за `#[cfg(feature="taffy")]`. `Row/Column` делегируют в backend. | `docs/ui-kit.md` §«Layout-примитивы», `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` |
| `canvas-ui/src/layout/flex.rs` (W2) | НОВЫЙ `FlexLayoutEngine` (grow/shrink/basis/wrap, побитовая идентичность с taffy); `Overflow/Clip/Scroll` примитивы. `NativeBackend` переключается на `FlexLayoutEngine` (если `taffy` off). | `docs/ui-kit.md` §«Свой layout-движок», `docs/adr/adr-0015-long-term-ui-stack-strategy.md` |
| `canvas-ui/src/shaper.rs` (W2) | НОВЫЙ `Shaper` trait (изолирует cosmic-text); cosmic-text — default impl за `#[cfg(not(feature="mock-shaper"))]`; mock для тестов. | `docs/ui-kit.md` §«Shaper trait» |
| `canvas-ui/src/component/` (W3) | НОВАЯ директория: `button.rs`, `panel.rs`, `dropdown.rs`, `text_field.rs`, `list.rs`, `modal.rs` (по ~500 строк каждый); `Component` trait (`props/layout/paint`). kit.rs 2 757 → ~500 (реэкспорты). | `docs/ui-kit.md` §«Компонентное дерево» |
| `canvas-ui/src/paint.rs` (W1) | РАСШИРЕНИЕ `PaintItem` — `PaintItem::ClipRect { rect: UiRect, items: Vec<PaintItem> }` для overflow:hidden; consumer (canvas-app kit_ui) конвертирует в scissor FR-056. | `docs/ui-kit.md` §«Painter» |
| `canvas-ui/src/kit.rs` (W0) | Hygiene: общий хелпер `viewport_clamp(rect, viewport: UiRect) -> UiRect` для dropdown/tooltip/modal/toast_area — устранение дублирующих ручных clamp'ов (~5 функций, ~15 строк на каждую). | `docs/ui-kit.md` §«Kit-компоненты» |
| `canvas-ui/tests/g4_lint.rs` (W0) | НОВЫЙ layout-линт: 5 canonical сцен × 3 окна × 2 языка = 30 прогонов; проверки выхода за parent/viewport/silent-clips; × N backend'ов (default + `--features taffy` W1+ + `--features flex-engine` W2+). | `docs/prd/prd-0009-ui-layering-uikit.md` §AC-3 |
| `canvas-ui/tests/snapshot.rs` (W0) | НОВЫЕ snapshot-тесты: 10 kit-компонентов × 3 состояния × 2 языка = 60 эталонов; `Painter.items()` → строка-дамп (округление до целого ui px, сортировка). | `docs/ui-kit.md` §«Snapshot-тесты» |
| `canvas-ui/tests/html5_demos/` (W1) | НОВАЯ директория: 10 эталонных HTML5 demo-layouts (mdn/css-tricks топ-10); golden-снапшоты `UiRect`-дампов. | `docs/ui-kit.md` §«Эталоны вёрстки» |
| `canvas-ui/tests/html5_demos/cd_*.rs` (W2) | +5 CanvasDesk-специфичных demos (palette-grid-multiline, whatif-bar-squeeze-tail-parity, kit-gallery-tab-focus, search-overlay-viewport-clip, fr061-tabular-body-grid). | `docs/ui-kit.md` §«Эталоны вёрстки» |
| `canvas-ui/tests/flex_vs_taffy_parity.rs` (W2) | НОВЫЙ regression test: 1000 случайных деревьев → `FlexLayoutEngine` vs `TaffyBackend` → побитово идентичные rect'ы для совместимых политик (`Fit`/`Start`/`End`/`SpaceBetween`/`Wrap`/равная 2D-сетка). | `docs/ui-kit.md` §«Паритет backend'ов» |
| `canvas-ui/Cargo.toml` | НОВЫЕ фичи: `taffy = ["dep:taffy"]` (W1), `flex-engine = []` (W2 — встроенный, 0 deps), `mock-shaper = []` (W2 — тестовый мок). default = `[]` (zero-dep). | `docs/DEPENDENCIES.md` §3→§2 (taffy после W1; cosmic-text — за trait, не меняется) |
| `canvas-app/src/*.rs` (W3) | ~497 мест `Child::fixed`+`lay_out` — миграция на компонентное API (`Component` trait); сигнатуры `Row::lay_out` НЕ меняются до W3. | `docs/ui-kit.md` §«Карта переносов» |
| `canvas-render/src/{search_ui,row_grid}.rs` (W3) | ~30 мест — миграция. | — |
| Wasm-бандл | W1: +~376 КБ raw / ~180 КБ gzip (taffy); W4: −~376 КБ raw (taffy вырезан). W2: +~5–15 КБ (FlexLayoutEngine, mock-shaper). Default (без фич) — 0 прироста. | `docs/plans/wasm-port.md` §8.8 |
| Документация | `docs/DEPENDENCIES.md` (taffy W1→§2, W4→§3 отвергнутые); `docs/SPEC.md` §6.3 (комментарий backend'ов); `docs/ui-kit.md` (карта переносов, эталоны, snapshot-тесты, паритет); `docs/adr/adr-0015-long-term-ui-stack-strategy.md` (стратегия); `docs/change-requests/index-cr-fr.md` (статус FR-068). | — |

## Анализ (Root Cause)

Слой canvas-ui после FR-051/053/056/057/058/059/060/062 (2026-09-23, main `64ec34d`): 7 080 строк чистой геометрии без wgpu/winit (headless-TDD, 128 тестов). Immediate-mode layout-примитивы V-5 (Row/Column/grid_cells/stack/constrain/pad), TextMeasurer (cosmic-text шейпинг + кэш), Painter (draw-слой PaintItem), UiLayer (9 полос), SurfaceRegistry (capture/hit), KitState/WidgetState (FR-057). Потребители layout API: ~497 мест (canvas-app ~400, canvas-render ~30, canvas-ui ~62).

Критический разбор ADR-0014 (ADR-0015 §Критический разбор) выявил 9 слабостей:

- **C1. Двойная поддержка навсегда.** ADR-0014 §Решение п.5: «`NativeBackend` остаётся как fast-path/фолбэк (P3)» — без финального состояния. → FR-068 W4: taffy вырезается.
- **C2. Retained-дерево taffy vs immediate-mode.** ADR-0014 §Решение п.3: «пересоздаёт `TaffyTree` на кадр (опция P3 — retained node-id)». → FR-068 W1: явная декларация immediate overhead; W3: retained-state для `Component` trait (если профиль W2 покажет > 1 мс).
- **C3. `SqueezeTail` ≠ `flex_shrink`.** ADR-0014 §Контракт-4: «документированное расхождение». → FR-068 W2: `FlexLayoutEngine` реализует `SqueezeTail` дословно (хвост до 0 ширины, half-open hit); taffy — fallback за фичей.
- **C4. Замер wasm без учёта маржинальности.** ADR-0014 §Валидация: «+376 КБ raw». → FR-068 W1: обязать замер на canvas-web cdylib ДО merge.
- **C5. Профиль кадра не замерен.** ADR-0014 §Валидация: «60 fps + ≤1 мс». → FR-068 каждая волна: perf-гейт (reflow 1000 узлов < 1 мс).
- **C6. 4-я тяжёлая UI-зависимость.** Taffy + cosmic-text + rustybuzz + ttf-parser (3 с RUSTSEC). → FR-068 W2: cosmic-text за `Shaper` trait; W4: taffy вырезается.
- **C7. Нет конечной цели «своя UI-библиотека».** ADR-0014 — план «как внедрить taffy». → FR-068 W4: `cargo build --no-default-features` = 0 внешних UI-runtime-deps.
- **C8. 10 HTML5 demos не описаны.** ADR-0014 §Решение п.6: «mdn/css-tricks топ-10». → FR-068 ADR-0015 §Решение п.4: канонический список 15 (10 W1 + 5 W2).
- **C9. G4 расширение не специфицировано.** ADR-0014 §P2: «новые проверки». → FR-068 ADR-0015 §Решение п.5: 5 canonical сцен × 3 окна × 2 языка = 30 прогонов × N backend'ов.

## Требуемые изменения (Changes)

### Волна W0 — UI hygiene (2–3 недели)

| Что | Где | Как |
|---|---|---|
| Расширение G4-линта | `canvas-ui/tests/g4_lint.rs` (новый) | 5 canonical сцен: (1) main canvas + whatif bar (L3), (2) palette dropdown open (L4), (3) explain modal (L5), (4) search overlay (L4), (5) settings panel (L3). × 3 окна (1280×800/1024×640/800×560) × 2 языка (RU/EN) = 30 прогонов. Проверки: (1) `UiRect::intersection(rect, parent).is_some()` для всех видимых элементов; (2) `UiRect::intersection(rect, viewport).is_some()` для L4+; (3) 0 silent-clips (grep-аудит `take(`/`break`/`truncate` в мигрированном коде — `Option::take` для переноса состояния разрешён). |
| Snapshot-тесты Painter.items | `canvas-ui/tests/snapshot.rs` (новый) | 10 kit-компонентов (Panel/Button/IconButton/Dropdown/Chip/Toast/Tooltip/Modal/TextField/Switch) × 3 состояния (default/hover/disabled) × 2 языка = 60 эталонов. `Painter.items()` → нормализованная строка-дамп (округление до целого ui px, сортировка по `(x, y, w, h, type)`); эталоны в `tests/snapshot/*.txt`; изменение — осознанный PR с diff. |
| Perf baseline | `canvas-ui/tests/perf_baseline.rs` (новый, `#[ignore]` по умолчанию) | reflow 1000 узлов (синтетический граф) → μs; 60 fps на 5000 нод — SPEC §6.3. Запуск: `cargo test -- --ignored perf_baseline`. Запись baseline в `tests/perf_baseline.txt`; регрессия > 20% — fail. |
| kit.rs hygiene | `canvas-ui/src/kit.rs:389,423,478,458` | Общий хелпер `pub fn viewport_clamp(rect: UiRect, viewport: UiRect) -> UiRect { rect.intersection(&viewport).unwrap_or(rect) }` — устранение дублирующих ручных clamp'ов в `dropdown_menu`/`tooltip`/`modal`/`toast_area` (~5 функций, ~15 строк на каждую → ~5 строк на функцию). |
| Устранение G4-нарушений | `canvas-ui/src/kit.rs`, `canvas-app/src/*.rs` | По результатам G4-линта — исправление найденных выходов за parent/viewport/silent-clips. Ожидание: 0–10 нарушений (аудит W0); каждое — отдельный под-коммит. |

**Гейты W0:**
- `cargo test -p canvas-ui --test g4_lint` — 30 прогонов, 0 нарушений.
- `cargo test -p canvas-ui --test snapshot` — 60 эталонов зелёные (или осознанный diff).
- `cargo test -p canvas-ui -- --ignored perf_baseline` — baseline зафиксирован.
- `cargo test --workspace` — все тесты зелёные.
- `cargo clippy -D warnings`; `cargo fmt --check`.
- `scripts/wasm_gate.sh --check` — зелёный (0 прироста).

### Волна W1 — taffy opt-in (4–6 недель, = ADR-0014 P1+P2)

| Что | Где | Как |
|---|---|---|
| `trait LayoutBackend` | `canvas-ui/src/layout.rs` | `pub trait LayoutBackend { fn lay_out_row(&self, row: Row, slot: UiRect, items: &[Child]) -> Vec<UiRect>; fn lay_out_column(...); fn lay_out_measured(...); fn lay_out_grid(...); fn features(&self) -> LayoutFeatures; }`. `LayoutFeatures` — битовая маска: `FLEX_GROW/FLEX_SHRINK/FLEX_BASIS/FLEX_WRAP/GRID_2D/AUTO_SIZE/OVERFLOW_CLIP/PERCENT/ASPECT_RATIO/STICKY`. `Row::lay_out(slot, items)` делегирует в `default_backend()`. |
| `NativeBackend` | `canvas-ui/src/layout.rs` | `pub struct NativeBackend;` — перенос текущих `Row/Column/grid_cells` (FR-062 F-13…F-18) в методы трейта 1:1 (поведение идентично). |
| `TaffyBackend` | `canvas-ui/src/layout/taffy_backend.rs` (новый) | `#[cfg(feature="taffy")] pub struct TaffyBackend { tree: taffy::TaffyTree }`. Адаптер: `Row{gap, main, cross, policy}` → `taffy::Style`; `Child::fixed/spacer/flexible` → `Style { size, flex_grow, flex_shrink, flex_basis }`; `MeasuredItem::Text` → `Style + measure_func` (closure на `TextMeasurer::width_of`); `RowPolicy::Wrap` → `flex_wrap: Wrap`; `SqueezeTail` → `flex_shrink: 1.0, min_width: 0` (расхождение C3 — документировано); `grid_cells` → `display: Grid + grid_template_columns: repeat(n, 1fr)`. `compute_layout(root, AvailableSpace::Definite(slot.size()))` → чтение `tree.layout(node)` → `Vec<UiRect>`. |
| Фича `taffy` | `canvas-ui/Cargo.toml` + корневой | `[features] taffy = ["dep:taffy"]`; `[dependencies] taffy = { workspace = true, optional = true }`. Workspace: `taffy = { version = "0.14", default-features = false, features = ["std", "taffy_tree", "flexbox", "grid", "block"] }`. |
| Pilot: `kit_ui::explain_dialog` + `fr061_row_grid` | `crates/canvas-app/src/{explain_ui,kit_ui}.rs`, `crates/canvas-render/src/row_grid.rs` | Перевод с `NativeBackend` на `TaffyBackend` (через backend-выбор); golden FR-062 F-18 — тот же оракул (для совместимых политик побитово). |
| 10 HTML5 demos | `canvas-ui/tests/html5_demos/` (новый) | (1) sticky-header-column, (2) sidebar-content-overflow-auto, (3) flexbox-navbar-space-between, (4) css-grid-12-col-responsive, (5) masonry-lite, (6) card-list-aspect-ratio, (7) modal-position-fixed-viewport-clip, (8) dropdown-flip, (9) scrollable-list-virtualization, (10) complex-form-layout. Каждый — `.rs` + golden `.txt` (UiRect-дамп, методология FR-062 F-18). |
| `PaintItem::ClipRect` | `canvas-ui/src/paint.rs` | `PaintItem::ClipRect { rect: UiRect, items: Vec<PaintItem> }` — clip-семантика в draw-слой; consumer (canvas-app kit_ui) конвертирует в scissor FR-056. |
| viewport-clamp + overflow/scroll на taffy | `canvas-ui/src/kit.rs:389,423,478,458,741` | `dropdown_menu/tooltip/modal/toast_area` — `position: absolute/fixed` через `AvailableSpace::Definite(viewport.size())` + `viewport_clamp` (из W0); `ScrollState` → `overflow:hidden` (taffy `overflow: Hidden` в Style) + `ScrollState.offset` для content-shift + `Painter::clip_rect`. |

**Гейты W1:**
- `cargo test -p canvas-ui` (default) — G4-линты + golden FR-062 F-18 на `NativeBackend`.
- `cargo test -p canvas-ui --features taffy` — G4-линты + golden на `TaffyBackend`.
- `cargo test -p canvas-ui --test html5_demos --features taffy` — 10/10 golden зелёные.
- `cargo build --no-default-features` — `TaffyBackend` не компилируется, zero-dep.
- `cargo build --release --target wasm32-unknown-unknown --features canvas-ui/taffy` (через canvas-web cdylib) — замер wasm raw/brotli; ≤8 МБ raw / ≤4 МБ brotli (wasm-port.md §8.8); прирост +370…380 КБ raw / +170…190 КБ gzip к default.
- `scripts/wasm_gate.sh --check` (default) + `cargo deny check` (taffy в allowlist).
- `cargo clippy -D warnings` (default + `--features taffy`); `cargo fmt --check`.
- Perf: reflow 1000 узлов < 1 мс (на TaffyBackend, immediate — пересоздание `TaffyTree`).

### Волна W2 — cosmic-text trait boundary + свой layout-движок (8–12 недель)

| Что | Где | Как |
|---|---|---|
| `Shaper` trait | `canvas-ui/src/shaper.rs` (новый) | `pub trait Shaper { fn shape(&mut self, text: &str, spec: &TextSpec) -> Measured; fn font_system(&mut self) -> &mut cosmic_text::FontSystem; }`. `CosmicShaper` (default impl, за `#[cfg(not(feature="mock-shaper"))]`) — обёртка над существующим `TextMeasurer`; `MockShaper` (за `#[cfg(feature="mock-shaper")]`) — детерминированные ширины для тестов. `TextMeasurer` мигрирует на `dyn Shaper`. |
| `FlexLayoutEngine` | `canvas-ui/src/layout/flex.rs` (новый) | `pub struct FlexLayoutEngine;` — реализация `LayoutBackend` без taffy. Flexbox: `grow/shrink/basis/wrap` (CSS-семантика, побитовая идентичность с taffy на совместимых политиках); `SqueezeTail` — дословно (хвост до 0, half-open hit); `Overflow/Clip/Scroll` — `overflow:hidden` (клиппинг через `Painter::ClipRect`), `overflow:auto` (scroll-контейнер через `ScrollState`). Grid — non-goal (taffy fallback за фичей). |
| Фича `flex-engine` | `canvas-ui/Cargo.toml` | `[features] flex-engine = []` (встроенный, 0 deps); default — `flex-engine` включён (если `taffy` off — `NativeBackend` использует `FlexLayoutEngine`). |
| Фича `mock-shaper` | `canvas-ui/Cargo.toml` | `[features] mock-shaper = []` (тестовый мок, не для продакшна). |
| `NativeBackend` переключение | `canvas-ui/src/layout.rs` | `default_backend()` → `FlexLayoutEngine` (если `taffy` off) или `TaffyBackend` (если `taffy` on). W4: `taffy` вырезается, `FlexLayoutEngine` — единственный. |
| 5 CanvasDesk-специфичных demos | `canvas-ui/tests/html5_demos/cd_*.rs` | (11) palette-grid-multiline, (12) whatif-bar-squeeze-tail-parity, (13) kit-gallery-tab-focus, (14) search-overlay-viewport-clip, (15) fr061-tabular-body-grid. |
| Паритет regression test | `canvas-ui/tests/flex_vs_taffy_parity.rs` (новый) | 1000 случайных деревьев (детерминированный seed) → `FlexLayoutEngine` vs `TaffyBackend` → побитово идентичные `Vec<UiRect>` для совместимых политик (`Fit`/`Start`/`End`/`SpaceBetween`/`Wrap`/равная 2D-сетка). Для `SqueezeTail` — `FlexLayoutEngine` даёт дословную семантику, `TaffyBackend` — `flex_shrink` (расхождение C3 — документировано, не fail). |

**Гейты W2:**
- `cargo test -p canvas-ui` (default, `flex-engine` on, `taffy` off) — G4-линты + golden + 15 HTML5 demos на `FlexLayoutEngine`.
- `cargo test -p canvas-ui --features taffy` — те же тесты на `TaffyBackend` (для совместимых политик побитово).
- `cargo test -p canvas-ui --features mock-shaper` — тесты с `MockShaper` (детерминированные ширины).
- `cargo test -p canvas-ui --test flex_vs_taffy_parity` — 1000 деревьев, побитовая идентичность на совместимых политиках ≥ 80%.
- `cargo build --no-default-features` — `TaffyBackend` не компилируется; `FlexLayoutEngine` — встроенный (0 deps); `cargo build --no-default-features --features mock-shaper` — `MockShaper` компилируется.
- `cargo build --release --target wasm32-unknown-unknown` (default) — прирост ~5–15 КБ raw (FlexLayoutEngine) к default; без taffy.
- `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.
- Perf: reflow 1000 узлов < 1 мс (на `FlexLayoutEngine`).

### Волна W3 — своя UI-библиотека (8–12 недель)

| Что | Где | Как |
|---|---|---|
| `Component` trait | `canvas-ui/src/component/mod.rs` (новый) | `pub trait Component { type Props; fn props(&self) -> &Self::Props; fn layout(&self, backend: &dyn LayoutBackend, slot: UiRect) -> Vec<UiRect>; fn paint(&self, painter: &mut Painter, rects: &[UiRect]); fn hit_test(&self, rects: &[UiRect], point: UiPoint) -> Option<HitTarget>; }`. Retained-state — `Component` хранит `state: WidgetState` (FR-057); между кадрами — стабильный `node_id` для инкрементального reflow (если профиль W2 покажет > 1 мс). |
| 6 компонентов | `canvas-ui/src/component/{button,panel,dropdown,text_field,list,modal}.rs` (новые) | kit.rs 2 757 → ~500 (реэкспорты `button_layout`/`panel_style`/...); каждый компонент ~500 строк: `Props` struct, `Component` impl, `paint` через `Painter`, `hit_test` через `UiRect::contains`. `button.rs` (~500), `panel.rs` (~400), `dropdown.rs` (~600 — viewport-clamp), `text_field.rs` (~700 — TextFieldModel + caret), `list.rs` (~500 — ScrollState + list_rows), `modal.rs` (~300). |
| Миграция canvas-app/render (~497 мест) | `crates/canvas-app/src/*.rs`, `crates/canvas-render/src/*.rs` | `Row{...}.lay_out(slot, &items)` → `component.layout(backend, slot)` (через `Component` trait); `Child::fixed(w, h)` где есть measure → `MeasuredItem::Text` (auto-size); manual `width_of → Child::fixed` — удалить (закрытие класса багов текст-мера). |
| Retained-state (опционально) | `canvas-ui/src/component/mod.rs` | Если профиль W2 (reflow 1000 узлов на `FlexLayoutEngine`) > 1 мс — `Component` хранит `node_id: stable` между кадрами; `LayoutBackend` поддерживает `lay_out_retained(root: NodeId, ...)` (для `FlexLayoutEngine` — dirty-кэш, для `TaffyBackend` — taffy built-in cache). Если ≤ 1 мс — не вводить (KISS). |

**Гейты W3:**
- `cargo test --workspace` (default + `--features taffy`) — все тесты зелёные на компонентном API.
- `cargo test -p canvas-ui --test snapshot` — 60 эталонов зелёные (или осознанный diff — компонентное дерево меняет структуру Painter.items, разрешено с diff в PR).
- `cargo test -p canvas-ui --test g4_lint` — 30 прогонов × 2 backend'а, 0 нарушений.
- `cargo test -p canvas-ui --test html5_demos` — 15/15 golden зелёные.
- `grep -r "Child::fixed" crates/ | wc -l` — снижение на ~30–50% (где заменено на `MeasuredItem::Text` auto-size и `Component::layout`).
- kit.rs ≤ 500 строк (реэкспорты); 6 компонентов × ~500 строк каждый.
- Perf: reflow 1000 узлов < 1 мс; 60 fps на 5000 нод — сохраняется.
- `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.

### Волна W4 — dep-минимизация (4–6 недель)

| Что | Где | Как |
|---|---|---|
| Вырезание taffy | `canvas-ui/Cargo.toml`, `canvas-ui/src/layout/taffy_backend.rs` | `grep -r "taffy" crates/ | grep -v "taffy_backend.rs"` — если 0 использований `TaffyBackend` (или только за `#[cfg(feature="taffy")]` для Grid-only сценариев): удалить `taffy_backend.rs`, убрать фичу `taffy` и `taffy` из `[workspace.dependencies]`. Если Grid используется в >3 местах — оставить фичу `taffy` для Grid-only (S7 — приемлемо). |
| Cosmic-text за `Shaper` trait | `canvas-ui/src/shaper.rs` (W2) — подтверждение | `CosmicShaper` остаётся единственной impl; `cargo build --no-default-features` = 0 внешних UI-runtime-deps кроме `cosmic-text` (за trait). Миграция на fontdue/ab_glyph + свой layout — отдельный ADR при появлении триггера (RUSTSEC-эскалация или явная потребность). |
| rustybuzz/ttf-parser | (транзитивные через cosmic-text) | Мигрируют с cosmic-text (когда cosmic-text обновится или будет заменён). Не отдельная задача FR-068. |
| Final state проверка | `cargo tree -p canvas-ui --no-default-features` | 0 внешних крейтов кроме `cosmic-text` (и его транзитивных). `cargo build --no-default-features` = zero-dep (кроме cosmic-text). `cargo deny check` зелёный. |

**Гейты W4:**
- `cargo build --no-default-features` — 0 внешних UI-runtime-deps (кроме cosmic-text за trait).
- `cargo tree -p canvas-ui --no-default-features` — проверка dep-дерева.
- `cargo test --workspace` — все тесты зелёные (taffy вырезан, `FlexLayoutEngine` единственный).
- `cargo test -p canvas-ui --test html5_demos` — 15/15 golden зелёные (на `FlexLayoutEngine`).
- `cargo test -p canvas-ui --test snapshot` — 60 эталонов зелёные.
- `cargo test -p canvas-ui --test g4_lint` — 30 прогонов, 0 нарушений.
- `cargo build --release --target wasm32-unknown-unknown` — прирост ~5–15 КБ (FlexLayoutEngine) к default; БЕЗ taffy (−376 КБ raw относительно W1).
- `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.
- Perf: reflow 1000 узлов < 1 мс; 60 fps на 5000 нод — сохраняется.

## Контракты на стыках

- **§Контракт-1 — интерфейс V-5 PRD-0009 стабилен до W3.** Сигнатуры `Row::lay_out(slot, items) -> Vec<UiRect>`, `Column::lay_out(...)`, `grid_cells(...)`, `Child::fixed/spacer/flexible`, `MeasuredItem`, `RowPolicy`, `MainAlign/CrossAlign` — НЕ меняются до W3. W3 вводит `Component` trait — новая абстракция, `Row/Column` остаются как API (делегируют в `Component::layout`).
- **§Контракт-2 — фичи default-off (zero-dep).** `taffy` (W1), `flex-engine` (W2 — встроенный, 0 deps), `mock-shaper` (W2). Default сборка — 0 внешних UI-runtime-deps (кроме cosmic-text за trait). B2B-инвариант PRD-0009 G7 сохранён.
- **§Контракт-3 — G4-линты × N backend'ов.** `cargo test -p canvas-ui --test g4_lint` (default NativeBackend/FlexLayoutEngine) + `--features taffy` (TaffyBackend W1+) — оба обязаны быть зелёными. 5 canonical сцен × 3 окна × 2 языка = 30 прогонов × N backend'ов.
- **§Контракт-4 — `SqueezeTail` ≠ `flex_shrink` — `FlexLayoutEngine` решает.** W2: `FlexLayoutEngine` реализует `SqueezeTail` дословно (хвост до 0, half-open hit); `TaffyBackend` — `flex_shrink: 1.0, min_width: 0` (расхождение C3 — документировано, не fail в parity test). Потребитель выбирает backend (default — `FlexLayoutEngine`).
- **§Контракт-5 — `UiLayer`/`Painter`/`SurfaceRegistry` ортогональны.** Движок вёрстки не меняет слои/capture/draw-порядок (D8 ADR-0013). W1 добавляет `PaintItem::ClipRect` (clip-семантика в draw-слой), не меняя UiLayer (9 полос).
- **§Контракт-6 — wasm-гейты зелёные на каждой волне.** `scripts/wasm_gate.sh --check` (default) + замер с `--features taffy` (W1+). `scripts/mcp_wasm_gate.sh --check` — без изменений (canvas-mcp не зависит от canvas-ui).
- **§Контракт-7 — замер wasm обязателен.** Каждая волна — `cargo build --release --target wasm32-unknown-unknown` (через canvas-web cdylib) + отчёт raw/brotli. Бюджет: ≤8 МБ raw / ≤4 МБ brotli (wasm-port.md §8.8). Превышение — явное решение владельца.
- **§Контракт-8 — perf-бюджет.** reflow 1000 узлов < 1 мс; 60 fps на 5000 нод (SPEC §6.3) — сохраняется на каждой волне. Замер: `cargo test -- --ignored perf_baseline`. Регрессия > 20% — fail.
- **§Контракт-9 — snapshot-тесты.** 60 эталонов (10 kit-компонентов × 3 состояния × 2 языка) — `Painter.items()` → строка-дамп. Изменение — осознанный PR с diff. W3 (компонентное дерево) — разрешено изменить эталоны с diff в PR (структура Painter.items меняется).

**Подчеркнуть (границы владений):** НЕ трогать `geometry.rs`, `UiLayer`, `SurfaceRegistry`/`capture`, `KeyboardRouter`, `frame.rs`, `hit.rs`, `widget.rs` (кроме W3 — `WidgetState` интегрируется в `Component`). Сигнатуры `Row::lay_out`/`Column::lay_out`/`grid_cells`/`Child`/`MeasuredItem`/`RowPolicy` — НЕ менять до W3. Вся работа — внутри `canvas-ui/src/` (новые модули `layout/{flex.rs, backend.rs, taffy_backend.rs}`, `shaper.rs`, `component/`, `tests/{g4_lint, snapshot, html5_demos, flex_vs_taffy_parity, perf_baseline}/`) + активация фич в `Cargo.toml` + staged миграция потребителей в W3.

## Фазы и проверяемые коммиты

5 волн — каждая поставляема и приёмлема независимо. Между волнами — G4-аудит + замер wasm + golden-сравнение. Волны W1 и W2 — частично параллельны (W2 стартует после W0, не ждёт W1).

### W0 — UI hygiene (2–3 недели)

- **Коммит(ы):** `test(ui): G4-lint + snapshot tests + perf baseline (FR-068 W0)`, `refactor(ui/kit): viewport_clamp helper (FR-068 W0)`, `fix(ui): G4 violations (FR-068 W0)` (если найдены)
- **Что:** см. §Требуемые изменения W0.
- **Гейт:** см. §Гейты W0.

### W1 — taffy opt-in (4–6 недель, = ADR-0014 P1+P2)

- **Коммит(ы):** `feat(ui/layout): trait LayoutBackend + NativeBackend + TaffyBackend pilot (FR-068 W1, ADR-0014)`, `feat(ui/kit): viewport-clamp + overflow/scroll on TaffyBackend (FR-068 W1)`, `test(ui): 10 HTML5 demo golden (FR-068 W1)`
- **Что:** см. §Требуемые изменения W1.
- **Гейт:** см. §Гейты W1.

### W2 — cosmic-text trait boundary + свой layout-движок (8–12 недель)

- **Коммит(ы):** `feat(ui/shaper): Shaper trait + CosmicShaper/MockShaper (FR-068 W2)`, `feat(ui/layout): FlexLayoutEngine — flexbox + overflow/clip/scroll (FR-068 W2)`, `test(ui): flex_vs_taffy_parity + 5 CD demos (FR-068 W2)`
- **Что:** см. §Требуемые изменения W2.
- **Гейт:** см. §Гейты W2.

### W3 — своя UI-библиотека (8–12 недель)

- **Коммит(ы):** `feat(ui/component): Component trait + 6 components (FR-068 W3)`, `refactor(app+render): migrate to Component API (FR-068 W3)` (возможно 2–3 коммита по подсистемам)
- **Что:** см. §Требуемые изменения W3.
- **Гейт:** см. §Гейты W3.

### W4 — dep-минимизация (4–6 недель)

- **Коммит:** `chore(ui): remove taffy — FlexLayoutEngine covers all used features (FR-068 W4)` (если taffy не используется) ИЛИ `chore(ui): taffy stays for Grid-only — grep audit (FR-068 W4)` (если Grid используется)
- **Что:** см. §Требуемые изменения W4.
- **Гейт:** см. §Гейты W4.

## Ограничения для агента-реализатора

- **Поэтапность — обязательно.** Не делать W1 до зелёного W0; не делать W2 до зелёного W1 (или параллельно — см. §Контракт-9); не делать W3 до зелёного W2; не делать W4 до зелёного W3.
- **Гибрид `trait LayoutBackend`** — не полная замена. `NativeBackend`/`FlexLayoutEngine` остаётся; потребители не переписываются до W3 (сигнатуры стабильны).
- **`SqueezeTail` ≠ `flex_shrink` — `FlexLayoutEngine` решает (W2).** Не пытаться выровнять `TaffyBackend` дословно — `FlexLayoutEngine` реализует `SqueezeTail` дословно (хвост до 0, half-open hit); taffy — fallback за фичей для Grid-only.
- **Cosmic-text — trait boundary (W2).** `Shaper` trait; cosmic-text — default impl. Не заменять cosmic-text на fontdue/ab_glyph в этом цикле — вне скоупа (отдельный ADR при появлении триггера).
- **`cargo build --no-default-features` — zero-dep инвариант.** Без `taffy`/`mock-shaper` фич — 0 внешних UI-runtime-deps (кроме cosmic-text за trait). `cargo deny check` зелёный.
- **Golden-эталоны HTML5 demos — побитово идентичны между backend'ами** только для совместимых политик (`Fit`/`Start`/`End`/`SpaceBetween`/`Wrap`/равная 2D-сетка). Для taffy-only (неравные треки, `flex_shrink`, `aspect-ratio`, `percent`) — `TaffyBackend` единственный оракул; `FlexLayoutEngine` не поддерживает, golden создаётся под taffy (W1).
- **Не трогать:** `geometry.rs`, `UiLayer`, `SurfaceRegistry`/`capture`, `KeyboardRouter`, `Painter` (кроме W1 `ClipRect`), `frame.rs`, `hit.rs`, `widget.rs` (кроме W3 интеграции в `Component`). Сигнатуры `Row::lay_out`/`Column::lay_out`/`grid_cells`/`Child`/`MeasuredItem`/`RowPolicy` — НЕ менять до W3.
- **Эскалация:** если W2 (`FlexLayoutEngine`) не достигает побитовой идентичности с taffy на >80% совместимых политик — taffy остаётся (W4 откладывается); если W3 (компонентное дерево) ухудшает perf > SPEC §6.3 — retained-state обязателен; если W4 grep-аудит показывает использование Grid в >3 местах — taffy остаётся за фичей для Grid-only (S7 — приемлемо).

## Наглядная проверка (5 минут)

**Демо.** 15 эталонных HTML5 demo-layouts в `canvas-ui/tests/html5_demos/` — каждый рендерится 1:1 в canvas-ui через `Painter` + `UiRect`-дамп; golden-снапшоты сравниваются с эталоном (mdn/css-tricks топ-10 + 5 CanvasDesk-специфичных). На demo-визуализации: sticky-header-column, sidebar-content-overflow-auto, css-grid-12-col, masonry-lite, palette-grid-multiline, whatif-bar-squeeze-tail-parity — видны как корректные rect'ы (без выхода за parent, без выхода за viewport, без наложений).

**Автотесты (зелёные):**

- `cargo test -p canvas-ui` (default, W2+ — `FlexLayoutEngine`) — G4-линты + golden + 15 HTML5 demos + snapshot (60 эталонов).
- `cargo test -p canvas-ui --features taffy` (W1+) — те же тесты на `TaffyBackend` (для совместимых политик побитово).
- `cargo test -p canvas-ui --test flex_vs_taffy_parity` (W2+) — 1000 деревьев, побитовая идентичность ≥ 80%.
- `cargo test -p canvas-ui -- --ignored perf_baseline` — reflow 1000 узлов < 1 мс; 60 fps на 5000 нод.
- `cargo build --no-default-features` — 0 внешних UI-runtime-deps (кроме cosmic-text за trait).
- `cargo build --release --target wasm32-unknown-unknown` — ≤8 МБ raw / ≤4 МБ brotli (W4 — без taffy, ~5–15 КБ FlexLayoutEngine).
- `scripts/wasm_gate.sh --check` (default) — зелёный.
- `cargo deny check` — taffy (W1+, если не вырезан) + cosmic-text + транзитивные deps в allowlist.

## Проверка (Verification)

Чек-лист приёмки (каждая волна — все зелёные):

### W0

- [x] `cargo test -p canvas-ui --test g4_lint` — 30 прогонов, 0 нарушений (выход за parent/viewport/silent-clips) — 2026-09-25, 6 тестов (5 сцен + счётчик 30), 0 нарушений; SqueezeTail-деградация на 800×560 подтверждена (хвостовой чип вырождается и исключается как невидимый — F-11c, без молчаливого среза).
- [x] `cargo test -p canvas-ui --test snapshot` — 60 эталонов зелёные (или осознанный diff) — 2026-09-25, 61 тест (60 сравнений по матрице 10×3×2 + счётчик), эталоны в `tests/snapshot/*.txt`, регенерация `CANVAS_UI_UPDATE_SNAPSHOTS=1`.
- [x] `cargo test -p canvas-ui -- --ignored perf_baseline` — baseline зафиксирован — 2026-09-25, медиана reflow 1000 узлов 45.1 μs (бюджет §8 < 1 мс — запас ×22; debug-профиль, black_box, медиана 200 итераций); регресс-гейт > 20% активен.
- [x] `cargo test --workspace` — все тесты зелёные — 2026-09-25, 1941 passed / 0 failed.
- [x] `cargo clippy -D warnings`; `cargo fmt --check` — 2026-09-25, чисто (canvas-ui + потребители canvas-app/canvas-render).
- [x] `scripts/wasm_gate.sh --check` — зелёный (0 прироста) — 2026-09-25.

### W1

- [x] `cargo test -p canvas-ui` (default) — G4-линты + golden FR-062 F-18 на `NativeBackend` — 2026-09-25: 161 lib + 61 snapshot + 6 g4_lint, не изменены относительно W0 (делегирование в `default_backend()` байт-в-байт).
- [x] `cargo test -p canvas-ui --features taffy` — G4-линты + golden на `TaffyBackend` — 2026-09-25: 190 lib (в т.ч. 15 taffy-smoke + 10 kit-taffy) + 61 snapshot + 12 g4_lint (6 native + 6 taffy: 30 taffy-прогонов, 0 нарушений, без skip'ов) + 13 parity (backend_parity.rs: Fit/grow-целые/SpaceBetween/End/Wrap/Column/grid/measured побитово; пины дивергенций C3/rounding/End+overflow) + 11 html5_demos.
- [x] `cargo test -p canvas-ui --test html5_demos --features taffy` — 10/10 golden зелёные — 2026-09-25: 10 сцен (sticky-header/sidebar-overflow/navbar-space-between/grid-12-col/masonry-lite/aspect-ratio/modal-fixed-clip/dropdown-flip/virtualization/complex-form), дампы DFS pre-order, регенерация `CANVAS_UI_UPDATE_HTML5=1`; golden 01 пересоздан осознанно (фикс sticky-поддерева).
- [x] `cargo build --no-default-features` — `TaffyBackend` не компилируется, zero-dep — 2026-09-25: сборка зелёная; `strings` default-бандла — 0 taffy-символов.
- [x] `cargo build --release --target wasm32-unknown-unknown --features canvas-ui/taffy` (через canvas-web passthrough `taffy = ["canvas-app/taffy"]`, канонический пайплайн `web_bundle.sh`: trunk + wasm-bindgen 0.2.127 + wasm-opt -Oz) — 2026-09-25: default 8.72 МБ raw / 3.83 МБ gzip → taffy 9.00 МБ raw / 3.93 МБ gzip; **дельта +283.9 КБ raw / +98.3 КБ gzip** (оценка ADR-0014 +376 КБ raw — фактическая ниже за счёт wasm-opt -Oz); gzip ≤4 МБ — OK; raw ≤8 МБ — превышен ДО W1 на default (8.72; рост после W12 6.74 — вне рамок W1, решение за владельцем).
- [x] `scripts/wasm_gate.sh --check` (default) + `cargo deny check licenses` — зелёные — 2026-09-25: deny OK в default и с taffy (taffy MIT + arrayvec/smallvec MIT|Apache-2.0 + slotmap Zlib — уже в allowlist, deny.toml не менялся); taffy перенесён в DEPENDENCIES.md §2.
- [x] `cargo clippy -D warnings` (default + `--features taffy`); `cargo fmt --check` — 2026-09-25, чисто (workspace).
- [x] Perf: reflow 1000 узлов < 1 мс (на TaffyBackend) — 2026-09-25: release-медиана **264.8 μs** (запас ×3.8, гейт §Контракт-8); dev-профиль ~2.7 мс — артефакт неоптимизированного taffy (относительный гейт дрейфа ±20% активен; baseline `tests/perf_taffy_baseline.txt`).

### W2

- [ ] `cargo test -p canvas-ui` (default, `flex-engine` on, `taffy` off) — G4-линты + golden + 15 HTML5 demos на `FlexLayoutEngine`.
- [ ] `cargo test -p canvas-ui --features taffy` — те же тесты на `TaffyBackend` (совместимые политики побитово).
- [ ] `cargo test -p canvas-ui --features mock-shaper` — тесты с `MockShaper`.
- [ ] `cargo test -p canvas-ui --test flex_vs_taffy_parity` — 1000 деревьев, побитовая идентичность ≥ 80%.
- [ ] `cargo build --no-default-features` — `TaffyBackend` не компилируется; `FlexLayoutEngine` встроенный (0 deps).
- [ ] `cargo build --release --target wasm32-unknown-unknown` — прирост ~5–15 КБ raw (FlexLayoutEngine); без taffy.
- [ ] `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.
- [ ] Perf: reflow 1000 узлов < 1 мс (на `FlexLayoutEngine`).

### W3

- [ ] `cargo test --workspace` (default + `--features taffy`) — все тесты зелёные на компонентном API.
- [ ] `cargo test -p canvas-ui --test snapshot` — 60 эталонов зелёные (или осознанный diff — W3 разрешён).
- [ ] `cargo test -p canvas-ui --test g4_lint` — 30 прогонов × 2 backend'а, 0 нарушений.
- [ ] `cargo test -p canvas-ui --test html5_demos` — 15/15 golden зелёные.
- [ ] `grep -r "Child::fixed" crates/ | wc -l` — снижение на ~30–50%.
- [ ] kit.rs ≤ 500 строк; 6 компонентов × ~500 строк каждый.
- [ ] Perf: reflow 1000 узлов < 1 мс; 60 fps на 5000 нод — сохраняется.
- [ ] `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.

### W4

- [ ] `cargo build --no-default-features` — 0 внешних UI-runtime-deps (кроме cosmic-text за trait).
- [ ] `cargo tree -p canvas-ui --no-default-features` — проверка dep-дерева (только cosmic-text).
- [ ] `cargo test --workspace` — все тесты зелёные (taffy вырезан или Grid-only за фичей).
- [ ] `cargo test -p canvas-ui --test html5_demos` — 15/15 golden зелёные (на `FlexLayoutEngine`).
- [ ] `cargo test -p canvas-ui --test snapshot` — 60 эталонов зелёные.
- [ ] `cargo test -p canvas-ui --test g4_lint` — 30 прогонов, 0 нарушений.
- [ ] `cargo build --release --target wasm32-unknown-unknown` — ≤8 МБ raw / ≤4 МБ brotli; без taffy.
- [ ] `cargo clippy -D warnings`; `cargo fmt --check`; `cargo deny check`.
- [ ] Perf: reflow 1000 узлов < 1 мс; 60 fps на 5000 нод — сохраняется.

## Точки входа (Entry Points)

- `docs/SPEC.md` §6.3 — бюджеты UI (комментарий о backend'ах вёрстки).
- `docs/adr/adr-0015-long-term-ui-stack-strategy.md` — стратегия (критический разбор ADR-0014 + план W0–W4).
- `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` — переходное решение (taffy opt-in W1, вырезается W4).
- `docs/adr/adr-0013-taffy-vs-layout-primitives.md` — заменённое решение (замеры taffy воспроизводимы, триггеры T1–T4, основа `NativeBackend`/`FlexLayoutEngine`).
- `docs/prd/prd-0009-ui-layering-uikit.md` §7.2 V-3 / §7.4 V-5 / §9.5 / AC-3.1 (G4-линты 3 окна × 2 языка).
- `docs/ui-kit.md` — карта переносов, эталоны вёрстки (15 HTML5 demos), snapshot-тесты, паритет backend'ов.
- `docs/DEPENDENCIES.md` §3→§2 (taffy после W1, cosmic-text — за trait W2) → §3 отвергнутые (taffy после W4, если вырезан).
- `docs/plans/wasm-port.md` §8.8 — ≤8 МБ raw / ≤4 МБ brotli.
- FR-051 (UiLayer — ортогонален), FR-053 (layout-примитивы — основа `NativeBackend`), FR-056 (scissor — переиспользуется W1 `ClipRect`), FR-057 (Painter — W1 расширение), FR-062 (собственные layout v2 — основа `FlexLayoutEngine`), FR-067 (план staged миграции на taffy — поглощается W1), FR-061 этап E (kit-Row — pilot W1).

## История изменений (Changelog)

- `2026-09-25` — агент (лид + 4 параллельных агента-исполнителя, git-worktree, последовательный merge): **W1 выполнено — taffy opt-in** (поглощает FR-067 = ADR-0014 P1+P2). (1) Фундамент: `trait LayoutBackend` (row/column/measured/grid + `features()`; объектно-безопасный, immediate) + `NativeBackend` (перенос Row/Column/grid_cells 1:1 — 24 юнит-теста пинят байт-в-байт через делегирование) + `TaffyBackend` за фичей `taffy` (адаптер Fit/grow/SpaceBetween/End/Wrap/Column/grid-Points-tracks; measured через `resolve` ДО адаптера — бит-в-бит; ZST immediate — `TaffyTree` пересоздаётся на вызов, C2; retained-поле — W3) + `SceneNode` (нейтральная расширенная сцена: Percent 0..1/Fill/Auto, aspect-ratio, position Absolute|Fixed|Sticky, overflow, scroll-offset; `lay_out_scene` — DFS pre-order + post-processing content-shift/sticky-кламп с трансляцией поддерева; Fixed — re-parent в корень). (2) `PaintItem::ClipRect` + `Painter::clip_rect` + `walk` (клип-семантика draw-слоя; 60 snapshot-эталонов не тронуты; consumer — scissor FR-056, сейчас прозрачный проход). (3) Pilot (ADR-0014 P1): kit-витрина gallery — 4 call-site через `pilot_backend()` (measured×2/flex/grow/grid), canvas-app 420 тестов без правки значений + parity-тест `--features taffy` (сетка побитово, grow-ряды ≤ 1 ui px — rounding); explain-центрирование — parity `TaffyBackend::centered` ≡ `kit::modal` (полный перевод — W3); fr061_row_grid — `RowGuides::cells_with` (4 неравных трека [leader/value/unit/badge] через `grid_cells_with`, T2; native ≡ замороженной формуле `with_right_edge`, taffy ≡ native). (4) Kit taffy-пути (ADR-0014 P2): `scroll_area_taffy`/`dropdown_menu_taffy`/`tooltip_taffy`/`toast_area_taffy`/`modal_taffy` — opt-in варианты с parity побитово на целых входах (finaльные гарантии `viewport_clamp` W0 сохранены); native-функции не тронуты. (5) 10 HTML5 demos + goldens (mdn/css-tricks топ-10). (6) G4-линт × backend'ов (§Контракт-3): 30 taffy-прогонов — 0 нарушений. (7) Parity-сюита 13 тестов + perf-taffy (release 264.8 μs < 1 мс). (8) DEPENDENCIES.md taffy §3→§2; deny зелёный (MIT/Zlib/MIT|Apache — deny.toml не менялся). Документированные расхождения (пины-тесты): C3 `SqueezeTail` ≠ `flex_shrink`; taffy `compute_layout` округляет ВСЕ координаты/размеры к целому ui px (round on freeze) — побитовый паритет только на целых входах, дробные ≤ 0.5–1 ui px; `End`+переполнение — CSS unsafe alignment; sticky — эмуляция post-processing'ом. Фикс фазы интеграции: sticky-кламп транслирует поддерево (CSS-семантика; найден demo 01, golden пересоздан осознанно). wasm-замер (§Контракт-6/7): default 8.72 МБ raw / 3.83 gzip → taffy 9.00 / 3.93 (+283.9 КБ raw post-opt, ниже оценки ADR +376 — wasm-opt -Oz); gzip ≤4 МБ OK; **абсолютный raw-лимит 8 МБ превышен ДО W1** (default 8.72; рост 6.74→8.72 после W12, вне рамок W1) — решение владельца по §Контракту-7. Гейты: workspace 1963 passed / 0 failed; canvas-ui default 161+61+6, taffy 190+61+12+13+11; canvas-app 420 (default) / 422 (taffy); clippy -D warnings (обе конфигурации, workspace); fmt; wasm_gate --check; deny licenses (обе).
- `2026-09-25` — агент (4 параллельных агента, изолированные git-worktree + последовательный merge): **W0 выполнено**. (1) kit.rs hygiene — `viewport_clamp` (пересечение при наличии, иначе исходный rect); применён как финальная гарантия в `dropdown_menu`/`toast_area`; в `tooltip`/`modal` оставлены position-clamp/constrain-семантики с W0-комментариями (size-preserving контракты; parity-тесты canvas-app `dialog_rect_kit_modal_matches_old_clamps` пинят min-инвариант панели — пересечение клипповало бы панель 320×240→100×100); +5 тестов. (2) snapshot-тесты `tests/snapshot.rs` — 60 эталонов (10 компонентов × Normal/Hovered/Disabled × RU/EN), дамп `Painter.items()` с округлением до целого ui px и сортировкой по `(x,y,w,h,type)`, эталоны `tests/snapshot/*.txt`, регенерация `CANVAS_UI_UPDATE_SNAPSHOTS=1`. (3) G4-линт `tests/g4_lint.rs` — 5 canonical сцен × 3 окна × 2 языка = 30 прогонов: parent-пересечение всех видимых элементов, viewport-пересечение L4+, 0 пересечений интерактивных rect'ов одной полосы, hit-rect'ы во вьюпорте — 0 нарушений; silent-clips grep-аудит kit.rs — чисто. (4) perf baseline `tests/perf_baseline.rs` (`#[ignore]`) — reflow 1000 узлов (Fit/Wrap/SqueezeTail/grid_cells) медиана 45.1 μs, гейт < 1 мс, регрессия > 20% — fail, baseline `tests/perf_baseline.txt`, регенерация `CANVAS_UI_UPDATE_PERF=1`. (5) G4-нарушений не найдено (0–10 ожидание — фактический 0). Гейты: canvas-ui 155 lib + 61 snapshot + 6 g4_lint (+1 ignored perf), workspace 1941 passed, clippy/fmt чисто, wasm-gate --check зелёный. Статусы: ADR-0015 → принят владельцем (приказ «начинай первый этап внедрения»); ADR-0014 — переходное решение (см. ADR-0015).
- `2026-09-24` — агент: создан документ (план, статус `выявлено`). Зафиксированы 5 волн (W0 hygiene, W1 taffy opt-in, W2 cosmic-text trait + FlexLayoutEngine, W3 своя UI-библиотека, W4 dep-минимизация), контракты на стыках (§Контракты 1–9), ограничения для агента-реализатора (поэтапность, гибрид не замена, `SqueezeTail` решает `FlexLayoutEngine`, cosmic-text trait, zero-dep инвариант, не трогать UiLayer/Painter/geometry). Привязан к ADR-0015 (стратегия); ADR-0014 — переходное решение (статус дополняется «переходное, см. ADR-0015»). FR-067 (план staged миграции на taffy) поглощается W1. Решения за владельцем — до гейта W0.

## Источники истины (References)

- `crates/canvas-ui/src/layout.rs:1–1114` — `Row/Column/Child/MeasuredItem/RowPolicy/grid_cells/stack/constrain/pad/Custom` (FR-053 F-7 + FR-062 F-13…F-18) — основа `NativeBackend`; сигнатуры НЕ меняются до W3 (контракт §1).
- `crates/canvas-ui/src/kit.rs:389,423,478,458,741` — `dropdown_menu/tooltip/modal/toast_area` viewport-clamp (W0 — общий хелпер `viewport_clamp`; W1 — `position: absolute/fixed` через taffy адаптер) + `ScrollState/list_rows` (W1 — `overflow:auto`).
- `crates/canvas-ui/src/paint.rs:31,52` — `PaintItem/Painter` (W1 — расширение `PaintItem::ClipRect`).
- `crates/canvas-ui/src/measure.rs` — `TextMeasurer` (FR-053 F-6, cosmic-text шейпинг + кэш) — W2: мигрирует на `dyn Shaper`.
- `crates/canvas-ui/src/layer.rs:11` — `UiLayer` 9 полос (FR-051, ОРТОГОНАЛЕН движку вёрстки).
- `crates/canvas-ui/src/geometry.rs` — `UiRect` (НЕ меняется; `intersection` переиспользуется в G4-линтах W0+).
- `crates/canvas-ui/Cargo.toml` — `[features] default = []; taffy = ["dep:taffy"]` (W1); `flex-engine = []` (W2 — встроенный); `mock-shaper = []` (W2 — тестовый).
- `Cargo.toml` (workspace) — `[workspace.dependencies] taffy = { version = "0.14", default-features = false, features = ["std", "taffy_tree", "flexbox", "grid", "block"] }` (W1, вырезается W4).
- `crates/canvas-app/src/{kit_ui,explain_ui,whatif_ui,palette,settings_ui,docs_ui,template_ui,scheme_gallery_ui,debug_overlay,app}.rs` — ~497 мест `Child::fixed`+`lay_out` (W3 — миграция на `Component` trait).
- `crates/canvas-render/src/{search_ui,row_grid}.rs` — ~30 мест (W3).
- `docs/adr/adr-0015-long-term-ui-stack-strategy.md` — стратегия (критический разбор ADR-0014 + план W0–W4).
- `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` — переходное решение (taffy opt-in W1, вырезается W4).
- `docs/adr/adr-0013-taffy-vs-layout-primitives.md` — заменённое решение (статус `заменено (ADR-0014)`; замеры taffy воспроизводимы; основа `NativeBackend`/`FlexLayoutEngine`).
- `docs/prd/prd-0009-ui-layering-uikit.md` §7.2 V-3 / §7.4 V-5 / §9.5 / AC-3.1 (G4-линты 3 окна × 2 языка).
- `docs/DEPENDENCIES.md` §3 (taffy — кандидат) → §2 (после W1 — прямая прод-зависимость) → §3 отвергнутые (после W4, если вырезан).
- `docs/SPEC.md` §6.3 (бюджеты UI — комментарий о backend'ах).
- `docs/plans/wasm-port.md` §8.8 — ≤8 МБ raw / ≤4 МБ brotli.
- `docs/ui-kit.md` — карта переносов, эталоны вёрстки (15 HTML5 demos), snapshot-тесты, паритет backend'ов.
- `deny.toml` `[licenses] allow = ["MIT", "Apache-2.0", ...]` — taffy + транзитивные (slotmap/smallvec/cssparser/serde) в allowlist; `[advisories].ignore` — RUSTSEC-2024-0436 (paste), RUSTSEC-2026-0206 (rustybuzz), RUSTSEC-2026-0192 (ttf-parser) — мигрируют с cosmic-text.
- `scripts/wasm_gate.sh` / `scripts/mcp_wasm_gate.sh` — гейты wasm-совместимости (default + `--features taffy` для canvas-web W1+).
- Taffy 0.14.0 — `https://crates.io/crates/taffy` (MIT; продакшн-потребители: Servo, Bevy, Zed, Slint — ADR-0013 §Контекст).
- Cosmic-text 0.12 — `https://crates.io/crates/cosmic-text` (MIT; шейпинг + редактирование; тянет rustybuzz/ttf-parser — RUSTSEC unmaintained).
