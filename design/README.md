# Design — CanvasDesk design-system and UI-system rules

This folder is the **normative source of rules** from which the CanvasDesk UI
is built. It is deliberately separated from the code: the owner edits here,
the agent carries the edits into code (tokens → `canvas-core::tokens`/`ThemeColors`,
rules → component refactoring, use cases → behavior contracts).

Three contours, three file types:

| Folder | Contents | How it is edited | What happens on an edit |
|---|---|---|---|
| `tokens/` | **Primitives** — machine-readable JSON (colors, dimensions, motion, themes) in the W3C spirit (`$type`/`$value`/`$desc`) | `$value`s change | The mirror `crates/canvas-core/src/tokens.rs` + parity test (I-5); invariant I-1: a value edit = a managed visual shift, not an accidental one |
| `rules/` | **Normative rules** of the design system and UI system (human-readable) | Rules change | The agent refactors components to the new rule; the rule takes effect for all new code immediately |
| `use-cases/` | **Component behavior contracts** — anatomy, states, interaction | A component's scenario/geometry changes | The agent brings the component implementation to the new contract |

## Contents

**Rules** (`rules/`) — read in order:

1. `rules/00-principles.md` — the foundation: three token layers, invariants (I-1
   zero visual shift, I-5 JSON↔Rust parity), the slot-only kit,
   "input = what is visible", measured text, lints G1–G8.
2. `rules/01-colors.md` — the color system: a single accent, alpha steps,
   semantic states, the ban on color literals.
3. `rules/02-typography.md` — fonts (Noto Sans Display / Noto Sans Mono /
   CanvasDesk Mono Oblique), the size scale 10/10.5/11/12/13/14/16, line height,
   text measurement and truncation rules.
4. `rules/03-spacing-radius.md` — spacing scales 6/8/10/12/24 and radii
   6/8/10/12, fixed component heights, gaps.
5. `rules/04-contrast-a11y.md` — the contrast machine (text ≥ 4.5:1, graphics
   ≥ 3:1), auto-contrast for user fills, hit zones, window degradation.
6. `rules/05-layering.md` — 9 layers L0–L8, capture policies (Block/Capture/
   PassThrough/Passive), the Esc stack, the scissor policy.
7. `rules/06-motion.md` — the duration scale 150/300/600/1200/1600/2500 ms,
   dt-determinism, animation principles.
8. `rules/07-theming.md` — themes as data (7 presets), dark/light,
   derived slots, the bridge into the kit.
9. `rules/08-states.md` — the control state matrix
   (Disabled > Pressed > Hovered > Selected > Normal), state slots.
10. `rules/09-input.md` — behavioral adequacy of input modules: a single
    field model (kit TextFieldModel), the keyboard contract, scroll-follows-caret,
    the password mask, IME/web parity, the ban on duplicates.
11. `rules/10-components.md` — the kit v2 component registry (K1), the
    KitPalette slot registry (K2), the scroll model (K3), the UI console
    showcase composition (K4). Introduced by the 2026-10-09 audit
    (`rules-audit-2026-10.md`).
12. `rules/11-layouts.md` — layouts and adaptivity: the slot model
    (LAY1), primitives ↔ web equivalents (LAY2), overflow policies
    Fit/SqueezeTail/Wrap (LAY3), flex factors (LAY4), grid + the SceneNode
    scene: percent/aspect/sticky/minmax (LAY5), measured text (LAY6),
    gap scales (LAY7), adaptivity: G4 viewports / HideBelow /
    breakpoints / 44 touch targets / safe-area (LAY8), engines and HTML5 parity
    (LAY9), typical skeletons and anti-patterns (LAY10), the checklist (LAY11).
    Introduced 2026-10-09 — closing the "no layout rules" gap
    (the fragments previously lived in П7/П10/S6).

**Use cases** (`use-cases/`) — one file per component:

- Kit elements: `use-cases/button.md`, `use-cases/chip-badge.md`, `use-cases/text-field.md`,
  `use-cases/switch.md`, `use-cases/dropdown.md`, `use-cases/tooltip.md`, `use-cases/toast.md`, `use-cases/modal-dialog.md`,
  `use-cases/scrollbar.md`, `use-cases/icon.md`.
- Canvas: `use-cases/card.md` (node card), `use-cases/edge.md` (edges), `use-cases/minimap.md`,
  `use-cases/wheel-menu.md`.
- Surfaces: `use-cases/context-menu.md`, `use-cases/search-panel.md`, `use-cases/template-palette.md`,
  `use-cases/scheme-gallery.md` (+ empty state), `use-cases/whatif-bar.md`, `use-cases/hud.md`.

Every use case follows a single structure: Purpose → Anatomy and dimensions →
Tokens → States → Interaction (mouse/keyboard) → Edge cases →
Where it is in the code → What changes when the file is edited.

The rules↔kit conformance audit: `rules-audit-2026-10.md`
(the 00–09 → implementation matrix, rule gaps, the prioritized W1–W4 backlog).

## Sources and priority

If `rules/` or `use-cases/` contradict the code — these files win (they are
the target state), and the divergence is filed as a refactoring task.
If they contradict `docs/prd/prd-0006-design-system-tokens.md`,
`docs/prd/prd-0009-ui-layering-uikit.md`, `docs/ui-kit.md` — this folder wins
for **visual/behavioral** values; architectural decisions
(ADR-0013 dropping taffy, ADR-0015 the UI-stack strategy) are not revisited
here — only via an ADR.

## Quick value cheatsheet

| Group | Values |
|---|---|
| Spacing | 6 / 8 / 10 / 12 / 24 (SPACING_S/SM/MD/LG/XL) |
| Radii | 6 chip / 8 card / 10 panel / 12 pill |
| Heights | button 30, chip 24, list row 26, field 30, card header 34 |
| Font sizes | 10 badge / 10.5 zone / 11 template / 12 label·result / 13 rows / 14 body·HUD / 16 title |
| Text | text ≥ 4.5:1, graphics ≥ 3:1, line spacing ×1.3 (screen) |
| Animations | 150 ms micro / 300 ms flight / 1200+ ms decorative |
| Layers | World → WorldOverlay → Widgets → Panels → Popups → Modals → Drag → Toasts → Debug |
| States | Disabled > Pressed > Hovered > Selected > Normal |
| Layouts | slot → Row/Column/grid_cells/stack → scene (percent/sticky) → Custom; overflow — a named policy; 3 G4 viewports (11-layouts) |
