# Core API

## Construction

`Game::new(seed)` creates the default Guideline Marathon game.
`Game::with_config(seed, GameConfig)` creates the same deterministic engine
with explicit rules and mode configuration.

`GameConfig::guideline_marathon()` is the default composition.

`start_with_seed(seed)` begins a new session in `Ready` with an explicit seed.
`restart_with_seed(seed)` does the same while emitting a restart event, and
`return_to_title()` resets the session to `Title` without changing the selected
configuration. These are useful when a frontend owns seed selection.

## Stepping

`Game::step(&[Command])` accepts an ordered batch and advances exactly one
simulation tick. A batch larger than `MAX_COMMANDS_PER_TICK` returns
`StepError::TooManyCommands` without advancing state.

`StepResult` contains ordered `GameEvent` values and a read-only `GameSnapshot`.
Snapshots contain rendering state only; frontends must not implement mechanics.

The lifecycle phases are `Title`, `Ready`, `StartCountdown`, `Falling`,
`Clearing`, `Paused`, `ResumeCountdown`, `GameOver`, and `Complete`.
`TimingConfig` controls the ready, start-countdown, and resume-countdown
durations. `Timers::phase_remaining_ticks` gives frontends the current phase
countdown without duplicating timing logic.

`Command::Pause` is accepted only while falling or clearing. `Resume` creates a
resume countdown; `Restart` and `ReturnToTitle` are accepted from paused,
game-over, and complete states. Relevant lifecycle events are emitted in the
same deterministic tick as their transitions.

## Compatibility

The existing `Game::new`, `step`, `snapshot`, `restart`, `state_hash`, command,
event, and snapshot contracts remain supported. Python retains its existing
default `Game(seed=0)` constructor and string command API.

## Replay

`ReplayRecorder` captures lifecycle methods and `step` command batches at the
authoritative core boundary. `Replay` serializes strict versioned JSON and
`verify()` performs headless deterministic playback. See [Replay](REPLAY.md).
