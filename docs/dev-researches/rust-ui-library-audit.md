# Rust UI Library Research Audit

> **Purpose**: Deep research into six Rust UI libraries to inform a canvas-ui kit audit.
> The audit will propose new components and patterns for an immediate-mode Rust kit
> with cosmic-text measurement and slot-based theming.
>
> **Date**: 2025-01
> **Sources**: Official docs, docs.rs API listings, GitHub source code, widget gallery source

---

## Table of Contents

1. [egui](#1-egui)
2. [iced](#2-iced)
3. [slint](#3-slint)
4. [gpui](#4-gpui)
5. [xilem](#6-xilem)
6. [druid](#6-druid)
7. [Comparison Table](#comparison-table)
8. [Top 10 Patterns to Steal](#top-10-patterns-to-steal)

---

## 1. egui

**Repository**: https://github.com/emilk/egui
**Latest version**: 0.36.2
**License**: MIT OR Apache-2.0
**Docs**: https://docs.rs/egui

### A. Architecture & Component Model

**Paradigm**: Pure immediate mode (multi-pass capable).

egui is a library, not a framework. You call into it every frame. There are no callbacks,
no event handlers to register, no widget references to store.

- **Component definition**: Widgets are functions/methods on `&mut Ui`. A widget is any type
  implementing `egui::Widget` (a single `fn ui(self, ui: &mut Ui) -> Response` method).
  Custom widgets are typically just functions: `fn toggle_switch(ui: &mut Ui, on: &mut bool) -> Response`.
- **State management**: egui retains minimal state in `Memory`, keyed by `Id` (hashed from
  position in the call stack + user-provided seeds). The application owns all meaningful state
  in plain Rust variables. egui stores things like scroll offsets, window positions, widget
  interaction state (which slider is being dragged).
- **Props/children**: There is no props system. Children are passed as closures:
  `ui.horizontal(|ui| { ui.label("..."); ui.button("..."); })`. The closure receives a
  fresh `&mut Ui` with a clipped/positioned cursor.
- **Composition pattern**: Builder + closure nesting. `ui.add(Widget::new(...))` or the
  shorthand `ui.button("text")`. Containers like `ScrollArea::vertical().show(ui, |ui| { ... })`
  return a `ScrollAreaOutput` containing the inner response.

**Key types**:
```
Context          // central handle, holds Memory, Style, Fonts
Ui               // per-region builder + cursor
Response         // returned by every widget call
Id               // hashed identity for retained state
UiBuilder        // configures a Ui scope (disabled, invisible, etc.)
Widget           // trait: fn ui(self, &mut Ui) -> Response
```

### B. Layout System

**Engine**: Custom — no taffy, no cassowary. Layout is sequential cursor-based.

- **Primitives**: `ui.horizontal(|ui| ...)`, `ui.vertical(|ui| ...)`,
  `ui.horizontal_wrapped(|ui| ...)` (auto-wrapping), `ui.columns(n, |ui, i| ...)`,
  `egui::Grid::new(id).num_columns(2).spacing([40.0, 4.0]).striped(true).show(ui, |ui| ...)`.
- **Constraints**: No min/max/flex-grow. The cursor advances rightward (horizontal) or
  downward (vertical). Widgets report their desired size via `fn desired_size(&self) -> Vec2`.
  `ui.allocate(size)` / `ui.allocate_with_layout(size, layout)` for explicit sizing.
- **Text measurement**: Custom text layout engine (not cosmic-text). Uses `epaint::text::FontImage`
  and `emath` for measurement. `Context::fonts()` provides `Fonts::layout()` / `layout_no_wrap()`.
- **Overflow**: `ScrollArea` handles overflow with scrollbars. `ui.clip_rect()` for clipping.
  No automatic overflow policy beyond scroll.
- **Responsive**: No built-in responsive breakpoints. `ui.available_width()` / `available_size()`
  for manual adaptation. `ui.horizontal_wrapped` adapts to width.
- **Frame-delay problem**: For auto-sized windows/areas, egui uses the previous frame's size
  to position, causing a 1-frame delay. Mitigated by `Context::request_discard()` for rare
  multi-pass rendering (most frames are single-pass).

### C. Component Inventory

**Inputs** (from widget gallery source `widget_gallery.rs`):
| Component | API |
|-----------|-----|
| Button | `ui.button("text")` → `Response` |
| Link button | `ui.link("text")` → `Response` |
| Checkbox | `ui.checkbox(&mut bool, "label")` |
| Radio button | `ui.radio_value(&mut val, Enum::X, "label")` |
| SelectableLabel | `ui.selectable_value(&mut val, Enum::X, "label")` |
| ComboBox | `egui::ComboBox::from_label("...").selected_text("...").show_ui(ui, \|ui\| ...)` |
| Slider | `egui::Slider::new(&mut f32, 0.0..=360.0).suffix("°")` |
| RangeSlider | `egui::RangeSlider::new(&mut min, &mut max, 0.0..=360.0)` |
| DragValue | `egui::DragValue::new(&mut f32).speed(1.0)` |
| TextEdit | `ui.text_edit_singleline(&mut String)` / `TextEdit::multiline(...)` |
| Color picker | `ui.color_edit_button_srgba(&mut [u8; 4])` |
| DatePicker | `egui_extras::DatePickerButton::new(&mut date)` |
| Spinner | `egui::Spinner::new()` |
| Hyperlink | `ui.hyperlink_to("text", "url")` |
| Image | `ui.add(egui::Image::new(source))` |
| Button w/ image | `ui.button((icon, "text"))` |

**Containers** (from `egui::containers` module):
| Component | API |
|-----------|-----|
| Window | `egui::Window::new("title").show(ctx, \|ui\| ...)` |
| CentralPanel | `egui::CentralPanel::default().show(ctx, \|ui\| ...)` |
| TopBottomPanel | `egui::TopBottomPanel::top("id").show(ctx, \|ui\| ...)` |
| SidePanel | `egui::SidePanel::left("id").show(ctx, \|ui\| ...)` |
| Area | `egui::Area::new(id).show(ctx, \|ui\| ...)` (floating) |
| Frame | `egui::Frame::group(ui.style()).show(ui, \|ui\| ...)` |
| CollapsingHeader | `ui.collapsing("title", \|ui\| ...)` |
| ScrollArea | `egui::ScrollArea::vertical().show(ui, \|ui\| ...)` |
| Grid | `egui::Grid::new(id).show(ui, \|ui\| ...)` |
| Menu | `egui::menu::bar(ui, \|ui\| ...)` / `egui::menu::button(ui, "File", \|ui\| ...)` |
| Combo (popup) | `egui::Combo::new(...)` |

**Feedback**:
| Component | API |
|-----------|-----|
| ProgressBar | `egui::ProgressBar::new(0.5).show_percentage().animate(bool)` |
| Separator | `ui.separator()` |
| Tooltip | `.on_hover_text("text")` / `.on_hover_ui(\|ui\| ...)` on any `Response` |

**Typography**:
| Component | API |
|-----------|-----|
| Label | `ui.label("text")` / `ui.heading("text")` / `ui.monospace("text")` |
| Code label | `ui.code("text")` |

### D. State & Interaction Patterns

- **Hover/focus/pressed/disabled**: Every widget returns a `Response` struct:
  ```rust
  pub struct Response {
      pub ctx: Context,
      pub id: Id,
      pub rect: Rect,
      pub sense: Sense,        // what interactions were tracked
      // Interaction state:
      pub hovered: bool,
      pub clicked: bool,        // [resp.clicked()]
      pub clicked_by: Option<PointerButton>,
      pub double_clicked: bool,
      pub dragged: bool,
      pub drag_delta: Vec2,
      pub is_pointer_button_down_on: bool,
      pub changed: bool,        // value changed this frame
      pub active: bool,         // has keyboard focus or is being interacted with
      pub has_focus: bool,
      pub lost_focus: bool,
      pub gained_focus: bool,
  }
  ```
- **Disabled**: `UiBuilder::new().disabled()` → `ui.scope_builder(builder, |ui| ...)`.
  All widgets in scope render as disabled.
- **Invisible**: `UiBuilder::new().invisible()`.
- **Opacity**: `ui.multiply_opacity(0.5)`.
- **Focus management**: `Response::request_focus()`, `Response::surrender_focus()`.
  `Memory::has_focus(id)`. No focus-trap or roving tabindex built-in.
- **Keyboard**: `ui.input(|i| i.key_pressed(Key::Escape))`. No built-in arrow-key navigation
  for composite widgets.
- **Animations**: `Style::animation_time` controls animation duration.
  `Response::animate_value_to(value, target, speed)`. `egui::animation::easing`.
  Built-in but limited.
- **Gestures**: `Response::dragged()`, `Response::drag_delta()`. `ui.input(|i| i.multi_touch())`
  for pinch-zoom. No built-in swipe.
- **Accessibility**: AccessKit integration. `Response::labelled_by(other_id)` associates labels.
  Widget IDs are automatically mapped to accessibility nodes.

### E. Theming & Tokens

**How themes are defined**: Runtime via `Style` struct.

```rust
pub struct Style {
    pub spacing: Spacing,       // item_spacing, indent, button_padding, etc.
    pub visuals: Visuals,       // widget colors, window_bg, etc.
    pub text_styles: TextStyles, // mapping of Heading/Body/Monospace → FontId + size
    pub animation_time: f32,
    pub interaction: InteractionSettings,
    pub debug: bool,
    pub explanation: bool,
}
```

- **Color system**: `Color32` (sRGBA). `Visuals` has `widgets` (WidgetVisuals per state:
  noninteractive, inactive, hovered, active, open). Semantic-ish but mostly primitive palette.
- **Spacing**: `Spacing` struct with `item_spacing: Vec2`, `indent`, `button_padding`,
  `button_inner_spacing`, `checkbox_spacing`, `slider_width`, `combo_width`, `text_spacing`,
  `menu_spacing`, `tooltip_width`, `indent_ends`, `combo_height`, `scroll_bar_width`, etc.
- **Dark/light**: `Theme::Light` / `Theme::Dark` enum. `ctx::set_theme(Theme::Dark)`.
  `ctx::theme()`. Toggle at runtime.
- **Fonts**: `Context::set_fonts(FontDefinitions)` for custom fonts. `text_styles` map
  TextStyle → (FontId, size).
- **No density modes** (compact/comfortable).

### F. What's notably GOOD (patterns to steal)

1. **`Response` pattern**: Every widget call returns a `Response` with full interaction state.
   Eliminates callbacks entirely. `if ui.button("Save").clicked() { save(); }`.
   **Transferable to canvas-ui**: each widget draw call returns a response struct.

2. **`on_hover_text` / `on_hover_ui` method chaining**: Attach tooltips to any widget:
   `ui.button("x").on_hover_text("Help text")` or `ui.button("x").on_hover_ui(|ui| { ... })`.
   Elegant, composable, zero boilerplate.

3. **`labelled_by` for a11y**: `let label = ui.label("Name:"); ui.add(widget).labelled_by(label.id)`.
   Associates a label widget with an input for screen readers. Clean immediate-mode a11y.

4. **`UiBuilder` for scoped state**: `UiBuilder::new().disabled()` creates a child `Ui` where
   all widgets are disabled. Also supports `.invisible()`. Scopes interaction state without
   threading flags through every call.

5. **`egui_kittest` snapshot testing**: `Harness::builder().with_theme(Theme::Dark).build_ui(|ui| { ... })`
   then `harness.snapshot("name")`. Visual regression testing for immediate mode. Runs across
   multiple `pixels_per_point` and themes.

6. **Auto-generated IDs**: Widgets auto-generate IDs from their position in the call stack.
   Only windows/scroll-areas need explicit IDs. No manual ID management for buttons.

7. **`multiply_opacity`**: `ui.multiply_opacity(0.5)` cascades opacity to all children.
   Simple, powerful for fade animations.

8. **Grid layout with `.striped(true)`**: `egui::Grid::new(id).striped(true).show(ui, ...)` —
   alternating row backgrounds for readability. Small but nice ergonomics.

### G. What's notably BAD (anti-patterns to avoid)

1. **Frame-delay layout**: Fundamental immediate mode limitation — window/area positioning
   uses last frame's size. Causes first-frame jitter. Multi-pass (`request_discard`) is
   expensive and rarely used.

2. **No flexbox/grid layout engine**: Layout is sequential cursor-based. No flex-grow,
  no CSS Grid, no proper constraint system. Complex layouts require manual workarounds.

3. **CPU usage with large UIs**: Full layout every frame. Long scroll areas are slow because
   all content is laid out each frame (no virtualization built-in).

4. **ID collision footgun**: When using dynamic widgets in loops, ID generation can collide.
   `ui.push_id(seed, |ui| ...)` is needed to disambiguate. Easy to forget.

5. **No CSS-like theming**: `Style` struct is a monolith. No cascade, no inheritance beyond
   `Visuals`. Overriding one widget's style requires modifying global `Style`.

6. **Custom text layout (not cosmic-text)**: egui's text layout is basic. No complex text
   shaping, no proper bidirectional text, no advanced font metrics.

---

## 2. iced

**Repository**: https://github.com/iced-rs/iced
**Latest version**: 0.14+
**License**: MIT
**Docs**: https://docs.rs/iced, https://book.iced.rs

### A. Architecture & Component Model

**Paradigm**: Retained, Elm-inspired reactive architecture.

iced splits UI into four concepts (The Elm Architecture):
- **State** — application data (arbitrary struct)
- **Messages** — enum of user interactions / events
- **Update logic** — `fn update(&mut state, Message)` — pure mutation
- **View logic** — `fn view(&self) -> impl Widget<Message>` — declarative tree

```rust
#[derive(Default)]
struct Counter { value: i32 }

#[derive(Debug, Clone, Copy)]
enum Message { Increment, Decrement }

impl Counter {
    fn view(&self) -> impl Widget<Message> {
        column![
            button("+").on_press(Message::Increment),
            text(self.value).size(50),
            button("-").on_press(Message::Decrement),
        ]
    }
    fn update(&mut self, msg: Message) {
        match msg {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
    }
}

fn main() -> iced::Result { iced::run(Counter::update, Counter::view) }
```

- **Component definition**: `Widget<Message>` trait. Built-in widgets are constructed via
  functions: `button("text")`, `text("hello")`, `column![...]`, `row![...]`. Custom widgets
  implement `Widget` trait with `fn layout()`, `fn draw()`, `fn update()`, etc.
- **State management**: Application state is a plain struct. Messages flow through `update()`.
  Per-widget state (e.g., `text_input::State`, `scrollable::State`) is stored in the app struct.
  Async via `Task` / `Subscription` (futures-based).
- **Props/children**: Widgets take props as function arguments. Children via macros:
  `column![widget_a, widget_b, widget_c]` or `column((0..n).map(|i| text(i).into()))`.
- **Composition pattern**: Functional + macro. `column![]` / `row![]` are vec-producing macros.
  `.on_press(Message)` attaches events. `button("text").on_press(Msg).style(theme::Button::Primary)`.

### B. Layout System

**Engine**: Custom layout engine in `iced_core::layout` (not taffy).

- **Primitives**: `column`, `row`, `container`, `grid`, `stack`, `scrollable`, `pane_grid`.
- **Constraints**: `Length` enum:
  ```rust
  pub enum Length {
      Shrink,          // content-sized
      Fill,            // take all available space
      FillPortion(u16), // fill proportional share
      Fixed(f32),      // exact pixels
  }
  ```
  `column![].width(Length::Fill).height(Length::Shrink)`.
  `Alignment::Start` / `Center` / `End` for main and cross axes.
- **Text measurement**: Custom text layout via `cosmic-text` (in recent versions) or
  `iced_graphics` text pipeline.
- **Overflow**: `Scrollable` widget with configurable scrollbar policies.
  `scrollable(ScrollDirection::Vertical)`.
- **Responsive**: `Responsive` widget receives available size:
  ```rust
  Responsive::new(|size| {
      if size.width > 600.0 { wide_layout() } else { narrow_layout() }
  })
  ```
  `Container::centered()` for centering.

### C. Component Inventory

Full widget list from `iced::widget` module (docs.rs):

**Inputs**:
| Component | Module |
|-----------|--------|
| Button | `button` |
| Checkbox | `checkbox` |
| ComboBox | `combo_box` |
| PickList (dropdown) | `pick_list` |
| Radio | `radio` |
| Slider | `slider` |
| VerticalSlider | `vertical_slider` |
| Toggler (switch) | `toggler` |
| TextInput | `text_input` |
| TextEditor (multiline) | `text_editor` |
| MouseArea | `mouse_area` |

**Navigation**:
| Component | Module |
|-----------|--------|
| Tab-like | (via custom) |
| PaneGrid (split panes) | `pane_grid` |

**Containers**:
| Component | Module |
|-----------|--------|
| Column | `column` |
| Row | `row` |
| Container | `container` |
| Grid | `grid` |
| Stack | `stack` |
| Scrollable | `scrollable` |
| Float | `float` (floating overlay) |
| Tooltip | `tooltip` |
| Pin | `pin` (pin to position) |
| Themer | `themer` (scoped theme) |
| Keyed | `keyed` (reconciliation hint) |
| Responsive | `responsive` (adaptive) |
| Space | `space` |
| Rule (divider) | `rule` |

**Data display**:
| Component | Module |
|-----------|--------|
| Text | `text` |
| Markdown | `markdown` |
| Image | `image` |
| Svg | `svg` |
| Canvas | `canvas` |
| Shader | `shader` |
| QRCode | `qr_code` |

**Feedback**:
| Component | Module |
|-----------|--------|
| ProgressBar | `progress_bar` |
| Sensor | `sensor` |

### D. State & Interaction Patterns

- **Hover/focus/pressed**: `button::State` tracks interaction. `button("x").on_press(Msg)`.
  `button("x").on_press(Msg).style(theme::Button::Primary)`. State structs per widget type.
- **Disabled**: `button("x").on_press_maybe(None)` (no message = disabled).
  Or `.on_press_maybe(is_enabled.then_some(Msg))`.
- **Focus**: `text_input::State::focus()`. `TextInput::new(&mut state, "placeholder", &value)`.
  No built-in focus-trap.
- **Keyboard**: `keyboard::on_key_press(|key, modifiers| Some(Msg))`. `Subscription` for
  global key events.
- **Animations**: `iced::animation` module. `animation::Builder` for transitions.
  `Subscription::run` for animation frames. Built-in but requires manual orchestration.
- **Gestures**: `mouse_area::MouseArea` wraps any widget for mouse events (press, release,
  move, enter, exit, scroll). No built-in drag/pinch.
- **Accessibility**: AccessKit integration. Each widget has accessibility properties.

### E. Theming & Tokens

**How themes are defined**: Runtime `Theme` enum + per-widget `Catalog` traits.

```rust
pub enum Theme {
    Light,     // built-in light variant
    Dark,      // built-in dark variant
    Custom(Palette),  // custom palette
}
```

- **Color system**: `Palette` with semantic slots (background, foreground, primary, secondary,
  success, danger, warning). `Theme::Mode` for brightness.
- **Per-widget styling**: Each widget type has a `Catalog` trait (e.g., `button::Catalog`)
  that maps `Theme` → `button::Style`. `button("x").style(my_style_fn)`.
  `Themer` widget wraps children with a scoped theme override.
- **Spacing**: `Padding` struct (`Padding::new(16.0)` or `Padding::ZERO`).
  `border::Radius`, `border::Width`. No global spacing scale enum.
- **Dark/light**: `Theme::Dark` / `Theme::Light`. Runtime switching.
  `iced::application::Application::theme()` overrides per-view.
- **No density modes**.

### F. What's notably GOOD (patterns to steal)

1. **`PaneGrid` for resizable split panes**: `PaneGrid::new(state, |pane, state, is_maximized| { ... })`
   with built-in drag-to-resize handles. `PaneGrid::split`, `PaneGrid::resize`. Solves a
   common complex UI need out of the box.

2. **`Themer` widget for scoped theme overrides**: `Themer::new(child, |theme| theme.with_palette(...))`.
   Wraps a subtree with a modified theme. Clean way to scope dark/light or accent overrides.

3. **`Responsive` widget**: `Responsive::new(|size: Size| { ... })` — receives available
   dimensions, enabling adaptive layouts. Simple, powerful breakpoint pattern.

4. **`Keyed` widget for list reconciliation**: `keyed(id, widget)` — hints to the reconciler
   that a widget is the same across re-renders even if its position changes. Prevents state
   loss in dynamic lists.

5. **`Length` enum for constraint expression**: `Length::Fill`, `Length::FillPortion(u16)`,
   `Length::Shrink`, `Length::Fixed(f32)`. Clean, type-safe way to express layout intent
   without CSS strings.

6. **`column![]` / `row![]` macros**: Declarative list construction.
   `column![widget_a, widget_b, widget_c]` reads like JSX. Reduces `.push()` boilerplate.

7. **Time-traveling debug tooling**: Built-in debug view with performance metrics and
   state history replay. Testing/debugging infrastructure.

8. **First-class async**: `Task::perform(future, Msg)` and `Subscription::run(stream)`.
   Async is part of the architecture, not an afterthought.

### G. What's notably BAD (anti-patterns to avoid)

1. **Per-widget `State` structs are verbose**: `text_input::State`, `scrollable::State`,
   `slider::State`, etc. Must be stored in the app struct and threaded through. Adds
   boilerplate for every interactive widget.

2. **Message explosion**: Complex apps generate enormous message enums. Every interaction
   needs a variant. Can become unwieldy without disciplined decomposition.

3. **No built-in virtual list**: `scrollable` handles large content but there's no
   virtualization. Large lists render all items.

4. **Still experimental**: Breaking changes between minor versions. Not production-stable.

5. **Update function boilerplate**: Every message needs a match arm. Simple interactions
   require navigating the full Elm pipeline.

---

## 3. slint

**Repository**: https://github.com/slint-ui/slint
**Latest version**: 1.17+
**License**: GPLv3 / Royalty-free / Commercial (tri-license)
**Docs**: https://docs.slint.dev

### A. Architecture & Component Model

**Paradigm**: Declarative DSL (`.slint` files), compiled to native code. Retained mode
with reactive property bindings.

The UI is defined in `.slint` markup, compiled ahead-of-time to native Rust/C++/JS/Python
code. Business logic connects via callbacks and properties.

```slint
export component HelloWorld inherits Window {
    width: 400px;
    height: 400px;
    Text {
        y: parent.width / 2;
        x: parent.x + 200px;
        text: "Hello, world";
        color: blue;
    }
}
```

- **Component definition**: DSL `component` keyword. Components inherit from built-in
  elements (`Window`, `Rectangle`, `Text`) or other components. Properties declared with
  `in`, `out`, `in-out` keywords. Pure expressions for bindings.
- **State management**: Property bindings are reactive — when a dependency changes, all
  bound expressions re-evaluate automatically. `in-out property <int> counter;`.
  States and transitions declared in DSL: `states [...]`, `transitions [...]`.
- **Props/children**: Properties are declared in the DSL. Children are nested elements:
  ```slint
  component MyCard inherits Rectangle {
      in property <string> title;
      Text { text: root.title; }
  }
  ```
- **Composition pattern**: Declarative nesting. `@children` placeholder for slot children.
  Pure functions for computed properties. Callbacks for events: `callback clicked();`.

### B. Layout System

**Engine**: Built-in layout engine (Flexbox-style) in the Slint runtime.

- **Primitives**: `HorizontalLayout`, `VerticalLayout`, `GridLayout`, `Row`.
  Also absolute positioning via `x`/`y` properties.
- **Constraints**: Per-element properties:
  ```slint
  min-width: 100px;
  max-width: 300px;
  preferred-width: 200px;
  horizontal-stretch: 1;   // flex-grow equivalent
  vertical-stretch: 0;
  ```
  `Alignment` enum: `start`, `center`, `end`, `stretch`, `space-between`.
- **Text measurement**: Custom text layout integrated with rendering backend.
  `Text` element has `wrap: word-wrap`, `overflow: elide`, `horizontal-alignment`.
- **Overflow**: `clip: true` on elements. `Flickable` for scrollable/pannable regions.
  `overflow` property: `visible`, `hidden`.
- **Responsive**: `@tr(...) ` for i18n. Dynamic grids (Slint 1.15+). No explicit breakpoint
  system, but layouts adapt via property bindings on window dimensions.
- **Layout debugging**: `layout-visible` debug property highlights layout rectangles.

### C. Component Inventory

**Built-in elements** (from `slint` crate docs):
| Element | Purpose |
|---------|---------|
| `Rectangle` | Basic colored rectangle (the `div` of Slint) |
| `Text` | Text display with wrapping, eliding, alignment |
| `TextInput` | Editable text input |
| `Image` | Raster image display |
| `TouchArea` | Interaction capture (clicked, pressed, moved) |
| `Flickable` | Scrollable/pannable container |
| `FocusScope` | Keyboard focus management scope |
| `Path` | Vector path drawing (LineTo, MoveTo, CubicTo, QuadraticTo) |
| `Window` | Root container |
| `PopupWindow` | Floating popup |
| `HorizontalLayout` | Flexbox row |
| `VerticalLayout` | Flexbox column |
| `GridLayout` | CSS Grid equivalent |
| `Row` | Simple horizontal stack |

**Standard widgets** (from `std-widgets.slint`):
| Widget | Purpose |
|--------|---------|
| `Button` | Standard button |
| `StandardButton` | Button with preset kind (ok, cancel, apply) |
| `CheckBox` | Checkbox with label |
| `ComboBox` | Dropdown select |
| `Slider` | Horizontal slider |
| `SpinBox` | Numeric stepper input |
| `Switch` | Toggle switch |
| `ProgressIndicator` | Progress bar/spinner |
| `LineEdit` | Single-line text input |
| `TextEdit` | Multi-line text editor |
| `ListView` | Virtual scrolling list |
| `StandardListView` | List with string items |
| `StandardTableView` | Table with columns |
| `TabWidget` | Tabbed container |
| `GroupBox` | Labeled group container |

### D. State & Interaction Patterns

- **Hover/focus/pressed**: `TouchArea` element provides:
  ```slint
  TouchArea {
      clicked => { root.callback-clicked(); }
      pressed: bool;      // read-only state
      contains-press: bool;
      mouse-x, mouse-y: length;
      mouse-cursor: MouseCursor;
  }
  ```
  No separate state struct — state is in properties.
- **Disabled**: `enabled: false` property on widgets (std-widgets support this).
- **Focus**: `FocusScope` element:
  ```slint
  FocusScope {
      has-focus: bool;
      key-pressed(event) => { ... accept/reject }
      key-released(event) => { ... }
  }
  ```
  Keyboard events handled via `key-pressed` / `key-released` callbacks.
- **Animations**: Built-in in DSL:
  ```slint
  animate x { duration: 250ms; easing: ease-in-out; }
  animate color { duration: 200ms; }
  ```
  States with transitions:
  ```slint
  states [
      disabled when !root.enabled : {
          opacity: 0.5; additional-opacity: 0.3;
      }
  ]
  transitions [
      in disabled : animate opacity { duration: 150ms; }
  ]
  ```
- **Gestures**: `TouchArea` provides `moved`, `pointer-event`. `Flickable` for drag-scroll.
  No built-in pinch/swipe.
- **Accessibility**: `accessible-role`, `accessible-label`, `accessible-value`,
  `accessible-description`, `accessible-delegate-focus` properties on elements.

### E. Theming & Tokens

**How themes are defined**: Compile-time styles + runtime property overrides.

- **Styles** (compile-time): `native` (platform-native via Qt on Linux), `fluent` (Windows 11),
  `material` (Material Design), `cocoa` (macOS). Selected via build config or `SLINT_STYLE` env var.
- **Color system**: Properties on elements — `color`, `background`, `border-color`.
  Global singletons for theme tokens:
  ```slint
  global Theme {
      in-out property <color> bg-primary: #ffffff;
      in-out property <color> text-primary: #1a1a1a;
  }
  ```
  No semantic slot system built-in — you define your own.
- **Spacing**: No global scale. `padding`, `margin` properties with length values.
- **Dark/light**: Via property bindings on a global singleton. Runtime switching:
  ```slint
  global Settings { in-out property <bool> dark-mode: false; }
  // In components:
  background: Settings.dark-mode ? #1a1a1a : #ffffff;
  ```
- **Density**: Not built-in.

### F. What's notably GOOD (patterns to steal)

1. **Property bindings (reactive without boilerplate)**: `text: "Count: " + root.counter;`
   automatically re-evaluates when `counter` changes. No subscriptions, no signals.
   The compiler optimizes away unnecessary recomputations.

2. **States + transitions in DSL**: Declarative state machines with animation transitions:
   ```slint
   states [ active when root.is-active : { bg: green; } ]
   transitions [ in active : animate bg { duration: 200ms; } ]
   ```
   Self-contained, readable, no imperative animation code.

3. **`TouchArea` as a separate composable element**: Rather than every widget having built-in
   interaction, `TouchArea` is a separate element you layer on top:
   ```slint
   Rectangle {
       TouchArea { clicked => { ... } }
   }
   ```
   Separation of visuals from interaction. Composable.

4. **Global singletons for theme tokens**: `global Theme { property <color> primary: ...; }`
   accessible from any component. Clean, no prop drilling.

5. **`@children` placeholder for slot composition**: Components declare where children go:
   ```slint
   component Card inherits Rectangle {
       @children  // slot for child elements
   }
   ```
   Simple, explicit slot mechanism.

6. **Compile-time optimization**: The Slint compiler inlines constant properties, removes
   dead code, and optimizes property bindings. Zero-cost abstraction for UI.

7. **Live Preview + LSP + Figma plugin**: Designer tooling is first-class. LSP server
   provides autocomplete, go-to-definition, live preview. Figma → Slint plugin.

8. **Multiple rendering backends**: `femtovg` (OpenGL ES), `skia` (Skia), `software` (CPU).
  Configurable at compile time. Same UI code runs on all backends.

### G. What's notably BAD (anti-patterns to avoid)

1. **DSL is a separate language**: Learning curve, separate tooling, no Rust IDE support
   for `.slint` files. Interop boundary between Rust and Slint is friction.

2. **Not pure Rust**: Business logic in Rust, UI in DSL. Two languages, two mental models.
   Harder to share types, debug across boundary.

3. **License complexity**: Tri-license (GPLv3 / royalty-free / commercial). Commercial use
   requires either royalty-free terms (with restrictions) or paid license. Creates
   uncertainty for adoption.

4. **Custom widget creation is harder**: Must implement in DSL or drop to C++/Rust FFI.
   Less flexible than pure-Rust frameworks for bespoke widgets.

5. **Property-based debugging is hard**: Reactive bindings can create hidden dependency
   chains. Circular dependencies, unnecessary recomputations are hard to trace.

---

## 4. gpui

**Repository**: https://github.com/zed-industries/zed/tree/main/crates/gpui
**Latest published version**: 0.2.2 (but tied to Zed's monorepo)
**License**: Apache-2.0
**Docs**: https://docs.rs/gpui, https://gpui.rs

### A. Architecture & Component Model

**Paradigm**: Hybrid immediate and retained mode, GPU-accelerated.

GPUI has three registers:
1. **Entity** (state) — owned by `App`, accessed via `Entity<T>` (like `Rc<T>`). Communicates
   via observations and subscriptions.
2. **View** (declarative) — an `Entity` implementing `Render` trait. `fn render(&mut self,
   window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement`. Called each frame.
3. **Element** (imperative) — low-level building blocks with total control over layout and
   painting. `Element` trait with `fn layout()`, `fn paint()`.

```rust
struct HelloWorld { text: SharedString }

impl Render for HelloWorld {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex().flex_col().gap_3()
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center().items_center()
            .text_xl().text_color(rgb(0xffffff))
            .child(format!("Hello, {}!", &self.text))
            .child(
                div().flex().gap_2()
                    .child(div().size_8().bg(gpui::red()))
                    .child(div().size_8().bg(gpui::green()))
            )
    }
}
```

- **Component definition**: `Render` trait for views. `div()` builder for elements.
  Custom elements implement `Element` trait. No separate component abstraction — a view IS
  the component.
- **State management**: `Entity<T>` system (like `Rc<RefCell<T>>`). `Context<T>` provides
  access to the entity and app services. `cx.observe(&entity, |this, cx| { ... })` for
  reactivity. `cx.subscribe(&entity, |this, emitter, event, cx| { ... })` for events.
- **Props/children**: `.child(element)` adds a single child. `.children(iterable)` adds many.
  No props struct — styling via method chaining on `div()`.
- **Composition pattern**: Tailwind-style builder. `div().flex().gap_3().bg(color).child(...)`.
  Views compose via `.child(view)`. `AnyElement` for type-erased children.

### B. Layout System

**Engine**: **Taffy** (CSS Flexbox/Grid layout engine, `taffy = 0.9.0`).

- **Primitives**: Flexbox via `.flex()`, `.flex_col()`, `.flex_row()`. Grid via `.grid()`,
  `.grid_cols(n)`. Absolute positioning via `.absolute()`, `.relative()`.
- **Constraints**: Full CSS-style properties:
  ```rust
  div()
      .gap_3()              // gap: 0.75rem
      .size(px(500.0))      // width + height
      .size_full()           // width: 100%, height: 100%
      .w(px(200.0))         // width
      .h_full()              // height: 100%
      .flex_1()              // flex-grow: 1
      .flex_shrink_0()       // flex-shrink: 0
      .justify_center()      // justify-content: center
      .items_center()        // align-items: center
      .min_w(px(100.0))     // min-width
      .max_w(px(500.0))     // max-width
      .aspect_ratio(16.0 / 9.0)
  ```
- **Text measurement**: Uses **`cosmic-text`** (`cosmic-text = 0.14.0`) for text layout
  and measurement. High-quality text shaping, bidirectional text, font fallback.
- **Overflow**: `.overflow_hidden()`, `.overflow_scroll()`, `.overflow_x_scroll()`.
  `ContentMask` for rectangular clipping regions.
- **Responsive**: Grid layout example on gpui.rs uses container queries:
  switches from 5-column grid to stacked flex layout when container becomes narrow.
  `Responsive` pattern via measuring available space.

### C. Component Inventory

GPUI provides **no pre-built widgets** (no Button, Checkbox, Slider). Everything is built
from `div()` and styled elements. Built-in element types:

| Element | Purpose |
|---------|---------|
| `div()` | Universal container + styler (the swiss-army knife) |
| `Anchored` | Display UI that avoids window bounds overflow (popovers, tooltips) |
| `Canvas` | Low-level custom paint callback (`paint_fn`) |
| `Animation` / `AnimationElement` | Apply animation to any element |
| `Deferred` | Delay painting until after ancestors (z-order control) |
| `AnyTooltip` / `AnyDrag` | Tooltip and drag state containers |
| `img()` | Image element |
| `svg()` | SVG element |
| `text()` | Styled text (via `StyledText` or `InteractiveText`) |
| `list()` | Virtual scrolling list (from `gpui::list`) |

**Zed's UI crate** (separate from gpui core) provides higher-level widgets like buttons,
but these are in `zed/crates/ui`, not in the `gpui` crate itself.

### D. State & Interaction Patterns

- **Hover/active/focus**: `InteractiveElement` trait adds `.hoverable()`, `.active()`,
  `.on_click()`, `.on_mouse_down()`, `.on_mouse_move()`. `StatefulInteractiveElement`
  adds `.tracked_by_tag(tag)` for persistent interaction state.
  ```rust
  div()
      .id("my-button")
      .child("Click me")
      .on_click(cx.listener(|this, event, window, cx| { ... }))
  ```
- **Disabled**: No built-in disabled state. Implemented via conditional styling + event
  guards.
- **Focus**: `FocusHandle` system. `cx.focus_handle()`. `element.focusable(true)`.
  `window.focused()`. Key bindings via `Action` system:
  ```rust
  actions![copy, paste, quit];  // defines action types
  // Register keybindings:
  cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
  // Handle in view:
  fn on_action(&mut self, _: &Quit, cx: &mut Context<Self>) { ... }
  ```
- **Keyboard**: `Action` system converts keystrokes to logical operations. `KeyContext`
  for context-sensitive bindings. Dispatch system handles propagation.
- **Animations**: `Animation` struct:
  ```rust
  div()
      .id("fade")
      .child("Hello")
      .with_animation("fade-in", Animation::new(200).with_easing(ease_in_out), |this| {
          this.opacity(1.0)
      })
  ```
- **Gestures**: `AnyDrag` for drag operations. `on_drag(element, cx.listener(...))`.
  `DragData` trait. No built-in pinch/swipe.
- **Accessibility**: AccessKit integration. `AccessibleElement` trait.
  `element.accessible_label("text")`. `FocusHandle` for focus management.

### E. Theming & Tokens

**How themes are defined**: No built-in theming system. Colors applied directly.

- **Color system**: `colors` module with predefined colors (`gpui::red()`, `gpui::green()`,
  etc.). `rgb(0xffffff)`, `hsla(0.0, 1.0, 0.5, 1.0)`. `Background` enum: `Solid` or
  `LinearGradient`. No semantic slots.
- **Styling**: `Styled` trait with ~100+ methods: `.bg()`, `.text_color()`, `.border_color()`,
  `.border_1()`, `.rounded_lg()`, `.shadow_sm()`, `.gap_3()`, `.p_4()`, `.m_2()`, etc.
  Tailwind-inspired naming: `gap_3` = `gap: 0.75rem`, `p_4` = `padding: 1rem`.
- **Cascade system**: `Cascade` and `CascadeSlot` for merging refinements in priority order.
  `Refineable` trait for style merging.
- **Dark/light**: No built-in. Zed's app layer handles theme switching. `colors` module
  is static.
- **Density**: Not built-in. Zed uses its own `Theme` struct in the `ui` crate.

### F. What's notably GOOD (patterns to steal)

1. **Tailwind-style builder API**: `div().flex().flex_col().gap_3().bg(color).size(px(500))
  .justify_center().items_center().child(...)`. Extremely ergonomic. No builder struct
  to manage — method chaining on `Div`. **This is the most directly transferable pattern
  for canvas-ui** if it moves to a retained/hybrid model.

2. **Taffy layout engine**: Using taffy for proper CSS Flexbox + Grid layout. Full support
  for flex-grow, flex-shrink, min/max, aspect-ratio, gap. **CanvasDesk already uses taffy
  (ADR-0013/0014) — this validates the choice**.

3. **`Anchored` element**: `Anchored::new(child).anchor(Corner::TopLeft).position(point)`.
  Automatically repositions to avoid window overflow. Perfect for popovers, tooltips,
  dropdowns. Solves a common hard problem.

4. **`Deferred` element**: `Deferred::new(child)` — delays painting until after all ancestors,
  while keeping layout in the normal tree. Enables overlays/modals without z-index hacks.

5. **`Animation` with easing**: `.with_animation("id", Animation::new(200ms).with_easing(ease_in_out),
   |this| this.opacity(1.0))`. Declarative, composable, per-element.

6. **`Cascade` + `Refineable` for style composition**: Styles can be cascaded (merged in
   priority order) and refined (partially overridden). Enables theme inheritance without
   monolithic structs.

7. **Entity system for state ownership**: `Entity<T>` is like `Rc<RefCell<T>>` but owned
   by the `App`. Clean ownership model, prevents use-after-free, enables cross-component
   communication via observe/subscribe.

8. **`AnyElement` for type-erased dynamic trees**: `AnyElement` wraps any element type.
  Enables heterogeneous children lists without enums. `Vec<AnyElement>` for dynamic composition.

9. **`Action` system for keyboard shortcuts**: `actions![copy, paste, quit]` defines action
   types. `cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)])` registers bindings.
   Context-sensitive dispatch. Clean separation of intent from implementation.

10. **`#[gpui::test]` + `TestAppContext`**: Test macro provides simulated platform input.
    `TestAppContext` simulates windows, timers, async. Headless testing without GPU.

### G. What's notably BAD (anti-patterns to avoid)

1. **No pre-built widgets**: Everything is DIY from `div()`. No Button, Checkbox, Slider.
   High upfront cost. Zed's `ui` crate has them but it's not in the gpui crate.

2. **Still pre-1.0, tied to Zed**: Breaking changes between versions. API evolves with
   Zed's needs, not as a general-purpose framework.

3. **macOS/Linux only**: No Windows support (as of latest). GPU-accelerated via Metal
   (macOS) or Blade/Vulkan (Linux).

4. **Steep learning curve**: Best documentation is "read Zed's source code". No book,
   limited examples outside of gpui.rs.

5. **No theming system**: Colors are ad-hoc. No semantic slots, no theme switching in
   the core crate. Each app builds its own theme layer.

6. **Heavy dependencies**: `cosmic-text`, `taffy`, `usvg`/`resvg`, `lyon`, `parking_lot`,
   `smol`, `image`, etc. Large dependency tree.

---

## 5. xilem

**Repository**: https://github.com/linebender/xilem
**Latest version**: 0.4.0
**License**: Apache-2.0
**Docs**: https://docs.rs/xilem

### A. Architecture & Component Model

**Paradigm**: Experimental reactive view-diffing. Inspired by Flutter, SwiftUI, and Elm.

Xilem is a reactive layer on top of **Masonry** (a retained widget toolkit). Views are
lightweight, diffed each update to produce minimal changes to the retained Masonry tree.

```rust
#[derive(Default)]
struct Counter { num: i32 }

fn app_logic(data: &mut Counter) -> impl WidgetView<Counter> + use<> {
    flex(Axis::Vertical, (
        label(format!("{}", data.num)),
        text_button("increment", |data: &mut Counter| data.num += 1),
    ))
}

fn main() -> Result<(), EventLoopError> {
    let app = Xilem::new_simple(Counter::default(), app_logic, WindowOptions::new("Counter"));
    app.run_in(EventLoop::with_user_event())?;
    Ok(())
}
```

- **Component definition**: Views are values implementing `WidgetView<State>` trait.
  Built-in views are functions: `label(text)`, `text_button(label, callback)`, `flex(axis, children)`.
  Custom views implement `View` trait with `fn build()`, `fn rebuild()`, `fn message()`.
- **State management**: Application state is an arbitrary `'static` Rust type. `app_logic(&mut State)`
  produces a view tree. The framework diffs old and new views, applies minimal updates to
  Masonry widgets. No signals, no subscriptions — just re-run and diff.
- **Props/children**: Views are pure functions returning view values. Children passed as
  tuples (for fixed arity) or `ViewSequence` (for dynamic lists):
  ```rust
  flex(Axis::Vertical, (view_a, view_b, view_c))  // tuple
  flex(Axis::Vertical, (0..n).map(|i| label(format!("{}", i))))  // ViewSequence
  ```
- **Composition pattern**: Functional + tuple-based. `lens` adapter for scoped state:
  `lens(State::field, |field| some_view(field))`. `memoize(key, |key| view)` to skip
  view recreation.

**Key types**:
```
WidgetView<State>      // trait for views that produce Masonry widgets
ViewSequence<State>    // trait for 0..N view sequences
WidgetViewSequence     // ordered sequence of widget views
Xilem                  // runtime builder
ViewCtx                // context passed to View methods
MasonryDriver         // bridges Xilem views to Masonry widgets
```

**Masonry** (the retained layer):
- `Widget` trait with `fn layout()`, `fn paint()`, `fn accessiblity()`, etc.
- Built on Vello (GPU 2D renderer), Parley (text), AccessKit, winit.

### B. Layout System

**Engine**: Masonry's layout (BoxConstraints-based, like druid).

- **Primitives**: `flex(Axis, children)` — row/column layout. `grid` — divides window
  into regions. `sized_box` — forces specific dimensions. `split` — resizable two-pane.
  `zstack` — overlay children. `portal` — scrollable region.
- **Constraints**: BoxConstraints passed down from parent to child. Child returns Size.
  `sized_box` can force `width` / `height`. Flex layout distributes space.
- **Text measurement**: **Parley** (linebender's text layout crate) — high-quality text
  shaping, bidirectional text, font fallback. Integrates with Vello for rendering.
- **Overflow**: `portal` view wraps content in a scrollable region.
- **Responsive**: Not yet implemented (experimental stage).

### C. Component Inventory

**View elements** (from xilem docs):

| View | Purpose |
|------|---------|
| `button(label, callback)` | Basic button |
| `label(text)` | Static text label |
| `prose(text)` | Immutable, selectable text |
| `text_input(state, placeholder)` | Editable text input |
| `image(source)` | Image display |
| `progress_bar(value)` | Progress bar |
| `flex(axis, children)` | Row/column layout |
| `grid(children)` | Grid layout |
| `sized_box(child, width, height)` | Force dimensions |
| `split(first, second, direction)` | Resizable split pane |
| `zstack(children)` | Overlay stack |
| `portal(child)` | Scrollable region |
| `task(future, view_fn)` | Async task view |

**Adapters** (from xilem_core):
| Adapter | Purpose |
|---------|---------|
| `lens(accessor, view_fn)` | Scoped state access |
| `memoize(key, view_fn)` | Skip view recreation when key unchanged |

**Style traits** (from `xilem::style`):
- `Style` trait for setting custom styles on views (background color, text size, etc.)
- `palette` module with predefined color palettes

### D. State & Interaction Patterns

- **Hover/focus/pressed**: Not yet well-developed (experimental). Button callback:
  `text_button("label", |data: &mut State| { ... })`. No hover state API yet.
- **Disabled**: Not yet implemented in the public API.
- **Focus**: Limited. Masonry has focus infrastructure but Xilem's view layer doesn't
  expose it fully yet.
- **Keyboard**: Via Masonry's event system. Not yet exposed at the Xilem view level.
- **Animations**: Not yet implemented.
- **Gestures**: Not yet implemented at view level.
- **Accessibility**: AccessKit integration via Masonry. `Masonry::accessibility()` method.

### E. Theming & Tokens

**How themes are defined**: Early stage. `style` module with traits. `palette` module.

- **Color system**: `Color` type alias (from `peniko::Brush`). `palette` module with
  predefined colors. No semantic slot system.
- **Styling**: `Style` trait — views can implement it to accept style overrides:
  ```rust
  label("text").style(|style| style.text_size(16.0).text_color(Color::BLACK))
  ```
  (API is still evolving)
- **Dark/light**: Not yet implemented.
- **Density**: Not yet implemented.

### F. What's notably GOOD (patterns to steal)

1. **View diffing (React-style reconciliation)**: `app_logic` runs each update, returns a
   view tree. Framework diffs old vs new, applies minimal updates to retained widgets.
   No manual update logic. **Transferable concept for canvas-ui**: build a view tree, diff it.

2. **`lens` adapter for scoped state**: `lens(State::field, |field| view_for_field(field))`.
   Lets a sub-component work with a slice of state without knowing the full type.
   Clean separation of concerns. Composable.

3. **`memoize` for performance**: `memoize(key, |key| expensive_view(key))`. If the key
   hasn't changed, skip view rebuild entirely. Opt-in performance optimization.

4. **`+ use<>` precise capturing**: `impl WidgetView<State> + use<>` — the 2024 edition
  syntax that says the return type doesn't borrow from parameters. Required because view
  types must be `'static`. Clean, type-safe.

5. **`task` view for async**: `task(async_future, |result| view_for_result(result))`.
   Launches an async task that runs until the view leaves the tree. Natural async integration.

6. **Parley for text**: Using Parley (linebender's text layout) for high-quality text
   shaping and layout. **CanvasDesk uses cosmic-text — both are high-quality options**.

7. **Tuple-based children**: `flex(Axis::Vertical, (a, b, c))` — fixed-arity children as
   tuples. Type-safe, no allocation, no vec. `ViewSequence` for dynamic lists.

8. **Pure function view builders**: Views are pure functions of state. No stored state,
   no side effects. Easy to test, easy to reason about.

### G. What's notably BAD (anti-patterns to avoid)

1. **Very experimental / alpha**: Not production-ready. Limited widget set, missing
   features (animations, gestures, theming). API still changing.

2. **Limited documentation**: Best resource is the source code and Raph Levien's talks.
   No book, limited examples.

3. **No theming system**: Colors are ad-hoc. No theme switching, no semantic slots.

4. **Steep conceptual barrier**: View diffing + precise capturing + `'static` views is
   a lot of Rust type system gymnastics. Not beginner-friendly.

5. **Masonry/Xilem split**: Two layers (Masonry retained, Xilem reactive) can be confusing.
   Unclear when to use which.

---

## 6. druid

**Repository**: https://github.com/linebender/druid (archived)
**Last version**: 0.8.3
**License**: Apache-2.0
**Docs**: https://docs.rs/druid, https://linebender.org/druid/

> **Note**: Druid is archived and succeeded by xilem/masonry. It remains influential for
> its architectural patterns.

### A. Architecture & Component Model

**Paradigm**: Data-first, retained mode. `Widget<T>` trait parametrized by data type.

```rust
struct AppState { count: i32 }

impl Widget<AppState> for CounterWidget {
    fn event(&mut self, ctx: &mut EventCtx, event: &Event, data: &mut AppState, env: &Env) { ... }
    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, data: &AppState, env: &Env) { ... }
    fn update(&mut self, ctx: &mut UpdateCtx, old_data: &AppState, data: &AppState, env: &Env) { ... }
    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, data: &AppState, env: &Env) -> Size { ... }
    fn paint(&mut self, ctx: &mut PaintCtx, data: &AppState, env: &Env) { ... }
}
```

- **Component definition**: `Widget<T>` trait — five methods (event, lifecycle, update,
  layout, paint). Built-in widgets in `druid::widget` module. Custom widgets implement the trait.
- **State management**: `Data` trait (requires `Clone + PartialEq`). Change detection via
  `PartialEq::ne(old, new)`. `Lens` for focused sub-state access:
  ```rust
  druid::lens!(AppState, count, i32);  // generates a lens
  Label::new("").lens(AppState::count);  // widget sees only the i32
  ```
  `Env` for environment/theme values. `Scope` for encapsulated sub-state.
- **Props/children**: `WidgetExt` fluent API adds methods to any widget:
  `button("text").on_click(handler).padding(10.0).background(color)`.
  `ControllerHost` wraps widget + behavior controller.
- **Composition pattern**: Builder via `WidgetExt`. Controllers via `Controller` trait.
  `WidgetPod` for wrapping child widgets. `LensWrap` for scoped data.

### B. Layout System

**Engine**: BoxConstraints-based (not cassowary, but constraint-passing).

```rust
pub struct BoxConstraints {
    pub min: Size,
    pub max: Size,
}
```

- **Primitives**: `Flex` (row or column via `Axis::Horizontal` / `Axis::Vertical`).
  `Align`, `Padding`, `Container`, `ClipBox`, `SizedBox`, `AspectRatioBox`, `ZStack`,
  `Split`, `IntrinsicWidth`, `Viewport`.
- **Constraints**: `BoxConstraints` passed from parent to child. Child returns `Size`.
  `FlexParams` per-child: `FlexParams::new(flex_factor, None)` or `FlexParams::new(0.0,
  Some(Length::Fixed(100.0)))`.
  ```rust
  pub enum MainAxisAlignment { Start, Center, End, SpaceBetween, SpaceEvenly }
  pub enum CrossAxisAlignment { Start, Center, End, Fill, Baseline }
  ```
- **Text measurement**: `xi-unicode` for text segmentation. `druid::text` module for
  layout. `LocalizedString` for i18n.
- **Overflow**: `ClipBox` for rectangular clipping. `Scroll` for scrollable content.
  `Viewport` for viewport-into-larger-area.
- **Responsive**: No built-in. `ViewSwitcher` for conditional layouts.

### C. Component Inventory

Full widget list from `druid::widget` module (docs.rs):

**Inputs**:
| Widget | Description |
|--------|-------------|
| `Button` | Button with text label |
| `Checkbox` | Checkbox toggling a `bool` |
| `Radio` | Single radio button |
| `RadioGroup` | Group of radio buttons |
| `Slider` | Slider for numeric value |
| `RangeSlider` | Slider with two thumbs (range) |
| `Stepper` | Step-wise increment/decrement |
| `Switch` | Toggle switch |
| `TextBox` | Text input widget |
| `ValueTextBox` | TextBox with `Formatter` for formatting/validation |

**Containers**:
| Widget | Description |
|--------|-------------|
| `Flex` | Row or column layout |
| `Container` | Visual styling wrapper |
| `Padding` | Padding wrapper |
| `Align` | Alignment wrapper |
| `SizedBox` | Fixed size wrapper |
| `AspectRatioBox` | Preserves aspect ratio |
| `ClipBox` | Rectangular viewport into child |
| `Split` | Two-pane resizable split |
| `ZStack` | Stack children on top of each other |
| `Scroll` | Scrollable container |
| `Tabs` | Tabbed container with `TabsPolicy` |
| `Scope` | Encapsulated sub-state |
| `Either` | Switch between two child views |
| `Maybe` | Switch based on `Option<T>` |
| `ViewSwitcher` | Dynamic child selection |
| `EnvScope` | Scoped environment override |
| `IdentityWrapper` | Add identity to anonymous widget |
| `IntrinsicWidth` | Size to child's max intrinsic width |
| `Viewport` | Viewport into larger area |

**Data display**:
| Widget | Description |
|--------|-------------|
| `Label` | Static/dynamic text label |
| `RawLabel` | Text display without layout logic |
| `List` | Variable-size collection list |
| `Image` | Bitmap image |
| `Svg` | SVG image |
| `Painter` | Paint-only widget (custom drawing) |

**Feedback**:
| Widget | Description |
|--------|-------------|
| `ProgressBar` | Progress display |
| `Spinner` | Animated loading spinner |

**Controllers** (behavior wrappers):
| Controller | Description |
|------------|-------------|
| `Click` | Makes child clickable (`on_click` via `WidgetExt`) |
| `Added` | Responds to `LifeCycle::WidgetAdded` |

### D. State & Interaction Patterns

- **Hover/focus/pressed**: Via `Event` enum in `event()` method:
  ```rust
  pub enum Event {
      MouseDown(MouseEvent),
      MouseUp(MouseEvent),
      MouseMove(MouseEvent),
      Wheel(MouseWheelEvent),
      KeyDown(KeyEvent),
      KeyUp(KeyEvent),
      Timer(TimerToken),
      // ...
  }
  ```
  `EventCtx` provides: `ctx.set_active(bool)`, `ctx.set_handled()`, `ctx.request_focus()`,
  `ctx.request_paint()`, `ctx.request_layout()`, `ctx.request_timer(duration)`.
- **Disabled**: `DisabledIf` widget wrapper: `child.disabled_if(|data, env| condition)`.
- **Focus**: `EventCtx::request_focus()`, `EventCtx::resign_focus()`.
  `LifeCycle::FocusReceived`, `LifeCycle::FocusLost`. No focus-trap.
- **Keyboard**: `Event::KeyDown(KeyEvent)`, `KeyEvent` has `key`, `code`, `mods`.
  `HotKey` + `SysMods` for keyboard shortcuts.
- **Animations**: `ctx.request_timer(duration)` → `Event::Timer(TimerToken)`.
  `Spinner` widget self-animates via timers. No built-in animation framework.
- **Gestures**: `MouseEvent` has `pos`, `button`, `count` (click count), `mods`.
  `MouseWheelEvent` for scroll. No built-in drag/pinch.
- **Accessibility**: AccessKit integration. `LifeCycleCtx::new_accessibility_node()`.

### E. Theming & Tokens

**How themes are defined**: `Env` (environment) with `Key<T>` for typed access.

```rust
// Define a key:
const MY_COLOR: Key<Color> = Key::new("my_app.my_color");

// Set in env:
let mut env = Env::default();
env.set(MY_COLOR, Color::rgb8(0xff, 0x00, 0x00));

// Read in widget:
let color = env.get(MY_COLOR);
```

- **Color system**: `Color` type. `BackgroundBrush` enum: `Color`, `LinearGradient`,
  `RadialGradient`. `theme` module with predefined keys:
  ```rust
  theme::PRIMARY_COLOR    // Key<Color>
  theme::BACKGROUND_COLOR
  theme::TEXT_COLOR
  theme::BUTTON_DARK
  theme::BUTTON_LIGHT
  theme::SELECTED_TEXT_BACKGROUND_COLOR
  // ... dozens of predefined keys
  ```
- **Spacing**: `theme::WIDGET_PADDING_COMPONENT`, `theme::WIDGET_CONTROL_COMPONENT_PADDING`.
  Keys for spacing values. No enum scale.
- **Dark/light**: Switch `Env` at runtime. `EnvScope` widget for scoped overrides:
  ```rust
  child.env_scope(|env, _| {
      env.set(theme::BACKGROUND_COLOR, Color::BLACK);
  })
  ```
- **Density**: Not built-in.

### F. What's notably GOOD (patterns to steal)

1. **`Data` trait with automatic change detection**: `Data: Clone + PartialEq`.
   `update()` is only called when `old_data != data`. Automatic, zero-config change detection.
   **But**: requires `Clone` which can be expensive for large state.

2. **`Lens` for composable state access**: `druid::lens!(AppState, field, FieldType)`.
   `widget.lens(AppState::field)` — widget sees only the field. Lenses compose:
   `LensExt::then(other_lens)`. `LensWrap` widget. Clean scoping without prop drilling.

3. **`Env` + `Key<T>` for type-safe theming**: Typed keys provide compile-time safety:
   ```rust
   const ACCENT: Key<Color> = Key::new("accent");
   env.get(ACCENT)  // returns Color, not &dyn Any
   ```
   **Transferable to canvas-ui**: slot-based theming with typed key access.

4. **`Controller` trait for behavior composition**: Separate behavior from rendering:
   ```rust
   impl Controller<MyData, MyWidget> for ClickController {
       fn event(&mut self, child: &mut MyWidget, ctx, event, data, env) {
           child.event(ctx, event, data, env);
           if let Event::MouseDown(_) = event {
               // handle click
           }
       }
   }
   ```
   `ControllerHost::new(widget, controller)`. Or via `WidgetExt::on_click(closure)`.

5. **`WidgetExt` fluent API**: Extension trait adds methods to any `Widget`:
   ```rust
   Label::new("text")
       .on_click(|ctx, data, env| { ... })
       .padding(10.0)
       .background(Color::WHITE)
       .border(Color::BLACK, 1.0)
       .disabled_if(|data, env| !data.enabled)
   ```
   Composable modifiers without modifying widget internals.

6. **`Either` / `Maybe` / `ViewSwitcher` for conditional rendering**:
   - `Either::new(condition, widget_a, widget_b)` — switch between two
   - `Maybe::new(option, widget_fn)` — for `Option<T>` data
   - `ViewSwitcher::new(data, |data, env| build_widget(data))` — dynamic child
   Clean conditional composition patterns.

7. **`EnvScope` for scoped theme overrides**: `child.env_scope(|env, _| { env.set(key, val); })`.
   Overrides theme for a subtree without affecting siblings.

8. **`Scope` for encapsulated state**: `Scope::new(state_fn, child_widget)` — child widget
   operates on a derived state type, isolated from the parent's data. Encapsulation.

9. **`BoxConstraints` layout is clean and composable**: `bc.constrain(size)`, `bc.max()`,
   `bc.min()`, `bc.is_width_bounded()`. Simple, well-understood constraint passing.

10. **`TabsPolicy` for flexible tab configuration**: Trait-based tab configuration:
    ```rust
    impl TabsPolicy for MyTabs { ... }
    ```
    Controls tab derivation, labels, transitions, edge placement.

### G. What's notably BAD (anti-patterns to avoid)

1. **`Data` trait requires `Clone + PartialEq`**: Cloning large state structs every update
   is expensive. `Rc<T>` workaround is common but breaks the pattern's simplicity.

2. **Widget trait is complex**: Five methods with five different context types
   (`EventCtx`, `LifeCycleCtx`, `UpdateCtx`, `LayoutCtx`, `PaintCtx`). High cognitive load
   for custom widgets.

3. **No async support**: No built-in way to handle futures. External event loop needed.
   Major limitation for networked/IO-heavy apps.

4. **Archived / unmaintained**: Succeeded by xilem/masonry. No new development, bug fixes,
   or updates. Use for pattern inspiration only.

5. **No virtual list**: `List` widget renders all items. No built-in virtualization for
   large collections.

6. **Per-frame full data comparison**: `PartialEq::ne` on the entire data tree every frame.
   Expensive for large/deep state.

7. **Manual layout for custom widgets**: Implementing `layout()` with `BoxConstraints` is
   non-trivial. Easy to get wrong, especially with text measurement.

---

## Comparison Table

| Feature | egui | iced | slint | gpui | xilem | druid |
|---------|------|------|-------|------|-------|-------|
| **Architecture** | Immediate (multi-pass) | Retained, Elm | Declarative DSL, retained | Hybrid (imm + retained) | Reactive view-diffing | Data-first, retained |
| **Component Model** | Functions on `&mut Ui` | `Widget<Msg>` trait + msg enums | DSL `component` keyword | `Render` trait + `div()` builder | `WidgetView<State>` trait + view fns | `Widget<T>` trait (5 methods) |
| **State Model** | ID-hashed `Memory` | Message-passing (Elm) | Reactive property bindings | `Entity<T>` (Rc-like) + observe/subscribe | Arbitrary `'static` type + view diffing | `Data` trait (Clone+PartialEq) + `Lens` |
| **Layout Engine** | Custom cursor-based | Custom flexbox-like | Built-in (HBox/VBox/Grid/Flex) | **Taffy** (CSS Flexbox + Grid) | Masonry BoxConstraints | BoxConstraints + Flex |
| **Text Engine** | Custom (epaint) | cosmic-text (recent) | Custom (per-backend) | **cosmic-text** | **Parley** | xi-unicode |
| **Widget Count** | ~20 inputs + ~14 containers | ~35 widgets | ~14 elements + ~16 std-widgets | ~0 (DIY from div) | ~13 view elements | ~30 widgets + ~20 containers |
| **Theming** | `Style`/`Visuals` struct (runtime) | `Theme` enum + `Catalog` per-widget | Compile-time styles + runtime globals | Tailwind-style (ad-hoc, no system) | `style` traits + `palette` (early) | `Env` + `Key<T>` (type-safe) |
| **Dark/Light** | `Theme::Light`/`Dark` (runtime) | `Theme::Light`/`Dark`/`Custom` | Property bindings on globals | None (app-level) | Not yet | `Env` switch (runtime) |
| **Animations** | `animation_time` + `animate_value_to` | `animation` module (manual) | Built-in DSL `animate` + `transitions` | `Animation` struct + easing | Not yet | Timer-based (manual) |
| **Accessibility** | AccessKit + `labelled_by` | AccessKit | AccessKit + `accessible-*` props | AccessKit + `FocusHandle` | AccessKit (via Masonry) | AccessKit |
| **Async** | No (immediate mode) | `Task`/`Subscription` (futures) | Callbacks (language-agnostic) | `smol` executor | `task` view | No |
| **Testing** | `egui_kittest` (snapshots) | Debug tooling (time-travel) | Live Preview + LSP | `#[gpui::test]` + `TestAppContext` | Limited | `test-log` |
| **Focus Management** | `request_focus`/`surrender_focus` | Per-widget `State::focus` | `FocusScope` element | `FocusHandle` + `Action` system | Limited (Masonry-level) | `EventCtx::request_focus` |
| **Layout Constraints** | None (sequential cursor) | `Length` enum (Fill/Shrink/Fixed) | `min/max/preferred-width` + `stretch` | Full CSS (flex/grid/min/max/aspect) | BoxConstraints (min/max) | BoxConstraints + `FlexParams` |
| **Maturity** | Active, production (Rerun) | Active, experimental | Stable 1.x, production | Pre-1.0, tied to Zed | Alpha, experimental | Archived (succeeded by xilem) |
| **GPU Rendering** | wgpu / glow (OpenGL) | wgpu / tiny-skia (software) | femtovg / skia / software | Metal / Blade / Vulkan | Vello (wgpu) | piet (CPU/GPU) |

---

## Top 10 Patterns to Steal

Ranked by transferability to an immediate-mode Rust kit with cosmic-text measurement
and slot-based theming (canvas-ui in CanvasDesk).

### 1. egui's `Response` Pattern — Universal Interaction Return Type
**Source**: `egui::Response` struct, returned by every `ui.button()` / `ui.add()` call.
**What it does**: Every widget call returns a `Response` with `hovered`, `clicked`,
`dragged`, `has_focus`, `changed`, `active`, etc. No callbacks needed.
**Why steal it**: Eliminates callback boilerplate entirely. In canvas-ui, each widget
draw call (`kit.button(ctx, label, rect)`) could return a `Response` struct with
interaction state. The caller checks `if resp.clicked { ... }` right after drawing.
**Signature**: `fn button(&mut self, label: &str) -> Response`
**Transferability**: **Direct** — canvas-ui is already immediate mode.

### 2. egui's `on_hover_text` / `on_hover_ui` — Composable Tooltips
**Source**: `Response::on_hover_text(&self, text: impl ToString) -> &Self`
**What it does**: Method on `Response` that registers a tooltip. Chains: `ui.button("x").on_hover_text("Help")`.
**Why steal it**: Zero-boilerplate tooltip attachment. In canvas-ui: `kit.button(...).on_hover_text(ctx, "Help")`.
Can also support `on_hover_ui` for rich tooltips (draw a mini UI).
**Transferability**: **Direct**.

### 3. gpui's Tailwind-Style Builder API — Ergonomic Styling
**Source**: `div().flex().flex_col().gap_3().bg(color).size(px(500)).justify_center().items_center().child(...)`
**What it does**: Method chaining on a `Div` builder. ~100+ style methods with
Tailwind-inspired naming (`gap_3` = 0.75rem, `p_4` = 1rem).
**Why steal it**: Most ergonomic styling API in the Rust UI ecosystem. If canvas-ui
adds a retained/hybrid element layer, this pattern eliminates builder struct boilerplate.
Even in immediate mode, a style builder could pre-configure draw calls.
**Transferability**: **Medium** — requires a retained element layer or style builder struct.

### 4. druid's `Env` + `Key<T>` — Type-Safe Slot Theming
**Source**: `druid::Env`, `druid::Key<T>`, `env.get(KEY)` returns typed `T`.
**What it does**: Theme values stored in an `Env` map, accessed via typed `Key<T>`.
`const ACCENT: Key<Color> = Key::new("accent");` — compile-time type safety.
`env.set(ACCENT, Color::RED)` at runtime.
**Why steal it**: CanvasDesk already uses slot-based theming. This pattern adds type
safety: a `Key<Color>` can only be read as `Color`, not `f32`. Prevents runtime type
errors. `EnvScope` for scoped overrides is also clean.
**Transferability**: **Direct** — maps exactly to slot-based theming.
**Concrete proposal**: `const ACCENT: Slot<Color> = Slot::new("accent");` then
`theme.get(ACCENT)` returns `Color`.

### 5. druid's `WidgetExt` Fluent API — Composable Modifiers
**Source**: `druid::widget::WidgetExt` trait: `.on_click()`, `.padding()`,
`.background()`, `.border()`, `.disabled_if()`, `.env_scope()`.
**What it does**: Extension trait that adds modifier methods to any widget.
`Label::new("text").padding(10.0).background(Color::WHITE).border(Color::BLACK, 1.0)`.
**Why steal it**: Composable modifiers without modifying widget internals. In canvas-ui,
a `WidgetExt`-like trait could add `.on_hover_text()`, `.disabled()`, `.with_slot(slot, value)`,
`.with_theme(theme)` to any widget response.
**Transferability**: **High** — works with immediate mode via response chaining.

### 6. iced's `Responsive` Widget — Adaptive Layouts
**Source**: `iced::widget::Responsive::new(|size: Size| { if size.width > 600.0 { ... } else { ... } })`
**What it does**: Widget receives available dimensions, enabling adaptive layouts without
CSS media queries.
**Why steal it**: CanvasDesk needs responsive node layouts (see ADR-0013/0014 taffy
hybrid). A `responsive(|width, height| { ... })` draw call that branches on available
space is clean and immediate-mode friendly.
**Transferability**: **Direct** — `kit.responsive(ctx, rect, |size| { ... })`.

### 7. egui's `labelled_by` — Immediate-Mode Accessibility
**Source**: `Response::labelled_by(&self, other_id: Id) -> &Self`
**What it does**: Associates a label widget with an input widget for screen readers.
`let label = ui.label("Name:"); ui.add(text_field).labelled_by(label.id)`.
**Why steal it**: Accessibility in immediate mode is hard because there are no persistent
widget references. `labelled_by` solves this with IDs returned from draw calls.
Canvas-ui can adopt: `let lbl = kit.label(ctx, "Name:", rect); kit.text_field(ctx, rect).labelled_by(lbl.id)`.
**Transferability**: **Direct**.

### 8. gpui's `Anchored` Element — Overflow-Aware Positioning
**Source**: `gpui::Anchored::new(child).anchor(Corner::TopLeft).position(point)`
**What it does**: Automatically repositions child to avoid window bounds overflow.
Used for popovers, tooltips, dropdowns, context menus.
**Why steal it**: Overflow-aware positioning is a common hard problem (tooltips near
screen edges, dropdowns near window bottom). An `anchored()` draw call that auto-flips
position would save significant manual logic.
**Transferability**: **High** — `kit.anchored(ctx, preferred_rect, |ctx| { ... })`.

### 9. slint's States + Transitions DSL — Declarative State Machines
**Source**: Slint `.slint` language:
```slint
states [ active when root.is-active : { bg: green; } ]
transitions [ in active : animate bg { duration: 200ms; } ]
```
**What it does**: Declarative state definitions with automatic animation transitions.
State changes trigger property animations.
**Why steal it**: CanvasDesk widgets need state machines (hover → pressed → focused →
disabled). A declarative state + transition system (even in Rust builders) would replace
imperative animation code. `widget.states([hovered when is_hovered]).transitions([in
hovered: animate bg { 200ms }])`.
**Transferability**: **Medium** — needs a retained style state or animation system.

### 10. egui's `UiBuilder` for Scoped Interaction State
**Source**: `egui::UiBuilder::new().disabled()` → `ui.scope_builder(builder, |ui| { ... })`
**What it does**: Creates a child `Ui` scope where all widgets are disabled (or invisible,
or have modified opacity). State cascades to all children.
**Why steal it**: CanvasDesk needs to disable groups of widgets (e.g., settings panel when
not connected). A scoped disable/readonly pattern avoids threading flags through every
draw call. `kit.scope(ctx, rect, Scope::disabled(), |kit| { ... })`.
**Transferability**: **Direct** — `kit.with_disabled(ctx, true, |kit| { ... })`.

---

## Bonus: Patterns Worth Noting (Beyond Top 10)

| # | Pattern | Source | Transferability |
|---|---------|--------|-----------------|
| 11 | `Cascade` + `Refineable` for style merging | gpui | Medium — retained style composition |
| 12 | `Animation` with easing functions | gpui | High — `Animation::new(200ms).with_easing(ease_in_out)` |
| 13 | `Action` system for keyboard shortcuts | gpui | High — `actions![copy, paste]` + `bind_keys` |
| 14 | `PaneGrid` for resizable split panes | iced | High — common UI need |
| 15 | `Themer` for scoped theme override | iced | High — `themer(child, \|theme\| theme.with_palette(...))` |
| 16 | `Keyed` for list reconciliation | iced | Medium — for dynamic lists |
| 17 | `Either` / `Maybe` / `ViewSwitcher` | druid | High — conditional rendering wrappers |
| 18 | `lens` + `memoize` for scoped state + perf | xilem | Medium — view-level optimization |
| 19 | `task` view for async | xilem | Medium — async in UI tree |
| 20 | `egui_kittest` snapshot testing | egui | **Direct** — visual regression for canvas-ui |
| 21 | Property bindings (reactive) | slint | Low — requires DSL or macro system |
| 22 | `@children` slot placeholder | slint | Medium — explicit slot composition |
| 23 | `Controller` trait for behavior separation | druid | Medium — wrap widget + add behavior |
| 24 | `TouchArea` as separate composable element | slint | Medium — separate interaction from visuals |
| 25 | Time-traveling debug | iced | Low — needs full state serialization |

---

## Summary for Canvas-UI Audit

**Most directly transferable patterns** (immediate-mode compatible, no architecture change):
1. `Response` return type from every widget draw call (egui)
2. `on_hover_text` / `on_hover_ui` method chaining (egui)
3. `labelled_by` for a11y label association (egui)
4. `Env` + `Key<T>` type-safe slot theming (druid)
5. `WidgetExt` fluent modifier API (druid)
6. `Responsive` widget for adaptive layouts (iced)
7. `Anchored` for overflow-aware positioning (gpui)
8. Scoped interaction state (`UiBuilder::disabled`) (egui)
9. `Animation` with easing (gpui)
10. Snapshot testing harness (egui_kittest)

**Validates existing CanvasDesk choices**:
- **Taffy for layout** — gpui also uses taffy (`=0.9.0`), confirming it's the right choice
- **cosmic-text for text** — gpui also uses cosmic-text (`=0.14.0`), confirming it's production-grade
- **Slot-based theming** — druid's `Env` + `Key<T>` is the type-safe version of this pattern
- **Immediate-mode architecture** — egui proves this scales to production (Rerun Viewer)
