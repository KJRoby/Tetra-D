# Frontend Guide

Frontends own physical input, rendering, audio, windowing, and presentation
effects. They must keep authoritative gameplay in Dynamo Core.

- Batch ordered commands once per simulation tick, then call `Game::step`.
- Render `GameSnapshot` and react to `GameEvent`; do not recalculate scores,
  collisions, ghost placement, clears, or timers.
- Preserve hidden-row clipping in visual renderers.
- On focus loss, release latched controls and pause if appropriate.
- Pygame can provide key-down/key-up simultaneous input; terminal frontends may
  need a press/repeat fallback because terminals do not reliably provide key-up.
