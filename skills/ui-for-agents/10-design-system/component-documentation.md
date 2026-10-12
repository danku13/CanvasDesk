# Component Documentation

> **Read when:** documenting a kit component, releasing one, or when implementation and docs disagree. | **Section:** 10-design-system | **Related:** building-a-ui-kit.md, component-api-design.md, ../30-ux-patterns/microcopy.md, ../40-quality/visual-qa-protocol.md, ../GLOSSARY.md

**Core idea:** documentation doubles as acceptance criteria — a state not documented is a state not built. Write rules as testable statements, keep docs versioned with the component, and cover the edge cases that break components: longest string, empty, RTL, dark.

## Rules

### R1. Fixed page structure, same order everywhere
**Rule:** Every component page contains, in order: purpose (one line), anatomy (named parts), props/variants table, states matrix, do/don't examples, accessibility notes, content rules.
**Why:** A fixed structure makes pages scannable and comparable — consumers find "what is this / what parts / what props / what states / when not to use it" in the same place every time; freeform docs hide gaps.
**Example:** The Button page opens: purpose, anatomy (container, label, icon), props table, 6-state matrix, do/don't, keyboard map, label rules.

### R2. Purpose in one line, decision-oriented
**Rule:** Start with one sentence stating what the component is for and when to choose it — never a description of what it looks like.
**Why:** Consumers pick components by intent; appearance-first descriptions force them to reverse-engineer usage from pixels, which is exactly where lookalike misuse begins.
**Example:** "Use Toast for transient confirmation; use Dialog when a decision is required." — not "A small rounded box that appears in the corner."

### R3. Anatomy = named parts
**Rule:** Document the component as named parts (container, label, icon, badge, addon) and use those names consistently in props, states, and QA notes.
**Why:** Named anatomy is the shared vocabulary for everything else — a "label" prop needs an anatomy "label" to be unambiguous, and QA reports reference parts instead of guesses.
**Example:** Input anatomy: container, prefix, field, suffix, label, help-text, error; docs state "error replaces the help-text slot" — one meaning, no debate.

### R4. The states matrix is complete and rendered
**Rule:** Document the full states matrix — default / hover / focus / active / disabled / loading, plus empty and error where content is involved — with a rendered example of each cell.
**Why:** The matrix is the implementation contract for both builders and QA; text-only state lists get skipped during build because nothing forces them to exist visually.
**Example:** Button matrix rendered as six labeled screenshots per variant; the loading cell shows spinner size and label behavior.

### R5. Do/don't pairs, each don't with a one-line reason
**Rule:** Every usage rule appears as a do/don't pair with a one-line reason, and the don't shows the actual broken rendering — not a description of it.
**Why:** Contrastive examples teach the boundary precisely, and the reason lets agents extrapolate to unlisted cases; a don't without the visual gets re-violated because it looks plausible in code.
**Example:** Do: one primary button per view. Don't: two filled buttons side by side — "competing primaries halve CTA clarity."

### R6. Accessibility notes: keyboard map, roles, announcements
**Rule:** Document the keyboard map key by key, the roles and accessible names, and the expected assistive-tech announcements (on focus, on change, on error).
**Why:** A11y behavior is contract for consumers and QA; announcements are the most common silent breakage and can only be verified against a documented expectation (see ../40-quality/accessibility-wcag.md).
**Example:** Checkbox: "Space toggles; role checkbox; announces 'checked'/'unchecked' + label; errors announced via the label association, never by color alone."

### R7. Content rules as testable statements
**Rule:** Write content and microcopy constraints as measurable statements ("label ≤ 3 words", "error names the fix, ≤ 80 chars"), not as prose guidance (see ../30-ux-patterns/microcopy.md).
**Why:** Testable statements can be verified by review, lint, or an agent in seconds; adjectives like "concise" cannot be checked and therefore never are.
**Example:** "Button label: verb + object, ≤ 3 words, no trailing punctuation" — a reviewer can pass/fail it on sight.

### R8. Edge-case examples: longest string, empty, RTL, dark
**Rule:** Include rendered examples for the longest expected content, empty content, RTL direction, and the dark theme — for every component.
**Why:** These four break components in ways default examples never reveal (truncation, collapse, mirroring, contrast), and each one discovered in production costs more than the example would have.
**Example:** UserCard shown with a 64-character name (truncation + full-content access rule), with its empty state, mirrored RTL, and in dark tokens.

### R9. Docs are the acceptance criteria
**Rule:** Treat the documentation page as the implementation's acceptance criteria: a state, variant, or behavior not documented is not built — and anything built but undocumented is removed or documented in the same change.
**Why:** Docs-as-criteria closes the gap where "extra" behavior ships unreviewed, and it makes review and visual QA objective: compare the build against the page, not against memory (see ../40-quality/visual-qa-protocol.md).
**Example:** A PR adds a "warning" button variant absent from docs → rejected: document it (states, do/don't, content rules) or remove it.

### R10. Single source of truth, versioned with the component
**Rule:** Docs live with the component version and update in the same change; a version's docs describe exactly that version.
**Why:** Separately maintained docs drift immediately and are worse than none — consumers trust stale pages; version-locked docs make upgrades verifiable ("what changed for me?").
**Example:** The 2.1 changelog entry "Button: added loading state" ships in the same PR as the loading section on the 2.1 docs page.

### R11. Examples use tokens and realistic content
**Rule:** Documentation examples use semantic tokens and realistic (but safe) content; raw values and placeholder filler are rejected.
**Why:** Examples are the most-copied code in a library — a raw hex in an example legitimizes bypassing tokens; lorem ipsum hides content-rule violations that real-shaped text would expose.
**Example:** The example renders `bg-surface` / `text-primary` with the label "Save changes"; `#3b82f6` and "Lorem ipsum" fail review.

### R12. Cross-link shared rules, don't restate them
**Rule:** Link to shared guidance (microcopy, accessibility, motion) instead of re-deriving it on the component page; the page states only what is specific to this component.
**Why:** Duplicated guidance diverges — when the shared rule changes (e.g. pointer target 24×24 px), restatements go stale and contradict the source; a link always resolves to the current rule.
**Example:** The Button page links to motion principles for hover timing instead of re-printing duration numbers.

### R13. State the scope boundaries
**Rule:** Each page states what the component deliberately does not do — the adjacent cases that belong to another component or pattern.
**Why:** Most misuse is boundary confusion ("should this be a Toast or a Dialog?"); an explicit boundary turns a wrong pick into a fast redirect instead of a review cycle.
**Example:** "Toast does not ask for decisions — for confirmations, see Dialog. Toast cannot be pinned — for persistent notices, see the banner pattern."

## Checklist
- [ ] Page has all seven sections in the fixed order
- [ ] Purpose fits one line and states when to choose the component
- [ ] Anatomy parts named; names used consistently across the page
- [ ] States matrix fully rendered (default → loading, plus empty/error)
- [ ] Every don't shown visually with a one-line reason
- [ ] Keyboard map + expected assistive-tech announcements written
- [ ] Content rules are testable statements (numbers, not adjectives)
- [ ] Longest-string / empty / RTL / dark examples present
- [ ] Docs updated in the same change as the component
- [ ] No raw values or placeholder filler in examples

## Anti-patterns
- "See the code for details" as documentation
- A props table without the states matrix
- Don'ts described in words only ("don't make it too colorful")
- Screenshots in light theme only
- Docs in a separate wiki, updated "when there's time"
- Lorem ipsum in usage examples
- A shipped variant that exists nowhere in the docs
- Re-stating global a11y rules per page until they contradict the source

## Sources
- Atlassian Design System — component usage guidelines model: https://atlassian.design/components
- Shopify Polaris — do/don't component documentation: https://polaris.shopify.com/components
- IBM Carbon — component anatomy and usage docs: https://carbondesignsystem.com/components/overview/
- Material Design 3 — component specs (anatomy, states): https://m3.material.io/components
- WAI-ARIA Authoring Practices — keyboard maps per pattern: https://www.w3.org/WAI/ARIA/apg/
- GitHub Primer — component documentation structure: https://primer.style/components
- Storybook — docs as part of the component workflow: https://storybook.js.org/docs
