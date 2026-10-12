# Component API Design

> **Read when:** creating a new component (NEW-COMPONENT), defining props/slots/events, or reviewing kit APIs. | **Section:** 10-design-system | **Related:** building-a-ui-kit.md, component-documentation.md, ../20-components/components-catalog.md, ../40-quality/accessibility-wcag.md, ../GLOSSARY.md

**Core idea:** the API is the component's contract: controlled and uncontrolled modes, closed variant sets, composition over prop trees, keyboard behavior as contract, and no undocumented props.

## Rules

### R1. Support controlled and uncontrolled value modes
**Rule:** Value-bearing components (input, select, checkbox, tabs) support both uncontrolled use (they own internal state) and controlled use (value + change callback owned by the app).
**Why:** Quick usage wants zero wiring, while form libraries and validation need to own state; forcing one mode either buries the forms library or taxes every simple screen.
**Example:** `Input(default_value="a")` works standalone; `Input(value=s, on_change=fn)` is fully app-driven; both documented and tested.

### R2. Variants are one enum, not boolean explosions
**Rule:** Express variants as a single enumerated prop with named members (`size="md"`, `variant="ghost"`); never booleans like `small` + `large` + `medium` on one component.
**Why:** Booleans create illegal combinations (`small` and `large` both set) that the component must silently arbitrate; an enum makes every legal state enumerable, documentable, and testable.
**Example:** `<Button small large>` is unrepresentable with `size="sm|md|lg"` — tooling rejects the value instead of the component guessing.

### R3. Composition beats prop trees
**Rule:** Prefer slots/children composition (card header/body/actions; select options) over props that mirror internal structure (`header_text`, `header_icon`, `header_align`…).
**Why:** Prop trees must grow a prop for every future part and arrangement — an unbounded API; composition lets consumers insert arbitrary valid structure without a library release.
**Example:** `Card(header, body, footer slots)` replaces `Card(title, subtitle, title_icon, footer_buttons, footer_alignment…)`.

### R4. One naming vocabulary across the kit
**Rule:** Apply the same naming conventions to every component: `on*` for callbacks, `is*`/`has*` for booleans, `render*`/named slots for customization, `default*` for uncontrolled initial values.
**Why:** Conventions are the API's grammar — consumers transfer learning from one component to the whole kit; inconsistent naming makes every new component a fresh negotiation.
**Example:** `on_select`, `is_open`, `has_error`, `render_item`, `default_value` appear with identical meaning in Select, Tabs, and Dialog.

### R5. Sizes map to scale tokens
**Rule:** Size values (`sm|md|lg`) resolve to token-scaled dimensions — spacing insets and type roles — never to raw pixel values passed by consumers.
**Why:** Token mapping keeps every component on the shared rhythm and makes a global density change a token edit; pixel props smuggle hardcoding back in through the API.
**Example:** `size="md"` → `space-inset-md` padding + `text-md` role (body stays 16px-equivalent, line-height 1.4–1.6); a density pass touches tokens only.

### R6. Keyboard and focus behavior is part of the API
**Rule:** Document and test the keyboard map (Tab order, arrow keys, Enter/Space, Escape) and focus behavior (where focus goes on open/close/disable) as API contract — not as implementation detail.
**Why:** Consumers build flows, tests, and a11y guarantees on this behavior; undocumented, it breaks silently on upgrades and cannot be verified at all (see ../40-quality/accessibility-wcag.md).
**Example:** Dialog documents: "Escape closes; focus is trapped while open; focus returns to the opener on close" — and a test asserts all three.

### R7. Provide one documented escape hatch
**Rule:** Provide a documented passthrough (style/class, a named slot for custom content) for uncovered cases, and prefer extending it over adding a prop for every exception.
**Why:** An escape hatch prevents both API bloat and consumer forks; without one, consumers resort to undocumented DOM spelunking that breaks on every upgrade.
**Example:** Button accepts an icon slot and a style passthrough — consumers stop forking it, and fifteen hypothetical props never exist.

### R8. Deprecate: mark, warn in dev, remove only in major
**Rule:** Deprecate by marking in docs and types, emitting a development-time warning that names the replacement, and removing the old API only in a major release.
**Why:** Silent removal breaks consumer builds without warning; a deprecation window lets migration be scheduled and verified, and dev warnings catch stragglers before the removal lands.
**Example:** "1.9: `flat` deprecated, use `variant='ghost'` (dev warning). 2.0: `flat` removed."

### R9. Every prop has a documented, one-sentence effect
**Rule:** A prop may exist only if its effect can be stated in one sentence and demonstrated in docs; props nobody can explain are dead weight and get removed.
**Why:** Undocumented props are either unexercised dead weight or hidden behavior — both force consumers to guess and QA to skip them; removal with a migration note is cheaper than permanent ambiguity.
**Example:** An audit finds `auto_focus_days` with no doc entry and one legacy caller — removed next minor with a note.

### R10. Events express intent, not implementation
**Rule:** Name events and payloads around user intent and value (`on_change(value)`, `on_open`, `on_dismiss(reason)`), never around internal rendering details.
**Why:** Intent-level events survive internal rewrites and port across platforms; implementation-leaking events couple every consumer to internals that will change.
**Example:** `on_dismiss('escape' | 'backdrop' | 'close-button')` lets analytics distinguish cancels from completes without exposing the DOM.

### R11. Use logical directions: start/end, not left/right
**Rule:** Alignment and position props use logical directions (`start`/`end`), not physical ones (`left`/`right`).
**Why:** RTL locales mirror layout; left/right props force per-consumer conditionals and break mirrored layouts, while start/end follows the document direction automatically.
**Example:** A menu with `align="start"` mirrors correctly in Arabic; `align="left"` pins it to the wrong side.

### R12. Props describe state; callbacks describe change
**Rule:** Props express what the component is (`is_open`, `disabled`); changes flow out through callbacks; avoid imperative command methods (`open_now()`, `refresh()`) except as documented, rare handles.
**Why:** State-driven APIs have one source of truth, are renderable and testable, and fit both immediate- and retained-mode GUIs; command-style APIs create a second source of truth that fights the render model.
**Example:** Dialog is controlled by `is_open` + `on_dismiss`; no business code calls `dialog.open()` (a handle may exist as a documented escape hatch).

### R13. Defaults are the most common usage
**Rule:** Every prop's default must match the most frequent real usage from the inventory — a component must be correct with zero props set.
**Why:** Defaults are what consumers get when they skip the docs; making a rare case the default means most usages carry noise and the rare behavior fires by accident.
**Example:** Input defaults to `size="md"`, `type="text"`, no icon; the rare `type="search"` is opt-in, never the baseline.

## Checklist
- [ ] Value components support controlled + uncontrolled; both documented and tested
- [ ] Variants are enums; no boolean variant pairs anywhere
- [ ] Complex components compose via slots; no structural prop trees
- [ ] `on*` / `is*` / `render*` / `default*` conventions applied kit-wide
- [ ] Sizes resolve through tokens; no pixel props in the API
- [ ] Keyboard map + focus behavior documented and asserted by tests
- [ ] One documented escape hatch; no consumer DOM spelunking needed
- [ ] Deprecations warn in dev; removals only in majors
- [ ] Every prop has a one-sentence documented effect
- [ ] Alignment props are start/end (RTL-safe)

## Anti-patterns
- `small`, `medium`, `large` booleans on one component
- 25 props where 3 slots would do
- An undocumented prop "someone added for the dashboard"
- Focus behavior that changed silently in a minor release
- `left`/`right` alignment props that break RTL
- `color="#ff00aa"`-style props bypassing tokens
- Props renamed or removed in a patch release
- A component usable only via ref-level spelunking

## Sources
- WAI-ARIA Authoring Practices — keyboard interaction per component pattern: https://www.w3.org/WAI/ARIA/apg/
- Material Design 3 — component behavior and states specs: https://m3.material.io/components
- IBM Carbon — component guidelines: https://carbondesignsystem.com/components/overview/
- Radix Primitives — controlled/uncontrolled and composition model: https://github.com/radix-ui/primitives
- Adobe React Aria — API naming and a11y conventions: https://github.com/adobe/react-spectrum
- Shopify Polaris — component prop conventions: https://polaris.shopify.com/components
- GitHub Primer — component API guidelines: https://primer.style/components
