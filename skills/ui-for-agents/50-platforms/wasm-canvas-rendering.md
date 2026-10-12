# WASM & Canvas Rendering

> **Read when:** rendering UI to a `<canvas>`/WebGL/wgpu surface, or shipping Rust as WASM where the DOM is not your UI layer. | **Section:** 50-platforms | **Related:** 40/accessibility-wcag.md, 30/empty-loading-error-states.md, 40/visual-qa-protocol.md, 40/perceived-performance.md, 50/rust-native-gui.md

**Core idea:** the DOM gave you accessibility, selection, input, and sizing for free — a canvas gives you pixels. Everything below is a platform capability you must now design and implement yourself.

## Rules

### R1. Rebuild the accessibility layer deliberately
**Rule:** Expose an accessibility tree for the canvas: use an AccessKit adapter (wgpu/winit ecosystems) or a mirrored overlay DOM layer with real roles, names, and states for every interactive element, kept in sync with focus and state.
**Why:** Screen readers read the a11y tree, not pixels; a canvas without one makes the whole UI invisible, and the contrast ≥ 4.5:1 / target ≥ 24×24 px rules are moot if the user cannot reach the control at all.
**Example:** A canvas toolbar mirrors each painted button as `<div role="button" tabindex="0" aria-pressed="false" aria-label="Bold">` positioned over its rect; arrow keys move focus in both layers.

### R2. Canvas text is not selectable — provide copy paths
**Rule:** Any text the user may want to reuse (values, IDs, logs, code, error messages) needs a copy affordance: a copy button, an overlay DOM mirror with real selection, or selection rendered on the canvas.
**Why:** Painted glyphs are not text nodes — users cannot select, copy, translate, or search them; blocking copy on data-heavy UIs forces error-prone retyping and fails "copy the error text to file a bug" flows.
**Example:** A chart tooltip renders on canvas with a "Copy value" button; a code viewer overlays a transparent `<pre>` synced to the canvas layout for native selection.

### R3. Handle IME composition, not just keycodes
**Rule:** Route keyboard input through composition events (`compositionstart/update/end`, `data`, or a hidden `<input>` / `beforeinput` path) and render the in-progress composition string inline with the candidate-window position — never map CJK or accented input from raw keydown codes.
**Why:** Japanese/Chinese/Korean and accented Latin input is multi-step (keys → candidates → commit); keydown pipelines commit partial gibberish, block the IME popup, or fire on the wrong keystroke because Enter means "pick candidate" mid-composition.
**Example:** A hidden input feeds composition `data` into the canvas text field; the canvas confirms text only after `compositionend`, not on any keydown during composition.

### R4. Use pointer events with capture; keep keyboard focus visible
**Rule:** Use pointer events with `setPointerCapture` for drags (movements keep arriving outside the canvas), handle `pointercancel`, re-hit-test after resize/DPI change, and draw a visible focus ring when keyboard focus lands on the surface.
**Why:** Without capture a fast drag "loses" the pointer mid-gesture and rows/thumbs stick; without visible focus and keyboard handling the canvas is mouse-only by accident, violating keyboard operability.
**Example:** `canvas.setPointerCapture(e.pointerId)` on `pointerdown` for a slider thumb; a `:focus-visible`-equivalent ring painted when focus arrives via keyboard.

### R5. Size the canvas from devicePixelRatio
**Rule:** On every resize and DPI change: `canvas.width = cssWidth * devicePixelRatio`, scale the drawing context by the ratio, set CSS size separately, and listen for DPI changes across monitors.
**Why:** The backing store is physical pixels while layout is CSS pixels: skipping the ratio renders blurry on hi-dpi, doubling it renders oversized, and dragging the window between monitors changes the ratio at runtime.
**Example:** `function resize(){ const d = devicePixelRatio; canvas.width = cssW * d; canvas.height = cssH * d; ctx.setTransform(d, 0, 0, d, 0, 0); }`

### R6. Stage startup: paint first, load the rest progressively
**Rule:** Split the WASM payload: paint a meaningful first frame (shell + loader) before heavy modules instantiate, show real progress, and never leave a black canvas as the "loading state"; stage functionality so the shell is interactive before the heavy views mount.
**Why:** WASM compile + instantiate + GPU init takes visible time; users read an empty black rectangle as a crash within seconds, while a staged shell reads as working software (see 30-ux-patterns/empty-loading-error-states.md and 40-quality/perceived-performance.md).
**Example:** HTML shell shows a layout skeleton + progress bar; `WebAssembly.instantiateStreaming` runs async; the renderer swaps in when ready; a timeout swaps in an error state with Retry.

### R7. Provide a software fallback for missing GPU
**Rule:** Probe adapter support at startup (`navigator.gpu.requestAdapter()`, WebGL context creation); on failure, switch to a software/CPU path or a reduced static-rendering mode — with a visible notice — instead of a white screen.
**Why:** VMs, old drivers, and locked-down browsers expose no GPU; a renderer that assumes one panics at init and the user cannot tell their environment apart from your bug.
**Example:** No adapter ⇒ fall back to a 2D-canvas renderer for tables/lists, showing "Accelerated graphics unavailable — reduced visuals enabled".

### R8. Treat host-page limits as design constraints
**Rule:** Await fonts before first canvas text and re-measure after `document.fonts.ready`; wrap storage calls in quota-aware error handling; call clipboard and file-save APIs only from within a user gesture.
**Why:** Canvas measures text with whatever font is loaded — a late font swap silently mis-measures every layout; storage throws when quota is exceeded and clipboard writes reject without a gesture, and unhandled rejections read as freezes.
**Example:** `await document.fonts.load('16px Inter'); await document.fonts.ready; buildLayout();` — and `navigator.clipboard.writeText(v)` only inside a click handler.

### R9. The platform floor still applies to pixels
**Rule:** Enforce the non-negotiable numbers inside renderer constants: text contrast ≥ 4.5:1 (large text and UI components ≥ 3:1) against the painted background, hit targets ≥ 24×24 px, motion durations 150–300 ms.
**Why:** Canvas computes no contrast and exposes no hit targets to assistive tech, so violations are both easier to commit and harder to detect — they must be built into theme constants and asserted in tests, never hoped for.
**Example:** The theme struct carries precomputed `text_on_surface = 7.2:1` and `min_target = 24`; a golden test asserts tooltip text passes 4.5:1 in light and dark themes.

### R10. Keep a WASM/headless-browser test harness — even for desktop products
**Rule:** If the rendering core is shared with a WASM target, run screenshot and console-error checks in a headless browser in CI per 40-quality/visual-qa-protocol.md; fail the build on console errors or pixel diffs beyond threshold.
**Why:** The browser is the cheapest CI-renderable surface for the same core (no display server needed), and it catches shader, font, and state bugs that logic-only unit tests never execute.
**Example:** `wasm-pack build && playwright screenshot app.html golden.png` + a script asserting zero `console.error` entries and a pixel-diff below threshold vs the golden image.

### R11. Redraw on change, capped by the frame budget
**Rule:** Redraw only when something changed (dirty flag) or while an animation is actively running inside the 150–300 ms band; coalesce input events; never spin a render loop at max rate over static content.
**Why:** Canvas has no free compositing — every frame repaints pixels, so an always-on loop burns battery and steals the frame budget from actual interaction, which reads as jank first on low-end devices (see 40-quality/perceived-performance.md).
**Example:** `if dirty { draw(); dirty = false; } requestAnimationFrame(loop);` — a hover transition runs its 200 ms and then stops, leaving the loop idle.

### R12. Virtualize and clip large scenes
**Rule:** Paint only what intersects the viewport: cull offscreen items, clip to dirty regions where practical, and virtualize long lists (layout math for all rows, glyphs for visible rows).
**Why:** Full-scene repaints scale linearly with content; a 10k-row list that paints everything each frame stutters exactly when the dataset gets impressive, and weak GPUs drop frames first.
**Example:** `if row.y + h > scroll.y && row.y < scroll.y + view_h { paint(row); }` — with a layout cache keyed by scroll position so virtualization itself stays O(visible).

## Checklist
- [ ] Every interactive canvas element exists in an a11y tree (AccessKit or DOM mirror) with role + name + state
- [ ] All user-relevant text has a copy or selection path
- [ ] CJK/accented input verified via composition events, not keycodes
- [ ] Drags survive leaving the canvas (pointer capture; `pointercancel` handled)
- [ ] Backing store sized by devicePixelRatio; verified at 100 / 150 / 200 % zoom
- [ ] First frame paints a shell/loader, not black; timeout shows an error state with Retry
- [ ] No-GPU environment renders a functional fallback with a visible notice
- [ ] Fonts awaited before text measurement; clipboard/storage errors handled
- [ ] Contrast 4.5:1, targets 24×24 px, motion 150–300 ms enforced in renderer constants
- [ ] Headless browser screenshot + console check runs in CI

## Anti-patterns
- A canvas with no a11y tree and a "we'll add it later" note
- Rendering important text with no way to copy or select it
- Keydown-only text input that breaks Japanese/Chinese/Korean IME
- Fixed `canvas.width = 800` with CSS-stretched scaling (blurry or huge)
- A black rectangle for 5 seconds during WASM instantiate
- `requestAdapter() == null` → panic or white screen
- Clipboard writes on page load (no user gesture) without rejection handling
- Shipping the shared core with zero browser-render tests because "it's a desktop app"

## Sources
- MDN — devicePixelRatio and canvas sizing: https://developer.mozilla.org/en-US/docs/Web/API/Window/devicePixelRatio
- MDN — composition events (IME): https://developer.mozilla.org/en-US/docs/Web/API/CompositionEvent
- MDN — pointer events and capture: https://developer.mozilla.org/en-US/docs/Web/API/Pointer_events
- MDN — Clipboard API (gesture requirement): https://developer.mozilla.org/en-US/docs/Web/API/Clipboard_API
- web.dev — loading WebAssembly efficiently: https://web.dev/articles/loading-wasm
- wgpu: https://wgpu.rs
- AccessKit (adapters incl. web): https://accesskit.dev
- MDN — Canvas API tutorial: https://developer.mozilla.org/en-US/docs/Web/API/Canvas_API
- MDN — WebAssembly overview: https://developer.mozilla.org/en-US/docs/WebAssembly
- WCAG 2.2 — contrast, targets, keyboard: https://www.w3.org/TR/WCAG22/
