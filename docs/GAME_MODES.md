# Game Modes

## Implemented: Marathon

`GameModeConfig::marathon()` completes at 150 lines, advances one level every
10 lines, and caps at level 15. It retains Monad, Dyad, Triad, and Tetra|D
terminology.

Game modes define objectives and progression. They do not own piece movement,
rotation, scoring mechanics, rendering, or input-device handling.

## Planned

Future modes can be introduced as additional `GameModeConfig` variants while
reusing a mechanical ruleset.
