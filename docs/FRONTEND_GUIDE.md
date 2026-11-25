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
- The native Macroquad frontend uses the core directly and reconciles held keys
  each frame before batching them into fixed simulation ticks.
- Treat `Ready`, `StartCountdown`, and `ResumeCountdown` as presentation
  phases: do not render well cells until `Falling` or `Clearing`. Pause menus
  may issue only `Resume`, `Restart`, or `ReturnToTitle` actions.
- Frontends choose session seeds outside the core and call
  `start_with_seed`/`restart_with_seed`; a chosen seed is part of the read-only
  snapshot and can be displayed for replay/debugging.
- Build sessions from a selected `GameConfig`; the shipped Solo choices are
  Guideline Marathon and 40-line fixed-speed Guideline Sprint. Restart must
  preserve the active configuration.
- Display elapsed mode time from `Timers::mode_elapsed_ticks` as
  `HH:MM:SS:cc`. This includes falling and clear-delay time but excludes
  ready/countdown and paused phases.
- A recording frontend must own the simulation through `ReplayRecorder` and
  submit only core command batches/lifecycle actions. Do not record physical
  key events, rendering time, or derived gameplay outcomes.
- Playback frontends must advance `ReplayPlayer` only; viewer pause must freeze
  presentation without submitting commands to the recorded simulation.
