//! Thin Python adapter for Dynamo Core; it owns no gameplay rules.

use pyo3::{
    exceptions::PyValueError,
    prelude::*,
    types::{PyDict, PyList},
};
use tetradynamo_core::{
    shape_cells, Clear, ClearNotice, Command, Game, GameEvent, GameSnapshot, PieceKind, Spin,
};

#[pyclass(name = "Game")]
struct PyGame {
    game: Game,
}

#[pymethods]
impl PyGame {
    #[new]
    #[pyo3(signature = (seed = 0))]
    fn new(seed: u64) -> Self {
        Self {
            game: Game::new(seed),
        }
    }

    fn step(&mut self, py: Python<'_>, commands: Vec<String>) -> PyResult<Py<PyAny>> {
        let parsed: PyResult<Vec<_>> = commands.iter().map(|value| parse_command(value)).collect();
        let result = self
            .game
            .step(&parsed?)
            .map_err(|error| PyValueError::new_err(format!("{error:?}")))?;
        let output = PyDict::new(py);
        output.set_item("events", events_to_py(py, &result.events))?;
        output.set_item("snapshot", snapshot_to_py(py, &result.snapshot)?)?;
        Ok(output.into_any().unbind())
    }

    fn snapshot(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(snapshot_to_py(py, &self.game.snapshot())?
            .into_any()
            .unbind())
    }

    fn state_hash(&self) -> u64 {
        self.game.state_hash()
    }
    fn restart(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let result = self.game.restart();
        let output = PyDict::new(py);
        output.set_item("events", events_to_py(py, &result.events))?;
        output.set_item("snapshot", snapshot_to_py(py, &result.snapshot)?)?;
        Ok(output.into_any().unbind())
    }
}

#[pymodule]
fn tetradynamo(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyGame>()?;
    module.add("TICK_HZ", tetradynamo_core::TICK_HZ)?;
    module.add(
        "COMMANDS",
        vec![
            "left_press",
            "left_release",
            "right_press",
            "right_release",
            "soft_drop_press",
            "soft_drop_release",
            "rotate_cw",
            "rotate_ccw",
            "hard_drop",
            "hold",
            "pause",
            "resume",
            "start",
            "restart",
        ],
    )?;
    Ok(())
}

fn parse_command(value: &str) -> PyResult<Command> {
    match value {
        "left_press" => Ok(Command::LeftPress),
        "left_release" => Ok(Command::LeftRelease),
        "right_press" => Ok(Command::RightPress),
        "right_release" => Ok(Command::RightRelease),
        "soft_drop_press" => Ok(Command::SoftDropPress),
        "soft_drop_release" => Ok(Command::SoftDropRelease),
        "rotate_cw" => Ok(Command::RotateCw),
        "rotate_ccw" => Ok(Command::RotateCcw),
        "hard_drop" => Ok(Command::HardDrop),
        "hold" => Ok(Command::Hold),
        "pause" => Ok(Command::Pause),
        "resume" => Ok(Command::Resume),
        "start" => Ok(Command::Start),
        "restart" => Ok(Command::Restart),
        _ => Err(PyValueError::new_err(format!(
            "unknown Dynamo command: {value}"
        ))),
    }
}

fn piece_name(piece: PieceKind) -> &'static str {
    match piece {
        PieceKind::I => "I",
        PieceKind::J => "J",
        PieceKind::L => "L",
        PieceKind::O => "O",
        PieceKind::S => "S",
        PieceKind::T => "T",
        PieceKind::Z => "Z",
    }
}
fn clear_name(clear: Clear) -> &'static str {
    clear.label()
}
fn spin_name(spin: Spin) -> &'static str {
    match spin {
        Spin::Full => "full",
        Spin::Mini => "mini",
    }
}

fn snapshot_to_py<'py>(py: Python<'py>, snapshot: &GameSnapshot) -> PyResult<Bound<'py, PyDict>> {
    let value = PyDict::new(py);
    let board = PyList::empty(py);
    for row in snapshot.board {
        let cells = PyList::empty(py);
        for cell in row {
            cells.append(cell.map(piece_name))?;
        }
        board.append(cells)?;
    }
    let active = PyDict::new(py);
    active.set_item("kind", piece_name(snapshot.active.kind))?;
    active.set_item("rotation", snapshot.active.rotation)?;
    active.set_item("x", snapshot.active.x)?;
    active.set_item("y", snapshot.active.y)?;
    active.set_item(
        "cells",
        shape_cells(snapshot.active.kind, snapshot.active.rotation).to_vec(),
    )?;
    let timers = PyDict::new(py);
    timers.set_item("simulation_ticks", snapshot.timers.simulation_ticks)?;
    timers.set_item(
        "active_gameplay_ticks",
        snapshot.timers.active_gameplay_ticks,
    )?;
    timers.set_item("mode_elapsed_ticks", snapshot.timers.mode_elapsed_ticks)?;
    timers.set_item("gravity_ticks", snapshot.timers.gravity_ticks)?;
    timers.set_item("lock_ticks", snapshot.timers.lock_ticks)?;
    timers.set_item("clear_ticks", snapshot.timers.clear_ticks)?;
    timers.set_item("clear_notice_ticks", snapshot.timers.clear_notice_ticks)?;
    value.set_item("board", board)?;
    value.set_item("active", active)?;
    value.set_item("ghost_y", snapshot.ghost_y)?;
    value.set_item("hold", snapshot.hold.map(piece_name))?;
    value.set_item(
        "next",
        snapshot
            .next
            .iter()
            .copied()
            .map(piece_name)
            .collect::<Vec<_>>(),
    )?;
    value.set_item("phase", format!("{:?}", snapshot.phase).to_lowercase())?;
    value.set_item("score", snapshot.score)?;
    value.set_item("lines", snapshot.lines)?;
    value.set_item("level", snapshot.level)?;
    value.set_item("combo", snapshot.combo)?;
    value.set_item("back_to_back", snapshot.back_to_back)?;
    value.set_item("gravity_cps", snapshot.gravity_cps)?;
    value.set_item("clearing_rows", &snapshot.clearing_rows)?;
    value.set_item("timers", timers)?;
    if let Some(ClearNotice {
        clear,
        points,
        spin,
        back_to_back,
    }) = snapshot.clear_notice
    {
        value.set_item(
            "clear_notice",
            (clear_name(clear), points, spin.map(spin_name), back_to_back),
        )?;
    } else {
        value.set_item("clear_notice", py.None())?;
    }
    Ok(value)
}

fn events_to_py<'py>(py: Python<'py>, events: &[GameEvent]) -> Bound<'py, PyList> {
    let list = PyList::empty(py);
    for event in events {
        let value = PyDict::new(py);
        match event {
            GameEvent::Started => {
                value.set_item("type", "started").unwrap();
            }
            GameEvent::Restarted { seed } => {
                value.set_item("type", "restarted").unwrap();
                value.set_item("seed", seed).unwrap();
            }
            GameEvent::Paused => {
                value.set_item("type", "paused").unwrap();
            }
            GameEvent::Resumed => {
                value.set_item("type", "resumed").unwrap();
            }
            GameEvent::Spawned { kind } => {
                value.set_item("type", "spawned").unwrap();
                value.set_item("kind", piece_name(*kind)).unwrap();
            }
            GameEvent::Moved { dx, dy } => {
                value.set_item("type", "moved").unwrap();
                value.set_item("dx", dx).unwrap();
                value.set_item("dy", dy).unwrap();
            }
            GameEvent::Rotated { rotation, kick } => {
                value.set_item("type", "rotated").unwrap();
                value.set_item("rotation", rotation).unwrap();
                value.set_item("kick", kick).unwrap();
            }
            GameEvent::Held {
                outgoing,
                incoming,
                hold,
            } => {
                value.set_item("type", "held").unwrap();
                value.set_item("outgoing", piece_name(*outgoing)).unwrap();
                value.set_item("incoming", piece_name(*incoming)).unwrap();
                value.set_item("hold", hold.map(piece_name)).unwrap();
            }
            GameEvent::HardDropped { distance } => {
                value.set_item("type", "hard_dropped").unwrap();
                value.set_item("distance", distance).unwrap();
            }
            GameEvent::Locked { kind } => {
                value.set_item("type", "locked").unwrap();
                value.set_item("kind", piece_name(*kind)).unwrap();
            }
            GameEvent::LinesCleared {
                rows,
                clear,
                spin,
                back_to_back,
            } => {
                value.set_item("type", "lines_cleared").unwrap();
                value.set_item("rows", rows).unwrap();
                value.set_item("clear", clear_name(*clear)).unwrap();
                value.set_item("spin", spin.map(spin_name)).unwrap();
                value.set_item("back_to_back", back_to_back).unwrap();
            }
            GameEvent::ScoreAwarded { points, total } => {
                value.set_item("type", "score_awarded").unwrap();
                value.set_item("points", points).unwrap();
                value.set_item("total", total).unwrap();
            }
            GameEvent::LevelChanged { from, to } => {
                value.set_item("type", "level_changed").unwrap();
                value.set_item("from", from).unwrap();
                value.set_item("to", to).unwrap();
            }
            GameEvent::Combo { count } => {
                value.set_item("type", "combo").unwrap();
                value.set_item("count", count).unwrap();
            }
            GameEvent::BackToBack { active } => {
                value.set_item("type", "back_to_back").unwrap();
                value.set_item("active", active).unwrap();
            }
            GameEvent::GameOver => {
                value.set_item("type", "game_over").unwrap();
            }
            GameEvent::Completed => {
                value.set_item("type", "completed").unwrap();
            }
        }
        list.append(value).unwrap();
    }
    list
}
