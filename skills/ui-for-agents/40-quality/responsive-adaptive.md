# Responsive & Adaptive Layout

> **Read when:** any UI must survive variable window sizes, screen sizes, zoom levels, or input methods. **Section:** 40-quality. **Related:** 00-fundamentals/spacing-layout-grids.md, 00-fundamentals/typography.md, accessibility-wcag.md, visual-qa-protocol.md, 30-ux-patterns/touch-and-mobile.md, 50-platforms/rust-native-gui.md, 50-platforms/web-css-dom.md

**Core idea:** responsive = one layout reflowing fluidly; adaptive = deliberate layout switches per environment (window class, input type, screen). Real products need both: fluid within a class, adaptive between classes. Every claim in this file is verified at the extremes, not at the demo size.

## Rules

### R1. Declare a strategy per region: responsive or adaptive
**Rule:** For each layout region, decide whether it reflows continuously (fluid) or switches between discrete arrangements at defined thresholds (adaptive) — and write the decision down.
**Why:** Undeclared strategy is how broken middles happen: a sidebar that sometimes wraps, sometimes overlaps, and sometimes vanishes means nobody decided its behavior.
**Example:** Mail client: list + preview are fluid between 900–1400 px; below 900 px the preview becomes a separate adaptive route; above 1400 px a third column appears.

### R2. Breakpoints are labeled ranges, not devices
**Rule:** Define breakpoints as named pixel ranges ("compact ≤ 599, medium 600–1023, expanded ≥ 1024") — never as device names or model lists.
**Why:** Devices vary and multiply; a named range survives hardware you have never seen, and rules stay meaningful ("compact ⇒ single column") instead of chasing device tables forever.
**Example:** "iPhone SE ⇒ stack columns" breaks on the next phone; "compact ⇒ stack columns" covers every current and future device under 600 px.

### R3. Reflow content first: measure and targets hold at every size
**Rule:** Body text keeps a 45–75 character measure and all interactive targets stay ≥ 24×24 px at every width the product supports.
**Why:** Legibility is the last thing to sacrifice: full-width lines lose reading position, and targets squeezed below the floor trade usability for pixels — at exactly the small sizes where mis-hits hurt most.
**Example:** At 360 px, an article view constrains text to a readable measure and collapses the surrounding chrome — it does not stretch paragraphs to 900 px on a wide monitor.

### R4. Never hide functionality behind hover
**Rule:** Anything reachable by hover must also be reachable without it: visible by default, via tap, via keyboard focus, or via an explicit reveal control.
**Why:** Touch has no hover and keyboards barely do; hover-only affordances are invisible to entire input classes, and their content is undiscoverable even by mouse users until they hover by luck.
**Example:** Row actions shown on hover in a desktop table become a visible overflow ("⋯") menu on touch — same actions, different trigger.

### R5. Design the minimum supported size explicitly
**Rule:** Pick a minimum supported window size per platform and define what happens below it and at min-content: which region collapses, which scrolls, which is clamped. Desktop windows are resizable — assume users will drag to the minimum.
**Why:** Without a declared minimum the failure mode is silent overlap and clipped actions at odd sizes; explicit collapse/scroll rules make every width between minimum and maximum a designed state.
**Example:** "Minimum 720×480: below this the editor shows a horizontal scrollbar rather than compressing; the sidebar auto-collapses to icons under 900 px."

### R6. Test the extremes, not the middle
**Rule:** Verify each screen at: narrowest supported width (~360 px), widest supported, 200% zoom / large-scale text, both orientations where orientation exists, and the minimum window size.
**Why:** Defects live at boundaries — middle sizes inherit working behavior from both neighbors, extremes expose overflow, wrapping, and clipped actions; this is where QA time pays for itself.
**Example:** A settings page perfect at 1280 px breaks at 360 px (tab labels overlap) and at 200% (save button pushed off-screen) — found only because the extremes were on the test list.

### R7. Let input type change layout logic, not just size
**Rule:** When the primary pointer is touch, raise targets to 44×44 px, increase spacing between adjacent actions, and replace hover reveals with explicit controls; with a fine pointer (mouse/pen), density may rise and 24×24 px is the floor.
**Why:** A finger is an order of magnitude less precise than a cursor — spacing is the compensation; shrinking a desktop UI onto a phone changes the input channel, not just the viewport, and ignores that.
**Example:** Same list component: 8 px row gaps and hover actions with a mouse; 12 px gaps, 44 px rows, and a swipe/overflow action set on touch.

### R8. Native targets: DPI scaling and multi-window are layout inputs
**Rule:** On desktop-native apps, support OS display scaling (100–200%+) and multiple simultaneous windows of different sizes; never assume one fixed surface. (See 50-platforms/rust-native-gui.md.)
**Why:** Native apps get no "page width" handed to them — each window is its own responsive surface, and a user at 150% DPI on a 1080p panel sees far less space than the demo machine did.
**Example:** An egui/iced app lays out from the actual window size every frame; text and spacing scale with the DPI factor; a second, narrower window still renders every control.

### R9. Layout shift during load is a defect
**Rule:** Reserve space for late-arriving content (images, async lists, web fonts) so nothing jumps after first paint.
**Why:** Shift moves the target the user is about to hit — taps land on the wrong control and reading position is lost; the movement reads as brokenness even when every final frame is correct.
**Example:** An image card declares its aspect-ratio box before the image loads; a table fixes row height so inserted data does not push the toolbar down.

### R10. Prefer fluid flexibility before adding switches
**Rule:** Before adding a breakpoint, ask whether flexible sizing can absorb the change; add a switch only when relationships break — a row would wrap illegibly, a measure would exceed 75ch.
**Why:** Every breakpoint multiplies the states that must be tested; fluid layouts degrade gracefully between thresholds, while switch-heavy layouts break at the seams and need testing at every threshold ±1 px.
**Example:** A three-card row becomes a wrapping flex row with min-widths before it becomes a "1 / 2 / 3 columns" ladder — it adapts continuously with zero extra states.

### R11. One content source, many presentations
**Rule:** Do not fork content or behavior per breakpoint; re-present, reorder, or conditionally reveal the same widget tree — never duplicate it.
**Why:** Duplicated markup/widgets drift: one copy gets the fix, the other ships the bug, and every content update must be made twice forever.
**Example:** Navigation that is a sidebar on desktop and a drawer on compact screens renders from one nav model — not two hand-maintained variants.

### R12. Design both orientations when orientation exists
**Rule:** Portrait and landscape each get a deliberate arrangement; never let one orientation inherit a scaled result of the other.
**Why:** Width and height swap roles — a list fine at 800×1280 becomes cramped at 1280×800, where the reclaimed vertical space should buy content, not padding.
**Example:** A tablet in portrait shows a single-pane list with drill-down; in landscape the same view becomes two-pane master–detail.

## Minimum test matrix

| Surface | Sizes to verify |
|---|---|
| Web / WASM | ~360 px, 600 px, 1024 px, widest supported, 200% zoom, both orientations |
| Native window | Min supported size, default size, maximized, 150% DPI, two windows side by side |
| Touch device | Compact width, 44×44 targets, portrait + landscape |

## Checklist
- [ ] Every region has a declared strategy: fluid or adaptive, with named thresholds
- [ ] Breakpoints are labeled px ranges; no device names anywhere
- [ ] 45–75ch measure and ≥ 24 px targets hold at every supported width
- [ ] No hover-only affordance exists; a non-hover path covers every action
- [ ] Minimum supported size declared; collapse/scroll behavior defined and tested
- [ ] Extremes tested: ~360 px, widest, 200% zoom, both orientations, min window
- [ ] Touch surfaces use 44×44 targets and increased action spacing
- [ ] Native targets: DPI scaling and multi-window verified
- [ ] No layout shift after first paint (space reserved for async content)
- [ ] No content or widget tree duplicated per breakpoint

## Anti-patterns
- A breakpoint named after a device ("iPad layout")
- Desktop UI shrunk onto a phone: 16 px targets, hover menus, dense tables
- Testing only 1280 px and calling the result responsive
- Text reflowing to 120-character full-bleed lines on wide monitors
- A sidebar overlapping content below a width that was never tested
- A separate "mobile version" codebase drifting from desktop behavior
- Zoom to 200% reveals the save button off-screen or clipped
- Layout jump every time an image or font finishes loading

## Sources
- Material Design 3 — Adaptive design & window classes: https://m3.material.io/foundations/adaptive-design-lifecycle
- web.dev — Responsive web design basics: https://web.dev/articles/responsive-web-design-basics
- web.dev — Cumulative Layout Shift (CLS): https://web.dev/articles/cls
- Nielsen Norman Group — Adaptive vs. responsive design: https://www.nngroup.com/articles/responsive-vs-adaptive-design/
- Apple HIG — Layout: https://developer.apple.com/design/human-interface-guidelines/layout
