# Roadmap

## Implemented

- Deterministic Rust simulation at 1,000 Hz.
- Guideline mechanics, seven-bag, SRS, hold, T-spins, scoring, combo/B2B, and
  Marathon.
- Ratatui terminal and PyO3/Pygame frontends sharing the same simulation.
- Configured Guideline/Marathon defaults and expanded deterministic state hash.

## Current limitations

- Only Guideline and Marathon presets are implemented.
- Board dimensions are fixed at 10×22 with two hidden rows.
- Replay recording and playback do not yet exist.
- Several engine concepts remain implementation-private pending the next module
  extraction milestone.

## Planned milestones

1. Extract pieces, rotation, randomizer, timing, scoring, events, and
   snapshots from `engine` into dedicated implementation modules.
2. Add fixture-based behavior-equivalence and long deterministic replay tests.
3. Add additional enum-based ruleset and mode presets.
