# UI/UX Knowledge Library for AI Agents

![Agent Skills compatible](https://img.shields.io/badge/agent_skills-compatible-8A2BE2) ![Knowledge files](https://img.shields.io/badge/knowledge_files-75+-blue) ![Style presets](https://img.shields.io/badge/style_presets-33-orange) ![Live demo](https://img.shields.io/badge/demo-live-success)

A comprehensive, **stack-agnostic** knowledge base that teaches AI agents (and helps humans) design and implement user interfaces: web apps, desktop-native applications (including Rust GUI toolkits such as egui / iced / slint), and WASM- or canvas-rendered UIs where a browser DOM may not exist at all.

Four design decisions define this library:

1. **Stack-agnostic.** Rules describe intent, constraints and acceptance criteria — not framework syntax. Rendering-specific execution is isolated in `50-platforms/`, so the same principle (e.g. "body text contrast ≥ 4.5:1") applies to CSS, Slint styles, or a custom wgpu renderer alike.
2. **Rule + Why.** Every rule is followed by a short explanation of the underlying mechanism, so an agent can extrapolate correctly to cases the library does not explicitly list.
3. **Adapted best practice.** Distilled from Material Design 3, Apple HIG, WCAG 2.2, Nielsen Norman Group research, and leading open design systems. Full link list lives in `REFERENCES.md`.
4. **Progressive loading.** Files are small (100–170 lines), heavily cross-linked, and grouped so an agent can load 3–8 files per task instead of the whole library.

> **If you are an agent:** start with [`SKILL.md`](SKILL.md), then [`AGENT-WORKFLOW.md`](AGENT-WORKFLOW.md), then load only the files your task requires.
> **If you are a human:** jump to [For designers & developers](#for-designers--developers).

## Live demo

**[Content OS landing — one structure, five heritage styles](https://danku13.github.io/ui-for-agents/)** — a product-marketing landing rendered from a single DOM in Art Nouveau, De Stijl, Bauhaus, Constructivism and Art Deco, with a runtime style switcher. Every theme's tokens come verbatim from the matching preset in `60-styles/`. Source and structure contract: [`demo/content-os-landing/`](demo/content-os-landing/) (`STRUCTURE.md` explains the DOM/token split).

## Use it with your AI agent

The repo ships a standard **Agent Skills** wrapper (`SKILL.md` with frontmatter), so it plugs into any skill-aware agent as-is.

| Environment | Setup |
|---|---|
| Claude Code (project) | `git clone https://github.com/danku13/ui-for-agents .claude/skills/ui-for-agents` — the skill is discovered automatically |
| Claude Code (personal) | Clone into `~/.claude/skills/ui-for-agents` |
| Claude app (web/desktop) | Download the repo as ZIP, then upload it under *Settings → Capabilities → Skills* |
| Cursor / Windsurf / Codex | Clone anywhere, then add `SKILL.md` (or `AGENT-WORKFLOW.md`) to your rules/context files and point the agent at the cloned folder |
| Any chat agent with file access | Provide the repo folder and instruct: *"Start from SKILL.md; follow AGENT-WORKFLOW.md; load only the files your task requires"* |

**What the agent gets:** a 7-step working protocol (classify task → load 5–8 relevant files → plan states → implement by rules → run checklists → mandatory visual QA gate → report), a task→files routing matrix, measurable hard floors (contrast, target sizes, durations, measure), and a mandatory quality gate covering browser **and** non-browser renderers.

**Example prompts that route well:** *"Build a settings screen in React following your UI skill"*, *"Review this dashboard for WCAG and dashboard-UX violations"*, *"Create a design-token set and apply the bauhaus preset to this landing"*, *"Build the same screen for mobile: touch targets, thumb zone, bottom sheet"*, *"Make this product ready for Arabic and German locales"*.

## Library map

| Section | Path | Contents |
|---|---|---|
| Skill wrapper | `SKILL.md` | Agent Skills entry point — routes to the protocol below. |
| Protocol | `AGENT-WORKFLOW.md` | The 7-step loading & working protocol, task→files matrix, conflict-resolution order. **Read first.** |
| Fundamentals | `00-fundamentals/` | Universal visual laws: `visual-hierarchy`, `color`, `typography`, `spacing-layout-grids`, `iconography`, `elevation-depth`, `motion-principles`, `internationalization-rtl`. |
| Design system | `10-design-system/` | `design-tokens`, `theming-dark-mode`, `building-a-ui-kit`, `using-existing-libraries`, `component-api-design`, `component-documentation`. |
| Components | `20-components/` | `components-catalog` (selection), `buttons-and-actions`, `forms-and-inputs`, `navigation-patterns`, `modals-and-overlays`, `tables-and-data-lists`, `feedback-toasts-alerts`. |
| UX patterns | `30-ux-patterns/` | `forms-validation-ux`, `empty-loading-error-states`, `microcopy`, `onboarding-first-run`, `search-filtering-sorting`, `dashboards-data-viz-ux`, `data-visualization`, `touch-and-mobile`. |
| Quality | `40-quality/` | `accessibility-wcag`, `responsive-adaptive`, `usability-heuristics`, `perceived-performance`, `visual-qa-protocol` (includes non-browser QA paths — mandatory final gate). |
| Platforms | `50-platforms/` | `web-css-dom`, `rust-native-gui`, `wasm-canvas-rendering`, `cross-platform-parity`. |
| Styles | `60-styles/` | `style-design-process` (how to invent a style), `style-catalog` (selector: decision table + movement map), plus 33 ready presets — 11 product presets (`minimalist-swiss`, `dark-premium`, `glassmorphism`, `neumorphism`, `brutalism`, `editorial`, `playful-friendly`, `corporate-trust`, `retro-vintage`, `futuristic-neon`, `handcrafted-organic`) and 22 movement presets (`bauhaus`, `de-stijl`, `constructivism`, `art-nouveau`, `art-deco`, `mid-century-modern`, `pop-art`, `psychedelia`, `punk-zine`, `memphis`, `japanese-zen`, `dark-academia`, `maximalism`, `neubrutalism`, `acid-graphics`, `skeuomorphism`, `flat-design`, `corporate-memphis`, `claymorphism`, `y2k-chrome`, `frutiger-aero`, `vaporwave-synthwave`). |
| Reference | `REFERENCES.md`, `GLOSSARY.md` | Curated GitHub/web links (copy-paste ready), terminology. |

## File format (every knowledge file)

```
# Title
> **Read when:** ... | **Section:** ... | **Related:** ...

## Rules
### R1. Rule name
**Rule:** imperative instruction.
**Why:** short mechanistic explanation (so the rule can be extrapolated).
**Example:** minimal concrete illustration.

## Checklist      <- concrete, pass/fail, measurable
## Anti-patterns  <- what to avoid
## Sources        <- systems the rules were adapted from
```

## Quick-start by task type

| Task | Load |
|---|---|
| Build a new screen / feature UI | fundamentals: `visual-hierarchy`, `spacing-layout-grids`, `color`, `typography` + relevant `20-components/` + relevant `30-ux-patterns/` + `40-quality/visual-qa-protocol.md` |
| Build a UI kit from scratch | all `00-fundamentals/` + all `10-design-system/` |
| Pick / integrate a component library | `10-design-system/using-existing-libraries.md` + `20-components/components-catalog.md` |
| Review or audit existing UI | `40-quality/` (all five) + `30-ux-patterns/empty-loading-error-states.md` |
| Theme / dark mode work | `10-design-system/design-tokens.md` + `theming-dark-mode.md` |
| Touch-first / mobile product | `30-ux-patterns/touch-and-mobile.md` + `40-quality/accessibility-wcag.md` + `40-quality/visual-qa-protocol.md` |
| Localized / RTL product | `00-fundamentals/internationalization-rtl.md` + `00-fundamentals/typography.md` + `00-fundamentals/spacing-layout-grids.md` |
| Charts & data-heavy views | `30-ux-patterns/data-visualization.md` + `30-ux-patterns/dashboards-data-viz-ux.md` + `00-fundamentals/color.md` |
| Rust native / WASM target | your task files + relevant `50-platforms/` file + `40-quality/visual-qa-protocol.md` |
| Create or apply a distinct visual style | `60-styles/style-catalog.md` + `60-styles/style-design-process.md` + the chosen `60-styles/<style>.md` + `10-design-system/design-tokens.md` + `40-quality/visual-qa-protocol.md` |

The full matrix with mandatory vs on-demand files lives in `AGENT-WORKFLOW.md`.

## For designers & developers

The library was built for agents, but it doubles as a compact design handbook:

- **Design review.** Run the `## Checklist` sections of `40-quality/` against an existing screen — they are written as pass/fail criteria, so they work as a review script (and as onboarding for new reviewers).
- **Fast onboarding.** Reading `00-fundamentals/` in order is a distilled course on visual craft: hierarchy, color systems, type, grids, depth, motion — each rule with the *why*, so you can extrapolate rather than memorize.
- **Style exploration.** Every `60-styles/` preset is a token-ready spec: palette ramps, type stack, shape language, motion, and accessibility watchpoints. Use them as moodboard-plus-contract — and see the [live demo](https://danku13.github.io/ui-for-agents/) for one DOM wearing five of them.
- **Pairing with agents.** Point your coding agent at this repo and review its S7 report (task type, files loaded, checklist results) — it makes design QA auditable in PRs.

## Extending the library

Add rules as new numbered entries (`R<n>`) inside the matching file; keep the `Rule → Why → Example` shape; keep files under ~170 lines (split into two files when a file outgrows this); update `Related:` links in both directions. Platform-specific exceptions belong in `50-platforms/`, never inside fundamentals — fundamentals must stay true for every renderer. If you add a new file, wire it into the map tables above and into the `AGENT-WORKFLOW.md` matrix.
