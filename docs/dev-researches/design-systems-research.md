# Design Systems Research — Contracts, States & Tokens for the canvas-ui Kit Audit

> Research feed for the deep audit of `canvas-ui` (immediate-mode, slot-based,
> cosmic-text-measured, `KitPalette` with ~19–22 semantic slots).
>
> Goal: decide **which component contracts and state patterns to adopt**.
>
> Sources verified live (2026-10) via web search + page fetch:
> m3.material.io, material-web.dev, carbondesignsystem.com, fluent2.microsoft.design,
> developer.apple.com/design, spectrum.adobe.com, lightningdesignsystem.com.
> Where a live page was JS-gated, values are cross-checked against the official
> token reference repos (Material web CSS custom properties, Fluent UI v9 tokens,
> Carbon v11 tokens, Spectrum design-data, SLDS 2 tokens).

---

## 0. Baseline — what the kit currently has (so gaps are explicit)

Read from `crates/canvas-ui/src/component/mod.rs` + `crates/canvas-core/src/theme_presets.rs`:

- **`KitState`** (5): `Normal`, `Hovered`, `Pressed`, `Disabled`, `Selected`.
  → **No `Focused`, no `Dragged`, no `Loading`, no `Error`.** Focus is implied via the `accent` slot, not a state.
- **`ButtonVariant`** (4): `Primary`, `Secondary`, `Ghost`, `Danger`.
  → No `Text`/`Outlined`/`Tonal` low-emphasis variants; no icon-button variant distinction.
- **`KitPalette`** (~19 fields): `panel_fill/border`, `control_fill/border/primary/danger`,
  `hover_fill`, `primary_hover_fill`, `selected_fill`, `text/title/muted`, `disabled_text`,
  `accent`, `control_success/warning`, `stage_dim`, `scrollbar_thumb`, `rule_color`.
  → **No `on-*` foreground pair per container**, no `outline`/`outline-variant`,
  no per-state-layer opacity slots, no elevation/shadow slot, no `focus_ring` slot.
- **`ControlStyle`** (4 props): `fill`, `border`, `text`, `radius`.
  → No elevation, no state-layer, no typography-coupled slot.
- **Metrics are bare constants**: `BUTTON_HEIGHT=30`, `BUTTON_PAD_H=12`,
  `BUTTON_WIDTH=100`, `ICON_BUTTON_SIZE=26`. → **No named size scale (sm/md/lg),
  no density modes, no shape scale tokens** (only a comment "radius-scale chip/card/panel/pill").
- **Components present** (~18): `button`, `panel`, `dropdown`, `modal`, `text_field`,
  `list`, `row`, `chip`, `banner`, `chat_bubble`, `crumbs`, `footer`, `icon`,
  `panel_header`, `radio_card`, `table`, `two_column`, `tooltip`.
  → Missing vs. industry norm: `checkbox`, `switch`, `slider`, `radio` (group),
  `combobox`/`autocomplete`, `date-picker`, `menu` (as distinct from dropdown),
  `tabs`, `accordion`, `pagination`, `popover`, `snackbar`/`toast`, `progress`
  (bar+spinner), `skeleton`, `avatar`, `badge`, `tree`, `data-table` (rich),
  `segmented control`, `command palette`.
- **No motion tokens**, no elevation system, no density system, no breakpoint/window-size-class system.

This is the lens for every “transferable” note below.

---

## 1. Material Design 3 (Material You) — Google

### A. Component Contracts (button as canonical example)

- **Anatomy** — every component is documented as **named regions**: a Button =
  `container` + `label` + optional `icon` (+ leading/trailing). Dialog =
  `container` + `headline` + `icon`(optional) + `supporting text` + `actions`.
  Text field = `container` + `label` + `input text` + `trailing icon` +
  `helper text` + `character counter`. These region names are the public API
  (Material Web exposes them as `::part()`/slots).
- **States** — **7 interaction states**, designed to be **combinable**:
  `hover`, `focus`, `pressed`, `dragged`, `disabled`, `selected`, `error`
  (error is a field-state, not pointer). Material explicitly says states are
  **additive** (hover+focused+selected can stack) and resolved via a **state
  layer**, not by recoloring the container.
- **Variants** (common button) — **5**: `text`, `outlined`, `filled`,
  `elevated`, `filled tonal`. Plus separate FAB family (`FAB`, `small FAB`,
  `large FAB`, `extended FAB`) and `icon button` (`standard`, `filled`,
  `tonal`, `outlined`).
- **Sizes** — M3 does **not** ship sm/md/lg numeric constants; instead a
  **density scale** (see D) and touch-size guidance (≥48dp touch target).
  Icon button touch target 48dp, visual 40dp.
- **Density modes** — `compact … default/comfortable` integer scale from
  **−5 (most compact) to 0 (default)**; each step reduces height by ~4dp and
  tightens internal padding. Applied per-component.
- **Motion** — **durations are named tokens**: `short1..short4` = 50/100/150/200ms,
  `medium1..medium4` = 250/300/350/400ms, `long1..long4` = 450/500/550/600ms.
  **Easings are named tokens**: `standard` `cubic-bezier(0.2,0,0,1)`,
  `emphasized` `(0.2,0,0,1)`, `emphasized-decelerate` `(0.05,0.7,0.1,1)`,
  `emphasized-accelerate` `(0.3,0,0.8,0.15)`, `standard-decelerate` `(0,0,0,1)`,
  `standard-accelerate` `(0.3,0,1,1)`. Hover ≈ short2 (100ms); press ≈ short;
  larger surfaces use medium/long.

### B. Component Inventory (~45)

- **Inputs**: common buttons (5), FAB family (4), icon button (4), checkbox,
  chips (**assist / filter / input / suggestion** — 4 semantic types),
  radio button, slider (continuous+discrete), switch, text field (**filled** +
  **outlined**), date picker, time picker, search.
- **Navigation**: navigation bar (bottom), navigation rail, navigation drawer,
  tabs (primary/secondary), top app bar, bottom app bar, breadcrumbs (via list),
  menu, search.
- **Containers**: card (elevated/filled/outlined), dialog (basic/full-screen),
  bottom sheet, side sheet, snackbar, banner, tooltip, divider, progress
  (**linear** + **circular**, determinate+indeterminate), badge.
- **Data display**: list, image list, data table, carousel, grid view.
- **Feedback**: snackbar, banner, progress indicators, badge, loading.
- **Typography scale** — **5 roles × 3 sizes = 15 styles**: Display, Headline,
  Title, Body, Label, each `large/medium/small`. (M3 Expressive adds
  Display extra-large + numeral scale.) Tokenized as
  `--md-sys-typescale-*`.

### C. State & Interaction Model

- **7 combinable states** (above) resolved through a **state layer overlay** —
  the single most transferable idea in M3.
- **Focus rendering** — Material Web renders a separate `focus-ring` element
  (`::part(focus-ring)`) with `--md-focus-ring-shape` (inherits component
  radius) and an active/inactive scale animation. Focus is **a peer element,
  not a border**, so it never clips or fights the container stroke.
- **Focus management** — dialogs trap focus and return it on close; menus
  restore focus to anchor; radio groups use roving tabindex (one tab stop,
  arrow keys move within).
- **Keyboard** — menus: arrow + home/end + type-ahead + escape; tabs: arrow
  + tab; chips (filter): arrow + space; sliders: arrows + Home/End + PageUp/Down.
- **Touch target** — **48dp minimum** (Android accessibility); visual size may
  be 40dp with an invisible 48dp hit region.
- **Gestures** — swipe-to-dismiss (snackbar/sheet), drag (sliders, reorders),
  long-press, pull-to-refresh (Widget/Material pull-to-refresh), edge swipe.

### D. Theming & Tokens Architecture

- **3-tier model**: **Reference** (`--md-ref-*`, raw palette/shape/type scales) →
  **System** (`--md-sys-color-*`, `--md-sys-typescale-*`, `--md-sys-shape-*`,
  `--md-sys-motion-*`, `--md-sys-elevation-*` — semantic roles) →
  **Component** (`--md-comp-button-*`, per component).
- **Naming**: `--md-{ref|sys|comp}-{domain}-{role}-{state}`. Highly regular and grep-friendly.
- **Color roles** — the **4-role pair system**: every tonal role is a quartet
  `primary / on-primary / primary-container / on-primary-container`. Roles:
  primary, secondary, tertiary, error (each as the 4-tuple); plus surface family
  (`surface / on-surface / surface-variant / on-surface-variant`,
  `surface-container-lowest/low/default/high/highest`), `outline`,
  `outline-variant`, `inverse-*`, `scrim`, `shadow`, `surface-tint`. **5 tonal
  palettes**: primary, secondary, tertiary, neutral, neutral-variant.
- **State layer** — overlay drawn on top of content using a content/on-surface
  color at fixed opacities: **hover 8%, focus 10%, pressed 10%, dragged 16%**.
  This means **state is separable from color** — one slot set serves all states.
- **Elevation** — **6 levels (0–5)** as ambient+key shadow pairs (dp):
  L0 = none; L1 = (1,3); L2 = (3,6); L3 = (6,8); L4 = (6,10); L5 = (8,12).
  Also **tonal elevation**: a same-color container raised on a surface picks up
  `surface-tint` (so dark-mode depth reads without harsh shadows).
- **Motion tokens** — durations + easings above, exposed as
  `--md-sys-motion-duration-*` and `--md-sys-motion-easing-*`.
- **Shape** — **7 named steps**: `none(0)`, `extra-small(4)`, `small(8)`,
  `medium(12)`, `large(16)`, `extra-large(28)`, `full(9999)`. Components map to
  a shape step (`--md-sys-shape-*`).
- **Density** — integer `−5…0` (0 = default/comfortable); negative = compact.
  Changes height + inner padding only; not color/shape.

### E. Accessibility Contracts

- Per-component **ARIA role + state attrs** mandated (e.g. switch = `role="switch"`
  + `aria-checked`; chip filter = `role="checkbox"` + `aria-selected`).
- Contrast — **4.5:1** for body text, **3:1** for large text and for
  non-text UI (state layers, focus rings, icon-only controls) per WCAG.
- `:focus-visible` (not `:focus`) drives the focus ring; ring color must meet 3:1.
- Live regions: snackbars use `aria-live="polite"`; dialogs `aria-modal` +
  labelledby; menus `aria-expanded`/`aria-haspopup`.
- `prefers-reduced-motion` → durations collapse to ~0 (state still changes, just no tween).
- High contrast: Material Web honors `forced-colors: active` (replaces fills
  with `CanvasText`/system colors, doubles selected borders).

### F. Layout & Adaptive Patterns

- **Window size classes** (the adaptive core): width **Compact <600dp**,
  **Medium 600–839dp**, **Expanded 840–1199dp**, Large 1200–1599dp,
  Extra-large ≥1600dp; mirrored height classes. Components choose layout by
  class (e.g. navigation bar → rail → drawer).
- **Container queries** — first-class in Material Web (`md-comp-*` responds to
  container, not just viewport).
- **Grid** — 12-column (Material 2 legacy) + adaptive columns; breakpoints
  align to the size classes.
- **Safe area / notch** — `WindowInsets`/`env(safe-area-inset-*)` honored.
- **Foldables** — Material `DisplayFeature`/hinge APIs; canonical layouts avoid
  the fold seam.

### G. What’s notably GOOD (transferable)

1. **State-layer model** — decouples interaction state from color. One palette
   serves all 7 states via opacity overlays. *The single highest-value adoption
   for our kit.* Our `KitState` has no `Focused`; state layers would give us
   hover/focus/pressed/dragged uniformly across every component with **zero
   new color slots**.
2. **4-role color pair** (`primary / on-primary / primary-container /
   on-primary-container`) — guarantees a foreground exists for every fill.
   Our `KitPalette` has container fills (`palette_row_fill`, `menu_fill`, …)
   but **no paired `on-*` foreground** → contrast is hand-tuned per site.
3. **Focus-ring as a peer element with its own `--md-focus-ring-shape`** — never
   clips against rounded containers, animates scale-in. Our `accent`-border
   approach is fragile.
4. **Named duration/easing tokens** (`short1..long4`, `emphasized-*`) — we have
   **no motion tokens at all**; adopting a 12-step duration + 4 easing set is cheap.
5. **Shape scale of 7 named steps** — our `radius` is a bare float; promoting it
   to `Shape::None|Xs|S|M|L|Xl|Full` kills magic numbers.
6. **Component-anatomy as named regions** (container/label/icon/trailing) → maps
   directly onto our slot-based layout (`Child::fixed` slots).

---

## 2. Fluent Design System 2 — Microsoft

### A. Component Contracts

- **Anatomy** — Fluent documents parts but more loosely than M3; relies on
  **slot APIs** in React (`Button` = root + `icon` + `content`). “Slots” is the
  literal Fluent term for sub-regions.
- **States** — interactive controls carry a **state map** styled via tokens:
  `rest`, `hover`, `pressed`, `disabled`, `focus` (+ `focus-visible`), and
  selection/checked for toggles. ~5–6 states; **not** a formal additive model
  like M3, but Fluent React applies `:hover/:active/:disabled/:focus-visible`
  selectors mapping to per-state color tokens.
- **Variants (Button)** — `appearance`: `primary`, `outline`, `subtle`,
  `transparent` (≈ M3 filled/outlined/text/ghost). Plus `size`: `small`,
  `medium`, `large` (with explicit heights).
- **Sizes** — **3 control sizes with fixed heights**: small=24px, medium=32px,
  large=40px (control heights; tap targets padded to 32/40/48). **4px base unit**.
- **Density** — Fluent has a **“high density”** persona (compact) vs default;
  achieved via the size prop + tighter spacing ramp rather than a global density knob.
- **Motion** — durations as tokens: `durationUltraFast` 50ms, `durationFast` 100ms,
  `durationNormal` 200ms, `durationGentle` 250ms, `durationSlow` 300ms,
  `durationSlower` 450ms(?). Easings: `curveEasyEase`, `curveAccelerateMid`,
  `curveDecelerateMid`, `curveAccelerateMax`, `curveDecelerateMax`,
  `curveEasyEaseMax`.

### B. Component Inventory

- **Inputs**: Button, IconButton, SplitButton, ToggleButton, Checkbox,
  Switch, RadioGroup/Radio, Slider, SpinButton, Textarea, Input, Combobox,
  DatePicker, TimePicker, SearchBox, SpinButton, Rating, Slider, `Slider`,
  **SegmentedControl** (Fluent v9), Chips/Persona.
- **Navigation**: Tab/TabList, Breadcrumb, Nav (drawer/rail equivalents),
  Menu/MenuButton, Toolbar, Pagination.
- **Containers**: Card, Dialog, Popover, Tooltip, TeachingBubble (coachmark),
  MessageBar (≈ banner), Separator, Progress (ProgressIndicator),
  Spinner/Spinner, Skeleton, Scrollbar.
- **Data display**: Table/DataGrid, Tree, List, Persona (avatar+name), Badge,
  Avatar, Empty state patterns.
- **Feedback**: Toast (via `useToast`), MessageBar, Progress, Spinner, Skeleton.
- **Typography** — global type ramp ~10 steps; sizes from `fontSizeBase100` (10px)
  up; tokenized as `fontSize*` + `fontFamily*` + `lineHeight*` + `fontWeight*`.

### C. State & Interaction Model

- ~5–6 states (rest/hover/pressed/disabled/focus/checked).
- **Focus rendering** — Fluent v9 ships a dedicated **`FocusTrap`/`FocusZone`**
  system + a `:focus-visible` outline using `strokeColorFocus`/`colorStrokeFocus2`
  tokens; on Win11 the focus is a **2px accent ring with a 1px gap** (the
  “reveal” focus). Strong, opinionated focus contract.
- **Focus management** — `FocusZone` (directional arrow navigation within a
  region) and `FocusTrap` (modal) are **first-class utilities**, not per-component
  ad-hoc code. This is Fluent’s signature contribution.
- **Keyboard** — Menu/Combobox/Tree all use **Fluent’s keyboard interaction
  model** (arrows + home/end + type-ahead), `Escape` to close, `Tab` to leave.
- **Touch target** — **32px control / 40px recommended touch** (WinUI) — lighter
  than Apple/Material.
- **Gestures** — swipe (nav panes), pull-to-refresh, context menu via long-press
  / right-tap; **light dismiss** (click-outside / Esc) is a formal contract for
  popovers/menus.

### D. Theming & Tokens Architecture

- **3-tier**: **Global** (raw: `colorPaletteRedBackground3`, `borderRadiusLarge`,
  `shadow4`…) → **Alias/Semantic** (`colorNeutralBackground1`, `colorBrandBackground`,
  `colorNeutralForeground1`, `colorNeutralStroke1`…) → **Component**
  (`colorSubtleButtonBackgroundHover`, etc.).
- **Naming**: `color{Palette|Neutral|Brand|Status}{Role}{Weight}` — e.g.
  `colorNeutralBackground1Hover`, `colorBrandForeground2Pressed`. **State is
  encoded in the token suffix** (`Hover/Pressed/Disabled/Focus/Selected`),
  not via a state layer. This is the opposite design choice from M3.
- **Color** — **neutral ramp** (neutralBackground1–5, neutralForeground1–4,
  neutralStroke1–3), **brand ramp** (Brand10…Brand160, ~16 shades), **status**
  (statusSuccess/Warning/Danger/SevereBackground/Foreground…), **communications**
  (presence colors). Dark + light + high-contrast themes are peer citizens.
- **State representation** — Fluent **does not use a state layer**; it instead
  defines **hover/pressed/focus variants of every semantic color token**. More
  tokens, but zero opacity math at render time.
- **Elevation** — **Elevation 2.0**: a **5-step (or 6, depending on alias)
  shadow ramp**: `shadow2, shadow4, shadow8, shadow16, shadow28, shadow64`
  (named by blur px), calibrated for perceptual uniformity. Plus a `shadowBrand`
  tinted shadow.
- **Shape** — `borderRadiusNone(0)/Small(4)/Medium(6)/Large(8)/XLarge(16)/Circular(9999)`.
- **Motion** — durations + curves as tokens (above).
- **Density** — via size + the 4px ramp; no global integer density knob.

### E. Accessibility Contracts

- Every component maps to an **ARIA pattern** from WAI-ARIA APG; Fluent publishes
  the role/aria contract per component.
- Contrast: 4.5:1 text, 3:1 non-text; **Fluent explicitly supports a High
  Contrast theme** (Windows) as a first-class token theme.
- `:focus-visible` ring is mandatory; `FocusZone` gives arrow-key nav.
- Toasts `role="alert"`/`aria-live`; MessageBar `role="alert"`/`status`.
- Reduced motion: `prefers-reduced-motion` collapses token durations.
- High contrast mode is a **peer theme**, not a fallback.

### F. Layout & Adaptive Patterns

- **4px base unit**, global spacing ramp (`spacingHorizontalNone…XXL`).
- Breakpoints: Fluent defines **small (320)/medium (480/600)/large (640–1024)/
  XL/XXL** via CSS `@media` + container queries in v9.
- Grid: 12-column responsive grid; sidebar/content adaptive.
- Safe area / notch handled via `env(safe-area-inset-*)`.
- Foldables: Windows 11 `ApplicationView`/Spanning APIs; Fluent supports
  dual-screen snap states.

### G. What’s notably GOOD (transferable)

1. **`FocusZone` + `FocusTrap` as reusable utilities** — arrow-key navigation
   and modal focus trapping are **infrastructure**, not per-component. Our kit
   has a `KeyboardRouter` + esc-stack; promoting a `FocusZone` concept would
   unify radio/menu/tab/list arrow handling.
2. **State encoded in token suffix** (`…Hover/Pressed/Focus/Selected`) — an
   alternative to M3 state layers; for a retained-slot kit this is very
   tractable: each semantic slot gains `*_hover`/`*_pressed`/`*_focus` siblings.
   (Material’s overlay approach needs blend math; Fluent’s just needs more
   slots — simpler for an immediate-mode painter.)
3. **5–6-step shadow ramp named by blur px** (`shadow2..shadow64`) — a clean,
   small elevation API we currently lack entirely.
4. **3 named control sizes with exact heights (24/32/40)** — our `BUTTON_HEIGHT`
   is a single magic 30; a `Size::Sm/Md/Lg` enum with heights would match
   industry norm.
5. **High-contrast as a peer theme** — tokens-only theming means HC is just
   another preset, exactly our `ThemeColors` preset model.

---

## 3. Carbon Design System — IBM

### A. Component Contracts

- **Anatomy** — Carbon documents components as **“Anatomy” diagrams** listing
  numbered elements (Button = 1 container, 2 label, 3 icon). Specs are
  **pixel-precise tables** on a “Specifications” tab.
- **States** — Carbon’s interactive components use a **fixed 5-state set**:
  `default`, `hover`, `focus` (focus-visible), `active/pressed`, `disabled`
  (+ `selected`, `warning`, `error` for inputs). Fewer combinable states than M3.
- **Variants (Button)** — **5 semantic variants**: `Primary`, `Secondary`,
  `Tertiary`, `Ghost`, `Danger` (≈ our Primary/Secondary/Ghost/Danger + Tertiary).
  Plus `size`: `small (32px)`, `medium (40px default)`, `large (48px)`,
  `x-large (64px)` and `extra-large` fluid. **Danger** is a variant, not just a color.
- **Sizes** — explicit per-component: Button heights 32/40/48/64; field heights
  32/40/48; very table-driven.
- **Density** — Carbon has **“condensed”** (h32) and **“compact”** density modes
  applied at the grid/spacing level; components expose `size` prop separately.
- **Motion** — duration tokens: `duration-fast-01` 70ms, `fast-02` 110ms,
  `moderate-01` 150ms, `moderate-02` 210ms, `slow-01` 300ms, `slow-02` 400ms;
  easings: `standard`, `standard-productive`, `entrance`, `exit`, `expressive`,
  `linear`.

### B. Component Inventory (~45)

- **Inputs**: Button, ButtonSet, Checkbox, Combobox, ContentSwitcher (≈ segmented),
  DatePicker, Dropdown, FileUploader, Form/FormGroup/FormItem, NumberInput,
  RadioButton/RadioButtonGroup, Search, Select, Slider, Toggle, ToggleSmall,
  TextInput, TextArea, Checkbox indeterminate.
- **Navigation**: Tabs, Breadcrumb, Pagination, Menu, MenuButton, Tabs (contained),
  UI shell header / left panel / right panel (app chrome).
- **Containers**: Modal, ComposedModal, ModalWrapper, Popover, Tooltip,
  Toggletip, Tag (≈ chip), Notification (inline/toast), ProgressIndicator
  (stepper), Loading, InlineLoading, Tag (Operational/dismissible/filterable).
- **Data display**: DataTable (rich: sortable/filterable/batch), TableKing,
  TreeView, List, OrderedList, UnorderedList, DefinitionList, StructuredList,
  ContainedList, Tile, ClickableTile, ExpandableTile, CodeSnippet.
- **Feedback**: InlineLoading, Loading (spinner), ProgressBar, ProgressIndicator,
  Notification, SkeletonText, SkeletonIcon.
- **Typography** — type scale with **named semantic tokens**: `caption-01/02`,
  `label-01`, `helper-text-01`, `legal-01`, `body-01/02/compact-01/02`,
  `code-01/02`, `heading-01..heading-08`, `display-01..display-04`. ~20 tokens
  covering a full ramp; sizes on an 8px-ish rhythm.

### C. State & Interaction Model

- 5 base states (+ selected/warning/error).
- **Focus rendering** — Carbon uses a **2px solid `focus` token outline inset
  2px** (box-shadow based, never displaces layout); `:focus-visible` only.
- **Focus management** — Modal traps + returns; Tabs/Menu use roving tabindex;
  Carbon publishes an **accessibility info table per component** (keyboard
  interactions enumerated).
- **Keyboard** — full APG compliance per component, with explicit key tables.
- **Touch target** — Carbon leans 32px control min on desktop; mobile guidance
  pushes to 44px.
- **Gestures** — limited; swipe for mobile tiles; long-press not central.

### D. Theming & Tokens Architecture

- **3-tier**: **Global** (raw, e.g. `$spacing-05`, `$purple-50`, `$interactive-01`)
  → **Contextual/Alias** (semantic, **auto-adapt to a `Layer`** — one/two/three)
  → **Component** (`$button-primary`, `$tag-background-*`).
- **Layering** — Carbon’s signature: a `<Layer>` component sets a depth (0/1/2/3)
  and **contextual tokens recompute** (background, text, border) for that depth.
  Components placed inside a modal (Layer 2) automatically re-skin without
  prop drilling. *Directly relevant to our Popups/Modals layers.*
- **Spacing scale** (verified live from carbondesignsystem.com): multiples of 2/4/8 —
  `$spacing-01`=2px, `02`=4, `03`=8, `04`=12, `05`=16, `06`=24, `07`=32,
  `08`=40, `09`=48, `10`=64, `11`=80, `12`=96, `13`=160.
- **Color** — gray ramp (white, g10, g90, g100 themes) + semantic
  (`interactive-01/02`, `text-01/02/03/04/05/06/07`, `ui-01..05`,
  `ui-background`, `field-01/02`, `container-01..05`, `support-*`).
- **Elevation** — Carbon de-emphasizes shadow; uses **flat color layering**
  (container tokens per Layer) rather than a shadow ramp. (Notably *different*
  from M3/Fluent.)
- **2x Grid** (verified): **16 subcolumns** at the widest breakpoint; breakpoints
  **sm=320 / md=672 / lg=1056 / xlg=1312 / max=1584** px; gutters and margins
  scale per breakpoint; 16-col on lg+ with the “2x” = each col split into 2
  subcolumns for fine alignment.
- **Motion** — tokens above.
- **Shape** — small set of radii (`radius-sm/md/lg`); Carbon favors square-ish
  corners (mostly 0–4px) — deliberately restrained.

### E. Accessibility Contracts

- Carbon publishes an **Accessibility tab per component** with role, keyboard
  table, and focus contract — the most rigorous a11y documentation of the set.
- Contrast 4.5:1/3:1; WCAG AA target.
- `:focus-visible` mandatory; skip-to-content, focus trap in modal, focus return.
- Reduced motion honored via motion tokens.
- High contrast: Carbon’s g100/Gray 90 themes approximate HC; full HC via
  `prefers-contrast`.

### F. Layout & Adaptive Patterns

- **2x Grid**: 16-subcolumn responsive grid, breakpoints above.
- Container queries: Carbon components are container-aware via the Layer model.
- Safe area: handled at the shell level.
- Foldables: not a first-class Carbon concern.

### G. What’s notably GOOD (transferable)

1. **The `Layer` component + contextual tokens** — components auto-re-skin by
   depth. **We already have `UiLayer` (World…Toasts)**; exposing a `Layer` depth
   on our surfaces and making `KitPalette` slots depth-aware would unify the
   repeated `palette_row_fill`/`search_row_fill`/`palette_chip_fill` sprawl into
   one `surface(Layer)` slot set.
2. **Anatomy-as-numbered-spec + pixel-precise Specifications tab** — the
   documentation format to copy for our kit audit: each component = named parts +
   a measurements table.
3. **5 button variants incl. Tertiary + Danger-as-variant + 4 sizes** — closest
   to our existing `ButtonVariant`; adding `Tertiary` and a `Size` enum is a
   minimal, high-value extension.
4. **Verified spacing scale (2/4/8/12/16/24/32/40/48/64/80/96/160)** — a ready-made
   `Spacing` token set to replace scattered `gap` literals.
5. **Per-component a11y table (role + keyboard + focus)** — the contract doc
   shape our audit should produce per component.
6. **Elevation by flat color layering, not shadow** — validates our current
   no-shadow approach for an editor canvas; but we should still define a small
   shadow ramp for Popups/Modals/Tosts (currently a gap).

---

## 4. Apple Human Interface Guidelines

### A. Component Contracts

- **Anatomy** — HIG is **descriptive, not tabular**. Components are documented
  by purpose + best practices rather than a numbered spec table. Less rigorous
  contract *format* than M3/Carbon, but extremely strong platform conventions.
- **States** — UIKit `UIControl.State`: `normal`, `highlighted`, `disabled`,
  `selected`, `focused` (and `application`/`reserved`). **~5 control states**.
  Selection often pairs with highlight.
- **Variants (Button)** — iOS 17+ `ButtonStyle`: `plain`, `gray`, `tinted`,
  `filled` (≈ M3 text/secondary/tonal/filled). Plus `bordered`,
  `borderedProminent`, `borderless`, and icon buttons. macOS: `bordered`,
  `borderless`, `push` (default), `toggle`.
- **Sizes** — HIG uses **Dynamic Type** (text-size-driven) rather than fixed
  sm/md/lg; controls size relative to the text scale. **No numeric size enum.**
- **Density** — driven by Dynamic Type + the platform; no compact/comfortable knob
  (iPad supports a compact split-view density implicitly).
- **Motion** — HIG defines **named animations** (spring, easeInOut, easeInEaseOut)
  and durations (~0.2–0.6s) tied to feel, but tokenized less formally than M3.

### B. Component Inventory

- **Inputs**: Button (plain/gray/tinted/filled + icon), Toggle (switch), Slider,
  Stepper, TextField/SecureField/TextEditor, Picker, DatePicker,
  SegmentedControl, Link, Toggle.
- **Navigation**: TabBar, NavigationBar/Toolbar, Sidebar (macOS/iPad),
  Breadcrumbs (macOS path control), PageControl, NavigationStack,
  SplitView, Search bar.
- **Containers**: Sheet, Form, GroupedList/InsetGroupedList, Alert, Popover,
  Menu/ContextMenu, ActionSheet, Toasts (custom), Card (custom), DisclosureGroup.
- **Data display**: List (plain/inset/grouped), Table (macOS), Grid
  (LazyVGrid), Label, Gauge, ProgressView (linear/circular).
- **Feedback**: Alert, ProgressView, Spinner, Badge, Toast (custom), HUD.
- **Typography** — **semantic text styles**, not px sizes: `largeTitle`,
  `title1/2/3`, `headline`, `body`, `callout`, `subheadline`, `footnote`,
  `caption1/2`. **~11 styles**, scaled by **Dynamic Type** (AX1–AX5 large).

### C. State & Interaction Model

- ~5 control states (normal/highlighted/disabled/selected/focused).
- **Focus rendering** — platform-specific: **macOS focus ring** (blue rounded
  ring, `NSFocusRingType`); **tvOS parallax focus** (the focused element lifts/
  scales); **iPad keyboard focus** (a focus effect on eligible controls); iOS
  touch has **no persistent focus ring** (relies on selection/hover on iPadOS).
- **Focus management** — `FocusState`/`@FocusState` (SwiftUI) for field focus;
  `focusable()`/`focusEffectDisabled()`; tab/arrow navigation on iPad/mac/tvOS.
  Modal sheets present/dismiss with focus handled by the platform.
- **Keyboard** — full keyboard nav on iPadOS/macOS: Tab/Shift-Tab, arrows in
  lists/menus, Escape to close, Space/Enter to activate, ⌘ shortcuts.
- **Touch target** — **44×44 pt minimum hit target** (verified). The canonical
  iOS number.
- **Gestures** — richest of all systems: tap, double-tap, long-press, swipe,
  pan, pinch, edge swipe, pull-to-refresh, **3D Touch / force / context menu**,
  drag-and-drop, hover (iPadOS).

### D. Theming & Tokens Architecture

- **No formal 3-tier token system** (HIG predates W3C tokens). Instead:
  **semantic system colors** + **materials**.
- **Semantic color roles** (adaptive light/dark): `label`, `secondaryLabel`,
  `tertiaryLabel`, `quaternaryLabel`, `systemBackground`,
  `secondarySystemBackground`, `tertiarySystemBackground`,
  `systemFill`, `secondarySystemFill`, `tertiarySystemFill`,
  `quaternarySystemFill`, `separator`, `opaqueSeparator`, `groupedBackground`
  (primary/secondary), `link`, `placeholderText`, plus accent (tint) and
  `systemRed/Orange/Yellow/Green/…`. **The “label × background × fill” trio
  with ordered emphasis tiers (primary/secondary/tertiary/quaternary) is the
  transferable pattern.**
- **Materials / vibrancy** — translucent visual-effect materials
  (`ultraThinMaterial`, `thinMaterial`, `regularMaterial`, `thickMaterial`,
  `chromeMaterial`, `sidebar`, `headerView`, `sheet`, `popover`) that adapt to
  content behind. This is Apple’s answer to elevation.
- **Elevation** — via **materials + depth conventions** (sheets slide up, popovers
  float, sheets have scrim) rather than shadow ramps.
- **Motion** — spring-based, declarative; not tokenized as a ramp.
- **Shape** — `RoundedCornerStyle` `.small/.medium/.large/.largeContinous`;
  platform-consistent radii (continuous/superellipse corners).
- **Density** — via Dynamic Type; compactness via size classes (see F).

### E. Accessibility Contracts

- **VoiceOver** is the reference screen reader; every control has an a11y
  trait + label/hint/value. **Most rigorous SR contract** of the set.
- Contrast: HIG defers to WCAG-like but relies on dynamic type; 4.5:1 expected.
- **Dynamic Type** scales all text per user setting — a11y and density in one.
- Reduce Motion, Reduce Transparency, Increase Contrast, Bold Text,
  Differentiate Without Color, Smart Invert — all first-class toggle contracts.
- Focus: `accessibilityElement`/`accessibilityContainer`; focus movement APIs.

### F. Layout & Adaptive Patterns

- **Size classes** (the adaptive core): **Horizontal/Vertical Regular vs Compact**
  (× iPad/mac multiplies). SwiftUI `@Environment(\.horizontalSizeClass)`.
- **Safe area** — first-class (`safeAreaInset`, `ignoreSafeArea`); notch/home-
  indicator handling is automatic and central.
- **Grid** — `LazyVGrid`/`LazyHGrid` adaptive columns, not a fixed 12-col.
- **Container/adaptive** — `ViewThatFits`, `NavigationSplitView` (auto sidebar→
  stack on compact), `containerRelativeFrame`.
- **Foldables** — Stage Manager / external display; not dual-screen-specific.

### G. What’s notably GOOD (transferable)

1. **Ordered emphasis tiers for text & fills** (`label/secondary/tertiary/
   quaternary` + `systemFill/secondary/tertiary/quaternary`) — a 4-step
   emphasis ladder. Our kit has `text/title/muted/disabled_text` (3 tiers);
   promoting to a **4-tier emphasis scale** (primary/secondary/tertiary/quaternary)
   for both text and fills would systematize the current ad-hoc muted variants.
2. **Materials as the elevation story** — translucency-over-content beats
   shadows for overlay UI; relevant to our Popups/Toasts layers.
3. **44pt touch target as a hard rule** — our `ICON_BUTTON_SIZE=26` and
   `BUTTON_HEIGHT=30` are **below the platform floor**; this is a concrete
   a11y gap to fix (visual can stay small, hit rect must be ≥44).
4. **Size classes (Compact/Regular)** as the adaptive primitive — lighter than
   M3’s 5 window-size classes; a 2-tier compact/regular split may suit our
   canvas shell.
5. **Semantic system colors that are adaptive** — validates our preset/theme
   model but argues for **paired emphasis tiers**, not one-off slots.
6. **`accessibilityLabel/Hint/Value` per element** — even an immediate-mode
   kit can carry an a11y-name per slot for SR export.

---

## 5. Spectrum (Adobe) — BONUS

### A–D. Contracts, Inventory, State, Tokens

- **Anatomy** — Spectrum documents **component anatomy** with named slots
  (e.g. Tag = icon + label + avatar + clear button; ActionButton = icon + label).
- **States** — `default, hover, down/active, focus, disabled, selected,
  hover-selected, focus-selected, key-focus` (Spectrum’s **“key-focus”**
  ring is distinct from mouse focus). ~8 states, **the richest explicit set**.
- **Variants (ActionButton)** — `quiet` (subtle), `emphasis` (filled),
  `staticColor` (white/black on color) + `quiet/primary/secondary`; Button has
  `cta`, `primary`, `secondary`, `negative` (≈ danger).
- **Sizes** — `S (24px), M (32px default), L (40px), XL (52px)`; **Spectrum 2**
  added a T-shirt size scale (XS/S/M/L/XL/2XL/3XL) for spacing/type.
- **Density** — Spectrum defines **scale (medium/large)** for the whole system
  (medium = compact, default for dense pro tools like Creative Cloud).
- **Tokens (3-tier)** — **global** (raw: `gray-50`, `blue-400`, `static-blue`) →
  **semantic/alias** by category (`action-*`, `background-*`, `border-*`,
  `text-*`, `neutral-*`, `static-*`) → **component** (`action-button-*`,
  `tag-label-to-clear-icon-medium`). Verified naming: semantic categories
  group aliases; component tokens are `<component>-<part>-<size>`.
- **Color** — large ramp with **“static” color tokens** (colors that do **not**
  flip in dark mode — for badges/charts) vs adaptive alias tokens.
- **Elevation** — drop-shadow scale (`drop-shadow-xs..2xl`) + Spectrum 2
  emphasizes flat + scrim.
- **Focus** — **`key-focus` ring** (a distinct, always-on for keyboard) using
  `focus-indicator-color` + `focus-indicator-thickness` + `focus-indicator-gap`.

### E. Accessibility

- WCAG AA (4.5:1 text), AAA encouraged; full APG patterns; reduced motion;
  high-contrast via static color tokens.

### G. What’s notably GOOD (transferable)

1. **Distinct `key-focus` state** (keyboard focus vs mouse focus) as a
   first-class state — improves on our single `accent` focus notion and on
   Fluent/M3 by making the distinction explicit in the enum.
2. **“static” color tokens** (colors that don’t adapt to dark mode) — solves
   the “status badge must stay green in both themes” problem cleanly. Our
   `control_success`/`control_warning` are effectively static; Spectrum
   formalizes it.
3. **T-shirt spacing/type scale (XS…3XL)** — a popular, human-readable naming
   that beats numeric `spacing-05`.
4. **Per-category semantic groups** (`action/background/border/text/neutral/
   static`) — a clean organization principle for slot namespaces.

---

## 6. Lightning Design System (Salesforce) — BONUS

### A–D. Contracts, Inventory, State, Tokens

- **Anatomy** — SLDS documents components with **“Anatomy”** + CSS classes;
  heavily class-based (`.slds-button`, `.slds-button_neutral`, …).
- **States** — `.slds-is-hovered`, `.slds-is-active`/`pressed`, `.slds-is-focused`,
  `.slds-is-disabled`, `.slds-is-selected`. ~5 states; **selection** is a
  prominent state (rows, tabs, pills).
- **Variants (Button)** — `neutral`, `brand` (primary), `outline-brand`,
  `destructive`, `text` (link-style), `inverse` (on dark), `success`. **7 variants**.
- **Sizes** — small/default; mostly single default with `—small`.
- **Density** — SLDS has explicit **compact density** classes and a density
  utility (`slds-density--compact`).
- **Tokens (3-tier)** — **global** (`--slds-g-color-brand-base-100`,
  `--slds-g-spacing-xs`, `--slds-g-radius-*`) → **alias/semantic** →
  **component / “styling hooks”** (`--slds-c-button-color-background`,
  `--slds-c-button-color-border`, …). Verified: SLDS uses a strict
   `--slds-{g|c}-<domain>-…` convention; **`g` = global, `c` = component**.
   **Styling hooks** (`--slds-c-*`) are the documented customization surface so
   components adapt to org brand colors.
- **Color** — palettes of ~100 shades each (brand, neutral, success/warning/error,
  accent…); semantic aliases map to them. Dark mode supported.
- **Spacing** — `--slds-g-spacing-xx-small(4)/x-small(8)/small(12)/medium(16)/
  large(24)/x-large(32)/xx-large(48)` (4px-based).
- **Shape** — `--slds-g-radius-*` (small/medium/large/border + rounded/circle).

### E. Accessibility

- SLDS ships **accessibility guidance per component** (role/aria/keyboard);
  focus visible via `slds-has-focus`; 4.5:1 contrast; reduced motion.

### G. What’s notably GOOD (transferable)

1. **`--slds-g-*` (global) vs `--slds-c-*` (component) naming prefix** — a
   trivially grep-able convention separating **system tokens from component
   tokens**. We could adopt `kit_sys_*` vs `kit_comp_*` (or a `System`/`Component`
   enum) to enforce the 3-tier discipline we already half-have.
2. **“Styling hooks” as the per-component override surface** — a named,
   documented set of per-component CSS vars (e.g.
   `--slds-c-button-color-background-hover`). Maps well to our immediate-mode
   `KitPalette` per-component slices.
3. **7 button variants incl. `inverse` (on-color) and `success`** — `inverse`
   is the missing case for buttons over colored headers/stage-dim backdrops.
4. **Explicit compact density utility** — validates adding a `Density::Compact`
   that tightens spacing rather than inventing a knob.

---

## 7. Comparison Matrix

| Dimension | Material 3 | Fluent 2 | Carbon | Apple HIG | Spectrum | SLDS |
|---|---|---|---|---|---|---|
| **Component count** | ~45 | ~50 | ~45 | ~40 | ~55 | ~60 |
| **Interactive states** | **7** (additive, state-layer) | 5–6 (token-suffix) | 5 (+sel/warn/err) | ~5 | **8** (key-focus distinct) | 5–6 |
| **Button variants** | 5 (text/outlined/filled/elevated/tonal) | 4 (primary/outline/subtle/transparent) | 5 (primary/sec/tertiary/ghost/danger) | 4 (plain/gray/tinted/filled) | 4 (cta/primary/secondary/negative) | 7 (neutral/brand/outline-brand/destructive/text/inverse/success) |
| **Button sizes** | density-driven (no sm/md/lg) | 3 (24/32/40) | 4 (32/40/48/64) | Dynamic Type | 4 (S24/M32/L40/XL52) | 2 (small/default) |
| **Token layers** | 3 (ref/sys/comp) | 3 (global/alias/comp) | 3 (global/contextual/comp) | 2 (semantic+materials) | 3 (global/semantic/comp) | 3 (global/alias/component-hook) |
| **State representation** | **State layer (opacity overlay)** | Token suffix (`…Hover/Pressed`) | token + per-state values | UIControl states | key-focus token | token suffix + `is-*` classes |
| **Color role model** | **4-role pair** (role/on/role-container/on-container) | neutral+brand+ramp+status | gray ramp + Layer contextual | label×background×fill (4 emphasis tiers) | category groups + static | palette100 + semantic alias |
| **Elevation** | **6 levels** (ambient+key dp) + tonal | **5–6 shadow ramp** (2/4/8/16/28/64) | flat Layer color (no shadow ramp) | materials/vibrancy | drop-shadow xs..2xl | shadow tokens (small/medium/large) |
| **Shape scale** | 7 steps (none…full) | 6 (none/sm/med/lg/xl/circle) | 3 (sm/md/lg) restrained | 4 (sm/md/large/largeContinuous) | T-shirt | radius tokens |
| **Spacing** | 4/8 rhythm | **4px base ramp** | 2/4/8…160 (13 steps) | 4/8 rhythm | T-shirt scale | 4px ramp (xxs…xxl) |
| **Motion tokens** | durations (short1..long4) + 6 easings | durations + 6 curves | 6 durations + 6 easings | spring/declarative | duration/easing tokens | duration/easing tokens |
| **Density modes** | integer −5…0 | high-density persona | condensed/compact | Dynamic Type | medium/large scale | compact utility |
| **Focus model** | peer `focus-ring` element + `:focus-visible` | `:focus-visible` + **FocusZone/FocusTrap** | 2px inset outline | platform focus ring / parallax | **`key-focus`** (kbd≠mouse) | `slds-has-focus` |
| **Touch target min** | 48dp | 32/40px | 32px desktop, 44 mobile | **44pt** | — | — |
| **Adaptive primitive** | 5 window-size classes + container queries | 4px + breakpoints | **2x Grid 16-subcol**, 5 breakpoints | **size classes** (Compact/Regular) | scale (med/large) | responsive utilities |
| **A11y rigor** | high (APG, contrast, reduced-motion, HC) | high (HC as peer theme) | **highest** (per-component a11y table) | highest (VoiceOver, Dynamic Type, many toggles) | high (WCAG AA/AAA) | high (per-component a11y) |
| **High-contrast** | `forced-colors` | **peer theme** | gray-100 theme | Increase Contrast toggle | static tokens | — |
| **Foldable/dual-screen** | DisplayFeature APIs | Win11 spanning | not first-class | Stage Manager | — | — |

---

## 8. Transferable Patterns — Top 15 to adopt in canvas-ui

Ranked by value-to-effort for an **immediate-mode, slot-based, cosmic-text kit
with a `KitPalette` and `UiLayer` surfaces**.

1. **Material 3 state-layer model.** Add `Focused` + `Dragged` to `KitState`
   (→ 7 states) and resolve hover/focus/pressed/dragged via **opacity overlays
   of the existing content/accent color** (8/10/10/16%). Zero new color slots;
   one uniform mechanism across every component. *(Closes our biggest gap: no
   Focused state.)*

2. **4-role color pairs (`fill / on-fill / fill-container / on-fill-container`).**
   For every container slot (`menu_fill`, `palette_row_fill`, `card_fill`…)
   mandate a paired **foreground** slot. Today contrast is hand-tuned per site;
   this makes it structural. Adopt Material’s pair discipline without the full
   5-palette system.

3. **Per-component “styling hooks” (SLDS `--slds-c-*`) as the override surface.**
   Expose a named, documented per-component slice (`button.fill`,
   `button.fill_hover`, `button.border`, `button.text`) instead of the current
   flat `ControlStyle{fill,border,text,radius}`. Pairs with #1/#2.

4. **`FocusZone` + `FocusTrap` as infrastructure (Fluent).** Our `KeyboardRouter`
   + esc-stack already half-do this; promote arrow-key navigation (radio/menu/
   tab/list/tree) and modal focus-trap+return into reusable primitives, not
   per-surface code. Add a peer **focus-ring element** (Material Web style) with
   its own radius token so it never clips rounded containers.

5. **Distinct `key-focus` state (Spectrum).** Encode keyboard-focus ≠ mouse-focus
   in the enum (or a `focus_visible` flag) so the focus ring only paints for
   keyboard users — matches `:focus-visible` semantics and avoids noisy mouse
   focus.

6. **Named size scale: `Sm/Md/Lg` with exact heights (Fluent 24/32/40 or Carbon
   32/40/48).** Replace `BUTTON_HEIGHT=30` / `ICON_BUTTON_SIZE=26` magic numbers
   with `Size::Sm|Md|Lg` applied to button/chip/field/icon-button. Lifts
   `ICON_BUTTON_SIZE=26` above the **44pt touch floor** via a separate hit-rect
   (visual 26–32, hit ≥44) — fixes a real a11y gap.

7. **`Layer` depth + contextual tokens (Carbon).** We already have `UiLayer`
   (World…Toasts). Add a `Layer(0..3)` to surfaces and make container/text/border
   slots **depth-aware**; collapses the `palette_row_fill` /
   `search_row_fill` / `palette_chip_fill` sprawl into one `surface(Layer)` family.

8. **Elevation ramp token (Fluent `shadow2/4/8/16/28/64` or M3 L0–L5).** We have
   **no shadow system**; overlays (Popups/Modals/Toasts) currently rely on
   `stage_dim` scrim only. A 5–6 step shadow ramp + tonal elevation (M3
   `surface-tint`) gives depth without abandoning our flat-canvas aesthetic.

9. **Motion token set (Material 12 durations + 4–6 easings).** We have **no
   motion tokens**. Adopt `Duration::{Short1..Long4}` (50…600ms) +
   `Easing::{Standard, EmphasizedDecelerate, EmphasizedAccelerate}`. Honor
   `prefers-reduced-motion` by collapsing durations to 0.

10. **Shape scale enum of 7 named steps (Material).** Replace bare `radius: f32`
    with `Shape::{None,Xs,S,M,L,Xl,Full}` → px. Removes magic radii; lets every
    component declare its shape step.

11. **Spacing scale of 13 steps (Carbon, verified): 2/4/8/12/16/24/32/40/48/64/
    80/96/160.** Replace scattered `gap` literals with a `Spacing` enum. 2/4/8
    base matches our existing `SPACING_*` constants.

12. **4-tier emphasis ladder for text & fills (Apple): primary/secondary/
    tertiary/quaternary.** Promote `text/title/muted/disabled_text` → a
    `TextEmphasis::{Primary,Secondary,Tertiary,Quaternary}` and a parallel
    `FillEmphasis`. Systematizes the ad-hoc muted variants.

13. **`ButtonVariant` extension: add `Tertiary` + `Text`/`Outlined` low-emphasis
    + `Inverse` (on-color) + keep `Danger`.** Converges to ~6 variants
    (Primary/Secondary/Tertiary/Ghost/Text/Danger + Inverse) matching the union
    of Carbon/M3/SLDS. Low cost, high expressive coverage.

14. **Window-size-class adaptive primitive (Material 5 / Apple Compact·Regular
    compromise).** Add a 3-tier `WindowClass::{Compact,Medium,Expanded}` (width
    <600/<840/≥840 dp) that drives navigation chrome (rail vs drawer) and
    density. Lighter than Material’s 5, richer than Apple’s 2.

15. **Per-component anatomy + a11y table as the audit deliverable (Carbon format).**
    For each kit component, document: **named regions** (maps to our
    slot-children), **state map** (which of the 7 states apply + how the state
    layer resolves), **size table**, **role + keyboard + focus contract**,
    **touch-target guarantee**. This is the contract doc shape the audit should
    produce — and it doubles as the spec to grow missing components
    (checkbox/switch/slider/radio/tabs/accordion/pagination/popover/snackbar/
    progress/skeleton/avatar/badge/tree/datatable/segmented/command-palette).

### Quick-win subset (do first, ~half the work)

- **#1 state layers + `Focused`/`Dragged` states**, **#2 on-fill pairs**,
  **#6 `Size` scale + 44pt hit rects**, **#9 motion tokens**, **#10 shape
  enum**, **#11 spacing enum**. These six are token/state-level changes that
  touch `KitState`, `KitPalette`, `ControlStyle` and the metrics constants —
  exactly the surface the audit targets — and require no new components.

---

## Appendix — verified source URLs (2026-10)

- M3 buttons/states/color/motion: m3.material.io/components/buttons,
  m3.material.io/foundations/interaction-states, m3.material.io/styles/color,
  m3.material.io/styles/motion/tokens-specs; material-web.dev/components/button
  (CSS custom-property token names confirmed: `--md-sys-color-*`,
  `--md-focus-ring-shape`, `--catalog-shape-*`).
- Carbon: carbondesignsystem.com/components/button/usage,
  /guidelines/spacing/overview (spacing scale table verified),
  /guidelines/2x-grid/overview (max breakpoint 1584px, 16-subcolumn verified),
  /guidelines/motion, /guidelines/color (Layer/contextual tokens verified).
- Fluent 2: fluent2.microsoft.design Layout (4px base unit verified),
  Elevation 2.0 (5-step shadow ramp verified), react.fluentui.dev tokens,
  FocusZone/FocusTrap docs.
- Apple HIG: developer.apple.com/design/human-interface-guidelines/buttons
  (component taxonomy verified), accessibility pages, 44×44pt hit target verified.
- Spectrum: spectrum.adobe.com + opensource.adobe.com/spectrum-design-data
  (semantic categories + component token naming `tag-label-to-clear-icon-medium`
  verified).
- SLDS: lightningdesignsystem.com Color, developer.salesforce.com SLDS design
  tokens (`--slds-g-*` / `--slds-c-*` “styling hooks” convention verified).
