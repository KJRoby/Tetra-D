# Core API

## Construction

`Game::new(seed)` creates the default Guideline Marathon game.
`Game::with_config(seed, GameConfig)` creates the same deterministic engine
with explicit rules and mode configuration.

`GameConfig::guideline_marathon()` is the default composition.

## Stepping

`Game::step(&[Command])` accepts an ordered batch and advances exactly one
simulation tick. A batch larger than `MAX_COMMANDS_PER_TICK` returns
`StepError::TooManyCommands` without advancing state.

`StepResult` contains ordered `GameEvent` values and a read-only `GameSnapshot`.
Snapshots contain rendering state only; frontends must not implement mechanics.

## Compatibility

The existing `Game::new`, `step`, `snapshot`, `restart`, `state_hash`, command,
event, and snapshot contracts remain supported. Python retains its existing
default `Game(seed=0)` constructor and string command API.
