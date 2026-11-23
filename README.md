# Tetra|D

Tetra|D is a deterministic falling-block game with a Rust simulation core and a
Ratatui/Crossterm terminal frontend.

## Run

```powershell
cargo run -p tetrad-term
```

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
| P | Pause |
| R | Restart after game over or completion |
| Esc | Quit |

Terminal key-release support is not portable. Directional input is therefore
implemented as press/repeat movement, which remains reliable in Windows
Terminal, macOS Terminal, and common Linux terminals.
