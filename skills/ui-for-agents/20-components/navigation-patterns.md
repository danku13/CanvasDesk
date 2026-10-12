# Navigation Patterns

> **Read when:** choosing or building any navigation structure — sidebar, top nav, tabs, breadcrumbs, bottom bar. **Section:** 20-components. **Related:** 20/components-catalog.md, 00/visual-hierarchy.md, 30-ux-patterns/microcopy.md, 40-quality/responsive-adaptive.md, 40-quality/usability-heuristics.md

**Core idea:** navigation answers two questions at all times — "where am I?" and "where can I go?" — and its structure must come from the content's depth and breadth, not from visual preference.

## Structure table

| Structure | Use when | Do not use when |
|---|---|---|
| Sidebar | Deep hierarchy, many destinations (admin panels, app shells) | Fewer than ~6 destinations — it wastes the prime column |
| Top nav | Shallow site/app, ≤ 6 items (marketing, content sites) | Many or nested destinations |
| Tabs | Sibling views of one context | Site-level destinations |
| Breadcrumbs | Depth ≥ 3, path orientation needed | Flat, one-level content |
| Bottom bar (mobile) | The 3–5 top-level destinations | More than 5 — move the rest behind "More" |
| Segmented control | Local view-mode switch (list/grid) | Navigation between destinations |

## Rules

### R1. Choose structure by depth vs breadth; show ≤ 2 levels at once
**Rule:** Map the content's hierarchy first; render at most two levels simultaneously and push deeper levels behind progressive disclosure or breadcrumbs.
**Why:** Each simultaneously visible level multiplies items to parse; beyond two, users stop distinguishing levels and orientation collapses — depth needs disclosure, not more sidebars.
**Example:** Admin app: sidebar sections → page tabs; the third level (per-item detail) is a page, not a nested menu-in-menu.

### R2. Sidebar for deep, top nav for shallow
**Rule:** Many or growing destinations → sidebar; a stable set of ≤ 6 destinations → top nav.
**Why:** A sidebar scales vertically and supports grouping/labels, while top nav scales horizontally until items truncate — the crossover is around six items; beyond it, top nav hides destinations in "More", which is where findability dies.
**Example:** Docs site with 4 sections = top nav; admin console with 30 resources grouped in 5 categories = sidebar.

### R3. Tabs only for sibling views of one context
**Rule:** Tabs switch between alternate views of the same object or context; for movement between destinations use navigation; for a local mode switch use a segmented control.
**Why:** Tabs promise persistence and siblinghood ("same context, another view"); using them for destinations breaks the back/forward model, while using navigation for views loses state the user expects to keep.
**Example:** One project: "Overview / Tasks / Activity" = tabs. "Projects / Team / Billing" = navigation. "List | Board" = segmented control.

### R4. Current location is always visible
**Rule:** The active item is marked in every navigation element, and the page itself states its location (document title, page title, path).
**Why:** "Where am I?" is the first question on every screen — especially after deep-linking, search jumps, or returning via back; navigation without location marking is a map without a "you are here" pin.
**Example:** Sidebar "Invoices" highlighted + bold, page title "Invoices", browser tab "Invoices — Acme".

### R5. Breadcrumbs at depth ≥ 3; clickable path minus current page
**Rule:** Show breadcrumbs when content sits three or more levels deep; every crumb except the current page is a clickable link; the current page renders as plain text at the end.
**Why:** Breadcrumbs compress the vertical path into one scannable line and enable jumps up the tree; linking the current page is a self-referential click that teaches users nothing.
**Example:** "Settings / Workspace / Members" — first two clickable, "Members" plain.

### R6. Keyboard and focus order follow visual order
**Rule:** Navigation is reachable first (or via skip link), Tab order matches the visual/DOM order of items, and active items carry the appropriate selected/current state for assistive tech.
**Why:** Users and tools traverse the interface linearly; a focus order that zigzags against the visual order makes the nav unpredictable, and a missing "current" state hides location from non-visual users.
**Example:** Sidebar read top-to-bottom by Tab; the current section announced as the current item (see 40-quality/accessibility-wcag.md).

### R7. Back never loses entered data
**Rule:** Navigating back from a partially filled flow preserves the entered data (draft or in-place state) or explicitly warns before discarding it.
**Why:** Back is a navigation expectation, not a discard command; silent data loss is the most expensive single navigation defect because it destroys trust in every other flow.
**Example:** "New invoice" → back → "New invoice" shows the draft intact, or a "Discard changes?" dialog intercepts.

### R8. State must be deep-linkable
**Rule:** On the web, view/filters/sort/tab state lives in the URL; in native apps it lives in a routable path — such that refreshing, sharing, or reopening lands on the same view.
**Why:** Deep links make views shareable, bookmarkable, and restorable after crashes; hidden state (all in memory) turns every refresh into "back to square one" and breaks team collaboration.
**Example:** `…/orders?status=open&sort=created:desc` reproduces the exact table view; a native app maps to the same route.

### R9. Icon-only nav items need labels
**Rule:** Every navigation item shows a text label; icons may accompany text, never replace it, in any persistent navigation.
**Why:** Icon-only navigation is mystery-meat: meaning is revealed only by hovering, so memorization never happens and non-pointer users get nothing — orientation dies exactly where it is needed most.
**Example:** Sidebar items "Dashboard, Invoices, Settings" with icons beside labels — icon-only rail only if labels appear on expand/hover plus full labels elsewhere.

### R10. Mobile bottom bar: 3–5 items, never more
**Rule:** The mobile top-level bar carries 3–5 destinations; anything else goes into "More", the sidebar-drawer, or the content itself.
**Why:** Five is the ceiling where thumb-sized targets (44×44 px) still fit without truncation and each item stays memorable; a sixth item shrinks targets and forces label truncation on every item.
**Example:** Bottom bar "Home / Search / Add / Inbox / Profile" — six roles compressed to five by merging.

### R11. Nav labels are nouns and stay stable
**Rule:** Navigation items are destination nouns ("Invoices", "Reports"), stable across sessions; commands and temporary state ("Do X", "New!") do not become nav items.
**Why:** Users navigate by remembering stable landmarks; renamed or volatile labels reset the mental map and make history and training materials wrong overnight.
**Example:** "Reports" stays "Reports" even when a new report type is trending — that is content, not a new landmark.

## Checklist
- [ ] Structure derived from depth/breadth; ≤ 2 levels visible at once
- [ ] Sidebar vs top nav chosen by destination count
- [ ] Tabs only for sibling views; segmented control only for local modes
- [ ] Current location marked in nav + page title + document title
- [ ] Breadcrumbs appear at depth ≥ 3; current page not a link
- [ ] Tab/focus order matches visual order; current state exposed
- [ ] Back preserves entered data or warns before discarding
- [ ] View/filter/sort state deep-links (URL or route)
- [ ] No icon-only items without labels in persistent nav
- [ ] Mobile bottom bar: 3–5 items, ≥ 44×44 px targets

## Anti-patterns
- Nested menus inside sidebars (level 3+ hidden in hover trees)
- Tabs for top-level site destinations
- Hover-only dropdowns as the only path to key sections
- "More" as a dumping ground for a too-long top nav
- Breadcrumbs on a two-level site, adding noise, not orientation
- Back that silently discards a half-filled form
- Filters/sort state that vanish on refresh or share
- Mystery-meat icon rails with labels only on hover

## Sources
- Nielsen Norman Group — Breadcrumbs: https://www.nngroup.com/articles/breadcrumbs/
- Nielsen Norman Group — Tabs, used right: https://www.nngroup.com/articles/tabs-used-right/
- Nielsen Norman Group — Mystery-meat navigation: https://www.nngroup.com/articles/mystery-meat-navigation/
- Apple HIG — Navigation (bars, tabs, sidebars): https://developer.apple.com/design/human-interface-guidelines/navigation-bars
- Material Design 3 — Navigation: https://m3.material.io/foundations/apply-navigation/overview
