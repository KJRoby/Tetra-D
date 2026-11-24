# Determinism

Dynamo Core advances at a fixed 1,000 Hz. Identical initial seed,
`GameConfig`, and ordered command batches at identical ticks produce identical
state and events.

The PRNG is seed-normalized so seed zero is valid. Restart restores the initial
seed and preserves the selected configuration. Frontends may choose a new seed
outside the simulation and pass it to `start_with_seed` or `restart_with_seed`;
that selected seed is exposed by snapshots.

`state_hash()` includes visible state plus PRNG/queue, held input, timers,
pending clears, active-piece rotation history, hold state, configuration,
mode progress, lifecycle phase, phase timer, and paused source phase. It is
intended for regression and replay verification, not a cryptographic digest.

Replays record a seed, immutable configuration, ordered command batches,
lifecycle operations, event hashes, and periodic state-hash checkpoints. The
monotonic replay timeline remains unambiguous when a lifecycle restart resets
the core's internal simulation tick.
