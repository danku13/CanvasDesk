# Style: Flat Design

> **Read when:** building UI in the flat-design style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/visual-hierarchy.md, 00/typography.md, 00/color.md, 10/design-tokens.md

**Personality:** clean, efficient, digital-native, honest. **Best for:** the default paradigm of modern product UI — dashboards, productivity tools, icons, anything wanting era-neutral clarity. **Avoid for:** tactile hardware-flavored products, luxury storytelling, content-led editorial experiences.

**Core idea:** remove all faux-realism — no gradients, bevels, or textures; meaning is carried entirely by color blocks, simple geometry, and typography (Metro's typographic hierarchy plus iOS 7's light thiness). Flat 2.0 (2014+) quietly reintroduced one subtle shadow layer when pure flat proved too ambiguous.

Lineage note: flat design is Swiss principles applied to screens — the historical movement that replaced skeuomorphism. minimalist-swiss.md documents its print-lineage sibling (hairline borders, grid discipline, one accent); this file covers the screen paradigm and its hard-won lessons.

## Signature (what makes it recognizable)
- Pure flat color fills; v1 bans gradients, shadows, and textures outright
- Metro typographic hierarchy — type IS the layout, on a tile grid
- Thin/light type at very large sizes (iOS 7)
- Ghost buttons: transparent fill + thin border
- Single-weight geometric line icons, grid-aligned
- Long-shadow trend artifact of the Flat 2.0 era
- Flat 2.0: one subtle shadow layer and gentle gradients return

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | none to small | 0 px (Metro) – 4 px (iOS lineage) |
| `shadow-*` | banned (v1) / one layer (2.0) | none, or a single 0/2/8 px at 10–15% for overlays |
| `space-scale` | tile-driven | 8 pt multiples; Metro gutter rhythm |
| `color-bg / surface` | white / near-white | #FFFFFF / #F2F2F2 |
| `color-ink` | near-black, regular weight | #111–#333, 400+ weight |
| `color-accent` | saturated flat hue | system-hue families; 4.5:1 for text use |
| `type-display` | thin/light, large | 300 weight, 32–56 px, tight tracking |
| `type-body` | regular and above | 16 px / 400 minimum; thin banned |
| `icon-weight` | single stroke | 1.5–2 px stroke on a 24 px grid |
| `elevation-budget` | flat 2.0 | one shadow layer, true overlays only |
| `motion-duration` | functional | 200–300 ms, ease-out |

## Rules

### R1. Color + typography carry ALL hierarchy
**Rule:** With depth removed, hierarchy comes from size and weight contrast STRONGER than the skeuomorphic era — titles one scale step and one weight apart from body, muted text used sparingly.
**Why:** The documented flat failure is same-weight gray text everywhere; without bevels to lean on, weak type contrast makes whole screens unreadable.
**Example:** Card: title 20/600 ink, body 16/400 ink at 80% — no gray-on-gray middle tier.

### R2. The thin-weight lesson: weight floors
**Rule:** Light/thin typefaces are display-only at ≥ 18 px with generous spacing; body text uses regular (400) or heavier at every size.
**Why:** iOS 7's ultra-thin body text is the movement's legibility disaster — thin strokes alias away at low DPI and low vision.
**Example:** Hero headline 48/200; the paragraph under it 16/400 — never 16/200.

### R3. Ghost buttons earn their border
**Rule:** A transparent button needs a border at ≥ 3:1 against its fill AND a filled hover/active state; the primary action of any view is never a ghost.
**Why:** Documented discoverability failure — weak ghosts get skipped; the filled active state is what proves they are buttons.
**Example:** Secondary "Cancel": 1 px border at 3:1, fills with surface tint on hover; primary "Save" is always solid.

### R4. Color blocks are semantic
**Rule:** Every flat fill maps to a role (action, surface, accent, status); decorative color blocks with no function are banned.
**Why:** In a style without texture, color is the scarcest signal — decorative fills spend it and teach users to ignore it.
**Example:** Blue only on links and actions, green only on success; the hero banner reuses `color-surface`, not a new decorative hue.

### R5. Flat 2.0 elevation budget
**Rule:** At most ONE subtle shadow layer, reserved for true overlays (dropdowns, modals, toasts); nothing at rest floats.
**Why:** This is the movement's own fix for pure-flat ambiguity — cards became indistinguishable from background — while keeping the honest flat default.
**Example:** Menu gets the single 0/8 px shadow; dashboard cards stay flat with a 1 px border instead.

### R6. Icons: single-weight, geometric, grid-aligned
**Rule:** All icons share one stroke weight (1.5–2 px on a 24 px grid), geometric construction; filled and outline never mix within a set.
**Why:** Icon weight reads as emphasis; mixed weights and skeuomorphic remnants re-introduce the noise flat design removed.
**Example:** Toolbar of five 2 px-stroke line icons, same rounded caps, snapped to the 8 pt grid.

### R7. Motion is functional and quick
**Rule:** 200–300 ms ease-out for state changes and transitions; no bounce, no parallax, no decorative loops.
**Why:** Flat UI reads as machinery — motion confirms causality; showy motion contradicts the style's honest, digital-native premise.
**Example:** Tile press scales 0.98 at 200 ms ease-out; panel slides 240 ms with a fade.

### R8. Flattening is not deleting
**Rule:** When simplifying a textured or skeuomorphic UI, re-establish every affordance explicitly — borders, labels, color roles, target sizes — before removing the old cues; never just strip the shadows.
**Why:** The iOS 6-to-7 lesson: removing depth without redesigning affordances made buttons unrecognizable; affordances must be replaced, not deleted.
**Example:** Old beveled button becomes solid accent + label + 44 px target — affordance moved to color role and size, not left to memory.

## A11y watchpoints
- The thin-type disaster IS the accessibility lesson: enforce weight floors (body ≥ 400) and check legibility at low DPI
- Color-only hierarchy fails: pair every color signal with type, shape, or position (WCAG 1.4.1)
- Ghost buttons: verify the border at 3:1 and make focus/hover states filled, not border tweaks
- Flat saturated fills under white text are classic mid-saturation 4.5:1 failures — verify every pair, both themes
- Focus cannot borrow elevation cues on flat surfaces — dedicated 2 px outline at 3:1, always visible

## Checklist
- [ ] Hierarchy from size/weight contrast; no same-weight gray tiers
- [ ] Body text ≥ 400 weight at all sizes; thin is display-only ≥ 18 px
- [ ] Ghost buttons have 3:1 borders + filled active states; primary action is solid
- [ ] Every flat fill maps to a semantic role
- [ ] Exactly one shadow layer, overlays only (or zero for pure v1)
- [ ] Icons single-weight, geometric, grid-aligned
- [ ] Motion 200–300 ms ease-out, functional only
- [ ] All text pairs 4.5:1, UI boundaries 3:1, both themes
- [ ] Focus ring explicit on all flat surfaces
- [ ] Flattened affordances re-established (label + color role + target size)

## Anti-patterns
- Same-weight gray text everywhere (the flat hierarchy failure)
- Thin weights for body text or small labels
- Ghost buttons as the primary action
- Long shadows on everything (dated Flat 2.0 artifact)
- Decorative color blocks with no role
- Mixed filled and outline icon sets
- "Flattening" by deleting shadows without re-adding affordances
- Gradients and textures sneaking back "just a little" outside the 2.0 budget

## Sources & inspiration
- Flat design — movement history including Flat 2.0: https://en.wikipedia.org/wiki/Flat_design
- Metro (design language) — typographic hierarchy and tile grids: https://en.wikipedia.org/wiki/Metro_(design_language)
- iOS 7 — the flattening of iOS and its legibility fallout: https://en.wikipedia.org/wiki/IOS_7
- NN/g — flat UI's discoverability costs: https://www.nngroup.com/articles/flat-design/
- NN/g — ghost buttons analysis: https://www.nngroup.com/articles/ghost-buttons/
- International Typographic Style — the print lineage flat design inherits: https://en.wikipedia.org/wiki/International_Typographic_Style
