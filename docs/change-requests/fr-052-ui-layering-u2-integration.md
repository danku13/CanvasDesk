# FR-052: Слои, вёрстка и UI kit — этап U2 PRD-0009: интеграция каркаса canvas-ui (единый диспетчер: реестр → UiFrame → HitStack/KeyboardRouter/draw-полосы)

- **Статус:** выполнено (U2)
- **Тип:** FR
- **Приоритет:** критично (инфраструктура всех UI-фич; устраняет класс z-регрессий CR-006/CR-010/CR-015 и рассинхронизацию ввода)
- **Владелец:** агент (по приказу владельца «Продолжи u2», сессия 2026-09-22)
- **Источник:** PRD-0009 §13 этап U2 (`docs/prd/prd-0009-ui-layering-uikit.md`); FR-051 (каркас canvas-ui, U0–U1 выполнены, merge 8970ca7, CI 12/12)
- **Связанные задачи:** PRD-0009 (V-4+V-5+V-6+V-7), FR-051 (U0–U1), PRD-0006 (токены — U3), PRD-0002/FR-042 (main stage — модальный потребитель), FR-017/FR-018/FR-021/FR-025/FR-026/FR-027/FR-028/FR-039/FR-049 (поверхности-потребители), ADR-0008 (зависимости), ADR-0011 (wasm-гейт)
- **Создан:** 2026-09-22
- **Обновлён:** 2026-09-22

## Описание (What)

Реализовать этап **U2** дорожной карты PRD-0009: каркас `canvas-ui` (реестр поверхностей, слои, capture-политики, HitStack, KeyboardRouter — FR-051) подключается как **единственный диспетчер** экрана. Приложение за кадр собирает `UiFrame` из реестра (адаптеры всех существующих поверхностей), рендерер исполняет **полосы слоёв** (draw-порядок выводится из реестра, а не из порядка вызовов), ввод (`on_left_button`, `on_key`, wheel-предикат, hover-глушение) читает `HitStack`/`KeyboardRouter`. Существующие z-тесты и клавиатурное поведение сохраняются (G3). Выявлено владельцем приказом «Продолжи u2» на основании PRD-0009 §13.

## Влияние (Impact)

| Объект/подсистема | Что меняется | Где в документации |
|---|---|---|
| `canvas-render` | Новый тип `ScreenBand` (полоса слоя: квады + тексты); `FrameOverlay.screen_instances/screen_texts` → `screen_bands: &[ScreenBand]`; рендерер исполняет полосы по порядку `UiLayer::DRAW_ORDER`; текст-система: одна текст-группа на полосу | `docs/SPEC.md` §6 (бюджеты — число draw-calls не меняется: те же проходы, полосы — порядок внутри screen-хвоста) |
| `canvas-render` (зависимости) | + внутренний workspace-крейт `canvas-ui` (0 внешних — G7) | `docs/adr/adr-0008` (allowlist не расширяется) |
| `canvas-app` | Новый модуль `ui_registry.rs`: декларации всех поверхностей в реестре + адаптеры hit-rect'ов + вывод владельца клавиатуры; RedrawRequested собирает экран в полосы; head `on_left_button`/`on_key` — через HitStack/Router; wheel-предикат и hover — из кадра | `docs/interface-objects/surface-registry.md` (создаётся — контракт поверхности, PRD §14) |
| `canvas-ui` | **0 правок** (каркас U1 стабилен; API достаточен) | `docs/prd/prd-0009-*.md` §9.1 |
| Формат `.canvas`, MCP | Не затрагивается | `docs/SPEC.md` §5.1, `docs/MCP.md` |
| Поведение | В редких состояниях совместной видимости поверхностей порядок нормализуется по слоям PRD (попапы над панелями, модали над всем) — список зафиксирован в Changes §«Нормализованные дельты» | `docs/ACCEPTANCE.md` (US-1) |

## Анализ (Root Cause) — что отсутствует

Каркас есть (FR-051), интеграции нет — диспетчеризация по-прежнему ручная:

1. **Draw-порядок экрана захардкожен последовательностью вызовов**: `canvas-app/src/app.rs` RedrawRequested (~12046–12244) — 15+ последовательных `extend` в один плоский `screen_instances`; два места правды (app.rs и порядок проходов `canvas-render/src/renderer.rs:1386–1442`). История: stage-регрессия «каши» (FR-042), тултип поверх затемнения.
2. **Приоритет клика — if-цепочка** `on_left_button` (app.rs:8999–10013, ~20 поверхностей): добавление поверхности = правки в начале цепочки; порядок веток не совпадает с визуальным порядком (дефект класса «клик уходит не туда»).
3. **Клавиатурная лестница** `on_key` (app.rs:7893–8228): head-приоритеты (онбординг → галерея → редактор → поиск → палитра → диалог) и Esc-лестница (8134–8220, 11 поверхностей) — два независимых ручных списка; stage-ветка (8222) противоречит собственному комментарию (Ctrl+F при stage открывает поиск вместо закрытия stage).
4. **wheel-предикат** `cursor_over_screen_surface` (app.rs:3721–3862) — третий ручной список тех же поверхностей (колесо/пинч глушатся над UI).
5. **Hover-глушение** (app.rs:10648–10652) — четвёртый список (`dialog || search || menu`).
6. Реестр/HitStack/Router из FR-051 существуют, но не вызываются ни одной точкой приложения (U1 — «0 правок app/render/web» по плану).

## Требуемые изменения (Changes)

### 1. `canvas-render`: полосы слоёв (F-2)

| Что → Где → Как |
|---|
| `ScreenBand` → `canvas-render/src/renderer.rs` → `{ layer: UiLayer, instances: &[CardInstance], texts: &[ScreenText] }`; `FrameOverlay.screen_instances/screen_texts` → `screen_bands: &[ScreenBand]` (плоские поля удаляются) |
| Зависимость → `canvas-render/Cargo.toml` → `canvas-ui.workspace = true` (внутренний крейт, 0 внешних — G7) |
| Исполнение полос → `Renderer::render` → квады полос конвертируются в буфер по порядку полос (диапазон каждой полосы запоминается), отрисовка screen-хвоста: **для каждой полосы: квад-диапазон → текст-группа полосы**; миникарта/stage — после всех полос (позиции не меняются) |
| Текст-группы → `canvas-render/src/text.rs` → `TitleFrame.screen_texts` → `screen_bands`; группы полос = `zplan.group_count() + i`, stage-группа = `group_count + band_count` (`TextSystem::band_group/stage_group`); пул рендереров растёт динамически (механизм есть) |
| Smoke-тесты → `canvas-render/tests/*` (7 файлов) → конструкция `FrameOverlay` переводится на `screen_bands: &[]`; новый тест `screen_bands_order_smoke`: тексты полосы N не рисуются над квадами полосы N+1 |

### 2. `canvas-app/src/ui_registry.rs` (новый модуль): реестр и адаптеры

| Что → Где → Как |
|---|
| Идентификаторы → `pub mod id` → константы `SurfaceId` 20 поверхностей: world, wheel, whatif, hotkeys, corner_buttons, minimap, settings, menu (контекстное), template_panel, template_strip, palette, docs, help_menu, stage, search, editor, dialog, gallery, onboarding, empty |
| Реестр → `build_registry(&App) -> SurfaceRegistry` → только активные поверхности; слой/capture/scope — по таблице FR-051 §U0; **порядок регистрации = обратный Esc-лестнице** (esc_stack воспроизводит текущую лестницу 8134–8220 дословно: stage → help_menu → docs → palette → strip → panel → menu → settings → hotkeys → whatif → wheel; head-поверхности — над stage) |
| Кадр → `build_frame(&App) -> UiFrame` → `UiFrame::from_registry` + адаптеры: hit-rect'ы каждой поверхности из **тех же чистых layout-функций**, что используют ввод и отрисовка (search_layout, modal_layout, dialog_button_rects, wheel_geometry, whatif_bar_layout, dock_strip_layout, template_panel_layout, onboarding_ui::card_rect, scheme_gallery_ui, minimap_rect…); деградация whatif HideBelow 900×600 |
| Владелец клавиатуры → `key_owner(&App) -> KeyOwner` → верх `esc_stack()`: перечисление `Onboarding/Gallery/Editor/Search/TemplatePanel/Dialog/Stage/Canvas` (Q4: NUMI-хоткеи и лестница команд не трогаются до U5) |
| Esc-диспетчер → `dispatch_esc` → per-surface обработчики с семантикой «поглотил/пропустил» (2-фазные: help_menu подменю, settings dropdown) |
| Тесты (TDD, headless) → `mod tests` → реестр состояний приложения (мутуально исключённые модали, whatif+dialog, hide-below 800×560), pick-матрица на реальных адаптерах (галерея backdrop закрывает, онбординг backdrop глотает, пустое место → канвас), esc_stack == старая лестница, draw-полосы по слоям, toast Passive не перехватывает, editor Capture вне сессии пропускает |

### 3. `canvas-app`: сборка кадра в полосы

| Что → Где → Как |
|---|
| RedrawRequested → `app.rs` → `let ui_frame = ui_registry::build_frame(self)` (до mut-заёмов); screen-контент собирается в `ScreenBands` (push по слою, внутри полосы — сегодняшний порядок); `FrameOverlay.screen_bands` — полосы в порядке `UiLayer::DRAW_ORDER` из `ui_frame.draw_bands()` |
| Порядок внутри полосы = сегодняшний draw-порядок (push-последовательность сохранена дословно) → 0 визуальных изменений в канонических состояниях |

### 4. `canvas-app`: ввод через HitStack/Router

| Что → Где → Как |
|---|
| `on_left_button` (Pressed) → head-диспетчер → `HitStack::pick(&frame, cursor)`: `Element{surface}` → `click_<surface>` (тела веток переносятся в методы дословно); `Backdrop{surface}` → контракт поверхности (gallery/search/settings/docs/help_menu/stage/menu — закрыть; onboarding/dialog — глотнуть); `None` → dismiss-transients (commit редактора, снятие фокуса палитры, закрытие flyout) → существующая canvas-цепочка без изменений |
| `on_key` → head через `key_owner` (Onboarding/Gallery/Editor/Search/TemplatePanel/Dialog — существующие обработчики; Stage — любой ключ закрывает: фикс противоречия 8222) → Esc-лестница через `esc_stack` + `dispatch_esc` (воспроизводит 8134–8220); остальная лестница (Ctrl+F/P/T/…, F1, буфер, Space) — без изменений (Q4) |
| wheel/pinch → `cursor_over_screen_surface` → `HitStack::absorbs(&frame, cursor)` (один диспетчер вместо третьего списка) |
| hover → `on_cursor_moved` → глушение при `HitStack::pick(cursor).is_some()` (вместо списка `dialog||search||menu`) |

### Нормализованные дельты (принятые отклонения от текущего поведения — только в состояниях совместной видимости, каждое — нормализация по слоям PRD)

1. Попапы (L4: меню, help, docs, hints) рисуются **над** панелями (L3: whatif бар) — раньше бар был над меню при пересечении.
2. Онбординг (L5) рисуется над палитрой выделения (L2) — модаль над тулбаром.
3. Миникарта выигрывает клик у whatif бара на пересечении — раньше клик уходил бару при визуальном верхе миникарты (теперь ввод = визуальному верху).
4. Stage закрывается любым ключом **до** открытия панелей (Ctrl+F при stage закрывает stage, поиск — следующим нажатием; соответствует документированному намерению 8222).
5. Тексты панелей нижней полосы больше не рисуются поверх квадов модалей верхней полосы (побочный фикс полосного исполнения текстов).

## Точки входа (Entry Points)

- `docs/change-requests/index-cr-fr.md` — строка FR-052, указатель следующего FR-053.
- `docs/prd/prd-0009-ui-layering-uikit.md` — §16 история (U2 выполнен), статус.
- `docs/prd/README.md` — статус PRD-0009.
- `docs/interface-objects/surface-registry.md` — новый контракт поверхности (реестр, capture-политики, «3 шага до поверхности»).
- `docs/SPEC.md` §3/§6 — состав крейтов (canvas-ui в графе зависимостей render/app), бюджеты (без изменений цифр).
- `worklog.md` — запись о реализации.

## Проверка (Verification)

- [x] Pick-матрица на реальных адаптерах: Block-поверхности глотают backdrop, Capture пропускает мимо rect'ов, PassThrough/Passive не перехватывают (G2, автотесты `ui_registry`: gallery_block_backdrop_swallows, onboarding_backdrop_swallows_without_close, idle_registry_has_world_and_chrome, whatif_hide_below_and_capture_rect, toast_is_passive).
- [x] Esc-лестница из реестра дословно воспроизводит прежнюю (автотест `esc_stack_matches_legacy_ladder`: stage → help → docs → palette → strip → panel → menu → settings → hotkeys → whatif → wheel; head-поверхности над stage — key_owner_follows_esc_top).
- [x] Draw-полосы: порядок полос = слои по возрастанию (автотест frame_bands_are_layer_ascending); порядок внутри полосы — дословно прежний (push-последовательность сохранена); тексты полосы N рисуются между квадами N-1/N+1 (полосное исполнение в renderer.rs:1437–1452).
- [x] G3: все существующие z-тесты (zorder.rs, zorder_smoke, stage-тесты) зелёные; `cargo test --workspace` — 48 тест-бинарей, 0 failed (245 в canvas-app, из них 11 новых).
- [x] Wheel/hover: `cursor_over_screen_surface` = `HitStack::absorbs` по кадру реестра; hover-глушение = pick по кадру (единый диспетчер вместо списков).
- [x] G7: 0 новых внешних зависимостей (canvas-ui — внутренний workspace-крейт в render/app); wasm-гейты зелёные.
- [x] Гейты на ветке 2e03eeb: `cargo fmt --all --check` ✓; `CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets -- -D warnings` ✓; `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 cargo test --workspace` ✓ (48/48); `bash scripts/wasm_gate.sh` ✓ (wasmtime 36.0.1); `bash scripts/mcp_wasm_gate.sh` ✓.
- [x] CI по merge SHA `f7008e4` — **зелёный (12/12 check-runs)**: gates ubuntu/macos/windows, artifacts ×3, build, wasm-check, web, licenses, deploy.

## История изменений (Changelog)

- `2026-09-22` — агент: **U2 выполнен** — слияние в main (03bd216, интеграция X2-конфликтов f7008e4), CI 12/12 по merge SHA `f7008e4`; X2-окно проверки цепочки (PRD-0007, параллельная волна) интегрировано через реестр U2 (поверхность explain: L5/Block/scope + KeyOwner::Explain + Backdrop→close_explain) — единый диспетчер выдержал параллельную миграцию без правок X2-кода.
- `2026-09-22` — агент: этап U2 реализован (коммиты ffff8d5 docs + 2e03eeb feat): ScreenBand/полосное исполнение в canvas-render, модуль `app/ui_registry` (20 поверхностей, build_registry/build_frame_at/key_owner/dispatch_esc/ScreenBands), head-диспетчеры HitStack/KeyOwner в on_left_button/on_key, wheel/hover из кадра; нормализованные дельты применены; 48 тест-бинарей зелёные, 5 гейтов зелёные. Статус — ожидание CI по merge SHA.
- `2026-09-22` — агент: создан документ (FR-052), статус `в работе`; постановка U2: ScreenBand/полосы в renderer+text, модуль ui_registry (реестр/адаптеры/KeyOwner), head-диспетчеры ввода через HitStack/Router, wheel/hover из кадра; зафиксированы нормализованные дельты.

## Источники истины (References)

- `docs/prd/prd-0009-ui-layering-uikit.md` — §7.3 (слои/политики/точки интеграции), §11 (Q2/Q3/Q4-дефолты), §13 (U2), §15 (G2/G3/G7).
- `docs/change-requests/fr-051-ui-layering-uikit.md` — таблица слоёв §U0, Приложение А.
- `crates/canvas-ui/src/{registry,hit,keyboard,frame,layer}.rs` — API каркаса (не меняется).
- `crates/canvas-app/src/app.rs` — RedrawRequested :12046+, on_left_button :8999+, on_key :7893+, cursor_over_screen_surface :3721+, hover :10648+, Esc-лестница :8134–8220.
- `crates/canvas-render/src/renderer.rs` — FrameOverlay :128–165, screen-хвост :1204–1231, проходы :1386–1442; `src/text.rs` — TitleFrame :1089–1157, группы :1934–1954, :2571–2607; `src/zorder.rs` — план мира (не меняется).
- egui `hit_test.rs`/`layers.rs` — семантика реверс-обхода (FR-051 Приложение А, L2).
