---
name: wasm-test
description: The mandatory WASM UI self-check — verification levels L0–L3, the L0 → native tests → L2 order, the wasm gates (wasm_gate.sh, mcp_wasm_gate.sh, wasm_ui_test.sh, web_bundle.sh) and the explicit skip protocol when the browser stand is unavailable. Load before reporting any UI change done.
version: 1
---

# $wasm-test

How to verify UI changes on the wasm platform. Decomposes AGENTS.md
("WASM UI self-check — mandatory before reporting" + the wasm-gates part of
"Build and tests"); the practical bench recipe is
[docs/WASM-TESTING.md](../../../../docs/WASM-TESTING.md).

## When to load

- Owner directive 2026-09-25: **any change touching UI (layout, input, panels,
  hit tests, rendering, themes) is verified by the agent itself on the web
  build** — native unit tests cover logic but do not substitute behavioral
  verification on the wasm platform (input, coordinates, compositing work
  differently than in winit).
- Load at task start (to plan verification) and before the final report (the
  report must state how the WASM check was done — or apply the skip protocol
  below).

## Mandatory verification order

1. **L0 — the compilation gate:** `scripts/wasm_gate.sh --check` (or a
   targeted `cargo check --target wasm32-unknown-unknown -p <crate>`).
2. **Native tests** (`cargo test`) — logic.
3. **L2 — the browser stand:** `scripts/wasm_ui_test.sh` (canvas-web build
   without trunk + Chromium/WebGPU under Xvfb + a scenario with a pixel diff)
   — behavior. Scenario/coordinates — per docs/WASM-TESTING.md §3 recipe.

"The phrase 'native tests are green' by itself does not close UI acceptance."

## Verification levels (docs/WASM-TESTING.md §1)

| Level | What it gives | When it suffices |
|---|---|---|
| **L0** — compilation for wasm32-unknown-unknown | catches ~90% of regressions (cfg, features, web-sys/wgpu types) | almost always for core/crate edits |
| **L1** — unit tests of the core/bridge in wasmtime (wasm32-wasip1) | execution of pure logic in a wasm runtime | edits to canvas-core / canvas-mcp |
| **L2** — a browser bench: Chromium + WebGPU (SwiftShader) under Xvfb | full UI verification: clicks, keyboard, pixel diffs | **mandatory for UI changes** (layout, input, panels, hit tests, rendering) |
| **L3** — `trunk serve` / `scripts/web_bundle.sh --release` / CI (`wasm-check`, `pages-web`) | the reference pipeline, the bundle size | pre-release acceptance |

## Skip protocol (when L2 is unavailable)

If the environment does not allow L2 (no node/playwright/Xvfb, a broken build,
disk/time) — the task's final report MUST:

- (a) explicitly state that the WASM check was not performed and why;
- (b) attach manual-verification instructions: what to open (the Pages web
  version / `trunk serve`), where to click, what counts as success.

"Native tests are green" by itself does not close UI acceptance.

## Gates and scripts

| Script | What it does |
|---|---|
| `scripts/wasm_gate.sh` | wasm target for core/render/widgets/mcp/web + core rlib + canvas-core tests under wasip1 (wasmtime). `--check` — compilation only, no wasmtime (the equivalent of the CI job `wasm-check`). FR-036 / ADR-0011. |
| `scripts/mcp_wasm_gate.sh` | the contract layer (canvas-scene, canvas-mcp, canvas-mcp-headless) under wasm32-unknown-unknown + wasip1 tests (scene 53 + bridge 13 + headless 12) + a REAL MCP session: initialize → tools/list → graph_apply oracle ±1 % → analyze_bottlenecks ρ-gate → negative branches. `--check` — compilation only. FR-037 / ADR-0012. |
| `scripts/wasm_ui_test.sh` | the L2 bench + scenario runner; `--no-build` reuses `target/dist`. |
| `scripts/web_bundle.sh` | release web bundle: canvas-web release build (trunk 0.21.14 pipeline, `lto="thin"`), `wasm-opt -Oz`, size report (§8.8: ≤8 MB raw / ≤4 MB brotli; CI duplicates the numbers into $GITHUB_STEP_SUMMARY). `--dist _site/app` — as in pages-web. |
| `scripts/mcp_wasm_inspector.sh` | a live manual MCP check by the owner without Windows (official inspector; needs node 18+/npx); `--check` — automated acceptance (tools/list + graph_apply oracle). Not part of the gates/CI. |

The inclusion of the web layer `canvas-web` in the compilation stage is
M8/W12 (wasm-port §6.1 item 4).

## Environment facts that break runs (verified 2026-09-25)

- Headless Chromium does NOT composite the WebGPU canvas in
  `captureScreenshot` — only a headed browser under Xvfb (the main L2 trap).
- The SwiftShader flags are mandatory (`--enable-unsafe-webgpu
  --enable-features=Vulkan --use-vulkan=swiftshader
  --use-webgpu-adapter=swiftshader --no-sandbox`), otherwise
  `request_adapter` → None.
- The `wasm-bindgen` CLI version must match the crate in `Cargo.lock`
  (0.2.127 at the time of writing).
- Background processes do not survive between bash calls — the bench http
  server is started by the scenario itself (a child) and shut down at the end.
- The dev wasm is ~34 MB: page load up to 90 s, wait ~7 s for the first frame.

## Cheat sheet (docs/WASM-TESTING.md §6)

```bash
scripts/wasm_gate.sh --check                  # L0 — always
scripts/wasm_gate.sh && scripts/mcp_wasm_gate.sh   # L1 (wasmtime installed once)
ITEM="1035,117" BODY="1078,708" LABEL=about scripts/wasm_ui_test.sh   # L2 scenario
scripts/wasm_ui_test.sh --no-build            # L2 re-run, without rebuild
```

## References

- [docs/WASM-TESTING.md](../../../../docs/WASM-TESTING.md) — the bench recipe:
  levels (§1), agent environment (§2), L2 step-by-step (§3), pitfalls (§4),
  verified chronology (§5), cheat sheet (§6).
- [docs/change-requests/fr-036-wasm-build-gate.md](../../../../docs/change-requests/fr-036-wasm-build-gate.md)
  and [docs/change-requests/fr-037-mcp-wasm-verification.md](../../../../docs/change-requests/fr-037-mcp-wasm-verification.md)
  — the two gate FRs; ADRs: `docs/adr/adr-0011-wasm-build-gate.md`,
  `docs/adr/adr-0012-mcp-wasm-verification.md`.
- `docs/plans/wasm-port.md` (§6.1), `docs/plans/mw2-wasm-bridge.md` — the
  port plans.
