# Tables and Data Lists

> **Read when:** building any tabular data view, data list, or row-based record browser. **Section:** 20-components. **Related:** 20/components-catalog.md, 30-ux-patterns/search-filtering-sorting.md, 30-ux-patterns/empty-loading-error-states.md, 40-quality/responsive-adaptive.md, 30-ux-patterns/dashboards-data-viz-ux.md

**Core idea:** a table exists to support comparison along columns; every rule below protects either alignment (comparison), headers (orientation), or the row (the unit of meaning).

## Rules

### R1. Numbers right-aligned with identical precision; text left-aligned
**Rule:** Numeric columns right-align and format to one identical precision per column (including trailing zeros); text left-aligns; headers take their column's alignment.
**Why:** Right alignment lines up magnitudes on the decimal point so 9 vs 88 vs 1,000 is comparable at a glance; mixed precision (9.1 / 88.03) silently breaks that comparison, and left-aligned numbers are noise.
**Example:** Amount column renders 1,240.00 / 88.00 / 9.00 — all right-aligned, all two-decimal; header "Amount" right-aligned above them.

### R2. Headers always visible; sticky when scrolling
**Rule:** Column headers are permanently visible; when the table scrolls vertically, the header row is sticky.
**Why:** Without headers, a scrolled table is unnamed numbers — the user must scroll back to decode a column, which stops the moment they do it the second time; sticky headers make long tables self-describing.
**Example:** 200-row invoice table: scrolling keeps "Date / Number / Amount / Status" pinned at the top.

### R3. Choose density deliberately; offer a toggle for mixed audiences
**Rule:** Pick one default row density: comfortable for general products, compact for pro/data-heavy tools; if both audiences exist, expose an explicit density switch.
**Why:** Density is a trade between rows-per-screen and readability/mistake rate — imposing pro density on casual users (or vice versa) optimizes for the wrong eye every day; a switch converts taste into a setting.
**Example:** CRM default comfortable (56 px rows), support power users flip compact (36 px); the switch lives in the table toolbar, not buried in settings.

### R4. Make the row-selection model explicit and consistent
**Rule:** Choose none, single, or multi selection per table; for multi, define select-all semantics: it applies to the currently visible/filtered rows, and the UI states so ("All 25 on this page selected — select all 1,340?").
**Why:** Ambiguous selection causes destructive accidents (bulk actions on "everything" when the user meant the filtered view) — the mismatch between visible and filtered sets is the classic data-loss trap.
**Example:** Filtered to 25 rows: header checkbox selects those 25; a notice offers "Select all 1,340 matching rows" as an explicit second step.

### R5. Truncate with a way out — never clip silently
**Rule:** Long text truncates with an ellipsis and a tooltip (or expandable row/detail view) exposing the full value; numbers are never truncated — they wrap, shrink the column's neighbors, or widen.
**Why:** Silent clipping is data corruption from the user's perspective: a truncated ID, amount, or error message reads complete; numbers truncated to "1,2…" are worse than useless for any decision.
**Example:** "Acme Industries International… " + tooltip with the full name; amounts always fully rendered even if the column must widen.

### R6. Empty cells show an explicit dash
**Rule:** A cell with no value renders a visible "—" (or "N/A"), never an empty cell.
**Why:** A blank cell is ambiguous between "no data" and "rendering bug"; the dash distinguishes intentionally absent values and keeps the row scannable without white gaps that break column tracking.
**Example:** Invoice without a due date shows "—" — a hole in the grid would read as a failed load.

### R7. Sort and filter affordances live on the headers; current sort is marked
**Rule:** Sortable columns show a sort control on the header; the active sort is marked with column + direction; filter controls sit in the toolbar with their active state visible.
**Why:** Sorting without visible state is unreviewable — users cannot tell why the data is ordered the way it is or how to undo it; header-attached controls put the action exactly where the decision is made.
**Example:** "Amount ↓" marked on the header; filter chip "Status: Open" visible above the table with an × (see 30-ux-patterns/search-filtering-sorting.md).

### R8. Paginate when position matters; infinite scroll only for casual browsing
**Rule:** Admin views, search with filters, and anything users return to or reference by position use pagination ("page 3 of 41"); infinite scroll only for feed-like casual browsing without positional needs.
**Why:** Pagination is a coordinate system — it supports "the one on page 3", stable deep links, and returning to the same spot; infinite scroll destroys position and gets slower and disorienting the longer it runs.
**Example:** Orders table paginated at 50 rows with count and jump; a marketing feed scrolls infinitely (see 40-quality/perceived-performance.md for the loading costs).

### R9. Row actions: ≤ 3 inline, the rest under an overflow menu
**Rule:** Show at most three actions per row inline (icons or short text); remaining actions collapse into an overflow "⋯" menu; consistent actions keep the same position across rows.
**Why:** More than three inline actions turns rows into button soup and invites mis-clicks at 24 px+ targets; the overflow keeps the row scannable while preserving full functionality one click away.
**Example:** Rows offer "Open / Edit / ⋯"; inside "⋯": Duplicate, Archive, Delete — same order in every row.

### R10. On narrow screens, tables become cards — plan the fallback
**Rule:** Below the width where 3+ columns survive without horizontal scrolling, restructure rows into cards (label:value pairs); never ship a horizontally scrolling dense table as the mobile answer.
**Why:** Horizontal scroll breaks column comparison (you can never see two compared columns at once) and hides headers; a card restatement preserves each row's meaning in the vertical scan mobile users actually do.
**Example:** 6-column orders table becomes tap-to-expand cards showing "Order 1042 · Open · 1,240.00" with the rest on expansion (see 40-quality/responsive-adaptive.md).

### R11. Aid row scanning with separators or zebra rows — meaning never encoded in row style alone
**Rule:** Dense multi-column tables use row separators or zebra striping plus hover highlighting to hold the scan line; row status colors are always paired with a text or icon marker.
**Why:** In 8+ column tables the eye loses the row between left and right edge; separators give it a rail — but color-only status (red row = failed) fails color-blind users and monochrome exports, violating the color-only rule.
**Example:** Failed rows are red-tinted and carry a "Failed" chip with an icon — never a bare red background (see 00/color.md).

### R12. Keep the table skeleton stable while loading
**Rule:** During loads, render placeholder rows with the same column count and widths as the loaded state; column widths do not jump when data arrives.
**Why:** Widths that change after load move every cell the eye had already anchored — the user re-locates columns per row; a stable skeleton makes loading feel like filling, not rearranging (see 30-ux-patterns/empty-loading-error-states.md, 40-quality/perceived-performance.md).
**Example:** Ten gray placeholder rows shaped like the final rows; the "Amount" column occupies the same 110 px before and after the data lands.

### R13. Every table has a toolbar stating what it shows
**Rule:** Above the table: its name (or item count) and, when rows are selectable, the bulk actions that apply to the current selection; the primary action of the view (e.g. "New order") also lives there, not inside rows.
**Why:** The toolbar orients ("what am I looking at, how many?") and gives bulk operations a stable, discoverable home; row areas must stay scannable, which they stop being when creation and bulk controls hide among rows.
**Example:** "Invoices — 1,340" left, "New invoice" primary right, bulk bar "2 selected — Archive, Export" appearing only while a selection exists.

## Checklist
- [ ] Numbers right-aligned, one precision per column; text left-aligned
- [ ] Headers visible and sticky; column widths stable during load
- [ ] Density chosen deliberately (and switchable for mixed audiences)
- [ ] Selection model explicit; select-all scoped and stated
- [ ] Toolbar states name/count; bulk + primary actions live there
- [ ] Truncated values reachable in full; empty cells show "—"
- [ ] Sort marked on header; filter state visible
- [ ] Pagination vs infinite scroll chosen by positional need
- [ ] ≤ 3 inline row actions, rest in overflow; targets ≥ 24×24 px
- [ ] Narrow screens restructure rows into cards; status never color-only

## Anti-patterns
- Left-aligned amount column with mixed decimals
- Table scrolled until headers are gone
- Header checkbox that silently selects 10,000 filtered-out rows
- Truncated "1,2…" in a currency column
- Empty cells that read as rendering bugs
- Infinite scroll on an admin table users must reference by page
- Five icon-only actions crammed into every row
- Mobile table as horizontal scroll with 8 columns

## Sources
- Material Design 3 — Data tables: https://m3.material.io/components/data-tables/overview
- IBM Carbon — Data table: https://carbondesignsystem.com/components/data-table/usage/
- Atlassian Design System — Table: https://atlassian.design/components/table/examples
- Nielsen Norman Group — Flat design / comparison scanning: https://www.nngroup.com/articles/comparison-tables/
- Apple HIG — Tables (iOS/iPadOS): https://developer.apple.com/design/human-interface-guidelines/tables
