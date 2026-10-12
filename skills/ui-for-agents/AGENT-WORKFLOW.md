# Agent Workflow Protocol

> **Read when:** once at the start of any UI-related session. **Section:** root protocol. **Related:** README.md, 40-quality/visual-qa-protocol.md

This protocol defines how an agent loads and applies this library. It exists to prevent two recurring failure modes: (a) dumping the whole library into context and diluting attention, and (b) shipping UI without a quality gate. Follow the steps in order.

## The 7-step protocol

### S0 — Classify the task
Pick exactly one: `NEW-SCREEN`, `NEW-COMPONENT`, `BUILD-KIT`, `INTEGRATE-LIB`, `MODIFY-UI`, `REVIEW-UI`, `FIX-DEFECT`, `THEME-CHANGE`, `NEW-STYLE` (invent a custom visual style), `APPLY-STYLE` (build UI in a chosen style). If scope is genuinely ambiguous (e.g. "improve the settings page" could be REVIEW-UI or MODIFY-UI), ask the user one clarifying question instead of guessing — rework is more expensive than a question.

### S1 — Select files with the matrix
Use the table below. Load **Mandatory** files; load **On demand** files only when the task actually touches their topic.

| Task type | Mandatory | On demand |
|---|---|---|
| NEW-SCREEN | 00/visual-hierarchy, 00/spacing-layout-grids, 00/color, 00/typography, 20/components-catalog, 40/visual-qa-protocol | 20/ files for component types on the screen; 30/ files for flows present (forms, search…); 30/touch-and-mobile for touch targets; 30/data-visualization for chart-heavy views; 00/internationalization-rtl for localized products; 50/ file for the target platform |
| NEW-COMPONENT | 10/component-api-design, 20/components-catalog, 40/accessibility-wcag, 40/visual-qa-protocol | 00/fundamentals files for visual decisions; 10/component-documentation.md |
| BUILD-KIT | all `00-fundamentals/` + all `10-design-system/` | 40/accessibility-wcag, 50/cross-platform-parity |
| INTEGRATE-LIB | 10/using-existing-libraries, 20/components-catalog | 10/design-tokens (to map library variables), 10/theming-dark-mode |
| MODIFY-UI | file(s) covering the modified element in `20-components/` or `30-ux-patterns/` + 40/visual-qa-protocol | 00/fundamentals if the change is visual; 50/ platform file |
| REVIEW-UI | 40/usability-heuristics, 40/accessibility-wcag, 40/visual-qa-protocol | 30/empty-loading-error-states, 20/components-catalog, 30/touch-and-mobile (touch products), 30/data-visualization (charts), 00/internationalization-rtl (localized products) |
| FIX-DEFECT | the file whose rules were violated | follow `Related:` links from that file |
| THEME-CHANGE | 10/design-tokens, 10/theming-dark-mode | 00/color; 00/internationalization-rtl (direction-aware tokens); 50/ file for renderer limits |
| NEW-STYLE | 60/style-design-process, 60/style-catalog, 10/design-tokens, 00/color, 00/typography | 60/ existing presets as references; 00/motion-principles; 40/visual-qa-protocol |
| APPLY-STYLE | 60/<style>.md, 10/design-tokens, 00/visual-hierarchy, 40/visual-qa-protocol | 20/ files per component types; 50/ platform file; 60/style-catalog if the choice is not yet fixed |

### S2 — Load minimally
- Budget: 5–8 files for a typical task. Never load a whole section folder "just in case".
- Follow a `Related:` link only when you hit a concrete case that file covers. Links are lazy, not eager.

### S3 — Plan before code
Write a short plan (5–10 bullets): layout structure, visual hierarchy of the view, component inventory, states to cover (loading / empty / error / hover / focus / active / disabled), and accessibility notes. A screen whose plan has no states list is incomplete by definition — ship it as broken.

### S4 — Implement by rules
- Tokens first: no hardcoded colors, sizes, spacing, radii, or durations — see `10-design-system/design-tokens.md`.
- Component behavior per `20-components/` files; flow behavior per `30-ux-patterns/` files.
- Platform execution details (CSS specifics, egui/iced specifics, WASM specifics) per the relevant `50-platforms/` file.

### S5 — Self-review with checklists
Run the `## Checklist` section of every file you loaded. A failed checklist item is a defect: either fix it, or explicitly document why it cannot pass in this context. Silent skips are violations.

### S6 — Visual QA gate (mandatory, always)
Run `40-quality/visual-qa-protocol.md`. It defines passes for browser UI **and** non-browser targets: native Linux (X11/Wayland screenshots), golden-image/snapshot tests (egui, iced, slint harnesses), a11y-tree assertions via AccessKit, and WASM harness rendering. The task is done only when the QA checklist passes or every deviation is documented in the deliverable notes.

### S7 — Report
Summarize: task type, files loaded, rules that materially changed the implementation, checklist results, known limitations. This lets a reviewer verify the work without re-reading everything.

## Conflict resolution order
When rules collide, the higher item wins:
1. **Accessibility (WCAG).** Non-negotiable floor in every renderer.
2. **Platform file** (`50-platforms/`). Wins over generic rules for rendering and toolkit specifics (e.g. scrollbars, keyboard conventions).
3. **This protocol.** Wins over any conflicting habit from pretraining data ("I usually do X" is not an argument).

## Hard rules (never violate)
- Never invent thresholds. Use the cited ones: text contrast ≥ 4.5:1 (large text and UI components ≥ 3:1), minimum pointer target 24×24 px (44×44 recommended for touch), micro-interaction durations 150–300 ms, reading measure 45–75 characters, body size 16 px-equivalent, body line-height 1.4–1.6.
- Never encode state in color alone; never use placeholder as a substitute for a label; never blame the user in error copy; never fake progress.
- If a rule cannot be followed (toolkit limit, deadline, legacy), say so explicitly in the deliverable notes. Silence is a violation.
