import tetradynamo
from pathlib import Path
from main import (
    BOARD_WIDTH,
    BOARD_X,
    LOGICAL_WIDTH,
    WELL_CENTER_X,
    chain_number,
    countdown_label,
    clear_notice_fragments,
    clearing_cell_is_white,
    focus_loss_commands,
    level_flash_visible,
    visible_piece_cells,
)


def test_well_layout_is_horizontally_centered():
    assert BOARD_X == (LOGICAL_WIDTH - BOARD_WIDTH) // 2
    assert WELL_CENTER_X == LOGICAL_WIDTH // 2


def test_zero_seed_and_batched_commands_are_deterministic():
    first = tetradynamo.Game(0)
    second = tetradynamo.Game(0)
    for commands in (["start"], ["rotate_cw", "hard_drop"], [], []):
        first.step(commands)
        second.step(commands)
    assert first.state_hash() == second.state_hash()
    snapshot = first.snapshot()
    assert snapshot["timers"]["simulation_ticks"] == 4
    assert len(snapshot["next"]) >= 5


def test_explicit_seed_start_exposes_ready_phase_and_timer():
    game = tetradynamo.Game(0)
    result = game.start_with_seed(42)
    assert result["snapshot"]["seed"] == 42
    assert result["snapshot"]["phase"] == "ready"
    assert result["snapshot"]["timers"]["phase_remaining_ticks"] == 1500


def test_replay_fixture_verifies_through_python_binding():
    fixture = Path(__file__).parents[2] / "crates" / "tetradynamo-core" / "tests" / "fixtures" / "ready-v1.tdr.json"
    replay = tetradynamo.Replay.from_json(fixture.read_text(encoding="utf-8"))
    assert replay.verify()["phase"] == "ready"


def test_recorded_sprint_round_trips_and_plays_through_core():
    recorder = tetradynamo.ReplayRecorder(42, "sprint")
    recorder.start_with_seed(42)
    for _ in range(100):
        recorder.step([])
    replay = tetradynamo.Replay.from_json(recorder.replay().to_json())
    assert replay.metadata()["mode"] == "sprint"
    player = replay.player()
    while player.advance() is not None:
        pass
    assert player.finish() == replay.verify()["final_state_hash"]


def test_focus_loss_never_requests_resume_and_hidden_rows_are_clipped():
    assert "resume" not in focus_loss_commands()
    assert visible_piece_cells(3, 0, [(1, 0), (1, 2), (1, 22)]) == [(4, 0)]


def test_clear_effect_chain_and_level_flash_helpers():
    assert clearing_cell_is_white([21], 0, 21, 0)
    assert clearing_cell_is_white([21], 249, 21, 9)
    assert not clearing_cell_is_white([21], 25, 21, 0)
    assert not clearing_cell_is_white([21], 250, 21, 9)
    assert chain_number(1) is None
    assert chain_number(2) == 1
    assert level_flash_visible(500)
    assert not level_flash_visible(375)
    assert level_flash_visible(250)
    assert clear_notice_fragments(("TETRA|D", 1200, "full", True)) == [
        ("B2B TSPIN TETRA|", (255, 255, 255)),
        ("D", (255, 0, 0)),
        ("  +1200", (255, 255, 255)),
    ]


def test_countdown_labels_use_core_phase_timer():
    assert countdown_label({"phase": "ready", "timers": {"phase_remaining_ticks": 1500}}) == "GET READY"
    assert countdown_label({"phase": "startcountdown", "timers": {"phase_remaining_ticks": 3000}}) == "3"
    assert countdown_label({"phase": "resumecountdown", "timers": {"phase_remaining_ticks": 1001}}) == "2"
    assert countdown_label({"phase": "falling", "timers": {"phase_remaining_ticks": 0}}) is None
