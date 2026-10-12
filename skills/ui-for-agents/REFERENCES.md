# References — Curated Links

> **Read when:** you need source material, a library, or a tool. **Section:** root reference. **Related:** README.md, GLOSSARY.md

Copy-paste ready list. Each entry is annotated with what it is good for so an agent (or a human with a text-expander) can grab it without browsing. Prefer loading knowledge from the section files first; use these links for depth, tooling, or when writing new rules.

## Design systems & guidelines (web)

- Material Design 3 — https://m3.material.io — most complete open system: color roles, elevation, motion durations, component guidance.
- Apple Human Interface Guidelines — https://developer.apple.com/design/human-interface-guidelines — platform conventions, touch targets, typography feel.
- Fluent 2 — https://fluent2.microsoft.design — desktop-first system, dense enterprise UI patterns.
- Atlassian Design System — https://atlassian.design — content/microcopy guidelines are among the best published.
- Shopify Polaris — https://polaris.shopify.com — admin/data-heavy UI, excellent usage do/don't docs.
- IBM Carbon — https://carbondesignsystem.com — grids, tokens, data visualization guidance.
- GitHub Primer — https://primer.style — real-world web app system, accessibility notes.
- U.S. Web Design System — https://designsystem.digital.gov — strict 508/WCAG compliance examples.

## Accessibility (norms & tools)

- WCAG 2.2 — https://www.w3.org/TR/WCAG22/ — the norm itself; cite criteria by number (e.g. 1.4.3).
- WAI-ARIA Authoring Practices — https://www.w3.org/WAI/ARIA/apg/ — keyboard interaction & ARIA patterns per component.
- WebAIM contrast checker — https://webaim.org/resources/contrastchecker/ — fast ratio verification.
- axe-core — https://github.com/dequelabs/axe-core — automated a11y rule engine (embeddable).
- pa11y — https://github.com/pa11y/pa11y — CLI a11y scans in CI.
- eslint-plugin-jsx-a11y — https://github.com/jsx-eslint/eslint-plugin-jsx-a11y — static a11y linting for JSX projects.
- NVDA screen reader — https://github.com/nvaccess/nvda — free manual testing on Windows.

## GitHub — component libraries & UI kits (web)

- shadcn/ui — https://github.com/shadcn-ui/ui — copy-in components on Radix + Tailwind; the "own the code" model.
- Radix Primitives — https://github.com/radix-ui/primitives — headless, a11y-complete primitives.
- Adobe React Spectrum / React Aria — https://github.com/adobe/react-spectrum — headless hooks with strong a11y and i18n.
- Headless UI — https://github.com/tailwindlabs/headlessui — small headless set for Tailwind projects.
- MUI — https://github.com/mui/material-ui — full Material kit, fast to ship.
- Ant Design — https://github.com/ant-design/ant-design — enterprise tables/forms/dashboard breadth.
- Chakra UI — https://github.com/chakra-ui/chakra-ui — ergonomic styled props system.
- Mantine — https://github.com/mantinedev/mantine — broad kit + hooks, good dark mode.
- Tailwind CSS — https://github.com/tailwindlabs/tailwindcss — utility styling; maps well to token thinking.
- Open Props — https://github.com/argyleink/open-props — CSS custom properties as a token starter.
- modern-normalize — https://github.com/sindresorhus/modern-normalize — minimal CSS baseline.

## Icons & fonts

- Lucide — https://github.com/lucide-icons/lucide — consistent stroke icons, many platform packages.
- Tabler Icons — https://github.com/tabler/tabler-icons — large stroke-consistent set.
- Phosphor — https://github.com/phosphor-icons/core — six weights per glyph, flexible.
- Heroicons — https://github.com/tailwindlabs/heroicons — clean outline/solid pair.
- Feather — https://github.com/feathericons/feather — tiny classic set.
- Material Symbols — https://github.com/googlefonts/material-design-icons — variable font icons.
- Inter — https://github.com/rsms/inter — default UI sans; excellent numerals and legibility.
- JetBrains Mono — https://github.com/JetBrains/JetBrainsMono — code/mono contexts.
- Google Fonts repo — https://github.com/google/fonts — open font hosting source.
- typescale tool — https://typescale.com — quick modular scale experiments.

## Motion

- Motion (Framer Motion successor) — https://github.com/motiondivision/motion — spring/transition primitives for web.
- GSAP — https://github.com/greensock/GSAP — timeline-based animation engine.
- Lottie-web — https://github.com/airbnb/lottie-web — designer-authored vector animations.
- React Spring — https://github.com/react-spring/react-spring — physics-based motion.

## Design tokens

- Style Dictionary — https://github.com/amzn/style-dictionary — token source → CSS/iOS/Android/any platform output.
- Tokens Studio — https://github.com/tokens-studio/figma-plugin — Figma-side token management that syncs to code.

## Internationalization & RTL

- W3C i18n — RTL/bidirectional guidance — https://www.w3.org/International/questions/qa-rtl — the canonical mirror rules and bidi pitfalls.
- MDN — CSS logical properties — https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_logical_properties_and_values — start/end instead of left/right; the RTL implementation tool.
- Unicode UAX #9 — Bidi Algorithm — https://unicode.org/reports/tr9/ — how mixed-direction runs resolve; basis for isolation decisions.
- ICU / MDN Intl — locale-aware formatting — https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/Intl — dates, numbers, currency, plurals per locale.

## Data visualization

- Cleveland & McGill — Graphical Perception — https://www.stat.berkeley.edu/~stark/Teach/s244/lec-notes/Cleveland-McGill.pdf — the encoding-accuracy ranking behind R1 of data-visualization.md.
- ColorBrewer 2 — https://colorbrewer2.org — colorblind-safe sequential/diverging/qualitative scales with export values.
- Viridis colormaps — https://bids.github.io/colormap/ — perceptually uniform ramps for magnitude data.
- Datawrapper Blog — https://blog.datawrapper.de/ — chart-choice, scales, and annotation craft in practice.
- ECharts examples — https://echarts.apache.org/examples/ — implementation reference for interaction patterns (brush, dataZoom).

## Touch & mobile

- NN/g — Magic numbers & thumb zone — https://www.nngroup.com/articles/magic-numbers-mobile/ — target sizes and one-handed reach research.
- Apple HIG — Gestures — https://developer.apple.com/design/human-interface-guidelines/gestures — platform gesture conventions and safe areas.
- Material Design 3 — Components (bottom sheets, FAB) — https://m3.material.io/components — behavior specs for mobile-first patterns.

## Rust & native GUI (first-class targets)

- egui — https://github.com/emilk/egui — immediate-mode; fast iteration; built-in AccessKit support.
- iced — https://github.com/iced-rs/iced — Elm-architecture retained mode; snapshot testing support.
- Slint — https://github.com/slint-ui/slint — declarative .slint language, embedded + desktop targets.
- Xilem — https://github.com/linebender/xilem — Linebender's evolving retained-mode framework.
- Dioxus — https://github.com/DioxusLabs/dioxus — React-like, targets web/desktop/mobile.
- Tauri — https://github.com/tauri-apps/tauri — Rust backend + system webview shell.
- Relm4 — https://github.com/Relm4/Relm4 — GTK4 + Elm pattern for Linux-native apps.
- Vizia — https://github.com/vizia/vizia — retained-mode, CSS-like styling.
- Makepad — https://github.com/makepad/makepad — GPU-first cross-platform UI.
- Ratatui — https://github.com/ratatui/ratatui — terminal UI (TUI) when that is the real target.
- AccessKit — https://github.com/AccessKit/accesskit — the a11y bridge for non-DOM toolkits; use it everywhere.
- wgpu — https://github.com/gfx-rs/wgpu — GPU layer many of these sit on; relevant for custom renderers.

## Testing & visual QA

- Playwright — https://github.com/microsoft/playwright — browser automation, screenshots, a11y snapshots; also the WASM harness.
- Puppeteer — https://github.com/puppeteer/puppeteer — Chrome automation alternative.
- BackstopJS — https://github.com/garris/BackstopJS — visual regression with diff reports.
- reg-suit — https://github.com/reg-viz/reg-suit — CI visual regression matching.
- lost-pixel — https://github.com/lost-pixel/lost-pixel — OSS visual regression for storybooks & pages.
- Lighthouse — https://github.com/GoogleChrome/lighthouse — perf/a11y/best-practice audits (browser targets).

## Style inspiration (60-styles)

- CARI — https://cari.institute — Consumer Aesthetics Research Institute; taxonomy of digital aesthetics (Y2K, Frutiger Aero, Corporate Memphis, vaporwave) with era boundaries.
- Web Design Museum — https://www.webdesignmuseum.org — archived websites by year (1991–2010s); primary material for UI-era presets.
- Brutalist Websites — https://brutalistwebsites.com — living gallery of brutalist web design.
- neobrutalism.dev — https://neobrutalism.dev — the modern neubrutalist kit and its conventions.
- Bauhaus Dessau Foundation — https://www.bauhaus-dessau.de — primary Bauhaus material (movement preset source).
- Tate — art movement glossary — https://www.tate.org.uk/art/art-terms — reliable one-stop definitions for Art Nouveau, Art Deco, Constructivism, De Stijl, Pop Art.
- Wikipedia: List of art movements — https://en.wikipedia.org/wiki/Art_movement — starting map for named directions; follow to each movement's article for palette/type evidence.
- Fonts In Use — https://fontsinuse.com — real typography indexed by industry, era, and style; the type-pairing research tool.
- Style Stage — https://stylestage.dev — one HTML document restyled many ways; proof that a style is a token/skin layer.
- Coolors — https://coolors.co — quick palette exploration when building a token preset.
- Google Fonts specimens — https://fonts.google.com — era display faces (geometric sans for Bauhaus, condensed for constructivism/deco, humanist for Frutiger Aero); pair with Fonts In Use evidence.

## Reading (concept sources)

- Nielsen Norman Group articles — https://www.nngroup.com/articles/ — UX research basis for the 30/40 sections.
- Laws of UX — https://lawsofux.com — Hick, Fitts, Jakob etc. with citations; the "Why" material.
- Practical Typography — https://practicaltypography.com — typography rules that survive translation to any renderer.
- Every Layout — https://every-layout.dev — layout primitives thinking, CSS but conceptually universal.
- refactoring UI (book) — https://www.refactoringui.com — applied visual hierarchy for developers.

## Inspiration galleries

- Mobbin — https://mobbin.com — real app flow screenshots, searchable by pattern.
- Land-book — https://land-book.com — landing page gallery.
- Godly — https://godly.website — high-craft web reference.
