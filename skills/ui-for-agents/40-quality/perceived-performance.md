# Perceived Performance

> **Read when:** any wait, load, or async operation exists in the UI. **Section:** 40-quality. **Related:** 30-ux-patterns/empty-loading-error-states.md, 00-fundamentals/motion-principles.md, 20-components/feedback-toasts-alerts.md, usability-heuristics.md, visual-qa-protocol.md

**Core idea:** felt speed is a design property, not only an engineering one. Users judge latency against response tiers and by how the wait is presented. Two apps with identical timings can feel seconds apart — the difference is what the UI does during the wait.

## Rules

### R1. Under 0.1 s: act instantly, show nothing
**Rule:** Responses under 0.1 s feel instantaneous — no spinner, no artificial pacing; the micro-motion range (150–300 ms) applies to real state changes, never as added delay.
**Why:** Below ~100 ms the action and its effect feel causally connected; adding an indicator to an instant operation makes it feel slower and trains users to watch chrome instead of content.
**Example:** Toggling a checkbox, opening a dropdown, press highlight: state changes immediately, no feedback animation layered on top.

### R2. Around 1 s: confirm the action, keep the flow
**Rule:** For responses between 0.1 s and 1 s, show the state change immediately (pressed, "Saving…", row inserted as pending) without demanding attention; skeletons are acceptable here.
**Why:** Under 1 s users keep their train of thought — the design job is to confirm the input registered, not to entertain; a spinner flashing for 80 ms is noise, and silence feels broken.
**Example:** "Save" becomes "Saving…" for 600 ms, then "Saved" — no modal, no progress bar, no toast for an operation that short.

### R3. Approaching 10 s: progress, ETA, or background it
**Rule:** Operations approaching 10 s need determinate progress with phase labels where possible; beyond that, move the work to the background, let the user keep working, and notify on completion.
**Why:** Attention on an unexplained wait decays within seconds; a labeled progress ("Uploading 3 of 10…") converts dead time into expectation, and backgrounding restores control (usability-heuristics.md R3).
**Example:** An export runs with a determinate, cancelable bar; at completion a toast appears — the user never stared at a frozen screen deciding whether it died.

### R4. Optimistic UI for quick mutations — with a defined rollback
**Rule:** For low-risk mutations expected to finish under ~1 s, apply the change immediately and reconcile with the server response; define the rollback path (revert + error + retry) before shipping the optimism.
**Why:** Waiting for the server to echo a change the user already decided on is wasted felt time; optimism drops perceived latency to zero — but without rollback it converts into silent data loss on failure.
**Example:** Like/archive/toggle updates the row instantly; on request failure the row reverts and an inline error with Retry appears — never a stuck fake-success state.

### R5. Skeletons communicate structure for known layouts
**Rule:** When the layout of incoming content is known, show placeholder blocks in the final structure (skeleton), not a generic spinner; fall back to a simple indicator only when the layout is genuinely unknown. (See 30-ux-patterns/empty-loading-error-states.md)
**Why:** A skeleton previews the page's shape, so the swap-in reads as faster and causes no layout shift; a spinner carries zero structural information and centers attention on the wait itself.
**Example:** An article list renders 6 gray text-line blocks with avatar circles, replaced in place by real rows — same geometry, no jump.

### R6. Never fake progress
**Rule:** Progress bars move only with real information: bytes, items, completed steps. Never animate toward an assumed end, never invent durations, never park at 99%.
**Why:** A fake bar that completes while work continues — or stalls at 99% — is eventually discovered, and discovered dishonesty destroys trust in every future indicator, exactly when the UI needs to be believed.
**Example:** A step-based import shows "Step 2 of 4" advancing only as steps truly complete; a single unknown-duration request gets an indeterminate indicator instead of a lying percentage.

### R7. Motion may mask latency, never extend it
**Rule:** Short transitions (150–300 ms) may cover small perceptible delays (a shimmer during a 200 ms fetch); they must never delay when real work starts or when the result is shown.
**Why:** Motion buys felt smoothness by occupying the perceptual gap; stretching animation beyond the actual wait adds artificial latency — the exact opposite of the goal.
**Example:** A shimmer plays while data loads and disappears the instant data arrives; it does not run "one more loop" for visual balance.

### R8. First meaningful paint is not interactive
**Rule:** Do not count a view as loaded until it responds to input; if painted content cannot accept interaction yet, block visibly and briefly or disable with a reason — never let clicks vanish silently.
**Why:** A beautiful frame that ignores the first click feels broken and produces double-submits and rage clicks; users judge readiness by whether the UI answers, not by whether it drew.
**Example:** During initialization the submit button renders disabled with "Preparing…" instead of accepting a click that will be dropped.

### R9. Warm the likely next step
**Rule:** Where the platform allows, prefetch data and assets for the most probable next view or step while the user is still on the current one.
**Why:** The cheapest latency win is work done before it is requested; a correctly predicted next step converts a 1 s wait into an instant one without changing any server behavior.
**Example:** A wizard prefetches step 2's option list when step 1 validates; a list view warms the detail payload of the top visible rows.

### R10. Instrument before optimizing
**Rule:** Measure real timings at perceived entry points (navigation, interaction, completion) before changing anything; optimize the largest measured cost, not the suspected one.
**Why:** Felt slowness is frequently one blocking call, one unbatched loop, or one synchronous layout — guessing optimizes the wrong 10%; instrumentation turns debate into a sorted list.
**Example:** Profiling shows the "slow dashboard" spends 900 ms on one uncached config request, not on chart rendering — the fix is one cache header, not a renderer rewrite.

### R11. Batch and debounce input-driven work
**Rule:** Work triggered by rapid input (typing, scrolling, resizing) is debounced or throttled; intermediate requests are canceled cleanly or superseded atomically.
**Why:** Firing a full request cycle per keystroke makes the UI feel sluggish and produces out-of-order responses; collapsed bursts keep latency invisible and results consistent.
**Example:** Search-as-you-type waits 200 ms after the last keystroke and cancels the previous request; a chart resizes on throttle ticks, not per pixel.

## Tier cheat sheet

| Felt duration | Presentation |
|---|---|
| < 0.1 s | Immediate state change; no indicator |
| 0.1–1 s | State change + inline pending marker; skeleton allowed |
| 1–10 s | Determinate progress (or skeleton) + cancel where sensible |
| > 10 s | Background work + notify on completion |

## Checklist
- [ ] Every async operation classified into the 0.1 s / 1 s / 10 s tiers with a matching presentation
- [ ] No spinner shown for operations that complete under 0.1 s
- [ ] Long operations have determinate progress (or background + notify) and are cancelable where sensible
- [ ] Optimistic mutations have a tested rollback path
- [ ] Skeletons match the final layout geometry (no shift on swap-in)
- [ ] No artificial or fake progress anywhere; indeterminate used where duration is unknown
- [ ] The first interactive frame responds to input; no silently dropped clicks
- [ ] Transitions are 150–300 ms and never extend the actual wait
- [ ] Rapid input (typing/scroll/resize) is debounced; stale responses canceled
- [ ] Timings instrumented; the last optimization was chosen from measured data

## Anti-patterns
- A full-screen spinner for a 300 ms request
- Progress bars that jump to 90% instantly, then stall at 99%
- Toast spam for sub-second operations ("Saved!" on every keystroke-save)
- Optimistic UI without rollback — fake success on failure
- A skeleton that does not match the arriving layout, shifting content on load
- Artificial sleeps or throttles added "so the spinner is visible"
- A clickable button during initialization that silently drops the click
- Optimizing rendering because "it must be the charts" with zero measurements
- Keystroke requests racing each other with the stale one winning

## Sources
- Nielsen Norman Group — Response times: the 3 important limits (0.1 / 1 / 10 s): https://www.nngroup.com/articles/response-times-3-important-limits/
- Nielsen Norman Group — Progress indicators and pacing: https://www.nngroup.com/articles/progress-bars/
- Nielsen Norman Group — Website response times: https://www.nngroup.com/articles/website-response-times/
- web.dev — Core Web Vitals (LCP, INP): https://web.dev/articles/vitals
- Laws of UX — Doherty threshold: https://lawsofux.com/doherty-threshold/
- Chrome DevTools — Performance instrumentation: https://developer.chrome.com/docs/devtools/performance
