# Typography

> **Read when:** setting any text — sizes, weights, line-heights, measures, or choosing and stacking fonts. **Section:** 00-fundamentals. **Related:** 00/visual-hierarchy.md, 00/color.md, 00/spacing-layout-grids.md, 00/internationalization-rtl.md, 40/accessibility-wcag.md

**Core idea:** typography is the interface's voice and its hierarchy engine. A small modular scale, strict line-height and measure rules, and 2–3 weights do more for readability than any font choice.

Text is the primary interface material — most UI is text. These rules hold for any renderer: DOM, native toolkit text widgets, or canvas glyph rasterization. All sizes are px-equivalent in the platform's logical unit.

## Rules

### R1. Use a modular scale, not arbitrary sizes
**Rule:** Define 4–6 text steps from one ratio (~1.125–1.25) and use only those steps; never eyeball a one-off size.
**Why:** A single ratio produces harmonious, predictable steps and forces hierarchy decisions ("which level is this?") instead of size inflation; arbitrary sizes accumulate into a dozen near-identical values nobody can audit.
**Example:** Ratio 1.2 over a 16px base: 13.3 / 16 / 19.2 / 23 / 27.6 — caption, body, subtitle, title, display. Any new text must be one of these.

### R2. Body text starts at 16px-equivalent
**Rule:** Default body size is 16 px-equivalent; smaller sizes are reserved for captions and labels, never for primary reading text.
**Why:** 16px is the size major platforms assume for comfortable reading; below it, legibility and effective zoom behavior degrade — especially for low-vision users.
**Example:** Form labels and helper text may sit at 13–14px; the paragraph explaining billing terms is 16px. A 12px legal wall is a defect, not a style.

### R3. Line-height: 1.5 for body, 1.2–1.3 for headings
**Rule:** Set body line-height to 1.5 (never below 1.4); tighten headings to 1.2–1.3.
**Why:** Body lines need vertical air so the eye returns to the line start reliably; headings carry few words, so extra leading creates floaty, disconnected titles.
**Example:** Body 16/1.5 = 24px line box; a 28px page title at 1.2 = 33.6px. Setting the title to 1.5 only inserts dead space before the content.

### R4. Keep measure at 45–75 characters
**Rule:** Constrain text containers so a full line holds 45–75 characters (~65 ideal for long-form); cap text blocks with a max-width, never with the viewport.
**Why:** Below ~45 the eye turns too often and rags badly; above ~75 the return sweep loses its place and reading speed drops — the effect is independent of font size.
**Example:** An article column maxes at ~680px at 16px body; the surrounding app shell stays wider for toolbars and data panels.

### R5. Max 2 families and 2–3 weights
**Rule:** Use at most two type families (e.g. one for UI/body, optionally one for display or code) and at most 2–3 weights (400 regular, 500/600 semibold, rarely 700).
**Why:** Each family and weight is another axis the eye must compare; multiple weights also invite inconsistent application across teams — and every loaded weight costs bytes.
**Example:** Body 400, headings/emphasis 600, code in a monospace family — done. "Light" 300 body text on screens fails legibility and is banned here.

### R6. Sentence case for UI text
**Rule:** Write buttons, labels, titles, and menu items in sentence case ("Save changes"); use ALL CAPS only for tiny overline labels, and Title Case only where platform convention demands it.
**Why:** Mixed-case words have distinctive silhouettes and read faster than caps; all-caps strings slow scanning and read as shouting, which burns the emphasis channel.
**Example:** "Create new invoice" button, "Billing address" label; "OVERDUE" is allowed only as a small tracked-out overline badge.

### R7. Tabular figures for numbers in columns
**Rule:** Where numbers stack vertically or update live — tables, prices, timers, stats — use tabular (monospaced-width) figures so every digit shares one advance width.
**Why:** Proportional digits make columns jitter and comparisons error-prone; a value whose width changes frame-to-frame (counters, timers) reads as flicker.
**Example:** A price table where 9.99 / 110.00 / 7.50 right-align digit-for-digit; a stopwatch whose digits don't shake as they tick.

### R8. Letter-spacing: tighten large, loosen small caps
**Rule:** Apply slightly negative tracking to large display text (~-1 to -2%) and positive tracking (~+5–10%) to small ALL-CAPS labels; never track body text.
**Why:** Natural glyph spacing reads loose at display sizes and tight at caption sizes — an optical artifact of glyph size, not a style preference.
**Example:** A 32px headline at -1% looks composed; an 11px "SETTINGS" overline at +8% looks intentional; 16px body at -1% looks broken.

### R9. Build hierarchy by co-varying size, weight, and color
**Rule:** Differentiate text levels by changing at least two of size, weight, or color at once; never separate levels with a single subtle property.
**Why:** One property alone sits below perception thresholds (a 1px size change or a faint gray shift is invisible); combined small deltas compound into instantly scannable levels. See `00-fundamentals/visual-hierarchy.md`.
**Example:** Page title 24px/600, section title 19px/600, body 16px/400 in main ink, caption 13px/400 in secondary ink — four roles, all distinguishable in a squint test.

### R10. Always ship metric-aware fallbacks
**Rule:** Every custom font ships with a named fallback chain and size/x-height adjustment so line boxes don't jump when the primary font fails to load or is disabled.
**Why:** Font loading fails (offline, blocked, slow networks); without metric-compatible fallbacks, text reflows, truncates, and breaks layouts — and restricted-network users see the broken version.
**Example:** Custom display font layered over the platform system stack, adjusted so the fallback keeps every text container inside the 45–75ch measure.

### R11. Scale type responsively; respect user settings
**Rule:** Where the platform supports it, use fluid sizing (clamped interpolation between viewport bounds) for display-level text, and always honor user font-size preferences for body text.
**Why:** Fixed pixel headings overflow on small screens and look timid on large ones; ignoring user scaling breaks zoom expectations and accessibility requirements.
**Example:** Display title clamps 24→36px between 360 and 1200px viewport; body stays at the user's chosen size, growing only when they change it.

### R12. Underline is for links, not emphasis
**Rule:** Reserve underlines for hyperlinks; express emphasis with weight or color, and keep link styling distinct from all other text treatments.
**Why:** Users have learned underline = clickable; underlined non-link text creates false affordances, and unstyled links destroy the strongest inline navigation cue.
**Example:** A warning phrase gets weight 600, never an underline; every genuine link keeps its underline even inside dense body text.

### R13. Keep numbers and units together
**Rule:** Prevent line breaks inside numbers, between values and units, and between currency and amount; identifiers, dates, and phone numbers stay unbroken.
**Why:** Numeric tokens are read as single units — a value split across lines ("1 500 / USD") destroys scanability and can even change meaning, forcing users to re-read.
**Example:** "1 500 USD" wraps as a whole to the next line; the invoice number never breaks mid-digit even in the narrowest column.

## Checklist
- [ ] All text sizes come from one declared modular scale (4–6 steps); no ad-hoc sizes
- [ ] Body text is 16px-equivalent minimum; nothing users read for long is smaller
- [ ] Body line-height 1.4–1.6 (target 1.5); headings 1.2–1.3
- [ ] Every text container measures 45–75 characters per line
- [ ] ≤ 2 font families and ≤ 3 weights in the entire product
- [ ] UI strings in sentence case; caps only in small tracked overlines
- [ ] Numeric columns, timers, and counters use tabular figures
- [ ] Font stack has metric-adjusted fallbacks; layout survives font-not-loaded
- [ ] Text respects user font-size settings; display text scales fluidly where supported
- [ ] Underlines appear on links only

## Anti-patterns
- Ten-plus font sizes accrued across screens
- Long paragraphs at 80–120 characters per line
- Body line-height 1.1–1.2 "to fit more content"
- All-caps + bold + color applied together everywhere "for emphasis"
- Justified text without hyphenation (rivers)
- Webfont with no fallback and no metric compensation
- Underlining non-link text for decoration
- Light (300) weights for body text on screens

## Sources
- Material Design 3 — Typography: https://m3.material.io/styles/typography/overview
- Apple HIG — Typography: https://developer.apple.com/design/human-interface-guidelines/typography
- WCAG 2.2 — 1.4.4 Resize Text: https://www.w3.org/TR/WCAG22/
- Nielsen Norman Group — Line length online: https://www.nngroup.com/articles/line-length-online/
- Butterick's Practical Typography: https://practicaltypography.com/
- Web typography (Butterick): https://practicaltypography.com/web-typography.html
