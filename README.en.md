<div align="center">

<img src="assets/github-hero.svg" alt="CanvasDesk — models that compute themselves" width="100%">

### An infinite canvas where services, products and unit economics are **computed, not just drawn**

Connect nodes with edges — values flow through the graph and recalculate live.
Change one parameter — see what happens to P&L, runway or service capacity.

[![Release](https://img.shields.io/github/v/release/danku13/CanvasDesk?include_prereleases&color=2ea44f)](https://github.com/danku13/CanvasDesk/releases/latest)
[![CI](https://github.com/danku13/CanvasDesk/actions/workflows/ci.yml/badge.svg)](https://github.com/danku13/CanvasDesk/actions/workflows/ci.yml)
[![Multi-platform Build](https://github.com/danku13/CanvasDesk/actions/workflows/build-all.yml/badge.svg)](https://github.com/danku13/CanvasDesk/actions/workflows/build-all.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
[![CLA](https://img.shields.io/badge/CLA-required-orange.svg)](CLA.md)
[![PRs welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

[**🚀 Web demo**](https://danku13.github.io/CanvasDesk/app/) ·
[**⬇️ Download release**](https://github.com/danku13/CanvasDesk/releases/latest) ·
[**📚 Docs**](#-documentation) ·
[**🗂 Example models**](#-example-models) ·
[**❓ FAQ**](#-faq) ·
[**🤝 Contributing**](#-contributing)

**English** · [Русский](README.ru.md)

</div>

---

## ✨ What is it

CanvasDesk is a visual environment for mathematical modeling. On an infinite zoomable canvas you build **executable models** from computational nodes: plain-text notes with human-readable formulas and ready-made templates (load balancer, cache, queue, CAC, LTV, runway, retention…).

Edges between nodes carry not just "an arrow" but a **value**. Set `rps = 1000` in one node — latency, utilization and replica count recalculate in every connected node. Shift churn by one percentage point — see the delta in LTV, runway and burn rate in the scenario comparison table. The graph is a directed acyclic graph (DAG), cycles are blocked, live recalculation is instant.

Domain math is built into the core: units convert automatically (`50 ms`, `1000 rps`, `$10k/mo`), with queueing-theory formulas (`mm1`, `mmc`, `erlang_c`, Little's law) and finance functions (`npv`, `irr`, `cagr`, `cohort_ltv`). No writing formulas in Excel and keeping them in sync with a diagram — the diagram **is** the formula.

An AI agent via [MCP](#-ai-agents-and-mcp) assembles models from a text description and sanity-checks the numbers: "design a 10k rps service with cache and a queue" — the agent lays out the graph, fills in reference values, highlights bottlenecks.

CanvasDesk is for those who must simultaneously **design a system**, **compute it** and **defend the math** in front of a team, C-level or investors — in one place, without juggling Miro + Excel + Google Docs + PowerPoint.

<p align="center">
  <img src="docs/images/showcase-capacity-service.png" alt="The capacity-service model on the CanvasDesk canvas: values flow along edges, the bottleneck (ρ ≥ 1) is highlighted" width="88%">
  <br><sub><b>The capacity-service model</b> — values flow along graph edges, the bottleneck (ρ ≥ 1) is highlighted. Full gallery — <a href="#-example-models">below</a>.</sub>
</p>

## 👥 Who it's for

| Role | Typical questions CanvasDesk answers |
|---|---|
| **Solution / Technical architect** | Will the service hold 10k rps? How many replicas and workers? Where's the bottleneck at ×2? What happens to P99 with a cache? |
| **Product analyst / finance** | What LTV at this CAC? What happens to runway if churn drops by 1 pp? When does ARPU cover acquisition? |
| **Product owner / manager** | What if load doubles? Where does P&L break? Which scenario fits an 18-month path to profitability? |
| **AI assistant in the loop with a human** | Draft a model from a description in minutes, then let a human explore and refine: the agent builds the graph, wires edges, checks correctness. |

## 🚀 Quick start

**1. Web version — no install** → [danku13.github.io/CanvasDesk/app/](https://danku13.github.io/CanvasDesk/app/)
The same engine in the browser: Chrome or Edge (WebGPU) required, Firefox experimental. Loads in ~1 second.

**2. Ready-made binary** → [Releases](https://github.com/danku13/CanvasDesk/releases/latest)
Windows x64, Linux, macOS universal2 (`.app`) — no compiling needed.

**3. Build from source** (Rust 1.80+ required):

```bash
git clone https://github.com/danku13/CanvasDesk.git
cd CanvasDesk
cargo build --workspace --release
cargo run -p canvas-app --release -- path/to/file.canvas
cargo run -p canvas-app --release -- --stress 5000    # stress-test scene
canvasdesk mcp                                        # MCP bridge for AI clients
```

Platform specifics — see [docs/ENVIRONMENT.md](docs/ENVIRONMENT.md).

**4. AI agent** — add `canvasdesk mcp` to your AI client config: the agent assembles models, checks numbers, hunts for bottlenecks.

## 💡 What you can do right now

- **Open a reference model** — the [example gallery](#-example-models) in the repo and in the app: unit economics, capacity service, project budget, what-if. Twist the parameters, watch the downstream recalculate.
- **Use 45+ built-in templates** — palette on the left or the ring menu: infrastructure, unit economics, product analytics.
- **Run what-if** — swap one calculation line and see deltas across the whole graph; compare up to 3 scenarios in a table.
- **Collaborate in a single file** — a model is a `.canvas` file: commit it to git, review it as a diff, open it anywhere.

## 🧠 How it works

- **Formulas as text.** In any note, write `rps = 1000`, `latency = 50 ms`, `cost = rps * $0.01`. Units convert, variables flow top-down, like Numi.
- **Edges carry values.** Wire a source to a sink — the output value lands in the input parameter. Change the source — the whole downstream recalculates.
- **Ready-made templates.** No need to build a load balancer, cache, queue or LTV formula from scratch — grab one from the palette and plug in your numbers.
- **Domain math in the core.** Erlang-C for contact centers, queueing theory for services, NPV/IRR/CAGR for finance — built in, no external libraries.
- **Live recalculation and undo.** Everything recalculates instantly, any edit rolls back in one step. What-if scenarios never touch the base model — compare, pick the best, apply.

## 🎯 Use cases

**"Will the service hold 10k rps?"** — assemble a graph from templates: load balancer → API gateway → queue → workers → DB. Set the rps, watch per-node utilization, the bottleneck indicator (ρ ≥ 1) lights up red. Add a cache — recalculation, see where the load eased.

**"What happens to runway if churn drops by 1 pp?"** — open the unit-economics reference model, swap churn in a what-if, see the delta in LTV, MRR and runway in the comparison table. Export the scenario for investors.

**"Where does P&L break at ×2 load?"** — duplicate the graph, double the rps, see which cost lines balloon (infrastructure, support) and where margin goes negative. Defend the decision in front of C-level with a before → after table.

**"Design an Instagram MVP"** — tell the agent, it lays out the graph: users → retention → DAU → storage/CDN load → cost. Sanity-check the numbers, fine-tune, walk into the investor meeting with a model instead of slides.

## 🗂 Example models

15 reference models in [`assets/canvas-schemes/`](assets/canvas-schemes/) — open them right in the app (gallery on start or File → Open):

| Category | Models |
|---|---|
| **Infrastructure** | [capacity-service](assets/canvas-schemes/com.canvasdesk.scheme.capacity-service/scheme.json) · [support-staffing](assets/canvas-schemes/com.canvasdesk.scheme.support-staffing/scheme.json) · [renovation-estimate](assets/canvas-schemes/com.canvasdesk.scheme.renovation-estimate/scheme.json) |
| **Unit economics** | [unit-economics](assets/canvas-schemes/com.canvasdesk.scheme.unit-economics/scheme.json) · [runway](assets/canvas-schemes/com.canvasdesk.scheme.runway/scheme.json) · [investment-case](assets/canvas-schemes/com.canvasdesk.scheme.investment-case/scheme.json) · [project-budget](assets/canvas-schemes/com.canvasdesk.scheme.project-budget/scheme.json) |
| **Product & growth** | [ab-testing](assets/canvas-schemes/com.canvasdesk.scheme.ab-testing/scheme.json) · [cohort-launch](assets/canvas-schemes/com.canvasdesk.scheme.cohort-launch/scheme.json) · [cjm-saas](assets/canvas-schemes/com.canvasdesk.scheme.cjm-saas/scheme.json) · [jtbd-saas](assets/canvas-schemes/com.canvasdesk.scheme.jtbd-saas/scheme.json) · [service-blueprint-saas](assets/canvas-schemes/com.canvasdesk.scheme.service-blueprint-saas/scheme.json) |
| **Learning** | [intro-calculations](assets/canvas-schemes/com.canvasdesk.scheme.intro-calculations/scheme.json) · [intro-whatif](assets/canvas-schemes/com.canvasdesk.scheme.intro-whatif/scheme.json) |

## 📚 Documentation

| Document | Contents |
|---|---|
| [docs/SPEC.md](docs/SPEC.md) | Engine and architecture technical specification |
| [user-docs/](user-docs/) | User documentation |
| [docs/RECIPES.md](docs/RECIPES.md) | Recipes: how to build typical models |
| [docs/ENVIRONMENT.md](docs/ENVIRONMENT.md) | Building and platform specifics |
| [docs/BYOK.md](docs/BYOK.md) | Bring your own AI keys (BYOK) |
| [docs/plans/product-roadmap.md](docs/plans/product-roadmap.md) | Product roadmap |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guidelines |
| [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) | Third-party licenses |

## 🤖 AI agents and MCP

CanvasDesk ships with an MCP bridge (`canvasdesk mcp`): any AI client speaking the [Model Context Protocol](https://modelcontextprotocol.io) can build and verify models right on the canvas. The agent decomposes a task into a graph, wires edges, fills in reference values and flags risks; the human verifies and refines. The model-verification protocol runs as wasm checks on the core side ([docs/SPEC.md](docs/SPEC.md)).

## 🧱 Tech stack

Rust · wgpu (WebGPU) · winit · cosmic-text · rstar (spatial index) · rusqlite · serde · MCP protocol. The UI stack is proprietary: surface layers, a component kit and the FlexLayoutEngine layout engine (a CSS Flexbox/Grid subset: auto/minmax tracks, sticky, rotate, z-index) with no external layout dependencies. One binary — the whole stack: GUI + MCP bridge for AI clients.

**Canvas features:** infinite canvas (panning, cursor zoom, pinch, grid) · Obsidian-compatible markdown notes (`**bold**`, `*italic*`, `==highlight==`) · file cards with system thumbnails · minimap and full-text search · undo/redo 50 deep · node grouping and color palette · 7 themes (tokyo-night, dracula, nord, gruvbox, catppuccin, solarized…) · "desktop wallpaper" mode on Windows · autosave with debounce and a `.bak` copy · **5000 nodes at 60 fps**.

## 🗺 Status

Canvas, renderer, notes, edges, minimap, search, undo, MCP, the Numi engine, value flow, 45 templates, what-if, AI agent, web version — **working and actively used**.

| Platform | State |
|---|---|
| Windows 10/11 x64 | full functionality |
| Linux, macOS | canvas and editing; platform features (drag-drop, MCP over Unix sockets) — planned |
| Web | Chrome/Edge (WebGPU), Firefox experimental |

The project is in active development, currently at the demand-validation stage: **looking for the first 5 external users** who build a model themselves and come back to it a second time. Try the [web version](https://danku13.github.io/CanvasDesk/app/) and tell us how it went — [@danku13](https://t.me/danku13).

Details — in the [specification](docs/SPEC.md), [user documentation](user-docs/) and the [roadmap](docs/plans/product-roadmap.md).

## 🤝 Contributing

PRs, bug reports and model ideas are welcome. Read [CONTRIBUTING.md](CONTRIBUTING.md) before your first PR. By submitting a Pull Request you accept the [CLA](CLA.md) — authorship stays yours, the right to relicense the contribution in the future goes to the project owner.

## 📜 License and ground rules

**Copyright © 2026 danku13.** The project is distributed under **[GNU AGPLv3](LICENSE)**. The model is simple: the code is free and open — forever; the project's fate (including its license) stays in the owner's hands.

| | |
|---|---|
| 💚 **The code is free** | All code is under [GNU AGPLv3](LICENSE): use, study, modify and redistribute freely. No "open core" tricks — the community gets the entire project. |
| 🔒 **Commercial rights with the owner** | Exclusive rights to the project and **the right to change its license in the future** remain with the owner (**danku13**): a commercial or proprietary edition is possible, as is dual-licensing. Code already released under AGPLv3 will not be closed off from the community. |
| ✍️ **PR = signing the CLA** | By submitting a Pull Request you **automatically accept the [CLA](CLA.md)**: authorship of your code stays with you, but the right to license the contribution in the future goes to the owner. This shields the project from legal risks if the license changes. |

## ❓ FAQ

**Can I use CanvasDesk commercially?**

Yes — under AGPLv3 terms: commercial use is legal as long as you comply with the license (your modifications stay under AGPLv3; for network services §13 applies — the sources of the version available to users must be opened). No owner permission required.

**AGPLv3 terms don't suit me (closed integration, OEM, white-label, SaaS without opening sources)**

Write to the owner — we'll discuss a **commercial license** for your scenario: Telegram [@danku13](https://t.me/danku13) or [danku13@yandex.ru](mailto:danku13@yandex.ru).

**Which operating systems are supported?**

Windows 10/11 x64 — full functionality. Linux and macOS — canvas, editing, search; platform integrations are in progress. Web version — Chrome/Edge (WebGPU), Firefox experimental.

**Is there a web version?**

Yes — [danku13.github.io/CanvasDesk/app/](https://danku13.github.io/CanvasDesk/app/). The same engine as the desktop app runs in the browser: text input, Cyrillic, search, browser storage. Loads in about a second.

**What happens to my contribution?**

Authorship is yours (git history preserves it), but under the [CLA](CLA.md) the owner gains the right to license the contribution on any terms in the future. Participation rules — [CONTRIBUTING.md](CONTRIBUTING.md).

**Is Russian supported?**

The UI is Russian and English (switchable in settings). Formulas and units are universal.

**Investment and partnership?**

Reach out any way you like: Telegram [@danku13](https://t.me/danku13), [danku13@yandex.ru](mailto:danku13@yandex.ru).
