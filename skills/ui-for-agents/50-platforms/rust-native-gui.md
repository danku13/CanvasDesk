# Rust Native GUI (egui / iced / Slint / GTK4)

> **Read when:** implementing a desktop UI in Rust with egui, iced, Slint, Relm4/GTK4, or a custom wgpu renderer. | **Section:** 50-platforms | **Related:** 10/design-tokens.md, 00/typography.md, 00/color.md, 50/cross-platform-parity.md, 40/visual-qa-protocol.md, 40/accessibility-wcag.md

**Core idea:** in native Rust UIs the toolkit's execution model (immediate vs retained) dictates your state architecture, and nothing the web gives for free — DPI, fonts, a11y, dialogs — exists unless you deliberately provide it.

## Rules

### R1. Choose the mental model before writing widgets
**Rule:** Decide upfront whether you are in immediate mode (egui: the UI is re-declared every frame, state lives in your app structs) or retained/Elm style (iced, Slint, Relm4/GTK4: widgets persist, messages/commands flow one way) — then structure state to match.
**Why:** Mixing models is the classic Rust-GUI failure: widget-local state in an immediate-mode app creates double-source-of-truth bugs, while shared mutation scattered through a retained app defeats the one-way message flow that made it testable.
**Example:** egui: `if ui.button("Save").clicked() { self.model.save(); }`. iced: `update(state, Message::Save)` mutates state; `view` is a pure function of state.

### R2. Immediate mode: state in app structs, UI as a read-only projection
**Rule:** Keep all durable state in plain app structs; the frame function only reads that state and reports interactions back into it.
**Why:** Every frame is rebuilt from scratch, so anything that exists only inside widget closures evaporates between frames; a single owned state tree makes undo, serialization, and headless testing almost trivial.
**Example:** `struct App { query: String, zoom: f32 }` — `fn ui(&mut self, ctx)` paints the field from `self.query` and writes back on edit; no widget holds the truth.

### R3. Give every widget a stable, unique identity
**Rule:** In loops and generated views, seed widget IDs with a stable key (`egui::Id::new(id)`, `ui.push_id(idx, …)`, `id_salt`) — never let two widgets in the same container share an auto-hashed ID.
**Why:** Immediate-mode toolkits key focus, scroll position, and drag state by widget ID hash; a collision silently steals another widget's state — focus jumps rows, a list item drags the wrong pane.
**Example:** `egui::ScrollArea::both().id_salt(row.key)` per row; `ui.push_id(i, |ui| …)` inside `for (i, item) in items.iter().enumerate()`.

### R4. Treat DPI scaling as a first-class requirement
**Rule:** Author all sizes in points/logical units, never raw pixels, and verify the UI at 100%, 150%, and 200% scale factors before shipping.
**Why:** winit reports a per-monitor scale factor and most laptop users run 125–200%; pixel-hardcoded layouts blur or clip, and mixed-DPI multi-monitor setups expose every hardcoded constant.
**Example:** Set spacing/rounding via the toolkit style in points (`egui` multiplies by `pixels_per_point` internally); smoke-test with `WINIT_X11_SCALE_FACTOR=1.5`.

### R5. Own the text stack explicitly
**Rule:** Load fonts at startup, define an explicit fallback chain (Latin → CJK → emoji → monospace for code), and verify Unicode shaping, line-breaking, and mixed-script rendering with real content.
**Why:** Rust toolkits ship minimal default fonts — a missing fallback renders tofu boxes for CJK users, and naive space-based wrapping breaks on Japanese, Thai, and emoji clusters where "words" are not space-separated.
**Example:** `FontDefinitions` with Noto Sans + a CJK fallback; every text-heavy golden test includes a string like `"日本語 — مرحبا — 👋🏽"` so tofu cannot ship unnoticed.

### R6. Map design tokens at exactly one integration point
**Rule:** Convert the token schema into toolkit style variables in one module (`theme.rs` filling egui's `Style`, an iced `Theme` impl, or Slint globals); components read toolkit style only, never token literals.
**Why:** One mapping module keeps the schema the single source of truth (see 10-design-system/design-tokens.md), makes dark mode a one-file change, and prevents drift between platforms that share a design.
**Example:** `fn apply(tokens: &Tokens, ctx: &egui::Context)` sets `Visuals` colors, spacing, and rounding from `tokens.color.surface` / `tokens.space[2]` — and nothing else in the crate names a hex value.

### R7. Build custom widgets with hit-testing AND a11y in one code path
**Rule:** A custom widget drawn from primitives must implement correct hit-testing/interaction and expose an AccessKit node (role, name, state) in the same code path; ship neither without the other.
**Why:** Painted pixels carry no semantics — without an AccessKit node the widget is literally invisible to screen readers, and retrofitting a11y later means re-deriving every widget's geometry and state mapping twice.
**Example:** A custom color-swatch picker returns clicks on swatch rects AND publishes `Role::Button`, name ("Red"), and selected-state per swatch through AccessKit — contrast ≥ 3:1 for swatch borders included.

### R8. Prefer native dialogs, menus, and OS conventions
**Rule:** Use the platform's file dialogs (rfd or the toolkit's native bridge), system menu bar, and standard accelerator conventions instead of rebuilding them in-toolkit.
**Why:** Native dialogs get OS accessibility, keyboard navigation, and user muscle memory for free; a hand-built file picker is weeks of work that still feels wrong on every platform it ships to.
**Example:** `rfd::FileDialog::new().add_filter("Image", &["png", "jpg"])` for save; Ctrl+O/Ctrl+S on Linux/Windows, Cmd+O/Cmd+S on macOS.

### R9. Budget the frame: no per-frame allocations or text recomputes
**Rule:** In immediate-mode apps, hoist allocations out of frame closures, gate expensive filtering/layout behind change detection, and let the toolkit cache what it can (egui caches text layout per widget ID).
**Why:** Immediate mode repaints at display rate, so per-frame `Vec`/`String` churn and re-wrapping unchanged text burn the 16 ms frame budget and stutter on low-end hardware — perceived performance dies first in scroll and drag.
**Example:** `if self.filter != last_filter { self.filtered = compute(&rows, &filter); }` outside `ui()`; format labels into reusable buffers instead of fresh `String`s each frame.

### R10. Test through harnesses: snapshots plus a11y-tree assertions
**Rule:** Cover every screen with golden/snapshot tests (egui kittest, iced snapshot testing, Slint screenshot API) and assert on the accessibility tree — roles, names, states — not just pixels, per 40-quality/visual-qa-protocol.md.
**Why:** Pixels catch visual regressions; a11y-tree assertions catch missing labels and roles that pixels cannot show — a screen can look perfect and still be unreadable to a screen reader.
**Example:** kittest: `harness.click_button("Save")`; then assert a node with `Role::Button` / name "Save" exists and the result label's `Role::Status` text updated.

### R11. Follow host keyboard conventions
**Rule:** Map shortcuts per platform (Ctrl on Linux/Windows, Cmd on macOS), honor the host's menu and accelerator conventions, and never hardcode one modifier for all OSes.
**Why:** Users carry per-OS muscle memory — a Ctrl+C that should be Cmd on macOS reads as "this app is foreign" and gets reported as a bug, not a style choice.
**Example:** winit's `mods.command()` is Cmd on macOS and Ctrl elsewhere, so `if mods.command() { save(); }` honors both with one code path.

### R12. Never assume a display server; keep headless runs possible
**Rule:** Architect so the app and its tests can run without a visible window — software rendering, offscreen contexts, or xvfb — and keep window creation out of core logic.
**Why:** CI has no display; the moment `App::new()` requires a real window manager, snapshot tests and a11y assertions become unrunnable and QA degrades to manual-only, which is how regressions ship.
**Example:** Split `App::handle(&mut state, &events)` from `run_windowed(state)`; CI runs `xvfb-run cargo test` or the harness with a software renderer.

### R13. Announce async state changes to the a11y tree
**Rule:** Async outcomes (save finished, upload failed, validation error) must reach the accessibility tree — via AccessKit events/announcements or a status node update — not only as a pixel change.
**Why:** A toast that repaints but announces nothing is invisible to a screen-reader user; state coverage (loading/done/error, per 40-quality/accessibility-wcag.md) has no meaning if state changes are silent.
**Example:** After `save()` completes, update the status widget's AccessKit node (`Role::Status`, text "Invoice saved") and assert the announcement in the kittest a11y snapshot.

## Checklist
- [ ] Toolkit model chosen deliberately; state architecture matches it (no widget-held truth in immediate mode)
- [ ] Every widget in loops/generated views has a stable unique ID
- [ ] All sizes in logical units; layout verified at 100 / 150 / 200 % scale
- [ ] Font fallback chain covers Latin, CJK, emoji; mixed-script demo text renders without tofu
- [ ] All tokens flow through one theme module; zero color/spacing literals in components
- [ ] Every custom widget exposes AccessKit role + name + state
- [ ] File dialogs and shortcuts follow host OS conventions (Ctrl vs Cmd correct)
- [ ] Snapshot tests + a11y-tree assertions run headless in CI
- [ ] Interactive elements ≥ 24×24 px; text contrast ≥ 4.5:1 in the native theme (light and dark)
- [ ] Micro-interaction durations within 150–300 ms in the theme constants

## Anti-patterns
- Storing UI state (open/closed, scroll, focus) inside immediate-mode widget closures
- ID hash collisions in list loops → ghost focus and swapped drag state
- Raw-pixel sizes that shatter at 200 % DPI
- A custom widget with no AccessKit node ("a11y later")
- Reimplementing the OS file picker or menu bar
- Rebuilding text layout for unchanged strings every frame
- Ctrl-only shortcuts on macOS
- A `main()` that cannot run without a display server, breaking CI QA

## Sources
- egui: https://github.com/emilk/egui , https://docs.rs/egui
- kittest (egui harness / a11y assertions): https://docs.rs/kittest
- iced: https://github.com/iced-rs/iced
- Slint: https://docs.slint.dev
- AccessKit: https://accesskit.dev
- Relm4 (GTK4): https://relm4.org
- winit (scale factors, `command()` modifier): https://docs.rs/winit
- rfd (native file dialogs): https://docs.rs/rfd
- WCAG 2.2 (floor the native theme must also meet): https://www.w3.org/TR/WCAG22/
