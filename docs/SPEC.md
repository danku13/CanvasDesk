# CanvasDesk — project specification

Working title: **CanvasDesk**. **A visual mathematical modeling system**
(ADR-0007): an infinitely zoomable canvas on which executable mathematical
models are built from computational nodes (Numi sheets, templates), values
spill along value edges, domain mathematics is built into the core, and an
AI agent assembles and verifies models via MCP. The model's carrier is a
file canvas over the real file system and (in M4 mode) instead of the
standard desktop.

> This document describes the infrastructure specification (the carrier).
> The computational core (Numi, value flow, templates, domain units) —
> the FR-013…FR-029 wave (index — `docs/change-requests/index-cr-fr.md`),
> decisions — `docs/adr/` (start with ADR-0007, then ADR-0002…ADR-0006).

---

## 1. Scope

**In scope:**
- M1 — canvas core: camera (pan/zoom), file cards, system thumbnails, saving to JSON Canvas
- M2 — text notes, edges between nodes, drag-drop from Explorer, file system watcher
- M3 — live previews (images, PDF, text/code, Office via preview handlers), minimap, search
- M4 — desktop embedding mode (WorkerW): the canvas behind the icons, hiding system icons, context menu interception
- M5 — extension engine: widgets as JS/HTML micro-frontends on the canvas (WebView2, manifest, bridge, sandbox)
- The modeling wave (FR-013…FR-029) — the Numi engine in notes, value flow
  over value edges (DAG), domain units and queueing functions, the template
  library (45), the palette/wheel UI, per-line outputs, unit economics,
  onboarding and built-in documentation. FR specifications live in the FR
  documents themselves; architectural decisions — `docs/adr/` (ADR-0002…ADR-0007);
  the make-up and acceptance of the composition concept — CR-013
- M7 — cross-platform: Windows 10/11, Linux (X11/Wayland), macOS — builds,
  CI gates and platform implementations (thumbnails, drag-drop, MCP) per the plan
  `docs/plans/M7-crossplatform.md`

**Deliberately out of scope:**
- Replacing the shell (taskbar and tray stay Explorer)
- Desktop mode (embedding into the desktop) outside Windows — Unix
  equivalents (layer-shell etc.) are postponed until after v1.2; windowed
  mode — on all OSes
- Live widgets on Linux/macOS until M5 (T21/T22) is complete: outside
  Windows a widget node renders as a snapshot/placeholder
- Cloud sync, multi-user mode
- Editing document content inside the canvas
- A public widget catalog/marketplace, cloud widgets, remote JS download —
  widgets are local-only packages installed explicitly by the user

## 2. User scenarios

1. The user drags a project folder onto the canvas → files are laid out in a grid, thumbnails are visible
2. The user groups the project's documents in space, connects them with edges, adds context notes
3. Zooms into a PDF card → the card becomes a readable preview of the first page
4. In M4 mode: PC boot → the last canvas opens instead of the standard desktop; double-clicking a file opens it in the associated application
5. A file is renamed in Explorer → the card updates; a file is deleted → the card is marked "broken link", it does not silently disappear
6. The user places a widget on the canvas (a clock, a calendar, a custom dashboard from a Vite build): the widget lives as a node — it moves, connects with edges, becomes interactive when zoomed in, turns into a static snapshot when zoomed out

## 3. Technology stack

| Layer | Choice | Why |
|---|---|---|
| Language | Rust stable (1.80+), edition 2021 | — |
| Window/input | `winit` 0.30 | Cross-platform foundation, touchpad gestures |
| GPU render | `wgpu` 22+ | DX12/Vulkan backends, batching, future portability |
| Text | `glyphon` (over `cosmic-text`) | Native wgpu integration, shaping, emoji |
| Spatial index | `rstar` (R-tree) | Hit-testing, viewport culling at 5–10 thousand nodes |
| Canvas format | JSON Canvas spec 1.0 (`serde_json`) | Obsidian compatibility, human-readable |
| Statistics (L2, ADR-0008) | `statrs` 0.17 + `rand`/`rand_chacha`/`rand_distr` — behind the cargo feature `stats` in `canvas-core` (FR-063) | Distributions, quantiles, confidence intervals, deterministic RNG (ChaCha8, seed `FNV-1a(content) ⊕ scenario_seed`); a B2B build without the feature — zero-dep |
| Metadata/cache | `rusqlite` (bundled) | Thumbnail cache, search index, sessions |
| File watcher | `notify` 6+ | Three backends behind one API: ReadDirectoryChangesW (Win), inotify (Linux), FSEvents (macOS); differences are normalized in canvas-shell |
| Win32/COM | `windows-rs` (features: Win32_UI_Shell, Win32_Graphics_Dwm, System_Com) | Thumbnails, preview handlers, WorkerW — the Windows layer |
| Thumbnails | `IShellItemImageFactory::GetImage` | System cache, matches Explorer |
| PDF | `pdfium-render` (pdfium binary, BSD license) | Fast page rendering to bitmap |
| Images | `image` | Decode to RGBA → GPU texture |
| Preview handlers | `IPreviewHandler` in an out-of-process host | Isolation of crashing COM components |
| Widgets (M5) | WebView2 Evergreen + `webview2-com` | Live HTML/JS nodes; snapshots via `ICoreWebView2::CapturePreview` |
| Widget bridge | `postMessage` JSON-RPC (`serde_json`) | A narrow typed host API with permissions, no eval |
| Config serialization | `serde` + `toml` | — |
| Logging | `tracing` + `tracing-subscriber` | Diagnostics on user machines |
| WASM targets (FR-036, ADR-0011) | `wasm32-unknown-unknown` (product, plan M8) + `wasm32-wasip1` (service test target, wasmtime) | The core and the MCP layer (core/render/widgets/mcp/scene/headless, FR-037) must build and run under wasm: gates `scripts/wasm_gate.sh` + `scripts/mcp_wasm_gate.sh`, CI `wasm-check` |
| Packaging | `cargo-wix` → MSI, Authenticode signing | M4 requires system trust |

## 4. Workspace structure

```
canvasdesk/
├── Cargo.toml               # workspace
├── crates/
│   ├── canvas-core/         # модель данных, JSON Canvas I/O, spatial index, Numi-движок (expr/),
│   │                        #   DAG-поток значений (flow.rs), валидация модели (validate.rs, FR-032),
│   │                        #   реестр шаблонов (templates.rs) — без ОС/GPU
│   ├── canvas-render/       # wgpu-рендер: камера, батчинг, текст, текстуры, LOD
│   ├── canvas-shell/        # Windows-only: тамбнейлы, preview handlers, drag-drop, WorkerW (cfg(windows))
│   ├── canvas-preview-host/ # отдельный exe — песочница для IPreviewHandler
│   ├── canvas-widgets/      # M5: WebView2-хост, bridge, манифесты, снапшоты (cfg(windows))
│   ├── canvas-mcp/          # MCP-посредник: stdio JSON-RPC ↔ named pipe, 41 инструмент канваса (FR-032: edges_list/edge_get/graph_validate; FR-033: graph_apply; FR-016: analyze_bottlenecks; FR-017: whatif_*; PRD-0008 Q5: schemes_list/schemes_apply; PRD-0007 X2/FR-048: lineage; FR-066: monte_carlo_run — native-only, §5.8)
│   ├── canvas-scene/        # модель сцены (SceneState) + mcp_dispatch — платформенно-нейтральный, wasm (FR-037/ADR-0012)
│   ├── canvas-mcp-headless/ # headless MCP-сервер для wasmtime/wasip1 (FR-037) — верификация сессий без Windows
│   └── canvas-app/          # приложение: event loop, команды, UI-состояние, mcp_dispatch, main()
├── assets/                  # шрифты, иконки нод, виджеты (widgets/), шаблоны (templates/)
├── docs/                    # SPEC.md, TASKS.md, RECIPES.md, adr/, change-requests/
└── AGENTS.md                # инструкции для агента
```

Boundary rule: `canvas-core` imports nothing from `canvas-shell` and `canvas-render`. All platform logic sits behind traits (`ThumbnailProvider`, `PreviewProvider`, `ShellIntegration`), so that core is testable on any OS.

## 5. Data model

### 5.1. The canvas file — `*.canvas` (JSON Canvas 1.0)

```json
{
  "nodes": [
    { "id": "n1", "type": "file", "file": "C:/Projects/alpha/spec.pdf",
      "x": 120, "y": 80, "width": 340, "height": 440 },
    { "id": "n2", "type": "text", "text": "Согласовать до пятницы",
      "x": 520, "y": 80, "width": 260, "height": 120, "color": "3" },
    { "id": "n3", "type": "group", "label": "Альфа-проект",
      "x": 60, "y": 20, "width": 800, "height": 600 },
    { "id": "n4", "type": "widget", "x": 920, "y": 80, "width": 320, "height": 200,
      "canvasdesk": { "widgetId": "com.example.clock", "props": { "timezone": "Europe/Moscow" } } }
  ],
  "edges": [
    { "id": "e1", "fromNode": "n2", "fromSide": "right",
      "toNode": "n1", "toSide": "top", "label": "блокирует" }
  ]
}
```

Extensions on top of the spec (stored in nodes, ignored by other applications — in the `canvasdesk` field or per spec convention):
- `file` allows absolute Windows paths (the spec describes paths inside a vault; we document the deviation)
- `previewState`: `thumbnail | live | none` — the node's last level of detail
- `brokenLink: true` — the file is unavailable, the card is kept with a gray border
- `type: "widget"` + the `canvasdesk: { widgetId, props }` object — a widget node (M5, §7.6); applications that do not know the type skip such a node, the file stays valid
- `canvasdesk: { expr }` on a text node — the Numi formula of a calc node (FR-013); the result is computed by the application and is not written to the file (invariant 4)
- `canvasdesk: { flow: { kind: "value" | "control" } }` on an edge — the flow type (FR-014). `value` — the edge carries the source's value into `$in`/`$1..$N` of the downstream formula; the field's absence and `control` — a visual edge (the default, backward compatibility). The value-edge graph is a DAG: cycles are blocked at creation (a dialog with a fallback to control in the UI, isError in MCP). Recalc results (live, the propagator `canvas-core/src/flow.rs`) are not written to the file
- `fromLine: <uint>` on an edge — a per-line source (FR-025): the value edge carries the value of formula line `fromLine` of the source's Numi sheet (not the node's total value). Field absence — the node's value (current behavior; old files unchanged); broken values (`-1`, fractional) are read as absence. Created by a drag from a per-line port (the `line_ports` feature flag in settings, default off); rebinding the from-end clears the field
- `fromOutput: "<имя>"` on an edge — a named source (FR-029): the value edge carries the value of the source's named output (a template node — the `outputs` section of the manifest/snapshot; a text node — a variable of the Numi sheet, addressing survives line shifts). Mutually exclusive with `fromLine` (checked by MCP; the model priority — `fromLine`)
- `toParam: "<имя>"` on an edge — a spill into a parameter (FR-029): the value edge substitutes its value into `$<имя>` of the sink template node, OVERRIDING the parameter's local value ("the spill beats the default"), without editing the formula. Edges with `toParam` do not occupy the positional slots `$1..$N`; several edges into one parameter — the last one in `canvas.edges` wins (a warning in `flow_recalc`; strict diagnostics — FR-032 `graph_validate`)
- `canvasdesk: { pin_ports: ["from", "to"] }` on an edge — pinned connection ends (CR-008). An unpinned end connects to the shortest-path port (`best_sides`, recomputed when nodes are dragged and on auto-layout — not written to the file); a pinned one follows the saved `fromSide`/`toSide`. The array may contain one or both ends; empty/absent — both ends auto. Removing the last pin deletes the field (a clean round-trip)
- `canvasdesk: { template }` on a text node — a snapshot of the template reference (FR-018): `{ id, version, expr, params: { имя: { num, unit? } }, icon, color, outputs? }` — `outputs` (FR-029, manifest schema 1.1): `[{ name, unit?, line | expr }]`, named outputs addressable by `fromOutput` edges; the key is written only when the section is non-empty (round-trip of old files). `expr` — a Numi formula with `$param` references (the result goes to the card footer and the FR-014 flow, not to the file); `icon`/`color` — snapshots of the role/category (header rendering without the registry). The node text is a Numi sheet of parameter assignments; editing the text syncs `params`. The field survives round-trips (a snapshot, not a registry reference)
- MCP graph reading and validation (FR-032) — runtime, not written to the file: `edges_list`/`edge_get` return the canonical edge schema `{id, from, to, kind, fromLine?, fromOutput?, toParam?, fromSide, toSide}` (FR-029 port addressing); `graph_validate` — a report `{valid, issues: [{severity, code, node_id, edge_id, message}]}` from the pure function `canvas-core/src/validate.rs`. The codes are a stable contract for the agent recipe: `E-CYCLE` (a value-edge cycle), `E-OVERLOAD` (ρ ≥ 1), `W-AMBIGUOUS-SRC` (a multi-line source without line/output addressing), `W-UNUSED-SLOT` (a positional input `$N` not read by the formula), `E-UNIT`/`E-PORT-UNKNOWN`/`E-DOUBLE-INPUT` (the FR-029 port contract — implemented at the CP1 merge)
- `canvasdesk: { whatif: { scenarios: [{ name, overrides: [{ node, line, expr }] }] } }` in `Canvas.extra` (FR-017, CP6) — persistent what-if scenarios (limit 3): per-line substitutions `(id ноды, индекс строки текста) → новый исходник`. The active scenario's substitutions are runtime-only: the propagator computes against the virtual source (`canvas-core/src/flow.rs`, `whatif_virtual_text`), `.canvas` is not mutated without Apply; `whatif_apply` writes the substitutions into lines/params and deletes the scenario (one undo step). Stale substitutions (node/line deleted, line became prose) are silently skipped by the recalc and marked in the overrides list; an empty scenario list deletes the field (a clean round-trip of old files)
- **Freeze (PRD-0007/FR-048, G6): the `.canvas` format is NOT extended** — the number's lineage tree (`LineageTree`), the chain verification window, highlighting, breadcrumb chips, the protection mode, the session cache, the coverage indicator — computed/runtime data, not written to the file; what-if scenarios from the tree are serialized by the existing `canvasdesk.whatif` (see above). The Obsidian round-trip stays clean

### 5.2. SQLite (`~/.canvasdesk/cache.db`)

- `thumb_cache(file_path, mtime, size_class, blob_hash)` — invalidated by mtime
- `search_index(file_path, display_name, extracted_text_fts5)` — FTS5 for Ctrl+F
- `sessions(canvas_path, camera_x, camera_y, zoom, opened_at)` — view restoration

### 5.3. Single source of truth rule

Layout (coordinates, sizes, edges) — only in the `.canvas` file. SQLite is a recreatable cache; deleting it breaks nothing.

**Scheme assets (`assets/canvas-schemes/*/scheme.json`, FR-049):** the built-in
template library of ready-made schemes — a metadata manifest (id
`com.canvasdesk.scheme.*`, bilingual name/description/category, version) +
a graph `content.nodes[]`/`content.edges[]` in a subset of JSON Canvas (types
`text`/`group`, the group `label` — the standard header field, color — the
presets `1..6` only, value edges `flowKind: "value"` with FR-025/FR-029 addressing:
`fromLine` — a 0-based source line, `fromOutput` — a named output of an
assignment, the fields are mutually exclusive; `toParam` — spilling a value into
the sink's `$параметр`). The `.canvas` format is not extended: the instancer
(`canvas-scene::scheme_apply`) remaps ids and moves the addressing fields into
standard canvas edges; the registry validator (`canvas-core::schemes`) —
limits ≤ 200 nodes / ≤ 400 edges, mandatory fields, addressing on value
edges only, `fromLine XOR fromOutput`. Scheme content follows the invariants of
PRD-0008 §7.2.1 (node descriptions, main stage bundles, multiple edges,
`scheme_apply` autotests). Schemes are compiled into the binary (`include_dir`,
wasm gate ADR-0011); scheme metadata is not written to the canvas file.

## 6. Render architecture

### 6.1. Frame

```
input → camera update → world-space culling (rstar query по viewport)
→ LOD assignment по zoom → batch build (quads, текст, текстуры)
→ один render pass → minimap pass (offscreen → corner quad)
```

### 6.2. Levels of detail (LOD)

| Zoom | Content of a file card |
|---|---|
| < 0.25 | A colored rectangle + the file type icon |
| 0.25–0.6 | + system thumbnail + file name |
| 0.6–1.5 | + content preview (image, first PDF page, first N lines of text) |
| > 1.5 | A live preview with scrolling (only for nodes under the cursor/in focus, at most 3 at a time) |

Live previews are expensive (1024²+ textures), their count is hard-limited; when zooming out the texture is freed, the thumbnail remains.

Widgets (M5) — a separate LOD strategy: zoom < 0.25 or outside the viewport → a snapshot texture; visible and zoom ≥ 0.25 → a live WebView2, the live instance limit per §7.6.

**Structural edge aggregation (FR-042)** — a zoom-independent LOD: edges of one ordered node pair (N ≥ 2) are drawn as a single aggregated line with continuous thickness by weight (`d = clamp(1.8 + (N−1)·0.9, 1.8, 8.0)`) and a multiplicity badge `×N`; single edges — the previous look. Bundle detail — the modal "main stage" mode (§8). Disabled by the "Edge aggregation" setting.

### 6.3. Performance — target metrics

- 60 fps while panning/zooming on a scene of 5 000 nodes (a GTX 1050 / Iris Xe-class GPU)
- Cold start to the first frame < 2 s
- Opening a 1 000-node canvas < 500 ms (thumbnails — asynchronously, cards appear immediately)
- Memory < 500 MB at 5 000 nodes without live previews
- Lineage tree construction (PRD-0007/FR-048, F-2): pure assembly ≤ 100 ms for
  1 000 nodes / 3 000 edges (budget `LINEAGE_MAX_NODES` = 4096 nodes,
  iterative traversal); in the UI — asynchronously (native — a background thread, G5),
  reopening from the session cache ≤ 1 s (G1); the chain coverage
  indicator (F-12, opt-in) — recomputed per model revision, not per frame
- Value flow recalc (FR-014/FR-064): the full `propagate_with_lines` ≤ 10 ms per
  1 000 nodes. Native — heavy runs (baseline + the active what-if) on a
  worker thread with a double buffer `Arc<RwLock<FlowSolutions>>`; on the UI thread —
  an O(N) output pass (`expr_results`/`analyze`/`bundles`/diff); the live invariant:
  edit → result within 1–2 frames (wake `AppEvent::FlowReady`).
  Degradation = a sync recalc on the UI thread + `warn` (refusal/3 s timeout/worker
  panic; on wasm — the normal sync path). Determinism: the worker path yields
  bitwise-identical numbers to sync (golden tests `worker_smoke.rs`).
- DAG recalc `flow::propagate_with_lines_data` (FR-013/014/029): the single-threaded
  flatten path ≤ 10 ms per 1 000 nodes (the ADR-0005/0006 reference models are ≤45 nodes — under
  10 ms); the parallel path (the `parallel` feature, `rayon` `par_iter` over the tiers of
  `topo_levels` FR-065) — a win on heavy modes (FR-017 scenario batches of the v2 grid with 20+ runs,
  Monte Carlo FR-066 10⁴×1000 nodes), not on a single
  reval. The ADR criterion (§9 M4): ≥2× per 1 000 nodes / 4 cores. Bitwise
  identical to the single-threaded path (the §5.7.3 collect-then-reduce contract).
- MC/QMC engine (FR-066, the `qmc` feature, `flow::propagate_monte_carlo`): 10⁴
  runs of the ADR-0006 reference model №5 (unit economics, ~45 nodes) — **< 1 s / 4 cores**
  (chunks of 256 per task, rayon; the gate test `mc_perf_10k_etalon5_under_budget`,
  canvas-core). Seed reproducibility is first-class: the same `McConfig.seed` →
  bitwise-identical P50/P90/P99 quantiles (the gate test
  `mc_seed_reproducibility_is_bitwise`).

### 6.4. Textures

Atlas manager: thumbnails are packed into 2048² atlases (LRU eviction), previews — separate textures. RGBA8 format, no mipmaps needed (the LOD is discrete).

### 6.5. DPI

The application manifest — **Per-Monitor V2** DPI awareness. All canvas coordinates are in logical pixels (world-space), rendering — in physical ones (`scale_factor` from winit); when the window moves between monitors with different scales — the surface is recreated and text sizes are recomputed. Do not rely on the system's DPI-virtual scaling (bitmap-stretch) — the text will be blurry.

### 6.6. Palette and design tokens (PRD-0006, FR-046)

The single source of truth for visual characteristics — a three-layer design token system:

1. **Primitives** — `design/tokens/{colors,dimensions,motion}.json` (a structure in the spirit of W3C Design Tokens, `$type`/`$value`/`$desc` with source coordinates). The mirror — the platform-neutral `canvas_core::tokens` (data, wasm-compatible); a JSON↔Rust divergence is caught by parity tests (FR-046 I-5).
2. **Semantics** — `canvas_render::theme::ThemeColors`: palette slots (background/grid/cards/menus/text/GFM/groups/guides FR-038 + v2: `accent`, `selection_fill`, `highlight`, `whatif_fill`, `whatif_badge`, `error`, `hud`); both palettes (`dark()`/`light()`) are assembled from the primitives. Contrast is guaranteed by the `contrast.rs` machine + tests (CR-007); the known exceptions are documented and have regression boundaries.
3. **Consumers** — the accent family (selection/edges/draft/widget chrome/drop-ghost/selection frame/groups) — aliases of the `ACCENT` primitive (a single source, G4); minimap/wheel/dialogs/toasts — values from the tokens; card metrics (`HEADER_HEIGHT`, `CORNER_RADIUS`) and typography — aliases of `dimensions.json` (the PRD-0004 F-1 seam).

Flow rule: color reaches the shader only from an instance/uniform filled from `ThemeColors`/tokens; a literal in a builder = a defect — caught by `scripts/token_lint.sh` (a hex outside tokens/Win32 domains/tests/data contracts = an error). Data domains with their own hex contracts (template manifests FR-018 — `templates::DEFAULT_TEMPLATE_COLOR`) are outside the render palette. Theme presets — data in `design/tokens/themes/*.json` (37 semantic slots of `ThemeColors` — including `stage_dim` FR-042) + the `canvas_core::theme_presets` registry (key-set validation, a `OnceLock` cache); the selection — `Settings.theme_preset`, the effective palette — `ThemeColors::from_settings(theme, preset)` (FR-047, stage D4 of PRD-0006: Nord, Dracula, Catppuccin Mocha/Latte, Solarized Light, Tokyo Night, Gruvbox Dark; the contrast of each preset is pinned by G3 tests). Custom palettes and VSCode/Obsidian theme import — the PRD-0006 §13 roadmap (V2).

## 7. Shell integration (crate `canvas-shell`)

### 7.1. Thumbnails

`IShellItemImageFactory::GetImage(SIIGBF_BIGGERSIZEOK | SIIGBF_THUMBNAILONLY)` → HBITMAP → RGBA → atlas. Requests go to a thread pool (4), results come back through a channel to the render thread. Queue priority: visible nodes → the closest to the viewport.

### 7.2. Preview handlers (M3)

A separate process `canvas-preview-host.exe`:
1. The parent passes the file path and size over a named pipe
2. The host resolves the `IPreviewHandler` by CLSID from the registry, renders into an offscreen window, copies the bitmap, returns the pixels
3. A 3 s timeout → kill the process, fall back to the thumbnail
4. A host crash does not affect the canvas; the host is restarted on the next request

### 7.3. Drag-drop from Explorer (M2)

`IDropTarget` on the window: we accept `CF_HDROP` and `FileGroupDescriptor`. Dropping a folder → a recursive walk (depth 1), auto-layout in a grid with a step based on the card size, starting from the drop point.

### 7.4. Desktop mode (M4)

**The desktop window hierarchy differs between versions — this is the key implementation fork:**

- **Win10 / Win11 ≤ 23H2 (the classic scheme):** we send `Progman` the `0x052C` message → Explorer spawns a top-level `WorkerW` behind `SHELLDLL_DefView`; we make our window a child of that `WorkerW` via `SetParent`.
- **Win11 24H2 / 25H2 (build ≥ 26100, including the target 26200):** Explorer changed the background rendering for the sake of HDR wallpapers. `Progman` is created with `WS_EX_NOREDIRECTIONBITMAP`, `SHELLDLL_DefView` is a `WS_EX_LAYERED` child window of `Progman`, and `WorkerW` is a child window of `Progman` **below** DefView in Z-order. The `0x052C` message no longer separates DefView into its own top-level `WorkerW`. The working strategy: our window is a **`WS_EX_LAYERED` child of `Progman` with a Z-order between DefView (top) and WorkerW (bottom)**, aligned via a series of `SetWindowPos` calls (`SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOMOVE`). DefView is almost fully transparent and draws only the icons and text on top of us.

**The embedding sequence:**

1. Determine the strategy from the actual window hierarchy (detection, not guessing by build number)
2. The classic scheme: `SendMessageTimeout(0x052C)` → find the top-level `WorkerW` (by enumerating `EnumWindows`, the one whose neighbor is `SHELLDLL_DefView`) → `SetParent(наше_окно, workerw)`
3. The 24H2+ scheme: create our window as a child of `Progman` with the `WS_EX_LAYERED` style, set the Z-order: `DefView` → our window → `WorkerW`
4. The window: spanning the whole virtual screen (multi-monitor via `EnumDisplayMonitors`), borderless, `WS_EX_NOACTIVATE` until the first click
5. Hiding the system icons: `SHELLDLL_DefView` + `WM_COMMAND 0x7402` (toggle) — we save the original state and restore it on exit
6. The desktop context menu: intercepting `WM_RBUTTONUP` on our window → our own menu (Open canvas / New file / Show icons / Exit)
7. A double-click on a file node → `ShellExecuteEx` with `SEE_MASK_INVOKEIDLIST` (the "like in Explorer" behavior)

**The strategy table (the version gate):**

| Version | Build | Embedding strategy | Status |
|---|---|---|---|
| Windows 11 25H2 | 26200 (dev machine: 26200.9168) | The 24H2+ scheme (layered child of Progman) | **Основная цель разработки** |
| Windows 11 24H2 | 26100 | The 24H2+ scheme | Обязательная проверка |
| Windows 11 23H2 | 22631 | The classic one (top-level WorkerW) | Проверка на VM |
| Windows 10 22H2 | 19045 | The classic one | Проверка на VM |
| Unknown / newer | — | Runtime detection of the hierarchy (is there a top-level WorkerW with a DefView neighbor) → scheme selection; on failure → windowed mode with a warning | Фолбэк |

Each strategy is a separate module behind the common `DesktopEmbedder` trait. Reference implementations to study before coding: Lively Wallpaper (C#, supports 24H2+), Seelen UI (Rust) — links in §11. **A detailed breakdown of both projects and the ready recipes R1–R17 are in `docs/RECIPES.md`, mandatory reading before implementing this section and the §7.4-related tasks.**

### 7.5. File watcher (M2)

`notify` with `RecursiveMode::NonRecursive` on every directory that has nodes. A 300 ms debounce. Events:
- `Modify` → invalidate the thumbnail cache, update the card
- `Rename` → if the path matches a node — update `file` in the model (and in `.canvas`)
- `Remove` → `brokenLink: true`, the card goes gray, edges are preserved

### 7.6. Widget engine (M5)

**Concept.** A widget is a self-contained micro-frontend: a folder with a `widget.json` manifest and static files (HTML/JS/CSS — the build of any framework: React, Svelte, vanilla). The canvas is the host orchestrator: it places the widget as a node, manages the life cycle, isolates it. The user can put any JS/HTML object on the canvas — from a clock to their own micro-app (a funnel dashboard, a contacts panel, a mini-tracker).

**The `widget.json` manifest:**

```json
{
  "id": "com.example.clock",
  "name": "Clock",
  "version": "1.0.0",
  "entry": "index.html",
  "defaultSize": [320, 200],
  "permissions": ["canvas:read", "shell:open", "fs:read", "network"]
}
```

**Render.** WebView2 (Evergreen Runtime):

- A live widget gets one WebView2 in a child HWND positioned exactly over the node's rect; camera sync — `SetWindowPos(SWP_ASYNCWINDOWPOS)` every frame (cheap), corner rounding — a region on the HWND
- **The airspace constraint:** the WebView2 HWND draws over the wgpu canvas → canvas overlays (minimap, search panel, context menus) must not overlap live widgets or are drawn as separate layered windows. A documented architectural limitation
- **LOD:** zoom < 0.25 or the node outside the viewport → the WebView2 is hidden and suspended, a snapshot texture instead (`CapturePreview` on a change event or every 5 s for animated ones); zoom ≥ 0.25 and visible → a live instance
- **The live instance limit** (6 by default, configurable): LRU — long-unseen ones are moved to a snapshot. One user-data-folder per application → shared runtime browser processes

**The bridge.** Two-way JSON-RPC over `postMessage`/`WebMessageReceived`, typed (`serde`):

- host → widget: `init { nodeId, props, theme, zoom }`, `propsChanged`, `visibility { visible }`, `themeChanged`
- widget → host: `ready`, `resize { w, h }`, `setProps { ... }` (persisted to `.canvas`), `openFile { path }` (perm `shell:open`), `readDir { path }` (perm `fs:read`, allowlist directories only), `toast { text }`
- Every call is checked against the manifest's `permissions`; the message schemas are validated by deserialization — an invalid message = drop + warn, not a panic

**Security.**

- Widgets are local-only packages in `%APPDATA%/canvasdesk/widgets/<id>/`; installation = the user explicitly copying the folder (dragging a folder onto the canvas → an offer to install)
- Content is served via `SetVirtualHostNameToFolderMapping` (a virtual origin), navigation outside the package is forbidden; the default CSP is `default-src 'self'`
- The `network` permission is opt-in: without it all external `WebResourceRequested` are blocked
- A widget by URL (remote JS) is **architecturally forbidden** — local bundles only

**Data.** `props` and geometry — in the `.canvas` node (§5.1), they survive an export to Obsidian as an unknown type. Bulky widget state — in SQLite `widget_state(node_id, key, value)`.

**Input.** A click inside a widget — focus goes to WebView2 (keyboard/mouse go to the widget); moving a node — dragging by the border/header, which the canvas draws on top (a 24px strip); Esc — focus returns to the canvas. Edges to widget nodes work as to ordinary ones.

**Built-in widgets.** Shipped in `assets/widgets/`: clock/date, calendar, sticky notes. They also serve as the SDK references (T22).

**v1.1 clarifications (the M5 wave, the detailed plan — `docs/plans/M5-widgets.md`).**
Fixed product and technical decisions closing the forks of this section:

- **Content zoom — `ICoreWebView2Controller::ZoomFactor`** (clamped 0.25–5.0):
  the widget's CSS viewport equals the world size of the content area, the content
  scales with the canvas zoom without reflow; at zoom > 5 the content is clamped.
- **Node chrome:** a 28 world-px header + an 8 world-px border inset — canvas-side;
  the WebView occupies the inner area (a refinement of the "24px strip").
- **Adding to the canvas:** the canvas RMB menu "Widgets ▸" (installed +
  built-in); dragging a folder — the installation path (see below).
- **Installation/update/removal:** dragging a folder with a valid `widget.json` →
  an in-canvas Yes/No dialog (the permissions list) → copying the package and a node at
  the drop point; dragging a new version again → an update dialog (files
  are replaced, nodes/props are preserved); removing a package — via the submenu
  "Widgets ▸ <name> ▸ Remove package", the nodes become "broken".
- **Packages live in `%USERPROFILE%\.canvasdesk\widgets\<id>\`** (a single application
  data root together with `cache.db`; the deviation from "%APPDATA%/canvasdesk" above is documented). The WebView2 user-data-folder — `~/.canvasdesk/webview2/`.
  Built-in packages are baked into the binary and materialized at startup (the tombstone
  `widgets/.deleted/<id>` respects manual removal until a new version ships).
- **The `fs:read` allowlist:** the directories of the current canvas's file nodes + the folder
  of the `.canvas` file (the T10 watcher set).
- **The airspace policy:** when a live widget is overlapped by a canvas overlay
  (minimap, search, hotkeys, settings, menus, dialogs, the selection frame,
  edge dragging) the widget is temporarily hidden, a snapshot is drawn.
- **Theme:** `init`/`themeChanged` send `{ dark: bool, accent: "#hex" }` from the theme
  of the application.
- **The bridge is extended** with the `stateGet`/`stateSet` methods (access to
  the `widget_state` SQLite table, see "Data"); `canvas:read` is reserved.
- **MCP (v1.1):** the `widget_list`, `widget_add`, `widget_set_props` tools.
- Snapshot: clamped to 512×512, a 5 s refresh only for visible snapshot widgets
  at zoom ≥ 0.25 (beyond the live limit); separate textures (not the atlas), an LRU cap of 16.

## 8. Input

| Action | Gesture |
|---|---|
| Panning | Middle button / Space+drag / two-finger touchpad scroll |
| Zoom | Ctrl+wheel (toward the cursor), pinch |
| Selection | LMB drag — a frame; Shift — add |
| Moving nodes | LMB drag |
| Edge | drag from the node's port (ports appear on hover) |
| Node context menu | RMB |
| Search | Ctrl+F |
| Overview (fit to content) | Ctrl+0; the minimap — click/drag of the viewport rectangle |
| Double-click on a file | Open in the associated application |
| Settings (FR-039) | The ⚙ button / `Ctrl+,` — a centered modal over a dimmed backdrop; left navigation over 4 sections, rows of "label + description + control": booleans — pill toggles, multi-valued — dropdown buttons (a menu clamped to the window, the control's width); clicking the dimmed backdrop closes; adaptive size with ceilings (the FR-026 invariants carry over) |
| What-if mode (FR-017) | `Ctrl+Shift+I` / the "What-if" pill / the canvas menu; in the mode, a double-click on a calc line — a substitution; Esc — exit |
| Help menu and documentation | The "?" button (the ⚙/theme cluster): "Documentation ▸" — 7 sections in the built-in viewer (a right dock: the wheel — scrolling, internal links — navigation, × / Esc / a click outside — close); "Take the onboarding" (FR-031/FR-028) |
| Onboarding | First launch — a tour carousel (8 steps); "Skip"/Esc — postponed until the next launch (after 3 in a row the auto-show goes silent); repeating the tour — "?" → "Take the onboarding"; a full pass ("Done") turns the auto-show off forever (FR-028) |
| Interface language (FR-040) | Settings → "Appearance" → the "Language" dropdown ("русский"/"English", the names — in their own locale); applied on the fly, saved to the config (`language`, serde default `ru`); all UI texts — keys of the `i18n` table (RU/EN), fallback — RU |
| Input inside a widget (M5) | Clicking a widget — focus goes to the widget; Esc — focus returns to the canvas |
| Moving a widget (M5) | drag by the node's header/border |
| **Main stage (FR-042)** | Click/RMB/double-click on a bundle's aggregated line — a modal detail (a rect ≤ 70% of the viewport: both nodes + all the bundle's edges fanned out, the `fromLine`/`fromOutput`/`toParam` labels + values); a click inside — selecting the edge of the live connection; exit — `Esc` or a click on the dimmed background; pan/zoom/drag/edit inside are muted; opening any overlay closes the stage; the switch — the "Edge aggregation" setting (FR-039, the "Edges and ports" section) |
| **Number chain verification (FR-048, PRD-0007)** | A hover-"?" next to the result bar/per-line row, or clicking a number — a floating verification window over the full-screen canvas (the main stage pattern): the lineage tree, the breadcrumb navigation chips, what-if from the tree, the protection mode (a toggle in the header: ×1.5, step-by-step reveal — Space/click/buttons); Esc — a two-stage exit (protection → window → close), ✕/a click on the background — close; clicking a highlighted canvas node — highlights the tree node (У2); modality — the PRD-0007 §6.5 registry (F-10): the stage ↔ the panel are mutually exclusive, the what-if bar coexists, the auto-edge/settings dialogs — on top; the "Chains: N%" indicator — an opt-in setting (F-12) |

## 9. Risks and assumptions

| Risk | Mitigation |
|---|---|
| WorkerW breaks on a Windows update | The version gate + a fallback to windowed mode; the canvas file does not depend on the display mode |
| A third-party preview handler crashes | An out-of-process host with a timeout and a restart |
| An antivirus false positive (shell hooks) | Authenticode signing from M4, submission for whitelisting before release |
| Performance on large scenes | LOD + culling + atlases; the 5k-node load test is part of CI |
| Canvas data loss | Autosave with a 2 s debounce + `.bak` of the previous version; JSON is human-readable |
| Hotkey conflicts with Explorer in M4 | The window does not intercept the keyboard without explicit focus (a click on the canvas) |
| WebView2 Runtime missing on the machine | The Evergreen bootstrapper in the MSI (T19); without the runtime — widgets are unavailable with a clear error, the canvas works |
| A malicious widget | Local-only packages, permissions in the manifest, remote JS and navigation forbidden, `network` opt-in (§7.6) |
| fps degradation from many WebView2s | The live instance limit + the snapshot LOD (§7.6); the 10-widget load test is part of the M5 acceptance |
| Airspace: WebView2 over the wgpu canvas | A documented limitation (§7.6): overlays do not cover widgets or are moved into layered windows |

## 10. Release readiness criteria

- **M1 done:** the canvas opens/saves `.canvas`, 5 000 file cards with thumbnails at 60 fps, the file opens in Obsidian without errors
- **M2 done:** notes are edited inline, edges are drawn and saved, dropping a folder from Explorer lays out the files, renaming a file in Explorer updates the card in < 1 s
- **M3 done:** PDF/images/text show a live preview at zoom > 0.6, docx — via the preview host, the minimap is navigable, search finds by name and text (FTS5)
- **M4 done:** the `--desktop` flag embeds the canvas behind the icons, the icons are hidden and restored on exit, an Explorer restart is survived, a crash does not leave the desktop without icons
- **M5 done:** a widget from a local folder is installed on the canvas, live at zoom ≥ 0.25, a snapshot at far zoom, props survive a restart, bridge calls without permission are blocked, 10 widgets on the scene do not drop fps below 60 thanks to the live instance limit

## 11. Documentation for the agent

The mandatory reading list before coding the corresponding module. The agent must check these sources rather than rely on its own memory — APIs change, and undocumented techniques differ between OS versions.

### 11.1. Target platform

- Windows 11 release information (versions and build numbers — for the version gate §7.4): https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information
- Windows 11 25H2 update history (the KB-by-KB changes on the target build 26200.x): https://support.microsoft.com/en-us/servicing/os/windows-11/2025/07/windows-11-version-25h2-update-history

### 11.2. Data formats

- JSON Canvas spec 1.0 (the source of truth on the `.canvas` format, §5.1): https://jsoncanvas.org/spec/1.0/
- SQLite FTS5 (the search index, §5.2): https://www.sqlite.org/fts5.html

### 11.3. Rust ecosystem (docs.rs — read against the versions pinned in Cargo.toml)

- winit (window, input, `ApplicationHandler` — the 0.30 event loop differs from 0.29): https://docs.rs/winit/latest/winit/
- wgpu (render): https://docs.rs/wgpu/latest/wgpu/ + a tutorial: https://sotrh.github.io/learn-wgpu/
- glyphon (text over wgpu): https://docs.rs/glyphon/latest/glyphon/
- cosmic-text (shaping, layout): https://docs.rs/cosmic-text/latest/cosmic_text/
- rstar (R-tree, §6.1): https://docs.rs/rstar/latest/rstar/
- notify (the file watcher, §7.5): https://docs.rs/notify/latest/notify/
- rusqlite (§5.2): https://docs.rs/rusqlite/latest/rusqlite/
- serde / serde_json (a lossless round-trip, §5.1): https://docs.rs/serde_json/latest/serde_json/
- image (image decoding): https://docs.rs/image/latest/image/
- pdfium-render (PDF previews, §6.2): https://docs.rs/pdfium-render/latest/pdfium_render/
- thiserror / anyhow (errors): https://docs.rs/thiserror/latest/thiserror/ · https://docs.rs/anyhow/latest/anyhow/
- tracing (logging): https://docs.rs/tracing/latest/tracing/
- cargo-wix (MSI): https://github.com/volks73/cargo-wix

### 11.4. Win32 / Shell (Microsoft Learn)

- windows-rs (API metadata and bindings): https://github.com/microsoft/windows-rs · reference: https://microsoft.github.io/windows-docs-rs/doc/windows/
- IShellItemImageFactory (thumbnails, §7.1): https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishellitemimagefactory
- IPreviewHandler and hosting (§7.2): https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ipreviewhandler · https://learn.microsoft.com/en-us/windows/win32/shell/preview-handlers
- OLE Drag and Drop / IDropTarget (§7.3): https://learn.microsoft.com/en-us/windows/win32/com/drag-and-drop · https://learn.microsoft.com/en-us/windows/win32/api/oleidl/nn-oleidl-idroptarget · CF_HDROP: https://learn.microsoft.com/en-us/windows/win32/shell/clipboard
- ShellExecuteEx (§7.4): https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shellexecuteexw
- ReadDirectoryChangesW (what notify does under the hood, §7.5): https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw
- EnumDisplayMonitors / multi-monitor (§7.4): https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaymonitors
- High DPI / Per-Monitor V2 (§6.5): https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-desktop-application-development-on-windows
- `.canvas` association / ProgID registration (M4): https://learn.microsoft.com/en-us/windows/win32/shell/fa-progids
- WebView2 (M5): the feature overview — https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/overview-features-capabilities · Get started Win32 — https://learn.microsoft.com/en-us/microsoft-edge/webview2/get-started/win32 · `ICoreWebView2` (postMessage, CapturePreview, WebResourceRequested) — https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2 · `SetVirtualHostNameToFolderMapping` — https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2_3#setvirtualhostnametofoldermapping
- webview2-com (Rust bindings for WebView2): https://docs.rs/webview2-com/latest/webview2_com/ · https://github.com/wravery/webview2-rs
- Evergreen Runtime bootstrapper (distribution, T19): https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution

### 11.5. Desktop embedding (§7.4) — undocumented territory

There is no official documentation. The sources behind the §7.4 strategy table:

- The classic technique (≤ 23H2): "Draw Behind Desktop Icons in Windows", CodeProject: https://www.codeproject.com/Articles/856020/Draw-Behind-Desktop-Icons-in-Windows-plus
- The hierarchy change in 24H2+ (Progman with `WS_EX_NOREDIRECTIONBITMAP`, DefView as a layered child, WorkerW as a child below DefView — a breakdown and working Z-order code): https://learn.microsoft.com/en-us/answers/questions/1386630/doubts-about-the-window-of-program-manager · https://stackoverflow.com/questions/79763352/setting-a-window-as-wallpaper
- Reference implementations that already solve this task on 24H2+/25H2 — study the code before writing your own:
  - Lively Wallpaper (C#, actively supports 24H2+): https://github.com/rocksdanister/lively
  - Seelen UI (Rust — also a reference for windows-rs in production): https://github.com/eythaann/Seelen-UI

The rule for the agent: any claim about the Progman/WorkerW/DefView behavior not confirmed by these sources or by our own runtime detection is considered unverified and is implemented only behind a fallback.

## 12. Post-Release Documentation Requirement (mandatory)

After every release the agent is OBLIGED to create a devlog entry —
`docs/devlog/` (one wave/epic = one file; for example `M5-widgets.md`):

1. Format: date, version/tag, type (micro | feature)
2. Mandatory fields:
   - **Problem** — what did not work / what was missing
   - **Solution** — what was done, 2–4 sentences, no stack jargon
   - **Why this way and not another** — alternatives, the compromise
   - **What was hard / unexpected** — for storytelling
   - **Metric/result**, if applicable (an estimate if the fact is unknown)

**The "full article" trigger** (not a micro-note):
- an epic/milestone from `docs/plans/product-roadmap.md` (the 0/A/B/V/S waves)
  or the T0–T22/M1–M7 waves is closed,
- OR a feature that changes user-facing behavior is added.

In that case the agent prepares an 800–1200 word article draft:
task context → the solution path → pitfalls → result → what's next.

## 13. Canvas MCP tools

The tool catalog — `crates/canvas-mcp/src/lib.rs` (the `TOOLS` constant,
the canonical source; `tools/list` serves the same schemas automatically).
This section pins the contracts critical for agent-side assembly (ADR-0004:
MCP — the only channel; CR-013: wave A).

**The "MCP visibility = UI" invariant (the 2026-09-22 request "align MCP with the updates" / «подтянуть MCP под обновления»):** the value-reading tools (`flow_recalc`,
`analyze_bottlenecks`, `lineage`, the `flow` field of `graph_apply`/
`schemes_apply`) return the ACTIVE what-if state — the same numbers/flags
the user sees on the canvas (the FR-050 cascade Р-1: what-if overrides
spill overrides local). The recalc is always FRESH (the lazy
`node_edit` mutations with text — CR-012 — do not distort the report); the reading
touches neither the model revision nor the undo history. The FR-050 Р-4 auto-rows (`autoRows`) —
in the `flow_recalc`/`graph_apply`/`schemes_apply` responses: slot, source edge,
the "Object.Field" path, the active scenario's value, or `unmapped`.

The canonical call order for agent-side model assembly (reconnaissance → nodes →
value edges → recalc → batch → validation) is fixed in the recipe
`user-docs/agent-recipe.md` (R5/CP4, CR-013). The error codes of `graph_validate`
and the `graph_apply` operations are a stable contract of the recipe: change them only
together with the recipe.

**The `skills/` skill package (the owner's 2026-09-22 request):** a published
derivative of the tool registry for external AI agents — 4 skills
(`canvasdesk-mcp` connection/reconnaissance, `canvasdesk-model-build` assembly,
`canvasdesk-model-verify` verification, `canvasdesk-whatif` scenarios) +
the full catalog `canvasdesk-mcp/references/tools.md` + the reference example
Instagram MVP (ADR-0005) in `canvasdesk-model-build/examples/`.
The "skills = registry" sync invariant is pinned by the contract tests
`skills_*` in canvas-mcp (`include_str!` of the package): (1) the catalog describes
every tool of `TOOLS`; (2) every tool is mentioned in at least
one SKILL.md; (3) the tool counter in `skills/README.md` is up to date;
(4) the markdown-call positions `` `имя` {…} `` — registry names or
`graph_apply` operations only. Changing the MCP tools updates `skills/`
in the same commit — the protocol `skills/UPDATE-PROTOCOL.md` (history —
`skills/CHANGELOG.md`).

### Headless MCP verification (FR-037, ADR-0012)

The tool layer (`canvas-scene`), the protocol bridge (`canvas-mcp`) and
the headless server (`canvas-mcp-headless`) are verified in the wasm runtime:
compilation for `wasm32-unknown-unknown` (CI `wasm-check`), execution of
the tests under `wasm32-wasip1` in wasmtime, and a full MCP session with a real
client (initialize → tools/list → tools/call → the oracle gates of the reference models
CP1/CP3/CP5 → negative branches) — the gate `scripts/mcp_wasm_gate.sh`, without
Windows and GUI. The owner's live manual sessions — with the official inspector
`@modelcontextprotocol/inspector`: `scripts/mcp_wasm_inspector.sh`
(web UI; `--check` — automated oracle acceptance at ±1 %; node/npx — outside the gates, MW5).
`HeadlessSession` — the server side of the future
WebSocket bridge (wave 2 of the M8 plan).

### graph_apply (FR-033) — atomic batch composition

Call schema: `graph_apply { operations: [Op; 1..=256] }`. `Op` — an object
tagged with `op`:

| op | Fields | Note |
|---|---|---|
| `node_create_note` | `ref?, x, y, text?, width?, height?` | lines "= …" are formulas (FR-013) |
| `node_create_file` | `ref?, x, y, path, width?, height?` | the file is not created on disk |
| `template_instantiate` | `ref?, template, params?, x, y` | `params` — `{имя: число \| {num, unit}}`; outside min/max — an error |
| `edge_create` | `fromRef\|from, toRef\|to, kind?, fromLine?, fromOutput?, toParam?, fromSide?, toSide?` | `kind`: `"value"\|"control"` (the default is control); ports — the FR-029 contract; `fromLine`/`fromOutput` are mutually exclusive; port names are validated against the template snapshots |
| `param_set` | `ref\|id, param, value, unit?` | edits exactly one line "param = value unit" (the text + the template snapshot); no such parameter — an error (no append) |
| `node_move` | `ref\|id, x, y` | |

**Limits:** ≤ 256 operations, ≤ 128 new nodes per batch (protecting the live budget
of SPEC §6.3). Exceeding them — a call-level error (isError).

**Transactional semantics:** the operations are applied to a clone of the canvas;
an error in ANY operation → `{ok: false, op_index, code, message}` and the canvas
stays byte-for-byte the same (the clone is discarded); success → the canvas is replaced, exactly
**one** undo step for the whole batch (Ctrl+Z reverts the entire assembly),
a full value flow recalc, autosave.

**Response (success):** `{ok: true, created: [{op_index, ref?, node_id?, edge_id?}],
report: [{op_index, op, id}], flow: {node_id: {value, unit, outputs?,
lines?, autoRows?, error?}}}` — `flow` in the flow_recalc v2 format
(FR-029): the values of the ACTIVE scenario + named outputs +
per-line values + auto-rows (FR-050 Р-4); a second call of
flow_recalc is not needed.

**Operation error codes:** `E-BAD-OP` (shape/fields), `E-NOT-FOUND`
(ref/id/template), `E-PORT-UNKNOWN` (an unknown parameter/output),
`E-CYCLE` (a value cycle, the participants in message), `E-PARAM-UNKNOWN`
(no parameter line), `E-RANGE` (outside min/max). The numbering of
`op_index` starts at 0; refs live only inside the batch (they address nodes
created earlier in the same call).

### whatif_* (FR-017, CP6) — "what if" scenarios

Nine tools over the active canvas. Substitutions are runtime-only:
the canvas is not mutated without `whatif_apply` (invariant 2).

| Tool | Semantics |
|---|---|
| `whatif_set_override {node_id, line, expr}` | a per-line substitution of the active scenario; the mode/scenario are raised automatically (the implicit "MCP Scenario"); `expr` is normalized (a literal `\n` → line breaks) |
| `whatif_set_param {node_id, param, value}` | sugar: finds the line `param = …` and builds the substitution |
| `whatif_scenario_list` | the scenarios with substitution counts and stale markers |
| `whatif_scenario_create {name}` | a new scenario (limit 3), active immediately; the `canvasdesk.whatif` mutation is one undo step |
| `whatif_scenario_delete {name}` | deletion (an undo step) |
| `whatif_scenario_activate {name}` | switching Base ↔ scenario; runtime-only, the file is untouched |
| `whatif_deltas` | the deltas of the active scenario — the same "was → now" pairs as seen on the canvas (invariant 6) |
| `whatif_apply` | write the substitutions into the persisted lines/params and delete the scenario; one undo step |
| `whatif_reset` | resetting the substitutions of the active scenario (runtime) |

### schemes_list / schemes_apply (PRD-0008, Q5 v2) — the scheme gallery for the agent

The same packages the user sees in the gallery (Ctrl+T) — the invariant
"MCP visibility = UI": the agent can show a demo canvas with a single insertion.

| Tool | Semantics |
|---|---|
| `schemes_list {}` | the array of packages: `{id, name, name_en, category, category_ru/en, version, description/description_en, nodes, edges}` — RU-first, as in the UI; the array arrives in text content (FR-034) |
| `schemes_apply {id, x?, y?}` | inserting a scheme into the current canvas — like "Open" in the gallery: an id remap without collisions (note-N/group-N/edge-N), centering at (x, y) or the viewport center; one undo step, a full recalc, what-if scenarios are untouched. The response `{applied, name, nodes[], edges[] (контракт edges_list), bbox [4], flow}` — flow as in flow_recalc (the active state + autoRows); an unknown id — isError without an undo step |

### lineage (PRD-0007 X2, FR-048) — the number's lineage tree

Call schema: `lineage {node_id, line?}` — `line` null/absent = the node's total
(the D bar), otherwise the index of a Numi sheet line (FR-025). The same model
as the chain verification window (the PRD-0007 F-5 invariant: a single source);
the values are of the active what-if state, a flow cycle — topology without
values (AC-2.4), the 4096-node budget — a `truncated` collapse.

**Response:** `{root: {node_id, line}, nodes: [{node_id, line, kind
(calc|leaf|cycle|unmapped|unlinked|truncated), value+unit | error,
formula?, title, label?, children: [{child (индекс в nodes), via?}]}]` —
DFS order (parent before child, a diamond is expanded without
deduplication); `via` = `{edge_id, from_node, to_node, from_line?,
from_output?, to_param?}` — the edge for highlighting the chain on the canvas
(F-4/AC-3.1). Negative branches: the node is not found / line < 0 / prose
as the root — isError.

### analyze_bottlenecks (FR-016) — bottlenecks and queueing risk

Call schema: `analyze_bottlenecks {}` (no parameters — the active canvas).
Reading: the recalc is fresh and of the ACTIVE what-if state (as `flow_recalc`);
the canvas and undo are not touched.

**Response:** `{nodes: [{id, severity, utilization?, queue_length?, wait_sec?,
badge}], thresholds}` — the same flags the user sees on the canvas
(the FR-016 invariant 4: `badge` — the canvas badge string, for example
`"OVERLOAD 223% · W: 1.2 s"`). `severity` — `none|warn|critical|overload`;
`utilization` — ρ (a share of 0..1, > 1 under overload); `wait_sec` — W in base
seconds; `thresholds` — the default thresholds (0.7/0.9, 100 ms/1 s, 1/10).

**Detection (the `canvas-core/src/analyze.rs` analyzer):** a node value that is
an `Overload{ρ}` error → `overload` (ρ from the error); a named output
`utilization` of a template (13 queue manifests) or a Percent value → the thresholds
0.7/0.9; a Time value (W) → 100 ms/1 s; the named outputs
`queue_length`/`wait_time` — the manifests' extension points. The order —
`canvas.nodes` (determinism).

### monte_carlo_run (FR-066) — Monte Carlo/QMC and the P50/P90/P99 quantiles

Call schema: `monte_carlo_run {runs, params, mode?, seed?, quantiles?}`.
**Native-only** (the canvas-scene feature `qmc`: stats+parallel+sobol_burley;
a wasm build does not serve the tool, §5.8 of FR-066; a build without the feature —
`tools/list` does not declare it, a call — isError).

- `runs` — an integer 1..=10⁶ (the FR-066 reference norm — 10⁴); `mode` —
  `"qmc"` (the default: Owen-scrambled Sobol → inverse-CDF `statrs`, a lower
  variance at the same N; the length limit 2¹⁶ = 65536) | `"mc"` (ChaCha8);
  `seed` — u64 (the default 0; **the same seed → bitwise-identical quantiles** —
  reproducibility is first-class, the seed of §5.7.2: `hash(content) ⊕
  scenario_seed ⊕ run_idx` with FNV mixing); `quantiles` — shares of 0..1,
  the default `[0.5, 0.9, 0.99]` (P10 — the "runway" case: `[0.1, …]`).
- `params` — `{"node_id:param": {"dist": "normal"|"lognormal"|"exp"|
  "poisson", …}}`: normal/lognormal — `{mean, sd}` in the natural
  space, exp/poisson — `{lambda}` (a Poisson λ ≤ 1000 — the budget of
  inverse-CDF). A parameter is a line "param = …" of the node's Numi sheet (substituted
  on every run, the `whatif_set_param` pattern); the unit is conferred
  by the consuming formula (the Numi semantics FR-050 Н5: `rho = load × 1 %`).
- **Response:** `{runs, failed_runs, mode, seed, stale, duration_ms,
  quantiles, outputs{node:{P50:{value,unit},…}}, lines{"node:line":{…}},
  named{"node:выход":{…}}, analysis{quantile, nodes, thresholds},
  severity}` — the quantiles of node totals, per-line and named outputs
  (collect-then-reduce §5.7.3, builtin `percentile`).
- `analysis` — the bottlenecks (the `analyze_bottlenecks` format) at the TAIL
  quantile (P90): a synthetic `FlowSolutions` → `analyze::analyze`
  **without analyzer modifications** (§5.5) — severity escalates on the tail
  ("sub-critical at the median, critical at P90").
- Mutation: the engine metadata `canvas.extra["canvasdesk"]["engine"] =
  {version: "M5.0", seed, stats, parallel, qmc}` (raw JSON, the
  `whatif.rs` pattern; §5.7.4) — an undo step + autosave on an actual change;
  a version desync → `stale: true`.
- Call errors: a flow cycle; parameters not from the node's sheet (strict
  validation BEFORE the runs); invalid dist/quantiles/mode (see the test
  `mcp_fr066_monte_carlo_run_strict_validation`).

### stdio transport (FR-034, ADR-0009)

The bridge `canvasdesk-mcp` / `canvasdesk mcp` is a standalone MCP server:
the handshake does not depend on the state of the GUI application.

- **Protocol versions:** the client version is echoed if supported
  (`2025-06-18`, `2025-03-26`, `2024-11-05`); an unknown one → `2024-11-05`.
- **Batch requests:** a JSON-RPC array is processed element by element; an empty
  array → one response `-32600`; a batch of notifications → silence.
- **The tools/call result:** `content[0].text` — the clean JSON of the result,
  `structuredContent` — the same object (spec 2025-06-18); an isError result
  of the application passes through.
- **Offline mode:** the pipe is unavailable → `initialize` succeeds, `tools/call`
  → isError "CanvasDesk is not running…" («CanvasDesk не запущен…»); the bridge process lives
  until stdio is closed (no exit codes for the application being unavailable).
- **Reconnect:** before every incoming packet, with a dead transport —
  a short connection attempt to the pipe (500 ms, no auto-spawn); an application
  that came up later than the bridge is picked up without restarting the MCP session.
- **Tolerant stubs:** `resources/list`, `prompts/list`,
  `resources/templates/list` → empty lists; `logging/setLevel` → `{}`;
  `notifications/cancelled` → ignored.
- Framing — newline-delimited JSON (no Content-Length); the response timeout
  of the application — 30 s → isError.

### stdout purity (FR-035, ADR-0010)

The stdout of the bridge process is **only** newline-delimited JSON-RPC; neither logs,
nor ANSI sequences, nor the output of child processes have the right
to appear in the protocol channel (a violation is caught by the client as
`Invalid JSON`).

- **Auto-spawn isolation:** the service is raised with
  `stdin/stdout/stderr = Stdio::null()` — the inheritance of the bridge's handles
  is excluded by construction (previously GUI tracing logs with ANSI ended up in the
  JSON-RPC stream).
- **Auto-spawn orientation:** the single `canvasdesk` binary spawns itself
  (GUI mode without arguments); the standalone `canvasdesk-mcp` looks for the GUI binary
  `canvasdesk.exe` in its own directory; no neighbor — offline mode
  (ADR-0009), recursive spawning is excluded.
- **Diagnostics — to stderr:** the bridge's own messages and the GUI logs
  (`tracing_subscriber::fmt().with_writer(io::stderr)`) go to
  stderr; ANSI in the GUI logs — only when stderr is a live terminal.
