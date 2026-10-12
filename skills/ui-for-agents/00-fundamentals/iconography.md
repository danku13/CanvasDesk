# Iconography

> **Read when:** adding, sizing, pairing, or auditing any icon in a UI. **Section:** 00-fundamentals. **Related:** 00/visual-hierarchy.md, 00/typography.md, 00/color.md, 40/accessibility-wcag.md

**Core idea:** icons compress known meanings into small glyphs. They work only when the metaphor is established, the family is consistent, and meaning never depends on the glyph alone.

Rules here apply equally to SVG sets, icon fonts, and glyphs drawn by a canvas or native renderer.

## Rules

### R1. One icon family per product
**Rule:** Use a single icon set with consistent stroke weight, corner radius, and filling style; never mix families in one view.
**Why:** Mixed families violate the "consistent look = consistent meaning" contract — differing stroke weights read as differing importance or as different eras of the product, and icon semantics stop being trusted. See `00-fundamentals/visual-hierarchy.md` R6.
**Example:** All icons 1.5px stroke, rounded joins, on the 24px grid. Importing a second set for "just one more icon" is a defect — redraw it or pick an equivalent from the primary set.

### R2. Design on a 24px grid with keyline shapes
**Rule:** Author icons on a 24px grid using standard keyline shapes (circle, square, rectangle, orthogonals) so optical weight matches across the set.
**Why:** Keylines compensate for optical illusions — a raw circle and square at equal pixel bounds look unequal; keyline discipline makes any row of glyphs feel calibrated.
**Example:** Home, search, and settings in a nav bar: the roofline hits the horizontal keyline, the lens touches the circular keyline — all read as the same visual size.

### R3. Icons never carry meaning alone
**Rule:** Every meaningful icon has a text label, a tooltip on hover/focus, and an accessible name; if the meaning is not near-universal, show the label permanently.
**Why:** Icon-only meaning fails for screen-reader users, first-time users, and ambiguous metaphors; the accessible name is also what assistive tech and voice control address. See `40-quality/accessibility-wcag.md`.
**Example:** A trash icon button carries tooltip "Delete" and accessible name "Delete item"; a hamburger menu is borderline-universal at best — label it "Menu" or add text.

### R4. Touch target is independent of visual size
**Rule:** The interactive area of an icon button is at least 24×24px (44×44px recommended for touch), regardless of the glyph's rendered size.
**Why:** Target size is a motor constraint, visual size a typographic one — they must be decoupled; small glyphs with equally small hit areas cause mis-taps and rage clicks.
**Example:** A 16px "close" glyph inside a 44×44px button with a centered hit area and no overlapping neighbors.

### R5. Use established metaphors
**Rule:** Pick the conventional glyph for the action — trash = delete, magnifier = search, gear = settings, envelope = message; invent icons only for genuinely novel concepts, and label those permanently.
**Why:** Icons are read by recognition, not instruction — a novel metaphor costs every user a guess, while established ones cost nothing; the floppy-disk "save" survives precisely because it is learned convention, not realism.
**Example:** A search field gets a magnifier, not a telescope; a floppy "save" is legacy-acceptable; a novel "fork this document" icon always ships with a label.

### R6. Don't mix filled and outline arbitrarily
**Rule:** Within one family, filled and outline variants encode a rule (state or category) — never a per-icon taste choice.
**Why:** Fill is a weight/emphasis signal; random mixing reads as noise and destroys the fill channel for state (e.g. filled = active/selected).
**Example:** Navigation rail: outline icons by default, filled when the destination is active — the fill flip is the selection signal.

### R7. Icon size pairs with adjacent text size
**Rule:** Match icon size to the text it accompanies (body 16px → ~20px icon; title 24px → 24–28px icon) and keep one declared pairing table for the product.
**Why:** Icons sized independently of text break line rhythm and optical centering; a declared pairing keeps every icon-text lockup a single visual unit.
**Example:** A 16px menu label pairs with a 20px icon; the same glyph in a 24px heading scales to 24–28px — never a one-size-fits-all 16px icon everywhere.

### R8. Verify legibility at the minimum rendered size
**Rule:** Test every icon at its smallest real size (typically 16px) — the silhouette must stay recognizable; simplify the glyph or add a label when it doesn't.
**Why:** Detail designed at 64px dies at 16px: strokes merge and shapes blur, leaving only silhouette features; dense icons become noise dots.
**Example:** A document-with-fine-lines icon unreadable at 16px becomes a plain page outline; a dense chart legend glyph is reduced to a single-shape mark.

### R9. One glyph per concept, everywhere
**Rule:** The same concept always uses the same glyph in the same style; never ship synonyms (two different "settings" gears, two "share" arrows) or reuse one glyph for two concepts.
**Why:** Users learn icon semantics once per product; synonyms force re-recognition and double meanings force guessing — both erode the trust that makes icons faster than labels.
**Example:** Audit result: 3 different "user" icons across screens collapse to one; the bookmark glyph used for both "save" and "flag" gets a dedicated flag glyph.

### R10. Mark decorative icons as decorative
**Rule:** Icons that carry no information (pure ornament, illustrations next to already-labeled text) are excluded from the accessibility tree and don't respond to input.
**Why:** Redundant icons in the accessibility tree force screen-reader users to listen to noise ("image, image, image") before content; decorative and semantic icons must be distinguishable to machines.
**Example:** A icon-only button's glyph is announced ("Delete"); the same glyph repeated as section ornament is hidden from assistive tech and ignores clicks.

### R11. Declare the icon-to-label gap
**Rule:** The gap between an icon and its adjacent label is one declared scale value (typically 8px), applied to every icon-text lockup in the product.
**Why:** Variable gaps make lockups sloppy and break vertical rhythm across screens; one declared value keeps icon and text reading as a single unit everywhere.
**Example:** Menu items, buttons, and chips all separate icon and label by 8px — a button never hand-tunes its gap to 6px.

### R12. Every icon must earn its place
**Rule:** Ship an icon only when it is a control, a status signal, or a recognized navigation aid — never as heading decoration or "visual interest".
**Why:** Icons are meaning carriers; decoration dilutes the channel until users stop scanning icons at all, and each extra glyph competes with the real controls.
**Example:** A card header with a settings gear (control) is fine; the same header with three ornamental icons around the title is noise — remove them.

## Checklist
- [ ] One icon family everywhere; stroke weight, corners, and style identical in any row of icons
- [ ] Icons authored on the declared 24px grid with keylines; mixed rows read as equal optical weight
- [ ] Every meaningful icon has a label and/or tooltip AND an accessible name
- [ ] All icon hit areas ≥ 24×24px (44×44px for touch-primary UIs)
- [ ] No invented metaphors for standard actions (delete, search, settings, close, share)
- [ ] Filled/outline usage follows one declared rule (e.g. filled = active), not per-icon choice
- [ ] Icon sizes follow the declared pairing table with adjacent text sizes
- [ ] Every icon verified recognizable at 16px
- [ ] One glyph per concept across the whole product (synonym audit done)
- [ ] Decorative icons excluded from the accessibility tree

## Anti-patterns
- Icons from three different sets in one toolbar
- Icon-only buttons with no accessible name ("the X button")
- 12px hit areas on 16px glyphs
- A "creative" custom icon for save/delete/search
- Random filled/outline mixing driven by taste
- The same glyph meaning two things on different screens
- Decorative icons on every heading, diluting real controls
- Detailed illustrations dropped to 16px and left unreadable

## Sources
- Material Design 3 — Icon styles & system icons: https://m3.material.io/styles/icons/overview
- Apple HIG — Icons: https://developer.apple.com/design/human-interface-guidelines/icons
- WCAG 2.2 — 1.1.1 Non-text Content & 2.5.8 Target Size (Minimum): https://www.w3.org/TR/WCAG22/
- Nielsen Norman Group — Icon usability: https://www.nngroup.com/articles/icon-usability/
- Material Design (v2) — System icons & keyline shapes: https://m2.material.io/design/iconography/system-icons.html
- Material Symbols — icon library & variable-axis guidance: https://fonts.google.com/icons
- Microsoft Fluent 2 — Iconography: https://fluent2.microsoft.design/iconography
