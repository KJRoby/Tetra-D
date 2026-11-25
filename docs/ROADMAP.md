# Roadmap

## Implemented

- Deterministic Rust simulation at 1,000 Hz.
- Guideline mechanics, seven-bag, SRS, hold, T-spins, scoring, combo/B2B, and
  Marathon plus fixed-speed 40-line Sprint.
- Ratatui terminal and PyO3/Pygame frontends sharing the same simulation.
- Native Macroquad desktop frontend sharing the same simulation directly.
- Configured Guideline/Marathon defaults and expanded deterministic state hash.
- Shared ready/start/resume countdown phases and pause-menu lifecycle across
  all maintained frontends.
- Optional frontend seed selection and locked-seed restarts for deterministic
  reproduction.
- Versioned JSON replay recording, playback, verification, and a Macroquad
  completed-game save flow.

## Current limitations

- Only Guideline mechanics and the Marathon/Sprint mode presets are implemented.
- Board dimensions are fixed at 10×22 with two hidden rows.
- Replay seeking, speed controls, and non-Macroquad replay browsers do not yet
  exist.
- Several engine concepts remain implementation-private pending the next module
  extraction milestone.

## Planned milestones

1. Extract pieces, rotation, randomizer, timing, scoring, events, and
   snapshots from `engine` into dedicated implementation modules.
2. Add fixture-based behavior-equivalence and long deterministic replay tests.
3. Add additional enum-based ruleset and mode presets.
