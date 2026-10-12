# Style: Neubrutalism

> **Read when:** building UI in the neubrutalism style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/elevation-depth.md, 00/color.md, 00/typography.md, 00/motion-principles.md

**Personality:** bold, cheeky, direct, playful-brash. **Best for:** startups with personality, dev tools with attitude, portfolios, event sites, creative SaaS marketing. **Avoid for:** banking, medical, government, dense enterprise tools.

**Core idea:** brutalism that went to art school — brutalism's thick borders and hard offset shadows fused with pop-saturated color blocks, chunky friendly type, and designed playfulness; the structure stays raw and exposed, but everything is curated and warm.

## Signature (what makes it recognizable)
- 2–4px solid black borders on EVERYTHING — cards, buttons, inputs, images, badges
- Hard offset shadow 4–8px, solid black, zero blur; hover shrinks the shadow as the element translates into it
- Saturated flat color blocks — lime `#B8F818`, pink `#FF90E8`, cyan, purple — one to three per view
- Off-white background `#FFFDF5`; pure black `#000` glues borders, type, and shadows
- Chunky grotesque display type (Archivo Black class), often uppercase, tight leading
- Radius consistency with commitment: all-0 or all 12–24px — never mixed
- Sticker-like badges and oversized, unmistakable buttons
- Flat everything: no gradients, no glows, no soft elevation

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `border-width` | thick, everywhere | 2–4px solid `#000` |
| `shadow-rest` | hard offset | `4px 4px 0 #000` (up to 8px for hero cards) |
| `shadow-press` | hover/active state | `0 0 0 #000` + translate(4px, 4px) |
| `radius-*` | commit to one system | all 0, or all 12–24px |
| `space-scale` | standard, chunky | 8pt scale; generous component padding 16–24px |
| `color-bg` | warm off-white | `#FFFDF5` |
| `color-block-1` | hero block | lime `#B8F818` (black ink ≈16:1) |
| `color-block-2` | section block | pink `#FF90E8` (black ink ≈10:1) |
| `color-block-3` | optional third | cyan `#7DF9FF` or purple `#C4A7FF` |
| `color-ink` | pure black — on brand here | `#000` for type, borders, shadows |
| `type-display` | chunky grotesque | Archivo Black class, 800–900, uppercase allowed |
| `type-body` | readable sans | 16px / 1.5 — the style's one quiet voice |
| `motion-duration` | snap | 100–150 ms ease-out; no springs |

## Rules

### R1. Border + shadow is the interaction system
**Rule:** Every element pairs its 2–4px border with a hard offset shadow at rest; on hover/active the shadow shrinks to 0 and the element translates into the shadow's footprint — the "press" physics. Never communicate hover with color alone.
**Why:** The press is the style's signature physical metaphor (a real key being pushed); it makes state changes visible in position and geometry, which also keeps them perceivable without color.
**Example:** Button at rest: `4px 4px 0 #000`. Hover: `translate(4px,4px)`, shadow `0 0 0`. Focus: + 3px offset outline in an accent color.

### R2. Color blocks carry zones
**Rule:** Each page section gets exactly ONE block color as its ground; blocks never alternate randomly inside a section, and one to three blocks total per view.
**Why:** The blocks are the style's wayfinding — each saturated field signals "you are in a different part of the page"; random color changes read as confetti and destroy orientation.
**Example:** Hero on lime, features on off-white, CTA band on pink — three fields, three zones, clear boundaries.

### R3. Commit to a radius system
**Rule:** Pick all-0 (hard, brutalist) or all 12–24px (friendly-brash) and apply it to every bordered element without exception; mixing rounded pills with sharp cards in one view is a defect, not variety.
**Why:** Radius is the biggest single lever between "approachable toy" and "raw machine"; mixing it makes the system look unfinished rather than deliberate.
**Example:** All inputs, buttons, cards, and badges at 16px — or all at 0. Nothing in between, anywhere.

### R4. Black is the glue
**Rule:** Borders, display type, shadows, and icon strokes share the same `#000`; pure black is on-brand here (unlike soft styles) — never substitute dark-gray borders or softened shadows.
**Why:** One shared black fuses loud color blocks into a coherent system; softened blacks read as "neubrutalism with the contrast turned down" and lose the punch.
**Example:** A lime card, pink button, and white input all carry identical 3px `#000` borders and `#000` shadows — the black is the grid holding them together.

### R5. Black ink on saturated fills
**Rule:** Text on color blocks is black by default; white text on saturated fills fails catastrophically (white-on-lime ≈1.3:1, white-on-pink ≈2:1) — white text is allowed only on black or near-black fills that verify ≥4.5:1.
**Why:** The palette is too light for white ink; black-on-lime is ≈16:1 and black-on-pink ≈10:1, so the accessible combination is also the punchier one.
**Example:** "Sign up free": lime fill, 3px black border, black 900-weight label. Never white-on-lime.

### R6. Motion snaps
**Rule:** All motion is 100–150 ms ease-out — the press translate, dropdown opens, toast slides; no springs, no fades past 200 ms, no looping animation.
**Why:** The style's energy is immediate and physical; slow motion makes it sluggish and breaks the mechanical-toy metaphor the press physics establish.
**Example:** Mobile nav opens in 120 ms with the same shadow-press feel; nothing eases in gently.

### R7. Badges mark real states
**Rule:** Stickers and badges — "NEW", "BETA", "SALE", "v2" — always mark an actual state, count, or fact, never pure decoration; every badge is text-labeled (icon-only badges get `aria-label`).
**Why:** In a style this loud a decorative badge is indistinguishable from an important one; tying badges to facts keeps them information, and labels keep them accessible.
**Example:** Pricing card: "MOST POPULAR" sticker on the genuinely popular plan; the other cards stay clean.

### R8. Neubrutalism is not brutalism
**Rule:** Brutalism is anti-design: radius 0 everywhere, raw mono/grotesque type, default blue links, clashing louds on a stark background, a deliberately "unstyled" stance. Neubrutalism is designed playfulness: curated palette, chunky friendly type, allowed radius, a hard-shadow press system. Default links and clashing randoms = brutalism; color system + press physics = neubrutalism.
**Why:** The names collide constantly and the boundary is the style's identity — one refuses design, the other is a design system wearing refusal's wardrobe.
**Example:** Same product, brutalist build: system fonts, default link blue, `#DDD` background. Neubrutalist build: Archivo Black, lime/pink blocks, 4px black borders, press shadows.

## A11y watchpoints
- Hard offset shadows are decoration, not state: state changes also need a border/fill change — and focus is NEVER the shadow alone (add a distinct ring, offset from the 2–4px border, in a different color, ≥3:1)
- White text on saturated fills fails hard: default to black ink; verify any remaining white-text case ≥4.5:1 on its actual fill
- Pure black borders around pure black text zones need surface differentiation: give text zones an off-white fill so the border separates from the content
- The press translate shifts targets by 4–8px: keep taps stable (translate on hover, shadow-press on active) and never move a target away from a pressing finger
- 100–150 ms motion is below noticeable threshold for some users — no essential information may be carried by motion alone (the border/fill change carries it)
- Grayscale test: the CTA must stay findable when blocks go gray — through isolation and size, not just lime

## Checklist
- [ ] Every interactive element: 2–4px black border + hard offset shadow at rest
- [ ] Hover/active = press physics (shadow shrink + translate) plus a non-color state cue
- [ ] One block color per section; 1–3 blocks per view
- [ ] Radius all-0 or all 12–24px, zero exceptions
- [ ] Borders, type, shadows all the same `#000`
- [ ] All text on fills ≥4.5:1 (black-on-color default)
- [ ] Focus ring distinct from the border: offset + different color, ≥3:1
- [ ] Motion 100–150 ms; no springs or loops
- [ ] Badges all mark real states and are text-labeled
- [ ] Body text is plain 16px sans on solid ground

## Anti-patterns
- Soft blurred shadows or glows — instantly generic
- White text on lime/pink/cyan fills
- Focus indicated only by the shadow shrinking — invisible to keyboard users
- Mixed radii: rounded pills next to sharp cards
- Five color blocks in one view — carnival, not system
- Gradient or neon versions of the palette
- Decorative stickers with no meaning crowding real badges
- Gray borders and 80%-black shadows "to soften it"

## Sources & inspiration
- Brutalist architecture — the movement the name borrows: https://en.wikipedia.org/wiki/Brutalist_architecture
- Gumroad — the canonical neubrutalist product: https://gumroad.com
- Archivo Black — the signature display face: https://fonts.google.com/specimen/Archivo+Black
- Brutalist Websites — the raw end of the spectrum (the boundary): https://brutalistwebsites.com
- Neobrutalism components — the design-system incarnation: https://www.neobrutalism.dev
