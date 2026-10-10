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

## Agent navigation layer

For task-based routing use: `docs/agent/MAP.md` (what to read per task type),
`docs/agent/routes.yaml` (task type → skill/brief → gates), the workflow
skills — `docs/agent/skills/` ($docs-links, $worklog-rotate, $issue-workflow,
$ui-kit-review, $shell-win32, $wasm-test), briefs — `docs/agent/briefs/`
(template: `docs/agent/BRIEF-TEMPLATE.md`), agent-workflow terms —
`docs/agent/glossary.md`, the eval task set — `docs/agent/evals/`. The
workflow skills decompose the detailed sections of this file; load the one
matching your task type.

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

The full map (task → documents → gates) — `docs/agent/MAP.md`; routing
(task type → skill/brief) — `docs/agent/routes.yaml`. Verify against these
documents, not your own memory — APIs change, undocumented Windows behavior
differs across OS versions (SPEC §7.4: the Progman/WorkerW/DefView hierarchy
in Win11 24H2+ differs from the classic one).

Core entry points: `docs/SPEC.md` (read before any task), `docs/TASKS.md`
(order within a milestone is strict), `docs/adr/` (an ADR is written BEFORE
implementing the decision; `Статус: принято` — only on the owner's direct
request, ADR-0001 rule 2), `docs/change-requests/` (implementation follows
the FR/CR document only; on completion update its status, Changelog and
`docs/ACCEPTANCE.md`; active index only — do not read the archive without a
task), `docs/plans/product-roadmap.md` (order of FR/CR work and new
dependencies), `docs/RECIPES.md` (mandatory before T15–T18 and shell
tasks), `docs/DEMO.md`, `docs/WASM-TESTING.md`.

**`skills/` (MCP package):** the tool counter lives ONLY in
`skills/README.md` — consult it, not memory. Any change to MCP tool
set/semantics (`TOOLS` in canvas-mcp, tool logic in canvas-scene) must
update `skills/` in the same commit; the `skills_*` contract tests fail CI
on drift. Protocol — `skills/UPDATE-PROTOCOL.md`.

Link naming: within the repository, links to md sources — relative `*.md`
only; `*.html` only for real on-disk artifacts and inside `user-docs/`
(FR-031). Gate: `python3 scripts/doc_lint.py` (CI job `docs-lint`: links,
backtick paths, file budgets — 0 errors required).

## Mandatory question for every FR/CR: onboarding and documentation

**When finishing any FR or CR implementation, the agent MUST explicitly ask
the owner whether onboarding and user documentation need updating.** The
question goes into the task's final report — the owner's silence does not
cancel the need. What to check (onboarding tour steps, `user-docs/` pages,
hotkeys sync) — the $docs-links skill (`docs/agent/skills/docs-links/SKILL.md`).
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

Undocumented techniques (WorkerW, messages 0x052C and 0x7402) — only with
runtime detection and a fallback to a normal window, per `docs/RECIPES.md`;
each carries a comment linking the recipe. Claims about
Progman/WorkerW/DefView behavior not confirmed by RECIPES or runtime
detection are unverified. Recipes are ported cleanly (Lively — GPL-3.0,
Seelen UI — AGPL-3.0: mechanics only, no copied code/identifiers/comments).
Any desktop-embedding step failure → fallback to a normal window + a warning
to the user. The full rule set (R2/R3/R4/R5/R9/R10/R13 constraints, Win32
enum idiom) — the $shell-win32 skill
(`docs/agent/skills/shell-win32/SKILL.md`).

## Security and widget rules (M5)

Widgets — local packages only (`%APPDATA%/canvasdesk/widgets/<id>/`),
installed explicitly by the user; a remote URL as a widget — forbidden
architecturally. Permissions from `widget.json` are checked on EVERY bridge
call; navigation outside the package is forbidden; an invalid bridge
message = drop + warn, never a panic. No network calls in the host — the
network exists only inside widgets with the `network` permission. The full
rule set — the $ui-kit-review skill, section "Widget security (M5)".

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

**Tasks first — then code; an issue is closed only after verification.** Any
improvement starts with a high-level GitHub issue BEFORE the first line of
code — work without an issue does not start. A high-level issue is the
atomic unit of value: no issue per intermediate step (cosmetics and subtasks
live inside the task). Waves/stages/parallel sub-agent splits — as
sub-issues (progress via the Sub-issues field). The board — Projects #1
(CanvasDesk): Todo → In Progress → Done, one wave In Progress at a time.
Closing = gates green (`cargo test --workspace`, clippy `-D warnings`, fmt,
wasm gates per context) + acceptance criteria met + UI changes passed the
WASM check, then a summary comment (what done, key commits, verification).
The tool — `scripts/github_tasks.py` (idempotent; `plan` creates issue +
sub-issues + board in one step; tokens `GITHUB_TOKEN` / `GITHUB_PROJECT_TOKEN`):

```
GITHUB_TOKEN=ghp_... python scripts/github_tasks.py plan --spec plan.json --dry-run
```

Full procedure — the $issue-workflow skill
(`docs/agent/skills/issue-workflow/SKILL.md`).

## Per-task token accounting (development cost tracing)

**Every completed task — a CR/FR implementation, defect fix, document,
research — ends with recording the tokens spent** (owner rule 2026-10-08).
Counted per task scope (not per session; sub-agents report the same way);
the record is the last line of the task's worklog entry:

```
Tokens: in≈<N>, out≈<N>, total≈<N>, model=<models>, scope=<CR-019|FR-100|...>
```

The agent has no direct API counter — provide a **justified estimate** marked
`estimate` (~4 chars ≈ 1 token for mixed RU/EN/code, plus session-context and
tool-call overhead). Estimates accumulate as calibration data. Worklog
rotation (current period ~7 days, Journal index, lossless archive moves) —
the $worklog-rotate skill (`docs/agent/skills/worklog-rotate/SKILL.md`).

## UI kit — mandatory rule for Rust UI work

**Any Rust UI work goes through the existing `canvas-ui` kit**
(`crates/canvas-ui/src/`). Hardcoding quads/colors/geometry bypassing the
kit is forbidden — it breaks theme consistency and reuse, and makes visual
regressions invisible until manual testing.

Before touching UI code, load the $ui-kit-review skill
(`docs/agent/skills/ui-kit-review/SKILL.md`): the "what to use from the kit"
table (KitPalette slots, `tokens::SPACING_*`, kit control sizes, layout
primitives, `TextMeasurer`, `Painter`), the forbidden patterns (inline rgba
literals, magic geometry numbers, duplicated local constants, manual
`Buffer::new`/`CardInstance` in overlay logic, `len() * factor` width
heuristics), the kit-extension procedure (identify gap → propose → FR if
significant → implement the kit extension FIRST), the UI task review
checklist, the acceptable exceptions, and the M5 widget-security rules.
Guide — `docs/ui-kit.md`; architecture —
`docs/prd/prd-0009-ui-layering-uikit.md`; tokens —
`crates/canvas-core/src/tokens.rs`.

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

Owner directive 2026-09-25: **any change touching UI (layout, input, panels,
hit tests, rendering, themes) is verified by the agent itself on the web
build** — native unit tests cover logic but do not substitute behavioral
verification. Order: L0 — the compilation gate `scripts/wasm_gate.sh
--check`; native tests (`cargo test`) — logic; L2 — the browser stand
`scripts/wasm_ui_test.sh` (behavior; scenario per `docs/WASM-TESTING.md` §3).
If the environment does not allow L2 — the task's final report MUST (a)
explicitly state the WASM check was not performed and why, and (b) attach
manual-verification instructions (what to open, where to click, what counts
as success). "Native tests are green" alone does not close UI acceptance.
Details, gates and pitfalls — the $wasm-test skill
(`docs/agent/skills/wasm-test/SKILL.md`).

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


