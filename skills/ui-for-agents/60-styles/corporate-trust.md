# Style: Corporate Trust

> **Read when:** building UI in the corporate-trust style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/spacing-layout-grids.md, 00/typography.md

**Personality:** credible, stable, professional, organized. **Best for:** banking, insurance, B2B SaaS, government, legal, healthcare admin. **Avoid for:** creative portfolios, entertainment, kids products.

**Core idea:** trust is earned through consistency and discipline — the interface is a uniform, not a costume; every deviation reads as instability.

## Signature (what makes it recognizable)
- Conservative palette: one institutional blue/steel anchor + strict neutrals; one warm CTA accent allowed
- High information density: compact 8pt spacing, tables-first layouts
- System-like neutral sans (IBM Plex / Source Sans-like) with disciplined weights (2–3)
- Strict symmetric grid; edges, columns, and baselines align everywhere
- Restrained thin-outline iconography — functional, single-weight, no decoration
- Minimal photography: authentic people-at-work only, or none
- Data presentation is the hero: tables, charts, status chips

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | minimal | 2–4 px |
| `shadow-*` | subtle layered | 2 low-alpha layers; only for real elevation (menus, modals) |
| `space-scale` | compact 8pt | 4 / 8 / 12 / 16 / 24 steps favored |
| `color-bg / surface` | white / light-gray professional | #FFFFFF / #F4F6F8 range |
| `color-primary` | institutional blue | ≥ 4.5:1 on white (e.g., #1B4B8F range) |
| `color-accent` | one warm CTA hue | the money action only, per view |
| `status-colors` | one semantic system | fixed success/warning/error/info set, always with text labels |
| `type-family` | neutral grotesque | IBM Plex Sans / Source Sans-like; 2 weights |
| `type-scale-ratio` | small | ~1.2 |
| `motion-duration` | fast, functional | 150–200 ms, ease-out; zero personality |
| `density-mode` | compact default | comfortable-mode token set for accessibility |

## Rules

### R1. Consistency is the product
**Rule:** Components are pixel-identical across every page — same table header, same button, same form rhythm; any deviation needs a written reason.
**Why:** For trust products the UI itself is evidence of operational discipline; inconsistency is read, correctly, as organizational sloppiness.
**Example:** Claims, payments, and settings all use the same table, the same header row, the same button set.

### R2. Density with escape hatches
**Rule:** Default to compact density and ship a documented comfortable mode (larger spacing, bigger targets) users can switch to — never compress below the 24×24 px target floor.
**Why:** Expert users want throughput, but accessibility and long-session comfort require an out; both modes are first-class, not an afterthought.
**Example:** Transactions table ships at 40px rows; the density toggle switches to 52px rows and larger inputs.

### R3. Status is systematic
**Rule:** One fixed semantic color set (success/warning/error/info) is used identically in tables, chips, banners, and charts — always paired with a text label, never color alone.
**Why:** Semantic drift ("amber means delayed here, pending there") forces users to re-learn each screen and breeds errors in exactly the products that can least afford them.
**Example:** "Failed" chip: red icon plus the word "Failed" in every table, toast, and report across the product.

### R4. Blue is structural; the warm accent is rare
**Rule:** Institutional blue carries structure — headers, links, primary surfaces, focus; the warm CTA hue appears on at most one action per view, the money step.
**Why:** Two hues with strict jobs create instant legibility of what matters here; a third hue or a second loud CTA dissolves it.
**Example:** Payment page: blue everywhere, warm accent only on "Confirm transfer".

### R5. Tables are the home screen
**Rule:** Design data presentation first — right-aligned numbers, sortable headers, sticky headers, scan rails — then compose around it, never through it.
**Why:** Corporate-trust users live in tables; marketing-style stat cards over dense data waste the style's core competence.
**Example:** Portfolio view: dense holdings table with right-aligned amounts and status chips, not a grid of cards.

### R6. Charts obey institutional rules
**Rule:** Series colors are muted, ordered, and consistent product-wide; semantic red/green appear only to mean semantics; direct labels preferred; no 3D, no dual axes, no decorative gradients.
**Why:** In regulated contexts a chart is a document — it must be precise, reproducible, and honest; decoration undermines the credibility this style exists to build.
**Example:** Revenue chart reuses the same four series colors as every other chart, with labeled endpoints.

### R7. Photography honest or none
**Rule:** Use real workplaces and real team photography sparingly — or skip imagery entirely; never stock handshakes, glossy smiles, or hero montages.
**Why:** Banking, legal, and government audiences are stock-photo-literate; staged imagery actively costs credibility.
**Example:** Careers page uses real office photos in natural light; product screens use no photography.

### R8. Motion stays invisible
**Rule:** Transitions run 150–200 ms, ease-out, and only communicate state change (expand, reveal, progress) — no personality motion, no parallax, no celebratory animation.
**Why:** Motion in trust products must confirm causality and vanish; visible personality motion reads as instability and slows expert users down.
**Example:** Accordion expands in 180 ms; that is the entire motion vocabulary of the admin.

### R9. Hierarchy without new hues
**Rule:** Build hierarchy from weight, size, ink levels, and spacing — adding a new hue to "make it stand out" is a defect; file it as a design-system issue instead.
**Why:** Palette discipline keeps long sessions legible and the brand credible; hue inflation is how trust interfaces decay.
**Example:** "Important" callout = heavier weight plus left border plus a background step — not a new purple.

### R10. Dark mode is tonal, not inverted
**Rule:** Dark theme uses tonal grays (e.g., #121826-range surfaces, near-white text) with blue re-tinted lighter — never pure black with pure white and untouched brand blue.
**Why:** Users spend 8-hour days in these products; full inversions cause halation and eye strain, and unadjusted blues fail contrast on dark surfaces.
**Example:** Dark dashboard: #121826 background, #E8EBF0 text, links in a lightened blue verified at 4.5:1.

## A11y watchpoints
- Blue on gray is the classic failure: verify every blue text and chip pair at 4.5:1 against each gray surface, both themes
- Dense layouts must keep 24×24 px targets (44px touch on mobile) — density never shrinks hit areas
- Status never color-only: chips carry text labels and icons (WCAG 1.4.1)
- Long sessions: tonal dark mode, never pure white-on-black inversion
- Dense tables: visible focus (2px outline, 3:1 against adjacent) and full keyboard traversal of rows and cells

## Checklist
- [ ] One primary blue plus one warm CTA hue; no ad-hoc hues introduced
- [ ] Single semantic status set with text labels everywhere
- [ ] Compact default plus working comfortable mode
- [ ] Targets ≥ 24×24 px in dense layouts (44px touch)
- [ ] Charts: muted consistent series; red/green semantic-only
- [ ] Photography authentic or absent
- [ ] Motion 150–200 ms, functional only
- [ ] Blue-on-gray and all text pairs ≥ 4.5:1, both themes
- [ ] Components identical across pages; deviations documented
- [ ] Dark mode tonal, not inverted

## Anti-patterns
- Trust-blue gradient hero with a stock handshake photo
- A new accent color per department or per campaign
- Status conveyed by a colored dot alone
- Density so compact that targets drop below the floor
- Playful imports: rounded buttons, mascots, confetti in a bank app
- Chart rainbows and 3D pie charts
- Pure black/white dark-mode inversion
- Parallax and marketing animation on transactional screens

## Sources & inspiration
- IBM Carbon Design System — enterprise discipline and data density: https://carbondesignsystem.com
- US Web Design System — government-grade trust patterns: https://designsystem.digital.gov
- GOV.UK Design System — plain, credible, accessible defaults: https://design-system.service.gov.uk
- Atlassian Design System — B2B SaaS density and status systems: https://atlassian.design
- Stanford Web Credibility guidelines — what builds trust: https://credibility.stanford.edu
- WCAG 2.2 — contrast and non-color floors: https://www.w3.org/TR/WCAG22/
