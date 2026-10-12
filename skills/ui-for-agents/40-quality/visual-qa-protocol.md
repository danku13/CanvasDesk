# Visual QA Protocol — the mandatory final gate

> **Read when:** any UI work is claimed done — this is step S6 of AGENT-WORKFLOW.md, and nothing ships before it runs. **Section:** 40-quality. **Related:** accessibility-wcag.md, responsive-adaptive.md, usability-heuristics.md, perceived-performance.md, 30-ux-patterns/empty-loading-error-states.md, 50-platforms/rust-native-gui.md, 50-platforms/wasm-canvas-rendering.md

**Purpose:** catch visual and interaction defects without a human designer. An agent cannot "look" the way a designer does — so it runs fixed passes with concrete, pass/fail checks instead of impressions. Run every pass that applies to the target; document the skip for any that genuinely does not.

## The 7 passes

### Pass 1 — Layout
Verify geometry and order, not style opinions.
- Every block aligns to the declared axis/axes; nothing sits off-grid by 1–2 px "where it looks fine"
- Spacing values come from the scale — no 13 px between two 12 px-scale neighbors
- No overflow, no clipped text, no unintentional overlaps; z-order is deliberate
- Minimum supported size holds: collapse/scroll behavior matches the declared rules (responsive-adaptive.md R5)
- Component metrics match the spec: button heights, icon sizes, and radii consistent across the view
- Squint/blur test: primary action and key data identifiable without reading (00-fundamentals/visual-hierarchy.md R12)

### Pass 2 — Content
Layouts lie until they hold real content.
- Test with the longest expected string per field/list/column: truncation behaves as designed (ellipsis, wrap, scroll) and the full value stays available (tooltip, detail view, wrap)
- i18n samples: one long German compound, one short CJK string, one RTL sample — layout survives ~+40% expansion and RTL mirroring
- Numbers, dates, currencies in one consistent, locale-correct format per view
- Empty values render the designed placeholder ("—"), never "null", "undefined", or a collapsed blank row
- Review screenshots use realistic data — never lorem ipsum

### Pass 3 — States
For EVERY interactive element: default, hover, focus, active/pressed, disabled, loading, empty, error.
- All designed states are rendered and visually distinct where they must be
- Focus is visible and ≥ 3:1 against adjacent colors (accessibility-wcag.md R5)
- Disabled elements are explainable: why they are disabled is discoverable (tooltip, helper text, adjacent copy)
- Hover and focus remain distinguishable from each other when both can be active at once
- State coverage matches the S3 plan — an element with no designed error state is itself a defect

### Pass 4 — Interaction
Walk the core flow keyboard-only first, then with pointer.
- Correct focus order (matches reading order, no jumps); skip link works where navigation repeats
- Esc closes/cancels overlays; back behaves consistently; focus returns to the trigger
- No keyboard traps; every action operable by keyboard alone (2.1.1 / 2.1.2)
- Every hover path has a non-hover equivalent (touch and keyboard reach the same actions)
- Async results are perceivable without looking at the screen (live-region announcement or a11y alert) whenever the flow depends on them

### Pass 5 — Accessibility
- Sample contrast of real rendered pairs — text, icons, borders, focus ring — in BOTH themes: 4.5:1 text, 3:1 large text and UI components
- Target sizes ≥ 24×24 px (44×44 touch) on the dense controls: table row actions, toolbar icons, pagination
- Names, roles, and states exposed to the a11y tree (ARIA in DOM apps, AccessKit in native apps); icon-only buttons have names
- Landmarks/regions identified where the platform supports them (main, nav, search, complementary)
- Reduced-motion variant renders sanely: no dead UI, no information carried by motion alone

### Pass 6 — Theme
- Light AND dark screenshots captured for each key view and compared deliberately, side by side
- No hardcoded colors: changing a token flips every dependent element (grep/audit for literals before shipping)
- Surfaces, borders, and semantic colors are remapped in dark mode — not naively inverted
- Contrast is re-checked in dark theme; dark-mode failures are usually new pairs, not the ones checked in light
- Semantic colors (error/success/warning) and selection/focus states verified in dark theme specifically

### Pass 7 — Edge cases
- 1000 rows (or the realistic maximum) in lists/tables: performance holds, virtualization works, scrollbar behavior sane
- 255-character strings everywhere a string can be long: names, titles, emails, error messages
- Empty dataset: the designed empty state, not a blank region (30-ux-patterns/empty-loading-error-states.md)
- Offline / network failure: designed error state with a reachable retry
- Permission-denied: explainable state, not a silent no-op
- Extreme values: 0, negative, huge numbers; zero-results search; first-run with no data
- Rapid repeated submits do not duplicate rows, stack overlays, or double-fire background jobs

## Non-browser QA paths

Targets without a browser DOM still get visual QA. Pick the path that matches the renderer; skipping QA because "there is no browser" is not a valid outcome.

### Native Linux (Rust GUI: egui / iced / slint / gtk)
- Capture screenshots via X11/Wayland tooling — `grim` (wlroots-based Wayland), `gnome-screenshot` / `spectacle`, X11 `import` / `scrot` — or the toolkit's own capture API, then run the passes on the captured frames
- If no display manager is available in the environment, run under a virtual framebuffer or headless compositor (Xvfb, Weston/Sway headless) — "no display here" is a harness problem, not a QA exemption
- Golden-image / snapshot testing: harnesses such as egui's kittest-style snapshot tests, iced's screenshot-based testing, or slint's testing module render components deterministically and diff against baseline images
  - Baselines live in the repository and are reviewed like code
  - A diff is either an accepted change (update the baseline deliberately, note it in the deliverable) or a defect (fix the render)
  - Determinism matters: pin fonts, DPI factor, and RNG seed in the harness, or diffs become noise and get ignored

### Accessibility assertions on native targets
- Dump the AccessKit tree at runtime and assert: every interactive node has role + name; states (checked/disabled/selected) present; focus order sane
- This is the native equivalent of an axe pass — automate it in the test suite, not as a one-off manual dump

### WASM / canvas rendering
- Render the app in a headless browser harness (e.g. Playwright) even when the product ships outside the browser: screenshot the canvas, run the passes on it
- Require a console free of errors and panics — a WASM panic logged to console is a defect even when the rendered frame looks right
- Drive one core interaction programmatically (click/keypress) and assert the frame changes — canvas UIs must prove they respond, not just paint

### No visual target at all (headless service)
- Assert geometry/properties programmatically (computed sizes, token values, layout math) and snapshot the accessibility tree as the artifact
- Document which passes were replaced by which assertions; an "N/A" without a replacement is a failed gate

## Pass log template

Record one line per pass per scope in the deliverable notes:

```
Pass | Scope                    | Result | Notes
1    | Settings page, light     | PASS   |
3    | Invoice form             | DEV    | loading state missing → fix #412
6    | Full app                 | DEV    | dark theme: 2 contrast fails → fix #415
```

`PASS` = all checks green. `DEV` = documented deviation (reason + owner + follow-up). `FAIL` = blocking; do not ship.

## Ship rule

UI is done when **all applicable passes are green** — or **every deviation is documented** with three fields:

- **Reason** — why it cannot pass in this context (toolkit limit, environment, scope)
- **Owner** — who or what will fix it
- **Follow-up** — ticket, task, or issue reference

A screenshot set plus a pass log belongs in the deliverable. A claim of "done" without them is not verifiable and is treated as not done (AGENT-WORKFLOW.md S6/S7).

## Checklist
- [ ] Pass 1: axes aligned, spacing scale respected, no overflow/clipping, min supported size holds
- [ ] Pass 2: longest strings, i18n + RTL samples, consistent number/date formats; no lorem ipsum
- [ ] Pass 3: all 8 states rendered for every interactive element; focus visible; disabled explainable
- [ ] Pass 4: keyboard-only walkthrough completed; focus order, Esc, and no traps verified
- [ ] Pass 5: contrast sampled in both themes; target sizes; names/roles/states exposed; reduced-motion checked
- [ ] Pass 6: light + dark screenshots captured and compared; no hardcoded colors
- [ ] Pass 7: 1000 rows, 255-char strings, empty dataset, offline, permission-denied exercised
- [ ] Non-browser target: screenshots via platform tooling or headless harness; a11y-tree dump asserted
- [ ] Golden-image diffs reviewed: baselines updated intentionally or defects filed
- [ ] Ship rule satisfied: all applicable passes green, or each deviation documented with reason + owner + follow-up

## Anti-patterns
- "It looks fine at 1280 px" as the entire QA report
- Screenshots only of the happy path with short fake data
- Skipping the states pass because "the component library handles states"
- Regenerating golden-image baselines (`--update-snapshots`) until green without reviewing diffs
- Baselines stored outside the repo, or regenerated per machine due to unpinned fonts/DPI
- A WASM build verified only by "it compiled"; console errors ignored
- A native app shipped without a single screenshot because the CI box is headless
- A deviation list with no owners or follow-ups ("known issues" as a graveyard)

## Sources
- AGENT-WORKFLOW.md — S6 defines this file as the mandatory gate
- W3C — WCAG 2.2 quick reference (a11y pass criteria): https://www.w3.org/WAI/WCAG22/quickref/
- Playwright — screenshots and headless harnesses: https://playwright.dev/docs/screenshots
- AccessKit — runtime accessibility tree: https://github.com/AccessKit/accesskit
- egui — egui_kittest snapshot harness: https://github.com/emilk/egui/tree/master/crates/egui_kittest
- iced — testing and screenshots: https://github.com/iced-rs/iced
- Slint — testing & mock renderer: https://docs.slint.dev/latest/docs/
- grim — screenshot utility for wlroots Wayland compositors: https://sr.ht/~emersion/grim/
- Xvfb — virtual framebuffer X server: https://www.x.org/releases/X11R7.6/doc/man/man1/Xvfb.1.xhtml
