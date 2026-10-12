# Building a UI Kit

> **Read when:** starting or reviewing a UI kit build (BUILD-KIT), or deciding what belongs in the kit. | **Section:** 10-design-system | **Related:** design-tokens.md, component-api-design.md, component-documentation.md, ../40-quality/accessibility-wcag.md, ../20-components/components-catalog.md

**Core idea:** a kit is built bottom-up — inventory, tokens, primitives, composites, patterns — with composition over configuration, the full states matrix implemented once per component, and accessibility built in from the first primitive.

## Rules

### R1. Build in layer order: inventory → tokens → primitives → composites → patterns
**Rule:** Follow the sequence: audit existing screens (UI inventory), then tokens, then primitives (button, input, text), then composites (field, card, menu), then patterns (form flows, empty states). Never start a layer before the one below exists.
**Why:** Each layer consumes the one below — a "form card" built before tokens hardcodes values that must be ripped out later, and a kit built without the inventory optimizes for imaginary screens instead of the product's real ones.
**Example:** The audit finds 6 button styles and 3 input styles across screens; the kit starts with one Button primitive covering the real variants, not a new invention.

### R2. Composition over configuration
**Rule:** Prefer small components the consumer composes (Field = Label + Input + HelpText + Error) over mega-components with props for every arrangement.
**Why:** Prop trees grow combinatorially, and every new layout need becomes a library release; composition moves flexibility to the consumer without widening the API you must maintain forever.
**Example:** `Field(label, input, help, error slots)` instead of `Input(labelPosition, errorPosition, showLabelAbove, helpPlacement…)`.

### R3. Separate headless logic from the styleable skin
**Rule:** Keep behavior (state machine, keyboard, ARIA) separate from presentation; make the behavior layer usable headless so the skin can be replaced without rewriting behavior.
**Why:** Behavior is expensive to build and stable over time; looks change with every rebrand. Coupling them means a re-theme forks behavior, and behavior fixes drag restyles along.
**Example:** A select's open/close, arrow-key, and ARIA logic is consumable with the default skin and survives a full skin swap untouched.

### R4. Minimal prop surface — variants form a closed set
**Rule:** Expose the smallest prop set that covers the inventory; every variant is a named member of a documented, closed set (see component-api-design.md).
**Why:** Every prop is a permanent promise and a test cell you must fill; open-ended props (any color, any node anywhere) create states you cannot enumerate, document, or verify.
**Example:** Button exposes `variant: primary|secondary|ghost|danger` and `size: sm|md|lg` — nothing else; custom colors go through tokens, never a `color` prop.

### R5. Implement the full states matrix once per component
**Rule:** For each component implement and verify the complete state matrix — default / hover / focus / active / disabled / loading, plus empty and error where content is involved — in one deliberate pass, not feature by feature.
**Why:** States added later are added inconsistently (some components get focus rings, some don't) and cost more than one pass; the matrix is the component's real contract with every screen that uses it.
**Example:** Button ships with all six interaction states styled and screenshot-tested on day one; "loading state later" is rejected at review.

### R6. Accessibility built into the first primitive
**Rule:** The first primitives ship with accessible names, roles, keyboard interaction, visible focus, and pointer targets ≥ 24×24 px; a component without these is a draft, not "done".
**Why:** Retrofitting accessibility re-opens every component's structure, behavior, and tests — roughly 10× the cost of building it in; consumers build screens on the kit's a11y baseline, so each gap propagates product-wide (see ../40-quality/accessibility-wcag.md).
**Example:** Tabs ships with arrow-key navigation, roving focus, tab/tablist roles, and a visible focus ring from the token set — in its first release.

### R7. Rule of three before generalizing
**Rule:** Do not promote a component into the kit until a third real use appears; until then it stays local to its feature.
**Why:** Two uses share too little information to reveal the true axis of variation; premature generalization freezes a wrong abstraction that the third use is then forced to break.
**Example:** A one-off pricing card stays in its feature; when marketing and settings also need cards, the shared Card composite is designed from all three use cases.

### R8. Every variant traces to the inventory
**Rule:** Every variant, size, and state must trace to a real usage found in the product inventory (or a committed roadmap need); no speculative variants.
**Why:** Speculative variants are never exercised by real screens, drift out of visual QA, and become dead code with documentation nobody can verify against reality.
**Example:** `size="lg"` is added only after the audit shows large CTAs in use; otherwise two sizes ship and stay.

### R9. Version the kit; changelog + migration notes
**Rule:** Version the kit, keep a changelog, and write a migration note for every breaking change; never break consumer screens silently.
**Why:** Consumers integrate the kit into screens that get their own visual QA; silent changes invalidate their screenshots and evidence, and unreviewable upgrades block adoption of future versions.
**Example:** Changelog: "2.0 — Button: `type` renamed `variant`; migration: rename map + codemod note included."

### R10. Ownership and contribution rules, written down
**Rule:** Document who may edit the kit and how: proposal → review → tokens-only implementation → docs + tests + visual QA → release; product screens may not fork or locally restyle kit components.
**Why:** Kits die in two directions — frozen (nobody may change anything) and flooded (every team commits local hacks); explicit contribution rules keep the kit single-sourced while it grows.
**Example:** Contributing an `InvoiceCard` means: propose composite, map to tokens, states matrix, docs page, screenshots in both themes — then release; not a copy-paste into one screen's folder.

### R11. Release gate: tokens-only styling + documentation page
**Rule:** A component releases only when it styles exclusively through tokens and has a documentation page (anatomy, props, states matrix, do/don't).
**Why:** One hardcoded value re-opens theme drift for the whole kit, and an undocumented component gets re-implemented by the next feature team that cannot discover it.
**Example:** The release checklist includes "grep for raw values = 0" and "docs page exists and matches the implementation".

### R12. Golden-image the states matrix per theme
**Rule:** Snapshot every component's full states matrix in every theme as golden images; kit releases run visual diffs against them.
**Why:** Kit changes propagate to every screen, so regressions must be caught at the kit level before release; per-screen QA finds them too late and too slowly.
**Example:** Button: 4 variants × 3 sizes × 6 states × 2 themes = 144 golden images regenerated and diffed on every kit change.

### R13. The kit is the only source of UI truth
**Rule:** Product screens compose kit components; when a screen needs something the kit lacks, the gap is filed to the kit, not solved by a local copy.
**Why:** A second source of components is where consistency dies — within a quarter two button styles exist and neither is authoritative; the contribution channel (R10) only works if it is the only channel.
**Example:** A screen needs a segmented control; the team files a proposal, uses a documented interim pattern, and the control lands in the kit — not a one-off styled block.

## Checklist
- [ ] Inventory/audit completed before the first primitive
- [ ] Token set exists; components reference only tokens
- [ ] Layer order respected: primitives → composites → patterns
- [ ] Every variant/size maps to a real inventory use
- [ ] Full states matrix implemented once per component (incl. loading/empty/error)
- [ ] A11y in every first release: names, roles, keyboard, visible focus, ≥ 24×24 px targets
- [ ] Composition/slots used where prop trees would grow
- [ ] Changelog + migration notes exist; breaking changes flagged
- [ ] Contribution rules written; no local forks of kit components
- [ ] Golden images cover states matrix × themes and diff on release

## Anti-patterns
- Building the kit from a showcase design instead of the product inventory
- A mega-component with 20 props doing what 3 slots would do
- "Disabled/loading/error states to be added later"
- A component used once promoted into the kit
- One hardcoded hex inside a kit component
- Docs written after release — or never
- Forking the Button locally because "our screen needs a tweak"
- Kit releases without a visual diff

## Sources
- Material Design 3 — Components: https://m3.material.io/components
- IBM Carbon — Component guidelines: https://carbondesignsystem.com/components/overview/
- Atlassian Design System — Component usage and governance: https://atlassian.design/components
- Shopify Polaris — Component docs model: https://polaris.shopify.com/components
- Radix Primitives — headless behavior model: https://github.com/radix-ui/primitives
- Brad Frost — Atomic Design (layers and primitives thinking): https://atomicdesign.bradfrost.com/chapter-2/
- Storybook — component-driven UI workflows: https://storybook.js.org/docs
