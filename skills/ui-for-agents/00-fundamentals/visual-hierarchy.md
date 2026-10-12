# Visual Hierarchy

> **Read when:** designing any new screen, section, or component arrangement. **Section:** 00-fundamentals. **Related:** 00/spacing-layout-grids.md, 00/color.md, 00/typography.md, 20/components-catalog.md

**Core idea:** hierarchy decides what the user sees first, second, third. If everything shouts, the user hears nothing. Hierarchy is built deliberately, with a small set of tools, and verified at a glance.

## Rules

### R1. One primary action per view
**Rule:** Exactly one visually dominant action per screen or section (filled/accent style). Every other action is secondary or tertiary.
**Why:** Competing primaries paralyze choice and dilute the call to action; users scan for the strongest signal and follow it, so two equal signals halve the chance of the right one.
**Example:** In a "Create invoice" dialog: primary "Create" (filled), secondary "Save draft" (outline), tertiary "Cancel" (ghost). Three filled buttons = broken.

### R2. Differentiate levels with three tools combined
**Rule:** Build hierarchy by combining size, weight, and color — never rely on a single property alone.
**Why:** Each property alone is weak (a 1px size change is invisible; color fails for color-blind users and in monochrome); combined, small changes compound into clear levels without visual noise.
**Example:** Page title 24px/600, section title 18px/600 same hue, body 16px/400 muted ink — three distinct levels, no decoration needed.

### R3. Limit text levels to three per view
**Rule:** Primary, secondary, tertiary — that is the full set of text emphasis levels for one view.
**Why:** Each additional level adds comparison cost; users can hold ~3 levels in working memory while scanning, beyond that hierarchy stops communicating.
**Example:** If a fourth level is needed (e.g. caption), reuse tertiary with reduced opacity instead of inventing a new size.

### R4. Place by reading order of the target locale
**Rule:** Put the most important content at the start of the scan path (top-left for LTR, top-right for RTL) and the primary action at the natural end of the path.
**Why:** Eye-tracking shows F/Z-shaped scanning; content outside the path is functionally invisible, and actions at the path's end benefit from completed orientation.
**Example:** A form dialog: title top, fields middle, primary button bottom-right (LTR) — the eye arrives at "Create" already knowing what it does.

### R5. Group by proximity
**Rule:** Spacing within a group must be clearly smaller than spacing between groups — target a visible 1.5–2× jump at group boundaries.
**Why:** Proximity is the strongest grouping cue (gestalt); equal spacing everywhere means the user must parse semantics themselves.
**Example:** Form fields 12px apart inside a section, 32px between sections — the sections read as blocks without any divider lines.

### R6. Consistent look means consistent meaning
**Rule:** Elements that look identical must behave identically; elements with different behavior must differ visibly.
**Why:** Users learn the interface by pattern-matching; a lookalike that behaves differently reads as a bug and destroys trust in every other element.
**Example:** If outline buttons navigate and filled buttons submit, never ship an outline "Submit".

### R7. Whitespace before decoration
**Rule:** When grouping or emphasis is unclear, add space or remove elements — do not add lines, boxes, or backgrounds first.
**Why:** Whitespace carries no pixel cost and cannot clash; boxes add borders that compete for attention and stack into visual mush at the second nesting level.
**Example:** Card-in-card-in-card layouts should be flattened into spacing-separated sections.

### R8. Align to one axis
**Rule:** Pick a single vertical (and where relevant horizontal) alignment axis per view and align every block to it; align icons optically, not mathematically.
**Why:** Alignment is the cheapest order cue; mixed axes make even correct layouts feel broken, and icon boxes rarely match text bounds optically.
**Example:** A settings page where labels, fields, and help text share one left edge reads instantly; icons next to labels are nudged 1px up to look centered.

### R9. Reserve the accent color for what matters
**Rule:** Use the accent/brand color for the primary action and critical signals only — as a budget, ≤ ~10% of the view.
**Why:** Accent works by contrast against everything else; spread everywhere, it stops marking anything, and truly important elements lose their signal channel.
**Example:** A dashboard with an accent "New report" button and neutral everything else; if every card header is accent-colored, nothing is.

### R10. Front-load keywords
**Rule:** Put the distinguishing word first in titles, list items, and buttons ("Invoice overdue — #1042", not "#1042 — the invoice is overdue").
**Why:** Users scan first words and left edges (F-pattern); back-loaded keywords are missed, especially in lists and narrow columns.
**Example:** Notification list readable when truncated to 40% width because every row starts with its subject.

### R11. Use progressive disclosure for depth
**Rule:** Show the default path flat; hide advanced, rare, or dangerous options behind a clearly-marked control ("Advanced", "More").
**Why:** Hierarchy includes frequency-of-use — flat equal UI forces every user to process every option every time.
**Example:** Export dialog: format + filename visible; "Metadata", "Encryption" behind "Advanced options".

### R12. Verify with the squint test
**Rule:** Blur or squint at the final layout for 3 seconds: title, primary action, and key data must be identifiable without reading details.
**Why:** The blur test removes text content and leaves only the hierarchy signals (size, weight, color, spacing) — exactly what a first-time scanner uses.
**Example:** If blurred, the page reads as one gray mass, hierarchy is decoration, not structure — fix levels before shipping.

## Checklist
- [ ] Exactly one primary action per view; all others secondary/tertiary
- [ ] No more than three text emphasis levels
- [ ] Spacing jump ≥ 1.5× at every group boundary
- [ ] Most important content sits on the locale's scan path; primary action at its end
- [ ] Lookalike elements behave identically
- [ ] Accent color covers ≤ ~10% of the view and marks only key elements
- [ ] Titles/list items front-load their distinguishing word
- [ ] Squint test passes: structure readable in 3 seconds

## Anti-patterns
- Three equal cards, each with its own filled button
- Bold + larger + colored applied at once everywhere "for emphasis"
- Center-aligned paragraphs of long text
- Divider lines and boxes instead of spacing
- A wall of same-strength links or nav items
- Important content placed below the fold or off the scan path because "it fits there"

## Sources
- Material Design 3 — Applying hierarchy & layout: https://m3.material.io/foundations
- Apple HIG — Visual design: https://developer.apple.com/design/human-interface-guidelines
- Nielsen Norman Group — Visual hierarchy & F-shaped scanning: https://www.nngroup.com/articles/visual-hierarchy-ux/ , https://www.nngroup.com/articles/f-shaped-pattern-reading-web-content/
- Laws of UX — Hick's law: https://lawsofux.com/hicks-law/
- Gestalt principles overview: https://www.nngroup.com/articles/gestalt-principles/
