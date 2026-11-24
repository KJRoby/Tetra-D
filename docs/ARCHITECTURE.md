# Architecture

## Implemented

`tetradynamo-core` is the authoritative deterministic simulation. `Game` owns
the board, active piece, input latches, PRNG state, timers, scoring state, and
mode progress. `rules` contains mechanical configuration and `mode` contains
objectives/progression. The terminal, Macroquad, and Python/Pygame frontends submit ordered
commands and render snapshots/events; neither owns gameplay state.

Current core modules:

- `board`: fixed board storage, collision, locking, and line operations.
- `engine`: simulation orchestration and phase transitions.
- `rules`: mechanical ruleset data and Guideline defaults.
- `mode`: Marathon objective/progression data.
- `lib`: stable public facade and composed `GameConfig`.
- `replay`: versioned recording, sparse playback, JSON validation, and
  headless deterministic verification.

## Data flow

`frontend input -> Command batch -> ReplayRecorder/Game::step -> events + snapshot -> frontend rendering`.
One `step` is always one 1,000 Hz simulation tick.

## Planned refinement

Piece geometry, rotation tables, randomization, timing, scoring, events, and
snapshots are intentionally isolated concepts inside `engine`; extracting their
implementation into dedicated modules is the next structural milestone.
