# Designing a Style (the process)

> **Read when:** a product needs a distinct visual identity, or you must invent a custom style preset. **Section:** 60-styles. **Related:** style-catalog.md, 10/design-tokens.md, 00/visual-hierarchy.md, 00/color.md, 00/typography.md, 00/motion-principles.md

**Core idea:** a style is not decoration — it is a consistent token preset plus a few deliberately exaggerated signature decisions, applied everywhere and never broken. Distinctiveness comes from exaggerating 2–3 dimensions; recognizability comes from never deviating.

## Rules

### R1. Start with 3–5 personality adjectives, not visuals
**Rule:** Before touching color or fonts, agree on 3–5 adjectives the style must express ("calm, precise, trustworthy" or "loud, young, chaotic").
**Why:** Adjectives are testable against every later decision ("does this harsh border say *calm*?"), while raw visual choices are infinite; adjectives also survive platform and framework changes.
**Example:** "banking for teenagers" → adjectives: friendly, fast, honest → later rules reject muted-gray minimalism (not friendly) and neon-glow cyber (not honest).

### R2. A style = token preset + signature components, nothing else
**Rule:** Implement a style as (a) a complete token preset — every scale remapped — and (b) 1–3 signature components or elements that carry the identity. No per-screen ad-hoc styling.
**Why:** The token preset guarantees consistency on every screen automatically; signature elements give instant recognition; anything beyond these two channels produces drift and unmaintainable one-offs.
**Example:** "dark premium" = token preset (near-black surfaces, gold accent, serif display) + two signatures: gold hairline dividers and serif numerals in KPI cards.

### R3. Exaggerate 2–3 dimensions, keep the rest neutral
**Rule:** Push at most 2–3 of the eight dimensions — color, typography, shape, elevation, density, motion, iconography, imagery — toward an extreme; leave everything else at library defaults.
**Why:** Everything-exaggerated reads as noise and multiplies QA surface; 2–3 extremes against a neutral base is precisely what makes a style readable, recognizable, and maintainable.
**Example:** Brutalism exaggerates shape (0 radius, thick borders) and typography (mono, oversized); its spacing and motion stay conventional — which is why it still looks coherent.

### R4. The accessibility floor is style-invariant
**Rule:** A style may remap colors, radius, and shadows — it may never go below the floors: text contrast 4.5:1, UI components 3:1, pointer targets 24×24 px, visible focus.
**Why:** Per AGENT-WORKFLOW conflict order, accessibility outranks aesthetics; a beautiful style that excludes users is a defect, and retro-fitting contrast later means re-doing the palette.
**Example:** Glassmorphism presets must include a scrim token behind text-on-blur; Neumorphism must compensate its low-contrast shadows with borders and strong state changes.

### R5. The signature must survive the grayscale test
**Rule:** Convert a screenshot to grayscale: the style must still be recognizable (shape, spacing, typography), and the primary action still findable.
**Why:** If recognition depends on hue alone, the style is fragile for color-blind users and breaks across themes; form — not color — must carry the identity.
**Example:** Editorial style keeps its identity through serif display scale and hairline rules even with color removed.

### R6. Define the motion personality inside the preset
**Rule:** Every style preset specifies a duration band, easing set, and what may or may not animate (e.g. brutalism: 0–100 ms or none; playful: 200–400 ms with overshoot).
**Why:** Motion is a personality channel users feel but cannot name; leaving it undefined lets every screen default to the platform and quietly dilutes the style.
**Example:** A premium style: 250–350 ms ease-out, no bounce; a playful style: spring overshoot on toggles and toasts.

### R7. One style per product surface; mixing needs a documented boundary
**Rule:** Choose one base style per surface; if a second style appears (marketing site vs application), document where the boundary runs and keep token families shared.
**Why:** Unmanaged mixing reads as inconsistency and erodes trust; managed contrast between expressive marketing and calm product UI is a legitimate, common pattern.
**Example:** An editorial marketing site + minimalist-swiss app sharing the same type family and accent, differing in density and imagery.

### R8. Write the style down as a reproducible preset
**Rule:** Finish by producing a style file in this section's format: personality adjectives, signature list, full token table, motion spec, a11y watchpoints.
**Why:** An undocumented style dies with the session; a preset makes the style reproducible by any agent or human, reviewable, and diffable when it evolves.
**Example:** A new "aquatic calm" style ships as a one-page preset plus token values — not as a mood board or a screenshot.

### R9. Borrow movements honestly: name influences, keep their logic
**Rule:** When a preset adapts an existing movement (Swiss, brutalism, glassmorphism), name it in the file and keep its characteristic internal constraints — not just its surface effects.
**Why:** Movements are coherent systems; taking only the surface (glass gradients without blur discipline, neon without the dark base) produces pastiche that reads cheap.
**Example:** A neo-brutalist preset keeps raw borders and the "honest UI" stance — not merely harsh colors on rounded cards.

## Checklist
- [ ] 3–5 personality adjectives written down and driving decisions
- [ ] 2–3 exaggerated dimensions chosen; all other dimensions at library defaults
- [ ] Full token preset produced (color, type, space, radius, shadow, motion, z-layers)
- [ ] 1–3 signature components defined and implemented once in the kit
- [ ] All contrast pairs ≥ 4.5:1 / 3:1 in the preset, in both themes
- [ ] Grayscale test: style recognizable, primary action findable
- [ ] Motion personality documented: duration band + easing + allowed animations
- [ ] Style preset documented in the 60-styles file format
- [ ] If surfaces mix styles: shared token families + written boundary

## Anti-patterns
- Picking a palette before picking adjectives
- Exaggerating every dimension at once ("everything must be unique")
- Applying the style via per-screen overrides instead of tokens
- Dropping contrast for aesthetics without a documented deviation
- A style that disappears in grayscale or in dark mode
- Two competing styles inside one application surface
- Copying a movement's surface effects without its logic
- A style that exists only in a designer's head or a mood board

## Sources
- Refactoring UI — systematic visual decisions: https://www.refactoringui.com
- Laws of UX — aesthetic-usability and consistency effects: https://lawsofux.com
- Material Design 3 — dynamic color & shape as system dimensions: https://m3.material.io/foundations
- Nielsen Norman Group — consistency and trust: https://www.nngroup.com/articles/consistency-and-standards/
