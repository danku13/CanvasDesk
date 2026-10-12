# Design Tokens

> **Read when:** starting any UI implementation, defining a kit's styling, or before any theming work. | **Section:** 10-design-system | **Related:** theming-dark-mode.md, building-a-ui-kit.md, ../00-fundamentals/color.md, ../00-fundamentals/spacing-layout-grids.md, ../GLOSSARY.md

**Core idea:** tokens are the contract between design and code — named, machine-readable decisions in three tiers. Name by role, never by value; components consume only semantic and component tokens; themes remap the semantic tier and nothing else.

## Rules

### R1. Keep the three-tier architecture intact
**Rule:** Structure tokens in exactly three tiers: primitives (raw values: `blue-500`, `space-4`), semantic (role-named: `bg-surface`, `text-primary`, `space-inset-md`), component (component-owned: `button-bg`, `input-border-focus`).
**Why:** Each tier changes at a different rate — palettes change rarely, roles change with brand and theme, component tokens with the component. Merging tiers couples every component to raw values and turns one rebrand into a repo-wide search-and-replace.
**Example:** `--blue-600` → `--color-accent` → `--button-primary-bg`; a rebrand edits only the middle link, zero component files.

### R2. Name by role, never by value
**Rule:** Semantic and component token names describe the job (`text-danger`, `bg-surface-raised`), never the appearance (`red-text`, `light-grey-bg`).
**Why:** A value name lies the first time the value changes — "red-error" that becomes orange poisons every decision made from the name afterwards. Role names survive re-theming, palette refreshes, and dark mode unchanged.
**Example:** Brand switches blue→green: `--color-accent` keeps its name and meaning; a token named `--blue` breaks every consumer's mental model.

### R3. Components may reference only semantic or component tokens
**Rule:** Component styles consume semantic or component tokens — never primitives, never raw values. Enforce with lint or a review checklist.
**Why:** Primitives are palette, not policy; a direct `blue-500` reference escapes theme remapping and silently breaks dark mode and rebrands while looking correct in the light theme.
**Example:** Review flags `color: #3b82f6` and `var(--blue-500)` inside a card; `var(--color-accent)` passes.

### R4. Cover every dimension — color, space, radius, type, elevation, motion, z-index
**Rule:** The token set must include color, spacing, radius, typography, elevation/shadow, motion (duration + easing), and z-index layers. A dimension without tokens will be hardcoded.
**Why:** Hardcoding concentrates exactly where tokens are missing; motion and z-index are forgotten most often and are the most expensive to retrofit (inconsistent feel, stacking bugs across overlays).
**Example:** The token file lists `duration-fast: 150ms` and `ease-out` next to `space-4`, `radius-md`, `shadow-1`, `z-modal` — nothing left to invent ad hoc.

### R5. One shared spacing scale
**Rule:** Use a single base scale (e.g. 4px base: 4 / 8 / 12 / 16 / 24 / 32 / 48 / 64) for all spacing and derive component dimensions from it; off-scale values (13px, 18px) are defects.
**Why:** Independent authors working from one scale produce layouts that align without coordination; two scales or free values produce visible-but-never-global misalignments that no cleanup pass can fix.
**Example:** Icon 16, gap 8, inset 16, section gap 32 — every number from the set, so nested components line up without per-screen nudging.

### R6. Type tokens carry the full stack
**Rule:** Typography tokens define family, size, weight, and line-height together as named roles (`text-body`, `text-title`); the body role is 16px-equivalent with line-height 1.4–1.6.
**Why:** Size without line-height invites per-component guesses that break vertical rhythm; role-bundling keeps text decisions atomic, themable, and readable by default.
**Example:** `text-body: { size: 16, weight: 400, line-height: 1.5 }` — no component sets 16px with a separately hand-picked 1.2.

### R7. Motion lives in tokens: 150–300 ms, named easings
**Rule:** Express micro-interaction durations as tokens within 150–300 ms (fast 150 / base 200 / slow 300) and pair them with named easing tokens (ease-out for entrances, ease-in for exits).
**Why:** Durations above ~300 ms feel laggy and below ~150 ms are invisible — banding the range prevents both; named easings stop every component from inventing its own bounce.
**Example:** `--duration-fast: 150ms; --ease-out: cubic-bezier(0, 0, 0.2, 1)`; every hover fade uses fast + ease-out.

### R8. Z-index is a named layer scale
**Rule:** Define z-index as a small ordered set of named layers (base, dropdown, sticky, overlay, modal, toast) and forbid raw numbers.
**Why:** Ad-hoc z-index grows into a 9999 arms race where every fix is local and quietly breaks another overlay; a layer scale keeps stacking reasoning finite and reviewable.
**Example:** `z-base: 0; z-dropdown: 1000; z-sticky: 1100; z-overlay: 1200; z-modal: 1300; z-toast: 1400` — a new layer slots between named neighbors.

### R9. The semantic tier is the theme seam
**Rule:** Themes (dark mode, brand variants, density) remap the semantic tier only; primitives and component tokens stay identical across themes.
**Why:** With one remap point a theme is a reviewable table of value changes; if themes touch component tokens, every component must be re-verified per theme and theming stops scaling.
**Example:** The dark theme overrides `bg-surface: grey-900` and `text-primary: grey-50`; no `button-*` token appears in the theme file (see theming-dark-mode.md).

### R10. Publish the alias table per theme
**Rule:** Maintain an explicit semantic→primitive mapping per theme, written or generated, so every semantic value is traceable in every theme.
**Why:** Contrast is a property of a pair — verifying the 4.5:1 floor (3:1 for large text and UI components) needs both ends; without the alias table, "what does text-primary resolve to in dark?" is archaeology.
**Example:** Row `text-primary ← grey-900 (light) / grey-50 (dark)`; the contrast check reads the table and fails the build on any sub-floor pair.

### R11. Repetition ≥ 2 becomes a token; one-offs stay local
**Rule:** A value repeated in two or more components becomes a token; a value used once may remain a local constant.
**Why:** Repetition is where inconsistency is born, but premature tokenization bloats the set until nobody can find the right name — both failures cost consistency.
**Example:** Three components use 12px radius → `radius-md` now exists; one modal uses 16px → stays local until a second use appears.

### R12. Every semantic token has a one-line usage note
**Rule:** Document each semantic token with one line stating when to use it ("text-muted — secondary copy, hints, captions; never for long body text").
**Why:** Role names alone under-determine usage; without the note consumers pick by trial and error, misuse spreads, and the vocabulary decays into guesswork.
**Example:** Token list row: `text-muted — secondary copy, hints, captions; not for body paragraphs`.

### R13. One machine-readable source, generated outputs
**Rule:** Keep a single source of truth for tokens (JSON/YAML or equivalent) and generate platform outputs (CSS custom properties, Rust constants, canvas palettes) from it — never hand-copy values across platforms.
**Why:** Hand-copying guarantees drift within weeks; one source lets CI detect drift, lets design changes propagate to every renderer, and makes the token set the actual contract it claims to be.
**Example:** One token file generates `:root { --space-4: 16px }` for web and `pub const SPACE_4: f32 = 16.0` for a Rust GUI from the same entry.

### R14. Version tokens; deprecate with replacement, remove in majors
**Rule:** Version the token set; deprecate by marking the token and naming its replacement, and remove it only in a major release.
**Why:** Silent deletion breaks every consumer at once and invisibly; a marked deprecation gives consumers a migration window and gives tooling something to flag in CI.
**Example:** Changelog: "DEPRECATED `text-secondary-2` → use `text-tertiary` (removal in 2.0)."

## Checklist
- [ ] Tokens cover color, space, radius, type (full stack), elevation/shadow, motion (duration + easing), z-index
- [ ] No semantic or component token name describes a value
- [ ] Zero primitives or raw values referenced from component styles (lint-enforced)
- [ ] All spacing on the one scale; no 13px/18px strays
- [ ] Body text role = 16px-equivalent, line-height 1.4–1.6
- [ ] All motion durations within 150–300 ms and from named tokens
- [ ] Z-index values come only from the named layer scale
- [ ] Theme files remap only the semantic tier
- [ ] Alias table maps every semantic token per theme; contrast pairs verified at 4.5:1 / 3:1
- [ ] Deprecations marked with a replacement; removals only in major releases

## Anti-patterns
- `--red-500` consumed directly as the error color by components
- Hex codes copy-pasted from a design file into component code
- z-index arms race: 99, 999, 9999
- 400 primitives and 5 semantic tokens — tokenized but unusable
- Per-component durations: 120ms, 250ms, 350ms in one app
- Dark mode via `!important` overrides instead of semantic remapping
- Deleting a token in a patch release
- A 13px `space-md` that quietly breaks the scale

## Sources
- Material Design 3 — Design tokens: https://m3.material.io/foundations/design-tokens/overview
- Material Design 3 — Color roles: https://m3.material.io/styles/color/roles
- IBM Carbon — Design tokens: https://carbondesignsystem.com/guidelines/tokens/overview/
- GitHub Primer — Primitives and functional tokens: https://primer.style/foundations
- Salesforce Lightning Design System — Tokens: https://www.lightningdesignsystem.com/design-tokens/
- Style Dictionary (Amazon) — token source to platform output: https://github.com/amzn/style-dictionary
- W3C Design Tokens Community Group — format draft: https://design-tokens.github.io/community-group/format/
