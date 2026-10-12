# Search, Filtering & Sorting

> **Read when:** building any list view, search field, filter bar, or table with more than a screenful of data. | **Section:** 30-ux-patterns | **Related:** ../20-components/tables-and-data-lists.md, ../20-components/forms-and-inputs.md, ../20-components/navigation-patterns.md, empty-loading-error-states.md, forms-validation-ux.md

**Core idea:** search and filtering are conversations with the dataset. The interface must always answer three questions — what was found, why, and what is currently excluded — and let the user revise cheaply.

## Rules

### R1. Put search at the top of the region it searches
**Rule:** The search box sits at the top of, and visually attached to, the region it operates on — app header for global search, panel header for a panel's list. Follow platform conventions for position.
**Why:** Users look for search in learned locations; a search box detached from its results forces a jump between query and feedback that breaks the correction loop.
**Example:** Global search in the app header; the user-list search inside the Users panel header — not a "Search" menu item buried two levels deep.

### R2. Always show result count and applied filters
**Rule:** The results region displays how many items matched and which filters are active at all times ("42 results · Type: Invoice · Status: Overdue").
**Why:** The count is the primary feedback that the query registered; without it users cannot tell "no matches" from "still loading" from "my filter disappeared".
**Example:** "42 results for 'invoice' — filtered: Overdue, Q3" above the list; the count updates the moment filters change.

### R3. Active filters are removable chips plus one "Clear all"
**Rule:** Render each active filter as a chip with its own remove control (minimum 24×24 px hit area), plus a single "Clear all" that resets everything.
**Why:** Chips make the current query legible and reversible one click at a time; without "Clear all", unwinding 6 filters costs 6 precise clicks and users give up instead of retrying.
**Example:** [Overdue ×] [Q3 ×] [Assignee: Ana ×]  Clear all — every × is a full-size target.

### R4. Debounce input 250–400 ms; never block typing
**Rule:** Fire search requests 250–400 ms after the last keystroke; every keystroke must register instantly in the field, and a pending request must not freeze or swallow input.
**Why:** Debounce bounds request volume without perceptible lag (people pause between words anyway); a field that locks while "searching" turns refinement into fighting the UI.
**Example:** Typing "overdue inv" runs one query for the final term, not six; the field never grays out while results stream.

### R5. Zero results is a designed state
**Rule:** No-match states offer, in order: a spelling hint, the nearest matches, and one-click relaxation ("Remove 'Q3' filter", "Search all sections").
**Why:** Zero results is where users most often abandon entirely; each offered escape converts a dead end into a revised query (full state treatment in empty-loading-error-states.md).
**Example:** "No invoices match 'invioce'. Did you mean 'invoice'? 12 results without the Overdue filter → [Remove filter]".

### R6. Default sort serves the task, not the alphabet
**Rule:** Choose the default sort by what makes the task fastest: recency for feeds and logs, relevance for search, due date for work queues. Alphabetical is a fallback for reference lists (names, countries), never the lazy default.
**Why:** Users scan the first screenful; the most useful items must be there. An alphabetical default buries the 3 overdue invoices behind 200 A–N rows.
**Example:** Inbox sorts by newest; search sorts by relevance; "Countries" in a settings form sorts alphabetically.

### R7. Persist query state in the URL or route
**Rule:** Search term, active filters, sort column, and direction are encoded in the URL/route; opening, sharing, refreshing, and Back restore the exact result set.
**Why:** Filtered results are artifacts users want to send to teammates and return to; ephemeral state makes Back a data-loss event and makes support tickets unanswerable ("send me the link you see").
**Example:** /invoices?q=acme&status=overdue&sort=-due_date reproduces the identical view for anyone who opens it.

### R8. Make scope explicit for large corpora
**Rule:** When the product has multiple searchable spaces (projects, files, people), the search UI states what it is searching and offers an explicit scope switch or "search all".
**Why:** A scopeless search returns confusing blends, and users cannot distinguish "no result" from "not in this section"; explicit scope restores predictability.
**Example:** The search bar reads "Search this project" with a scope dropdown "This project / All projects / People"; the results page groups by type with counts.

### R9. Offer recents and suggestions where privacy allows
**Rule:** Provide recent searches and type-ahead suggestions when the context allows it; do not persist or surface search history on shared devices or for privacy-sensitive corpora.
**Why:** Suggestions cut typing and teach the query syntax; surfaced history on a shared machine is a privacy leak that outweighs the convenience.
**Example:** Focusing search shows "Recent: 'invoice Q3', 'acme'" on a personal account; a kiosk or shared login shows no history at all.

### R10. Client-side filtering only for bounded datasets
**Rule:** Filter fully on the client only when the whole dataset is bounded and small (rule of thumb: up to a few thousand rows, all already shipped to the client). Write the chosen threshold and dataset-size assumption into the project documentation — not only into code comments.
**Why:** Client filtering degrades silently as data grows — first slow typing, then memory pressure; a documented threshold forces a deliberate architecture decision before users feel the decay.
**Example:** A 500-row settings list filters instantly in memory; a 200k-row transaction table queries the backend per debounced keystroke.

### R11. Sorting is visible, directional, and reversible
**Rule:** The active sort column and direction are always indicated; clicking a sortable header toggles direction; the toggle target meets the 24×24 px minimum.
**Why:** Sorting without visible state produces "the list changed mysteriously" reports; reversible toggling is how users verify the sort did what they meant.
**Example:** "Due date ↓" is highlighted; a second click flips to ascending; the indicator survives reload via R7 state.

### R12. Filter controls match the data type
**Rule:** Enum fields filter as checkboxes or multi-select chips, dates as explicit ranges, numbers as min/max, booleans as toggles — never as free-text inputs into structured fields.
**Why:** A free-text filter on a typed field either never matches ("true" vs "yes" vs checked) or silently narrows results in ways the user cannot see, producing wrong conclusions from correct data.
**Example:** Status filter shows checkboxes Draft/Paid/Overdue; date filter shows two date pickers — not one text field where users guess the format.

### R13. Show the query with the results
**Rule:** The results view keeps the active term visible (in the search box) and highlights matched terms within results; the user always sees what was searched next to what was found.
**Why:** Highlighting proves the match logic and lets users spot wrong-field matches instantly; a results list without the visible query invites misreading stale results for new ones.
**Example:** Searching "acme" shows every "Acme" occurrence bolded in titles and snippets, with "acme" still sitting in the search box.

## Checklist
- [ ] Search box sits at the top of the region it searches
- [ ] Result count and active filters are always visible with results
- [ ] Every active filter is an individually removable chip; one "Clear all" exists
- [ ] Input debounced 250–400 ms; typing never blocked
- [ ] Zero-results state offers spelling hint, nearest matches, or filter relaxation
- [ ] Default sort chosen by task, not alphabetical habit
- [ ] Query/filter/sort state survives refresh and Back, and is shareable via URL
- [ ] Search scope is explicit where multiple corpora exist
- [ ] Recents/suggestions appear only where privacy allows
- [ ] Client-side filtering threshold documented for the dataset at hand

## Anti-patterns
- Search box in a footer or hidden behind an icon where convention says top
- Filters that apply silently with no count, chip, or indication
- A mandatory "Search" button click for every query on a type-as-you-search surface
- Zero debounce hammering the backend per keystroke — or 2s debounce that feels broken
- "No results found." as the entire response to a typo
- Default alphabetical sort on a work queue
- Filter state lost on Back navigation or refresh
- Showing one user's recent searches on a shared/kiosk device
- A text-input filter over a boolean or enum field

## Sources
- Nielsen Norman Group — Ten usability heuristics (visibility of system status, user control): https://www.nngroup.com/articles/ten-usability-heuristics/
- Material Design 3 — Search component: https://m3.material.io/components/search/overview
- WCAG 2.2 — Target Size (Minimum) 2.5.8: https://www.w3.org/TR/WCAG22/#target-size-minimum
- MDN — URLSearchParams (state-in-URL technique): https://developer.mozilla.org/en-US/docs/Web/API/URLSearchParams
- Laws of UX — Hick's law (choice cost in filter panels): https://lawsofux.com/hicks-law/
