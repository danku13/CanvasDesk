---
name: shell-win32
description: Win32/shell integration rules for CanvasDesk — undocumented techniques (WorkerW, 0x052C/0x7402), licensing-safe porting, the RECIPES constraints (R2/R3/R4/R5/R9/R10/R13), the enum-wrapper idiom and the mandatory fallback. Load for any canvas-shell or desktop-embedding task.
version: 1
---

# $shell-win32

Rules for Windows shell integration. Decomposes AGENTS.md "Win32 / shell rules"
(5 numbered rules) and maps them onto docs/RECIPES.md; AGENTS.md stays the law.

## When to load

- Route `shell-win32` (docs/agent/routes.yaml): anything touching
  `crates/canvas-shell/` (esp. `src/desktop/`), Progman/WorkerW/DefView,
  messages 0x052C / 0x7402, icon hiding, re-parenting, DPI on the embedded
  window, shell events, desktop drag-drop.
- `docs/RECIPES.md` is a heavy read-on-demand document: load only the recipes
  the task touches, using the map below.

## Mandatory rules (5 — keep 1:1)

1. **Undocumented techniques — only with runtime detection and a fallback.**
   (WorkerW, messages 0x052C and 0x7402) — only with runtime detection and a
   fallback to a normal window, per the recipes in
   [docs/RECIPES.md](../../../../docs/RECIPES.md); each use carries a comment
   linking the recipe. Claims about Progman/WorkerW/DefView behavior not
   confirmed by RECIPES or runtime detection are considered unverified.
2. **Recipes are implemented cleanly.** Lively — GPL-3.0, Seelen UI — AGPL-3.0.
   Copying code, identifiers, structure and comments is forbidden; port
   mechanics only.
3. **Key constraints from RECIPES:**
   - send 0x052C ONLY if WorkerW is absent (R4);
   - `WS_EX_LAYERED` + `SetLayeredWindowAttributes(255)` strictly before
     `SetParent` (R2);
   - verify styles after re-parenting (R3);
   - idempotent icon hiding via reading `SHGetSetSettings` before the 0x7402
     toggle (R5);
   - do NOT use `SPI_SETDESKWALLPAPER` on a raised desktop (R9);
   - `SHGetSetSettings` with fSet does not work on Win10+ — do not waste time.
4. **Win32 enum wrappers — per the RECIPES R13 idiom** (boxed closure via
   LPARAM, `extern "system"` trampoline, safe API outward).
5. **Any desktop-embedding step failure → fallback to a normal window + a
   warning to the user** (not only for unknown OS versions).

## Recipe map (docs/RECIPES.md §2–§4)

| Recipe | Subject | Needed when |
|---|---|---|
| R1 | Hierarchy detection: two independent sources agree | writing the runtime detection code |
| R2 | Embedding into the raised desktop — full mechanics (the core of T15) | any SetParent / WorkerW work |
| R3 | Style scrubbing and the window initialization order | window init / re-parenting |
| R4 | Idempotent WorkerW spawn | spawning or finding WorkerW |
| R5 | Idempotent icon hiding | the desktop icon toggle |
| R9 | The RefreshDesktop trap (both stumbled into it) | any refresh / redraw attempt |
| R10 | DPI after reparenting | DPI changes on the embedded window |
| R13 | The safe enum-wrapper pattern (copy as an idiom) | any Win32 enumeration |
| R14 | Graceful degradation when embedding fails | the fallback path (rule 5) |

Also in RECIPES (load on demand): R6 WorkerW destruction watch, R7 Explorer
crash anti-flood, R8 session lock/unlock, R11 hidden system-events window,
R12 SHChangeNotifyRegister, R15–R17 render pause / energy saving. Licensing
framework — RECIPES §0 (read first).

## Also mandatory (AGENTS.md "Do not", shell items)

- Do not send 0x052C when WorkerW exists; do not use `SPI_SETDESKWALLPAPER`
  on a raised desktop.
- Do not replace the shell — taskbar and tray remain Explorer's.
- Do not spread platform code across `canvas-app` — traits and cfg-sections of
  platform crates only; unix equivalents of Win32 techniques — only per the
  decision table [docs/plans/M7-crossplatform.md](../../../../docs/plans/M7-crossplatform.md)
  §3.2, not from memory.

## Gates (route shell-win32)

- Runtime detection + a fallback to a normal window implemented for every
  undocumented technique used; each use links its RECIPES recipe in a comment.
- `cargo test --workspace` green; `unsafe` only in `canvas-shell` /
  `canvas-widgets`, every block with a SAFETY comment (AGENTS.md code style).
- Plan context: [docs/plans/T15-desktop-embed.md](../../../../docs/plans/T15-desktop-embed.md)
  (the embedding task) and `docs/plans/T17-desktop-polish.md` (polish/edge
  cases); shell events — `docs/plans/T16-shell-events.md`.

## References

- `docs/RECIPES.md` — the recipes (§2 desktop-embedding, §3 infrastructure,
  §4 pause/energy), licensing (§0), coverage map (§1), widget permissions
  model (§8).
- `crates/canvas-shell/src/desktop/` — the implementation: `attach.rs`,
  `hierarchy.rs`, `interop.rs`, `monitor.rs`, `menu.rs`, `explorer.rs`,
  `icons.rs`, `single_instance.rs`.
- `docs/plans/M7-crossplatform.md` — the cross-platform decision table.
