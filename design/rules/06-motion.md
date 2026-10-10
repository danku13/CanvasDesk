# 06 — Motion and animation

> Tokens: `design/tokens/motion.json` ↔ `canvas_core::tokens` (ms).
> Mechanics: `crates/canvas-ui/src/anim.rs`.

## M1. Duration scale

| Token | ms | What it does |
|---|---|---|
| focus_fade_ms | 150 | fading of non-focused elements (T23), body block flip, description clamp |
| camera_flight_ms | 300 | camera flight, ease-out |
| spill_wave_edge_ms | 600 | pulse of one edge of the spill-cascade wave |
| spill_wave_step_ms | 200 | wave step between topological orders of edges |
| result_pulse_ms | 1200 | pulse of the fresh-result border |
| focus_breath_ms | 1600 | "breathing" of the focused edge |
| show_source_ms | 2500 | "Show source": highlighting of the source/edge/receiver with dimming |
| body_block_flip_ms | 150 | collapse transition of the statement block |
| body_clamp_ms | 150 | transition of the description clamp «⋯ целиком ▾» ("⋯ full ▾") |

Selection rule: input reaction — 150; view movement/large transitions —
300; waves/ripples — 600–2500. An off-scale duration is a token edit,
not a local constant.

## M2. Mechanics

- `animate_value(from, to, t01)` — linear interpolation; the curve (ease-out
  for the flight) is set by the consumer, not the animator.
- `BoolAnim` — a 0..1 toggle, step `dt · speed_per_sec`.
- **dt-determinism**: animations are computed from the frame delta, no access
  to global clocks — the same frame = the same picture (reproducibility
  of the golden geometry and the tests).

## M3. Principles

1. Only what helps understanding is animated (direction highlighting,
   the source of a value, result freshness). No decorative animations.
2. Every movement has a semantic duration from M1; interruption
   by input is allowed (a click/pan cancels the flight).
3. No animation is also a state: if a token = 0, the transition is instant
   (important for tests and screenshots).
4. Interface delay times outside the motion tokens (their own groups):
   tooltip 500 ms, toast TTL 3000 ms, palette flyout opening 150 ms /
   closing 300 ms, search debounce 200 ms, double-click 500 ms & ≤5 px.
5. Kit components are static (the 2026-10-09 audit): ST1 state transitions
   are instant, animations are on the consumer/surface side (dt-determinism
   M2 there as well). The kit keeps no timers and no dt.
