# Глубокое ревью UI-кита CanvasDesk и эталонных библиотек (2026-10-10)

> **Цель ревью:** провести глубокий аудит `crates/canvas-ui` и подсмотреть
> паттерны у трёх семейств эталонов — Rust UI-библиотек (egui/iced/slint/gpui/
> xilem/druid), дизайн-систем (Material 3 / Fluent 2 / Carbon / Apple HIG /
> Spectrum / SLDS) и canvas-продуктов (Miro / Figma / tldraw / Excalidraw /
> Whimsical / Eraser.io). Результат — приоритизированный перечень того, **чем
> дополнить кит**: новые компоненты, примитивы вёрстки, состояния/интеракция,
> токены, архитектурные паттерны. Каждый пункт снабжён Rust-сигнатурой
> предлагаемого расширения.
>
> **Метод:** (1) сплошное чтение `crates/canvas-ui/src/` (16.8k строк, 19
> компонентов, 2 layout-движка, 7 инфраструктурных модулей); (2) три
> параллельных research-агента — каждый изучил своё семейство по официальным
> докам и исходникам (findings в `rust-ui-library-audit.md`,
> `design-systems-research.md`, `canvas-ui-research-findings.md`); (3) синтез
> кросс-референсов: паттерн считается «зрелым», если встречается у ≥2 эталонов.
>
> **Скоуп:** кит `canvas-ui` (Immediate-mode, slot-based, cosmic-text, zero-dep
> кроме cosmic-text). Прикладные поверхности `canvas-app/src/*_ui.rs` вне
> ревью — только как источник «где кит не дотягивает». Бэклог LAY-W1..W12 НЕ
> учитывается (по директиве владельца — будет закрыт отдельно).
>
> **Связанные документы:** `docs/ui-kit.md`, `docs/prd/prd-0009-ui-layering-uikit.md`,
> `docs/adr/adr-0013-taffy-vs-layout-primitives.md`, `docs/adr/adr-0014-taffy-hybrid-layout-backend.md`,
> `docs/adr/adr-0015-long-term-ui-stack-strategy.md`, `design/rules/03-spacing-radius.md`,
> `design/rules/11-layouts.md`, три research-документа этой же волны.

---

## 0. Резюме: карта находок

| Направление | Находок | Критичных (P1) | Quick-win (только токены/состояния) |
|---|---|---|---|
| Состояния и interaction model | 5 | 2 (Focused, Dragged) | 3 |
| Темизация и токены | 6 | 2 (on-fill pairs, elevation) | 4 |
| Компоненты (новые) | 18 | 4 (checkbox, switch, slider, tabs) | 0 |
| Примитивы вёрстки | 5 | 1 (grid-auto / minmax) | 0 |
| Архитектурные паттерны | 6 | 2 (Response, focus-trap) | 1 |
| Canvas-UX паттерны | 8 | 3 (command-palette, action-registry, nav-rail) | 0 |

**Итог:** 48 находок, 14 P1, 8 quick-win. Кит структурно здоров (zero-dep,
slot-only, cosmic-text, headless-тестируем), но отстаёт от индустрии по:
(а) **состояниям** — 5 vs 7-8 у Material/Spectrum; (б) **компонентам** — 19 vs
~45 норма; (в) **токенной дисциплине** — нет elevation/motion/shape/density
шкал; (г) **interaction-эргономике** — нет `Response`-паттерна egui.

**Рекомендованный порядок внедрения (см. §6):**
1. **Wave T (tokens)** — состояния, on-fill pairs, shape/spacing/motion/density
   шкалы (8 quick-win, 0 новых компонентов);
2. **Wave C (components)** — checkbox/switch/slider/radio/tabs/accordion/
   progress/skeleton/badge/avatar (10 компонентов, P1 первые 4);
3. **Wave L (layout)** — grid-auto, aspect-ratio, sticky, container-query;
4. **Wave A (architecture)** — `Response`, focus-trap, action-registry,
   command-palette.

Каждая волна — отдельный FR/ADR; кит остаётся zero-dep и headless-тестируемым.

---

## 1. Инвентарь текущего кита

### 1.1. Структура крейта (16 757 строк)

```
crates/canvas-ui/src/
├── lib.rs              207  — реэкспорт, crate-level доки
├── kit.rs               92  — фасад `pub use` (тонкий, FR-068 W3)
├── component/               — 19 компонентов
│   ├── mod.rs          248  — Component trait, KitState, KitPalette, метрики
│   ├── button.rs       928  — button/chip/switch/icon_button
│   ├── text_field.rs  1651  — TextFieldModel, caret, mask, actions/effects
│   ├── dropdown.rs     847  — dropdown/tooltip/toast/anchored_stack/viewport_clamp
│   ├── table.rs       1447  — Table retained + table_layout_immediate
│   ├── row.rs         1048  — row_layout/paint_row/row_guides/zebra
│   ├── list.rs         585  — ScrollState/list_rows/scroll_bar
│   ├── panel.rs        467  — panel_rect/style/card/backdrop
│   ├── icon.rs         581  — IconKind, icon_composition
│   ├── radio_card.rs   318  — radio_card (для настроек)
│   ├── panel_header.rs 284  — PanelHeader + HeaderButton
│   ├── modal.rs        277  — modal/focus_order
│   ├── crumbs.rs        ~   — crumbs (хлебные крошки)
│   ├── chat_bubble.rs   ~   — chat_bubble (для агент-панели)
│   ├── banner.rs        ~   — banner (уведомления)
│   ├── footer.rs       203  — footer_buttons_measured
│   ├── two_column.rs   129  — two_column
│   └── test_support.rs  79  — тестовые фикстуры
├── layout.rs          1663  — Row/Column/Child/grid_cells/stack/constrain/pad
│   ├── flex.rs             — FlexLayoutEngine (zero-dep, FR-068 W2)
│   └── scene.rs            — SceneNode (percent/aspect/sticky/overflow)
├── paint.rs            739  — Painter (rect/panel/control/label/clip)
├── measure.rs          730  — TextMeasurer (cosmic-text, width_of/wrap/ellipsis)
├── frame.rs            335  — UiFrame, HitRect, SurfaceFrame
├── keyboard.rs         412  — KeyboardRouter, FocusRing
├── registry.rs         298  — SurfaceRegistry, SurfaceDecl, DegradationPolicy
├── row_guides.rs       609  — zebra_run_flags, направляющие
├── widget.rs           249  — WidgetState (машина состояний)
├── geometry.rs         210  — UiRect, UiPoint, UiVec2, EdgeInsets
├── hit.rs              247  — HitStack, HitTarget
├── shaper.rs           209  — Shaper trait ( cosmic-text bridge)
├── anim.rs             109  — animate_value, BoolAnim
├── capture.rs           59  — CapturePolicy (Block/Capture/PassThrough)
└── layer.rs            106  — UiLayer (9 слоёв)
```

### 1.2. Состояния (`KitState` — 5 значений)

```rust
pub enum KitState {
    Normal, Hovered, Pressed, Disabled, Selected,
}
```

**Приоритет:** `Disabled > Pressed > Hovered > Selected > Normal` (матрица в
`WidgetState::kit_state`). Фокус НЕ входит в `KitState` — рисуется отдельной
рамкой по слоту `accent` (`WidgetState::set_focused` — флаг).

### 1.3. Палитра (`KitPalette` — ~19 слотов)

```rust
pub struct KitPalette {
    // Контейнеры
    pub panel_fill: [f32;4], pub panel_border: [f32;4],
    pub control_fill: [f32;4], pub control_border: [f32;4],
    pub control_primary: [f32;4], pub control_danger: [f32;4],
    // Состояния
    pub hover_fill: [f32;4], pub primary_hover_fill: [f32;4],
    pub selected_fill: [f32;4],
    // Текст
    pub text: [f32;4], pub text_title: [f32;4],
    pub text_muted: [f32;4], pub disabled_text: [f32;4],
    // Акцент/статусы
    pub accent: [f32;4], pub control_success: [f32;4],
    pub control_warning: [f32;4],
    // Спец
    pub stage_dim: [f32;4], pub scrollbar_thumb: [f32;4],
    pub rule_color: [f32;4],
}
```

### 1.4. Метрики (bare `const f32`)

```rust
pub const BUTTON_HEIGHT: f32 = 30.0;
pub const BUTTON_PAD_H: f32 = 12.0;
pub const BUTTON_WIDTH: f32 = 100.0;
pub const ICON_BUTTON_SIZE: f32 = 26.0;
pub const CHIP_HEIGHT: f32 = 24.0;
pub const CHIP_PAD_H: f32 = 8.0;
pub const GAP_CONTROLS: f32 = 8.0;
pub const TEXT_FIELD_HEIGHT: f32 = 30.0;
pub const TEXT_FIELD_MIN_W: f32 = 80.0;
pub const LIST_ROW_H: f32 = 26.0;
pub const LIST_ROW_GAP: f32 = 6.0;
pub const SCROLLBAR_WIDTH: f32 = 4.0;
pub const SCROLLBAR_KNOB_MIN: f32 = 20.0;
pub const SWITCH_W: f32 = 36.0;
pub const SWITCH_H: f32 = 20.0;
pub const SWITCH_KNOB_PAD: f32 = 2.0;
pub const TOAST_TTL_MS: u64 = 3000;
pub const TOOLTIP_DELAY_MS: u32 = 500;
pub const TOOLTIP_OFFSET: UiPoint = UiPoint::new(14.0, 18.0);
pub const DROPDOWN_GAP: f32 = 4.0;
```

### 1.5. Layout-примитивы (`layout.rs`)

| Примитив | Web-эквивалент | Статус |
|---|---|---|
| `Row { gap, main, cross, policy }` | `flex-direction:row` | ✅ |
| `Column { gap, main, cross }` | `flex-direction:column` | ✅ |
| `Child::fixed/flexible/spacer` | flex-item | ✅ |
| `MainAlign::{Start,SpaceBetween,End}` | `justify-content` | ✅ |
| `CrossAlign::{Start,Center,End}` | `align-items` | ✅ |
| `RowPolicy::{Fit,SqueezeTail,Wrap}` | overflow policies | ✅ |
| `grid_cells(slot,cols,rows,row_h,gap)` | `grid-template-columns` (равные) | ✅ |
| `stack(slot,size,HAlign,VAlign)` | absolute-center | ✅ |
| `constrain(min,max,desired)` | `clamp()` | ✅ |
| `pad(slot,EdgeInsets)` | `padding` | ✅ |
| `MeasuredItem::Text/Fixed/Spacer` | max-content children | ✅ |
| `SceneNode` (percent/aspect/sticky) | CSS advanced | ✅ (FR-074) |

### 1.6. Что кит уже делает ХОРОШО (не трогать)

1. **Zero-dep инвариант** (G7) — только cosmic-text за `Shaper`. taffy вырезан
   (W4). Это подтверждается gpui — единственный Rust UI-фреймворк production-
   уровня, который тоже использует taffy + cosmic-text, но у нас taffy вырезан
   ради wasm-бюджета (ADR-0013).
2. **Immediate-mode + slot-only** — геометрия от слота родителя, ничего не
   хранится между кадрами (кроме `WidgetState` и `ScrollState` как
   осознанных retained-островков). egui — референс; мы — облегчённая версия.
3. **Hit = draw** (П5, LAY1.2) — `ComponentHit::pick` из тех же `rects`, что
   `paint`. Второго расчёта геометрии для хитов нет.
4. **`KitPalette` — слоты, не цвета** — кит не знает конкретных цветов, только
   слоты. Маппинг в `canvas-render::theme::ThemeColors`. Это паттерн druid
   `Env + Key<T>`, но без типобезопасности `Key<T>` (см. §3.1).
5. **`TextMeasurer` — реальный шейпинг** — cosmic-text, та же пара
   (семейство, вес), что у рендера. Класс CR-015 (эвристики ширины) закрыт по
   построению.
6. **`UiLayer` + `SurfaceRegistry`** — 9 слоёв, capture-политики, Esc-стек,
   `DegradationPolicy::HideBelow`. Это эквивалент Material's window-size-
   classes + Fluent's FocusZone на уровне реестра.
7. **`FocusRing`** — Tab-навигация по rect'ам поверхности, `retain_order`
   после пересборки раскладки. egui `request_focus`/`surrender_focus` —
   референс; наш ring — декларативнее.
8. **G4-lint в CI** — `ui_layout_lint.rs` прогоняет 23 канонических состояния
   × 3 вьюпорта × RU/EN. Carbon's per-component a11y table — цель; мы у цели
   на уровне поверхностей (LAY-W11 закрыл слепые зоны).

### 1.7. Что кит делает ПЛОХО (пробелы — цель ревью)

Сводная таблица пробелов (детали в §3-5):

| Пробел | Текущее | Эталон (best) | Gap |
|---|---|---|---|
| Состояний | 5 | 7-8 (M3, Spectrum) | нет Focused/Dragged/Error |
| Цветовых пар | ~19 слотов, без on-fill | 4-role pair (M3) | нет on-fill/on-fill-container |
| Elevation | нет | 5-6 shadow ramp (Fluent/M3) | нет shadow-токенов |
| Motion | 6 длительностей в `tokens.rs`, не в ките | 12+4 (M3) | не формализованы в ките |
| Shape | bare `radius: f32` | 7-step enum (M3) | нет Shape-enum |
| Spacing | 5 значений в `tokens.rs` | 13 шагов (Carbon) | не enum, разброс литералов |
| Density | нет | compact/comfortable (M3, Carbon) | нет |
| Size scale | bare `BUTTON_HEIGHT=30` | Sm/Md/Lg (Fluent/Carbon) | нет |
| Компонентов | 19 | ~45 (M3, Carbon) | нет checkbox/switch/slider/radio/tabs/accordion/progress/skeleton/badge/avatar/tree/datatable/segmented/command-palette/popover |
| `Response` | нет | egui `Response` | каждый вызов виджета = void |
| Focus-trap | частично (KeyboardRouter) | Fluent FocusTrap | нет примитива |
| Action-registry | нет | tldraw/Excalidraw | нет единого реестра действий |

---

## 2. Эталоны: Rust UI-библиотеки

> Полный аудит — `docs/dev-researches/rust-ui-library-audit.md` (1 567 строк).
> Здесь — выжимка релевантного.

### 2.1. Сводка по библиотекам

| Библиотека | Архитектура | Layout | Text | Чем полезен нам |
|---|---|---|---|---|
| **egui** 0.36 | Immediate (multi-pass) | custom cursor | custom (epaint) | `Response`-паттерн, `on_hover_text`, `labelled_by`, `UiBuilder` scope |
| **iced** 0.14 | Retained, Elm | custom flexbox | cosmic-text | `Responsive` (адаптив по available-size), `PaneGrid`, `Themer` |
| **slint** 1.17 | DSL, retained | built-in (HBox/VBox/Grid/Flex) | per-backend | states+transitions DSL, property bindings |
| **gpui** 0.2 | Hybrid (imm+retained) | **taffy** | **cosmic-text** | Tailwind-builder, `Anchored`, `FocusHandle` + `Action` |
| **xilem** 0.4 | Reactive view-diff | masonry BoxConstraints | Parley | `lens` + `memoize` + `task` (reactive) |
| **druid** 0.8 | Data-first, retained | BoxConstraints + Flex | xi-unicode | `Env` + `Key<T>`, `WidgetExt`, `Controller` |

### 2.2. Top-10 паттернов из Rust UI (детали в research-документе)

1. **egui `Response`** — каждый вызов виджета возвращает `Response { hovered,
   clicked, dragged, has_focus, changed, active }`. Нет callbacks. → §3.5.
2. **egui `on_hover_text` / `on_hover_ui`** — метод на `Response`, цепочки
   `ui.button("x").on_hover_text("Help")`. → §3.5.
3. **gpui Tailwind-builder** — `div().flex().gap_3().bg(color).child(...)`.
   Эргономика, но требует retained-слоя. → §5.2 (v2).
4. **druid `Env` + `Key<T>`** — типобезопасные слоты: `const ACCENT: Key<Color>
   = Key::new("accent"); theme.get(ACCENT)`. → §3.1.
5. **druid `WidgetExt`** — `Label::new("x").padding(10).on_click(...)`.
   Композбл модификаторы. → §3.5.
6. **iced `Responsive`** — виджет получает available-size для адаптива без
   media-queries. → §4.4.
7. **egui `labelled_by`** — accessibility-ассоциация label↔input в immediate
   mode через ID. → §3.6.
8. **gpui `Anchored`** — overflow-aware позиционирование для popover/tooltip.
   У нас есть `dropdown_menu` (flip), но не обобщено. → §4.3.
9. **slint states + transitions DSL** — декларативные стейт-машины с
   анимациями. → §5.3 (v2).
10. **egui `UiBuilder`** — scoped interaction state (`disabled`, `invisible`,
    opacity). → §3.5.

### 2.3. Валидация наших решений

- **taffy вырезан (ADR-0013)** — gpui использует taffy `=0.9.0`, но мы
  сознательно отказались ради wasm-бюджета (108.5 КБ = весь G7-остаток).
  Решение подтверждено: `FlexLayoutEngine` zero-dep покрывает потребности
  8-10 панелей (KISS).
- **cosmic-text** — gpui использует `=0.14.0`. Наш выбор подтверждён как
  production-grade.
- **Immediate-mode** — egui доказывает, что это масштабируется до production
  (Rerun Viewer). Наш гибрид (immediate + retained `WidgetState`) —
  осознанная середина.

---

## 3. Эталоны: дизайн-системы

> Полный аудит — `docs/dev-researches/design-systems-research.md` (811 строк).
> Здесь — выжимка релевантного.

### 3.1. Сводка по дизайн-системам

| Система | Компонентов | Состояний | Токенных слоёв | Elevation | Motion | A11y |
|---|---|---|---|---|---|---|
| Material 3 | ~45 | **7** (additive, state-layer) | 3 (ref/sys/comp) | 6 dp-levels | 12 dur + 6 easing | high |
| Fluent 2 | ~50 | 5-6 (token-suffix) | 3 | **5-6 shadow ramp** | dur + 6 curves | high (HC peer theme) |
| Carbon | ~45 | 5 (+sel/warn/err) | 3 | flat Layer | 6 dur + 6 easing | **highest** (per-comp a11y table) |
| Apple HIG | ~40 | ~5 | 2 | materials/vibrancy | spring/declarative | highest (VoiceOver, Dynamic Type) |
| Spectrum | ~55 | **8** (key-focus distinct) | 3 | drop-shadow xs..2xl | dur/easing tokens | high |
| SLDS | ~60 | 5-6 | 3 | shadow tokens | dur/easing tokens | high |

### 3.2. Ключевые контрасты

- **Состояния:** M3 = 7 (additive через state-layer opacity 8/10/10/16%);
  Spectrum = 8 (отдельный `key-focus` ≠ mouse-focus); Carbon/Apple = ~5. У
  нас 5 — нет `Focused`, `Dragged`, `Error`, `Loading`.
- **Цвет:** M3 = 4-role pair (role/on-role/role-container/on-role-container).
  У нас ~19 слотов без пар — контраст hand-tuned.
- **Elevation:** Fluent = 5-6 shadow ramp (2/4/8/16/28/64); M3 = 6 dp-levels
  + tonal elevation. У нас **нет** shadow-системы — только `stage_dim` scrim.
- **Touch target:** Apple = 44pt; M3 = 48dp; Carbon = 32 desktop / 44 mobile.
  У нас `ICON_BUTTON_SIZE=26`, `BUTTON_HEIGHT=30` — **ниже пола** (исправлено
  LAY-W8 hit-only расширением, но визуал остаётся 26/30).

### 3.3. Top-15 паттернов из дизайн-систем (детали в research-документе)

1. **M3 state-layer модель** — `Focused` + `Dragged` в `KitState` (→ 7
   состояний), hover/focus/pressed/dragged через opacity-overlay 8/10/10/16%
   поверх content/accent. → §3.4.
2. **4-role color pairs** (`fill / on-fill / fill-container / on-fill-container`)
   — для каждого контейнера парный foreground-слот. → §3.4.
3. **Per-component "styling hooks"** (SLDS `--slds-c-*`) — named per-component
   slice вместо плоского `ControlStyle`. → §3.4.
4. **`FocusZone` + `FocusTrap`** (Fluent) — arrow-key навигация (radio/menu/
   tab/list/tree) + modal focus-trap+return как примитивы, не per-surface
   код. → §4.5.
5. **`key-focus` ≠ mouse-focus** (Spectrum) — фокус-рамка только для
   keyboard-юзеров (`:focus-visible` семантика). → §3.4.
6. **Named `Size` scale: Sm/Md/Lg** (Fluent 24/32/40, Carbon 32/40/48) —
   заменить `BUTTON_HEIGHT=30` / `ICON_BUTTON_SIZE=26`. → §3.4.
7. **`Layer` depth + contextual tokens** (Carbon) — `Layer(0..3)` к surface,
   depth-aware контейнер/текст/бордер слоты. → §3.4.
8. **Elevation ramp** (Fluent `shadow2/4/8/16/28/64` или M3 L0-L5) — 5-6 step
   shadow ramp + tonal elevation. → §3.4.
9. **Motion token set** (M3 12 durations + 4-6 easings) —
   `Duration::{Short1..Long4}` (50..600ms) + `Easing::{Standard,Emphasized*}`.
   Honor `prefers-reduced-motion`. → §3.4.
10. **Shape scale enum** (M3 7 шагов) — `Shape::{None,Xs,S,M,L,Xl,Full}` → px.
    → §3.4.
11. **Spacing scale 13 шагов** (Carbon 2/4/8/12/16/24/32/40/48/64/80/96/160) —
    `Spacing` enum. → §3.4.
12. **4-tier emphasis ladder** (Apple primary/secondary/tertiary/quaternary) —
    `TextEmphasis` + `FillEmphasis`. → §3.4.
13. **`ButtonVariant` расширение** — `Tertiary` + `Text`/`Outlined` +
    `Inverse` (on-color) + `Danger`. → §3.4.
14. **Window-size-class** (M3 5 / Apple Compact·Regular) —
    `WindowClass::{Compact,Medium,Expanded}` (width <600/<840/≥840). → §4.4.
15. **Per-component anatomy + a11y table** (Carbon) — документированные named
    regions + state map + size table + keyboard/focus contract + touch-target.
    → §6 (deliverable формат).

---

## 4. Эталоны: canvas-продукты

> Полный аудит — `canvas-ui-research-findings.md` (412 строк). Здесь — выжимка.

### 4.1. Сводка по canvas-продуктам

| Продукт | Toolbar | Inspector | Context-menu | Cmd+K | Minimap | Layers | Open-source |
|---|---|---|---|---|---|---|---|
| Miro | Left dock | Top-center popover | flat nested | search + AI | yes (interactive) | no | no |
| Figma UI3 | Top (mode switcher) | Right sidebar | flat nested | quick actions | no | **yes (full tree)** | no |
| tldraw | **Bottom-center** | Right "Style panel" | radial-style | yes (MainMenu) | yes (toggle) | no | **yes (MIT)** |
| Excalidraw | Top-center | Left "Island" | flat nested | yes | no | no | **yes (MIT)** |
| Whimsical | Left dock | Right popover | flat | yes | yes (small) | no | no |
| Eraser.io | Left dock | Right docs/AI | flat + AI | yes + AI | yes | no | no |

### 4.2. Top-15 паттернов из canvas-продуктов

1. **Vertical "navigation bar" of mode tabs** (Figma UI3) — slim left-most
   rail of tabs (File/Layers/Search/Templates/Calc/Explain/Flow Map/Agent/
   Settings/Docs), swap adjacent left sidebar. → §5.1.
2. **Resizable, collapsible sidebars** with `⌘⇧\` "minimize UI" (Figma). → §5.1.
3. **Contextual properties popover anchored to selection** (Miro/tldraw/
   Excalidraw). → §5.1.
4. **Bottom-center docked tools toolbar** with overflow "More" + shortcuts in
   tooltips (tldraw, 438×48, grouped tools+actions). → §5.1.
5. **Radial/contextual right-click menu with action predicates** (tldraw +
   Excalidraw `shapeActionPredicates.ts`). → §5.4.
6. **Command palette as universal action surface** (`Cmd+K`) (tldraw +
   Excalidraw + Eraser). → §5.4.
7. **Template gallery = full-screen categorized grid** (Miro: ~280px sidebar
   + search + responsive grid of preview+name+tag). → §5.1.
8. **Library drag-to-canvas with fractional-index ordering** (Excalidraw). → §5.1.
9. **Diagram-as-code / doc↔canvas duality** (Eraser.io) — calc panel ↔ flow
   map ↔ explain bidirectionally bound. → §5.5.
10. **Selection-aware AI inline actions** (Eraser + tldraw) — select sub-graph
    → "Explain/Generate/Simplify" floating actions. → §5.5.
11. **Smart guides + magenta distance labels + snap** (tldraw/Miro). → §5.2.
12. **8-handle resize + rotation + modifier semantics + numeric X/Y/W/H**
    (all; Figma has numeric fields). → §5.2.
13. **Minimap interactive + toggleable** (Miro/tldraw). → §5.1.
14. **Frame-as-sub-canvas + nested groups** (tldraw/Figma) — named, clipped
    sub-model / module, enterable, exportable, reorderable. → §5.2.
15. **First-run welcome + dismissible hint chips + `?` cheatsheet** (Excalidraw
    + tldraw) — generated from same shortcut registry. → §5.4.

### 4.3. Архитектурные takeaways из open-source (tldraw + Excalidraw)

- **tldraw — эталон для component-driven Rust kit:** headless core (`Editor`
  imperative façade + typed record `Store` + signals для fine-grained
  reactivity) + separable UI layer (every panel overridable). Map to Rust:
  `Store` of `Record` enums + signal/subscription + `Editor` service +
  pluggable panel widgets. **`ShapeUtil` trait** (`default_props`,
  `geometry`/hit-test, `render`, `indicator`, `on_resize`) — directly
  portable.
- **Excalidraw — урок что НЕ делать:** god `App` class + imperative
  `<canvas>` repaint — связывает state/input/render. Для Rust — предпочесть
  tldraw separation.
- **Action-registry** (Excalidraw `shapeActionPredicates` + `CommandPalette`):
  build one `Action` table в Rust, render three ways (context menu, command
  palette, mobile sheet). → §5.4.
- **Keyboard shortcuts as declarative registry** — map of `kbd-string →
  (predicate, action)`, feeds `?` cheatsheet + command palette. → §5.4.

---

## 5. Пробелы кита и предлагаемые расширения (с сигнатурами)

> Каждый пункт: **проблема** → **эталон** → **предлагаемое расширение** с
> Rust-сигнатурой → **приоритет** (P1/P2/P3) → **волна** (T/C/L/A).

### 5.1. Состояния и interaction model (Wave T)

#### 5.1.1. `KitState` расширить до 7 состояний + state-layer (P1, Wave T)

**Проблема:** 5 состояний, нет `Focused`, `Dragged`, `Error`. Фокус рисуется
отдельно (не часть `KitState`), `Dragged` совсем отсутствует. Material 3
имеет 7 состояний (additive), Spectrum — 8 (с `key-focus`).

**Эталон:** Material 3 state-layer — hover/focus/pressed/dragged = opacity
overlay 8/10/10/16% поверх content/accent. Не новые цвета, а единый механизм.

**Предлагаемое расширение:**

```rust
// crates/canvas-ui/src/component/mod.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KitState {
    Normal,
    Hovered,
    Pressed,
    /// Keyboard focus (отдельно от mouse-hover). Рисует focus-ring
    /// (слот accent) + state-layer opacity 10% (M3 focus).
    Focused,
    /// Dragged — элемент перетаскивается (drag-операция в процессе).
    /// state-layer opacity 16% (M3 dragged).
    Dragged,
    Disabled,
    Selected,
    /// Error — поле с невалидным значением (text_field, input).
    /// Слот control_danger + state-layer.
    Error,
}

/// state-layer opacity (M3 interaction-states spec)
pub fn state_layer_alpha(state: KitState) -> f32 {
    match state {
        KitState::Hovered  => 0.08,
        KitState::Focused  => 0.10,
        KitState::Pressed  => 0.10,
        KitState::Dragged  => 0.16,
        _ => 0.0,
    }
}

/// Composite state: состояния additive (hover+focused+selected могут стекаться).
/// Возвращает (base_fill, overlay_alpha, overlay_color).
pub fn resolve_state(
    base_fill: [f32;4],
    states: &[KitState],  // active states (multiple allowed)
    accent: [f32;4],
) -> ([f32;4], f32, [f32;4]) {
    let mut alpha = 0.0;
    let mut overlay = accent;
    for &s in states {
        let a = state_layer_alpha(s);
        if a > alpha { alpha = a; overlay = accent; }
    }
    (base_fill, alpha, overlay)
}
```

**Изменение `WidgetState`:**

```rust
pub struct WidgetState {
    inside: bool,
    pressed: bool,
    selected: bool,
    disabled: bool,
    focused: bool,
    /// keyboard-originated focus (vs mouse-click focus) — :focus-visible
    focus_visible: bool,
    /// drag in progress (set by consumer during drag ops)
    dragged: bool,
    /// error state (set by consumer for invalid input)
    error: bool,
    press_armed: bool,
}

impl WidgetState {
    /// Active states (additive — may return multiple).
    pub fn active_states(&self) -> SmallVec<[KitState; 4]> {
        let mut v = SmallVec::new();
        if self.disabled { v.push(KitState::Disabled); return v; }
        if self.pressed  { v.push(KitState::Pressed); }
        if self.inside   { v.push(KitState::Hovered); }
        if self.focused && self.focus_visible { v.push(KitState::Focused); }
        if self.dragged  { v.push(KitState::Dragged); }
        if self.selected { v.push(KitState::Selected); }
        if self.error    { v.push(KitState::Error); }
        v
    }
}
```

**Приоритет:** P1. **Волна:** T (tokens). **Эффект:** фокус-рамка становится
частью state-machine (не отдельный if); drag visual стандартизирован; error-
state для text_field. **Совместимость:** `kit_state()` оставлен как
deprecated alias (возвращает "highest-priority" state) — обратная совместимость
1 кадр, потом миграция.

---

#### 5.1.2. `key-focus` ≠ mouse-focus (P2, Wave T)

**Проблема:** `WidgetState::set_focused(bool)` не различает keyboard vs mouse
focus. Фокус-рамка рисуется на mouse-click — шумно. Spectrum различает
`key-focus` (только keyboard).

**Эталон:** Spectrum `key-focus`; CSS `:focus-visible`.

**Предлагаемое расширение:**

```rust
impl WidgetState {
    pub fn set_focused(&mut self, focused: bool, visible: bool) {
        self.focused = focused;
        self.focus_visible = focused && visible;
    }
    /// Рисовать ли focus-ring (только keyboard-originated focus).
    pub fn focus_ring_visible(&self) -> bool {
        self.focus_visible && !self.disabled
    }
}
```

`visible = true` ставится когда фокус пришёл через Tab/Shift+Tab (keyboard),
`false` — когда через mouse-click. `KeyboardRouter` передаёт `visible` флаг.

**Приоритет:** P2. **Волна:** T. **Эффект:** чистый UX — фокус-рамка только
для keyboard-юзеров.

---

#### 5.1.3. `ButtonVariant` расширение (P3, Wave T)

**Проблема:** 4 варианта (Primary/Secondary/Ghost/Danger). Нет `Tertiary`,
`Text` (low-emphasis), `Inverse` (on-color, для цветных хедеров).

**Эталон:** Carbon 5 (primary/secondary/tertiary/ghost/danger) + M3 5 (text/
outlined/filled/elevated/tonal) + SLDS 7 (neutral/brand/outline/destructive/
text/inverse/success).

**Предлагаемое расширение:**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    /// Tertiary — lowest emphasis solid button (Carbon tertiary).
    Tertiary,
    /// Ghost — transparent, no border (was in v1, kept).
    Ghost,
    /// Text — text-only, no container (M3 text button).
    Text,
    /// Danger — destructive action (delete/reset).
    Danger,
    /// Inverse — on colored background (SLDS inverse).
    /// Используется на stage_dim, цветных хедерах.
    Inverse,
}
```

**Приоритет:** P3. **Волна:** T. **Эффект:** покрывает union Carbon+M3+SLDS.

---

### 5.2. Темизация и токены (Wave T)

#### 5.2.1. 4-role color pairs (P1, Wave T)

**Проблема:** `KitPalette` имеет ~19 слотов без пар. `control_fill` без
`on_control_fill` — контраст hand-tuned. Material 3 имеет 4-role pair для
каждого контейнера.

**Эталон:** M3 `role / on-role / role-container / on-role-container`.

**Предлагаемое расширение `KitPalette`:**

```rust
pub struct KitPalette {
    // === Контейнеры (4-role pairs) ===
    // Primary (акция)
    pub primary: [f32;4],            // = control_primary
    pub on_primary: [f32;4],         // foreground на primary
    pub primary_container: [f32;4],  // тонированный контейнер
    pub on_primary_container: [f32;4],

    // Secondary (обычный контрол)
    pub secondary: [f32;4],          // = control_fill
    pub on_secondary: [f32;4],
    pub secondary_container: [f32;4],
    pub on_secondary_container: [f32;4],

    // Danger
    pub danger: [f32;4],             // = control_danger
    pub on_danger: [f32;4],
    pub danger_container: [f32;4],
    pub on_danger_container: [f32;4],

    // Success/Warning (статусы)
    pub success: [f32;4],            // = control_success
    pub on_success: [f32;4],
    pub warning: [f32;4],            // = control_warning
    pub on_warning: [f32;4],

    // Surface (панель/модаль)
    pub surface: [f32;4],            // = panel_fill
    pub on_surface: [f32;4],         // = text
    pub surface_variant: [f32;4],    // = control_fill (тоньше surface)
    pub on_surface_variant: [f32;4], // = text_muted

    // Outline
    pub outline: [f32;4],            // = control_border
    pub outline_variant: [f32;4],    // тоньше outline (divider)

    // === Акцент/фокус ===
    pub accent: [f32;4],             // = accent (фокус-рамка)
    pub accent_variant: [f32;4],     // тоньше (hover-accent)

    // === Спец ===
    pub stage_dim: [f32;4],
    pub scrollbar_thumb: [f32;4],
    pub rule_color: [f32;4],

    // === Elevation (NEW, см. 5.2.3) ===
    pub shadow_color: [f32;4],       // RGBA для shadow-quads
}

/// Типобезопасный accessor (druid Env+Key<T> pattern, упрощённый).
pub fn on_fill_of(p: &KitPalette, fill: [f32;4]) -> [f32;4] {
    if fill == p.primary   { p.on_primary }
    else if fill == p.danger { p.on_danger }
    else if fill == p.success { p.on_success }
    else { p.on_surface }
}
```

**Приоритет:** P1. **Волна:** T. **Эффект:** контраст структурный, не
hand-tuned. Обратная совместимость: старые имена — deprecated aliases.

---

#### 5.2.2. Named `Size` scale (P1, Wave T)

**Проблема:** `BUTTON_HEIGHT=30`, `ICON_BUTTON_SIZE=26` — magic numbers ниже
44pt touch floor. Fluent 24/32/40, Carbon 32/40/48.

**Эталон:** Fluent `Size::Small/Medium/Large` (24/32/40); Carbon 32/40/48.

**Предлагаемое расширение:**

```rust
/// Named size scale для контролов (Fluent/Carbon pattern).
/// Visual size; hit-zone расширяется до MIN_TOUCH_TARGET отдельно (LAY8.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlSize {
    /// 24px — tight UI (toolbar, dense lists).
    Xs,
    #[default]
    /// 30px — standard (current BUTTON_HEIGHT/TEXT_FIELD_HEIGHT).
    Sm,
    /// 36px — comfortable (settings, modals).
    Md,
    /// 44px — touch-friendly (mobile, onboarding).
    Lg,
}

impl ControlSize {
    pub const fn button_h(self) -> f32 {
        match self { Self::Xs => 24.0, Self::Sm => 30.0, Self::Md => 36.0, Self::Lg => 44.0 }
    }
    pub const fn chip_h(self) -> f32 {
        match self { Self::Xs => 20.0, Self::Sm => 24.0, Self::Md => 28.0, Self::Lg => 32.0 }
    }
    pub const fn field_h(self) -> f32 {
        match self { Self::Xs => 24.0, Self::Sm => 30.0, Self::Md => 36.0, Self::Lg => 44.0 }
    }
    pub const fn icon_btn(self) -> f32 {
        match self { Self::Xs => 20.0, Self::Sm => 26.0, Self::Md => 32.0, Self::Lg => 44.0 }
    }
    pub const fn list_row_h(self) -> f32 {
        match self { Self::Xs => 22.0, Self::Sm => 26.0, Self::Md => 30.0, Self::Lg => 36.0 }
    }
}

// Deprecated aliases (обратная совместимость, удалить после миграции)
#[deprecated(note = "use ControlSize::Sm.button_h()")]
pub const BUTTON_HEIGHT: f32 = 30.0;
```

**Приоритет:** P1. **Волна:** T. **Эффект:** явный size-scale; touch-target
поднят до 44 через `ControlSize::Lg` (или hit-only, как сейчас).

---

#### 5.2.3. Elevation ramp (P2, Wave T)

**Проблема:** нет shadow-системы. Overlays (Popups/Modals/Toasts) — только
`stage_dim` scrim. Fluent 5-6 shadow ramp (2/4/8/16/28/64), M3 6 dp-levels.

**Эталон:** Fluent `shadow2/4/8/16/28/64`; M3 L0-L5.

**Предлагаемое расширение:**

```rust
/// Elevation levels (M3 + Fluent hybrid). Каждому уровню —
/// shadow-offset + shadow-blur + shadow-alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Elevation {
    /// 0 — flat (cards, panels in-place).
    #[default]
    None,
    /// 1 — subtle (hovered card, dropdown shadow).
    Xs,
    /// 2 — default modal/popover.
    Sm,
    /// 3 — raised modal over another modal.
    Md,
    /// 4 — top modal (settings over stage).
    Lg,
    /// 5 — max (dialog over everything).
    Xl,
}

impl Elevation {
    /// (offset_y, blur, alpha) для shadow-квада.
    pub const fn shadow(self) -> (f32, f32, f32) {
        match self {
            Self::None => (0.0, 0.0, 0.0),
            Self::Xs   => (1.0, 2.0, 0.08),
            Self::Sm   => (2.0, 4.0, 0.12),
            Self::Md   => (4.0, 8.0, 0.16),
            Self::Lg   => (8.0, 16.0, 0.20),
            Self::Xl   => (16.0, 28.0, 0.24),
        }
    }
}

// Painter расширение:
impl Painter {
    /// Рисует shadow-квад под rect (перед основным fill).
    pub fn shadow(&mut self, rect: UiRect, elev: Elevation, color: [f32;4]) {
        let (dy, blur, alpha) = elev.shadow();
        if alpha <= 0.0 { return; }
        let s = UiRect::new(
            rect.x - blur, rect.y - blur + dy,
            rect.w + blur * 2.0, rect.h + blur * 2.0,
        );
        self.rect(s, [color[0], color[1], color[2], alpha]);
    }
}
```

**Приоритет:** P2. **Волна:** T. **Эффект:** глубина без отказа от flat-
canvas эстетики; modals/popovers получают каноничный shadow.

---

#### 5.2.4. Motion tokens (P2, Wave T)

**Проблема:** 6 длительностей в `canvas_core::tokens` (`FOCUS_FADE_MS`,
`CAMERA_FLIGHT_MS`, и т.д.), но не формализованы как шкала в ките. M3 имеет
12 durations + 6 easings как именованные токены.

**Эталон:** M3 `Duration::{Short1..Long4}` (50..600ms) + `Easing::{Standard,
EmphasizedDecelerate, EmphasizedAccelerate, ...}`.

**Предлагаемое расширение:**

```rust
// crates/canvas-ui/src/anim.rs (расширение)

/// Motion duration scale (M3 spec, 12 шагов).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duration {
    Short1 = 50,   // hover feedback
    Short2 = 100,  // press, toggle
    Short3 = 150,  // small panel slide
    Short4 = 200,  // chip expand
    Medium1 = 250, // tooltip
    Medium2 = 300, // panel expand, camera flight
    Medium3 = 350, // modal enter
    Medium4 = 400, // large surface
    Long1 = 450,   // scene transition
    Long2 = 500,   // onboarding step
    Long3 = 550,   // theme switch
    Long4 = 600,   // max (splash)
}

impl Duration {
    pub const fn ms(self) -> u64 { self as u64 }
}

/// Easing curves (M3 spec, 6 кривых).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    Standard,               // cubic-bezier(0.2, 0, 0, 1)
    StandardDecelerate,     // (0, 0, 0, 1)
    StandardAccelerate,     // (0.3, 0, 1, 1)
    Emphasized,             // (0.2, 0, 0, 1)
    EmphasizedDecelerate,   // (0.05, 0.7, 0.1, 1)
    EmphasizedAccelerate,   // (0.3, 0, 0.8, 0.15)
}

impl Easing {
    pub const fn bezier(self) -> [f32; 4] {
        match self {
            Self::Standard             => [0.2, 0.0, 0.0, 1.0],
            Self::StandardDecelerate   => [0.0, 0.0, 0.0, 1.0],
            Self::StandardAccelerate   => [0.3, 0.0, 1.0, 1.0],
            Self::Emphasized           => [0.2, 0.0, 0.0, 1.0],
            Self::EmphasizedDecelerate => [0.05, 0.7, 0.1, 1.0],
            Self::EmphasizedAccelerate => [0.3, 0.0, 0.8, 0.15],
        }
    }
}

/// Reduced-motion уважение: durations → 0, easings → linear.
/// Флаг из canvas_core::web_bridge (prefers-reduced-motion).
pub fn effective_duration(d: Duration) -> u64 {
    if reduced_motion() { 0 } else { d.ms() }
}
```

**Приоритет:** P2. **Волна:** T. **Эффект:** формализация motion-шкалы;
a11y (reduced-motion) уважается структурно.

---

#### 5.2.5. Shape scale enum (P3, Wave T)

**Проблема:** `ControlStyle.radius: f32` — bare float, magic numbers. M3 имеет
7-step shape scale.

**Эталон:** M3 `Shape::{None, Xs, S, M, L, Xl, Full}`.

**Предлагаемое расширение:**

```rust
/// Shape scale (M3 7 шагов). Заменяет bare `radius: f32`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    #[default]
    None, // 0
    Xs,   // 4
    S,    // 8  (= RADIUS_CHIP, current chip)
    M,    // 10 (= RADIUS_PANEL/CARD, current panel)
    L,    // 12 (= RADIUS_PILL, current pill)
    Xl,   // 16
    Full, // 999 (circle/pill)
}

impl Shape {
    pub const fn px(self) -> f32 {
        match self {
            Self::None => 0.0, Self::Xs => 4.0, Self::S => 8.0,
            Self::M => 10.0, Self::L => 12.0, Self::Xl => 16.0,
            Self::Full => 999.0,
        }
    }
}

// ControlStyle миграция:
pub struct ControlStyle {
    pub fill: [f32;4],
    pub border: [f32;4],
    pub text: [f32;4],
    pub shape: Shape,  // было: radius: f32
    pub elevation: Elevation,  // NEW (5.2.3)
}
```

**Приоритет:** P3. **Волна:** T. **Эффект:** magic radii исчезают; каждый
компонент декларирует свой shape-step.

---

#### 5.2.6. Spacing enum (P3, Wave T)

**Проблема:** 5 значений `SPACING_S/SM/MD/LG/XL` в `tokens.rs`, но в коде
поверхностей — литералы (gaps, paddings). Carbon имеет 13 шагов.

**Эталон:** Carbon 2/4/8/12/16/24/32/40/48/64/80/96/160.

**Предлагаемое расширение:**

```rust
/// Spacing scale (Carbon 13 шагов, our 5 — subset).
/// Используется в Row.gap, Column.gap, EdgeInsets, paddings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Spacing {
    Xxs = 2,   // hairline (divider inset)
    Xs = 4,    // tight (icon-to-label)
    #[default]
    S = 6,     // chip gap, row gap (current SPACING_S)
    Sm = 8,    // control gap (current SPACING_SM)
    Md = 10,   // bar padding (current SPACING_MD)
    Lg = 12,   // panel pad (current SPACING_LG)
    Xl = 16,   // section gap
    Xxl = 24,  // viewport margin (current SPACING_XL)
    Xxxl = 32, // hero spacing
    Huge = 48, // modal air
    Giant = 64,
    Mega = 96,
    Ultra = 160,
}

impl Spacing {
    pub const fn px(self) -> f32 { self as f32 }
}
```

**Приоритет:** P3. **Волна:** T. **Эффект:** `lay7_lint.sh` (LAY-W12)
получает enum-based allowlist вместо regex; magic literals исчезают.

---

### 5.3. Новые компоненты (Wave C)

> Порядок — по приоритету. P1 = закрыть базовые input-пробелы; P2 = навигация
> и feedback; P3 = data-display и polish.

#### 5.3.1. `Checkbox` (P1, Wave C)

**Проблема:** нет. В настройках используются `radio_card` (одиночный) и
`switch`, но multi-select checkbox отсутствует. Material/Carbon/Apple —
базовый компонент.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/checkbox.rs

/// Checkbox — двух/трёхсостояющий toggle с подписью.
pub struct CheckboxLayout {
    /// Rect чек-бокса (квадрат, сторона = kit::LIST_ROW_H или ControlSize).
    pub box_rect: UiRect,
    /// Rect подписи (измеренная).
    pub label_rect: UiRect,
    /// Подпись после ellipsis (если слот узкий).
    pub label: String,
    /// Состояние (unchecked/checked/indeterminate).
    pub state: CheckboxState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CheckboxState {
    #[default]
    Unchecked,
    Checked,
    /// Indeterminate — для "select all" когда выбраны не все.
    Indeterminate,
}

pub fn checkbox_layout(
    slot: UiRect,
    label: &str,
    state: CheckboxState,
    size: ControlSize,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
    family: &str,
    font_size: f32,
) -> CheckboxLayout { /* ... */ }

pub fn checkbox_style(
    state: CheckboxState,
    kit_state: KitState,
    p: &KitPalette,
) -> ControlStyle { /* ... */ }

/// Иконка галочки/прочерка (CanvasDeskSymbols или glyph).
pub fn checkbox_glyph(state: CheckboxState) -> &'static str {
    match state {
        CheckboxState::Unchecked => "",
        CheckboxState::Checked => "✓",
        CheckboxState::Indeterminate => "−",
    }
}
```

**Anatomy:** `box (квадрат) + label (текст справа)`. **States:** 7 KitState
× 3 CheckboxState. **Keyboard:** Space toggles. **Touch:** ≥44px hit-zone.

---

#### 5.3.2. `Switch` (переработать, P1, Wave C)

**Проблема:** `switch()` есть в `button.rs`, но возвращает только `SwitchLayout`
без `SwitchStyle` и без state-machine. Нет disabled/on-off visual difference
через state-layer.

**Предлагаемая переработка:**

```rust
// crates/canvas-ui/src/component/switch.rs (вынести из button.rs)

pub struct SwitchLayout {
    /// Rect трека (pill).
    pub track: UiRect,
    /// Rect бегунка (knob).
    pub knob: UiRect,
    /// On/off.
    pub on: bool,
}

pub struct SwitchStyle {
    pub track_fill: [f32;4],
    pub track_border: [f32;4],
    pub knob_fill: [f32;4],
    pub shape: Shape,
}

pub fn switch_layout(
    slot: UiRect,
    on: bool,
    size: ControlSize,
) -> SwitchLayout { /* ... */ }

pub fn switch_style(
    on: bool,
    state: KitState,
    p: &KitPalette,
) -> SwitchStyle {
    let track = if on { p.primary } else { p.secondary_container };
    let knob = if on { p.on_primary } else { p.outline };
    // state-layer overlay для hover/focus
    SwitchStyle { track_fill: track, track_border: p.outline_variant,
                  knob_fill: knob, shape: Shape::Full }
}

/// Анимация переключения (knob slide) — consumer-driven через anim.rs.
pub fn switch_knob_offset(
    on: bool,
    progress: f32,  // 0..1 анимация
    track_w: f32,
    knob_w: f32,
) -> f32 { /* lerp */ }
```

---

#### 5.3.3. `Slider` (P1, Wave C)

**Проблема:** нет. What-if сценарии, settings (cost-limit, conf-threshold)
нуждаются в slider. egui/iced/M3 — базовый компонент.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/slider.rs

pub struct SliderLayout {
    /// Rect трека (горизонтальная полоса).
    pub track: UiRect,
    /// Rect заполненной части (от начала до значения).
    pub filled: UiRect,
    /// Rect бегунка (knob, квадрат/круг).
    pub knob: UiRect,
    /// Текущее значение [0..1] (normalized).
    pub value: f32,
}

pub struct SliderOpts {
    pub min: f32,
    pub max: f32,
    pub step: Option<f32>,     // None — continuous
    pub discrete_ticks: bool,  // рисовать метки шага
    pub label_format: Option<fn(f32) -> String>,  // "50 ms"
}

pub fn slider_layout(
    slot: UiRect,
    value: f32,
    opts: &SliderOpts,
    size: ControlSize,
) -> SliderLayout { /* ... */ }

pub fn slider_style(
    state: KitState,
    p: &KitPalette,
) -> (ControlStyle, [f32;4]) { /* track_style, knob_fill */ }

/// Hit-зоны: knob (drag) + track (jump-to-click).
pub fn slider_hit(layout: &SliderLayout, p: UiPoint) -> Option<SliderHit> {
    if layout.knob.contains(p) { Some(SliderHit::Knob) }
    else if layout.track.contains(p) { Some(SliderHit::Track) }
    else { None }
}

pub enum SliderHit { Knob, Track }

/// Keyboard: ←/→ — step, Home/End — min/max, PageUp/Down — big step.
pub fn slider_key(kit_state: KitState, key: Key, opts: &SliderOpts) -> Option<f32> { /* */ }
```

---

#### 5.3.4. `RadioGroup` (P1, Wave C)

**Проблема:** `radio_card` есть (для настроек, карточный выбор), но
стандартный radio-button (круг + точка) отсутствует. Нужен для compact форм.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/radio.rs

pub struct RadioLayout {
    /// Rect круга (radio button).
    pub circle: UiRect,
    /// Rect подписи.
    pub label_rect: UiRect,
    pub label: String,
    pub selected: bool,
}

/// Группа radio: управляет exclusive-selection.
pub struct RadioGroup {
    pub options: Vec<String>,  // labels
    pub selected: usize,
}

pub fn radio_group_layout(
    slot: UiRect,
    group: &RadioGroup,
    orientation: Orientation,  // Horizontal/Vertical
    size: ControlSize,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> Vec<RadioLayout> { /* ... */ }

/// Arrow-key навигация по группе (Fluent FocusZone pattern).
pub fn radio_group_key(
    group: &mut RadioGroup,
    key: Key,
    orientation: Orientation,
) -> bool {
    match (key, orientation) {
        (Key::ArrowRight, Orientation::Horizontal)
        | (Key::ArrowDown, Orientation::Vertical) => {
            group.selected = (group.selected + 1) % group.options.len();
            true
        }
        (Key::ArrowLeft, Orientation::Horizontal)
        | (Key::ArrowUp, Orientation::Vertical) => {
            group.selected = if group.selected == 0 {
                group.options.len() - 1
            } else { group.selected - 1 };
            true
        }
        _ => false,
    }
}
```

---

#### 5.3.5. `Tabs` (P1, Wave C)

**Проблема:** нет. Настройки имеют tabs (через `settings_ui.rs` свою реализацию
на `y +=`), но как компонент кита отсутствует. Нужен для Figma-style nav-rail
(§5.5.1), настроек, explain panel.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/tabs.rs

pub struct TabLayout {
    /// Rect таба.
    pub rect: UiRect,
    /// Подпись после ellipsis.
    pub label: String,
    /// Подчёркивание активного таба (Material style) или фон (Carbon style).
    pub indicator: UiRect,
    pub active: bool,
}

pub struct TabsLayout {
    /// Rect полосы табов.
    pub bar: UiRect,
    /// Сами табы.
    pub tabs: Vec<TabLayout>,
    /// Rect контент-области под табами.
    pub content: UiRect,
}

pub enum TabStyle {
    /// Подчёркивание активного (Material 3).
    Underline,
    /// Заливка активного (Carbon).
    Fill,
    /// Pill-активный (Figma UI3).
    Pill,
}

pub fn tabs_layout(
    slot: UiRect,
    labels: &[String],
    active: usize,
    style: TabStyle,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> TabsLayout { /* ... */ }

/// Arrow-key навигация (Fluent FocusZone): ←/→ по табам, Tab — в контент.
pub fn tabs_key(active: &mut usize, count: usize, key: Key) -> bool { /* */ }
```

---

#### 5.3.6. `Accordion` / `CollapsibleSection` (P2, Wave C)

**Проблема:** нет. Настройки — плоский scroll, без сворачиваемых секций. egui
`CollapsingHeader`, M3 `Expansion panels`.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/accordion.rs

pub struct AccordionItem {
    pub header: String,
    pub expanded: bool,
    /// Content layout callback (called when expanded).
    pub content: Box<dyn Fn(UiRect) -> Vec<UiRect>>,
}

pub struct AccordionLayout {
    /// Rect заголовка (chevron + label).
    pub header: UiRect,
    /// Rect контента (0-height если свёрнут).
    pub content: UiRect,
    /// Rect шеврона (▶/▼).
    pub chevron: UiRect,
    pub expanded: bool,
}

pub fn accordion_layout(
    slot: UiRect,
    item: &AccordionItem,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> AccordionLayout { /* ... */ }
```

---

#### 5.3.7. `Progress` (bar + spinner) (P2, Wave C)

**Проблема:** нет. AI-агент busy, graph_builder busy, scenario worker —
используют текст "..." или ad-hoc. M3/Carbon/egui — базовый компонент.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/progress.rs

pub enum ProgressKind {
    /// Linear determinate (0..1).
    Linear { value: f32 },
    /// Linear indeterminate (анимация).
    LinearIndeterminate,
    /// Circular determinate (0..1).
    Circular { value: f32, radius: f32 },
    /// Circular indeterminate (spinner).
    Spinner { radius: f32 },
}

pub struct ProgressLayout {
    pub track: UiRect,
    pub fill: UiRect,  // для Linear
    pub kind: ProgressKind,
}

pub fn progress_layout(
    slot: UiRect,
    kind: ProgressKind,
    size: ControlSize,
) -> ProgressLayout { /* ... */ }

/// Animation phase для indeterminate (0..1, loop).
/// Consumer передаёт `time_ms` из anim.rs.
pub fn progress_phase(time_ms: u64, kind: &ProgressKind) -> f32 { /* */ }
```

---

#### 5.3.8. `Skeleton` (P2, Wave C)

**Проблема:** нет. Загрузка схем, поиск, explain — показывают пустоту. M3/
Carbon — skeleton-заглушки.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/skeleton.rs

pub struct SkeletonLayout {
    pub rects: Vec<UiRect>,
    /// Pulse phase (0..1) из anim.rs.
    pub phase: f32,
}

/// Генерирует skeleton-заглушки по шаблону (строки + блоки).
pub fn skeleton_layout(
    slot: UiRect,
    pattern: &SkeletonPattern,
) -> SkeletonLayout { /* ... */ }

pub enum SkeletonPattern {
    /// N строк текста (row_h + gap).
    TextLines { count: usize, row_h: f32 },
    /// Карточка: header + body lines + footer.
    Card { w: f32, h: f32 },
    /// Таблица: rows × cols.
    Table { rows: usize, cols: usize },
}

/// Pulse-анимация (M3 shimmer): alpha 0.04 ↔ 0.12, period 1500ms.
pub fn skeleton_alpha(phase: f32) -> f32 {
    0.04 + 0.08 * (phase * std::f32::consts::TAU).sin().abs()
}
```

---

#### 5.3.9. `Badge` (P2, Wave C)

**Проблема:** нет. Бейджи рисуются ad-hoc (severity, what-if delta, count).
M3 badge, Carbon tag, SLDS badge — стандартизированы.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/badge.rs

pub enum BadgeKind {
    /// Числовой счётчик (уведомления, выделенные элементы).
    Count(usize),
    /// Текстовый бейдж (статус, категория).
    Text(String),
    /// Точка-индикатор (онлайн, изменено).
    Dot,
}

pub enum BadgeTone {
    Default,    // secondary
    Primary,    // accent
    Success,    // green
    Warning,    // amber
    Danger,     // red
    Info,       // blue
}

pub struct BadgeLayout {
    pub rect: UiRect,
    pub kind: BadgeKind,
    pub tone: BadgeTone,
}

pub fn badge_layout(
    anchor: UiRect,  // позиционируется relative к якорю (top-right corner)
    kind: BadgeKind,
    tone: BadgeTone,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> BadgeLayout { /* ... */ }

pub fn badge_style(tone: BadgeTone, p: &KitPalette) -> ControlStyle { /* */ }
```

---

#### 5.3.10. `Avatar` (P3, Wave C)

**Проблема:** нет. Комментарии, collaboration, AI-агент — нуждаются в avatar
(инициалы/иконка/изображение).

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/avatar.rs

pub enum AvatarSource {
    Initials(String),   // "AB" — 2 буквы
    Icon(IconKind),     // иконка-плейсхолдер
    Image(texture_id),  // растровое изображение
}

pub struct AvatarLayout {
    pub rect: UiRect,   // квадрат
    pub source: AvatarSource,
    pub tone: BadgeTone,  // цвет фона для initials/icon
}

pub fn avatar_layout(
    slot: UiRect,
    source: AvatarSource,
    size: ControlSize,
) -> AvatarLayout { /* ... */ }
```

---

#### 5.3.11. `Tree` (P3, Wave C)

**Проблема:** нет. Explain panel (дерево расчёта), будущий layers-panel —
нуждаются в tree-компоненте. Carbon tree, M3 list — референсы.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/tree.rs

pub struct TreeNode {
    pub id: String,
    pub label: String,
    pub children: Vec<TreeNode>,
    pub expanded: bool,
    pub depth: usize,
    pub icon: Option<IconKind>,
}

pub struct TreeLayout {
    pub rows: Vec<TreeRow>,
}

pub struct TreeRow {
    pub rect: UiRect,
    pub indent: f32,    // depth * indent_step
    pub chevron: UiRect,  // ▶/▼ (if has children)
    pub icon: Option<UiRect>,
    pub label: UiRect,
    pub label_text: String,
    pub depth: usize,
    pub expanded: bool,
    pub has_children: bool,
}

pub fn tree_layout(
    slot: UiRect,
    root: &TreeNode,
    scroll: &ScrollState,
    row_h: f32,
    indent_step: f32,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> TreeLayout { /* flatten visible nodes */ }

/// Arrow-key навигация (Fluent FocusZone): ↑/↓ по строкам, ←/→ expand/collapse.
pub fn tree_key(
    layout: &TreeLayout,
    focused: &mut Option<String>,
    key: Key,
) -> TreeAction { /* */ }

pub enum TreeAction {
    FocusNext, FocusPrev, Expand, Collapse, Activate, None,
}
```

---

#### 5.3.12. `SegmentedControl` (P3, Wave C)

**Проблема:** нет. What-if mode switcher, view-mode switcher — рисуются ad-hoc.
Apple HIG segmented control, M3 segmented button.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/segmented.rs

pub struct SegmentedLayout {
    /// Rect контейнера (pill-фон).
    pub container: UiRect,
    /// Сегменты.
    pub segments: Vec<UiRect>,
    /// Активный сегмент (заливка).
    pub active_rect: UiRect,
    pub active: usize,
}

pub fn segmented_layout(
    slot: UiRect,
    labels: &[String],
    active: usize,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> SegmentedLayout { /* ... */ }
```

---

#### 5.3.13. `Popover` (P2, Wave C)

**Проблема:** `anchored_stack` есть, но как отдельный компонент с arrow,
state, close-on-outside-click — отсутствует. M3 popover, Radix popover.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/popover.rs

pub struct PopoverLayout {
    pub content: UiRect,
    /// Стрелка-указатель на якорь.
    pub arrow: UiRect,
    pub arrow_side: AnchoredSide,
    pub flipped: bool,
}

pub fn popover(
    anchor: UiRect,
    viewport: UiRect,
    content_size: UiVec2,
    preferred_side: AnchoredSide,
    show_arrow: bool,
) -> PopoverLayout { /* ... */ }

/// State: open/closed + close-on-outside-click handled by SurfaceRegistry
/// (Block capture policy).
```

---

#### 5.3.14. `Snackbar` / `Toast` с action (P2, Wave C)

**Проблема:** `toast_area` есть (позиционирование), но без action-button и
без proper state-machine. M3 snackbar имеет optional action + dismiss.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/snackbar.rs (расширение toast)

pub struct Snackbar {
    pub message: String,
    pub action_label: Option<String>,
    pub ttl_ms: u64,
    pub tone: BadgeTone,
}

pub struct SnackbarLayout {
    pub rect: UiRect,
    pub message_rect: UiRect,
    pub action_rect: Option<UiRect>,
    pub label: String,
    pub action_label: Option<String>,
}

pub fn snackbar_layout(
    viewport: UiRect,
    snack: &Snackbar,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> SnackbarLayout { /* bottom-center, pill shape */ }

/// Queue: несколько snackbars стакаются вертикально (bottom-center).
pub struct SnackbarQueue {
    pub items: Vec<(Snackbar, f64)>,  // (snack, expire_time)
}
impl SnackbarQueue {
    pub fn push(&mut self, snack: Snackbar, now: f64) { /* */ }
    pub fn tick(&mut self, now: f64) { /* expire */ }
}
```

---

#### 5.3.15. `CommandPalette` (P1, Wave A)

**Проблема:** нет. tldraw/Excalidraw/Eraser/Miro/Figma — все имеют Cmd+K.
CanvasDesk — 19 поверхностей, нужен единый action-surface.

**Предлагаемая сигнатура:**

```rust
// crates/canvas-ui/src/component/command_palette.rs

/// Action-registry — единый реестр действий (Excalidray shapeActionPredicates
// pattern). Один источник правды для: command palette, context menu,
// keyboard shortcuts cheatsheet, mobile sheet.
pub struct ActionRegistry {
    actions: Vec<Action>,
}

pub struct Action {
    pub id: String,
    pub label: String,
    /// Shortcut (kbd-string: "cmd+k", "shift+0").
    pub kbd: Option<String>,
    /// Predicate: visible/enable в текущем состоянии.
    pub predicate: Box<dyn Fn(&AppState) -> bool>,
    /// Handler.
    pub handler: Box<dyn Fn(&mut AppState)>,
    /// Category для группировки в palette.
    pub category: ActionCategory,
}

pub enum ActionCategory {
    File, Edit, View, Navigate, Tools, Help, Ai,
}

impl ActionRegistry {
    pub fn register(&mut self, action: Action) { /* */ }
    /// Filter by predicate + fuzzy-match query against label.
    pub fn search(&self, query: &str, state: &AppState) -> Vec<&Action> { /* */ }
}

pub struct CommandPaletteLayout {
    pub input: UiRect,
    pub results: Vec<(UiRect, &Action)>,
    pub selected: usize,
}

pub fn command_palette_layout(
    viewport: UiRect,
    query: &str,
    results: &[&Action],
    selected: usize,
    m: &mut TextMeasurer,
    fs: &mut cosmic_text::FontSystem,
) -> CommandPaletteLayout { /* centered-top, 560px wide, 8 results */ }

/// Keyboard: ↑/↓ navigate, Enter activate, Esc close, Tab — autocomlete.
```

---

### 5.4. Примитивы вёрстки (Wave L)

#### 5.4.1. `grid_auto` / `minmax` (P1, Wave L)

**Проблема:** `grid_cells` — только равные колонки row-major. Нет
auto-columns (card-grid с переменной шириной), нет `minmax(min, 1fr)`.

**Эталон:** CSS Grid `grid-template-columns: repeat(auto-fill, minmax(200px, 1fr))`.

**Предлагаемое расширение:**

```rust
// crates/canvas-ui/src/layout.rs

/// Grid с auto-columns (CSS Grid auto-fill + minmax).
pub struct GridAuto {
    /// Минимальная ширина колонки.
    pub min_col_w: f32,
    /// Максимальная ширина (None = 1fr — заполняет).
    pub max_col_w: Option<f32>,
    /// Зазор между колонками/рядами.
    pub gap: f32,
}

pub fn grid_auto(slot: UiRect, items: &[UiVec2], opts: &GridAuto) -> Vec<UiRect> {
    // 1. Вычислить число колонок: floor((slot.w + gap) / (min_col_w + gap))
    let n_cols = (((slot.w + opts.gap) / (opts.min_col_w + opts.gap)).floor()
        as usize).max(1);
    // 2. Фактическая ширина колонки: (slot.w - gap*(n-1)) / n
    let col_w = (slot.w - opts.gap * (n_cols - 1) as f32) / n_cols as f32;
    // 3. Row-major layout с Wrap-like переносом
    /* ... */
}

/// MinMax трек (CSS Grid minmax).
pub enum Track {
    Fixed(f32),
    /// Min-content.
    MinContent,
    /// Max-content.
    MaxContent,
    /// Fractional (1fr).
    Fr(f32),
    /// MinMax(min, max).
    MinMax(TrackMin, TrackMax),
    /// Auto (fill-available).
    Auto,
}

pub fn grid_template(
    slot: UiRect,
    cols: &[Track],
    row_h: f32,
    gap: f32,
    items: &[UiVec2],
) -> Vec<UiRect> { /* CSS Grid §11.5-11.8 resolution */ }
```

**Приоритет:** P1. **Волна:** L. **Эффект:** template-gallery, scheme-gallery
получают responsive card-grid без ручного cols расчёта.

---

#### 5.4.2. `aspect_ratio` (P2, Wave L)

**Проблема:** `SceneNode` имеет `aspect`, но в примитивах V-5 нет. Превью
схем, minimap, иконочные слоты — нуждаются.

**Предлагаемое расширение:**

```rust
// layout.rs

/// Зафиксировать aspect-ratio ребёнка в слоте.
pub fn aspect_ratio(slot: UiRect, ratio: f32, align: (HAlign, VAlign)) -> UiRect {
    let slot_ratio = slot.w / slot.h;
    let (w, h) = if slot_ratio > ratio {
        // Слот шире — ограничиваем по высоте.
        (slot.h * ratio, slot.h)
    } else {
        (slot.w, slot.w / ratio)
    };
    stack(slot, UiVec2::new(w, h), align.0, align.1)
}
```

---

#### 5.4.3. `sticky` (P2, Wave L)

**Проблема:** `ScenePosition::Sticky` есть (FR-074), но в примитивах V-5 нет
удобной обёртки. Заголовки таблиц при скролле — нуждаются.

**Предлагаемое расширение:**

```rust
/// Sticky-блок в scroll-контейнере: остаётся при скролле.
pub fn sticky_header(
    scroll_area: UiRect,
    scroll_offset: f32,
    header_h: f32,
) -> UiRect {
    UiRect::new(
        scroll_area.x,
        scroll_area.y + scroll_offset.min(0.0).abs().min(header_h) - header_h,
        scroll_area.w,
        header_h,
    )
}
```

---

#### 5.4.4. `Responsive` / window-size-class (P2, Wave L)

**Проблема:** нет. iced `Responsive` получает available-size; M3 5 window-
size-classes; Apple Compact·Regular.

**Предлагаемое расширение:**

```rust
/// Window size class (M3 5-step упрощённый до 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowClass {
    /// < 600px width — mobile/narrow.
    Compact,
    /// 600..840 — tablet/small desktop.
    Medium,
    /// >= 840 — desktop.
    Expanded,
}

impl WindowClass {
    pub fn from_width(w: f32) -> Self {
        if w < 600.0 { Self::Compact }
        else if w < 840.0 { Self::Medium }
        else { Self::Expanded }
    }
}

/// Responsive layout: вызывает closure с available-size.
pub fn responsive<T>(
    slot: UiRect,
    f: impl FnOnce(WindowClass, UiRect) -> T,
) -> T {
    f(WindowClass::from_width(slot.w), slot)
}
```

---

#### 5.4.5. Container queries (P3, Wave L)

**Проблема:** media-queries работают по viewport, но панели могут быть узкими
даже на широком экране (док слева). Нужны container-queries.

**Эталон:** CSS `@container`.

**Предлагаемое расширение:** `responsive()` (5.4.4) уже является container-
query (по слоту, не по viewport). Дополнить — `Density::from_slot(slot)`
возвращает Compact/Comfortable/Spacious на основе высоты слота.

---

### 5.5. Архитектурные паттерны (Wave A)

#### 5.5.1. `Response` pattern (egui) (P1, Wave A)

**Проблема:** каждый вызов `kit.button(...)` возвращает `ButtonLayout` (только
геометрия). Состояние (hovered? clicked? dragged?) — потребитель отслеживает
через `WidgetState` отдельно. egui возвращает `Response` — всё сразу.

**Эталон:** egui `Response { hovered, clicked, dragged, has_focus, changed }`.

**Предлагаемое расширение:**

```rust
// crates/canvas-ui/src/component/mod.rs

/// Результат вызова виджета: геометрия + interaction-state.
/// (egui Response pattern, адаптированный под immediate-mode kit.)
pub struct Response {
    /// Rect виджета (для hit-test, tooltip-anchor).
    pub rect: UiRect,
    /// Active KitState (из WidgetState).
    pub state: KitState,
    /// Hovered в этом кадре.
    pub hovered: bool,
    /// Clicked (press+release внутри) — true ровно один кадр.
    pub clicked: bool,
    /// Dragged (press inside + move > threshold).
    pub dragged: bool,
    /// Drag-delta (если dragged).
    pub drag_delta: UiVec2,
    /// Has keyboard focus.
    pub has_focus: bool,
    /// Focus-visible (keyboard-originated).
    pub focus_visible: bool,
    /// Changed (для input — значение изменилось).
    pub changed: bool,
}

impl Response {
    /// Chain tooltip (egui on_hover_text).
    pub fn on_hover_text(self, _text: &str) -> Self {
        // Consumer-side: register tooltip for next frame
        // (stored in TooltipState, shown after TOOLTIP_DELAY_MS).
        self
    }
    /// Chain disabled-if (egui UiBuilder scoped).
    pub fn disabled_if(self, _cond: bool) -> Self { self }
    /// Chain custom action on click.
    pub fn on_click(self, _f: impl FnOnce()) -> Self { self }
}

// Usage:
let resp = kit::button_draw(painter, &mut widget_state, rect, "Save", ...);
if resp.clicked { save(); }
resp.on_hover_text("Save changes (Ctrl+S)");
```

**Приоритет:** P1. **Волна:** A. **Эффект:** убирает boilerplate
`if widget_state.clicked() { ... }` + `set_tooltip(...)`. Эргономика egui без
перехода на retained.

---

#### 5.5.2. `FocusTrap` примитив (P2, Wave A)

**Проблема:** `KeyboardRouter` + `esc_stack` делают half-job. Modal focus-
trap (Tab не выходит за пределы модали) — нет примитива. Fluent FocusTrap.

**Предлагаемое расширение:**

```rust
// crates/canvas-ui/src/keyboard.rs

/// FocusTrap: Tab/Shift+Tab циклит по rect'ам внутри trap.
/// Используется modal-поверхностями (Block capture).
pub struct FocusTrap {
    ring: FocusRing,
    /// Outer rect (modal panel) — для проверки "не вышел ли Tab".
    boundary: UiRect,
}

impl FocusTrap {
    pub fn new(boundary: UiRect) -> Self {
        Self { ring: FocusRing::new(), boundary }
    }
    /// Add focusable rect (Tab-порядок).
    pub fn push(&mut self, r: UiRect) { self.ring.push(r); }
    /// Handle Tab/Shift+Tab — цикл по ring, clamp в boundary.
    pub fn on_tab(&mut self, shift: bool) -> Option<UiRect> {
        if shift { self.ring.prev() } else { self.ring.next() }
    }
    /// Текущий фокус-rect (для focus-ring paint).
    pub fn current(&self) -> Option<UiRect> { self.ring.current().copied() }
    /// Return focus при закрытии trap (focus-restore).
    pub fn restore(&mut self, previous: Option<UiRect>) { /* */ }
}
```

---

#### 5.5.3. `ActionRegistry` (P1, Wave A)

**Проблема:** каждое действие (open settings, toggle what-if, save, etc.)
привязано к своему хоткею / меню / кнопке независимо. Нет единого реестра.
Excalidraw `shapeActionPredicates` + `CommandPalette` — один реестр на 3
поверхности (context menu, palette, cheatsheet).

**Эталон:** Excalidraw action-registry; tldraw `defaultKeyboardShortcuts`.

**Предлагаемое расширение:**

```rust
// crates/canvas-ui/src/action.rs

pub struct ActionRegistry {
    actions: Vec<Action>,
}

pub struct Action {
    pub id: String,
    pub label: String,
    pub kbd: Option<String>,  // "cmd+k", "shift+0"
    pub category: ActionCategory,
    pub predicate: Box<dyn Fn(& dyn std::any::Any) -> bool>,
    pub handler: Box<dyn Fn(&mut dyn std::any::Any)>,
}

impl ActionRegistry {
    pub fn register(&mut self, a: Action) { /* */ }
    /// Search by fuzzy-match label, filter by predicate.
    pub fn search(&self, q: &str, state: &dyn std::any::Any) -> Vec<&Action> { /* */ }
    /// Parse kbd-string → KeyPattern for KeyboardRouter.
    pub fn shortcut_of(&self, id: &str) -> Option<KeyPattern> { /* */ }
    /// Generate cheatsheet (for ? dialog).
    pub fn cheatsheet(&self) -> Vec<(ActionCategory, Vec<&Action>)> { /* */ }
    /// Generate context-menu items (filtered by predicate).
    pub fn context_items(&self, state: &dyn std::any::Any) -> Vec<&Action> { /* */ }
}
```

**Приоритет:** P1. **Волна:** A. **Эффект:** command palette (5.3.15),
context menu, `?` cheatsheet, keyboard shortcuts — один источник правды.

---

#### 5.5.4. Figma-style nav-rail (P2, Wave A)

**Проблема:** 19 поверхностей — каждая со своим входом (хоткей/меню/кнопка).
Figma UI3 — slim left-most rail of tabs, swap adjacent sidebar.

**Эталон:** Figma UI3 nav-bar + dynamic left sidebar.

**Предлагаемое расширение:**

```rust
// crates/canvas-ui/src/component/nav_rail.rs

pub struct NavRail {
    pub items: Vec<NavRailItem>,
    pub active: usize,
}

pub struct NavRailItem {
    pub id: String,       // "templates", "calc", "explain"
    pub icon: IconKind,
    pub label: String,
    pub badge: Option<BadgeKind>,
}

pub struct NavRailLayout {
    /// Rect rail (вертикальная полоса слева, ~48px).
    pub rail: UiRect,
    /// Rect кнопок.
    pub buttons: Vec<UiRect>,
    /// Rect sidebar (dynamic content, ~280px).
    pub sidebar: UiRect,
    pub active: usize,
}

pub fn nav_rail_layout(
    viewport: UiRect,
    rail: &NavRail,
    sidebar_w: f32,
) -> NavRailLayout { /* */ }

/// Click on rail item → activate + emit AppEvent::NavRailActivate(id).
/// Sidebar content — consumer-side (per active item).
```

---

#### 5.5.5. `WidgetExt` modifier trait (P3, Wave A)

**Проблема:** `kit::button_draw(...)` — плоский вызов. druid `WidgetExt`
позволяет `.padding().on_click().disabled()`.

**Эталон:** druid `WidgetExt`.

**Предлагаемое расширение (для Response):**

```rust
pub trait WidgetExt: Sized {
    fn padding(self, _inset: EdgeInsets) -> Self { self }
    fn on_click(self, _f: impl FnOnce()) -> Self { self }
    fn on_hover_text(self, _t: &str) -> Self { self }
    fn disabled_if(self, _cond: bool) -> Self { self }
    fn with_state(self, _s: KitState) -> Self { self }
    fn with_slot(self, _key: &str, _val: impl ToString) -> Self { self }
}

impl WidgetExt for Response {}
```

**Приоритет:** P3. **Волна:** A. **Эффект:** chainable ergonomics.

---

#### 5.5.6. Canvas-UX: smart guides + distance labels (P2, Wave A)

**Проблема:** `snap.rs` делает grid-snap, но alignment-guides к siblings +
distance labels — нет. tldraw/Miro/Figma — стандарт.

**Эталон:** tldraw magenta distance labels; Miro smart guides.

**Предлагаемое расширение (canvas-app, не kit):**

```rust
// crates/canvas-app/src/app/smart_guides.rs

pub struct SmartGuides {
    /// Линии выравнивания (vertical/horizontal).
    pub lines: Vec<GuideLine>,
    /// Distance labels между dragged и siblings.
    pub labels: Vec<DistanceLabel>,
}

pub struct GuideLine {
    pub orientation: Orientation,
    pub pos: f32,  // x or y
    pub range: [f32; 2],  // start/end
}

pub struct DistanceLabel {
    pub pos: UiPoint,
    pub text: String,  // "12 px"
}

pub fn compute_smart_guides(
    dragged: UiRect,
    siblings: &[UiRect],
    threshold: f32,  // 6px snap
) -> SmartGuides { /* */ }
```

---

## 6. План внедрения (4 волны)

### Wave T — Tokens & States (Quick-win, 0 новых компонентов)

**Скоуп:** расширение `KitState`, `KitPalette`, метрики; добавление `Elevation`,
`Duration`/`Easing`, `Shape`, `Spacing`, `ControlSize` enum'ов.

| Задача | Приоритет | Файлы | Эффект |
|---|---|---|---|
| `KitState` +2 (Focused, Dragged) + state-layer | P1 | `component/mod.rs`, `widget.rs` | 7 состояний, M3-parity |
| `key-focus` ≠ mouse-focus | P2 | `widget.rs`, `keyboard.rs` | `:focus-visible` семантика |
| `ButtonVariant` +3 (Tertiary, Text, Inverse) | P3 | `component/mod.rs`, `button.rs` | union Carbon+M3+SLDS |
| 4-role color pairs (`on-*` slots) | P1 | `component/mod.rs` + `theme.rs` | контраст структурный |
| `ControlSize` enum (Xs/Sm/Md/Lg) | P1 | `component/mod.rs` | size-scale, touch floor |
| `Elevation` enum + shadow | P2 | `paint.rs`, `component/mod.rs` | depth без flat-отказа |
| `Duration` + `Easing` motion tokens | P2 | `anim.rs` | motion-scale, reduced-motion |
| `Shape` enum (7 шагов) | P3 | `component/mod.rs` | magic radii исчезают |
| `Spacing` enum (13 шагов) | P3 | `component/mod.rs` | `lay7_lint.sh` → enum-allowlist |

**Гейты:** `cargo test` зелёный (deprecated aliases держат совместимость);
`lay7_lint.sh` проходит; `ui_layout_lint` 23 состояния зелёные.

### Wave C — Components (10+ новых компонентов)

**Скоуп:** закрыть базовые input/nav/feedback/data-display пробелы.

| Компонент | Приоритет | Зависимости |
|---|---|---|
| `Checkbox` | P1 | Wave T (ControlSize, state-layer) |
| `Switch` (переработать) | P1 | Wave T |
| `Slider` | P1 | Wave T (ControlSize, state-layer) |
| `RadioGroup` | P1 | Wave T, FocusZone (5.5.2) |
| `Tabs` | P1 | Wave T, FocusZone |
| `Accordion` | P2 | Wave T |
| `Progress` (bar + spinner) | P2 | Wave T (Duration) |
| `Skeleton` | P2 | Wave T (anim pulse) |
| `Badge` | P2 | Wave T (BadgeTone) |
| `Avatar` | P3 | Wave C Badge |
| `Tree` | P3 | Wave C, FocusZone |
| `SegmentedControl` | P3 | Wave T |
| `Popover` | P2 | Wave L (anchored) |
| `Snackbar` (с action) | P2 | Wave C Badge |
| `CommandPalette` | P1 | Wave A (ActionRegistry) |

**Гейты:** каждый компонент — TDD (golden-тесты + hit-test); витрина `kit_ui`
обновлена (K4); G4-lint канонические состояния добавлены.

### Wave L — Layout (примитивы)

**Скоуп:** grid-auto, aspect-ratio, sticky, responsive, container-queries.

| Примитив | Приоритет | Зависимости |
|---|---|---|
| `grid_auto` + `Track` (minmax/fr) | P1 | — |
| `aspect_ratio` | P2 | — |
| `sticky_header` | P2 | — |
| `Responsive` + `WindowClass` | P2 | — |
| Container-query (`Density::from_slot`) | P3 | Wave T (Spacing) |

**Гейты:** `html5_demos.rs` golden-эталоны для каждого нового примитива;
`lay_out_with(pilot_backend())` паритет FlexLayoutEngine.

### Wave A — Architecture (Response, focus-trap, action-registry, nav-rail)

**Скоуп:** interaction-эргономика + canvas-UX паттерны.

| Паттерн | Приоритет | Зависимости |
|---|---|---|
| `Response` pattern (egui) | P1 | Wave T (state-layer) |
| `FocusTrap` примитив | P2 | Wave T (Focused) |
| `ActionRegistry` | P1 | — |
| `CommandPalette` (компонент) | P1 | Wave A (ActionRegistry) |
| Figma-style `NavRail` | P2 | Wave C (Tabs) |
| `WidgetExt` modifier trait | P3 | Wave A (Response) |
| Smart guides + distance labels | P2 | — (canvas-app) |

**Гейты:** command palette e2e (открыть → поиск → активировать); `?`
cheatsheet из ActionRegistry; nav-rail в canvas-app.

---

## 7. Per-component anatomy + a11y contract (Carbon-формат)

> Carbon format: для каждого компонента кита — named regions + state map +
> size table + keyboard/focus contract + touch-target guarantee. Это
**контракт-спецификация**, по которой растут новые компоненты.

### 7.1. Button

| Поле | Значение |
|---|---|
| **Anatomy** | `container` + `label` + optional `leading_icon` + optional `trailing_icon` |
| **States** | 7: Normal, Hovered, Focused, Pressed, Dragged, Disabled, Selected (+ Error для toggle-buttons) |
| **Variants** | 7: Primary, Secondary, Tertiary, Ghost, Text, Danger, Inverse |
| **Sizes** | Xs(24) / Sm(30) / Md(36) / Lg(44) |
| **Keyboard** | Enter/Space → activate; Tab → focus; Esc → blur |
| **Focus** | `FocusRing` по слоту `accent`, `focus_visible`-gated |
| **Touch** | hit-rect ≥ `MIN_TOUCH_TARGET` (44) на coarse-pointer |
| **A11y role** | `button`; `aria-pressed` для toggle; `aria-disabled` для Disabled |

### 7.2. Text Field

| Поле | Значение |
|---|---|
| **Anatomy** | `container` + `label` (optional) + `input_text` + `leading_icon` (optional) + `trailing_icon` (optional) + `helper_text` (optional) + `character_counter` (optional) |
| **States** | 7 + Error (invalid value) |
| **Variants** | Filled, Outlined |
| **Sizes** | Xs(24) / Sm(30) / Md(36) / Lg(44) |
| **Keyboard** | typing → input; Enter → commit (FR-061 IN2); Esc → cancel; Tab → focus next |
| **Focus** | caret-blink + `FocusRing` |
| **Touch** | hit-rect ≥ 44 |
| **A11y** | `textbox`; `aria-label` / `aria-labelledby`; `aria-invalid` для Error; `aria-describedby` для helper |

### 7.3. Modal

| Поле | Значение |
|---|---|
| **Anatomy** | `scrim` (backdrop) + `panel` + `header` + `content` + `footer` (actions) |
| **States** | opening (anim) / open / closing (anim) |
| **Sizes** | min/max через `constrain()` |
| **Keyboard** | Esc → close (esc_stack); Tab → `FocusTrap` cycle; Enter на primary action |
| **Focus** | `FocusTrap` inside panel; focus-restore при close |
| **Touch** | scrim tap → close (Block capture); panel hit ≥ 44 |
| **A11y** | `dialog` / `alertdialog`; `aria-modal=true`; `aria-labelledby` header |

### 7.4. Tabs

| Поле | Значение |
|---|---|
| **Anatomy** | `tab_bar` + `tab`(N) + `indicator` (active) + `content` |
| **States** | 7 per tab; active = Selected |
| **Variants** | Underline (M3), Fill (Carbon), Pill (Figma) |
| **Sizes** | bar_h = `ControlSize` |
| **Keyboard** | ←/→ → navigate tabs (FocusZone); Tab → enter content; Enter/Space → activate |
| **Focus** | `FocusRing` on active tab; `focus_visible` |
| **Touch** | tab hit ≥ 44 |
| **A11y** | `tablist` + `tab` (aria-selected); `tabpanel` (aria-labelledby tab) |

### 7.5. Checkbox

| Поле | Значение |
|---|---|
| **Anatomy** | `box` (квадрат) + `check_glyph` (✓/−) + `label` |
| **States** | 7 × 3 (Unchecked/Checked/Indeterminate) |
| **Sizes** | box = `ControlSize` |
| **Keyboard** | Space → toggle; Tab → focus next |
| **Focus** | `FocusRing` на box |
| **Touch** | hit ≥ 44 (box + label) |
| **A11y** | `checkbox`; `aria-checked` = false/true/mixed |

*(полный набор — 19 существующих + 15 новых = 34 компонента — в отдельном
документе `docs/interface-objects/kit-component-contracts.md`, который
является deliverable Wave T/C).*

---

## 8. Открытые вопросы владельцу

1. **`KitState` + `Focused`/`Dragged`** — это breaking change для
   `WidgetState::kit_state()` (возвращает 1 состояние, а их может быть
   несколько active). Миграция: deprecated alias → 1 релиз → удаление.
   Альтернатива: `active_states()` без удаления `kit_state()`. Решение?
2. **4-role color pairs** — расширяет `KitPalette` с ~19 до ~35 слотов. Все 7
   theme-presets (`theme_presets.rs`) нужно дополнить. Готов ли владелец
   обновить пресеты (или авто-derive `on_*` из контраста)?
3. **`ControlSize::Lg = 44`** — поднимает `BUTTON_HEIGHT` с 30 до 44 для
   Lg. Визуальный сдвиг в моделях, где кнопки Sm. Оставить Sm=30 как default,
   Lg=44 как opt-in? Или мигрировать все модали на Lg?
4. **`Elevation` shadow** — добавляет shadow-квады под modals/popovers. В
   flat-canvas эстетике CanvasDesk это может выглядеть чужеродно. Включить
   по умолчанию или за `Elevation::None` (как сейчас)?
5. **`ActionRegistry` + `CommandPalette`** — это большой архитектурный сдвиг
   (все действия через реестр). Делать Wave A сразу или после Wave T+C?
6. **Figma-style `NavRail`** — заменяет ли он текущие точки входа (хоткеи,
   `?`-меню, угловые кнопки)? Или сосуществует?
7. **`Response` pattern** — миграция всех вызовов `kit.button_draw(...)`
   → `let resp = kit.button(...)` — большой рефакторинг. Поэтапно (новые
   компоненты на Response, старые — deprecated) или сразу?
8. **Онбординг/документация** (AGENTS.md обязательный вопрос): Wave T
   меняет состояния/токены — нужны ли шаги тура? Wave C добавляет компоненты
   — обновлять ли `docs/interface-objects/` и user-docs?

---

## 9. Связанные документы

- `docs/dev-researches/rust-ui-library-audit.md` — полный аудит egui/iced/
  slint/gpui/xilem/druid (1 567 строк, top-10 паттернов с сигнатурами).
- `docs/dev-researches/design-systems-research.md` — полный аудит Material 3
  /Fluent 2/Carbon/Apple HIG/Spectrum/SLDS (811 строк, top-15 паттернов).
- `canvas-ui-research-findings.md` (во временной директории) — полный аудит
  Miro/Figma/tldraw/Excalidraw/Whimsical/Eraser.io (412 строк, top-15
  canvas-UX паттернов + open-source architecture takeaways).
- `docs/ui-kit.md` — текущий гайд кита.
- `docs/prd/prd-0009-ui-layering-uikit.md` — архитектура UI/kit (V-4..V-7).
- `docs/adr/adr-0013-taffy-vs-layout-primitives.md` — решение по layout-
  движку (taffy вырезан).
- `docs/adr/adr-0014-taffy-hybrid-layout-backend.md` — hybrid backend.
- `docs/adr/adr-0015-long-term-ui-stack-strategy.md` — long-term стратегия.
- `design/rules/03-spacing-radius.md` — шкалы S1-S6.
- `design/rules/11-layouts.md` — норматив LAY1-LAY11.

---

## 10. Итоговая статистика ревью

- **Прочитано кода кита:** 16 757 строк (19 компонентов + 7 инфра-модулей).
- **Исследовано эталонов:** 6 Rust UI-библиотек + 6 дизайн-систем + 6 canvas-
  продуктов = 18 эталонов.
- **Research-документов:** 3 (2 790 строк суммарно).
- **Находок:** 48 (5 состояний, 6 токенов, 18 компонентов, 5 примитивов, 6
  архитектурных, 8 canvas-UX).
- **P1 (критичных):** 14.
- **Quick-win (только токены, 0 компонентов):** 8.
- **Предложено волн внедрения:** 4 (T/C/L/A).
- **Rust-сигнатур предложено:** 25+ (компоненты, enum'ы, trait'ы).
- **Per-component anatomy контрактов:** 5 (Button, TextField, Modal, Tabs,
  Checkbox) — формат Carbon для всех 34 компонентов = deliverable Wave T/C.
