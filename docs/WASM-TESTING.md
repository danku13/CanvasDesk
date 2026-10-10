# WASM UI testing — a quick-setup recipe

A recipe for the agent's UI self-check on the web build, without Windows or a GUI (the rule in
`AGENTS.md` → "WASM UI self-check"). The goal is for a new session to reproduce the bench in
~10 minutes from ready-made commands instead of re-discovering the path from scratch.
Precedent: fix 52027bc «панели закрываются при клике на себя» ("panels close when clicked on
themselves") was verified on a wasm build by browser clicks with pixel diffs (worklog 2026-09-25).

---

## 1. Verification levels — from cheap to full

| Level | What it gives | When it suffices |
|---|---|---|
| **L0** — compilation for wasm32-unknown-unknown | catches 90% of regressions (cfg, features, web-sys/wgpu types) | almost always for core/crate edits |
| **L1** — unit tests of the core/bridge in wasmtime (wasm32-wasip1) | execution of pure logic in a wasm runtime | edits to canvas-core / canvas-mcp |
| **L2** — a browser bench: Chromium + WebGPU (SwiftShader) under Xvfb | FULL-FLEDGED UI verification: clicks, keyboard, pixel diffs | **mandatory for UI changes** (layout, input, panels, hit tests, rendering) |
| **L3** — `trunk serve` / `scripts/web_bundle.sh --release` / CI (`wasm-check`, `pages-web`) | the reference pipeline, the bundle size | pre-release acceptance |

Native unit tests (cargo test) are not canceled — they cover the logic; L2
covers the behavior on the web platform, where input (pointer/keyboard), coordinates,
and compositing work differently than in winit.

## 2. The agent environment (verified 2026-09-25)

**Preinstalled:**

| Tool | Where | Why |
|---|---|---|
| rust stable + `~/.cargo/bin` | add `$HOME/.cargo/bin` to PATH | building |
| the `wasm32-unknown-unknown` target | rustup | L0/L2 |
| `wasm-bindgen-cli` **0.2.127** | `~/.cargo/bin/wasm-bindgen` | L2 (the version = the crate in Cargo.lock) |
| node 24 + the playwright module | `/home/z/.npm-global/lib/node_modules/` | L2 scenarios (npm playwright, `createRequire` from this root) |
| python3-playwright | pip | an alternative for scenarios (`scripts/web_smoke.py`) |
| Chromium 1243 (+ headless shell) | `~/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome` | the browser |
| Xvfb | system | a virtual display (headed rendering); the `xvfb-run` wrapper does not work in this environment — no `xauth` |
| python3 + PIL + numpy | pip | the pixel diff (`scripts/wasm_ui_diff.py`) |

**Missing (install as needed):**

- `wasmtime` (L1): `curl https://wasmtime.dev/install.sh -sSf | bash`
  (the binary lands in `~/.local/bin` — add it to PATH) + `rustup target add wasm32-wasip1`;
  then `scripts/wasm_gate.sh` / `scripts/mcp_wasm_gate.sh` without `--check`.
- `trunk` (L3): not installed in this environment — the prebuilt assets of GitHub releases
  return 404 (as in `pages-web.yml` for wasm-bindgen); `cargo install trunk --locked`
  builds from sources (~5–10 min). Not a blocker: the manual build below gives an identical
  result (the same bindgen step as trunk).

**Environment quirks (important):**

- Background processes do NOT survive between bash calls → the http server is started
  by the scenario itself (a child) and shut down at the end; or everything in a single call.
- The dev wasm weighs ~34 MB → page load up to 90 s, wait ~7 s for the first frame.
- The sandbox disk is small → for a full `wasm_gate.sh` clean
  `target/wasm32-unknown-unknown/incremental` (precedent FR-057/059).

## 3. Level L2 — step by step

### Step 1. Build the bench

`trunk` is not needed — bindgen by hand (the equivalent of trunk's rust pipeline):

```bash
cd <корень репо> && export PATH="$HOME/.cargo/bin:$PATH"
cargo build -p canvas-web --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir target/dist \
    target/wasm32-unknown-unknown/debug/canvas_web.wasm
cp crates/canvas-web/index.html target/dist/
# Инъекция init-глю (то, что trunk делает сам):
python3 - target/dist/index.html <<'PY'
import sys, pathlib
p = pathlib.Path(sys.argv[1]); html = p.read_text(encoding="utf-8")
if "canvas_web.js" not in html:
    p.write_text(html.replace("</body>", """  <script type="module">
  import init from './canvas_web.js';
  init();
</script>
</body>""", 1), encoding="utf-8")
PY
```

One command does all of this: **`scripts/wasm_ui_test.sh`** (a re-run is
`scripts/wasm_ui_test.sh --no-build`, which reuses `target/dist`).

### Step 2. Run the scenario

The scenario itself brings up `http.server` on the bench and shuts it down; Xvfb is brought up
by the wrapper script:

```bash
# Пример — тест панели «О интерфейсе» (координаты калибруются по скриншоту):
ITEM="1035,117" BODY="1078,708" LABEL=about scripts/wasm_ui_test.sh --no-build
# (скрипт сам поднимает Xvfb и гасит его по выходу; дисплеи 90–99)
```

What happens inside (`scripts/wasm_ui_scenario.mjs`):

1. Chromium (playwright, **headed**) under Xvfb, which `wasm_ui_test.sh` itself
   brings up (xvfb-run breaks in this environment — no xauth; Xvfb manually
   on a free display 90–99).
2. The flags (critical, see §4): `--enable-unsafe-webgpu --enable-features=Vulkan
   --use-vulkan=swiftshader --use-webgpu-adapter=swiftshader --no-sandbox`.
3. Load → wait for `body > canvas` (winit creates the canvas at `create_window`)
   + 7 s for the first frame.
4. Scenario steps driven by env parameters: `SKIP` (skip onboarding),
   `HELP` ("?"), `ITEM` (a menu item), `BODY` (a point on the panel BODY — padding outside
   the interactive rects), `BACKDROP` (the background outside the panel). Every click is
   a screenshot.
5. Pixel diffs (`scripts/wasm_ui_diff.py`, PIL): before/after the body click
   (~0 — the panel stayed) and before/after the backdrop click (hundreds of thousands
   of px — the panel closed). The oracle is the count of changed pixels, not the picture.

### Step 3. Read the oracles

- The browser console: `[render] renderer инициализирован backend=BrowserWebGpu`
  — the renderer is alive; the line `[canvas-web compat] лимиты не распознаны…` is normal
  (the `index.html` shim against `maxInterStageShaderComponents`).
- `pageerror` is not expected; any panic lines are a regression.
- Screenshot diffs — as in step 2; a diff of ~3–4 thousand px from a "stuck" button hover
  is frame-delivery cosmetics (the frame was not redrawn on mouse-move), it has nothing
  to do with the logic (precedent in the worklog).

## 4. Pitfalls (all verified in practice)

1. **Headless Chromium does NOT composite the WebGPU canvas in captureScreenshot** —
   the reference clear frame is invisible. Only a headed browser under Xvfb (a live
   compositor). This is the main L2-level trap.
2. **Without the SwiftShader flags** `request_adapter` → None ("no GPU adapter
   found"). Moreover, in recent Chromium the request is rejected outright because of the
   `maxInterStageShaderComponents` limit (removed from the spec) — the shim in
   `index.html` removes it; without it the failure masquerades as "no GPU".
3. **The `wasm-bindgen` CLI version must match the crate in `Cargo.lock`**
   (currently 0.2.127) — otherwise the JS glue is incompatible with the wasm module. Check:
   `wasm-bindgen --version` vs `rg 'name = "wasm-bindgen"' -A1 Cargo.lock`.
4. **Click coordinates** — CSS px of the viewport (1280×800 = the canvas window);
   the DOM toolbar (top-right, `#w6-toolbar`) overlaps the top of the canvas — the "?"
   button is clicked on its lower part. GPU-menu items — only by canvas coordinates,
   calibrated from the step's screenshot.
5. **Hover artifacts in the diff**: the cursor is placed at the target point BEFORE
   the reference frame (+400 ms), otherwise the diff catches the hover change, not the event.
6. **Loading**: `waitUntil: 'load'` is not enough — wait for the `body > canvas`
   selector (timeout 90 s) + a pause for the first frame; the dev build is heavy.
7. **The bench server** — only inside the scenario process (see §2, background
   processes); the default port is 8081.

## 5. What this recipe has verified (chronology)

- 2026-09-25: fix 52027bc — the UI console and the «О интерфейсе» ("About") panel do not close on
  a click on their own body; they close on the background (the backdrop contract), Esc alive.
  Screenshots and diffs are in the repo worklog; the ancestor scenarios of the current
  `wasm_ui_scenario.mjs` are `wasm_probe*.mjs` (an agent session).
- 2026-09-25 (the same day): the recipe itself was committed and verified end to end
  (`ITEM="1035,117" BODY="1078,708" LABEL=about scripts/wasm_ui_test.sh`):
  the bench built, `backend=BrowserWebGpu`, the body click — 3,697 px
  (hover cosmetics, the panel stayed), the backdrop click — 552,616 px (the panel
  closed). Two pitfalls were caught in the run and written up above: xvfb-run without
  xauth (Xvfb manually) and the http.server child holding the node event loop
  (an explicit `kill` in finally + a watchdog).
- 2026-10-08 (FR-100, UR-001-02): the web keyboard smoke was extended with the editor's
  chords — section 2b of `scripts/web_smoke.py`. The path: dblclick → a note →
  `Control+Backspace` (word-delete; winit-web delivers the chord to the editor —
  the FR-095 shim suppresses the browser default `preventDefault`) → `Control+a`
  (select-all, the browser select-all suppressed) → replacing the text → Enter →
  the oracle is finding the replacement in the model (the log `поиск завершён rows≥1`)
  + the absence of `pageerror`. The scenario was added; the run was not executed in this
  environment — manual web acceptance by the owner (a macOS browser: the same chords via Cmd).

## 6. Cheat sheet

```bash
# L0 — всегда
scripts/wasm_gate.sh --check                 # или cargo check --target wasm32-unknown-unknown -p <крейт>

# L1 — единожды ставится wasmtime, затем
scripts/wasm_gate.sh && scripts/mcp_wasm_gate.sh

# L2 — UI-проверка (стенд + сценарий)
ITEM="1035,117" BODY="1078,708" LABEL=about scripts/wasm_ui_test.sh
scripts/wasm_ui_test.sh --no-build           # повторно, без пересборки

# L3 — релизный бандл / дев-сервер (если trunk доступен)
(cd crates/canvas-web && trunk serve)        # http://127.0.0.1:8080
scripts/web_bundle.sh --dist _site/app       # как в CI pages-web
```

When L2 is unavailable (an environment without node/playwright/Xvfb) — the rule from
`AGENTS.md`: explicitly report «WASM-проверка не выполнялась, причина» ("the WASM check
was not performed, reason") and give the owner a manual instruction (what to open,
where to click, what counts as success).
