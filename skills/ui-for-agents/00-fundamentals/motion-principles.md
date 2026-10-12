# Motion Principles

> **Read when:** animating any UI state change, transition, or feedback. **Section:** 00-fundamentals. **Related:** 00/elevation-depth.md, 20/feedback-toasts-alerts.md, 40/perceived-performance.md, 40/accessibility-wcag.md

**Core idea:** motion explains change — what reacted, where things came from, what to look at. If an animation can be deleted without losing information, it is decoration and should be deleted.

These rules apply to CSS/WAAPI transitions, native toolkit animation APIs, and hand-rolled interpolation in canvas or WASM renderers alike.

## Rules

### R1. Motion must inform, never decorate
**Rule:** Every animation answers one question: what changed, what caused it, or where did it go (causality, feedback, orientation). Nothing ships purely "for polish".
**Why:** Motion consumes attention and time; informative motion guides the eye and builds a mental model of the UI's spatial logic, while decorative motion trains users to wait and ignore.
**Example:** A dialog scales from the button that opened it (orientation: it came from there); a button ripple with no causal meaning is a candidate for removal.

### R2. Micro-interactions 150–300ms; large transitions up to 300–500ms
**Rule:** Hover, press, toggle, and fade micro-interactions run 150–300ms; full-screen or multi-element transitions may use 300–500ms. Nothing essential runs longer.
**Why:** Below ~100ms motion is invisible (reads as a glitch); above ~500ms users perceive waiting — the range keeps feedback perceivable but faster than thought.
**Example:** Checkbox check 150ms, dropdown expand 200ms, page-level panel slide 350ms; a 1s toast fade-out is reduced to 300ms.

### R3. Easing by direction: out for entrances, in for exits, in-out for travel
**Rule:** Use ease-out curves for elements entering, ease-in for elements leaving, and ease-in-out for elements moving between two visible points; never plain linear for UI transitions.
**Why:** Entering objects must decelerate to feel responsive (fast start), exits accelerate away, and point-to-point movement needs acceleration at both ends — linear motion reads mechanical and unnatural.
**Example:** A menu opens with ease-out over 200ms and closes with ease-in over 150ms; a dragged card uses ease-in-out while snapping into place.

### R4. Animate only transform and opacity where the platform allows
**Rule:** Restrict animation to position/transform and opacity channels; avoid animating layout properties (width, height, top/left, font-size) in DOM and canvas systems.
**Why:** Transform and opacity are handled by the compositor and stay on the fast path; layout animations force reflow or re-rasterization every frame and drop frames on mid-range hardware.
**Example:** A sidebar collapses via horizontal translation plus an opacity crossfade — not by animating its width and re-laying-out the page each frame.

### R5. Choreograph groups with a 20–50ms stagger
**Rule:** When several related items animate together, offset each by 20–50ms in sequence; never fire all at once, and never stagger longer.
**Why:** A slight stagger preserves group unity while making each item perceivable — simultaneous motion blurs into one mass, and long staggers (100ms+) make lists feel slow and gimmicky.
**Example:** A 10-row list entering: rows fade/slide in at 30ms intervals — total choreography stays brief and scannable.

### R6. Motion must be interruptible and retarget gracefully
**Rule:** Users can interact with anything mid-animation; the animation re-targets from the current state instead of snapping back or restarting.
**Why:** Non-interruptible animations block input and force waiting, and restarts double the wait; graceful retargeting is what makes 150–300ms motion feel fluid rather than film-like.
**Example:** A user re-opens a collapsing accordion halfway: it expands from its current height, not from fully collapsed; hover state changes blend instead of replaying.

### R7. Honor the reduced-motion preference
**Rule:** When the platform signals reduced motion, disable non-essential motion (slides, scale, parallax, auto-playing loops); keep short opacity fades (≤ 200ms) and instant state changes.
**Why:** Vestibular disorders make large movement physically nauseating (WCAG 2.3.3, Animation from Interactions); the information must survive as state change, not as trajectory.
**Example:** With reduced motion on: dialogs appear/disappear with a 150ms fade and no slide; skeletons stop shimmering; carousels stop auto-advancing.

### R8. Don't animate what must be instant
**Rule:** Responses that must feel immediate — keypress echo, toggle selection, drag tracking — happen within ≤ 100ms with no animation; motion budgets apply to state transitions, not to input acknowledgment.
**Why:** Perceived instant feedback requires causality inside ~100ms; inserting an animation between action and acknowledgment makes the UI feel laggy even at high frame rates. See `40-quality/perceived-performance.md`.
**Example:** Typing in a text field and dragging a slider: zero animation delay. The panel that opens after the click: 200ms.

### R9. Scale duration with distance and size of change
**Rule:** Bigger or farther movements get proportionally more time within the allowed ranges; identical durations across differently-sized transitions read as arbitrary.
**Why:** Constant velocity reads unnatural — real objects accelerate; matching duration to distance preserves the illusion of mass while staying inside the perceivable band.
**Example:** A tooltip fades in 150ms; the same content sliding 400px into view takes 300–400ms — not both at 200ms.

### R10. Loops must be finite or user-controlled
**Rule:** No infinite ambient motion (pulsing badges, bouncing hints, auto-advancing carousels) without user control and a way to stop it; looping indicators (spinners) are the only standing exception.
**Why:** Continuous motion keeps pulling peripheral attention, prevents reading, and compounds into an accessibility barrier; WCAG 2.2.2 requires pause/stop/hide for moving content.
**Example:** A "tour" pulse plays three times and stops; a carousel advances only on user input; the save spinner stops the moment the result arrives.

### R11. One motion grammar across the product
**Rule:** The same interaction animates the same way everywhere — same duration, same curve, same direction for the same trigger type, declared in a motion token set.
**Why:** Users learn the motion vocabulary like any other pattern; per-screen variations read as different teams' work and slow recognition of what just happened.
**Example:** Every panel in the product slides from the same edge at 300ms ease-out; a one-off bounce on a single modal is a defect, not personality.

### R12. Prefer shorter over janky
**Rule:** On targets that cannot hold steady frames (low-end hardware, heavy canvas scenes), shorten or cut the animation — a 250ms animation rendered in three janky frames is worse than an instant switch.
**Why:** Broken motion reads as lag and damages perceived performance more than no motion; frame budget, not design taste, decides the final duration on constrained renderers.
**Example:** A WASM/canvas UI measures its stable frame budget: where frames drop, durations drop to ≤ 150ms or the change becomes instant.

### R13. Direction carries the navigation model
**Rule:** Tie entry/exit direction to navigation semantics — new layers enter from the flow's edge, dismissed layers exit back the way they came — and keep those directions consistent product-wide.
**Why:** Direction is orientation information; consistent directions make the UI's spatial model learnable, while random directions force users to re-orient on every transition.
**Example:** Forward navigation slides content in from the trailing edge and back slides it out; a dialog always scales from its trigger — never from a random corner.

## Checklist
- [ ] Every animation has a stated informative purpose (causality/feedback/orientation)
- [ ] Micro-interaction durations within 150–300ms; large transitions ≤ 500ms
- [ ] Entrances ease-out, exits ease-in, point-to-point ease-in-out; no linear UI transitions
- [ ] Only transform/opacity animated (or a documented platform exception)
- [ ] Group entrances staggered 20–50ms with brief total choreography
- [ ] All animations interruptible mid-flight and retargeting from current state
- [ ] Reduced-motion preference honored: non-essential motion off
- [ ] Input acknowledgment (typing, toggling, dragging) instant — ≤ 100ms, no animation added
- [ ] Duration proportional to distance/size of the change
- [ ] No uncontrolled infinite loops; looping content can be paused or stopped

## Anti-patterns
- Animations "for polish" with no informational content
- 700ms+ micro-interactions; staggered lists that take seconds to finish
- Linear easing everywhere
- Animating width/height/top/left in hot paths
- Entrance animations replaying on every focus change or rerender
- Spinners replacing instant responses (a toggle that "spins" to activate)
- Parallax and auto-looping motion that ignore reduced-motion
- A different animation for the same interaction on different screens

## Sources
- Material Design 3 — Motion, easing & duration: https://m3.material.io/styles/motion/overview
- Apple HIG — Motion: https://developer.apple.com/design/human-interface-guidelines/motion
- WCAG 2.2 — 2.3.3 Animation from Interactions & 2.2.2 Pause, Stop, Hide: https://www.w3.org/TR/WCAG22/
- Nielsen Norman Group — Animation for attention and comprehension: https://www.nngroup.com/articles/animation-purpose-ux/
- web.dev — Animations guide (compositor-friendly properties): https://web.dev/animations/
