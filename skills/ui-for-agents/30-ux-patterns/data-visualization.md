# Data Visualization Craft

> **Read when:** building the chart itself — encodings, color scales, axes, legends, uncertainty, chart interactions, or chart accessibility. | **Section:** 30-ux-patterns | **Related:** ./dashboards-data-viz-ux.md (dashboard-level UX: KPI cards, chart selection, honest axes), ../00-fundamentals/color.md, ../40-quality/accessibility-wcag.md, ../10-design-system/theming-dark-mode.md, ../20-components/tables-and-data-lists.md

**Core idea:** a chart is a perceptual encoding pipeline — every visual channel has a decoding accuracy, so the message-critical dimension goes to the most accurate channel, color scales match the data type, and the story is readable without the pixels.

This file covers the craft inside one chart. Dashboard-level concerns (tile layout, KPI cards, chart selection by question, glanceability) live in `dashboards-data-viz-ux.md`.

## Rules

### R1. Rank channels by decoding accuracy; give the best to the message
**Rule:** The dimension the user must judge most precisely goes to position (or length); angle, area, color saturation, and texture decode progressively worse — use them only for supporting context.
**Why:** Perceptual experiments (Cleveland & McGill) rank encoding accuracy: position ≫ length ≫ angle ≫ area ≫ volume ≫ color; putting the critical comparison into area or hue forces users to judge magnitudes they cannot actually judge.
**Example:** "Which region leads?" → region on a common-position axis (bar/line); "audience segment mix" as a side context may stay a donut — never the reverse.

### R2. Match the color scale to the data type
**Rule:** Magnitude data uses a sequential single-hue (or viridis-class) ramp; data with a meaningful midpoint (zero, target, pre/post) uses a diverging ramp centered there; unordered categories use ≤ 6–7 maximally distinguishable, colorblind-safe hues — never the rainbow jet scale for ordered data.
**Why:** Each scale type encodes a different data relationship; a rainbow scale on ordered data invents boundaries that don't exist, and a diverging scale without a real midpoint implies a fake "good/bad" split.
**Example:** Income by district → sequential blues; profit/loss around 0 → red–gray–blue diverging anchored at zero; sales channels → 5 colorblind-safe categorical hues.

### R3. Survive grayscale and color-vision deficiency
**Rule:** Any two series remain distinguishable without hue: vary lightness, dash pattern, or marker shape; verify the chart in a CVD simulator or grayscale pass; red/green pairs are banned as sole differentiators.
**Why:** ~8% of men have impaired red-green perception, charts get printed, screenshotted, and projected in grayscale, and hue-only series collapse into identical noise (see ../00-fundamentals/color.md R9–R10).
**Example:** Forecast vs actual = solid line + dashed line with distinct lightness; a grayscale print still tells them apart.

### R4. Sort categories by value; annotate, don't alphabetize by default
**Rule:** Bar-like charts order categories by the encoded value (descending, or by a meaningful order), not alphabetically; the "story" data point (record, outlier, event) gets a direct annotation.
**Why:** Sorted bars let readers rank instantly and reveal distribution shape; alphabetical order scatters the message and forces mental re-sorting, and unannotated outliers get read as errors instead of events.
**Example:** Revenue by product sorted top-to-bottom with the launch month of the spike annotated ("v2 launch") — not A-to-Z bars where the spike looks like a bug.

### R5. Small multiples beat one overloaded chart
**Rule:** When comparing the same measure across many groups or time windows, repeat one small chart per group with identical scales and axes instead of stacking 8+ series into one plot.
**Why:** Perceptual comparison across panels with shared scales is fast and accurate, while a 12-line spaghetti chart forces legend decoding (see R3) and line-crossing ambiguity.
**Example:** Traffic by 6 channels = 6 mini sparkline-panels sharing one y-scale; one 12-line chart is unreadable even with 12 direct labels.

### R6. Show uncertainty where it exists
**Rule:** Forecasts, estimates, and measured samples carry their uncertainty visibly — confidence bands, error bars, or explicit ranges; the rendering precision never implies data precision.
**Why:** A point estimate drawn as an exact line claims knowledge it doesn't have; users make operational decisions ("order 48,200 units") from the pixel, so the honest interval changes the decision.
**Example:** Demand forecast = median line + shaded 80% band; "€48.2k ±3.1k" on the KPI (see ./dashboards-data-viz-ux.md R6).

### R7. Interaction: default view must stand alone
**Rule:** Zoom, pan, brush, and cross-filtering are enhancements on top of a complete static read — axis resets are obvious, selections highlight across linked charts, and no essential value lives behind hover only.
**Why:** Screenshots, exports, screen readers, and the first 3 seconds all see the static frame; interaction that hides the message (hover-only labels) breaks glanceability and accessibility at once.
**Example:** Brushing a time range highlights the matching bars in the linked region chart and shows an obvious "Reset zoom" chip; the current values remain labeled without any hover.

### R8. Every chart is accessible without vision
**Rule:** Each chart exposes a text alternative — a `role="img"`/figure description stating the takeaway, plus the exact values via an accompanying table or downloadable data; series meaning never rides on color alone (WCAG 1.4.1).
**Why:** Screen-reader users get nothing from pixels; a description with the conclusion ("Revenue grew 12% QoQ, led by EU") serves them, and the table serves everyone needing exact numbers (see ./dashboards-data-viz-ux.md R5).
**Example:** `<figure>` with `aria-label` "Line chart: monthly revenue Jan–Jun, up 12% overall; peak May €48.2k" + a details/table fallback with all plotted points.

### R9. Re-theme charts for dark mode; never invert them
**Rule:** Dark themes re-map chart roles through tokens: series colors shift lightness for dark surfaces, gridlines become faint light-on-dark, axis text follows the theme text role — a CSS/filter inversion is a defect.
**Why:** Inverted charts turn brand blues into oranges, halate saturated series, and break the ≥ 3:1 mark-to-surface floor (see ../00-fundamentals/color.md R7, R13 and ../10-design-system/theming-dark-mode.md).
**Example:** Dark dashboard: series #8ab4f8 on #202124 surface, gridlines at 12% white — same data voice as the light theme, zero inverted panels.

### R10. Control the ink; the data is the loudest element
**Rule:** Chart furniture (gridlines, borders, tick marks) stays lighter than data marks; drop 3D, shadows, and double borders; axes label units explicitly.
**Why:** Contrast is attention — furniture that out-shouts data inverts the figure-ground relationship (see ../00-fundamentals/visual-hierarchy.md), and decorative depth distorts decoded magnitude (see ./dashboards-data-viz-ux.md R7).
**Example:** Faint 12%-opacity gridlines behind 3 px data lines; the y-axis reads "Revenue (€k)"; no bevels anywhere.

## Checklist
- [ ] Message-critical dimension encoded as position/length, not angle/area/hue
- [ ] Color scale type matches data type: sequential / diverging (real midpoint) / categorical (≤ 6–7 hues)
- [ ] Series distinguishable in grayscale and CVD simulation (lightness/dash/marker, not hue alone)
- [ ] Categories sorted by value by default; story points annotated directly
- [ ] Many-group comparisons rendered as small multiples with shared scales
- [ ] Uncertainty shown (band/error bars/range) wherever estimates exist
- [ ] Static frame self-sufficient: no essential value hover-only; zoom/filter have visible resets
- [ ] Text alternative states the takeaway; exact values available via table/data export
- [ ] Dark theme re-maps chart tokens; marks keep ≥ 3:1 against surface; nothing inverted
- [ ] Furniture quieter than data: faint gridlines, labeled axes with units, zero decoration distorting magnitude

## Anti-patterns
- Rainbow (jet) scale on ordered data inventing false boundaries
- 12-series line chart with one shared legend of similar hues
- A pie chart asked "which is 3rd largest?" (angle ranking is unreliable)
- Alphabetical bars hiding the ranking
- A forecast line drawn with the same confidence as history, no band
- Hover-only value labels in a dashboard that gets screenshotted for reports
- `filter: invert(1)` dark mode producing orange brand charts
- 3D donut with shadow reading 30% as 45°
- Chart with no unit labels — "is that € or units? months or weeks?"
- An embedded chart image with no description and no data table

## Sources
- Cleveland & McGill — Graphical Perception: theory & ranking of encodings: https://www.stat.berkeley.edu/~stark/Teach/s244/lec-notes/Cleveland-McGill.pdf
- ColorBrewer — colorblind-safe sequential/diverging/qualitative scales: https://colorbrewer2.org/
- Viridis colormaps — perceptually uniform ramps: https://bids.github.io/colormap/
- Datawrapper — chart-choice and scale guidance: https://blog.datawrapper.de/
- Edward Tufte — small multiples & data-ink ratio, The Visual Display of Quantitative Information: https://www.edwardtufte.com/
- Stephen Few — perceptual edge, chart color & dashboard craft: https://www.perceptualedge.com/
- WCAG 2.2 — 1.4.1 Use of Color, 1.1.1 Non-text Content: https://www.w3.org/TR/WCAG22/
