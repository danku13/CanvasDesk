# Dashboards & Data Visualization UX

> **Read when:** designing dashboards, KPI rows, charts, reports, or any data display region. | **Section:** 30-ux-patterns | **Related:** ./data-visualization.md, ../00-fundamentals/visual-hierarchy.md, ../00-fundamentals/color.md, ../40-quality/usability-heuristics.md, ../20-components/tables-and-data-lists.md, ../40-quality/perceived-performance.md

**Core idea:** a chart is an answer to a question. Pick the form from the question, not from what the library offers; make the current value readable in 3 seconds and the distortion level zero.

## Rules

### R1. Choose the chart by the user's task
**Rule:** Map the question to the form: comparison across categories → bar; trend over time → line; part-of-whole (≤ 5 parts) → pie or stacked bar; distribution → histogram; relationship between two variables → scatter.
**Why:** Each visual form encodes one comparison best (decoding accuracy falls from position to length to angle to area); the wrong form forces the user to mentally re-encode the data, adding error at every read.
**Example:** Revenue per region = bar. Revenue over 24 months = line. Market share of 4 vendors = pie. Order-value distribution = histogram. Ad spend vs conversions = scatter.

### R2. Bars start at zero; lines may zoom
**Rule:** Bar charts always use a zero baseline. Line charts may use a zoomed baseline when trend, not magnitude, is the message — with the axis clearly labeled.
**Why:** Bars are read as length, so a truncated bar shows 2× the real value — a lie even with an axis label; lines are read as position along a trend, where zoom legitimately reveals slope detail.
**Example:** 90 vs 100 comparison: bar chart at zero honestly shows a 10% gap; starting bars at 85 draws a 6× taller bar for the same data = broken.

### R3. Direct labeling beats legends
**Rule:** When space allows, label series at their line ends or bar tops instead of a separate legend; keep the legend only for many-series or dense charts.
**Why:** A legend forces a color-key round trip for every reading; direct labels remove the mapping step and survive color-vision deficiencies and grayscale output.
**Example:** A 3-line chart gets company names at each line's right end — no 3-swatch legend box to decode.

### R4. Limit chart colors; red means bad only
**Rule:** Use the minimum number of hues (≤ 5–6 per chart) and reserve semantic colors: red only for negative/failure, green only for positive. Keep text ≥ 4.5:1 and chart marks ≥ 3:1 contrast against the background.
**Why:** Semantic colors are a learned signal channel (status visibility — see ../40-quality/usability-heuristics.md); diluting red with "this series is red because it's brand primary" makes genuine alerts unreadable.
**Example:** A P&L chart colors losses red and gains in neutral blues; "current year" is never red unless it lost money.

### R5. Tables for lookup, charts for shape
**Rule:** Use a table when the user needs exact values for known rows; use a chart when they need shape, trend, or outliers. Never force lookup through tooltips, or shape-reading through a 40-row table.
**Why:** Charts trade precision for perception; tooltip-hunting for one number is slower than a table, and a table makes "trending down" computable only by the user's own mental math.
**Example:** Invoice list = table. "How did collections move this quarter?" = line chart; clicking a point opens the underlying table rows.

### R6. KPI cards: value + delta + comparison window
**Rule:** Every KPI card shows the current value, the delta, and the explicit comparison window ("€48.2k +12% vs last month").
**Why:** A bare number is uninterpretable — good or bad? The delta plus window supplies the judgment in one glance and prevents misreading a seasonal value as a trend (visibility of status — ../40-quality/usability-heuristics.md).
**Example:** "Revenue €48.2k ↑ 12% vs last month" — not "Revenue 48.2k", which forces the user to remember last month's number.

### R7. No dual axes, no 3D, no decorative distortion
**Rule:** Prohibit dual y-axes, 3D charts, shadows on bars, gradients that change perceived area, and any effect that alters the encoded magnitude.
**Why:** Dual axes invent relationships through arbitrary scaling; 3D and area effects distort the encoding (angles and areas read 2–3× off), so every reading becomes wrong by an unknowable amount.
**Example:** Revenue (line) and orders (bar) shown as two stacked charts sharing an x-axis — not one chart with two y-scales tuned until the lines "intersect nicely".

### R8. One tooltip/hover behavior across all charts
**Rule:** Every chart in the product uses the same hover/touch detail pattern: same placement, same formatting (localized numbers, full dates), same keyboard/screen-reader accessible equivalent.
**Why:** Users learn one inspection gesture; per-chart quirks mean every chart must be re-learned, and inconsistent formats (42% vs 0.42 vs "42 pct") cause misreads at exactly the moments that matter.
**Example:** Hovering any series point shows "Mar 2024 — Revenue: €48,200" with the same offset and typography on every dashboard.

### R9. Glanceability first: 3-second read
**Rule:** Each chart card answers title, current value, and direction within 3 seconds: descriptive title ("Revenue — last 12 months"), prominent current value, trend visible without interaction. Verify with the squint test (../00-fundamentals/visual-hierarchy.md R12).
**Why:** Dashboards are revisited dozens of times a day in seconds-long sessions; anything requiring manipulation (hover, click, legend decode) fails the actual usage pattern.
**Example:** A card whose title, big number, and sparkline read correctly blurred to 20% is done; one where you must hover to learn the unit is not.

### R10. Label units and time ranges explicitly
**Rule:** Every axis and KPI states its unit and its time range ("€ thousands", "UTC", "Jan–Jun 2024"); never leave scale or period implicit.
**Why:** The same numbers mean opposite things across units and periods ("+12%" of what window?); unstated ranges are the most common source of confident wrong decisions made from dashboards.
**Example:** "Revenue (€k) — Jan–Jun 2024" on the card beats a chart whose users silently assume the fiscal year.

### R11. Honest axes: few, labeled, no noise
**Rule:** Use at most ~5 gridlines with direct value labels, evenly spaced ticks, no gratuitous decimals, and no axis breaks without a visible, labeled break marker.
**Why:** Axis furniture is the reference frame for every value read; dense gridlines and split axes corrupt that frame, so even honest data reads wrong.
**Example:** A y-axis with 0 / 25k / 50k / 75k / 100k labels reads instantly; one with nine faint unlabeled gridlines and a hidden break does not.

### R12. Answer-first layout
**Rule:** Place the most important KPI in the prime position (start of the scan path — top-left for LTR) and give it the largest tile; importance maps to position and size, never to decoration.
**Why:** Dashboards are scanned, not read (F-pattern — ../00-fundamentals/visual-hierarchy.md R4); the answer users open the dashboard for must be the first thing the eye lands on.
**Example:** "Active incidents" (the reason this dashboard exists) is the biggest tile at top-left; supporting charts fill smaller tiles below in descending importance.

## Checklist
- [ ] Every chart maps to a stated user question (comparison/trend/part-of-whole/distribution/relationship)
- [ ] Bars start at zero; any non-zero baseline appears on lines only and is labeled
- [ ] Direct labels used where the series count allows
- [ ] ≤ 5–6 hues per chart; red/green reserved for bad/good semantics; marks ≥ 3:1 contrast
- [ ] Exact-value needs served by tables, shape needs by charts
- [ ] All KPI cards show value + delta + explicit comparison window
- [ ] No dual axes, no 3D, no magnitude-distorting decoration
- [ ] Tooltip format and placement identical across all charts
- [ ] Squint test passes: title, current value, and trend readable in 3 seconds
- [ ] Units and time ranges visible on every axis and KPI
- [ ] Most important KPI occupies the prime scan-path position
- [ ] Axes: ≤ ~5 labeled gridlines, no hidden breaks, no gratuitous decimals

## Anti-patterns
- A pie chart with 9 slices and a legend of similar grays
- Bar chart with a zoomed baseline "to show the difference better"
- Dual-axis chart implying correlation between two arbitrarily scaled series
- 3D exploded pie with shadows on a corporate dashboard
- Giant number with no delta, period, or comparison ("Revenue: 48213")
- Red used as "current period" brand color on financial charts
- Tooltip showing raw "1710720000" or "0.482" instead of a localized value
- Sparkline-only cards where nothing is readable without hovering
- A wall of equal-sized tiles with no answer-first ordering
- A chart whose y-axis silently omits zero or hides an axis break

## Sources
- Stephen Few — Perceptual Edge, dashboard design & perceptual accuracy of encodings: https://www.perceptualedge.com/
- Edward Tufte — The Visual Display of Quantitative Information: https://www.edwardtufte.com/
- Datawrapper Blog — chart type selection ("chart chooser") guides: https://blog.datawrapper.de/
- Nielsen Norman Group — Ten usability heuristics (visibility of system status): https://www.nngroup.com/articles/ten-usability-heuristics/
- Material Design 3 — Color system: https://m3.material.io/foundations
