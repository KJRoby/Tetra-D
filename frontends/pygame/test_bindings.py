import tetradynamo
from main import focus_loss_commands, visible_piece_cells


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


def test_focus_loss_never_requests_resume_and_hidden_rows_are_clipped():
    assert "resume" not in focus_loss_commands()
    assert visible_piece_cells(3, 0, [(1, 0), (1, 2), (1, 22)]) == [(4, 0)]
