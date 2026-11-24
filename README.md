# Tetra|D

Tetra|D is an early-alpha deterministic falling-block game with a reusable Rust
simulation core plus Ratatui/Crossterm, Pygame, and Macroquad frontends.

The core defaults to the Guideline mechanical ruleset and Marathon game mode.
See the [architecture](docs/ARCHITECTURE.md), [core API](docs/CORE_API.md),
[rulesets](docs/RULESETS.md), [game modes](docs/GAME_MODES.md),
[determinism](docs/DETERMINISM.md), [frontend guide](docs/FRONTEND_GUIDE.md),
[replay](docs/REPLAY.md), and [roadmap](docs/ROADMAP.md) for implementation
details and limitations.

## Run

```powershell
cargo run -p tetrad-term
```

## Native Macroquad frontend

```powershell
cargo run -p tetrad-macroquad
```

The Macroquad frontend is a native Rust desktop implementation that uses Dynamo
Core directly; it does not require Python or the PyO3 binding.
Completed Macroquad games can be saved as verified replay JSON with F5. Files
are written under `replays/`; choose REPLAY from the Macroquad title menu to
browse and deterministically play them back.

## Python and Pygame

From the repository root on Windows, the quickest way to build/update and
launch the frontend is:

```powershell
.\Run-Pygame.ps1
```

`Run-Pygame.cmd` is an equivalent double-click/Command Prompt launcher.
`Update-Pygame.ps1` creates the local `.venv`, refreshes dependencies, and
rebuilds the binding without launching the game.

The Python bindings are built from `crates/tetrad-python` with Maturin and
expose the `tetradynamo` module. They are the only game-state dependency of
the Pygame frontend.

```powershell
python -m venv .venv
.\.venv\Scripts\Activate.ps1
python -m pip install maturin
maturin develop --manifest-path crates/tetrad-python/Cargo.toml
python -m pip install -r frontends/pygame/requirements.txt
python frontends/pygame/main.py
```

`Game.step(commands)` accepts an ordered batch of lowercase commands such as
`"rotate_cw"` and `"hard_drop"`, advances exactly one 1,000 Hz simulation
tick, and returns events plus a read-only snapshot dictionary. The binding also
offers `start_with_seed`, `restart_with_seed`, and `return_to_title` for
frontend session flow.

The title screen uses a white `TETRA|` and red `D`. Gameplay is deliberately
monochrome red (`#FF0000`) on black, with HOLD to the left of the well, five
NEXT previews to the right, and controls in the lower-right corner.

## Controls

| Keys | Action |
| --- | --- |
| Left/Right or A/D | Move |
| Up/X and Z | Clockwise / counter-clockwise rotate |
| Down/S | Soft drop |
| Space | Hard drop |
| C/H | Hold |
| P or Esc | Pause/resume |
| Up/Down, Enter/Space | Pause-menu selection |
| R | Restart after game over or completion |

After starting Marathon, the core presents a 1.5-second **GET READY** stage and
a three-second countdown. Pausing hides the well visually; resuming uses the
same three-second countdown. Enter the standard Konami sequence
(`Up Up Down Down Left Right Left Right B A`) on the title screen to choose a
decimal or `0x` hexadecimal seed. Enter it while paused to toggle a visible,
locked seed for reproducible restarts.

Terminal key-release support is not portable. Directional input is therefore
implemented as press/repeat movement, which remains reliable in Windows
Terminal, macOS Terminal, and common Linux terminals.

## License and attribution

Tetra|D is independently authored and distributed under the [MIT License](LICENSE).
Copyright (c) 2026 KJRoby.

Tetra|D is not affiliated with, endorsed by, or sponsored by The Tetris Company
or any other trademark holder. This repository contains no third-party game
source code, art, audio, logos, or branding assets. It uses common
falling-block game conventions implemented independently.

See [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for direct dependency
notices.
