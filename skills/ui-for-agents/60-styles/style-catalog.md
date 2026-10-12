# Style Catalog — choosing a direction

> **Read when:** starting a new product, site, or major page and the visual direction is not yet chosen. **Section:** 60-styles. **Related:** style-design-process.md, 10/design-tokens.md, 00/visual-hierarchy.md

**Core idea:** choose the style from product personality + audience expectations + content density — then load its preset file and stay inside it. The catalog has two tiers: **product presets** (mood/genre, chosen from the decision table) and **movement presets** (named historical directions like Bauhaus or Art Deco, chosen when the brief names or implies a movement). Both tiers follow the same preset format.

## Selection rules

### R1. Audience expectations first, brand second, taste last
**Rule:** Pick the style the audience already trusts for this product category, then differentiate inside it — not against it.
**Why:** Styles carry genre conventions (fintech = calm and dense); violating them reads as risk, and "unique" that confuses is more expensive than "familiar but well-crafted".
**Example:** A bank landing in brutalism may win an award and lose customers; the same brand's careers page may safely use it.

### R2. Content density constrains the style
**Rule:** Data-heavy surfaces (tables, forms, dashboards) get restrained styles; expressive content (photos, stories, brand moments) can carry expressive styles.
**Why:** Stimulation competes with information for attention; expressive chrome over dense content raises error rates and reading fatigue.
**Example:** A trades dashboard keeps swiss neutrality and lets the marketing site carry the neon.

### R3. Long-session products favor low-stimulation styles
**Rule:** The longer users stay per session, the calmer the style — reserve glow, grain, oversaturated accents for short visits.
**Why:** Visual "loudness" taxes accumulate over hours; short-visit pages are judged by impression, tools by sustained comfort.
**Example:** A 9-hour admin tool in glassmorphism fails in usability tests even when the demo screenshot looks great.

### R4. Marketing may be louder than the product — on purpose
**Rule:** It is legitimate to use an expressive style for landing pages and a restrained one inside the app, provided token families are shared and the boundary is documented.
**Why:** Landing pages must differentiate in seconds; product UI must not exhaust over months — different jobs, different intensity.
**Example:** Neon-glow hero page → same accent hue, swiss app shell.

### R5. When styles compete, decide by cost of misreading
**Rule:** If two candidate styles both fit, choose by what a misinterpretation costs: trust products lean conservative, impression products can gamble.
**Why:** The asymmetry is stark — a playful bank loses deposits, a boring game landing loses nothing but installs.
**Example:** Insurance → corporate-trust; indie game → playful-friendly, even if the client "likes both".

### R6. Named movement in the brief → movement map; otherwise decision table
**Rule:** If the brief names a movement ("Bauhaus", "deco", "vaporwave", "Y2K") or the brand's identity is built on one, go straight to the Movement map and load that preset; otherwise choose from the product decision table.
**Why:** A named movement is a hard requirement with its own rules; the decision table is for when direction is open.
**Example:** "Make our hotel site Art Deco" → `art-deco.md` regardless of the table's first-choice logic.

### R7. Always finish by loading the preset file
**Rule:** After selecting a style here, open its preset file (`60-styles/<name>.md`) and implement from the preset — never from the one-line description in this catalog.
**Why:** The table chooses; only the preset contains the token values, signatures, a11y watchpoints, and anti-patterns that make the style consistent.
**Example:** "We chose dark-premium" without opening the preset leads to pure black + gold text — the exact anti-pattern the preset warns about.

## Decision table — product presets

| Product / content | First choice | Alternatives | Avoid |
|---|---|---|---|
| SaaS dashboard, dev tool, docs | `minimalist-swiss` | `corporate-trust`, `dark-premium` | `neumorphism`, `playful-friendly` |
| Fintech, banking, insurance | `corporate-trust` | `minimalist-swiss` | `brutalism`, `neumorphism` |
| Creative tool marketing site | `editorial` | `dark-premium`, `futuristic-neon` | `corporate-trust` |
| Portfolio, agency, high-craft | `editorial` | `brutalism`, `dark-premium` | `corporate-trust` |
| Entertainment, kids, games | `playful-friendly` | `futuristic-neon` | `corporate-trust`, `neumorphism` |
| Gaming, crypto, web3 | `futuristic-neon` | `dark-premium` | `corporate-trust` |
| Media, fashion, long-form content | `editorial` | `retro-vintage` | `glassmorphism` (text legibility) |
| Health, wellness, food, eco | `handcrafted-organic` | `minimalist-swiss` | `brutalism`, `futuristic-neon` |
| AI / hardware product landing | `dark-premium` | `futuristic-neon`, `minimalist-swiss` | `playful-friendly` (unless kids) |
| Internal enterprise tool | `corporate-trust` | `minimalist-swiss` | `glassmorphism` (eye fatigue over hours) |

## Movement map — named historical directions

When the brief names a movement (or the brand identity is built on one), load its preset directly:

### Avant-garde & ornament (1890–1939)
| Movement | Preset | One-line signature |
|---|---|---|
| Art Nouveau (1890–1910) | `art-nouveau.md` | whiplash organic lines, botanical ornament frames, arch containers, gold on cream |
| Bauhaus (1919–1933) | `bauhaus.md` | circle/triangle/square primitives, primary triad + black, geometric sans, asymmetric balance |
| De Stijl (1917–1931) | `de-stijl.md` | Mondrian grid: orthogonal black lines, white cells, ≤3 primary-colored landmarks |
| Constructivism (1915–1930s) | `constructivism.md` | red/black diagonal wedges, photomontage, condensed uppercase staircase type |
| Art Deco (1920s–30s) | `art-deco.md` | ceremonial symmetry, gold + black + jewel tones, sunburst/fan/chevron motifs |

### Mid-century & counterculture (1945–1990s)
| Movement | Preset | One-line signature |
|---|---|---|
| Mid-Century Modern (1945–69) | `mid-century-modern.md` | atomic-age shapes (amoebas, starbursts), mustard/teal on cream, optimistic illustration |
| Pop Art (1955–70) | `pop-art.md` | halftone dots, thick comic outlines, speech bubbles, silkscreen primaries |
| Psychedelia (1965–75) | `psychedelia.md` | liquid warped type, vibrating complementaries, swirls — hero zones only |
| Punk / DIY zine (1976–90s) | `punk-zine.md` | xerox grain, torn edges, tape, ransom-note headlines, one riot color |
| Memphis Design (1981–88) | `memphis.md` | squiggle confetti, terrazzo, clashing brights on pastel, tilted cards |

### Lifestyle & contemporary aesthetics
| Direction | Preset | One-line signature |
|---|---|---|
| Japanese Zen (Ma / wabi-sabi) | `japanese-zen.md` | double-baseline whitespace, asymmetric balance, sumi ink + washi + one natural hue |
| Dark Academia | `dark-academia.md` | parchment/oxblood/gold, old-style serif everywhere, engraved imagery, candlelit dark mode |
| Maximalism / Cluttercore | `maximalism.md` | layered patterns, 6–8 colors, up to 3 display faces — curated abundance, CTA survives |
| Neubrutalism (2016–) | `neubrutalism.md` | thick black borders + hard shadows + saturated blocks; brutalism that went to art school |
| Acid Graphics (2019–) | `acid-graphics.md` | liquid chrome type, hyper-saturation, glitch/RGB-split — rave energy, hero only |

### UI-era movements (digital design history)
| Movement | Preset | One-line signature |
|---|---|---|
| Skeuomorphism (2000s–2013) | `skeuomorphism.md` | realistic materials (leather, metal, glass), gloss + bevel, physics-honest controls |
| Flat Design / Metro (2013–16) | `flat-design.md` | zero faux-realism; color blocks + typography carry hierarchy — the modern default paradigm |
| Corporate Memphis (2017–) | `corporate-memphis.md` | flat vector humans with exaggerated limbs; illustration-led, quiet UI chrome |
| Claymorphism (2021–) | `claymorphism.md` | puffy double-inner-shadow surfaces, pastel candy palette, big radii, 3D clay props |

### Web-nostalgia revivals (major 2020s trends)
| Movement | Preset | One-line signature |
|---|---|---|
| Y2K / Chrome (1997–2004) | `y2k-chrome.md` | liquid chrome type, iridescent bubbles, translucent tech-blue, lens flares |
| Frutiger Aero (2004–13) | `frutiger-aero.md` | glossy aqua buttons, nature imagery, blue-green + white, humanist sans |
| Vaporwave / Synthwave | `vaporwave-synthwave.md` | vapor: pastel pink/cyan + busts + VHS glitch; synth: purple night, sunset grid, neon chrome |

### Niche directions — no preset; blend from listed parents
Some recognized directions rarely justify a full preset. Blend from the named parents and document the mix via style-design-process.md:
- **Cyberpunk** → `futuristic-neon` + punk-zine grunge textures + acid distortion (hero zones)
- **Space Age / Googie** → inside `mid-century-modern` (starbursts, boomerangs are its vocabulary)
- **Steampunk** → `skeuomorphism` materials (brass, leather, gears) + `dark-academia` palette
- **Gothic** → `dark-academia` type (blackletter accents) + stark contrast; display-only lettering
- **Scandinavian / Japandi** → `minimalist-swiss` layout + `japanese-zen` warmth + `handcrafted-organic` materials
- **Industrial / loft** → `brutalism` structure + `skeuomorphism` concrete/metal materials
- **Kitsch / camp** → `maximalism` + `memphis`, self-aware tone documented in microcopy
- **Bento grids, aurora gradients** — layout/gradient *techniques*, not styles: see `00-fundamentals/spacing-layout-grids.md` and `00-fundamentals/color.md`; combinable with any preset

## The product presets

- `minimalist-swiss.md` — precise, calm, systematic; the library's baseline.
- `dark-premium.md` — near-black layers, one metallic or jewel accent, refined type.
- `glassmorphism.md` — frosted translucent panels over vivid backgrounds.
- `neumorphism.md` — soft extruded/pressed same-hue surfaces (strict a11y limits).
- `brutalism.md` — raw, honest, harsh borders and type; anti-corporate.
- `editorial.md` — magazine typography, asymmetric grid, photography.
- `playful-friendly.md` — rounded, saturated, illustrated, springy motion.
- `corporate-trust.md` — conservative blue, dense, tables-first, B2B credibility.
- `retro-vintage.md` — era palettes and type, texture, nostalgia (70s/80s/90s).
- `futuristic-neon.md` — dark base + neon glow, HUD decorations.
- `handcrafted-organic.md` — warm cream, earth tones, organic shapes, illustration.

## Checklist
- [ ] Movement named in the brief? → Movement map row found and preset loaded
- [ ] No movement named? → product located in the decision table; avoid-column respected
- [ ] Audience expectation checked before personal taste
- [ ] Content density of the densest screen compatible with the style
- [ ] Session length accounted for (calm for long sessions)
- [ ] Style boundaries (marketing vs app; expressive hero vs calm core) documented if different
- [ ] Adjacent-preset boundary from the preset's own "vs" note respected
- [ ] Preset file opened and being followed

## Anti-patterns
- Choosing a style because it is trendy, not because it fits
- Applying an expressive style to the densest screen of the product
- Two competing styles inside one app surface
- Blending two named movements casually (e.g. "Bauhaus + vaporwave") without a documented token-level merge via style-design-process.md
- Deciding from this catalog's one-liner without opening the preset
- Forcing one style across products with different audiences
- Recreating a movement's period defects (racist caricatures, unreadable type sizes) as "authenticity"

## Sources
- Nielsen Norman Group — aesthetic-usability effect: https://www.nngroup.com/articles/aesthetic-usability-effect/
- Laws of UX — Jakob's law (familiar patterns): https://lawsofux.com/jakobs-law/
- Material Design 3 — expressive vs functional tiers: https://m3.material.io/foundations
- CARI — Consumer Aesthetics Research Institute (digital aesthetic eras): https://cari.institute
- Web Design Museum — archived web-era artifacts: https://www.webdesignmuseum.org
