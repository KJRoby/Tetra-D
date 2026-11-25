# Replay System

Tetra|D replays are versioned JSON (`.tdr.json`) files owned by Dynamo Core.
They record an initial seed, complete configuration, sparse tick-indexed
command batches, lifecycle operations, event hashes, state-hash checkpoints,
and the final result. They never record derived gameplay state.

## Recording and verification

`ReplayRecorder` owns a `Game` while recording. Frontends submit commands and
lifecycle actions through it, then save `recorder.replay().to_json_pretty()`.
`Replay::from_json` validates format/configuration compatibility and
`Replay::verify` reconstructs a fresh simulation at 1,000 Hz.

Timeline ticks are monotonic even when a restart resets the core's internal
simulation tick. Frames omit empty ticks; playback recreates them. A verifier
reports exact event/checkpoint/final hash mismatches.

## Frontend requirements

Frontends must record authoritative `Command` batches, not keyboard events.
Macroquad begins recording on game start, freezes the replay timeline while
paused, and saves completed sessions with F5 under `replays/`. Its title-screen
Replay browser loads compatible files through `ReplayPlayer`, verifies each
tick during playback, and keeps viewer pause separate from recorded pause
commands.

Pygame also records and plays these same files through Python bindings to the
core recorder and player. F5 saves completed games; the Replay menu browses
`replays/`, and viewer pause does not advance or alter the simulation.

The terminal frontend also records and plays the same files. Its keyboard-only
Replay menu supports viewer pause, replay restart, and return to title.

Replay configuration includes the complete game-mode variant. Macroquad uses
the active mode identifier (`marathon` or `sprint`) as the default saved replay
filename prefix.

## Fixtures and compatibility

Fixtures live in `crates/tetradynamo-core/tests/fixtures` and are verified by
the Rust test suite. Format, simulation compatibility, and state/event hash
algorithm versions are checked strictly. A simulation behavior or canonical
hash change requires a compatibility/version decision and fixture refresh.
