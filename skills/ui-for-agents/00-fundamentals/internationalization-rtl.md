# Internationalization & RTL

> **Read when:** the product targets more than one language, locale, or script — especially any right-to-left language (Arabic, Hebrew, Persian, Urdu). **Section:** 00-fundamentals. **Related:** 00/typography.md, 00/spacing-layout-grids.md, 10/design-tokens.md, 30/microcopy.md, 40/accessibility-wcag.md, 50/cross-platform-parity.md

**Core idea:** locale is a dimension of the design, not a late translation pass. A layout survives direction switching and text expansion only if it was built with logical structure, flexible containers, and locale-delegated formatting from day one.

This file defines the universal laws that hold in every renderer. CSS-specific mechanics (logical properties, `dir` attributes) live in `50-platforms/web-css-dom.md`; token naming for direction-dependent values lives in `10-design-system/design-tokens.md`.

## Rules

### R1. Think in logical axes, not physical directions
**Rule:** Express every layout decision as start/end and inline/block (reading direction and flow direction), never as left/right or top/bottom — in specs, tokens, and code alike.
**Why:** A logically defined layout re-mirrors automatically when direction flips; a physically defined one must be hand-mirrored, and every hand mirror leaks. This is the single decision from which all RTL correctness follows.
**Example:** "Back-arrow at the inline-start of the header, content padding-inline 16" survives Arabic; "arrow left, padding-left 16" breaks it.

### R2. Mirror the layout and chrome in RTL — not everything
**Rule:** In RTL contexts mirror structural placement (navigation order, text alignment, progress direction, directional icons like back/next) — but keep orientation-significant content unmirrored: media player controls, clocks, time-axis charts, phone numbers, code snippets, math.
**Why:** The reader's mental model of the page flips, but the physical world does not; mirroring a play button or a time axis inverts meaning instead of direction.
**Example:** Arabic music app: header, tabs, and carousel arrows mirror; the play/pause/skip controls and the track-progress direction stay LTR.

### R3. Isolate bidirectional runs
**Rule:** User-generated content and embedded foreign-direction fragments (a Latin product name inside Arabic sentence, a number in a Hebrew list) must be direction-isolated so punctuation and truncation stay on the correct side.
**Why:** The Unicode bidi algorithm resolves mixed runs at the paragraph level by default; unisolated fragments scatter their punctuation across the line and corrupt sentence structure.
**Example:** "مبيعات iPhone 15 — الأفضل" without isolation renders the dash and trailing word out of place; an isolated run keeps "iPhone 15" as one unit.

### R4. Design containers for ±30% text expansion
**Rule:** Never fix the width of anything that holds text (buttons, tabs, nav items, labels); use min-width or content-plus-padding, and reserve room for text 1.3–2× the English length (German ~+35%, Russian ~+15–20%, Finnish ~+30–100% depending on context).
**Why:** Fixed-width text containers truncate or wrap in translation, and truncation of navigation labels or CTAs breaks task completion — the layout contract must absorb expansion, not fight it.
**Example:** A "Save" button specified as `min-width: 96px; padding: 0 16px` renders "Speichern" and "Сохранить" fully; a fixed 64px button ellipsizes them.

### R5. Format data in the locale of the user
**Rule:** Dates, times, numbers, currencies, units, addresses, and phone numbers are rendered by the locale layer (ICU/Intl), never by string templates or hardcoded separators; the comparison window and timezone are explicit.
**Why:** "03/04/2024" is March 4 in en-US and April 3 in de-DE; a wrong € / $ position or dot-vs-comma decimal changes the perceived magnitude of financial values.
**Example:** €48,200.50 (en) renders as 48 200,50 € (de) and ٤٨٬٢٠٠٫٥٠ or 48,200.50 € (ar) — same data, one formatting source.

### R6. Set typography per script, not per product
**Rule:** Line-height, letter-spacing, font stack, and case transforms are script-aware: Arabic and Devanagari need taller line-height (deep ascenders/descenders), CJK takes no letter-spacing and no synthetic italics, `text-transform: uppercase` is meaningless or harmful outside bicameral scripts, and bold is faked where the family has no bold cut for the script.
**Why:** Latin-tuned metrics clip or suffocate other scripts — Arabic with line-height 1.2 collides diacritics; letterspaced CJK destroys word boundaries (CJK does not use spaces).
**Example:** Arabic body text: line-height 1.7–1.9 and a naskh-family cut; Japanese body: line-height 1.6–1.8, no uppercase, no italic, `font-family` resolves through a CJK stack.

### R7. Strings live in resources; never concatenate
**Rule:** Every visible string is a complete resource-key lookup with placeholders; never build sentences by joining fragments, and never render text inside images or icons.
**Why:** Word order and grammar differ across languages (a German adjective moves, a Japanese sentence ends with the verb), so concatenated fragments produce word-salad in most locales, and baked-in image text cannot be translated or scaled.
**Example:** "3 files uploaded" is one ICU message with a plural rule — not `"3" + " files" + " uploaded"`; a banner illustration contains no slogan text.

### R8. Language and direction are declared metadata
**Rule:** The active language and direction are declared once at the root (of the document or window) and inherited — never inferred per element or hardcoded per screen; a language switch flips direction at the root only.
**Why:** Per-element guessing scatters exceptions, while the bidi algorithm needs paragraph-level direction to resolve anything; screen readers also announce pronunciation from the declared language, so missing metadata breaks reading aloud.
**Example:** `<html lang="ar" dir="rtl">` — every descendant inherits; one embedded English chart gets `lang="en" dir="ltr"` at its own root, nothing else changes.

### R9. Verify cultural signals per target locale
**Rule:** Icons, color meanings, gestures, illustration content, and name/address field structures are validated per locale — flags never serve as language selectors, name fields never force First/Last order, and status colors keep their non-color signals (see 00/color.md R9).
**Why:** A "thumbs-up", an owl, white garments, or a red envelope carry opposite meanings somewhere; a mandatory "Last name" field fails most of Asia; flag-as-language maps languages to countries that host but do not speak them.
**Example:** A Japanese release swaps the hand-gesture success icon for a neutral check; the checkout replaces First/Last with a single full-name field whose order the locale decides.

## Checklist
- [ ] Zero physical left/right in layout code and tokens — everything is start/end, inline/block
- [ ] RTL pass done: chrome and navigation mirrored; media controls, clocks, time axes, code left unmirrored
- [ ] All mixed-direction content isolated (names, numbers, foreign fragments) and punctuation lands correctly
- [ ] No fixed-width text containers; tested with German (+35%) and Russian (+20%) strings
- [ ] Dates, numbers, currency, units rendered through the locale layer; timezone and comparison window explicit
- [ ] Per-script typography verified: Arabic line-height ≥ 1.6, CJK without letter-spacing or fake italics, no blind uppercase
- [ ] All strings are resource lookups with placeholders; zero concatenated sentences; zero text baked into images
- [ ] Language and direction declared at the root; language switch flips direction exactly once
- [ ] Locale-sensitive review done for icons, colors, illustrations, and name/address fields
- [ ] Screen reader announces the page in the declared language; RTL navigation order is logical

## Anti-patterns
- "We'll add RTL later" — direction built into components from the first screen, or it never lands
- Hand-mirrored CSS scattered per component instead of logical properties at the layout layer
- Fixed-width buttons and tabs that ellipsize in German
- Flag icons as language switchers (en-GB ≠ flag; Spanish ≠ Spain alone)
- Hardcoded `MM/DD/YYYY` parsing or display
- letter-spacing on CJK body text "for style"
- First/Last name as required separate Latin-only fields
- Text inside hero illustrations that no translation can reach

## Sources
- W3C — RTL / bidirectional text guidance: https://www.w3.org/International/questions/qa-rtl
- MDN — CSS logical properties and values: https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_logical_properties_and_values
- W3C — Typography & internationalization per script: https://www.w3.org/International/i18n-drafts/
- Apple HIG — Internationalization: https://developer.apple.com/design/human-interface-guidelines/internationalization
- Material Design 3 — Localization & internationalization guidance: https://m3.material.io/foundations
- WCAG 2.2 — 3.1.1 Language of Page / 3.1.2 Language of Parts: https://www.w3.org/TR/WCAG22/
- Unicode Bidi Algorithm (UAX #9): https://unicode.org/reports/tr9/
