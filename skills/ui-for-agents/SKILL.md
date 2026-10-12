---
name: ui-for-agents
description: UI/UX design knowledge base for building, reviewing, and theming user interfaces - web (CSS/DOM), desktop-native (egui/iced/slint), and WASM/canvas targets. Use when designing or implementing any screen or component: layouts, grids, color, typography, buttons, forms, tables, navigation, modals, dashboards, charts, empty/loading/error states, microcopy, onboarding, accessibility (WCAG), responsive, touch & mobile patterns, internationalization & RTL, design tokens, dark mode, visual styles (Bauhaus, Art Deco, glassmorphism, brutalism, minimalist-swiss, and 30+ presets), or visual QA. Trigger words: UI, UX, design, layout, styling, colors, typography, components, wireframe, landing page, dashboard, chart, accessibility, a11y, theme, dark mode, style, tokens, responsive, mobile, RTL, i18n.
---

# UI/UX Knowledge Library for AI Agents

A stack-agnostic knowledge base that teaches agents how to design and implement user interfaces. Every rule states intent, constraints, and acceptance criteria — never framework syntax — so the same principle (e.g. "body text contrast ≥ 4.5:1") applies to CSS, Slint styles, or a custom wgpu renderer alike.

## Entry protocol

Do not read the whole library. Follow `AGENT-WORKFLOW.md`:

1. **Classify the task** — one of: `NEW-SCREEN`, `NEW-COMPONENT`, `BUILD-KIT`, `INTEGRATE-LIB`, `MODIFY-UI`, `REVIEW-UI`, `FIX-DEFECT`, `THEME-CHANGE`, `NEW-STYLE`, `APPLY-STYLE`.
2. **Load 5–8 files max** using the task→files matrix in `AGENT-WORKFLOW.md`.
3. **Plan before code** — layout, hierarchy, component inventory, state list (loading/empty/error/hover/focus/active/disabled), a11y notes.
4. **Self-review** — run the `## Checklist` section of every loaded file; then run the mandatory QA gate `40-quality/visual-qa-protocol.md` (covers browser and non-browser targets).

## Library map

| Path | Contents |
|---|---|
| `AGENT-WORKFLOW.md` | 7-step protocol, task→files matrix, conflict order, hard rules. Read first. |
| `00-fundamentals/` | `visual-hierarchy`, `color`, `typography`, `spacing-layout-grids`, `iconography`, `elevation-depth`, `motion-principles`, `internationalization-rtl` |
| `10-design-system/` | `design-tokens`, `theming-dark-mode`, `building-a-ui-kit`, `using-existing-libraries`, `component-api-design`, `component-documentation` |
| `20-components/` | `components-catalog`, `buttons-and-actions`, `forms-and-inputs`, `navigation-patterns`, `modals-and-overlays`, `tables-and-data-lists`, `feedback-toasts-alerts` |
| `30-ux-patterns/` | `forms-validation-ux`, `empty-loading-error-states`, `microcopy`, `onboarding-first-run`, `search-filtering-sorting`, `dashboards-data-viz-ux`, `data-visualization`, `touch-and-mobile` |
| `40-quality/` | `accessibility-wcag`, `responsive-adaptive`, `usability-heuristics`, `perceived-performance`, `visual-qa-protocol` |
| `50-platforms/` | `web-css-dom`, `rust-native-gui`, `wasm-canvas-rendering`, `cross-platform-parity` |
| `60-styles/` | `style-design-process`, `style-catalog` (selector), 11 product + 22 movement presets (bauhaus, art-deco, brutalism, glassmorphism, minimalist-swiss, y2k-chrome …) |
| `REFERENCES.md`, `GLOSSARY.md` | Curated source links; terminology |

## Hard floors (never violate)

Text contrast ≥ 4.5:1 (large text & UI components ≥ 3:1) · pointer targets ≥ 24×24 px (44×44 for touch) · micro-interactions 150–300 ms · measure 45–75 characters · body 16 px-equivalent, line-height 1.4–1.6 · never encode state in color alone · never fake progress · if a rule cannot be followed, say so explicitly in the deliverable notes.

## File format

Every knowledge file follows one shape: `Read when` header → numbered `Rules` (Rule / Why / Example) → `Checklist` (pass/fail) → `Anti-patterns` → `Sources`. Run the checklist as your self-review; extrapolate new cases from the Why.

## Conflicts

When rules collide: accessibility (WCAG) wins over platform specifics (`50-platforms/`), which wins over this protocol, which wins over pretraining habits.
