# Rulesets

## Implemented: Guideline

`RulesetConfig::guideline()` provides the current mechanical behavior:

- Guideline SRS rotation and kick tables.
- Deterministic seven-bag randomizer.
- Guideline gravity curve.
- 500 ms lock and clear delays, 1,000 ms clear notice, and 15 lock resets.
- 167 ms DAS, 33 ms ARR, and the current soft-drop interval.
- One hold per active piece, T-spin recognition, combo/B2B scoring, and the
  current 3/2 B2B multiplier.

Rulesets intentionally exclude objectives and level progression; those belong
to game modes.

## Planned

Additional rotation, randomizer, gravity, scoring, and spin variants will be
added as enum variants and configuration data. Runtime plugin traits are not
part of the current design.
