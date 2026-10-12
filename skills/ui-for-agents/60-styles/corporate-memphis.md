# Style: Corporate Memphis

> **Read when:** building UI in the corporate-memphis (Alegria) style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/iconography.md, 00/visual-hierarchy.md, 00/spacing-layout-grids.md

**Personality:** friendly, safe, inoffensive, tech-corporate. **Best for:** corporate sites, HR and onboarding flows, ed-tech, banks wanting warmth, explainer content, illustrated empty states. **Avoid for:** brands needing edge or street credibility (the designer backlash is documented), luxury, underground culture products.

**Core idea:** an illustration-led style, not a chrome style — flat vector humans with exaggerated proportions (tiny heads, long bending limbs), a muted-bright palette, and geometric plant/blob scenery; the UI chrome around the illustrations stays quiet and standard.

Boundary: distinct from playful-friendly.md, where the product chrome itself is warm and chunky — here the chrome is standard flat minimal and the illustration system IS the style.

## Signature (what makes it recognizable)
- Flat vector people with exaggerated proportions: tiny heads, long bending limbs
- Non-realistic skin tones appear freely: blue, purple, green
- No outlines — flat fills only, at any scale
- Muted-bright palette: indigo, coral, teal, sand
- Geometric scenery: plants, blobs, window frames, ladders
- Floating, disconnected limbs; minimal facial detail
- UI chrome: standard minimal sans, generous white space, quiet buttons

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | standard flat chrome | 4–12 px |
| `shadow-*` | none on chrome | flat; scene depth comes from layering, not shadows |
| `color-bg / surface` | white / quiet neutral | #FFFFFF / #F6F7F9 |
| `color-ink` | near-black, regular | #1B1F27, 400–600 weights only |
| `color-accent` | one indigo-family hue | action color; 4.5:1 for text use |
| `illu-indigo` | system people and props | muted indigo family, 2 tints |
| `illu-coral` | warm accents in scenes | muted coral family, 2 tints |
| `illu-teal` | cool accents in scenes | muted teal family, 2 tints |
| `illu-sand` | ground and scenery | sand/ochre family, 2 tints |
| `illu-skin-tones` | documented range | realistic span + optional non-realistic (blue/purple) set |
| `illustration-stroke` | none | flat fills only; no outlines anywhere |
| `type` | neutral sans | scale ratio ~1.25, sentence case |
| `motion-duration` | gentle | 200–300 ms; scene drift slow and optional |

## Rules

### R1. Illustration carries emotion, UI carries function
**Rule:** Never stylize interactive controls to match the illustration — buttons stay buttons, forms stay forms; illustration lives in designated zones.
**Why:** The style works because the chrome is credible; controls drawn like cartoon limbs lose affordance and drag the product into costume.
**Example:** Onboarding hero: full-bleed Alegria scene; the "Get started" button below it is a plain standard button.

### R2. Do not fix the anatomy
**Rule:** Keep the proportion exaggeration — tiny heads, elongated curved limbs, floating hands. Do not nudge it toward realism.
**Why:** Half-real anatomy is a different, uncanny style; the exaggeration IS the recognizable signature and its safe distance from realism.
**Example:** A "corrected" seven-heads-tall figure with jointed hands reads as bad realism, not Corporate Memphis — ship the exaggerated one.

### R3. One illustration system
**Rule:** A single library of people, props, and scenery is reused across the product — same flat-fill treatment, same `illu-*` palette tokens, same exaggeration rules.
**Why:** Mixed illustration sources (stock + custom + AI one-offs) fragment the visual voice; the system, not any single image, is the style.
**Example:** Empty states, success banners, and help articles all draw from the same tokenized library; no third-party clip-art enters.

### R4. Representation is decided, not defaulted
**Rule:** Document the inclusion plan inside the system: skin-tone range (including any non-realistic hues), body types, age mix, ability aids, and role variety per scene.
**Why:** Flat casting makes diverse figures cheap — defaulting to one body type squanders it and invites the style's "same person everywhere" criticism.
**Example:** The scene matrix requires: every flow features ≥ 3 distinct figures spanning tone tokens, ≥ 2 body types, one older figure.

### R5. Illustrations support text, never replace it
**Rule:** Instructions and steps remain fully readable as text; illustrations reinforce mood or spatial metaphor beside the copy.
**Why:** The style's faces and gestures are too abstract to carry meaning; meaning-dependent-on-illustration breaks screen readers and translation.
**Example:** "Connect your bank" step: numbered text instruction plus a small side illustration that adds nothing the text requires.

### R6. Match the context: warmth vs credibility
**Rule:** Use where warmth beats credibility — onboarding, empty states, culture pages. For credibility-critical surfaces (security notices, dev tools, legal), use real UI screenshots or neutral geometry instead.
**Why:** The documented backlash is about trust: the style reads as sanitized corporate cheer, and forcing it onto serious moments amplifies the problem.
**Example:** Marketing homepage gets Alegria scenes; the security-incident banner gets text, status color, and a real product screenshot.

### R7. Motion is gentle and optional
**Rule:** 200–300 ms for UI transitions; illustration scenes may drift subtly (parallax ≤ 8 px, slow sway) and must stop under reduced-motion.
**Why:** Busy looping scenes contradict the calm, inoffensive brief and tax attention; drift adds life without demanding it.
**Example:** Hero scene: one leaf sways on a 6 s loop; `prefers-reduced-motion` swaps it for a static frame.

### R8. Density drops where illustration enters
**Rule:** Give illustrated zones extra whitespace and cap them at one scene per view; task-dense views (tables, settings) get zero illustrations.
**Why:** The style's flat shapes consume visual space greedily; mixed into dense UI they create mud and slow scanning.
**Example:** Dashboard = no illustration; the empty state above the table = one scene with 48 px clearance, nothing else.

## A11y watchpoints
- Every illustration needs a decision: meaningful → concise alt text; decorative → `aria-hidden="true"` (default to decorative for scenes)
- Illustration-heavy sections still need 4.5:1 text on solid panels — never set copy over blob shapes
- Do not encode instructions in illustration gestures alone (a pointing figure is not a "next" affordance)
- Scene drift and sway require a reduced-motion static fallback
- Muted-bright fills used AS UI colors (badges, buttons) must re-verify 4.5:1/3:1 — the palette is tuned for scenes, not for text

## Checklist
- [ ] Interactive controls never stylized to match illustrations
- [ ] Proportion exaggeration preserved (no realism drift)
- [ ] One illustration system; scene colors from `illu-*` tokens only
- [ ] Inclusion plan documented (tones, bodies, ages, abilities)
- [ ] All instructions readable as plain text
- [ ] Credibility-critical surfaces use screenshots or neutral geometry
- [ ] Scene motion ≤ 8 px drift with a reduced-motion fallback
- [ ] One scene per view; dense views have none
- [ ] Every asset has an alt-text or aria-hidden decision
- [ ] Chrome stays standard flat minimal sans

## Anti-patterns
- Cartoon-styled buttons, inputs, or toggles
- "Fixed" realistic anatomy — the uncanny middle
- Mixing stock, custom, and AI-generated one-off illustrations
- Busy looping scene animations
- Illustrations carrying required instructions
- Corporate Memphis on security, legal, or developer surfaces
- Off-palette scene colors (per-illustration hue inventions)
- Diversity expressed only through skin-tone swaps on one body

## Sources & inspiration
- Corporate Memphis — the style, its Alegria origin (Buck, for Facebook), and the backlash: https://en.wikipedia.org/wiki/Corporate_Memphis
- Buck — the studio behind the Alegria system: https://buck.co
- Flat design — the chrome paradigm this style borrows: https://en.wikipedia.org/wiki/Flat_design
- W3C WAI images tutorial — alt-text decisions per illustration: https://www.w3.org/WAI/tutorials/images/
- WebAIM alternative text guide: https://webaim.org/techniques/alttext/
- Coolors — building the muted-bright palette ramps: https://coolors.co
