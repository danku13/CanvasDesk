# Style: Playful Friendly

> **Read when:** building UI in the playful-friendly style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/motion-principles.md, 00/iconography.md

**Personality:** warm, energetic, approachable, optimistic. **Best for:** kids/education, consumer apps, fitness, food delivery, community products. **Avoid for:** banking, legal, medical-critical flows, enterprise admin.

**Core idea:** warmth through shape, color, and motion — energy is budgeted to moments (rewards, empty states) and calmed for work (forms, settings).

## Signature (what makes it recognizable)
- Large radius everywhere: 16–24px surfaces, pill buttons
- Saturated multi-hue palette — 2–3 bright hues mapped to roles over warm neutrals
- Illustration over photography; custom character/mascot allowed
- Chunky controls: fat buttons, big toggles, thick progress bars
- Springy motion 200–400 ms with overshoot reserved for success moments
- Rounded display font (Baloo/Nunito-like) paired with a legible body sans
- Emoji/sticker energy in empty states and celebrations

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `radius-*` | large | 16–24 px surfaces; pill (999px) buttons |
| `shadow-*` | soft, hue-tinted | accent shadow (20–30% alpha) allowed; never gray mush |
| `space-scale` | standard | 8pt scale with 4pt half-steps |
| `color-bg / surface` | light warm neutral | #FFF8F2 / #FFFFFF range |
| `color-accent-*` | 2–3 saturated hues mapped to roles | primary/action, secondary/decor, fixed feedback set |
| `color-ink / text` | neutral dark / white | near-black ink on fills; never hue-on-hue text |
| `type-family` | rounded display + legible body sans | Baloo/Nunito-like display; Inter-like body |
| `type-scale-ratio` | medium–large | ~1.25–1.333 |
| `motion-duration` | springy | 200–400 ms; overshoot on celebration, ease-out elsewhere |
| `illustration` | state-marking, custom | one scene per state: empty / success / error / onboarding |
| `target-size` | chunky | ≥ 44 px touch targets everywhere |

## Rules

### R1. Two energy modes: expressive vs calm
**Rule:** Expressive surfaces (onboarding, empty states, success, rewards) may use full color, illustration, and motion; working surfaces (forms, settings, lists) stay tamed — neutral background, standard controls, minimal motion.
**Why:** Constant stimulation exhausts users and buries the actions that matter; alternating intensity makes both modes land harder.
**Example:** Checkout form is plain neutral with one primary pill button; the post-purchase screen erupts with confetti and a mascot.

### R2. Play in shape and color, never in text
**Rule:** Ink and body text stay neutral — near-black on light fills, white on saturated fills — with every pair verified at 4.5:1; playfulness lives in radius, hue, illustration, and motion.
**Why:** Colored text on colored backgrounds is this style's most common contrast failure; the fun must never cost legibility.
**Example:** Coral CTA carries white text at 4.5:1; helper text stays ink-gray, never pink-on-peach.

### R3. Illustration carries meaning, not noise
**Rule:** Every illustration marks a real state or step (empty, error, success, progress, tutorial) and sits next to text explaining it; decorative wallpaper illustrations are cut.
**Why:** State-marking illustrations teach and reassure; orphan decoration inflates load time and cognitive noise while teaching nothing.
**Example:** Empty inbox: friendly character + "Nothing here yet — send your first note" + one action button.

### R4. Overshoot is for rewards only
**Rule:** Spring/overshoot easing appears only on success and celebration moments; navigation, destructive confirms, and error states use plain ease-out at 200–300 ms.
**Why:** Overshoot is an emotional high-five; using it everywhere inflates rewards, and using it on a delete confirmation reads as mocking.
**Example:** Streak counter springs with overshoot; the "Delete account" dialog animates a plain 200 ms fade.

### R5. Chunky is functional
**Rule:** Touch targets ≥ 44px everywhere (24×24 px absolute floor); fat buttons, big toggles, and generous hit areas are features, not waste.
**Why:** The audience skews young, casual, and mobile; small elegant controls betray the promise of approachability and hurt real motor performance.
**Example:** Settings toggles are 32px-tall pills with 56px hit areas; primary buttons ≥ 52px tall.

### R6. One loudest action per screen
**Rule:** Enforce hierarchy by size plus hue: exactly one primary button per view in the loudest hue; secondary actions are quiet outlines or text links.
**Why:** With a multi-hue palette, competing loud buttons paralyze choice; scarcity of the loudest treatment keeps the path obvious.
**Example:** Lesson end screen: one huge "Continue" pill; "Share" and "Review" are smaller outline buttons.

### R7. Personality with a way out
**Rule:** Empty, error, and onboarding states may carry sticker/emoji energy but must always state what happened and offer one clear next action.
**Why:** Charm without recovery is friction; the mascot may comfort, but the button must solve.
**Example:** Error: sad character + "Couldn't load your stamps — check your connection" + "Try again" button.

### R8. Dark mode re-maps hues, never dims
**Rule:** The dark theme rebuilds the palette: lighter tint variants of each hue, elevated surface lightness steps, every pair's contrast re-verified — never darkened originals.
**Why:** Dimmed saturated hues go muddy and quietly fail contrast; re-mapped hues keep the playful identity intact on dark surfaces.
**Example:** Coral #FF6B4A becomes a lighter #FF8A6B on dark ink, verified 4.5:1 before shipping (see 10/theming-dark-mode.md).

### R9. Budget the saturation for long sessions
**Rule:** Large-area surfaces stay in warm neutrals; saturated hues cover accents, controls, and illustration only — squint-test any screen a user sits on for 30+ minutes.
**Why:** Full-saturation fields cause measurable eye fatigue; the palette stays bright precisely by not being everywhere.
**Example:** A 20-minute quiz flow runs on a cream background with coral reserved for the progress bar and CTA.

## A11y watchpoints
- Hue-on-hue text fails constantly: verify every text/fill pair at 4.5:1 in both themes; ink stays neutral
- Reduced motion must not break comprehension: overshoot and confetti switch off, but state changes stay clearly visible (WCAG 2.3.3)
- Emoji and mascot are decorative only (hidden from assistive tech) — never the sole iconography or status indicator
- Saturated palettes fatigue: long-session screens pass a squint/contrast audit
- Control boundaries keep 3:1 against playful fills — outlined buttons on colored cards included

## Checklist
- [ ] Working surfaces (forms, settings) tame; expressive elements off
- [ ] All text pairs ≥ 4.5:1 on their fills, both themes
- [ ] Overshoot only on success; reduced-motion keeps states visible
- [ ] Every illustration marks a state and sits next to explanatory text
- [ ] Touch targets ≥ 44px; nothing below the 24×24 px floor
- [ ] Exactly one loudest primary action per view
- [ ] Dark mode re-maps hues with contrast re-verified
- [ ] Emoji decorative only; meaning never emoji-only
- [ ] Long-session screens dominated by neutrals

## Anti-patterns
- Overshoot on navigation, modals, and destructive confirms
- Rainbow palette: 6+ hues with no role mapping
- Illustration wallpaper with no state meaning
- Dark mode as dimmed originals; contrast quietly dropped
- Emoji as the only icons or status indicators
- Rounded display font used for body text and long-form content
- Radius so large that controls lose their shape and alignment
- Confetti on every click — reward inflation

## Sources & inspiration
- Material Design 3 — expressive motion and spring physics: https://m3.material.io/styles/motion
- WCAG 2.2 — contrast, target size, animation floors: https://www.w3.org/TR/WCAG22/
- MDN — prefers-reduced-motion: https://developer.mozilla.org/en-US/docs/Web/CSS/@media/prefers-reduced-motion
- Laws of UX — aesthetic-usability effect: https://lawsofux.com/aesthetic-usability-effect/
- Game UI Database — playful and illustrated UI reference: https://www.gameuidatabase.com
