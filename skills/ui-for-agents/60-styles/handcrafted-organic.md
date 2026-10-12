# Style: Handcrafted Organic

> **Read when:** building UI in the handcrafted-organic style. **Section:** 60-styles. **Related:** style-design-process.md, style-catalog.md, 00/color.md, 00/typography.md, 00/iconography.md

**Personality:** warm, natural, human, grounded. **Best for:** wellness, food, eco/artisan brands, education, community. **Avoid for:** trading tools, gaming, crypto, data-dense enterprise.

**Core idea:** the interface feels made by a person — warm paper, brown ink, soft organic shapes, botanical illustration — while spacing, structure, and contrast stay strictly systematic, because craft is discipline, not mess.

## Signature (what makes it recognizable)
- Warm cream paper surfaces (#FAF6EF-family); pure white and pure black appear nowhere
- Earth-tone palette: warm dark-brown ink, terracotta, sage, ochre mapped to fixed roles
- Humanist serif display + warm sans body; never geometric-cold type
- Organic shapes from a fixed variant set: 12–20px radii, mixed per corner, 2–3 blob containers
- Visible paper/canvas grain at 3–6% opacity — under chrome, never under body text
- Illustration-forward: hand-drawn botanicals and spot illustrations at narrative moments
- Gentle motion: soft 250–400 ms fades, nothing springs or bounces

## Token preset (starting point — adjust per brand, keep the direction)

| Token | Direction | Typical value |
|---|---|---|
| `color-bg` | warm cream paper | #FAF6EF family; alternating sections #F6F0E4 |
| `color-surface` | white-warm cards | #FFFDF8 on cream, or deeper cream #F3ECDF |
| `color-ink` | warm dark brown, never #000 | #3E2F23 text; muted #6B584A secondary (verified 4.5:1 on cream) |
| `color-accent` | terracotta or sage | #C05B3C-family: ≥3:1 on cream for UI; dark variant #A04A2F for text |
| `palette-support` | ochre, sage, clay | tags, illustration, borders — not text |
| `radius-*` | irregular-friendly | 12–20px; 2–3 mixed-corner variants reused sitewide |
| `shadow-*` | warm-tinted, very soft | 0 2px 12px rgba(90,60,30,0.08); one layer max |
| `space-scale` | generous | 8pt scale, +1 step vs default; section gaps ≥48px |
| `type-display` | humanist serif | Fraunces-class warm serif; scale ~1.333 |
| `type-body` | warm sans | 16px, line-height 1.5–1.6, measure 45–75ch |
| `texture-*` | paper grain, under chrome | 3–6% opacity layer below content, absent behind text |
| `motion-duration` | soft fades | 250–400 ms ease-in-out; no springs, no bounce |

## Rules

### R1. Ink is warm, never black
**Rule:** All text and icons use warm dark brown (#3E2F23-family); pure #000 and cool grays are banned, including in shadows and borders.
**Why:** Pure black on cream vibrates and kills the hand-made feel; warm ink is what makes the same layout read as "crafted" instead of "default".
**Example:** Heading #3E2F23, body #4A3B2A, borders #D8C9B4 — no #000/#333 anywhere, including box-shadows.

### R2. Organic shapes follow a system
**Rule:** Define 2–3 shape variants (e.g. card radius 16/20/12/20, blob-A, blob-B) and reuse them; never invent a new asymmetric radius or blob per element.
**Why:** Randomized organic shapes read as sloppy, not handmade; repeated variants are what make irregularity feel authored.
**Example:** Every image uses blob-A, every card the 16/20/12/20 radius; the 404 page reuses blob-B instead of introducing blob-C.

### R3. Illustration marks moments; chrome stays calm
**Rule:** Hand-drawn botanicals and spot illustrations appear at narrative moments — empty states, success, onboarding story, section interludes — while navigation, forms, and tables stay plain.
**Why:** Illustration carries warmth where users have time to feel it; scattered through functional chrome it becomes noise and slows tasks.
**Example:** A sprig illustration welcomes the empty journal list; the "Add entry" form beside it has no decoration at all.

### R4. Texture under everything, text above it
**Rule:** Paper grain sits at 3–6% opacity as a background layer on chrome and hero zones; every text block sits on solid, texture-free backing.
**Why:** Texture is the style's tactile signature — but under glyphs it makes edges fuzzy and contrast unpredictable; the fix is layering, not removal.
**Example:** The hero band carries visible canvas weave; the article body below sits on plain #FAF6EF with no overlay.

### R5. Photography in natural light only
**Rule:** Photos are shot or graded in natural, warm light — no studio flash, no neon, no oversaturated filters — and framed with the organic radius variants.
**Why:** Harsh artificial light contradicts the grounded, natural promise in one glance; warm-graded photos blend into the cream world.
**Example:** A workshop photo in window light inside the 16/20/12/20 frame; never a black-and-white moody grade.

### R6. Motion is a breeze, not a spring
**Rule:** All movement is soft fades and gentle slides at 250–400 ms ease-in-out; no overshoot, no bounce, no elastic scrolling, nothing loops.
**Why:** Springs read as toy-like and synthetic — the opposite of grounded; slow soft motion is what makes transitions feel calm and human.
**Example:** A card fades up 300 ms ease-in-out on reveal; the confirmation check draws once over 400 ms and stops.

### R7. Earth tones never carry meaning alone
**Rule:** Sage "success" or terracotta "error" always pair with an icon and a text label; hue is reinforcement, never the sole signal.
**Why:** Earth palettes compress the red/green axis that color-blind users rely on for status; without labels, success and error become indistinguishable.
**Example:** Error = terracotta border + warning icon + "Payment failed" text — not a reddish tint alone.

### R8. Warm does not mean low contrast
**Rule:** Verify every text pair against the actual cream background (#FAF6EF), not white; muted browns that pass on #FFF often fail on cream — darken the ink instead of lightening the mood.
**Why:** Cream lowers contrast versus white, and warm muted tones start lower still; this is the style's most common silent failure.
**Example:** #8A7360 "muted" text fails on cream → switch secondary text to #6B584A, which clears 4.5:1.

### R9. A human-touch budget
**Rule:** Cap hand-drawn accents (arrows, underlines, doodles) at 1–2 per view, chosen from a small reusable set; everything else stays clean structure.
**Why:** The hand feel works by scarcity — a few deliberate human marks signal a maker, dozens signal a scrapbook.
**Example:** One hand-drawn underline on the page title; forms and lists get none.

### R10. Generous space is the craft signal
**Rule:** Use the airy end of spacing: section gaps ≥48px, card padding 24px+, text measure 45–75ch; never compress the layout to fit more decoration.
**Why:** Crowded organic layouts read as cluttered market stalls; whitespace lets the warmth and shapes register as intentional.
**Example:** A wellness landing with 64px between story sections and one illustration each — nothing touching.

## A11y watchpoints
- Warm-gray-on-cream fails silently: verify 4.5:1 against the real #FAF6EF bg (not white) — darken inks rather than trusting "it looked fine on white"
- Terracotta/sage accents need 3:1 on cream for UI components (borders, icons, controls) and 4.5:1 when used as text
- Earth-tone status colors collide for red-green color-blind users — always pair hue with icon + text (WCAG 1.4.1)
- Texture never sits behind text; blob containers must not clip focus rings — keep outlines fully visible and targets ≥24×24px
- Serif display faces lose detail at small sizes: keep display ≥24px and never use it for labels or buttons

## Checklist
- [ ] No pure #000 text or #FFF surfaces anywhere
- [ ] All text pairs ≥4.5:1 measured against the actual cream bg
- [ ] Accent UI components ≥3:1 on cream; status colors paired with icon + text
- [ ] 2–3 shape variants reused; no per-element random blobs
- [ ] Texture ≤6% opacity and never behind body text
- [ ] Display serif ≥24px; body in the warm sans at 45–75ch
- [ ] Motion 250–400 ms ease-in-out; zero bounce/spring/loops
- [ ] ≤2 hand-drawn accents per view, from the reusable set
- [ ] Illustration at moments only; forms, tables, chrome undecorated
- [ ] Focus rings fully visible on rounded/blob containers; targets ≥24×24px

## Anti-patterns
- Pure black headings on pure white cards — an organic palette with a cold core
- A different random blob on every element; nothing repeats
- Grain or canvas texture running directly under paragraphs
- Sage-on-cream or terracotta-on-cream text below contrast floors
- Kraft-paper overload: brown on brown until everything is low-contrast mud
- Bouncy spring animations "for charm"
- Harsh studio-flash stock photos dropped into a natural-light world
- Doodles, arrows, and stickers on every section until it is a scrapbook

## Sources & inspiration
- Earth tone — the palette family and its logic: https://en.wikipedia.org/wiki/Earth_tone
- Fraunces — reference humanist "wonky" serif: https://fonts.google.com/specimen/Fraunces
- WCAG 2.2 — 1.4.11 Non-text contrast (the 3:1 floor): https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html
- WCAG 2.2 — 1.4.1 Use of color (status needs more than hue): https://www.w3.org/WAI/WCAG22/Understanding/use-of-color.html
- Botanical illustration — the illustration tradition to borrow from: https://en.wikipedia.org/wiki/Botanical_illustration
- Material Design 3 — shape as a system with variant discipline: https://m3.material.io/foundations
