# Game Modes

## Implemented: Marathon and Sprint

`GameModeConfig::marathon()` completes at 150 lines, advances one level every
10 lines, and caps at level 15. It retains Monad, Dyad, Triad, and Tetra|D
terminology.

`GameModeConfig::sprint()` completes after 40 cleared lines. It uses fixed
level-1 gravity for the entire session: lines do not advance its speed or emit
level-change events. Sprint otherwise uses the selected mechanical ruleset's
normal scoring, line-clear delay, hold, and piece behavior.

Game modes define objectives and progression. They do not own piece movement,
rotation, scoring mechanics, rendering, or input-device handling.

## Planned

Future modes can be introduced as additional `GameModeConfig` variants while
reusing a mechanical ruleset.
