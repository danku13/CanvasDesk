# Style: Futuristic Neon

> **Read when:** building UI in the futuristic-neon style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/elevation-depth.md, 00/motion-principles.md

**Personality:** high-tech, electric, immersive, bold. **Best for:** gaming, esports, crypto/web3, AI products, nightlife/events. **Avoid for:** healthcare, government, banking forms, long-reading surfaces.

**Core idea:** neon is a light in a dark room — the style earns its drama from a layered near-black base plus two disciplined accent lights that mark what is clickable and what is alive; everything else stays dark and quiet.

## Signature (what makes it recognizable)
- Deep dark base built from layered near-black steps (#050510-family), never flat #000
- Exactly two saturated neon hues + white, with fixed roles (e.g. cyan = action, magenta = alert)
- Glow = outer shadow in the accent hue (30–60% blur), reserved for interactive and live states
- HUD-style labels: thin techy display type, uppercase, letter-spaced; mono faces for data
- Sci-fi structure decoration — corner brackets, 1px translucent frames, subtle scanlines — used sparingly
- Elevation by surface lightness: each layer steps lighter toward the viewer
- Dark imagery with a neon rim light; one glow direction per composition

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `color-bg` | layered near-black steps | #050510 → #0A0A18 → #101024 (3–4 steps, one hue family) |
| `color-surface` | steps lighter than bg | #14142C cards / #1C1C3A popovers |
| `color-accent-1` | one neon, action role | cyan #22D3EE-family (or lime) |
| `color-accent-2` | second neon, alert/live | magenta #E879F9-family; ≤10% of any view |
| `glow-*` | same-hue outer shadow | 0 0 12px @40% (hover) / 0 0 24px @60% (focus, live) |
| `radius-*` | small, angular | 2–6px; corners may be clipped/notched instead of rounded |
| `border-*` | thin translucent neon | 1px accent at 30–50% alpha |
| `type-display` | techy, uppercase, tracked | +0.08–0.15em tracking; scale ~1.333; ≥14px |
| `type-body` | clean neutral sans | near-white 16px, line-height 1.5, never neon |
| `motion-duration` | quick + glow pulse | 200–400 ms ease-out; pulse 1.5–2 s loop only for live states |

## Rules

### R1. Neon is a light source, not paint
**Rule:** Glow marks interactivity and liveness — focus, hover, active, live/online states — and nothing else; static decoration never glows.
**Why:** In the dark-room metaphor, light means "something happens here"; glowing decoration severs that mapping and floods the interface with false affordances.
**Example:** The primary CTA glows on hover and focus; the section headline above it does not.

### R2. Two hues, fixed jobs
**Rule:** Use at most two neon hues plus white; assign each a role (cyan = action, magenta = alert/live) and keep those roles on every screen and component.
**Why:** Neon hues are extremely loud; three or more compete for attention and the primary path disappears; fixed roles keep the system predictable.
**Example:** Cyan on buttons, links, and active tabs; magenta only on "LIVE" badges and destructive warnings — never swapped between screens.

### R3. Build depth with lightness steps, not shadows
**Rule:** Layer the dark base — bg darkest, surfaces one step lighter, popovers two — as in dark-premium; blur shadows barely read on near-black.
**Why:** On dark backgrounds elevation must be carried by surface lightness and edge highlights; shadow-based elevation goes muddy and invisible.
**Example:** Modal #1C1C3A over card #14142C over bg #050510, each edged with a 1px 20%-alpha border.

### R4. HUD decorations annotate structure
**Rule:** Corner brackets, scanlines, and thin frames mark hot zones (featured cards, critical panels) — they never wrap body text, and at most one zone per view is decorated.
**Why:** HUD chrome says "instrument panel, data here"; around paragraphs it only adds noise and chops reading flow.
**Example:** The live-match card gets corner brackets; the rules text beside it sits in a plain bordered panel.

### R5. Neon text is display-only
**Rule:** Body copy is always near-white on the dark base; neon appears in text only for short display strings (headlines, stat numerals) verified at 4.5:1 on their surface.
**Why:** Saturated hues pass contrast on dark only at high luminance and large sizes; neon paragraphs are unreadable and exhausting over time.
**Example:** Score "87:104" in cyan at 32px; the play-by-play paragraph below in white 16px.

### R6. Tokenize the glow
**Rule:** Define one glow ladder — rest = none, hover = 12px @40%, focus/live = 24px @60% — and reuse it everywhere; no per-screen improvised strengths or rainbow shadows.
**Why:** Glow is the style's most expensive resource; uncontrolled intensities make some screens feel broken and others blinding, and QA becomes impossible.
**Example:** Every hoverable card uses the same `glow-hover` token; nothing invents a 60px pink shadow.

### R7. Pulse means alive
**Rule:** Glow pulses animate only live states (recording, online, match in progress) on a 1.5–2 s loop; everything else transitions once, 200–400 ms ease-out, and stops.
**Why:** A pulse is a heartbeat — semantic, not decorative; constant pulsing destroys the signal and creates photosensitivity risk.
**Example:** The "● LIVE" badge pulses while the match runs; the CTA next to it glows statically on hover only.

### R8. Geometry stays angular and disciplined
**Rule:** Radius 2–6px (or clipped/notched corners), 1px translucent borders, content on a consistent grid with generous padding — angular consistency is the structural signature.
**Why:** Random rounded blobs or missing borders dissolve the sci-fi precision; the framework of thin lines is what makes sparse glow read as technology.
**Example:** All cards share a 4px radius and 1px 30% border; notched corners appear only on the hero frame.

### R9. The style must survive without glow
**Rule:** Every state the glow communicates must also be visible without it — borders brighten, weight shifts, icons change — so high-contrast mode, print, and low-power renderers stay usable.
**Why:** Glow is low-intensity light that some displays, projectors, and modes drop entirely; if state lives only in glow, those users lose the interface.
**Example:** A focused button shows glow plus a 2px solid outline; the outline alone still passes 3:1.

## A11y watchpoints
- Neon on mid-tones fails: text is white or sits on the darkest layers; verify every accent-on-surface pair at 4.5:1 (3:1 for ≥24px display) and UI boundaries at 3:1
- Glow is never the only focus indicator — pair every glow with a solid 2px outline at 3:1 against adjacent colors
- Photosensitivity: nothing flashes more than 3×/second (WCAG 2.3.1); `prefers-reduced-motion` stops all pulses, scans, and loops
- Uppercase tracked HUD labels shrink perceptually — set a ≥14px floor for labels, never use the display face for body copy, and keep letter-spacing ≥0.08em so glyphs do not collide
- Large dark fields hide thin 1px borders: confirm every interactive control meets 3:1 against its own surface, not against the page bg

## Checklist
- [ ] Exactly 2 neon hues + white; roles documented and never swapped
- [ ] Backgrounds are layered steps; flat #000 unused
- [ ] Glow only on interactive/live elements, from the token ladder
- [ ] Focus = glow + solid outline; outline passes 3:1
- [ ] Body text near-white, ≥4.5:1 on its actual surface
- [ ] No more than 3 flashes/second; reduced-motion kills every pulse/loop
- [ ] HUD decoration ≤1 zone per view; never around body text
- [ ] Transitions 200–400 ms ease-out; pulses 1.5–2 s and live-only
- [ ] State still readable with glow disabled (borders/weight/icons)

## Anti-patterns
- Rainbow neon: 4+ hues, every element glowing, no findable primary action
- Static glow on headlines and decoration ("because cyberpunk")
- Neon body text or neon-on-gray labels below contrast floors
- Pure #000 canvas with default-gray text
- Everything pulsing; infinite loops on non-live elements
- Full-screen scanline/grid overlays running under paragraphs
- A soft glow replacing the focus outline
- HUD brackets, rivets, and chrome crowding every card into a cosplay prop

## Sources & inspiration
- Material Design 3 — color system & dark-theme elevation logic: https://m3.material.io/styles/color/system/overview
- Nielsen Norman Group — dark mode user expectations: https://www.nngroup.com/articles/dark-mode/
- Game UI Database — real HUD and game interface references: https://www.gameuidatabase.com
- WCAG 2.2 — 2.3.1 Three Flashes or Below Threshold: https://www.w3.org/WAI/WCAG22/Understanding/three-flashes-or-below-threshold.html
- Synthwave — the movement whose palette this style formalizes: https://en.wikipedia.org/wiki/Synthwave
- Scan line — the CRT heritage of the texture: https://en.wikipedia.org/wiki/Scan_line
