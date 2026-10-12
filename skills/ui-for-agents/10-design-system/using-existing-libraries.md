# Using Existing Libraries

> **Read when:** evaluating, adopting, or upgrading a third-party component library (INTEGRATE-LIB). | **Section:** 10-design-system | **Related:** design-tokens.md, building-a-ui-kit.md, ../20-components/components-catalog.md, ../40-quality/accessibility-wcag.md, ../REFERENCES.md

**Core idea:** adopt a library as a dependency, not as identity: screen it for accessibility first, wrap it behind your own component API, bind its variables to your semantic tokens, and always know your exit.

## Rules

### R1. Screen candidates in a fixed order; stop at the first disqualifier
**Rule:** Evaluate in this order and drop the candidate at the first failure: (1) accessibility compliance evidence, (2) themability (can semantic tokens map onto it), (3) headless vs styled trade-off, (4) bundle size and tree-shaking, (5) maintenance activity, (6) license.
**Why:** The order reflects reversal cost — a11y gaps are unfixable from outside without rewriting the component, while size and activity are observable but tolerable; choosing on visuals first is the classic mistake that gets reversed at 10× cost later.
**Example:** A visually perfect library is dropped because its date-picker has no keyboard support and the issue has been open in its tracker for two years.

### R2. Demand hands-on accessibility evidence, not claims
**Rule:** Verify keyboard interaction per component, correct roles and names, visible focus, and contrast of the default skin (4.5:1 / 3:1) yourself; check the project's tracker for open a11y bugs.
**Why:** "Accessible" is marketing until demonstrated; the library's defaults become your baseline for every screen on day one, so its a11y debt is your a11y debt.
**Example:** Before adoption, Tab and arrow keys are walked through the candidate's menu, modal, and date-picker in a live demo; two components trap or lose focus → disqualify or plan explicit remediation.

### R3. Adopt as a dependency, not as identity — wrap it
**Rule:** Wrap the library behind your own component API (adapter layer); screens import your components only, never the library directly.
**Why:** Direct imports weld the library into every screen file; a wrapper confines the dependency to one layer, so it can be replaced — or survive a major upgrade — without touching product code.
**Example:** Screens import `app/ui/Button`; the adapter maps your props to the library's and translates your tokens; replacing libraries touches only `app/ui/*`.

### R4. Bind library variables to your semantic tokens at integration time
**Rule:** At integration, connect the library's style/theming variables to your semantic token values in one central place; screens never consume library-default values.
**Why:** One binding makes the library participate in your theming pipeline (dark mode, rebrands) automatically; unbound, every theme change needs a parallel library configuration that drifts.
**Example:** The library's surface variable is set to `var(--bg-surface)` and its radius variables to `var(--radius-md)` — configured once, not per screen.

### R5. Do not fight the library — the 30% rule
**Rule:** If more than ~30% of usages need overrides to look or behave correctly, choose a different library — or fork deliberately, with a written decision.
**Why:** Override-heavy integration means mismatched design DNA; every library upgrade re-breaks your overrides, so maintenance grows linearly with each screen you ship on top.
**Example:** Restyling every table, modal, and form control via CSS overrides is not integration — it is an unacknowledged fork; fork deliberately or pick another library.

### R6. Headless vs styled is a control-versus-speed decision
**Rule:** Choose headless (behavior + a11y, no skin) when you have tokens and a kit and need design control; choose a full styled kit when speed and borrowed consistency matter more.
**Why:** Headless maximizes token control but you write the skin; a styled kit ships instantly but you inherit its visual system and its theming ceiling — decide explicitly per project phase, not by habit.
**Example:** A brand-heavy product uses headless primitives skinned with its tokens; an internal admin tool under deadline uses a full styled kit, restyled only as far as tokens allow.

### R7. Measure size with your real import set
**Rule:** Measure bundle/size impact using the components you will actually import, including their styles; require tree-shaking or modular imports.
**Why:** Libraries differ by an order of magnitude in per-component cost; a data-grid's real price only appears with your column set and its CSS, and WASM or mobile targets feel every kilobyte.
**Example:** A proof-of-concept imports Button + Select + Modal, builds, and measures: acceptable → adopt; the README's "tree-shakeable" claim is not a measurement.

### R8. Pin, test, then bump
**Rule:** Pin library versions; upgrade in a branch that runs visual diffs and keyboard smoke tests against the new version, then merge and bump on a schedule.
**Why:** Library majors change markup, styles, and behavior underneath your screens — the only safe path is verify-then-adopt; un-upgraded libraries rot into forced big-bang migrations.
**Example:** Upgrade branch: v4→v5, golden-image diff + keyboard smoke tests run; 3 diffs reviewed (2 accepted, 1 filed upstream) before merge.

### R9. One library per component kind
**Rule:** Do not run two libraries of the same kind (two date pickers, two modal systems); if a gap exists, fill it deliberately and document the exception.
**Why:** Same-kind libraries duplicate dependencies and behavior, split keyboard conventions (two modals with different focus behavior), and double the a11y verification surface you must maintain.
**Example:** A second modal system appears for one feature — defect: standardize on the wrapped Modal and extend it, or document why two exist.

### R10. Keep library components under your own QA
**Rule:** After skinning and wrapping, re-run your accessibility and visual checks (keyboard map, focus order, contrast ≥ 4.5:1 / 3:1) — upstream passing is not evidence for your configuration.
**Why:** Theming and wrapping are exactly where library a11y breaks: your token remap can drop a pair below the floor, and your wrapper can drop an attribute or a focus behavior.
**Example:** After re-skinning the select with brand tokens, focus-indicator contrast is re-measured and the keyboard map re-walked; the upstream demo passing is irrelevant.

### R11. Know your exit before you enter
**Rule:** Before adoption, write the replacement path: which components, what surface your adapter exposes, and what headless behavior would be needed; prefer libraries whose behavior is separable from their markup.
**Why:** Every dependency is temporary — abandonment, license change, or stagnation; a known exit keeps the wrapper honest and stops screens from depending on library quirks the adapter never exposed.
**Example:** The adapter documents: "Modal exposes open/on_dismiss/title/body slot; focus trap replaceable" — a future swap implements that surface, not forty quirks.

### R12. Clear the license for your distribution context
**Rule:** Confirm the license (and any CLA/patent/additional terms) permits your use — including commercial, embedded, and WASM distribution — before writing integration code.
**Why:** Licenses are a disqualifier usually discovered too late; some terms force attribution walls or viral obligations that are unacceptable in shipped products, and uninstalling a library from 200 screens is not a plan.
**Example:** MIT/Apache-2.0 → proceed; an AGPL-licensed UI library in a commercial product → legal review before adoption, not after.

### R13. Wrap thin: translate, don't passthrough
**Rule:** The adapter exposes your vocabulary (your variants, your tokens, your events) and translates to the library — it never re-exports the library's full prop surface "for convenience".
**Why:** A passthrough adapter is a fake abstraction: every library prop leaks into screens anyway, so replacement still touches every screen; a thin translated surface keeps the exit real.
**Example:** The library button has 18 props; the adapter exposes 5 that map to your API — extra library capabilities become deliberate adapter extensions, not leaks.

## Checklist
- [ ] Candidate screened in the fixed order; a11y verified hands-on (keyboard, roles, focus)
- [ ] All usage behind adapter components; no direct library imports in screens
- [ ] Library theming variables bound to semantic tokens in one place
- [ ] Override rate < ~30% of usage
- [ ] Bundle impact measured with the real import set, styles included
- [ ] Version pinned; upgrades run diff + smoke tests in a branch first
- [ ] One library per component kind; exceptions documented
- [ ] Skinned result re-verified: contrast 4.5:1 / 3:1 and keyboard map
- [ ] Replacement path (exit) written before adoption
- [ ] License cleared for the actual distribution context

## Anti-patterns
- Choosing a library because the demos look good
- Screens importing the library directly ("just this once", everywhere)
- Theming by overriding library internals with CSS
- Two modal systems in one product
- Unpinned versions and surprise major upgrades
- Skipping a11y verification because the README says "accessible"
- A fork created because one component didn't match
- License review after integration

## Sources
- WAI-ARIA Authoring Practices — the a11y baseline any library must meet: https://www.w3.org/WAI/ARIA/apg/
- Radix Primitives — headless, a11y-complete model: https://github.com/radix-ui/primitives
- Adobe React Aria — headless behavior with a11y and i18n: https://github.com/adobe/react-spectrum
- shadcn/ui — own-the-code adoption model: https://github.com/shadcn-ui/ui
- MUI — styled kit with theming pipeline: https://github.com/mui/material-ui
- IBM Carbon — token-driven theming of a kit: https://carbondesignsystem.com/guidelines/tokens/overview/
- Open Props — CSS variable token baseline: https://github.com/argyleink/open-props
