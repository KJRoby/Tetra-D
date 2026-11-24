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
paused, and saves completed sessions with F5 under `replays/`. Playback UI and
replay browsing are future work; `ReplayPlayer` is the frontend-neutral
playback API today.

## Fixtures and compatibility

Fixtures live in `crates/tetradynamo-core/tests/fixtures` and are verified by
the Rust test suite. Format, simulation compatibility, and state/event hash
algorithm versions are checked strictly. A simulation behavior or canonical
hash change requires a compatibility/version decision and fixture refresh.
