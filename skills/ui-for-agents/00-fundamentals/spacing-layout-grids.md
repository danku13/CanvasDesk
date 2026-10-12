# Spacing, Layout & Grids

> **Read when:** placing anything on a screen — paddings, gaps, grids, columns, page structure. **Section:** 00-fundamentals. **Related:** 00/visual-hierarchy.md, 00/iconography.md, 00/elevation-depth.md, 40/responsive-adaptive.md

**Core idea:** space is a structural material. A fixed scale plus role-based padding makes layouts feel deliberate; arbitrary values make them feel accidental.

Spacing and grids are the skeleton of every screen. All values here are px-equivalent in the platform's logical unit — map them to CSS pixels, device-independent points, or grid cells as the target renderer requires.

## Rules

### R1. Adopt a 4/8pt base unit
**Rule:** Every spacing and sizing value is a multiple of 4 (fine-grained: icon-to-label gaps, chip padding) or 8 (coarse: component padding, section gaps).
**Why:** One base unit makes any combination of components align mathematically and optically; screens built from one unit read as a system, not as pasted-together parts.
**Example:** A card = 16px padding, icon 8px from label, label 4px from value — every edge of every element lands on the 4px grid.

### R2. Use a fixed spacing scale — no arbitrary values
**Rule:** Ship exactly one scale (4, 8, 12, 16, 24, 32, 48, 64) and pick from it; intermediate values like 13, 18, or 22 are defects.
**Why:** A short scale keeps spacing decisions fast and makes proximity relationships legible — "16 vs 32" is a meaningful difference, "14 vs 16" is noise; if a case seems to need 20, the grouping is wrong, not the scale.
**Example:** Label→field 8, field→field 16, field-group→field-group 32 — the whole form needs nothing outside the scale.

### R3. Inner spacing < outer spacing
**Rule:** Padding inside a group must be visibly smaller than the gap between groups — target a 1.5–2× jump at boundaries.
**Why:** Proximity is the strongest grouping cue (gestalt); when inner and outer spacing are equal, grouping dissolves and users must parse semantics from content alone. See `00-fundamentals/visual-hierarchy.md` R5.
**Example:** Fields inside a "Shipping address" group 12px apart; the next group starts 32px away — the form reads as blocks without any borders.

### R4. Wide views get a 12-column grid and capped content widths
**Rule:** Lay wide views on a 12-column grid (12 divides by 2, 3, 4, 6); cap text measures at 65–75ch and typical app shells around 1100–1300px.
**Why:** Twelve columns flex into nearly every layout need without re-deriving the grid; uncapped content stretches reading measure past 75ch and drags scan paths across the whole viewport.
**Example:** A settings page: shell capped at 1200px, form column spanning 6 of 12 columns, help text 3 columns — not full-bleed text at 1900px wide.

### R5. One padding value per role
**Rule:** Define padding by role — page edge, section, and component each have exactly one canonical value, identical everywhere that role appears.
**Why:** Consistent per-role padding is what makes different screens feel like one product; role-based values (not per-screen judgment calls) are auditable and tokenizable.
**Example:** Page padding 24, card padding 16, chip padding 8 — on every screen, with no exceptions "to make it fit".

### R6. Align to explicit axes
**Rule:** Declare the layout's alignment axes (e.g. content-left, meta-right) and snap every element to one of them; no "almost aligned" positions.
**Why:** The eye snaps to edges and registers 1–2px misalignments as brokenness even when it can't name them; explicit axes make alignment mechanical instead of per-element guesswork.
**Example:** In a list row, avatar, title, and chevron each snap to a declared column edge — none sits at a hand-placed offset.

### R7. Gutters scale with viewport, never below touch spacing
**Rule:** Let gutters and outer margins grow with available width, but keep the gap between interactive elements at or above pointer-target size (≥ 24px; 44px recommended for touch).
**Why:** Spacing between targets is as functional as the targets — gaps below target size create accidental taps; generous gutters at large sizes stop content from sprawling.
**Example:** A toolbar at 360px keeps ≥ 8px visible gaps between icon buttons (each with a 44px target); at 1440px its outer margin grows 24→48px.

### R8. Leave breathing room at view edges
**Rule:** Content never touches viewport, panel, or card edges — every view has a defined minimum edge inset, applied to scrollable content too.
**Why:** Edge-flush content looks cropped and reads harder (no entry point for the eye); scroll containers are where edge padding most often goes missing.
**Example:** A scrollable table keeps its 24px side padding while scrolling — not only on first paint.

### R9. Align icons optically against text
**Rule:** Center icons against adjacent text by optical bounds (glyph mass, cap-height), not by their bounding box; nudge when needed.
**Why:** Icon boxes contain asymmetric whitespace (arrows, triangles, glyphs with overshoot); mathematical centering makes icons float high or low next to text. See `00-fundamentals/iconography.md`.
**Example:** A 24px chevron next to a 16px label nudged 1px to sit on the cap-height line; a play triangle nudged 1px toward its visual center.

### R10. Prefer flow layouts over absolute positioning
**Rule:** Compose screens from flow/flex/grid-style containers that derive positions from content and constraints — not from absolute coordinates — except for genuinely floating layers (tooltips, dialogs).
**Why:** Absolute positioning breaks the moment text wraps, fonts fall back, or content grows; constraint-based layouts degrade gracefully and survive translation and scaling.
**Example:** A toolbar built from spacing between items survives a longer localized label; an absolutely-placed button overlaps it instead.

### R11. Plan spacing for content that grows
**Rule:** Layout must survive text wrapping, longer localized strings, and state text (error/help messages) without overlapping or shifting controls — reserve vertical room for states.
**Why:** Spacing designed only for the default string breaks on the first long German label or validation message; growth-proofed spacing is what keeps states from being visually "bolted on".
**Example:** A field slot reserves room for one line of helper/error text; when the error appears, no neighbor moves and nothing overlaps.

### R12. Components span the grid; the grid doesn't bend
**Rule:** Blocks occupy whole column spans of the declared grid; a leftover half-column or an off-grid one-off span is a layout defect, not flexibility.
**Why:** The grid's value is that everything shares one rhythm — bending it per element restores the per-screen judgment calls the grid exists to prevent.
**Example:** Five cards on a 12-column grid become a 4+2 or 6+6 arrangement — never five 2.4-column cards.

### R13. Compress density within the scale
**Rule:** Data-dense views (tables, tools, IDE-like UIs) tighten spacing by consistently using the lower half of the scale (4/8/12) — never by inventing off-scale values.
**Why:** Density is a legitimate need, but off-scale compression recreates the arbitrary-values problem; a consistent "dense profile" keeps tight screens part of the same system.
**Example:** The data table uses 4px cell padding and 8px row gaps everywhere — not 5px here and 6px there.

## Checklist
- [ ] Every spacing value is a multiple of the base unit and a member of the declared scale
- [ ] No arbitrary values (13, 18, 22 …) anywhere in the layout
- [ ] Inner spacing visibly smaller than outer spacing at every group boundary (≥ 1.5×)
- [ ] Wide views use a declared 12-column grid; text measures capped at 65–75ch
- [ ] App shell max-width defined (~1100–1300px) and applied
- [ ] Page / section / component paddings are single canonical values used consistently
- [ ] Gaps between interactive elements never below 24px (44px touch recommended)
- [ ] Scrollable content keeps edge insets; nothing touches view edges
- [ ] Layouts are constraint/flow-based; absolute positioning only for floating layers
- [ ] Layout verified with wrapped text, longer strings, and error states

## Anti-patterns
- Magic numbers: 13px here, 18px there, 22px "because it looked right"
- Equal spacing everywhere — no proximity hierarchy
- Full-viewport-width paragraphs of text
- Card inside card inside card, each adding its own padding
- Elements 1–2px off the shared axis ("close enough")
- Gutters collapsing below touch-target spacing on small screens
- Content flush against panel edges, especially inside scroll areas
- Absolutely-positioned controls that overlap as soon as text grows

## Sources
- Material Design 3 — Layout basics & an applied approach: https://m3.material.io/foundations/layout
- Apple HIG — Layout: https://developer.apple.com/design/human-interface-guidelines/layout
- Nielsen Norman Group — Gestalt principles & whitespace: https://www.nngroup.com/articles/gestalt-principles/
- 8-Point Grid specification: https://spec.fm/specifics/8-pt-grid
- Nielsen Norman Group — Screen/viewport size considerations: https://www.nngroup.com/articles/screen-size-and-breakpoints-in-responsive-design/
