# CanvasDesk — Agent Instructions

## Language policy

**Agent-facing documentation is maintained in English** (owner decision,
2026-10-10): English-first improves instruction-following and reduces token
cost. Scope: `AGENTS.md`, `CONTEXT.md`, `skills/`, and the machine-consumed
layer of `docs/` (SPEC, TASKS, RECIPES, ui-kit, WASM-TESTING, active
`index-cr-fr.md`) — translated in waves (#24); wave 1 = AGENTS.md, CONTEXT.md,
skills/README.md + `docs/translation-guide.md` (terminology for the next
waves). **Not translated:** `user-docs/` (user-facing, Russian),
`worklog.md` (journal, Russian), historical ADRs and `docs/prd/*` (human
review artifacts). Code comments remain Russian (project language) unless the
owner decides otherwise. Terminology and "what not to translate" —
`docs/translation-guide.md`.

## Project

CanvasDesk is a **visual mathematical modeling system** (ADR-0007): an
infinite zoomable canvas on which executable mathematical models are built
from computation nodes (Numi sheets and templates); values are spilled along
value-edges (DAG engine); domain math (units, queueing, finance) is built
into the core; and an AI agent assembles and verifies models via MCP
(`canvas-mcp`). The model carrier is a file-backed canvas (cards are real
files), JS/HTML widgets (M5), and a "replace the desktop" mode (M4).
**Cross-platform (M7): Windows 10/11 x64 — full functionality; Linux
(X11/Wayland) and macOS — windowed application, platform gaps are closed per
the plan `docs/plans/M7-crossplatform.md`.** Language — Rust stable (1.80+),
edition 2021.

**Current repository state:** M1–M4 done; M5 (T20–T22) done in full — widget
runtime, bridge/permissions, SDK and docs (`docs/WIDGETS.md`); M6 — T24 (MCP)
done. Cross-platform (M7) — plan in `docs/plans/M7-crossplatform.md`. Task
status — README.md and `docs/TASKS.md`.

## Documentation — single sources of truth

- `docs/SPEC.md` — the specification: stack, data model, rendering, shell
  integration, widgets, risks, milestone readiness criteria. Read before any
  task. (Still Russian — wave 2 translation; treat as canonical regardless.)
- `docs/TASKS.md` — infrastructure wave plan (T0–T22, M1–M5). Order within a
  milestone is strict.
- `docs/adr/` — architecture decision records (index — `docs/adr/README.md`,
  template — `docs/adr/adr-template.md`). Product positioning and the
  contracts "one template = one computation", value ports, and MCP
  composition live here. An ADR is written BEFORE implementing the decision;
  `Статус: принято` ("accepted") — only on the owner's direct request
  (ADR-0001, rule 2).
- `docs/change-requests/` — FR/CR documents following
  `docs/change-requests/cr-template.md` (index —
  `docs/change-requests/index-cr-fr.md`, active entries only: in progress /
  analysis / awaiting acceptance / draft; completed ones — in
  `docs/change-requests/index-cr-fr-archive.md`, do not read the archive
  without a task). The modeling wave is FR-013…FR-029, CR-013; acceptance
  references — ADR-0005 (Instagram MVP) and ADR-0006 (catalog №1–№5).
  Implementation follows the document only; upon completion the agent
  updates the status ("реализовано"), the document's Changelog, and
  `docs/ACCEPTANCE.md` (standing acceptance methodology §1–12; dated waves —
  `docs/ACCEPTANCE-archive.md`).
- `skills/` — the CanvasDesk MCP skill package for external AI agents
  (published derivative of the tool registry; the current tool counter lives
  only in `skills/README.md` — the single source of the number; consult it,
  not memory). **Any change to MCP tool set/semantics (`TOOLS` in
  canvas-mcp, `crates/canvas-scene/src/mcp.rs` in canvas-scene) must update
  `skills/` in the same commit**: the `skills_*` contract tests in canvas-mcp
  (catalog completeness, skill coverage, README counter, call positions)
  fail CI on drift. Protocol — `skills/UPDATE-PROTOCOL.md`.
- `docs/plans/product-roadmap.md` — the product roadmap (accepted by the
  owner 2026-09-18 together with ADR-0008): waves 0/A/B/V/S, the demand gate
  "≥ 5 external users built a model themselves and returned a second time",
  mapping of M0–M6 archdoc and R1–R5 of CR-013 to waves. Order of FR/CR
  modeling work and new dependencies is checked against this document.
- `docs/RECIPES.md` — recipes R1–R17 for desktop embedding (analysis of
  Lively Wallpaper and Seelen UI) + the list of source files. **Mandatory
  before T15–T18** and any shell-integration task.
- `docs/DEMO.md` — the recipe for a quick demo stand on Linux (Xvfb +
  lavapipe + `scripts/xdemo.py`): screenshots and GIFs from a live run. Read
  before recording any demo material.
- `docs/WASM-TESTING.md` — the recipe for quick WASM UI testing setup
  (levels L0–L3, web stand build without trunk, Chromium/WebGPU under Xvfb,
  scenarios and oracles, pitfalls). Read before the first wasm UI check
  (one-time setup ~10 minutes; commands in the recipe's §6 cheat sheet).

Rule: the agent verifies against these documents, not its own memory — APIs
change, undocumented Windows behavior differs across OS versions (see SPEC
§7.4: the Progman/WorkerW/DefView hierarchy in Win11 24H2+ differs from the
classic one).

Link naming in documentation: within the repository, links to md sources —
relative `*.md` only; `*.html` is allowed only for real on-disk artifacts
(e.g. `docs/prototypes/*.html`) and inside `user-docs/` (FR-031, link check
in the `docs_ui` tests). Links to built Pages pages use absolute web paths.
Gate: `python3 scripts/doc_lint.py` (CI job `docs-lint`; 0 errors required;
rule introduced 2026-10-10, issue #18).

## Mandatory question for every FR/CR: onboarding and documentation

**When finishing any FR or CR implementation, the agent MUST explicitly ask
the owner whether onboarding and user documentation need updating.** The
question is asked in the task's final report — the owner's silence does not
cancel the need. What to check:

1. **Onboarding** (`crates/canvas-app/src/onboarding_ui.rs`, FR-028): does
   the new functionality change tour steps? Add/reword steps (8±2, "one step
   = one thought"), update the step unit tests and
   `docs/interface-objects/onboarding.md`.
2. **User documentation** (`user-docs/`, 7 pages; built into the binary by
   the FR-031 viewer — `include_str!`, source
   `crates/canvas-app/src/docs_ui.rs`): are any formulations stale
   (behavior, hotkeys, object names)? Update the affected pages + the tables
   in `user-docs/README.md` and `user-docs/index.md`; links between pages —
   relative `*.html` only (the link check in the `docs_ui` tests fails CI on
   broken ones). Keep hotkeys in sync with the `HOTKEYS` list
   (`crates/canvas-app/src/lib.rs`, F1 overlay) and `user-docs/hotkeys.md`.

Docs/onboarding changes may be a separate commit at the owner's discretion;
the question does not imply automatic edits without confirmation.

## Target stack and workspace layout

Stack (details and versions — SPEC §3): winit 0.30 (window/input), wgpu 22+
(rendering), glyphon/cosmic-text (text), rstar (R-tree, spatial index),
serde_json (the `.canvas` format — JSON Canvas spec 1.0), rusqlite bundled
(thumbnail cache, FTS5 search, sessions), notify 6+ (file watcher),
windows-rs (Win32/COM), pdfium-render (PDF), image, webview2-com + WebView2
Evergreen (M5 widgets), tracing (logs), cargo-wix (MSI).

Workspace layout after T0 (SPEC §4):

```
Cargo.toml                 # workspace
crates/
  canvas-core/             # data model, JSON Canvas I/O, spatial index, Numi engine (expr/),
                           # DAG value-flow (flow.rs), template registry (templates.rs)
  canvas-render/           # wgpu renderer: camera, batching, text, textures, LOD
  canvas-shell/            # Windows-only: thumbnails, preview handlers, drag-drop, WorkerW
  canvas-preview-host/     # separate exe — sandbox for IPreviewHandler
  canvas-widgets/          # M5: WebView2 host, bridge, manifests, snapshots
  canvas-mcp/              # MCP mediator: stdio JSON-RPC <-> named pipe, run_stdio_with_transport (FR-037)
  canvas-scene/            # scene model + mcp_dispatch (tool catalog — TOOLS registry in canvas-mcp; counter — skills/README.md) — platform-neutral, wasm (FR-037/ADR-0012)
  canvas-mcp-headless/     # headless MCP server for wasmtime/wasip1 — MCP session verification without Windows (FR-037, leaf crate)
  canvas-web/              # M8/W4 (wasm-port): web platform layer — bindgen wrappers, web services; track B, leaf in the DAG (scaffold)
  canvas-app/              # the application: event loop, commands, UI state, main()
assets/                    # fonts, node icons, built-in widgets (assets/widgets/), templates (assets/templates/)
docs/                      # SPEC.md, TASKS.md, RECIPES.md, adr/, change-requests/
```

## Architecture rules

1. `canvas-core` imports nothing from `canvas-shell`, `canvas-render`, and in
   general does not depend on the OS or GPU. Platform logic — only behind
   traits (`ThumbnailProvider`, `PreviewProvider`, `ShellIntegration`) so
   that core is testable on any OS.
2. Platform code — in `canvas-shell`/`canvas-widgets`/`canvas-mcp` under
   `cfg(windows)`/`cfg(unix)`, or behind traits from canvas-core (the
   `ThumbnailProvider` + `NoopThumbnailProvider` pattern). Core and render
   must build and test on all three OSes (M7 CI matrix). Platform branches
   must not spread across `canvas-app`: app selects the trait implementation
   instead of branching on cfg at every call site.
3. All canvas coordinates — logical pixels (world-space); rendering —
   physical (`scale_factor`). DPI awareness — Per-Monitor V2. After
   re-parenting into the desktop (M4) do not trust `window.scale_factor()` —
   poll `GetDpiForWindow` (RECIPES R10).
4. Do not block the render thread: all I/O, COM and heavy decoders — in
   worker threads (thumbnail pool — 4 threads, results via channels). Heavy
   value-flow recomputation — the FR-064 scenario worker
   (`crates/canvas-scene/src/worker.rs`: double buffer
   `Arc<RwLock<FlowSolutions>>`, wake via `EventLoopProxy<AppEvent>`; O(N)
   flush — on the UI thread). The "fallback + warn" rule applies at this
   boundary: on worker failure/timeout (3 s)/panic — synchronous
   recomputation on the UI thread + `tracing::warn!` (result is
   bit-identical — golden tests `crates/canvas-scene/tests/worker_smoke.rs`);
   on wasm — the sync path is the norm.
5. Layout (coordinates, sizes, edges) — only in the `.canvas` file. SQLite
   (`~/.canvasdesk/cache.db`) — a recreatable cache; deleting it breaks
   nothing.
6. Autosave `.canvas` with a 2 s debounce + `.bak` of the previous version.
7. LOD by zoom (SPEC §6.2): < 0.25 — rectangle+icon; 0.25–0.6 — thumbnail+name;
   0.6–1.5 — preview; > 1.5 — live preview (max 3 at a time, 10% hysteresis).
   Widgets: live WebView2 at zoom ≥ 0.25, otherwise a snapshot; live-instance
   limit 6 (LRU).

## Win32 / shell rules

1. Undocumented techniques (WorkerW, messages 0x052C and 0x7402) — only with
   runtime detection and a fallback to a normal window, per the recipes in
   `docs/RECIPES.md`; each one carries a comment linking the recipe.
   Claims about Progman/WorkerW/DefView behavior not confirmed by RECIPES or
   runtime detection are considered unverified.
2. Recipes are implemented cleanly: Lively — GPL-3.0, Seelen UI — AGPL-3.0.
   Copying code, identifiers, structure and comments is forbidden; port
   mechanics only.
3. Key constraints from RECIPES: send 0x052C ONLY if WorkerW is absent (R4);
   `WS_EX_LAYERED` + `SetLayeredWindowAttributes(255)` strictly before
   `SetParent` (R2); verify styles after re-parenting (R3); idempotent icon
   hiding via reading `SHGetSetSettings` before the 0x7402 toggle (R5); do
   NOT use `SPI_SETDESKWALLPAPER` on a raised desktop (R9); `SHGetSetSettings`
   with fSet does not work on Win10+ — do not waste time.
4. Win32 enum wrappers — per the RECIPES R13 idiom (boxed closure via LPARAM,
   `extern "system"` trampoline, safe API outward).
5. Any desktop-embedding step failure → fallback to a normal window + a
   warning to the user (not only for unknown OS versions).

## Security and widget rules (M5)

- Widgets — local packages only (`%APPDATA%/canvasdesk/widgets/<id>/`),
  installed explicitly by the user. Remote URL as a widget — forbidden
  architecturally.
- Permissions from `widget.json` are checked on EVERY bridge call; without
  `network` all outgoing `WebResourceRequested` are blocked; navigation
  outside the package is forbidden; `SetVirtualHostNameToFolderMapping` +
  CSP `default-src 'self'`.
- Bridge — typed JSON-RPC over postMessage; all messages validated with
  serde schemas; an invalid message = drop + warn, never a panic.
- No network calls in the host — the product is local; the network exists
  only inside widgets with the `network` permission.

## Code style and quality

- Development is TDD: before implementing a task, write the tests describing
  the required behavior (per the acceptance criteria in `docs/TASKS.md`);
  the implementation is done when the tests are green. GPU/shell code that
  cannot be unit-tested moves its logic into pure functions that are tested.
- Errors: `thiserror` (library types) + `anyhow` (application boundaries);
  no `unwrap`/`expect` in production paths.
- `unsafe` — only in `canvas-shell`/`canvas-widgets`, every block with a
  SAFETY comment.
- Comments stay in Russian (project language); agent-facing documentation is
  English (see Language policy). Undocumented Win32 techniques — with a
  reference to the RECIPES recipe.
- Each task ends with a single commit using conventional commits message
  style (one task = one session = one commit; do not batch several tasks).
- New dependencies — only with justification in the commit/PR description.

## Work planning: GitHub issues + Projects (owner directive 2026-10-10)

**The order for any improvement: tasks first — then code; an issue is closed
only after verification.**

1. **Planning = creating tasks.** Any improvement (feature, wave, increment)
   starts with a high-level GitHub issue — before the first line of code.
   Work without an issue does not start.
2. **A high-level issue is the atomic unit of improvement** (a unit of
   value, not a batch of edits). Do not create an issue for every
   intermediate step: cosmetics and subtasks live inside the task.
3. **Sub-issues are parts of the implementation**: waves, stages, splits
   across parallel sub-agents (Task ID) are filed as sub-issues of the
   parent task; progress is aggregated by the Sub-issues progress field on
   the board.
4. **The board.** Tasks (high-level and sub-issues) land on Projects #1
   (CanvasDesk): Todo → In Progress → Done; In Progress — one wave at a
   time.
5. **Closing = verified fact.** An issue/sub-issue is closed only when: the
   gates are green (`cargo test --workspace`, clippy `-D warnings`, fmt,
   `wasm_gate --check` / `mcp_wasm_gate.sh` — per the task context), the
   acceptance criteria from the body are met, and UI changes passed the WASM
   check per the "WASM UI self-check" section. On closing — a summary
   comment: what was done, key commits, how it was verified.
6. **The tool — `scripts/github_tasks.py`** (all commands are idempotent;
   tokens: `GITHUB_TOKEN` — issues, `GITHUB_PROJECT_TOKEN` — the board, see
   the script header). The `plan` command creates a high-level task +
   sub-issues + the board in one step:
   ```
   GITHUB_TOKEN=ghp_... python scripts/github_tasks.py plan --spec plan.json --dry-run
   ```

## Per-task token accounting (development cost tracing)

**Every completed task — a CR/FR implementation, defect fix, document,
research — ends with recording the tokens spent** (owner rule 2026-10-08).
The data feeds end-to-end development cost tracing: task → estimate → actual
→ calibration of future estimates.

Rules:

1. Tokens are counted **per task** (scope — a specific CR/FR/defect/
   document), not per session. Sub-agents report the same way for their
   tasks.
2. The record goes into the worklog (the repository `worklog.md`, in the
   corresponding task's entry; the session system journal goes there too,
   for historical data).
2а. **Worklog rotation** (issue #23, 2026-10-10): `worklog.md` holds only
   the current period (~last 7 days) + the Journal index at the top (one
   line per entry: date, Task ID, scope, location — "current" or the
   archive file name). Entries older than the period move to
   `worklog/archive/` (one file per period:
   `worklog-<year-mm-dd>_<mm-dd>.md`) at the start of a month or when the
   worklog exceeds ~120 KB. The move is lossless (entry counts verified
   before/after), entry text is not edited; the Journal index row updates
   its location. An agent reading the journal reads the index and only the
   entries it needs; the archive is not read by default.
3. Line format:
   `Tokens: in≈<N>, out≈<N>, total≈<N>, model=<models>, scope=<CR-019|FR-100|...>`.
4. The agent has no direct API counter access — provide a **justified
   estimate** marked `estimate`: based on the volume of text
   read/written (guideline: ~4 chars ≈ 1 token for mixed RU/EN/code) plus
   session-context and tool-call overhead.
5. Estimates accumulate in the worklog as historical data used to calibrate
   effort and cost; the per-scope summary is made by the owner or a
   dedicated aggregation task.

## UI kit — mandatory rule for Rust UI work

**Any Rust UI work goes through the existing `canvas-ui` kit**
(`crates/canvas-ui/src/`). Hardcoding quads/colors/geometry bypassing the
kit is forbidden — it breaks theme consistency and reuse, and makes visual
regressions invisible until manual testing.

### What to use from the kit

| Need | Canonical kit path |
|---|---|
| UI element colors | `KitPalette` (`ThemeColors` v2 slots: `control_fill`, `control_danger`, `text`, `palette_border`, `menu_fill`, …) — NEVER inline `[f32;4]` rgba literals |
| Padding/radii/gaps | `canvas_core::tokens` (`SPACING_*`, `RADIUS_*`) — NEVER duplicate `4.0/8.0/16.0` as magic constants |
| Control sizes | `kit::BUTTON_HEIGHT`, `kit::ICON_BUTTON_SIZE`, `kit::LIST_ROW_H`, `kit::TEXT_FIELD_HEIGHT`, `kit::CHIP_HEIGHT`, `kit::SWITCH_W/H` — NEVER re-declare `HEADER_H=30.0`, `ROW_H=26.0`, `INPUT_H=32.0` locally |
| Layout (positioning) | `kit::constrain`, `kit::stack`, `kit::pad`, `Column`/`Row`/`Child`/`MeasuredItem`/`RowPolicy` — NEVER compute `x = panel.right() - 28.0, y = panel.y + 6.0` with magic numbers |
| Text (shaping + width + wrap) | `TextMeasurer` (`width_of`, `wrap`, `ellipsis`) — NEVER estimate width as `len() * 5.5 + 14.0` (CR-015 explicitly forbids this) and never build `Buffer::new` + `set_text` manually in overlay logic |
| Buttons / chips / icons / switches / fields | `kit::button_layout/style/size`, `kit::chip_*`, `kit::icon_button`, `kit::switch`, `kit::text_field` |
| Lists and scroll | `kit::list_rows` + `kit::ScrollState` + `kit::scroll_bar` |
| Popups / modals / tooltips | `kit::dropdown_menu`, `kit::modal`, `kit::tooltip` (anchor + flip + delay) |
| Panels / cards | `kit::panel_rect/style`, `kit::card` |
| Tables | `kit::Table` (retained) or `kit::row_guides` + `kit::paint_row` |
| Draw layer (quads on screen) | `Painter::rect/panel/control/label` + `PaintItem` (NOT a manual `CardInstance { pos, size, fill, border, params, corners }`) |

Full guide — `docs/ui-kit.md`; architecture —
`docs/prd/prd-0009-ui-layering-uikit.md`, surface contract —
`docs/interface-objects/surface-registry.md`, tokens —
`crates/canvas-core/src/tokens.rs` and
`docs/change-requests/fr-046-design-tokens.md`.

### What is forbidden (caught in review and by lints)

1. **Inline rgba literals in UI code** — `[0.30, 0.75, 0.55, 1.0]`,
   `Color::rgba(77, 191, 140, 255)` in overlay/panel logic. Even a
   "one-off" is future color drift. Route through `KitPalette` (an existing
   or new slot).
2. **Magic geometry numbers** — `4.0, 6.0, 22.0, 28.0, 110.0` in
   `UiRect::new(...)` and `pos: [...]`. Take them from `kit::*` constants or
   `tokens::SPACING_*`.
3. **Duplicated local constants** — `HEADER_H`, `ROW_H`, `INPUT_H`, `FONT_*`
   in every `*_ui.rs`. They must either `pub use` from `kit`, or become a new
   `kit::CONST` (if the value is unique).
4. **Manual `Buffer::new` + `set_text` + `shape` in overlay logic** — only
   via `TextMeasurer` (or a `Shaper` behind a trait boundary, FR-068 W2).
   `set_text`+`shape_until_scroll` is allowed only inside kit/components and
   in `crates/canvas-render/src/text.rs` (that is the renderer itself).
5. **Text width heuristics** — `len() as f32 * factor + pad`. The CR-015
   defect class: different glyphs have different widths, Cyrillic is wider
   than Latin, emojis consume space. Only `TextMeasurer::width_of`.
6. **Manual `CardInstance { pos, size, fill, border, params, corners }`**
   in overlay logic — route through `Painter::rect/panel/control` or
   `KitDraw` (the Painter↔`Vec<CardInstance>` adapter). `CardInstance`
   directly — only in `canvas-render` (its type) and in the adapters
   `crates/canvas-app/src/app/support.rs` (the layer boundary).

### If something is missing from the kit

**The agent MUST propose a kit extension instead of working around it with
hardcode.**

Procedure:

1. **Identify the gap** — which component/token/slot is missing, on which
   pattern it repeats in several places (≥ 2 files → a kit candidate).
2. **Propose the kit extension** — in the task/PR:
   - **New component** → add to `crates/canvas-ui/src/component/`
     (or extend an existing one) + export via `kit.rs`. The F-8 contract of
     PRD-0009: palette slots only, token scale only, text only via
     `TextMeasurer`.
   - **New `KitPalette` color slot** → add the field to `KitPalette`
     (`crates/canvas-ui/src/component/mod.rs`) + the mapping in
     `ThemeColors` (`crates/canvas-render/src/theme.rs`) + presets
     (`crates/canvas-render/src/theme_presets.rs`) + the key in
     `REQUIRED_KEYS` (the semantics-parity test).
   - **New geometry token** → `crates/canvas-core/src/tokens.rs`
     (`SPACING_*`, `RADIUS_*`, control heights) + the mirror JSON in
     `design/tokens/` (the JSON↔Rust parity test).
   - **New layout pattern** (radio_card, chat_bubble, crumbs, tree_layout,
     anchored_stack, footer_buttons, chip_strip, two_column, backdrop,
     banner) → a component in `crates/canvas-ui/src/component/` + export
     via `kit.rs`.
3. **File an FR document** (if the extension is significant) using the
   `docs/change-requests/cr-template.md` template: What/Impact/Changes/Tests.
   Small extensions (a new palette slot) may live in the task commit without
   an FR.
4. **Implement the kit extension FIRST** — only then build the surface via
   the new kit component. Never the other way around.

### UI task review checklist

Before submitting a task touching UI (overlay/panel/dialog/control), the
agent verifies:

- [ ] Colors come from `KitPalette`; no inline rgba literals
  (grep `\[\s*0\.[0-9]+\s*,\s*0\.[0-9]+` in changed lines).
- [ ] Geometry from `kit::*` constants and `tokens::SPACING_*`/`RADIUS_*`;
  no new `const HEADER_H: f32 = 30.0` in `*_ui.rs`.
- [ ] Layout via `Column`/`Row`/`stack`/`constrain`/`pad`; no manual
  `x = panel.right() - MAGIC` formulas.
- [ ] Text via `TextMeasurer`; no `Buffer::new` in overlay logic and no
  `len() * factor` width heuristics.
- [ ] Quads via `Painter::rect/panel/control` or `KitDraw`; no manual
  `CardInstance { ... }` literals in overlay logic.
- [ ] If a new component/slot/token was added — it is in `canvas-ui`, not in
  `canvas-app` (per G7: the kit does not depend on render/OS; render depends
  on the kit, not the other way).
- [ ] If something was missing — the agent explicitly stated it in the task
  (an FR or a commit note) instead of silently hardcoding around it.

### Exceptions (acceptable hardcoding)

- **Test fixtures** — `KitPalette::default()` with `[0.0;4]` slots for
  geometry checks, `Color::rgba(...)` in tests (`*_ui.rs::tests`,
  `admin_ui.rs::test_palette`) — no UI meaning, asserts only.
- **Layer adapters** — `crates/canvas-app/src/app/support.rs::paint_items_to_band`,
  `crates/canvas-app/src/app/overlays.rs::KitDraw` — the Painter↔GPU-instance
  boundary; `CardInstance` is built here by right (this IS the
  kit→renderer adapter).
- **Render** — `crates/canvas-render/src/{cards.rs,renderer.rs,text.rs}` —
  the GPU backend; `CardInstance` is its own type; tokens
  (`tokens::EDGE_*`, `tokens::ACCENT`) are already canonical.
- **Diagnostic overlays** — `crates/canvas-app/src/debug_overlay.rs` — layer
  colors by design "diagnostic, not theme"; documented in the file header.
- **Specialized primitives** (polar wheel in template_ui, sector SDF in
  `crates/canvas-render/src/sectors.rs`) — an escape hatch via `Custom(rect)`
  with a justification comment (the G8 grep audit).

### References

- `docs/ui-kit.md` — the kit guide (3 steps to add a surface, layout
  primitives, text measurement, lints).
- `docs/prd/prd-0009-ui-layering-uikit.md` — UI/kit layer architecture.
- `docs/prd/prd-0006-design-system-tokens.md` — design tokens (FR-046).
- `docs/change-requests/fr-046-design-tokens.md` — tokens in code.
- `docs/change-requests/fr-051-ui-layering-uikit.md` — the UI layer (U1).
- `docs/change-requests/fr-053-ui-layering-u3-pilots.md` — layout
  primitives (U3).
- `docs/change-requests/fr-055-ui-layering-u4-kit.md` — kit v1 (U4).
- `docs/change-requests/fr-057-ui-kit-painter-widget-state.md` — Painter +
  WidgetState.
- `docs/dev-researches/ui-hardcode-audit.md` — the hardcode audit (kit gaps,
  migration order).

## Build and tests

After T0 the repository must support (CI on ubuntu/windows/macos — the M7
matrix; until it is activated — windows-latest in ci.yml + the 3-OS
build-all):

```
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
```

WASM gate (FR-036, ADR-0011): the core (canvas-core/canvas-render/
canvas-widgets) must build under wasm32-unknown-unknown, and canvas-core
must execute in a wasm runtime; the CI job `wasm-check` checks compilation
on every push, the local gate covers both parts. M8/W12 (wasm-port §6.1
item 4): the product web layer `canvas-web` is included in the compilation
stage:

```
scripts/wasm_gate.sh          # check the wasm target (core/render/widgets/mcp/web) + core rlib + canvas-core tests under wasip1 (wasmtime)
scripts/wasm_gate.sh --check  # compilation only — no wasmtime (equivalent of the CI job)
```

Web bundle and deploy (M8/W12): `scripts/web_bundle.sh` — a release build of
canvas-web (trunk 0.21.14, `[profile.release] lto="thin"`), `wasm-opt -Oz`
optimization and a size report (§8.8: ≤8 MB raw / ≤4 MB brotli; in CI the
final numbers are duplicated into $GITHUB_STEP_SUMMARY). Publishing to
GitHub Pages (the `/app` path + the Jekyll build of docs/ — like the earlier
branch deploy) — the `pages-web.yml` workflow (Source: "GitHub Actions", see
README "Веб-версия").

MCP-wasm gate (FR-037, ADR-0012): the contract layer (canvas-scene,
canvas-mcp, canvas-mcp-headless) builds under wasm32-unknown-unknown, tests
run under wasip1 in wasmtime, and the driver performs a REAL MCP session
(initialize → tools/list → graph_apply oracle ±1 % → analyze_bottlenecks
ρ-gate → negative branches) with the headless server — an ADR-0004 contract
regression is caught without Windows and GUI:

```
scripts/mcp_wasm_gate.sh           # check + wasip1 tests (scene 53 + bridge 13 + headless 12) + an e2e session
scripts/mcp_wasm_gate.sh --check   # compilation only — no wasmtime
```

Inspector session (FR-037 MW5) — a live manual MCP check by the owner
without Windows, using the official `@modelcontextprotocol/inspector`
(needs node 18+/npx; the first run downloads the package; not part of the
gates/CI):

```
scripts/mcp_wasm_inspector.sh           # web UI: browser → 127.0.0.1:6274, the server is pre-connected
scripts/mcp_wasm_inspector.sh --check   # automated acceptance by the inspector client: tools/list + graph_apply oracle ±1 %
```

Test requirements:
- Unit tests for `canvas-core` are mandatory (camera transform round-trips,
  lossless `.canvas` round-trip preserving unknown fields, parsing samples
  from jsoncanvas.org).
- `canvas-shell` — integration tests where possible (watcher on a tempdir
  etc.).
- Performance is part of acceptance: 5,000 nodes at 60 fps (pan/zoom), cold
  start < 2 s, opening a 1,000-node canvas < 500 ms, memory < 500 MB
  (SPEC §6.3); the `--stress N` load test (T5); 10 widgets must not drop fps
  below 60 (M5).
- Manual acceptance of milestone criteria (SPEC §10) is performed by the
  owner after each tag (v0.1–v1.1) — the agent does not substitute it.

## WASM UI self-check — mandatory before reporting

The rule (owner directive 2026-09-25): **any change touching UI (layout,
input, panels, hit tests, rendering, themes) is verified by the agent
itself on the web build** — native unit tests cover logic but do not
substitute behavioral verification on the wasm platform. Order:

1. L0 — the compilation gate: `scripts/wasm_gate.sh --check` (or a targeted
   `cargo check --target wasm32-unknown-unknown -p <crate>`);
2. native tests (`cargo test`) — logic;
3. L2 — the browser stand: `scripts/wasm_ui_test.sh` (canvas-web build
   without trunk + Chromium/WebGPU under Xvfb + a scenario with a pixel
   diff) — behavior. Scenario/coordinates — per the `docs/WASM-TESTING.md`
   §3 recipe.

If the environment does not allow L2 (no node/playwright/Xvfb, a broken
build, disk/time) — the task's final report MUST: (a) explicitly state that
the WASM check was not performed and why; (b) attach manual-verification
instructions: what to open (the Pages web version / `trunk serve`), where
to click, what counts as success. The phrase "native tests are green" by
itself does not close UI acceptance.

## Do not

- Do not change the `.canvas` format incompatibly with jsoncanvas.org
  without an explicit task; unknown fields and node types must survive the
  round-trip.
- Do not add network calls to the host.
- Do not block the render thread (see above).
- Do not send 0x052C when WorkerW exists; do not use SPI_SETDESKWALLPAPER on
  a raised desktop.
- Do not replace the shell (taskbar, tray remain Explorer's).
- Do not spread platform code across `canvas-app` — traits and cfg-sections
  of platform crates only (see "Architecture rules" item 2); unix
  equivalents of Win32 techniques — only per the decision table
  `docs/plans/M7-crossplatform.md` §3.2, not from memory.


